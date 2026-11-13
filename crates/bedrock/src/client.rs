//! The Messages call: request body, headers, Bearer or SigV4 auth, and the retry loop of
//! anthropic-sdk-go v1.51.0 (internal/requestconfig/requestconfig.go).

use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::http::{Response, SendError, Transport, Url};
use crate::json::{Kind, Value, parse_first, write_string};
use crate::sha::{hex, sha256};
use crate::time::{Utc, parse_rfc1123};
use crate::{ApiError, Config, Error, MAX_NOTE_BYTES, TOOL_NAME, creds, proxy, sigv4};

#[cfg(test)]
mod tests;

/// graphify-go internal/semantic/bedrock.go:61-64, verbatim.
pub(crate) const SYSTEM_PROMPT: &str = "You extract the key concepts an engineering or design note is about, for a knowledge graph that links related notes.
Given one note, identify the 1-6 most central concepts (systems, tools, services, or topics \u{2014} e.g. \"AppConfig\", \"Flux\", \"forge\", \"build environment\", \"CI dependency management\").
Prefer concept names that other notes about the same system would also use, so notes about the same topic converge on the same concept.
Call the record_concepts tool exactly once. Do not invent concepts the note does not discuss.";

const BASE_URL_ENV: &str = "ANTHROPIC_BEDROCK_MANTLE_BASE_URL";
const USER_AGENT: &str = concat!("graphify-rs-bedrock/", env!("CARGO_PKG_VERSION"));

enum Auth {
    Bearer(String),
    SigV4 {
        source: creds::Source,
        cached: Mutex<creds::Credentials>,
    },
}

/// A Bedrock mantle Messages client.
pub struct Client {
    cfg: Config,
    /// The messages URL as text, for error messages.
    endpoint: String,
    url: Url,
    auth: Auth,
    transport: Transport,
    proxy: Option<Url>,
}

impl Client {
    /// Resolves the base URL and auth. With SigV4, fetches credentials now (awsauth.go:192).
    pub fn new(cfg: Config) -> Result<Client, Error> {
        // awsauth.go:80-145: the auth mode first, then the region (SigV4 only), then the base URL.
        let key = cfg
            .env
            .get("AWS_BEARER_TOKEN_BEDROCK")
            .or_else(|| cfg.env.get("ANTHROPIC_AWS_API_KEY"))
            .map(str::to_string);
        if key.is_none() && cfg.region.is_empty() {
            return Err(Error::Config(
                "no AWS region found; set AWSRegion in ClientConfig or set the AWS_REGION environment variable"
                    .to_string(),
            ));
        }
        let base = match cfg.env.get(BASE_URL_ENV) {
            Some(b) => b.to_string(),
            None if !cfg.region.is_empty() => {
                format!("https://bedrock-mantle.{}.api.aws/anthropic", cfg.region)
            }
            None => {
                return Err(Error::Config(format!(
                    "no base URL found; set BaseURL or AWSRegion in ClientConfig, or set the {BASE_URL_ENV} or AWS_REGION environment variable"
                )));
            }
        };
        let endpoint = join_base(&base).map_err(Error::Config)?;
        let url = Url::parse(&endpoint).map_err(Error::Config)?;
        let proxy = proxy::Config::from_env(&cfg.env)
            .proxy_for(&url)
            .map_err(Error::Config)?;
        let transport = Transport::new(cfg.extra_root_pem.as_deref()).map_err(Error::Config)?;
        let auth = match key {
            Some(k) => Auth::Bearer(k),
            None => {
                let (source, creds) = creds::resolve(&cfg.env).map_err(Error::Credentials)?;
                Auth::SigV4 {
                    source,
                    cached: Mutex::new(creds),
                }
            }
        };
        Ok(Client {
            cfg,
            endpoint,
            url,
            auth,
            transport,
            proxy,
        })
    }

    /// Asks the model for the note's concepts. Returns the raw JSON text of the
    /// `record_concepts` tool input, or None when the response has no such block.
    pub fn record_concepts(
        &self,
        note_file: &str,
        note_content: &str,
    ) -> Result<Option<String>, Error> {
        // bedrock.go:128-134: cut by bytes, possibly mid-rune; write_string encodes each bad byte.
        let content = note_content.as_bytes();
        let body = request_body(note_file, &content[..content.len().min(MAX_NOTE_BYTES)]);
        let body = body.as_bytes();
        let mut state = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64)
            | 1;
        // xorshift64; the bound is never 0.
        let mut jitter = |n: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % n.max(1)
        };

        let mut retry = 0;
        let res = loop {
            let deadline = Instant::now() + self.cfg.attempt_timeout;
            let headers = self.headers(retry, body)?;
            let sent =
                match self
                    .transport
                    .send(&self.url, self.proxy.as_ref(), &headers, body, deadline)
                {
                    Ok(r) => Ok(r),
                    Err(SendError::Io(msg)) => Err(msg),
                    // requestconfig.go:450-452: no retry after the attempt's deadline.
                    Err(SendError::Timeout) => return Err(self.timeout()),
                };
            if retry < self.cfg.max_retries && should_retry(sent.as_ref().ok()) {
                let delay = retry_delay(sent.as_ref().ok(), retry, SystemTime::now(), &mut jitter);
                // requestconfig.go:475-479: the wait runs on the attempt's context.
                match Instant::now().checked_add(delay) {
                    Some(at) if at <= deadline => {}
                    _ => return Err(self.timeout()),
                }
                std::thread::sleep(delay);
                retry += 1;
                continue;
            }
            break sent;
        };
        let res = res.map_err(|msg| Error::Transport {
            url: self.endpoint.clone(),
            msg,
        })?;
        let text = String::from_utf8_lossy(&res.body);
        if res.status >= 400 {
            return Err(Error::Api(ApiError {
                status: res.status,
                url: self.endpoint.clone(),
                request_id: res
                    .header("request-id")
                    .filter(|id| !id.is_empty())
                    .map(str::to_string),
                body: text.into_owned(),
            }));
        }
        // requestconfig.go:540-552.
        let content_type = res.header("content-type").unwrap_or("");
        let media = content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !(media.contains("application/json") || media.ends_with("+json")) {
            return Err(Error::Decode(format!(
                "expected destination type of 'string' or '[]byte' for responses with content-type '{content_type}' that is not 'application/json'"
            )));
        }
        let root = parse_first(&text)
            .map_err(|e| Error::Decode(format!("error parsing response json: {e}")))?;
        let Some(Kind::Array(blocks)) = root.get("content").map(|v| &v.kind) else {
            return Ok(None);
        };
        // bedrock.go:115-121: the first record_concepts tool_use block.
        Ok(blocks
            .iter()
            .find(|b| {
                b.get("type").and_then(Value::as_str) == Some("tool_use")
                    && b.get("name").and_then(Value::as_str) == Some(TOOL_NAME)
            })
            .map(|b| b.get("input").map_or("", |i| i.raw(&text)).to_string()))
    }

    fn timeout(&self) -> Error {
        Error::Timeout {
            url: self.endpoint.clone(),
        }
    }

    /// The headers for one attempt (plan decision 5), signed when SigV4. `Host` and
    /// `Content-Length` are signed but not returned: `Transport::send` adds them.
    fn headers(&self, retry: u32, body: &[u8]) -> Result<Vec<(String, String)>, Error> {
        let mut headers: Vec<(String, String)> = [
            ("Content-Type", "application/json"),
            ("Accept", "application/json"),
            ("anthropic-version", "2023-06-01"),
            ("User-Agent", USER_AGENT),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        headers.push(("X-Stainless-Retry-Count".to_string(), retry.to_string()));
        headers.push((
            "X-Stainless-Timeout".to_string(),
            self.cfg.attempt_timeout.as_secs().to_string(),
        ));
        let (source, cached) = match &self.auth {
            Auth::Bearer(key) => {
                headers.push(("Authorization".to_string(), format!("Bearer {key}")));
                return Ok(headers);
            }
            Auth::SigV4 { source, cached } => (source, cached),
        };
        // awsauth.go:226: refresh on expiry before each attempt.
        let creds = {
            let mut c = cached.lock().unwrap_or_else(PoisonError::into_inner);
            if c.expires.is_some_and(|t| t <= SystemTime::now()) {
                *c = creds::fetch(source, &self.cfg.env).map_err(Error::Credentials)?;
            }
            c.clone()
        };
        let mut signed = headers;
        signed.push(("Host".to_string(), self.url.host_header()));
        signed.push(("Content-Length".to_string(), body.len().to_string()));
        let mut req = sigv4::Request {
            method: "POST",
            path: &self.url.path,
            query: "",
            headers: signed,
            body_sha256_hex: hex(&sha256(body)),
        };
        let out = sigv4::sign(
            &mut req,
            &creds,
            &self.cfg.region,
            "bedrock-mantle",
            &Utc::from_system(SystemTime::now()),
            sigv4::Options {
                normalize_path: true,
                sign_session_token: true,
            },
        );
        // sign appended X-Amz-Date (and X-Amz-Security-Token) to req.headers.
        let mut headers = req.headers;
        headers.retain(|(k, _)| {
            !k.eq_ignore_ascii_case("Host") && !k.eq_ignore_ascii_case("Content-Length")
        });
        headers.push(("Authorization".to_string(), out.authorization));
        Ok(headers)
    }
}

/// The exact request body graphify-go sends (testdata/request_body.json).
pub(crate) fn request_body(note_file: &str, note_content: &[u8]) -> String {
    let mut note = format!("Note path: {note_file}\n\n").into_bytes();
    note.extend_from_slice(note_content);
    let mut out = String::from(r#"{"max_tokens":1024,"messages":[{"content":[{"text":"#);
    write_string(&mut out, &note);
    out.push_str(
        r#","type":"text"}],"role":"user"}],"model":"anthropic.claude-opus-4-8","system":[{"text":"#,
    );
    write_string(&mut out, SYSTEM_PROMPT.as_bytes());
    out.push_str(r##","type":"text"}],"tool_choice":{"name":"record_concepts","type":"tool"},"tools":[{"input_schema":{"properties":{"concepts":{"description":"The concepts the note discusses. 1-6 of the most central concepts.","items":{"additionalProperties":false,"properties":{"name":{"description":"Short canonical concept name (e.g. \"AppConfig\", \"Flux\", \"forge\", \"build environment\", \"CI dependency management\").","type":"string"},"relation":{"description":"How the note relates to the concept.","enum":["cites","conceptually_related_to","semantically_similar_to"],"type":"string"},"score":{"description":"Confidence the note is genuinely about this concept, 0.0-1.0.","type":"number"}},"required":["name","relation","score"],"type":"object"},"type":"array"}},"required":["concepts"],"type":"object"},"name":"record_concepts","description":"Record the key concepts this engineering/design note is about and how the note relates to each, for building a knowledge graph."}]}"##);
    out
}

/// The messages URL for a base URL (option/requestoption.go:148-152, requestconfig.go:391).
pub(crate) fn join_base(base: &str) -> Result<String, String> {
    Url::parse(base).map_err(|e| format!("requestoption: WithBaseURL failed to parse url {e}"))?;
    // Url::parse accepted it, so there is a scheme and an authority.
    let (scheme, rest) = base.split_once("://").unwrap_or_default();
    let rest = rest.split('#').next().unwrap_or("");
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    // Resolving `v1/messages` against the base drops its query.
    let mut path = tail.split('?').next().unwrap_or("").to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    Ok(format!("{scheme}://{authority}{path}v1/messages"))
}

/// requestconfig.go:248-273. `None` is a connection error.
pub(crate) fn should_retry(res: Option<&Response>) -> bool {
    let Some(res) = res else {
        return true;
    };
    match res.header("x-should-retry") {
        Some("true") => return true,
        Some("false") => return false,
        _ => {}
    }
    matches!(res.status, 408 | 409 | 429) || res.status >= 500
}

/// requestconfig.go:275-380. `jitter(n)` returns a value in `0..n` (nanoseconds).
pub(crate) fn retry_delay(
    res: Option<&Response>,
    retry: u32,
    now: SystemTime,
    jitter: &mut impl FnMut(u64) -> u64,
) -> Duration {
    if let Some(d) = res.and_then(|r| retry_after(r, now)) {
        return d;
    }
    let delay = (500_000_000u64 << retry.min(5)).min(8_000_000_000);
    Duration::from_nanos(delay - jitter(delay / 4))
}

/// requestconfig.go:122-167 parseRetryAfterHeader; a negative value is 0.
fn retry_after(res: &Response, now: SystemTime) -> Option<Duration> {
    for (name, per_second) in [("Retry-After-Ms", 1000.0), ("Retry-After", 1.0)] {
        let Some(v) = res.header(name).filter(|v| !v.is_empty()) else {
            continue;
        };
        if let Ok(f) = v.parse::<f64>() {
            // NaN.max(0.0) is 0.0; infinity or overflow saturates.
            return Some(
                Duration::try_from_secs_f64((f / per_second).max(0.0))
                    .unwrap_or(Duration::from_secs(u64::from(u32::MAX))),
            );
        }
        if name == "Retry-After"
            && let Some(t) = parse_rfc1123(v)
        {
            return Some(t.duration_since(now).unwrap_or_default());
        }
    }
    None
}

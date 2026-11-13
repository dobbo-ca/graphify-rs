//! AWS Signature Version 4, header mode (aws-sdk-go-v2 v1.30.3 aws/signer/v4; checked against the
//! aws-c-auth signing test suite).

use std::collections::BTreeMap;

use crate::creds::Credentials;
use crate::sha::{hex, hmac_sha256, sha256};
use crate::time::Utc;

#[cfg(test)]
mod tests;

/// The parts of an HTTP request the signature covers.
#[derive(Clone, Debug)]
pub(crate) struct Request<'a> {
    pub method: &'a str,
    /// The on-wire path, as sent in the request line (no query).
    pub path: &'a str,
    /// The raw query after `?`, or "".
    pub query: &'a str,
    /// Headers in send order, names as sent. `sign` appends `X-Amz-Date` and
    /// `X-Amz-Security-Token` here.
    pub headers: Vec<(String, String)>,
    pub body_sha256_hex: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Options {
    /// Remove dot segments and repeated `/` before encoding the path.
    pub normalize_path: bool,
    /// Add `X-Amz-Security-Token` before signing (Go) rather than after.
    pub sign_session_token: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SignOutput {
    pub canonical_request: String,
    pub string_to_sign: String,
    pub signature: String,
    /// The full `Authorization` header value.
    pub authorization: String,
}

/// Headers never signed (internal/v4/headers.go:4-13, lowercase).
const IGNORED: [&str; 4] = ["authorization", "user-agent", "x-amzn-trace-id", "expect"];

/// Percent-encodes every byte outside `A-Za-z0-9-._~` (and `/` when `keep_slash`), uppercase hex
/// (smithy-go encoding/httpbinding/path_replace.go:82-107).
fn encode(s: &[u8], keep_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s {
        if b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (keep_slash && b == b'/')
        {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Go `url.QueryUnescape`: `+` is a space, `%XX` a byte; a bad escape is kept literally.
fn query_unescape(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex_byte = || {
            let hi = char::from(*b.get(i + 1)?).to_digit(16)?;
            let lo = char::from(*b.get(i + 2)?).to_digit(16)?;
            u8::try_from(hi * 16 + lo).ok()
        };
        match b[i] {
            b'+' => out.push(b' '),
            b'%' => {
                if let Some(v) = hex_byte() {
                    out.push(v);
                    i += 2;
                } else {
                    out.push(b'%');
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    out
}

/// Removes `.`/`..` segments and repeated `/`; keeps a trailing `/` after a real segment
/// (aws-c-auth get-relative*, get-slash*, get-slashes-normalized vectors).
fn normalize(path: &str) -> String {
    let mut segs: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    let mut out = format!("/{}", segs.join("/"));
    if !segs.is_empty() && (path.ends_with('/') || path.ends_with("/.") || path.ends_with("/..")) {
        out.push('/');
    }
    out
}

/// Go `StripExcessSpaces` (internal/v4/util.go:12-47): trim, then runs of spaces become one.
fn strip_spaces(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut prev_space = false;
    for c in v.trim().chars() {
        if c == ' ' && prev_space {
            continue;
        }
        prev_space = c == ' ';
        out.push(c);
    }
    out
}

pub(crate) fn canonical_request(req: &Request<'_>, opts: Options) -> String {
    let path = if opts.normalize_path {
        normalize(req.path)
    } else {
        req.path.to_string()
    };
    // The on-wire path is escaped again, so `%3A` becomes `%253A` (Go's double escape).
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        encode(path.as_bytes(), true)
    };

    let mut pairs: Vec<(String, String)> = Vec::new();
    if !req.query.is_empty() {
        for kv in req.query.split('&') {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            pairs.push((
                encode(&query_unescape(k), false),
                encode(&query_unescape(v), false),
            ));
        }
    }
    pairs.sort();
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    // Sorted by lowercase name; duplicates join with `,` in send order.
    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (n, v) in &req.headers {
        let n = n.to_ascii_lowercase();
        if !IGNORED.contains(&n.as_str()) {
            headers.entry(n).or_default().push(strip_spaces(v));
        }
    }
    let mut canon_headers = String::new();
    for (n, vs) in &headers {
        canon_headers.push_str(&format!("{n}:{}\n", vs.join(",")));
    }
    let signed = signed_headers(&headers);

    format!(
        "{}\n{path}\n{query}\n{canon_headers}\n{signed}\n{}",
        req.method, req.body_sha256_hex
    )
}

fn signed_headers(headers: &BTreeMap<String, Vec<String>>) -> String {
    headers
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(";")
}

/// `scope` is `YYYYMMDD/<region>/<service>/aws4_request`.
pub(crate) fn string_to_sign(canonical: &str, t: &Utc, scope: &str) -> String {
    format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        t.amz_date(),
        hex(&sha256(canonical.as_bytes()))
    )
}

/// Signs `req`: adds `X-Amz-Date` (and the session token header) to `req.headers`, and returns
/// the signature. The caller adds `Authorization: <authorization>`.
pub(crate) fn sign(
    req: &mut Request<'_>,
    creds: &Credentials,
    region: &str,
    service: &str,
    t: &Utc,
    opts: Options,
) -> SignOutput {
    // v4.go:524-528: date and (optionally) token are added before canonicalisation.
    req.headers
        .retain(|(n, _)| !n.eq_ignore_ascii_case("x-amz-date"));
    req.headers.push(("X-Amz-Date".to_string(), t.amz_date()));
    let token = creds.session_token.as_deref().filter(|s| !s.is_empty());
    if let Some(tok) = token
        && opts.sign_session_token
    {
        req.headers
            .retain(|(n, _)| !n.eq_ignore_ascii_case("x-amz-security-token"));
        req.headers
            .push(("X-Amz-Security-Token".to_string(), tok.to_string()));
    }

    let canonical = canonical_request(req, opts);
    let date = t.short_date();
    let scope = format!("{date}/{region}/{service}/aws4_request");
    let sts = string_to_sign(&canonical, t, &scope);

    let k = hmac_sha256(
        format!("AWS4{}", creds.secret_access_key).as_bytes(),
        date.as_bytes(),
    );
    let k = hmac_sha256(&k, region.as_bytes());
    let k = hmac_sha256(&k, service.as_bytes());
    let k = hmac_sha256(&k, b"aws4_request");
    let signature = hex(&hmac_sha256(&k, sts.as_bytes()));

    // The signed-headers line is the second-to-last line of the canonical request.
    let signed = canonical.rsplit('\n').nth(1).unwrap_or_default();
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={signature}",
        creds.access_key_id
    );

    if let Some(tok) = token
        && !opts.sign_session_token
    {
        req.headers
            .retain(|(n, _)| !n.eq_ignore_ascii_case("x-amz-security-token"));
        req.headers
            .push(("X-Amz-Security-Token".to_string(), tok.to_string()));
    }

    SignOutput {
        canonical_request: canonical,
        string_to_sign: sts,
        signature,
        authorization,
    }
}

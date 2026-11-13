//! Go 1.27.4 `net/url`: `Parse`, `PathUnescape`, `URL.Hostname` and `URL.Port`. Go strings can hold
//! invalid UTF-8 (`%ff` in a host or path), so decoded fields are bytes. Call sites: graphify-go
//! `resolve.go:1367` (`PathUnescape`) and `security.go:66` (`Parse`, `Scheme`, `Hostname`).

use crate::strconv::quote;
use std::fmt;

/// Go `url.URL`, minus `OmitHost` (only `String` reads it) and the methods graphify-go never calls.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Url {
    pub scheme: String,
    pub opaque: Vec<u8>,
    pub user: Option<Userinfo>,
    pub host: Vec<u8>,
    pub path: Vec<u8>,
    pub raw_path: String,
    pub force_query: bool,
    pub raw_query: String,
    pub fragment: Vec<u8>,
    pub raw_fragment: String,
}

/// Go `url.Userinfo`; `password: None` is Go's `passwordSet == false`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Userinfo {
    pub username: Vec<u8>,
    pub password: Option<Vec<u8>>,
}

/// Go `url.Error`: `parse "<url>": <err>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub op: &'static str,
    pub url: String,
    pub err: ErrorKind,
}

impl fmt::Display for Error {
    /// Go `fmt.Sprintf("%s %q: %s", e.Op, e.URL, e.Err)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.op,
            quote(self.url.as_bytes()),
            self.err
        )
    }
}

/// Go `url.EscapeError`: the offending `%` escape, up to 3 bytes. Bytes, because Go's string can
/// hold a cut multi-byte character (`"%aé"` gives `EscapeError("%a\xc3")`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscapeError(pub Vec<u8>);

impl fmt::Display for EscapeError {
    /// `invalid URL escape "%zz"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URL escape {}", quote(&self.0))
    }
}

/// Go `url.InvalidHostError`: the offending byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidHostError(pub String);

impl fmt::Display for InvalidHostError {
    /// `invalid character " " in host name`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid character {} in host name",
            quote(self.0.as_bytes())
        )
    }
}

/// Every error value `Parse` can return (`getScheme`, `parse`, `parseAuthority`, `parseHost`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorKind {
    /// `missing protocol scheme`
    MissingScheme,
    /// `net/url: invalid control character in URL`
    InvalidControlCharacter,
    /// `first path segment in URL cannot contain colon`
    FirstSegmentColon,
    /// `net/url: invalid userinfo`
    InvalidUserinfo,
    /// `invalid IP-literal`
    InvalidIpLiteral,
    /// `missing ']' in host`
    MissingBracket,
    /// `invalid port ":x" after host`; holds the port with its colon.
    InvalidPort(String),
    /// `invalid host: <netip.ParseAddr error text>`
    InvalidHost(String),
    Escape(EscapeError),
    InvalidHostChar(InvalidHostError),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorKind::MissingScheme => f.write_str("missing protocol scheme"),
            ErrorKind::InvalidControlCharacter => {
                f.write_str("net/url: invalid control character in URL")
            }
            ErrorKind::FirstSegmentColon => {
                f.write_str("first path segment in URL cannot contain colon")
            }
            ErrorKind::InvalidUserinfo => f.write_str("net/url: invalid userinfo"),
            ErrorKind::InvalidIpLiteral => f.write_str("invalid IP-literal"),
            ErrorKind::MissingBracket => f.write_str("missing ']' in host"),
            ErrorKind::InvalidPort(p) => {
                write!(f, "invalid port {} after host", quote(p.as_bytes()))
            }
            ErrorKind::InvalidHost(e) => write!(f, "invalid host: {e}"),
            ErrorKind::Escape(e) => fmt::Display::fmt(e, f),
            ErrorKind::InvalidHostChar(e) => fmt::Display::fmt(e, f),
        }
    }
}

/// Go `encoding` (url.go gen_encoding_table.go:111), minus `encodeQueryComponent`, which only
/// `QueryUnescape` and `QueryEscape` use (not ported).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Path,
    PathSegment,
    Host,
    Zone,
    UserPassword,
    Fragment,
}

const UPPERHEX: &[u8; 16] = b"0123456789ABCDEF";

/// Go `unhex` (url.go:63). Precondition: `c` is a hex digit.
fn unhex(c: u8) -> u8 {
    9 * (c >> 6) + (c & 15)
}

/// Go `shouldEscape` (reference implementation, gen_encoding_table.go:139).
fn should_escape(c: u8, mode: Mode) -> bool {
    // §2.3 Unreserved characters (alphanum)
    if c.is_ascii_alphanumeric() {
        return false;
    }
    // §3.2.2 sub-delims, ':' for the port, '[' ']' for IPv6, and '<' '>' '"'.
    if matches!(mode, Mode::Host | Mode::Zone)
        && matches!(
            c,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'['
                | b']'
                | b'<'
                | b'>'
                | b'"'
        )
    {
        return false;
    }
    match c {
        // §2.3 Unreserved characters (mark)
        b'-' | b'_' | b'.' | b'~' => return false,
        // §2.2 Reserved characters: each section allows a few of them.
        b'$' | b'&' | b'+' | b',' | b'/' | b':' | b';' | b'=' | b'?' | b'@' => match mode {
            Mode::Path => return c == b'?',
            Mode::PathSegment => return matches!(c, b'/' | b';' | b',' | b'?'),
            Mode::UserPassword => return matches!(c, b'@' | b'/' | b'?' | b':'),
            Mode::Fragment => return false,
            Mode::Host | Mode::Zone => {}
        },
        _ => {}
    }
    if mode == Mode::Fragment && matches!(c, b'!' | b'(' | b')' | b'*') {
        return false;
    }
    true
}

/// Go `escape` (url.go:196) for the non-query modes: every byte `shouldEscape` selects becomes `%XX`.
fn escape(s: &[u8], mode: Mode) -> Vec<u8> {
    let mut t = Vec::with_capacity(s.len());
    for &c in s {
        if should_escape(c, mode) {
            t.push(b'%');
            t.push(UPPERHEX[usize::from(c >> 4)]);
            t.push(UPPERHEX[usize::from(c & 15)]);
        } else {
            t.push(c);
        }
    }
    t
}

/// Go `unescape` (url.go:106). `+` is kept in every mode here (Go turns it into a space only for
/// `encodeQueryComponent`, which is not ported).
fn unescape(s: &[u8], mode: Mode) -> Result<Vec<u8>, ErrorKind> {
    // Check that every % is well-formed.
    let mut i = 0;
    while i < s.len() {
        match s[i] {
            b'%' => {
                // url.go:112: not enough characters after %, or not hex.
                if i + 2 >= s.len()
                    || !s[i + 1].is_ascii_hexdigit()
                    || !s[i + 2].is_ascii_hexdigit()
                {
                    let end = s.len().min(i + 3);
                    return Err(ErrorKind::Escape(EscapeError(s[i..end].to_vec())));
                }
                // url.go:127: in a host, %-encoding only for non-ASCII bytes, and %25.
                if mode == Mode::Host && unhex(s[i + 1]) < 8 && s[i..i + 3] != *b"%25" {
                    return Err(ErrorKind::Escape(EscapeError(s[i..i + 3].to_vec())));
                }
                // url.go:132: in a zone, only escapes of bytes that are valid in a host (or space).
                if mode == Mode::Zone {
                    let v = (unhex(s[i + 1]) << 4) | unhex(s[i + 2]);
                    if s[i..i + 3] != *b"%25" && v != b' ' && should_escape(v, Mode::Host) {
                        return Err(ErrorKind::Escape(EscapeError(s[i..i + 3].to_vec())));
                    }
                }
                i += 3;
            }
            c => {
                // url.go:147: a raw ASCII byte a host or zone cannot hold.
                if matches!(mode, Mode::Host | Mode::Zone) && c < 0x80 && should_escape(c, mode) {
                    return Err(ErrorKind::InvalidHostChar(InvalidHostError(
                        char::from(c).to_string(),
                    )));
                }
                i += 1;
            }
        }
    }
    // url.go:164: decode.
    let mut t = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'%' {
            t.push((unhex(s[i + 1]) << 4) | unhex(s[i + 2]));
            i += 3;
        } else {
            t.push(s[i]);
            i += 1;
        }
    }
    Ok(t)
}

/// Go `url.PathUnescape`: `%XX` to a byte, `+` kept; the result can be invalid UTF-8.
pub fn path_unescape(s: &str) -> Result<Vec<u8>, EscapeError> {
    // url.go:100: `unescape(s, encodePathSegment)`; only the `%` check can fail in this mode.
    unescape(s.as_bytes(), Mode::PathSegment).map_err(|e| match e {
        ErrorKind::Escape(e) => e,
        _ => unreachable!("only host modes return other errors"),
    })
}

/// Go `validOptionalPort` (url.go:762): empty, or `:` and digits.
fn valid_optional_port(port: &[u8]) -> bool {
    match port.split_first() {
        None => true,
        Some((&b':', digits)) => digits.iter().all(u8::is_ascii_digit),
        Some(_) => false,
    }
}

/// Go `validUserinfo` (url.go:1283): ASCII letters, digits and `-._:~!$&'()*+,;=%@`.
fn valid_userinfo(s: &str) -> bool {
    s.bytes().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                b'-' | b'.'
                    | b'_'
                    | b':'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b'%'
                    | b'@'
            )
    })
}

/// Go `stringContainsCTLByte` (url.go:1317).
fn contains_ctl_byte(s: &str) -> bool {
    s.bytes().any(|b| b < b' ' || b == 0x7f)
}

/// `netip.parseAddrError.Error` (netip.go:146).
fn addr_error(input: &[u8], msg: &str, at: &[u8]) -> String {
    if at.is_empty() {
        format!("ParseAddr({}): {msg}", quote(input))
    } else {
        format!("ParseAddr({}): {msg} (at {})", quote(input), quote(at))
    }
}

/// `netip.parseIPv4Fields` (netip.go:154), checks only (the octets are not needed).
fn parse_ipv4_fields(input: &[u8], off: usize, end: usize) -> Result<(), String> {
    let s = &input[off..end];
    let (mut val, mut pos, mut dig_len) = (0u32, 0usize, 0usize);
    for (i, &c) in s.iter().enumerate() {
        if c.is_ascii_digit() {
            if dig_len == 1 && val == 0 {
                return Err(addr_error(
                    input,
                    "IPv4 field has octet with leading zero",
                    b"",
                ));
            }
            val = val * 10 + u32::from(c - b'0');
            dig_len += 1;
            if val > 255 {
                return Err(addr_error(input, "IPv4 field has value >255", b""));
            }
        } else if c == b'.' {
            // .1.2.3, 1.2.3., 1..2.3
            if i == 0 || i == s.len() - 1 || s[i - 1] == b'.' {
                return Err(addr_error(
                    input,
                    "IPv4 field must have at least one digit",
                    &s[i..],
                ));
            }
            // 1.2.3.4.5
            if pos == 3 {
                return Err(addr_error(input, "IPv4 address too long", b""));
            }
            pos += 1;
            val = 0;
            dig_len = 0;
        } else {
            return Err(addr_error(input, "unexpected character", &s[i..]));
        }
    }
    if pos < 3 {
        return Err(addr_error(input, "IPv4 address too short", b""));
    }
    Ok(())
}

/// `netip.parseIPv6` (netip.go:205), checks only: the zone is any non-empty string.
fn parse_ipv6(input: &[u8]) -> Result<(), String> {
    let mut s = input;
    // Split off the zone right from the start.
    let mut zone: &[u8] = b"";
    if let Some(i) = s.iter().position(|&c| c == b'%') {
        zone = &s[i + 1..];
        s = &s[..i];
        if zone.is_empty() {
            return Err(addr_error(input, "zone must be a non-empty string", b""));
        }
    }
    let mut ellipsis: Option<usize> = None; // position of ellipsis in the 16 bytes
    // Might have leading ellipsis.
    if s.len() >= 2 && s[0] == b':' && s[1] == b':' {
        ellipsis = Some(0);
        s = &s[2..];
        // Might be only ellipsis.
        if s.is_empty() {
            return Ok(());
        }
    }
    // Loop, parsing hex numbers followed by colon.
    let mut i = 0;
    while i < 16 {
        let mut off = 0;
        let mut acc = 0u32;
        while off < s.len() {
            let c = s[off];
            let d = match c {
                b'0'..=b'9' => c - b'0',
                b'a'..=b'f' => c - b'a' + 10,
                b'A'..=b'F' => c - b'A' + 10,
                _ => break,
            };
            acc = (acc << 4) + u32::from(d);
            if off > 3 {
                // more than 4 digits in group
                return Err(addr_error(
                    input,
                    "each group must have 4 or less digits",
                    s,
                ));
            }
            if acc > 0xffff {
                return Err(addr_error(input, "IPv6 field has value >=2^16", s));
            }
            off += 1;
        }
        if off == 0 {
            return Err(addr_error(
                input,
                "each colon-separated field must have at least one digit",
                s,
            ));
        }
        // If followed by dot, might be in trailing IPv4.
        if off < s.len() && s[off] == b'.' {
            if ellipsis.is_none() && i != 12 {
                return Err(addr_error(
                    input,
                    "embedded IPv4 address must replace the final 2 fields of the address",
                    s,
                ));
            }
            if i + 4 > 16 {
                return Err(addr_error(
                    input,
                    "too many hex fields to fit an embedded IPv4 at the end of the address",
                    s,
                ));
            }
            let mut end = input.len();
            if !zone.is_empty() {
                end -= zone.len() + 1;
            }
            parse_ipv4_fields(input, end - s.len(), end)?;
            s = b"";
            i += 4;
            break;
        }
        // Save this 16-bit chunk.
        i += 2;
        // Stop at end of string.
        s = &s[off..];
        if s.is_empty() {
            break;
        }
        // Otherwise must be followed by colon and more.
        if s[0] != b':' {
            return Err(addr_error(input, "unexpected character, want colon", s));
        } else if s.len() == 1 {
            return Err(addr_error(
                input,
                "colon must be followed by more characters",
                s,
            ));
        }
        s = &s[1..];
        // Look for ellipsis.
        if s[0] == b':' {
            if ellipsis.is_some() {
                return Err(addr_error(input, "multiple :: in address", s));
            }
            ellipsis = Some(i);
            s = &s[1..];
            if s.is_empty() {
                // can be at end
                break;
            }
        }
    }
    // Must have used entire string.
    if !s.is_empty() {
        return Err(addr_error(input, "trailing garbage after address", s));
    }
    // If didn't parse enough, an ellipsis must expand it; if enough, there must be none.
    if i < 16 {
        if ellipsis.is_none() {
            return Err(addr_error(input, "address string too short", b""));
        }
    } else if ellipsis.is_some() {
        return Err(addr_error(
            input,
            "the :: must expand to at least one field of zeros",
            b"",
        ));
    }
    Ok(())
}

/// `netip.ParseAddr` (netip.go:114); `Ok(true)` when the address is IPv4 (`Addr.Is4`).
fn parse_addr(s: &[u8]) -> Result<bool, String> {
    for &c in s {
        match c {
            b'.' => return parse_ipv4_fields(s, 0, s.len()).map(|()| true),
            b':' => return parse_ipv6(s).map(|()| false),
            // Assume this was an IPv6 address with a zone, but the address is missing.
            b'%' => return Err(addr_error(s, "missing IPv6 address", b"")),
            _ => {}
        }
    }
    Err(addr_error(s, "unable to parse IP", b""))
}

/// GODEBUG `urlstrictcolons` (url.go:30) as graphify-go runs it: `go 1.25.0` in its go.mod gives 0.
const URLSTRICTCOLONS: bool = false;

/// Go `parseHost` (url.go:549): `host` is `host[:port]` without user information. The result is
/// the unescaped host.
fn parse_host(scheme: &str, host: &str) -> Result<Vec<u8>, ErrorKind> {
    if let Some(open) = host.rfind('[') {
        if open > 0 {
            return Err(ErrorKind::InvalidIpLiteral);
        }
        // url.go:553: an IP-Literal in RFC 3986 and RFC 6874, e.g. "[fe80::1%25en0]:80".
        let close = host.rfind(']').ok_or(ErrorKind::MissingBracket)?;
        let colon_port = &host[close + 1..];
        if !valid_optional_port(colon_port.as_bytes()) {
            return Err(ErrorKind::InvalidPort(colon_port.to_string()));
        }
        let unescaped_colon_port = unescape(colon_port.as_bytes(), Mode::Host)?;
        let hostname = &host[open + 1..close];
        // url.go:582: %25 introduces the zone, which may use any (restricted) %-encoding.
        let mut unescaped = match hostname.find("%25") {
            Some(zone_idx) => {
                let mut v = unescape(&hostname.as_bytes()[..zone_idx], Mode::Host)?;
                v.extend(unescape(&hostname.as_bytes()[zone_idx..], Mode::Zone)?);
                v
            }
            None => unescape(hostname.as_bytes(), Mode::Host)?,
        };
        // url.go:608: only a host that is a valid IPv6 address can be in square brackets.
        if parse_addr(&unescaped).map_err(ErrorKind::InvalidHost)? {
            return Err(ErrorKind::InvalidIpLiteral);
        }
        unescaped.insert(0, b'[');
        unescaped.push(b']');
        unescaped.extend(unescaped_colon_port);
        return Ok(unescaped);
    }
    if let Some(first) = host.find(':') {
        let mut i = first;
        let last_colon = host.rfind(':').unwrap_or(first);
        // url.go:610-626: with several colons the port starts at the last one, except for http and
        // https under urlstrictcolons=1. graphify-go's go.mod says `go 1.25.0`, so its GODEBUG
        // default is urlstrictcolons=0 (the Go oracle's go.mod sets `godebug default=go1.25`):
        // every scheme takes the last colon.
        let strict = URLSTRICTCOLONS && (scheme == "http" || scheme == "https");
        if last_colon != first && !strict {
            i = last_colon;
        }
        let colon_port = &host[i..];
        if !valid_optional_port(colon_port.as_bytes()) {
            return Err(ErrorKind::InvalidPort(colon_port.to_string()));
        }
    }
    unescape(host.as_bytes(), Mode::Host)
}

/// Go `parseAuthority` (url.go:512): `[userinfo@]host`.
fn parse_authority(
    scheme: &str,
    authority: &str,
) -> Result<(Option<Userinfo>, Vec<u8>), ErrorKind> {
    let at = authority.rfind('@');
    let host = parse_host(scheme, at.map_or(authority, |i| &authority[i + 1..]))?;
    let Some(at) = at else {
        return Ok((None, host));
    };
    let userinfo = &authority[..at];
    if !valid_userinfo(userinfo) {
        return Err(ErrorKind::InvalidUserinfo);
    }
    let user = match userinfo.split_once(':') {
        None => Userinfo {
            username: unescape(userinfo.as_bytes(), Mode::UserPassword)?,
            password: None,
        },
        Some((username, password)) => Userinfo {
            username: unescape(username.as_bytes(), Mode::UserPassword)?,
            password: Some(unescape(password.as_bytes(), Mode::UserPassword)?),
        },
    };
    Ok((Some(user), host))
}

/// Go `getScheme` (url.go:369): `(scheme, rest)`, or `("", raw)` when there is no scheme.
fn get_scheme(raw: &str) -> Result<(&str, &str), ErrorKind> {
    for (i, c) in raw.bytes().enumerate() {
        match c {
            b'a'..=b'z' | b'A'..=b'Z' => {}
            b'0'..=b'9' | b'+' | b'-' | b'.' => {
                if i == 0 {
                    return Ok(("", raw));
                }
            }
            b':' => {
                if i == 0 {
                    return Err(ErrorKind::MissingScheme);
                }
                return Ok((&raw[..i], &raw[i + 1..]));
            }
            // An invalid character: there is no valid scheme.
            _ => return Ok(("", raw)),
        }
    }
    Ok(("", raw))
}

/// Go `URL.setPath` (url.go:660): `raw_path` is kept only when it is not the default encoding.
fn set_path(url: &mut Url, p: &str) -> Result<(), ErrorKind> {
    let path = unescape(p.as_bytes(), Mode::Path)?;
    url.raw_path = if escape(&path, Mode::Path) == p.as_bytes() {
        String::new()
    } else {
        p.to_string()
    };
    url.path = path;
    Ok(())
}

/// Go `URL.setFragment` (url.go:727).
fn set_fragment(url: &mut Url, f: &str) -> Result<(), ErrorKind> {
    let frag = unescape(f.as_bytes(), Mode::Fragment)?;
    url.raw_fragment = if escape(&frag, Mode::Fragment) == f.as_bytes() {
        String::new()
    } else {
        f.to_string()
    };
    url.fragment = frag;
    Ok(())
}

/// Go `parse(rawURL, false)` (url.go:432): `viaRequest` is false, so its branches are left out.
/// `OmitHost` is not a field (url.go:495).
fn parse_url(raw_url: &str) -> Result<Url, ErrorKind> {
    if contains_ctl_byte(raw_url) {
        return Err(ErrorKind::InvalidControlCharacter);
    }
    let mut url = Url::default();
    if raw_url == "*" {
        url.path = b"*".to_vec();
        return Ok(url);
    }
    // url.go:455: split off a leading "http:", "mailto:", etc.
    let (scheme, mut rest) = get_scheme(raw_url)?;
    url.scheme = scheme.to_ascii_lowercase();
    // url.go:462
    if rest.ends_with('?') && rest.matches('?').count() == 1 {
        url.force_query = true;
        rest = &rest[..rest.len() - 1];
    } else if let Some((r, q)) = rest.split_once('?') {
        rest = r;
        url.raw_query = q.to_string();
    }
    if !rest.starts_with('/') {
        if !url.scheme.is_empty() {
            // url.go:471: a rootless path is opaque.
            url.opaque = rest.as_bytes().to_vec();
            return Ok(url);
        }
        // url.go:483: the first segment of a relative path cannot contain a colon.
        let segment = rest.split('/').next().unwrap_or("");
        if segment.contains(':') {
            return Err(ErrorKind::FirstSegmentColon);
        }
    }
    // url.go:489
    if (!url.scheme.is_empty() || !rest.starts_with("///")) && rest.starts_with("//") {
        let mut authority = &rest[2..];
        rest = "";
        if let Some(i) = authority.find('/') {
            rest = &authority[i..];
            authority = &authority[..i];
        }
        let (user, host) = parse_authority(&url.scheme, authority)?;
        url.user = user;
        url.host = host;
    }
    // url.go:505
    set_path(&mut url, rest)?;
    Ok(url)
}

/// Go `url.Parse` (url.go:399).
pub fn parse(raw: &str) -> Result<Url, Error> {
    // Cut off #frag.
    let (u, frag) = raw.split_once('#').unwrap_or((raw, ""));
    let mut url = parse_url(u).map_err(|err| Error {
        op: "parse",
        url: u.to_string(),
        err,
    })?;
    if frag.is_empty() {
        return Ok(url);
    }
    set_fragment(&mut url, frag).map_err(|err| Error {
        op: "parse",
        url: raw.to_string(),
        err,
    })?;
    Ok(url)
}

/// Go `splitHostPort` (url.go:1209): the port must be numeric, else the whole input is the host.
fn split_host_port(host_port: &[u8]) -> (&[u8], &[u8]) {
    let mut host = host_port;
    let mut port: &[u8] = b"";
    if let Some(colon) = host.iter().rposition(|&c| c == b':')
        && valid_optional_port(&host[colon..])
    {
        host = &host_port[..colon];
        port = &host_port[colon + 1..];
    }
    if host.starts_with(b"[") && host.ends_with(b"]") {
        host = &host[1..host.len() - 1];
    }
    (host, port)
}

impl Url {
    /// Go `URL.Hostname`: the host without port, and without `[` `]` around an IPv6 literal.
    pub fn hostname(&self) -> &[u8] {
        split_host_port(&self.host).0
    }

    /// Go `URL.Port`: the numeric port without the colon, or empty.
    pub fn port(&self) -> &[u8] {
        split_host_port(&self.host).1
    }
}

//! AWS Signature Version 4, header mode (aws-sdk-go-v2 v1.30.3 aws/signer/v4; checked against the
//! aws-c-auth signing test suite).

use crate::creds::Credentials;
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

pub(crate) fn canonical_request(req: &Request<'_>, opts: Options) -> String {
    todo!("graphify-rs-u2o.5")
}

/// `scope` is `YYYYMMDD/<region>/<service>/aws4_request`.
pub(crate) fn string_to_sign(canonical: &str, t: &Utc, scope: &str) -> String {
    todo!("graphify-rs-u2o.5")
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
    todo!("graphify-rs-u2o.5")
}

//! SHA-256 (FIPS 180-4) and HMAC-SHA256 (RFC 2104), scalar.
//!
//! Private and small on purpose: graphify-rs-5sz swaps `sha256` for `gostd::sha256` when it lands.
//! Other modules use only the three functions below.

#[cfg(test)]
mod tests;

pub(crate) fn sha256(data: &[u8]) -> [u8; 32] {
    todo!("graphify-rs-u2o.3")
}

pub(crate) fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    todo!("graphify-rs-u2o.3")
}

/// Lowercase hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    todo!("graphify-rs-u2o.3")
}

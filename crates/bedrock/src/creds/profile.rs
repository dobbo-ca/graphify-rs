//! Shared config and credentials files: load, rename sections, merge (aws-sdk-go-v2
//! config v1.27.27 shared_config.go:687-894).

use std::collections::BTreeMap;

use crate::env::Env;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Profile {
    pub name: String,
    /// Merged across files, keys lowercased.
    pub keys: BTreeMap<String, String>,
    /// The config and credentials file paths used (for error messages).
    pub config_path: String,
    pub credentials_path: String,
}

/// The selected profile (`AWS_PROFILE`, `AWS_DEFAULT_PROFILE`, `default`). `Ok(None)` when the
/// profile does not exist and `AWS_PROFILE` is unset.
pub(crate) fn load(env: &Env) -> Result<Option<Profile>, String> {
    todo!("graphify-rs-u2o.6")
}

/// Every section after renaming and merging, by profile name (for the ported config tests).
pub(crate) fn load_all(
    config_files: &[&str],
    credentials_files: &[&str],
) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    todo!("graphify-rs-u2o.6")
}

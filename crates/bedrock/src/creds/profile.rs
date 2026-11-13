//! Shared config and credentials files: load, rename sections, merge (aws-sdk-go-v2
//! config v1.27.27 shared_config.go:687-894).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::ini::{self, IniValue, Section, Sections};
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

const PROFILE_PREFIX: &str = "profile ";
const ACCESS_KEY_ID: &str = "aws_access_key_id";
const SECRET_ACCESS_KEY: &str = "aws_secret_access_key";

/// The keys `mergeSections` copies into an existing section (shared_config.go:830-859), after the
/// session token.
const MERGED_KEYS: [&str; 27] = [
    "role_arn",
    "source_profile",
    "credential_source",
    "external_id",
    "mfa_serial",
    "role_session_name",
    "region",
    "endpoint_discovery_enabled",
    "credential_process",
    "web_identity_token_file",
    "s3_use_arn_region",
    "s3_disable_multiregion_access_points",
    "ec2_metadata_service_endpoint_mode",
    "ec2_metadata_service_endpoint",
    "ec2_metadata_v1_disabled",
    "use_dualstack_endpoint",
    "use_fips_endpoint",
    "defaults_mode",
    "retry_mode",
    "ca_bundle",
    "duration_seconds",
    "max_attempts",
    "sso_session",
    "sso_account_id",
    "sso_region",
    "sso_role_name",
    "sso_start_url",
];

/// Which profile to read and where (env_config.go:24-50, 101-105).
pub(crate) struct Selection {
    pub name: String,
    /// `AWS_PROFILE` is set: a missing profile is an error.
    pub strict: bool,
    /// `AWS_PROFILE` or `AWS_DEFAULT_PROFILE` is set.
    pub named: bool,
    pub config_path: String,
    pub credentials_path: String,
}

pub(crate) fn selection(env: &Env) -> Selection {
    let profile = env.get("AWS_PROFILE");
    let named = profile.or_else(|| env.get("AWS_DEFAULT_PROFILE"));
    // shareddefaults: HOME/.aws/{config,credentials}; no `~` expansion; an empty HOME gives a
    // relative path, as filepath.Join does.
    let aws_dir = Path::new(env.get("HOME").unwrap_or("")).join(".aws");
    let path = |var: &str, file: &str| {
        env.get(var).map_or_else(
            || aws_dir.join(file).to_string_lossy().into_owned(),
            str::to_string,
        )
    };
    Selection {
        name: named.unwrap_or("default").to_string(),
        strict: profile.is_some(),
        named: named.is_some(),
        config_path: path("AWS_CONFIG_FILE", "config"),
        credentials_path: path("AWS_SHARED_CREDENTIALS_FILE", "credentials"),
    }
}

/// A section and the error Go keeps in `Section.Errors` (a partial-credentials pair).
struct Entry {
    section: Section,
    err: Option<String>,
}

type Table = BTreeMap<String, Entry>;

/// The selected profile (`AWS_PROFILE`, `AWS_DEFAULT_PROFILE`, `default`). `Ok(None)` when the
/// profile does not exist and `AWS_PROFILE` is unset.
pub(crate) fn load(env: &Env) -> Result<Option<Profile>, String> {
    let sel = selection(env);
    let table = load_table(
        &[sel.config_path.as_str()],
        &[sel.credentials_path.as_str()],
    );
    let Some(entry) = table.get(&sel.name) else {
        if sel.strict {
            // SharedConfigProfileNotExistError (shared_config.go:1455)
            return Err(format!("failed to get shared config profile, {}", sel.name));
        }
        return Ok(None);
    };
    // shared_config.go:1043-1055 (Go wraps this in "Error using profile: ...")
    // Named by the selected profile: the merge ran before config sections lost their "profile ".
    if entry.err.is_some() {
        return Err(format!(
            "partial credentials found for profile {}",
            sel.name
        ));
    }
    Ok(Some(Profile {
        name: sel.name,
        keys: string_keys(&entry.section),
        config_path: sel.config_path,
        credentials_path: sel.credentials_path,
    }))
}

/// Every section after renaming and merging, by profile name (for the ported config tests).
pub(crate) fn load_all(
    config_files: &[&str],
    credentials_files: &[&str],
) -> Result<BTreeMap<String, BTreeMap<String, String>>, String> {
    let table = load_table(config_files, credentials_files);
    Ok(table
        .into_iter()
        .map(|(name, e)| (name, string_keys(&e.section)))
        .collect())
}

/// Go `Section.String(k)`: a nested block reads as "".
fn string_keys(section: &Section) -> BTreeMap<String, String> {
    section
        .values
        .keys()
        .map(|k| (k.clone(), section.get(k).unwrap_or("").to_string()))
        .collect()
}

/// shared_config.go:557-596 order: load and clean each kind, then merge credentials into config.
fn load_table(config_files: &[&str], credentials_files: &[&str]) -> Table {
    let mut config = load_ini_files(config_files);
    process_config_sections(&mut config);
    let mut credentials = load_ini_files(credentials_files);
    process_credentials_sections(&mut credentials);
    merge_sections(&mut config, credentials);
    config
}

/// shared_config.go:773-796: a file that cannot be read is skipped.
fn load_ini_files(paths: &[&str]) -> Table {
    let mut merged = Table::new();
    for path in paths {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let sections: Sections = ini::parse(&String::from_utf8_lossy(&bytes));
        let table = sections
            .sections
            .into_iter()
            .map(|(name, section)| (name, Entry { section, err: None }))
            .collect();
        merge_sections(&mut merged, table);
    }
    merged
}

/// shared_config.go:687-725: keep `profile X` (renamed to `X`, replacing a plain `X`),
/// `sso-session ` and `services ` sections and `default`; drop the rest.
fn process_config_sections(table: &mut Table) {
    let names: Vec<String> = table.keys().cloned().collect();
    let mut skip = BTreeSet::new();
    for name in names {
        if skip.contains(&name) {
            continue;
        }
        if let Some(new_name) = name.strip_prefix(PROFILE_PREFIX) {
            if let Some(entry) = table.remove(&name) {
                table.insert(new_name.to_string(), entry);
            }
            skip.insert(new_name.to_string());
        } else {
            let keep = name.starts_with("sso-session ")
                || name.starts_with("services ")
                || name.eq_ignore_ascii_case("default");
            if !keep {
                table.remove(&name);
            }
        }
    }
}

/// shared_config.go:754-771
fn process_credentials_sections(table: &mut Table) {
    table.retain(|name, _| !name.starts_with(PROFILE_PREFIX));
}

/// shared_config.go:798-894. A section new to `dst` is copied whole; for an existing one only
/// a complete key pair and the listed keys are taken, and the error is `src`'s.
fn merge_sections(dst: &mut Table, src: Table) {
    for (name, mut src_entry) in src {
        let src_sec = &src_entry.section;
        let has_id = src_sec.values.contains_key(ACCESS_KEY_ID);
        let has_secret = src_sec.values.contains_key(SECRET_ACCESS_KEY);
        if has_id != has_secret {
            src_entry.err = Some(String::new());
        }
        let Some(dst_entry) = dst.get_mut(&name) else {
            dst.insert(name, src_entry);
            continue;
        };
        dst_entry.err = src_entry.err.take();
        let src_sec = &src_entry.section;
        let dst_sec = &mut dst_entry.section;
        // mergeStringKey: a present key replaces the value, as a plain string.
        let mut merge_key = |key: &str| {
            if src_sec.values.contains_key(key) {
                let v = src_sec.get(key).unwrap_or("").to_string();
                dst_sec.values.insert(key.to_string(), IniValue::Str(v));
            }
        };
        if has_id && has_secret {
            merge_key(ACCESS_KEY_ID);
            merge_key(SECRET_ACCESS_KEY);
            merge_key("aws_session_token");
        }
        for key in MERGED_KEYS {
            merge_key(key);
        }
    }
}

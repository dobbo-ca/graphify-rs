use crate::creds::process;
use crate::creds::profile::{load, load_all};
use crate::creds::{Source, resolve};
use crate::env::Env;
use crate::testutil::{testdata, tmp_dir, write_script};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

// Fake keys only: the AWS documentation example values.
const AKID: &str = "AKIDEXAMPLE";
const SECRET: &str = "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY";
const FAKE_PROCESS_JSON: &str = r#"{"Version":1,"AccessKeyId":"AKIDEXAMPLE","SecretAccessKey":"wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY","SessionToken":"FAKESESSIONTOKEN"}"#;

/// A PATH that finds `sh`, `cat` and `sleep`.
const SYSTEM_PATH: &str = "/usr/bin:/bin";

type Profiles = BTreeMap<String, BTreeMap<String, String>>;

/// `crates/bedrock/testdata/config/<name>`.
fn cfg(name: &str) -> String {
    testdata(&format!("config/{name}"))
        .to_string_lossy()
        .into_owned()
}

/// Go's fixtures run `cat ./testdata/<f>` from the package dir. The test copies the fixture
/// with that prefix pointed at the absolute fixture dir (the fixture itself is not edited).
fn rewritten(fixture: &str) -> String {
    let text = std::fs::read_to_string(testdata(&format!("config/{fixture}"))).unwrap();
    let abs = format!("{}/", testdata("config").display());
    let path = tmp_dir("fixture").join(fixture);
    std::fs::write(&path, text.replace("./testdata/", &abs)).unwrap();
    path.to_string_lossy().into_owned()
}

/// Writes `config` and `credentials` files to a new temp dir; returns their paths.
fn write_files(config: &str, creds: &str) -> (String, String) {
    let dir = tmp_dir("aws");
    let config_path = dir.join("config");
    let creds_path = dir.join("credentials");
    std::fs::write(&config_path, config).unwrap();
    std::fs::write(&creds_path, creds).unwrap();
    (
        config_path.to_string_lossy().into_owned(),
        creds_path.to_string_lossy().into_owned(),
    )
}

/// An Env with HOME, both file paths and an empty PATH dir (so no `aws` is found), plus `extra`
/// (which wins). Nothing reads the real ~/.aws.
fn env_for(config: &str, creds: &str, extra: &[(&str, &str)]) -> Env {
    let home = tmp_dir("home").to_string_lossy().into_owned();
    let empty_path = tmp_dir("path").to_string_lossy().into_owned();
    let mut pairs = vec![
        ("HOME", home.as_str()),
        ("PATH", empty_path.as_str()),
        ("AWS_CONFIG_FILE", config),
        ("AWS_SHARED_CREDENTIALS_FILE", creds),
    ];
    pairs.extend_from_slice(extra);
    Env::from_pairs(pairs)
}

/// An executable `aws` in `dir` that appends its argv to `dir/argv` and prints process JSON.
fn fake_aws(dir: &Path) {
    let argv = dir.join("argv");
    write_script(
        dir,
        "aws",
        &format!(
            "echo \"$@\" >> '{}'\necho '{FAKE_PROCESS_JSON}'",
            argv.display()
        ),
    );
}

/// The lines the fake `aws` recorded, one per call.
fn argv_lines(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("argv"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// `load_all` over fixtures in testdata/config.
fn all(config: &[&str], creds: &[&str]) -> Profiles {
    let config_paths: Vec<String> = config.iter().copied().map(cfg).collect();
    let creds_paths: Vec<String> = creds.iter().copied().map(cfg).collect();
    let config_refs: Vec<&str> = config_paths.iter().map(String::as_str).collect();
    let creds_refs: Vec<&str> = creds_paths.iter().map(String::as_str).collect();
    load_all(&config_refs, &creds_refs)
        .unwrap_or_else(|e| panic!("load_all({config:?}, {creds:?}): {e}"))
}

fn expect_keys(profiles: &Profiles, case: &str, name: &str, want: &[(&str, &str)]) {
    let keys = profiles
        .get(name)
        .unwrap_or_else(|| panic!("{case}: profile {name} is missing"));
    for (k, v) in want {
        assert_eq!(
            keys.get(*k).map(String::as_str),
            Some(*v),
            "{case}: profile {name} key {k}"
        );
    }
}

fn expect_absent(profiles: &Profiles, case: &str, name: &str, absent: &[&str]) {
    if let Some(keys) = profiles.get(name) {
        for k in absent {
            assert!(
                !keys.contains_key(*k),
                "{case}: profile {name} must not have {k}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// config@v1.27.27 env_config_test.go
// ---------------------------------------------------------------------------------------------

struct EnvCredsCase {
    env: &'static [(&'static str, &'static str)],
    /// (access key, secret, session token; "" for none), or None when Go's Credentials are empty.
    want: Option<(&'static str, &'static str, &'static str)>,
}

const ENV_CREDS_CASES: [EnvCredsCase; 8] = [
    EnvCredsCase {
        env: &[("AWS_ACCESS_KEY", "AKID")],
        want: None,
    },
    EnvCredsCase {
        env: &[("AWS_ACCESS_KEY_ID", "AKID")],
        want: None,
    },
    EnvCredsCase {
        env: &[("AWS_SECRET_KEY", "SECRET")],
        want: None,
    },
    EnvCredsCase {
        env: &[("AWS_SECRET_ACCESS_KEY", "SECRET")],
        want: None,
    },
    EnvCredsCase {
        env: &[
            ("AWS_ACCESS_KEY_ID", "AKID"),
            ("AWS_SECRET_ACCESS_KEY", "SECRET"),
        ],
        want: Some(("AKID", "SECRET", "")),
    },
    EnvCredsCase {
        env: &[("AWS_ACCESS_KEY", "AKID"), ("AWS_SECRET_KEY", "SECRET")],
        want: Some(("AKID", "SECRET", "")),
    },
    EnvCredsCase {
        env: &[
            ("AWS_ACCESS_KEY", "AKID"),
            ("AWS_SECRET_KEY", "SECRET"),
            ("AWS_SESSION_TOKEN", "TOKEN"),
        ],
        want: Some(("AKID", "SECRET", "TOKEN")),
    },
    // Go also reads AWS_ACCOUNT_ID; Credentials here has no account id, so it is ignored.
    EnvCredsCase {
        env: &[
            ("AWS_ACCESS_KEY_ID", "AKID"),
            ("AWS_SECRET_ACCESS_KEY", "SECRET"),
            ("AWS_ACCOUNT_ID", "012345678901"),
        ],
        want: Some(("AKID", "SECRET", "")),
    },
];

// Port of config@v1.27.27/env_config_test.go:20 TestNewEnvConfig_Creds
// A case Go leaves empty (a single key) falls through the chain here; with no profile and no
// `aws` on PATH it is an error.
#[test]
fn new_env_config_creds() {
    let (config, creds) = write_files("", "");
    for (i, c) in ENV_CREDS_CASES.iter().enumerate() {
        let env = env_for(&config, &creds, c.env);
        match (resolve(&env), c.want) {
            (Ok((src, got)), Some((id, secret, token))) => {
                assert_eq!(src, Source::Env, "case {i}: source");
                assert_eq!(got.access_key_id, id, "case {i}: access key id");
                assert_eq!(got.secret_access_key, secret, "case {i}: secret");
                assert_eq!(
                    got.session_token.as_deref().unwrap_or(""),
                    token,
                    "case {i}: session token"
                );
            }
            (Err(e), None) => assert!(
                e.contains("no AWS credentials found"),
                "case {i}: unexpected error {e}"
            ),
            (got, want) => panic!("case {i}: got {got:?}, want {want:?}"),
        }
    }
}

struct EnvProfileCase {
    n: u32,
    env: &'static [(&'static str, &'static str)],
    profile: &'static str,
}

const ENV_PROFILE_CASES: [EnvProfileCase; 8] = [
    EnvProfileCase {
        n: 0,
        env: &[("AWS_REGION", "region"), ("AWS_PROFILE", "profile")],
        profile: "profile",
    },
    EnvProfileCase {
        n: 1,
        env: &[
            ("AWS_REGION", "region"),
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_PROFILE", "profile"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "profile",
    },
    EnvProfileCase {
        n: 2,
        env: &[
            ("AWS_REGION", "region"),
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_PROFILE", "profile"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "profile",
    },
    EnvProfileCase {
        n: 3,
        env: &[
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "default_profile",
    },
    EnvProfileCase {
        n: 4,
        env: &[("AWS_REGION", "region"), ("AWS_PROFILE", "profile")],
        profile: "profile",
    },
    EnvProfileCase {
        n: 5,
        env: &[
            ("AWS_REGION", "region"),
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_PROFILE", "profile"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "profile",
    },
    EnvProfileCase {
        n: 6,
        env: &[
            ("AWS_REGION", "region"),
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_PROFILE", "profile"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "profile",
    },
    EnvProfileCase {
        n: 7,
        env: &[
            ("AWS_DEFAULT_REGION", "default_region"),
            ("AWS_DEFAULT_PROFILE", "default_profile"),
        ],
        profile: "default_profile",
    },
];

// Port of config@v1.27.27/env_config_test.go:115 TestNewEnvConfig
// Only the profile cases (0-7) and the files case (10); the region, CA bundle, endpoint,
// retry and S3 cases are not read by this crate. Case 10 ADAPT: real temp files instead of
// "/path/to/aws/file", because a profile must exist for `load` to return its paths.
#[test]
fn new_env_config() {
    let (config, creds) = write_files(
        "[profile profile]\nregion = profile_region\n\n[profile default_profile]\nregion = default_profile_region\n",
        "",
    );
    for c in &ENV_PROFILE_CASES {
        let env = env_for(&config, &creds, c.env);
        let p = load(&env)
            .unwrap_or_else(|e| panic!("case {}: {e}", c.n))
            .unwrap_or_else(|| panic!("case {}: no profile", c.n));
        assert_eq!(p.name, c.profile, "case {}: profile name", c.n);
    }

    // Case 10: AWS_SHARED_CREDENTIALS_FILE and AWS_CONFIG_FILE.
    let (config, creds) = write_files(
        "[default]\nregion = from_config\n",
        "[default]\nregion = from_creds\n",
    );
    let env = env_for(&config, &creds, &[]);
    let p = load(&env).unwrap().expect("case 10: default profile");
    assert_eq!(p.config_path, config, "case 10: config file");
    assert_eq!(p.credentials_path, creds, "case 10: credentials file");
}

// ---------------------------------------------------------------------------------------------
// config@v1.27.27 shared_config_other_test.go
// ---------------------------------------------------------------------------------------------

/// HOME with `.aws/config` and `.aws/credentials`, USERPROFILE set to something else, and no
/// AWS_*_FILE variables.
fn home_env() -> (Env, String) {
    let home = tmp_dir("home_dir");
    std::fs::create_dir_all(home.join(".aws")).unwrap();
    std::fs::write(
        home.join(".aws/config"),
        "[default]\nregion = home_region\n",
    )
    .unwrap();
    std::fs::write(
        home.join(".aws/credentials"),
        "[default]\nregion = home_region\n",
    )
    .unwrap();
    let home = home.to_string_lossy().into_owned();
    let env = Env::from_pairs([("HOME", home.as_str()), ("USERPROFILE", "profile_dir")]);
    (env, home)
}

// Port of config@v1.27.27/shared_config_other_test.go:14 TestSharedCredsFilename
// ADAPT: a temp HOME instead of "home_dir", and the path is read from the loaded profile.
#[test]
fn shared_creds_filename() {
    let (env, home) = home_env();
    let p = load(&env).unwrap().expect("default profile");
    assert_eq!(
        p.credentials_path,
        format!("{home}/.aws/credentials"),
        "shared creds filename"
    );
}

// Port of config@v1.27.27/shared_config_other_test.go:29 TestSharedConfigFilename
// ADAPT: as above.
#[test]
fn shared_config_filename() {
    let (env, home) = home_env();
    let p = load(&env).unwrap().expect("default profile");
    assert_eq!(
        p.config_path,
        format!("{home}/.aws/config"),
        "shared config filename"
    );
}

// ---------------------------------------------------------------------------------------------
// config@v1.27.27 shared_config_test.go
// ---------------------------------------------------------------------------------------------

// Port of config@v1.27.27/shared_config_test.go:29 TestNewSharedConfig
// In scope: file selection, merge order, credentials, profile names, INI errors, and the raw
// keys the Go cases read. ADAPT: assume-role and SSO cases check that their keys are kept (the
// chain sends them to the AWS CLI, D-006). Skipped (typed parsing this crate does not do):
// S3 ARN region, MRAP, endpoint discovery, IMDS endpoint and mode, dual-stack, FIPS, defaults
// mode, retry, request compression, services section, account ID, IMDSv1. Skipped (not read
// here): the SSO-session parse (`sso-session success`) and the assume-role source chains,
// which need Go's source-profile resolution.
#[test]
fn new_shared_config() {
    let empty = "empty_creds_config";

    // "file not exist"
    let env = env_for(
        &cfg("file_not_exist"),
        &cfg(empty),
        &[("AWS_PROFILE", "default")],
    );
    let err = load(&env).expect_err("file not exist");
    assert!(
        err.contains("failed to get shared config profile"),
        "file not exist: {err}"
    );

    // "default profile"
    let env = env_for(
        &cfg("shared_config"),
        &cfg(empty),
        &[("AWS_PROFILE", "default")],
    );
    let p = load(&env).unwrap().expect("default profile");
    assert_eq!(p.name, "default", "default profile: name");
    assert_eq!(
        p.keys.get("region").map(String::as_str),
        Some("default_region"),
        "default profile: region"
    );

    // "multiple config files"
    let got = all(&["shared_config_other", "shared_config"], &[]);
    expect_keys(
        &got,
        "multiple config files",
        "config_file_load_order",
        &[
            ("region", "shared_config_region"),
            ("aws_access_key_id", "shared_config_akid"),
            ("aws_secret_access_key", "shared_config_secret"),
        ],
    );

    // "mutliple config files reverse order"
    let got = all(&["shared_config", "shared_config_other"], &[]);
    expect_keys(
        &got,
        "mutliple config files reverse order",
        "config_file_load_order",
        &[
            ("region", "shared_config_other_region"),
            ("aws_access_key_id", "shared_config_other_akid"),
            ("aws_secret_access_key", "shared_config_other_secret"),
        ],
    );

    let got = all(&["shared_config_other", "shared_config"], &[]);
    // "Assume role" (ADAPT: keys only)
    expect_keys(
        &got,
        "Assume role",
        "assume_role",
        &[
            ("role_arn", "assume_role_role_arn"),
            ("source_profile", "complete_creds"),
        ],
    );
    // "Assume role with invalid source profile" (ADAPT: Go errors; the keys are kept)
    expect_keys(
        &got,
        "Assume role with invalid source profile",
        "assume_role_invalid_source_profile",
        &[
            ("role_arn", "assume_role_invalid_source_profile_role_arn"),
            ("source_profile", "profile_not_exists"),
        ],
    );
    // "Assume role with creds"
    expect_keys(
        &got,
        "Assume role with creds",
        "assume_role_w_creds",
        &[
            ("role_arn", "assume_role_w_creds_role_arn"),
            ("external_id", "1234"),
            ("role_session_name", "assume_role_w_creds_session_name"),
            ("source_profile", "assume_role_w_creds"),
            ("aws_access_key_id", "assume_role_w_creds_akid"),
            ("aws_secret_access_key", "assume_role_w_creds_secret"),
        ],
    );
    // "AWS SSO Profile"
    expect_keys(
        &got,
        "AWS SSO Profile",
        "sso_creds",
        &[
            ("sso_account_id", "012345678901"),
            ("sso_region", "us-west-2"),
            ("sso_role_name", "TestRole"),
            ("sso_start_url", "https://127.0.0.1/start"),
        ],
    );
    // "AWS SSO Profile and Static Credentials"
    expect_keys(
        &got,
        "AWS SSO Profile and Static Credentials",
        "sso_and_static",
        &[
            ("aws_access_key_id", "sso_and_static_akid"),
            ("aws_secret_access_key", "sso_and_static_secret"),
            ("aws_session_token", "sso_and_static_token"),
            ("sso_account_id", "012345678901"),
            ("sso_region", "us-west-2"),
            ("sso_role_name", "TestRole"),
            (
                "sso_start_url",
                "https://THIS_SHOULD_NOT_BE_IN_TESTDATA_CACHE/start",
            ),
        ],
    );
    // "SSO Mixed with Additional Credential Providrer"
    expect_keys(
        &got,
        "SSO Mixed with Additional Credential Providrer",
        "sso_mixed_credproc",
        &[
            ("sso_account_id", "012345678901"),
            ("sso_region", "us-west-2"),
            ("sso_role_name", "TestRole"),
            ("sso_start_url", "https://127.0.0.1/start"),
            ("credential_process", "/path/to/process"),
        ],
    );

    // "Invalid INI file"
    let env = env_for(
        &cfg("shared_config_invalid_ini"),
        &cfg(empty),
        &[("AWS_PROFILE", "profile_name")],
    );
    let err = load(&env).expect_err("invalid ini");
    assert!(
        err.contains("failed to get shared config profile"),
        "Invalid INI file: {err}"
    );

    // Profile names are case-sensitive; credentials beat config, other keys merge.
    let got = all(&["shared_config"], &["shared_credentials"]);
    for (case, name, token, duration, region) in [
        (
            "profile names are case-sensitive (Mixed)",
            "DoNotNormalize",
            "DoNotNormalize",
            "1200",
            "eu-west-1",
        ),
        (
            "profile names are case-sensitive (lower)",
            "donotnormalize",
            "donotnormalize",
            "1500",
            "eu-west-2",
        ),
        (
            "profile names are case-sensitive (upper)",
            "DONOTNORMALIZE",
            "DONOTNORMALIZE",
            "1800",
            "eu-west-3",
        ),
    ] {
        let id = format!("{token}_credentials_akid");
        let secret = format!("{token}_credentials_secret");
        let session = format!("{token}_config_session_token");
        expect_keys(
            &got,
            case,
            name,
            &[
                ("aws_access_key_id", id.as_str()),
                ("aws_secret_access_key", secret.as_str()),
                ("aws_session_token", session.as_str()),
                ("duration_seconds", duration),
                ("region", region),
            ],
        );
    }
    // "source profile name is case-sensitive"
    expect_keys(
        &got,
        "source profile name is case-sensitive",
        "AssumeWithDoNotNormalize",
        &[
            ("role_arn", "AssumeWithDoNotNormalize_role_arn"),
            ("source_profile", "DoNotNormalize"),
        ],
    );

    // "ca bundle options"
    expect_keys(
        &got,
        "ca bundle options",
        "with_ca_bundle",
        &[("ca_bundle", "custom_ca_bundle_file.pem")],
    );

    // "merged profiles across files"
    expect_keys(
        &got,
        "merged profiles across files",
        "merged_profiles",
        &[
            ("role_arn", "creds_profile_arn"),
            ("duration_seconds", "1023"),
            ("sso_account_id", "0123456789"),
            ("sso_region", "us-west-2"),
            ("sso_role_name", "CredProfileRole"),
            (
                "sso_start_url",
                "https://my-sso-cred-profile-role.awsapps.com/start",
            ),
            ("ca_bundle", "/path/to/bundle.b"),
        ],
    );

    // "merged profiles across config files"
    let got = all(&["shared_config", "shared_config"], &[]);
    expect_keys(
        &got,
        "merged profiles across config files",
        "merged_profiles",
        &[
            ("role_arn", "config_profile_arn"),
            ("duration_seconds", "3601"),
            ("sso_account_id", "1234567890"),
            ("sso_region", "us-east-1"),
            ("sso_role_name", "ConfigProfileRole"),
            (
                "sso_start_url",
                "https://my-sso-config-profile-role.awsapps.com/start",
            ),
            ("ca_bundle", "/path/to/bundle.a"),
        ],
    );

    // "merged profiles across credentials files"
    let got = all(&[], &["shared_credentials", "shared_credentials"]);
    expect_keys(
        &got,
        "merged profiles across credentials files",
        "merged_profiles",
        &[
            ("role_arn", "creds_profile_arn"),
            ("duration_seconds", "1023"),
            ("sso_account_id", "0123456789"),
            ("sso_region", "us-west-2"),
            ("sso_role_name", "CredProfileRole"),
            (
                "sso_start_url",
                "https://my-sso-cred-profile-role.awsapps.com/start",
            ),
            ("ca_bundle", "/path/to/bundle.b"),
        ],
    );

    // "Profile with app ID" and "endpoint config test": raw keys.
    let got = all(&["shared_config"], &[]);
    expect_keys(
        &got,
        "Profile with app ID",
        "sdk_app_id",
        &[("sdk_ua_app_id", "12345")],
    );
    expect_keys(
        &got,
        "endpoint config test",
        "endpoint_config",
        &[
            ("ignore_configured_endpoint_urls", "true"),
            ("endpoint_url", "https://example.com"),
        ],
    );
}

struct SectionCase {
    case: &'static str,
    /// The section name without Go's `profile ` prefix.
    name: &'static str,
    want: &'static [(&'static str, &'static str)],
    absent: &'static [&'static str],
}

const SECTION_CASES: [SectionCase; 13] = [
    SectionCase {
        case: "Default as profile",
        name: "default",
        want: &[("region", "default_region")],
        absent: &[],
    },
    SectionCase {
        case: "prefixed profile",
        name: "alt_profile_name",
        want: &[("region", "alt_profile_name_region")],
        absent: &[],
    },
    SectionCase {
        case: "prefixed profile 2",
        name: "short_profile_name_first",
        want: &[("region", "short_profile_name_first_alt")],
        absent: &[],
    },
    SectionCase {
        case: "profile with partial creds",
        name: "partial_creds",
        want: &[("aws_access_key_id", "partial_creds_akid")],
        absent: &["aws_secret_access_key"],
    },
    SectionCase {
        case: "profile with role duration",
        name: "with_role_duration",
        want: &[("duration_seconds", "3601")],
        absent: &[],
    },
    SectionCase {
        case: "profile with complete creds",
        name: "complete_creds",
        want: &[
            ("aws_access_key_id", "complete_creds_akid"),
            ("aws_secret_access_key", "complete_creds_secret"),
        ],
        absent: &[],
    },
    SectionCase {
        case: "profile with complete creds and token",
        name: "complete_creds_with_token",
        want: &[
            ("aws_access_key_id", "complete_creds_with_token_akid"),
            ("aws_secret_access_key", "complete_creds_with_token_secret"),
            ("aws_session_token", "complete_creds_with_token_token"),
        ],
        absent: &[],
    },
    SectionCase {
        case: "complete profile",
        name: "full_profile",
        want: &[
            ("aws_access_key_id", "full_profile_akid"),
            ("aws_secret_access_key", "full_profile_secret"),
            ("region", "full_profile_region"),
        ],
        absent: &[],
    },
    SectionCase {
        case: "profile with partial assume role",
        name: "partial_assume_role",
        want: &[("role_arn", "partial_assume_role_role_arn")],
        absent: &[],
    },
    SectionCase {
        case: "profile using assume role",
        name: "assume_role",
        want: &[
            ("role_arn", "assume_role_role_arn"),
            ("source_profile", "complete_creds"),
        ],
        absent: &[],
    },
    SectionCase {
        case: "profile with assume role and MFA",
        name: "assume_role_w_mfa",
        want: &[
            ("role_arn", "assume_role_role_arn"),
            ("source_profile", "complete_creds"),
            ("mfa_serial", "0123456789"),
        ],
        absent: &[],
    },
    SectionCase {
        case: "does not exist",
        name: "does_not_exist",
        want: &[],
        absent: &["region"],
    },
    SectionCase {
        case: "profile with mixed casing",
        name: "with_mixed_case_keys",
        want: &[
            ("aws_access_key_id", "accessKey"),
            ("aws_secret_access_key", "secret"),
        ],
        absent: &[],
    },
];

// Port of config@v1.27.27/shared_config_test.go:792 TestLoadSharedConfigFromSection
// ADAPT: Go parses one section by its raw name (`profile alt_profile_name`); here the file is
// loaded by `load_all`, which renames sections, so cases use the name without the prefix.
// "does not exist" checks the profile is absent.
#[test]
fn load_shared_config_from_section() {
    let got = all(&["shared_config"], &[]);
    for c in &SECTION_CASES {
        if c.name == "does_not_exist" {
            assert!(
                !got.contains_key(c.name),
                "{}: profile {} must not exist",
                c.case,
                c.name
            );
            continue;
        }
        expect_keys(&got, c.case, c.name, c.want);
        expect_absent(&got, c.case, c.name, c.absent);
    }
}

// Port of config@v1.27.27/shared_config_test.go:933 TestLoadSharedConfig
// Cases 4-6 ADAPT: a missing default profile is `Ok(None)` (the chain goes on to the CLI), and
// a profile whose `source_profile` does not exist loads fine, because role profiles go to the
// AWS CLI (D-006) instead of Go's source-profile resolution.
#[test]
fn load_shared_config() {
    let empty = cfg("empty_creds_config");

    // Case 0: WithSharedConfigProfile("alt_profile_name"), default files = shared_config.
    // Case 1: the profile name through the default; the same result here.
    for (case, extra) in [
        ("0", [("AWS_PROFILE", "alt_profile_name")]),
        ("1", [("AWS_DEFAULT_PROFILE", "alt_profile_name")]),
    ] {
        let env = env_for(&cfg("shared_config"), &empty, &extra);
        let p = load(&env).unwrap().expect("alt_profile_name");
        assert_eq!(p.name, "alt_profile_name", "case {case}: name");
        assert_eq!(
            p.keys.get("region").map(String::as_str),
            Some("alt_profile_name_region"),
            "case {case}: region"
        );
    }

    // Case 2: the default profile, file does not exist.
    let env = env_for(
        &cfg("file_not_exist"),
        &empty,
        &[("AWS_PROFILE", "default")],
    );
    let err = load(&env).expect_err("case 2");
    assert!(
        err.contains("failed to get shared config profile"),
        "case 2: {err}"
    );

    // Case 3: a profile that is not in the file.
    let env = env_for(
        &cfg("shared_config"),
        &empty,
        &[("AWS_PROFILE", "profile_not_exist")],
    );
    let err = load(&env).expect_err("case 3");
    assert!(
        err.contains("failed to get shared config profile"),
        "case 3: {err}"
    );

    // Case 4: loadSharedConfigIgnoreNotExist with the default profile and no file.
    let env = env_for(&cfg("file_not_exist"), &empty, &[]);
    assert_eq!(load(&env), Ok(None), "case 4");

    // Cases 5 and 6: assume_role_invalid_source_profile over [shared_config_other, shared_config].
    let got = all(&["shared_config_other", "shared_config"], &[]);
    expect_keys(
        &got,
        "cases 5 and 6",
        "assume_role_invalid_source_profile",
        &[
            ("role_arn", "assume_role_invalid_source_profile_role_arn"),
            ("source_profile", "profile_not_exists"),
        ],
    );
}

#[derive(Debug)]
enum Want {
    /// `load` finds the profile with this name and these keys (Go `Expect`).
    Keys(&'static str, &'static [(&'static str, &'static str)]),
    /// `load` fails with this text (Go `Err`).
    Err(&'static str),
    /// `resolve` fails with "partial credentials found for profile <name>".
    Partial(&'static str),
    /// `load` finds no profile.
    Missing,
}

struct LoadCase {
    name: &'static str,
    /// AWS_PROFILE; None uses the default profile.
    profile: Option<&'static str>,
    config: &'static str,
    creds: &'static str,
    want: Want,
}

const EMPTY: &str = "empty_creds_config";

const LOAD_CASES: [LoadCase; 24] = [
    LoadCase {
        name: "duplicate profiles in the configuration files",
        profile: Some("duplicate-profile"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Keys("duplicate-profile", &[("region", "us-west-2")]),
    },
    LoadCase {
        name: "profile prefix not used in the configuration files",
        profile: Some("no-such-profile"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Err("failed to get shared config profile"),
    },
    LoadCase {
        name: "profile prefix overrides default",
        profile: None,
        config: "load_config",
        creds: EMPTY,
        want: Want::Keys("default", &[("region", "ap-north-1")]),
    },
    LoadCase {
        name: "duplicate profiles in credentials file",
        profile: Some("duplicate-profile"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Keys("duplicate-profile", &[("region", "us-west-2")]),
    },
    LoadCase {
        name: "profile prefix used in credentials files",
        profile: Some("unused-profile"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Err("failed to get shared config profile, unused-profile"),
    },
    LoadCase {
        name: "partial credentials in configuration files",
        profile: Some("partial-creds-1"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Partial("partial-creds-1"),
    },
    LoadCase {
        name: "parital credentials in the credentials files",
        profile: Some("partial-creds-1"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Partial("partial-creds-1"),
    },
    LoadCase {
        name: "credentials override configuration profile",
        profile: Some("complete"),
        config: "load_config",
        creds: "load_credentials",
        want: Want::Keys(
            "complete",
            &[
                ("aws_access_key_id", "credsAccessKey"),
                ("aws_secret_access_key", "credsSecretKey"),
                ("region", "us-west-2"),
            ],
        ),
    },
    LoadCase {
        name: "credentials profile has complete credentials",
        profile: Some("complete"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Keys(
            "complete",
            &[
                ("aws_access_key_id", "credsAccessKey"),
                ("aws_secret_access_key", "credsSecretKey"),
            ],
        ),
    },
    LoadCase {
        name: "credentials split between credentials and config files",
        profile: Some("partial-creds-1"),
        config: "load_config",
        creds: "load_credentials",
        want: Want::Partial("partial-creds-1"),
    },
    LoadCase {
        name: "replaced profile with prefixed profile in config files",
        profile: Some("replaced-profile"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Keys("replaced-profile", &[("region", "eu-west-1")]),
    },
    LoadCase {
        name: "replaced profile with prefixed profile in credentials files",
        profile: Some("replaced-profile"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Keys("replaced-profile", &[("region", "us-west-2")]),
    },
    LoadCase {
        name: "ignored profiles w/o prefixed profile across credentials and config files",
        profile: Some("replaced-profile"),
        config: "load_config",
        creds: "load_credentials",
        want: Want::Keys("replaced-profile", &[("region", "us-west-2")]),
    },
    LoadCase {
        name: "1. profile with name as `profile` in config file",
        profile: Some("profile"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Err("failed to get shared config profile, profile"),
    },
    LoadCase {
        name: "2. profile with name as `profile ` in config file",
        profile: Some("profile "),
        config: "load_config",
        creds: EMPTY,
        want: Want::Err("failed to get shared config profile, profile"),
    },
    LoadCase {
        name: "3. profile with name as `profile\\t` in config file",
        profile: Some("profile"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Err("failed to get shared config profile, profile"),
    },
    LoadCase {
        name: "profile with tabs as delimiter for profile prefix in config file",
        profile: Some("with-tab"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Keys("with-tab", &[("region", "cn-north-1")]),
    },
    LoadCase {
        name: "profile with tabs as delimiter for profile prefix in credentials file",
        profile: Some("with-tab"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Err("failed to get shared config profile, with-tab"),
    },
    LoadCase {
        name: "profile with name as `profile profile` in credentials file",
        profile: Some("profile"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Err("failed to get shared config profile, profile"),
    },
    LoadCase {
        name: "profile with name profile-bar in credentials file",
        profile: Some("profile-bar"),
        config: EMPTY,
        creds: "load_credentials",
        want: Want::Keys("profile-bar", &[("region", "us-west-2")]),
    },
    LoadCase {
        name: "profile with name profile-bar in config file",
        profile: Some("profile-bar"),
        config: "load_config",
        creds: EMPTY,
        want: Want::Err("failed to get shared config profile, profile-bar"),
    },
    LoadCase {
        name: "profile ignored in credentials and config file",
        profile: Some("ignored-profile"),
        config: "load_config",
        creds: "load_credentials",
        want: Want::Err("failed to get shared config profile, ignored-profile"),
    },
    LoadCase {
        name: "profile with sso_session",
        profile: Some("sso-session-test"),
        config: "load_config",
        creds: "load_credentials",
        want: Want::Keys("sso-session-test", &[("sso_session", "dev-session")]),
    },
    // ADAPT: Go fails with "failed to get shared config profile, default". A missing default
    // profile is not an error here (D-006: the chain goes on to the AWS CLI).
    LoadCase {
        name: "config and creds files explicitly set to empty slice",
        profile: None,
        config: "file_not_exist",
        creds: "file_not_exist_either",
        want: Want::Missing,
    },
];

// Port of config@v1.27.27/shared_config_test.go:1052 TestSharedConfigLoading
// The Go log-warning expectations (`ExpectLog`) are not ported: this crate logs nothing.
// The two cases that spread a profile over two files of the same kind are at the end, because
// `AWS_CONFIG_FILE` and `AWS_SHARED_CREDENTIALS_FILE` hold one path each; they run on `load_all`.
#[test]
fn shared_config_loading() {
    for c in &LOAD_CASES {
        let extra: Vec<(&str, &str)> = c.profile.map(|p| ("AWS_PROFILE", p)).into_iter().collect();
        let env = env_for(&cfg(c.config), &cfg(c.creds), &extra);
        match &c.want {
            Want::Keys(name, keys) => {
                let p = load(&env)
                    .unwrap_or_else(|e| panic!("{}: unexpected error {e}", c.name))
                    .unwrap_or_else(|| panic!("{}: no profile", c.name));
                assert_eq!(p.name, *name, "{}: profile name", c.name);
                for (k, v) in *keys {
                    assert_eq!(
                        p.keys.get(*k).map(String::as_str),
                        Some(*v),
                        "{}: key {k}",
                        c.name
                    );
                }
            }
            Want::Err(text) => {
                let err = load(&env).expect_err(c.name);
                assert!(err.contains(text), "{}: expect {text:?} in {err:?}", c.name);
            }
            Want::Partial(name) => {
                let err = resolve(&env).expect_err(c.name);
                let text = format!("partial credentials found for profile {name}");
                assert!(
                    err.contains(&text),
                    "{}: expect {text:?} in {err:?}",
                    c.name
                );
            }
            Want::Missing => {
                assert_eq!(load(&env), Ok(None), "{}", c.name);
            }
        }
    }

    // "credentials split between multiple credentials files": the second file's lone access
    // key does not complete the first file's lone secret, so the profile stays partial.
    let got = all(&[], &["load_credentials", "load_credentials_secondary"]);
    let case = "credentials split between multiple credentials files";
    expect_keys(
        &got,
        case,
        "partial-creds-1",
        &[("aws_secret_access_key", "notForSecretAccess")],
    );
    expect_absent(&got, case, "partial-creds-1", &["aws_access_key_id"]);

    // "credentials split between multiple configuration files": likewise, and the region of
    // the second file still merges (Go `Expect` Region us-west-2).
    let got = all(&["load_config", "load_config_secondary"], &[]);
    let case = "credentials split between multiple configuration files";
    expect_keys(
        &got,
        case,
        "partial-creds-1",
        &[
            ("aws_access_key_id", "notForAccess"),
            ("region", "us-west-2"),
        ],
    );
    expect_absent(&got, case, "partial-creds-1", &["aws_secret_access_key"]);
}

// ---------------------------------------------------------------------------------------------
// config@v1.27.27 resolve_processcreds_test.go
// ---------------------------------------------------------------------------------------------

/// Env for the credential_process fixtures: `config` or `credentials` is the rewritten fixture.
fn process_env(profile: Option<&str>, config: &str, creds: &str) -> Env {
    let extra: Vec<(&str, &str)> = [("PATH", SYSTEM_PATH)]
        .into_iter()
        .chain(profile.map(|p| ("AWS_PROFILE", p)))
        .collect();
    env_for(config, creds, &extra)
}

fn assert_process_creds(env: &Env, case: &str, id: &str, secret: &str, token: Option<&str>) {
    let (src, got) =
        resolve(env).unwrap_or_else(|e| panic!("{case}: error getting credentials: {e}"));
    assert!(
        matches!(src, Source::Process { .. }),
        "{case}: expected a process source, got {src:?}"
    );
    assert_eq!(got.access_key_id, id, "{case}: access key id");
    assert_eq!(got.secret_access_key, secret, "{case}: secret");
    assert_eq!(got.session_token.as_deref(), token, "{case}: session token");
}

// Port of config@v1.27.27/resolve_processcreds_test.go:32 TestProcessCredentialsProvider_FromConfig
#[test]
fn process_credentials_provider_from_config() {
    let env = process_env(None, &rewritten("proc_creds_config.ini"), &cfg(EMPTY));
    assert_process_creds(
        &env,
        "from config",
        "accessKey",
        "secret",
        Some("tokenDefault"),
    );
}

// Port of config@v1.27.27/resolve_processcreds_test.go:62 TestProcessCredentialsProvider_FromConfigWithProfile
#[test]
fn process_credentials_provider_from_config_with_profile() {
    let env = process_env(
        Some("not_expire"),
        &rewritten("proc_creds_config.ini"),
        &cfg(EMPTY),
    );
    assert_process_creds(
        &env,
        "from config with profile",
        "accessKey",
        "secret",
        Some("nonDefaultToken"),
    );
}

// Port of config@v1.27.27/resolve_processcreds_test.go:85 TestProcessCredentialsProvider_FromConfigWithStaticCreds
// Static keys in the profile beat its credential_process.
#[test]
fn process_credentials_provider_from_config_with_static_creds() {
    let env = process_env(
        Some("not_alone"),
        &rewritten("proc_creds_config.ini"),
        &cfg(EMPTY),
    );
    let (src, got) = resolve(&env).unwrap();
    assert!(
        matches!(src, Source::Static { .. }),
        "from config with static creds: expected a static source, got {src:?}"
    );
    assert_eq!(got.access_key_id, "notFromCredProcAccess");
    assert_eq!(got.secret_access_key, "notFromCredProcSecret");
}

// Port of config@v1.27.27/resolve_processcreds_test.go:112 TestProcessCredentialsProvider_FromCredentials
// Go leaves AWS_CONFIG_FILE unset (a missing file); here it is the empty fixture.
#[test]
fn process_credentials_provider_from_credentials() {
    let env = process_env(None, &cfg(EMPTY), &rewritten("proc_creds_credentials.ini"));
    assert_process_creds(
        &env,
        "from credentials",
        "accessKey",
        "secret",
        Some("tokenDefault"),
    );
}

// Port of config@v1.27.27/resolve_processcreds_test.go:142 TestProcessCredentialsProvider_FromCredentialsWithProfile
#[test]
fn process_credentials_provider_from_credentials_with_profile() {
    let env = process_env(
        Some("not_expire"),
        &cfg(EMPTY),
        &rewritten("proc_creds_credentials.ini"),
    );
    assert_process_creds(
        &env,
        "from credentials with profile",
        "accessKey",
        "secret",
        Some("nonDefaultToken"),
    );
}

// Port of config@v1.27.27/resolve_processcreds_test.go:165 TestProcessCredentialsProvider_FromCredentialsWithStaticCreds
#[test]
fn process_credentials_provider_from_credentials_with_static_creds() {
    let env = process_env(
        Some("not_alone"),
        &cfg(EMPTY),
        &rewritten("proc_creds_credentials.ini"),
    );
    let (src, got) = resolve(&env).unwrap();
    assert!(
        matches!(src, Source::Static { .. }),
        "from credentials with static creds: expected a static source, got {src:?}"
    );
    assert_eq!(got.access_key_id, "notFromCredProcAccess");
    assert_eq!(got.secret_access_key, "notFromCredProcSecret");
}

// ---------------------------------------------------------------------------------------------
// config@v1.27.27 resolve_credentials_test.go TestSharedConfigCredentialSource
// ---------------------------------------------------------------------------------------------

// Port of config@v1.27.27/resolve_credentials_test.go:178 TestSharedConfigCredentialSource
// Cases :197 :252 :258 :338 :348, ADAPT to D-006. Every other case in that Go test assumes a
// role, calls IMDS, ECS or SSO, or reads a web identity token, and is skipped here.
#[test]
fn shared_config_credential_source() {
    let config = rewritten("config_source_shared");
    let creds = cfg("credentials_source_shared");

    // :197 "credential source and source profile". Go fails on the bad profile even though env
    // keys are set; here env keys win and the profile is not read (D-006 b).
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "invalid_source_and_credential_source"),
            ("AWS_ACCESS_KEY", "access_key"),
            ("AWS_SECRET_KEY", "secret_key"),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(src, Source::Env, ":197 source");
    assert_eq!(got.access_key_id, "access_key", ":197 access key");
    assert_eq!(got.secret_access_key, "secret_key", ":197 secret");

    // :252 "credential process with no ARN set": a plain credential_process runs here.
    let env = process_env(Some("cred_proc_no_arn_set"), &config, &creds);
    let (src, got) = resolve(&env).unwrap();
    assert!(matches!(src, Source::Process { .. }), ":252 source {src:?}");
    assert_eq!(got.access_key_id, "cred_proc_akid", ":252 access key");
    assert_eq!(got.secret_access_key, "cred_proc_secret", ":252 secret");

    // :258, :338 and :348 need the AWS CLI (a role, or SSO), or env keys.
    let dir = tmp_dir("aws_cli");
    fake_aws(&dir);
    let dir_path = dir.to_string_lossy().into_owned();

    // :258 "credential process with ARN set": Go assumes the role; here the CLI does.
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "cred_proc_arn_set"),
            ("PATH", dir_path.as_str()),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(
        src,
        Source::Cli {
            profile: Some("cred_proc_arn_set".to_string())
        },
        ":258 source"
    );
    assert_eq!(got.access_key_id, AKID, ":258 access key");
    assert_eq!(got.secret_access_key, SECRET, ":258 secret");

    // :338 "environment credentials with invalid sso": env keys win over the broken profile.
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "sso_invalid"),
            ("AWS_ACCESS_KEY", "access_key"),
            ("AWS_SECRET_KEY", "secret_key"),
            ("PATH", dir_path.as_str()),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(src, Source::Env, ":338 source");
    assert_eq!(got.access_key_id, "access_key", ":338 access key");
    assert_eq!(got.secret_access_key, "secret_key", ":338 secret");

    // :348 "sso mixed with credential process provider": SSO wins, so the CLI runs, not the process.
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "sso_mixed_credproc"),
            ("PATH", dir_path.as_str()),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(
        src,
        Source::Cli {
            profile: Some("sso_mixed_credproc".to_string())
        },
        ":348 source"
    );
    assert_eq!(got.access_key_id, AKID, ":348 access key");
    assert_eq!(
        got.session_token.as_deref(),
        Some("FAKESESSIONTOKEN"),
        ":348 token"
    );

    assert_eq!(
        argv_lines(&dir),
        [
            "configure export-credentials --format process --profile cred_proc_arn_set",
            "configure export-credentials --format process --profile sso_mixed_credproc",
        ],
        "aws argv for :258 and :348"
    );
}

// ---------------------------------------------------------------------------------------------
// New tests (plan Task 5)
// ---------------------------------------------------------------------------------------------

const PROFILE_FILE: &str = "[profile other]\naws_access_key_id = AKIDPROFILE\naws_secret_access_key = profile-secret\naws_session_token = profile-token\n";

// New: env keys beat a static profile, and a broken AWS_PROFILE does not matter when env has keys
// (D-006 b).
#[test]
fn env_keys_win_over_profile() {
    let (config, creds) = write_files(PROFILE_FILE, "");
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "other"),
            ("AWS_ACCESS_KEY_ID", AKID),
            ("AWS_SECRET_ACCESS_KEY", SECRET),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(src, Source::Env, "source");
    assert_eq!(got.access_key_id, AKID, "access key id");
    assert_eq!(got.secret_access_key, SECRET, "secret");
    assert_eq!(
        got.session_token, None,
        "the profile's token must not leak in"
    );

    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "no-such-profile"),
            ("AWS_ACCESS_KEY_ID", AKID),
            ("AWS_SECRET_ACCESS_KEY", SECRET),
        ],
    );
    let (src, _) =
        resolve(&env).unwrap_or_else(|e| panic!("broken AWS_PROFILE with env keys: {e}"));
    assert_eq!(src, Source::Env, "source with a broken AWS_PROFILE");
}

// New: one env key is not enough; the chain goes on to the profile.
#[test]
fn env_needs_both_keys() {
    let (config, creds) = write_files(PROFILE_FILE, "");
    for (case, key, value) in [
        ("only the access key id", "AWS_ACCESS_KEY_ID", AKID),
        ("only the secret", "AWS_SECRET_ACCESS_KEY", SECRET),
    ] {
        let env = env_for(&config, &creds, &[("AWS_PROFILE", "other"), (key, value)]);
        let (src, got) = resolve(&env).unwrap_or_else(|e| panic!("{case}: {e}"));
        assert_eq!(
            src,
            Source::Static {
                profile: "other".to_string()
            },
            "{case}: source"
        );
        assert_eq!(got.access_key_id, "AKIDPROFILE", "{case}: access key id");
        assert_eq!(got.secret_access_key, "profile-secret", "{case}: secret");
        assert_eq!(
            got.session_token.as_deref(),
            Some("profile-token"),
            "{case}: token"
        );
    }
}

// New: AWS_ACCESS_KEY and AWS_SECRET_KEY are aliases; the long names win; "" counts as unset.
#[test]
fn env_alias_names() {
    let (config, creds) = write_files("", "");

    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_ACCESS_KEY", "AKIDALIAS"),
            ("AWS_SECRET_KEY", "alias-secret"),
            ("AWS_SESSION_TOKEN", "alias-token"),
        ],
    );
    let (src, got) = resolve(&env).unwrap();
    assert_eq!(src, Source::Env, "aliases: source");
    assert_eq!(got.access_key_id, "AKIDALIAS", "aliases: access key id");
    assert_eq!(got.secret_access_key, "alias-secret", "aliases: secret");
    assert_eq!(
        got.session_token.as_deref(),
        Some("alias-token"),
        "aliases: token"
    );

    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_ACCESS_KEY_ID", "AKIDPRIMARY"),
            ("AWS_ACCESS_KEY", "AKIDALIAS"),
            ("AWS_SECRET_ACCESS_KEY", "primary-secret"),
            ("AWS_SECRET_KEY", "alias-secret"),
        ],
    );
    let (_, got) = resolve(&env).unwrap();
    assert_eq!(
        got.access_key_id, "AKIDPRIMARY",
        "long names win: access key id"
    );
    assert_eq!(
        got.secret_access_key, "primary-secret",
        "long names win: secret"
    );

    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_ACCESS_KEY_ID", ""),
            ("AWS_ACCESS_KEY", "AKIDALIAS"),
            ("AWS_SECRET_ACCESS_KEY", ""),
            ("AWS_SECRET_KEY", "alias-secret"),
        ],
    );
    let (_, got) = resolve(&env).unwrap();
    assert_eq!(
        got.access_key_id, "AKIDALIAS",
        "empty long name: access key id"
    );
    assert_eq!(
        got.secret_access_key, "alias-secret",
        "empty long name: secret"
    );
}

const ROLE_SSO_CONFIG: &str = "\
[default]
role_arn = arn:aws:iam::123456789012:role/example

[profile base]
aws_access_key_id = AKIDEXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY

[profile role-static]
role_arn = arn:aws:iam::123456789012:role/example
aws_access_key_id = AKIDEXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY

[profile sso-process]
sso_session = example
credential_process = /bin/false

[profile sso-legacy]
sso_start_url = https://example.invalid/start

[profile via-source]
source_profile = base

[profile via-credential-source]
credential_source = Ec2InstanceMetadata

[profile web-identity]
web_identity_token_file = /nonexistent/token
";

// New (Review Focus): a profile with role or SSO keys goes to the CLI, even with static keys or
// a credential_process, and `--profile` is passed only when AWS_PROFILE or AWS_DEFAULT_PROFILE is set.
#[test]
fn role_or_sso_profile_uses_cli() {
    let (config, creds) = write_files(ROLE_SSO_CONFIG, "");
    let dir = tmp_dir("aws_cli");
    fake_aws(&dir);
    let dir_path = dir.to_string_lossy().into_owned();

    let mut want_argv = Vec::new();

    for name in [
        "role-static",
        "sso-process",
        "sso-legacy",
        "via-source",
        "via-credential-source",
        "web-identity",
    ] {
        let env = env_for(
            &config,
            &creds,
            &[("AWS_PROFILE", name), ("PATH", dir_path.as_str())],
        );
        let (src, got) = resolve(&env).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            src,
            Source::Cli {
                profile: Some(name.to_string())
            },
            "{name}: source"
        );
        assert_eq!(got.access_key_id, AKID, "{name}: access key id");
        assert_eq!(got.secret_access_key, SECRET, "{name}: secret");
        assert_eq!(
            got.session_token.as_deref(),
            Some("FAKESESSIONTOKEN"),
            "{name}: token"
        );
        want_argv.push(format!(
            "configure export-credentials --format process --profile {name}"
        ));
    }

    // AWS_DEFAULT_PROFILE names the profile too.
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_DEFAULT_PROFILE", "via-source"),
            ("PATH", dir_path.as_str()),
        ],
    );
    let (src, _) = resolve(&env).unwrap();
    assert_eq!(
        src,
        Source::Cli {
            profile: Some("via-source".to_string())
        },
        "AWS_DEFAULT_PROFILE: source"
    );
    want_argv
        .push("configure export-credentials --format process --profile via-source".to_string());

    // The default profile with role_arn: no --profile argument.
    let env = env_for(&config, &creds, &[("PATH", dir_path.as_str())]);
    let (src, _) = resolve(&env).unwrap();
    assert_eq!(
        src,
        Source::Cli { profile: None },
        "default profile: source"
    );
    want_argv.push("configure export-credentials --format process".to_string());

    // A static profile still goes to the CLI when AWS_WEB_IDENTITY_TOKEN_FILE is set.
    let env = env_for(
        &config,
        &creds,
        &[
            ("AWS_PROFILE", "base"),
            ("AWS_WEB_IDENTITY_TOKEN_FILE", "/nonexistent/token"),
            ("PATH", dir_path.as_str()),
        ],
    );
    let (src, _) = resolve(&env).unwrap();
    assert_eq!(
        src,
        Source::Cli {
            profile: Some("base".to_string())
        },
        "AWS_WEB_IDENTITY_TOKEN_FILE: source"
    );
    want_argv.push("configure export-credentials --format process --profile base".to_string());

    assert_eq!(argv_lines(&dir), want_argv, "aws argv");
}

fn not_found_message(profile: &str, config: &str, creds: &str) -> String {
    format!(
        "no AWS credentials found: no AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY in the environment, no static keys or credential_process in profile \"{profile}\" ({config}, {creds}), and the AWS CLI (aws) is not on PATH; it is needed for SSO, role and instance credentials. Set AWS_BEARER_TOKEN_BEDROCK, set AWS keys, or install the AWS CLI and run aws sso login"
    )
}

// New: no keys, nothing usable in the profile and no `aws` on PATH.
#[test]
fn cli_not_found_error() {
    // A profile that exists but has no keys.
    let (config, creds) = write_files("[profile plain]\nregion = us-east-1\n", "");
    let env = env_for(&config, &creds, &[("AWS_PROFILE", "plain")]);
    let err = resolve(&env).expect_err("plain profile");
    assert_eq!(
        err,
        not_found_message("plain", &config, &creds),
        "plain profile"
    );

    // The default profile is missing.
    let (config, creds) = write_files("", "");
    let env = env_for(&config, &creds, &[]);
    let err = resolve(&env).expect_err("missing default profile");
    assert_eq!(
        err,
        not_found_message("default", &config, &creds),
        "missing default profile"
    );

    // A profile with SSO keys says so.
    let (config, creds) = write_files("[profile sso]\nsso_session = x\n", "");
    let env = env_for(&config, &creds, &[("AWS_PROFILE", "sso")]);
    let err = resolve(&env).expect_err("sso profile");
    assert!(
        err.starts_with("no AWS credentials found: "),
        "sso profile: {err}"
    );
    assert!(
        err.contains("uses sso_session, which needs the AWS CLI"),
        "sso profile: {err}"
    );
    assert!(
        err.contains("\"sso\""),
        "sso profile names the profile: {err}"
    );
}

// New: `aws` exits non-zero; its stderr reaches the message.
#[test]
fn cli_failure_carries_stderr() {
    let (config, creds) = write_files("[profile sso]\nsso_session = x\n", "");
    let dir = tmp_dir("aws_cli_fail");
    write_script(
        &dir,
        "aws",
        "echo 'Token has expired and cannot be refreshed' >&2\nexit 255",
    );
    let dir_path = dir.to_string_lossy().into_owned();
    let env = env_for(
        &config,
        &creds,
        &[("AWS_PROFILE", "sso"), ("PATH", dir_path.as_str())],
    );
    let err = resolve(&env).expect_err("failing aws");
    assert!(
        err.starts_with("no AWS credentials found: aws configure export-credentials failed: "),
        "{err}"
    );
    assert!(err.contains("255"), "exit status in {err}");
    assert!(
        err.contains("Token has expired and cannot be refreshed"),
        "stderr in {err}"
    );
}

// New: a credential process that outlives its timeout.
#[test]
fn process_timeout() {
    let env = Env::from_pairs([("PATH", SYSTEM_PATH)]);
    let err = process::run("sleep 2", &env, Duration::from_secs(1)).expect_err("timeout");
    assert!(
        err.contains("credential process timed out"),
        "expected a timeout, got {err}"
    );
}

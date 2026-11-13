use super::*;
use crate::json::{self, Kind};
use crate::testutil::testdata;

// Port of internal/ini@v1.8.0/ini_test.go:13 TestValidDataFiles
#[test]
fn valid_data_files() {
    let dir = testdata("ini/valid");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();

    let mut checked = 0;
    for name in &names {
        if name.ends_with("_expected") {
            continue;
        }
        let text = String::from_utf8_lossy(&std::fs::read(dir.join(name)).unwrap()).into_owned();
        let sections = parse(&text);

        // Go ignores inputs that have no expected file.
        let Ok(expected_text) = std::fs::read_to_string(dir.join(format!("{name}_expected")))
        else {
            continue;
        };
        checked += 1;
        let expected = json::parse(&expected_text)
            .unwrap_or_else(|e| panic!("{name}: unexpected error during deserialization, {e}"));
        let Kind::Object(profiles) = &expected.kind else {
            panic!("{name}: expected file is not a JSON object");
        };
        for (profile, table) in profiles {
            let section = sections
                .get(profile)
                .unwrap_or_else(|| panic!("{name}: could not find profile {profile}"));
            let Kind::Object(entries) = &table.kind else {
                panic!("{name}: profile {profile} is not a JSON object");
            };
            for (key, want) in entries {
                let Kind::String(want) = &want.kind else {
                    panic!("{name}: unexpected type for {key} in profile {profile}");
                };
                // Go: a missing key is the zero Value, whose str is "".
                let got = section
                    .values
                    .get(key)
                    .map(IniValue::display)
                    .unwrap_or_default();
                assert_eq!(
                    want, &got,
                    "{name}: expected {want}, but received {got} for key {key} in profile {profile}"
                );
            }
        }
    }
    assert_eq!(checked, 20, "every valid fixture has an _expected file");
}

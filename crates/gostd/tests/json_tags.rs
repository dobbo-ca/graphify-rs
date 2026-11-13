//! Ported from encoding/json/tagkey_test.go and tags_test.go (Go 1.27.4, BSD-3-Clause, Copyright 2011
//! The Go Authors).
//! - TestStructTagObjectKey (tagkey_test.go:77): ported, adapted: 11 of 16 rows. Go marshals, unmarshals into
//!   `any` and checks the single key and value; the decode half is not built yet (graphify-rs-aa6.10), so
//!   the marshalled bytes are compared with the expected `{"key":"value"}` text (`Marshal` escapes HTML
//!   in the key, Go `nameEscHTML`: `&` `<` `>` are `\u0026` `\u003c` `\u003e`). The tag is a `json_fields!`
//!   key, taken literally.
//! - TestTagParsing (tags_test.go:11): not ported: it tests the internal `parseTag`/`tagOptions.Contains`,
//!   which have no counterpart (the macro takes the key and `omitempty` as syntax, there is no tag string)
//!
//! Rows left out of TestStructTagObjectKey (5 of 16):
//! - dashTag (`json:"-,"`, key "-"): `json_fields!` treats the literal key "-" as skip the field
//! - emptyTag (no tag, Go uses the field name "W"): `json_fields!` always takes an explicit key
//! - misnamedTag (`jsom:"Misnamed"`, not a json tag, Go uses the field name "X"): tag parsing is not reached
//! - badFormatTag (malformed struct tag `:"BadFormat"`, Go uses "Y"): tag parsing is not reached
//! - badCodeTag (`json:" !\"#&'()*+,."`, Go rejects the key as invalid and uses "Z"): tag validation is not reached

use gostd::json::marshal;

macro_rules! tag_struct {
    ($name:ident, $go:literal, $key:literal) => {
        struct $name {
            v: String,
        }
        gostd::json_fields!($name, $go, { v => $key });
    };
}

tag_struct!(BasicLatin2xTag, "json.basicLatin2xTag", "$%-/");
tag_struct!(BasicLatin3xTag, "json.basicLatin3xTag", "0123456789");
tag_struct!(BasicLatin4xTag, "json.basicLatin4xTag", "ABCDEFGHIJKLMO");
tag_struct!(BasicLatin5xTag, "json.basicLatin5xTag", "PQRSTUVWXYZ_");
tag_struct!(BasicLatin6xTag, "json.basicLatin6xTag", "abcdefghijklmno");
tag_struct!(BasicLatin7xTag, "json.basicLatin7xTag", "pqrstuvwxyz");
tag_struct!(MiscPlaneTag, "json.miscPlaneTag", "色は匂へど");
tag_struct!(PercentSlashTag, "json.percentSlashTag", "text/html%"); // https://golang.org/issue/2718
tag_struct!(
    PunctuationTag,
    "json.punctuationTag",
    "!#$%&()*+-./:;<=>?@[]^_{|}~ "
); // https://golang.org/issue/3546
tag_struct!(SpaceTag, "json.spaceTag", "With space");
tag_struct!(UnicodeTag, "json.unicodeTag", "Ελλάδα");

fn check<T: gostd::json::Marshal>(raw: &T, want: &str) {
    let b = marshal(raw).unwrap();
    assert_eq!(String::from_utf8(b).unwrap(), want);
}

#[test]
#[ignore = "graphify-rs-aa6.9"]
fn test_struct_tag_object_key() {
    check(&BasicLatin2xTag { v: "2x".into() }, r#"{"$%-/":"2x"}"#);
    check(
        &BasicLatin3xTag { v: "3x".into() },
        r#"{"0123456789":"3x"}"#,
    );
    check(
        &BasicLatin4xTag { v: "4x".into() },
        r#"{"ABCDEFGHIJKLMO":"4x"}"#,
    );
    check(
        &BasicLatin5xTag { v: "5x".into() },
        r#"{"PQRSTUVWXYZ_":"5x"}"#,
    );
    check(
        &BasicLatin6xTag { v: "6x".into() },
        r#"{"abcdefghijklmno":"6x"}"#,
    );
    check(
        &BasicLatin7xTag { v: "7x".into() },
        r#"{"pqrstuvwxyz":"7x"}"#,
    );
    check(
        &MiscPlaneTag {
            v: "いろはにほへと".into(),
        },
        r#"{"色は匂へど":"いろはにほへと"}"#,
    );
    check(
        &PercentSlashTag { v: "brut".into() },
        r#"{"text/html%":"brut"}"#,
    );
    check(
        &PunctuationTag {
            v: "Union Rags".into(),
        },
        r#"{"!#$%\u0026()*+-./:;\u003c=\u003e?@[]^_{|}~ ":"Union Rags"}"#,
    );
    check(
        &SpaceTag {
            v: "Perreddu".into(),
        },
        r#"{"With space":"Perreddu"}"#,
    );
    check(
        &UnicodeTag {
            v: "Loukanikos".into(),
        },
        r#"{"Ελλάδα":"Loukanikos"}"#,
    );
}

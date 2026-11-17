//! Upstream graphify v0.9.107 `tests/test_msbuild_dir_evidence.py`: Go never prunes `obj/` or
//! `bin/` (B55), so the build-layout cases pin Go's collected files. Not ported:
//! `test_vendored_binaries_do_not_prune_bin` (its subject is a `.ps1` file; Go has no PowerShell
//! extractor).

use std::collections::BTreeSet;

use super::{collected, must_write};
use crate::testutil::TempDir;

/// Upstream `_sdk_project`.
fn sdk_project(t: &TempDir) {
    must_write(
        &t.join("App.csproj"),
        "<Project Sdk=\"Microsoft.NET.Sdk.Web\" />\n",
    );
    must_write(&t.join("Program.cs"), "public class Program { }\n");
    must_write(&t.join("appsettings.json"), "{}\n");
}

fn under_obj_or_bin(files: &BTreeSet<String>) -> bool {
    files
        .iter()
        .any(|f| f.starts_with("obj/") || f.starts_with("bin/"))
}

/// `test_built_sdk_project_prunes_obj_and_bin`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn built_sdk_project_prunes_obj_and_bin() {
    let t = TempDir::new();
    sdk_project(&t);
    must_write(&t.join("obj/project.assets.json"), "{}\n");
    must_write(&t.join("obj/App.csproj.nuget.dgspec.json"), "{}\n");
    must_write(
        &t.join("obj/Debug/net8.0/App.GlobalUsings.g.cs"),
        "global using global::System;\n",
    );
    must_write(
        &t.join("obj/Debug/net8.0/App.AssemblyInfo.cs"),
        "using System.Reflection;\n",
    );
    must_write(&t.join("bin/Debug/net8.0/App.deps.json"), "{}\n");
    must_write(&t.join("bin/Debug/net8.0/App.runtimeconfig.json"), "{}\n");
    must_write(&t.join("bin/Debug/net8.0/appsettings.json"), "{}\n");
    let got = collected(t.path());
    assert!(
        got.contains("Program.cs") && got.contains("appsettings.json"),
        "{got:?}"
    );
    // pins Go v0.12.0 (obj/ and bin/ files collected); upstream 0.9.107 prunes them -> P3 (B55)
    assert!(under_obj_or_bin(&got), "{got:?}");
}

/// `test_rid_specific_bin_is_pruned`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn rid_specific_bin_is_pruned() {
    let t = TempDir::new();
    sdk_project(&t);
    must_write(
        &t.join("bin/Release/net8.0/linux-x64/App.runtimeconfig.json"),
        "{}\n",
    );
    must_write(
        &t.join("bin/Release/net8.0/linux-x64/appsettings.json"),
        "{}\n",
    );
    let got = collected(t.path());
    assert!(got.contains("Program.cs"), "{got:?}");
    // pins Go v0.12.0 (bin/ files collected); upstream 0.9.107 prunes them -> P3 (B55)
    assert!(under_obj_or_bin(&got), "{got:?}");
}

/// `test_legacy_framework_project_prunes_obj_and_bin`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn legacy_framework_project_prunes_obj_and_bin() {
    let t = TempDir::new();
    must_write(
        &t.join("Legacy.csproj"),
        "<Project ToolsVersion=\"15.0\" />\n",
    );
    must_write(&t.join("Program.cs"), "class Program { }\n");
    must_write(
        &t.join("obj/Debug/Legacy.csproj.FileListAbsolute.txt"),
        "C:\\src\\Legacy\\bin\\Debug\\Legacy.exe\n",
    );
    must_write(
        &t.join("obj/Debug/TemporaryGeneratedFile_036C0B5B.cs"),
        "\n",
    );
    std::fs::create_dir_all(t.join("bin/Debug")).expect("mkdir");
    std::fs::write(
        t.join("bin/Debug/Legacy.pdb"),
        b"Microsoft C/C++ MSF 7.00\r\n",
    )
    .expect("write");
    must_write(
        &t.join("bin/Debug/Legacy.exe.config"),
        "<configuration />\n",
    );
    let got = collected(t.path());
    assert!(got.contains("Program.cs"), "{got:?}");
    // pins Go v0.12.0 (obj/Debug/TemporaryGeneratedFile_036C0B5B.cs collected); upstream 0.9.107 prunes it -> P3 (B55)
    assert!(under_obj_or_bin(&got), "{got:?}");
}

/// `test_source_dirs_named_bin_and_obj_are_scanned`.
#[test]
#[ignore = "graphify-rs-ihc.14"]
fn source_dirs_named_bin_and_obj_are_scanned() {
    let t = TempDir::new();
    must_write(&t.join("src/bin/cli.rs"), "fn main() {}\n");
    must_write(&t.join("bin/deploy.sh"), "#!/bin/sh\necho deploy\n");
    must_write(&t.join("engine/obj/mesh.cpp"), "struct Mesh {};\n");
    let got = collected(t.path());
    // pins Go v0.12.0; upstream 0.9.107 differs -> P3 (B55) (holds because Go never prunes bin/ or obj/)
    for f in ["src/bin/cli.rs", "bin/deploy.sh", "engine/obj/mesh.cpp"] {
        assert!(got.contains(f), "{f}: {got:?}");
    }
}

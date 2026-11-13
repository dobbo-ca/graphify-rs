//! Helpers for unit tests: temp dirs, executable scripts, fixture paths.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// `crates/bedrock/testdata/<rel>`.
pub(crate) fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(rel)
}

/// A new empty directory under the system temp dir, unique per call and per process.
pub(crate) fn tmp_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("bedrock-{}-{tag}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `body` to `dir/name` as an executable `#!/bin/sh` script and returns its path.
pub(crate) fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// `d` times `BEDROCK_TEST_TIME_SCALE` (default 1). The `arm64` stage sets 10: under qemu-user every
/// safety deadline needs a wide margin. Only deadlines a test must never hit are scaled.
pub(crate) fn scaled(d: std::time::Duration) -> std::time::Duration {
    let k = std::env::var("BEDROCK_TEST_TIME_SCALE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(1)
        .max(1);
    d * k
}

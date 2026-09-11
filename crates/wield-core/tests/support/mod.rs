#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Write `body` to `<dir>/<name>`, chmod 0o755, return the path.
///
/// Explicitly syncs the file (and the file handle stays open through the
/// permissions change) before returning: writing then immediately exec'ing a
/// freshly created file is a real, observed race on at least one CI
/// runner's filesystem — the write/chmod can be reordered or not yet
/// durable when the caller execs the path moments later, intermittently
/// failing with a bogus "command not found"/timeout-shaped result instead
/// of actually running the script.
pub fn write_stub_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    let mut file = fs::File::create(&path).unwrap();
    file.write_all(body.as_bytes()).unwrap();
    let mut permissions = file.metadata().unwrap().permissions();
    permissions.set_mode(0o755);
    file.set_permissions(permissions).unwrap();
    file.sync_all().unwrap();
    drop(file);
    path
}

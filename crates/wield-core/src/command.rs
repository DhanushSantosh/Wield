//! Command execution. See plan P2 Tasks 7 and 9 through 11.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BinaryResolver {
    extra_dirs: Vec<PathBuf>,
}

impl BinaryResolver {
    pub fn from_env() -> Self {
        Self {
            extra_dirs: Vec::new(),
        }
    }

    pub fn with_dirs(dirs: Vec<PathBuf>) -> Self {
        Self { extra_dirs: dirs }
    }

    pub fn resolve(&self, binary: &str) -> Option<PathBuf> {
        if binary.contains('/') {
            let path = PathBuf::from(binary);
            return (path.is_file()).then_some(path);
        }

        self.extra_dirs
            .iter()
            .cloned()
            .chain(path_dirs())
            .map(|directory| directory.join(binary))
            .find(|candidate| is_executable(candidate))
    }
}

fn path_dirs() -> impl Iterator<Item = PathBuf> {
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

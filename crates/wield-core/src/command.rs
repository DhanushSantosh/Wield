//! Command execution. See plan P2 Tasks 7 and 9 through 11.

use crate::descriptor::{ProgressSpec, SuccessSpec};
use crate::outcome::Progress;
use std::env;
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

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

#[derive(Debug, Clone)]
pub struct OutputPlan {
    pub temp: PathBuf,
    pub final_path: PathBuf,
}

impl OutputPlan {
    pub fn for_final(final_path: PathBuf) -> Self {
        let final_name = final_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("output");
        let temp_name = format!(".wield-tmp-{}-{final_name}", Uuid::new_v4());
        let temp = final_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(temp_name);
        Self { temp, final_path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    Success {
        output_file: Option<PathBuf>,
    },
    NonZeroExit {
        code: Option<i32>,
        stderr_tail: String,
    },
    Timeout,
    Cancelled,
    SpawnFailed {
        detail: String,
    },
    OutputMissing,
}

pub struct RunSpec<'a> {
    pub binary: &'a Path,
    pub argv: &'a [String],
    pub cwd: Option<&'a Path>,
    pub output: Option<&'a OutputPlan>,
    pub progress_spec: &'a ProgressSpec,
    pub success: &'a SuccessSpec,
    pub timeout: Duration,
    pub progress: mpsc::Sender<Progress>,
    pub cancel: CancellationToken,
}

pub struct CommandRunner;

impl CommandRunner {
    pub async fn execute(spec: RunSpec<'_>) -> CommandResult {
        let _ = spec.progress.try_send(Progress::Started);
        let _ = (spec.progress_spec, spec.timeout, &spec.cancel);

        let mut standard = std::process::Command::new(spec.binary);
        standard
            .args(spec.argv)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        if let Some(cwd) = spec.cwd {
            standard.current_dir(cwd);
        }

        let mut child = match tokio::process::Command::from(standard).spawn() {
            Ok(child) => child,
            Err(error) => {
                cleanup_temp(spec.output);
                let _ = spec.progress.try_send(Progress::Finished);
                return CommandResult::SpawnFailed {
                    detail: error.to_string(),
                };
            }
        };

        let stdout_task = child.stdout.take().map(|mut stdout| {
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                stdout.read_to_end(&mut buffer).await.map(|_| buffer)
            })
        });
        let stderr_task = child.stderr.take().map(|mut stderr| {
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                stderr.read_to_end(&mut buffer).await.map(|_| buffer)
            })
        });

        let status = child.wait().await;
        let _stdout = collect_pipe(stdout_task).await.unwrap_or_default();
        let stderr = collect_pipe(stderr_task).await.unwrap_or_default();
        let stderr_tail = retained_tail(&stderr);

        let result = match status {
            Err(error) => {
                cleanup_temp(spec.output);
                CommandResult::SpawnFailed {
                    detail: error.to_string(),
                }
            }
            Ok(status) if matches!(spec.success, SuccessSpec::ExitZero) && status.success() => {
                finish_output(spec.output)
            }
            Ok(status) => {
                cleanup_temp(spec.output);
                CommandResult::NonZeroExit {
                    code: status.code(),
                    stderr_tail,
                }
            }
        };

        let _ = spec.progress.try_send(Progress::Finished);
        result
    }
}

async fn collect_pipe(
    task: Option<tokio::task::JoinHandle<io::Result<Vec<u8>>>>,
) -> io::Result<Vec<u8>> {
    match task {
        Some(task) => task
            .await
            .map_err(|error| io::Error::other(error.to_string()))?,
        None => Ok(Vec::new()),
    }
}

fn retained_tail(bytes: &[u8]) -> String {
    const LIMIT: usize = 8 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn finish_output(output: Option<&OutputPlan>) -> CommandResult {
    let Some(plan) = output else {
        return CommandResult::Success { output_file: None };
    };
    if !plan.temp.is_file() {
        return CommandResult::OutputMissing;
    }

    let sync_result = fs::OpenOptions::new()
        .read(true)
        .open(&plan.temp)
        .and_then(|file| file.sync_all());
    if let Err(error) = sync_result {
        cleanup_temp(output);
        return CommandResult::SpawnFailed {
            detail: error.to_string(),
        };
    }
    if let Err(error) = fs::rename(&plan.temp, &plan.final_path) {
        cleanup_temp(output);
        return CommandResult::SpawnFailed {
            detail: error.to_string(),
        };
    }
    CommandResult::Success {
        output_file: Some(plan.final_path.clone()),
    }
}

fn cleanup_temp(output: Option<&OutputPlan>) {
    if let Some(plan) = output {
        if plan.temp.exists() {
            let _ = fs::remove_file(&plan.temp);
        }
    }
}

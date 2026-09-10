//! Validation-to-outcome execution pipeline.

use crate::args::{validate_args, ArgMap, ArgValue};
use crate::command::{BinaryResolver, CommandResult, CommandRunner, OutputPlan, RunSpec};
use crate::descriptor::{Capability, Descriptor, Requires};
use crate::outcome::{hint_for_stderr, Progress, Stage, ToolOutcome};
use crate::template::{compute_output_path, render_argv};
use crate::validate::validate_descriptor;
use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default)]
pub struct AvailabilityView {
    pub binaries: HashSet<String>,
    pub portals: HashMap<String, u32>,
}

impl AvailabilityView {
    pub fn probe_binaries(resolver: &BinaryResolver, descriptors: &[Descriptor]) -> Self {
        let binaries = descriptors
            .iter()
            .filter_map(|descriptor| match &descriptor.requires {
                Requires::Binary(binary) if resolver.resolve(binary).is_some() => {
                    Some(binary.clone())
                }
                _ => None,
            })
            .collect();
        Self {
            binaries,
            portals: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub descriptor: Descriptor,
    pub args: ArgMap,
}

#[derive(Debug, Clone)]
pub struct Executor {
    resolver: BinaryResolver,
}

impl Executor {
    pub fn new(resolver: BinaryResolver) -> Self {
        Self { resolver }
    }

    pub async fn run(
        &self,
        request: ExecutionRequest,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        if let Err(errors) = validate_descriptor(&request.descriptor) {
            return failed(
                Stage::Validation,
                join_errors(errors.iter().map(ToString::to_string)),
            );
        }

        let effective = match validate_args(&request.descriptor.args, &request.args) {
            Ok(effective) => effective,
            Err(errors) => {
                return failed(
                    Stage::Validation,
                    join_errors(errors.iter().map(ToString::to_string)),
                );
            }
        };

        match &request.descriptor.requires {
            Requires::Binary(binary) if self.resolver.resolve(binary).is_none() => {
                return unavailable_binary(binary);
            }
            Requires::Portal { .. } => {
                return ToolOutcome::Unavailable {
                    reason: "portal capability probing lands in P3".to_owned(),
                    fix: None,
                };
            }
            Requires::None | Requires::Binary(_) => {}
        }

        match &request.descriptor.capability {
            Capability::Command(command) => {
                self.run_command(&request.descriptor, command, &effective, progress, cancel)
                    .await
            }
            Capability::Portal { .. } => ToolOutcome::Failed {
                stage: Stage::Portal,
                detail: "portal execution is implemented in P3".to_owned(),
                hint: None,
            },
            Capability::Native { .. } => ToolOutcome::Failed {
                stage: Stage::Native,
                detail: "native execution is implemented in P4".to_owned(),
                hint: None,
            },
        }
    }

    async fn run_command(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        let output_path = match compute_output_path(&descriptor.output, effective) {
            Ok(path) => path,
            Err(error) => return failed(Stage::Output, error.to_string()),
        };
        let output_plan = output_path.map(OutputPlan::for_final);
        let argv = match render_argv(
            &command.args,
            effective,
            output_plan.as_ref().map(|plan| plan.temp.as_path()),
        ) {
            Ok(argv) => argv,
            Err(error) => return failed(Stage::Command, error.to_string()),
        };
        let Some(binary) = self.resolver.resolve(&command.binary) else {
            return unavailable_binary(&command.binary);
        };
        let cwd = match effective.get("input") {
            Some(ArgValue::Path(input)) => input.parent().map(ToOwned::to_owned),
            _ => None,
        };

        let result = CommandRunner::execute(RunSpec {
            binary: &binary,
            argv: &argv,
            cwd: cwd.as_deref(),
            output: output_plan.as_ref(),
            progress_spec: &command.progress,
            success: &command.success,
            timeout: command.timeout,
            progress,
            cancel,
        })
        .await;

        match result {
            CommandResult::Success {
                output_file: Some(path),
            } => ToolOutcome::File { path },
            CommandResult::Success { output_file: None } => {
                failed(Stage::Output, "command produced no output")
            }
            CommandResult::NonZeroExit { code, stderr_tail } => ToolOutcome::Failed {
                stage: Stage::Command,
                detail: format!(
                    "exited with {}",
                    code.map(|value| value.to_string())
                        .unwrap_or_else(|| "signal".to_owned())
                ),
                hint: hint_for_stderr(&stderr_tail),
            },
            CommandResult::Timeout => failed(
                Stage::Command,
                format!("timed out after {:?}", command.timeout),
            ),
            CommandResult::Cancelled => ToolOutcome::Cancelled,
            CommandResult::SpawnFailed { detail } => ToolOutcome::Failed {
                stage: Stage::Command,
                detail,
                hint: Some(format!("could not start {}", command.binary)),
            },
            CommandResult::OutputMissing => {
                failed(Stage::Output, "expected output file was not created")
            }
        }
    }
}

fn failed(stage: Stage, detail: impl Into<String>) -> ToolOutcome {
    ToolOutcome::Failed {
        stage,
        detail: detail.into(),
        hint: None,
    }
}

fn unavailable_binary(binary: &str) -> ToolOutcome {
    ToolOutcome::Unavailable {
        reason: format!("{binary} is not installed"),
        fix: Some(format!("install {binary} or add it to your PATH")),
    }
}

fn join_errors(errors: impl Iterator<Item = String>) -> String {
    errors.collect::<Vec<_>>().join("\n")
}

//! Validation-to-outcome execution pipeline.

use crate::args::{validate_args, ArgMap, ArgValue};
use crate::command::{BinaryResolver, CommandResult, CommandRunner, OutputPlan, RunSpec};
use crate::descriptor::{Capability, Descriptor, Requires};
use crate::outcome::{hint_for_stderr, Progress, Stage, ToolOutcome};
use crate::template::{compute_output_path, render_argv};
use crate::validate::validate_descriptor;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
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

#[derive(Clone)]
pub struct Executor {
    resolver: BinaryResolver,
    portal: Option<Arc<dyn crate::portal::PortalRunner>>,
    availability: AvailabilityView,
}

impl std::fmt::Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Executor")
            .field("resolver", &self.resolver)
            .field("portal", &self.portal.is_some())
            .field("availability", &self.availability)
            .finish()
    }
}

impl Executor {
    pub fn new(resolver: BinaryResolver) -> Self {
        Self {
            resolver,
            portal: None,
            availability: AvailabilityView::default(),
        }
    }

    pub fn with_portal(mut self, portal: Arc<dyn crate::portal::PortalRunner>) -> Self {
        self.portal = Some(portal);
        self
    }

    pub fn with_availability(mut self, availability: AvailabilityView) -> Self {
        self.availability = availability;
        self
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
            Requires::Portal { iface, min_ver } => match self.availability.portals.get(iface) {
                Some(version) if version >= min_ver => {}
                Some(version) => {
                    return ToolOutcome::Unavailable {
                        reason: format!(
                            "the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"
                        ),
                        fix: Some(format!(
                            "update your desktop environment to one that provides {iface} portal v{min_ver} or newer"
                        )),
                    };
                }
                None => {
                    return ToolOutcome::Unavailable {
                        reason: format!("the {iface} desktop portal is not available"),
                        fix: Some(
                            "this tool needs a desktop environment with XDG Desktop Portal support"
                                .to_owned(),
                        ),
                    };
                }
            },
            Requires::None | Requires::Binary(_) => {}
        }

        match &request.descriptor.capability {
            Capability::Command(command) => {
                self.run_command(&request.descriptor, command, &effective, progress, cancel)
                    .await
            }
            Capability::Portal { adapter } => match &self.portal {
                Some(runner) => runner.run(adapter, &effective, cancel).await,
                None => ToolOutcome::Failed {
                    stage: Stage::Portal,
                    detail: "this build has no portal runner configured".to_owned(),
                    hint: None,
                },
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

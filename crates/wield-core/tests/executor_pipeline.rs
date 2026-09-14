mod support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::args::ArgValue;
use wield_core::command::BinaryResolver;
use wield_core::descriptor::*;
use wield_core::executor::{ExecutionRequest, Executor};
use wield_core::outcome::{Stage, ToolOutcome};
use wield_core::portal::PortalRunner;

struct StubPortal {
    outcome: ToolOutcome,
    seen: std::sync::Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl PortalRunner for StubPortal {
    async fn run(
        &self,
        adapter: &str,
        _args: &wield_core::ArgMap,
        _cancel: CancellationToken,
    ) -> ToolOutcome {
        *self.seen.lock().unwrap() = Some(adapter.to_string());
        self.outcome.clone()
    }
}

fn convert_descriptor(binary: &str) -> Descriptor {
    Descriptor {
        id: ToolId::parse("image.convert").unwrap(),
        title: "Convert".into(),
        keywords: vec![],
        category: Category::Convert,
        args: vec![
            ArgSpec {
                name: "input".into(),
                label: "in".into(),
                help: None,
                arg_type: ArgType::File {
                    filters: vec![],
                    multiple: false,
                },
                default: None,
                required: true,
                when: None,
            },
            ArgSpec {
                name: "format".into(),
                label: "fmt".into(),
                help: None,
                arg_type: ArgType::Enum {
                    options: vec!["out".into()],
                },
                default: Some(ArgValueLiteral::Str("out".into())),
                required: true,
                when: None,
            },
        ],
        requires: Requires::Binary(binary.into()),
        output: OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        },
        capability: Capability::Command(CommandSpec {
            binary: binary.into(),
            args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(5),
            success: SuccessSpec::ExitZero,
        }),
    }
}

fn portal_descriptor(adapter: &str) -> Descriptor {
    let mut descriptor = convert_descriptor("sh");
    descriptor.requires = Requires::None;
    descriptor.output = OutputSpec::Value(ValueKind::Color);
    descriptor.capability = Capability::Portal {
        adapter: adapter.into(),
    };
    descriptor
}

#[tokio::test]
async fn runs_a_command_tool_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "cp-conv", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let input = dir.path().join("photo.raw");
    std::fs::write(&input, b"PIXELS").unwrap();

    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    let executor = Executor::new(resolver);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input.clone()));

    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: convert_descriptor("cp-conv"),
                args,
            },
            tx,
            CancellationToken::new(),
        )
        .await;

    match outcome {
        ToolOutcome::File { path } => {
            assert_eq!(path, dir.path().join("photo.out"));
            assert_eq!(std::fs::read(&path).unwrap(), b"PIXELS");
        }
        other => panic!("expected File, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_binary_is_unavailable() {
    let resolver = BinaryResolver::with_dirs(vec![]);
    let executor = Executor::new(resolver);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x.raw".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: convert_descriptor("nope-missing"),
                args,
            },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Unavailable { .. }));
}

#[tokio::test]
async fn portal_capability_is_placeholder_failure() {
    let mut descriptor = convert_descriptor("sh");
    descriptor.requires = Requires::None;
    descriptor.capability = Capability::Portal {
        adapter: "screenshot.pick_color".into(),
    };
    descriptor.output = OutputSpec::Value(ValueKind::Color);
    let executor = Executor::new(BinaryResolver::from_env());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(
        outcome,
        ToolOutcome::Failed {
            stage: Stage::Portal,
            ..
        }
    ));
}

#[tokio::test]
async fn injected_portal_runner_receives_the_adapter_key() {
    let stub = Arc::new(StubPortal {
        outcome: ToolOutcome::Value {
            kind: ValueKind::Color,
            data: "#abcdef".into(),
        },
        seen: std::sync::Mutex::new(None),
    });
    let executor = Executor::new(BinaryResolver::from_env()).with_portal(stub.clone());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: portal_descriptor("screenshot.pick_color"),
                args,
            },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Value { .. }));
    assert_eq!(
        stub.seen.lock().unwrap().as_deref(),
        Some("screenshot.pick_color")
    );
}

#[tokio::test]
async fn portal_requires_unmet_is_unavailable() {
    let mut descriptor = portal_descriptor("screenshot.pick_color");
    descriptor.requires = Requires::Portal {
        iface: "Screenshot".into(),
        min_ver: 2,
    };
    let executor = Executor::new(BinaryResolver::from_env()).with_portal(Arc::new(StubPortal {
        outcome: ToolOutcome::Cancelled,
        seen: std::sync::Mutex::new(None),
    }));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Unavailable { .. }));
}

#[tokio::test]
async fn portal_requires_met_runs_the_adapter() {
    let mut descriptor = portal_descriptor("screenshot.pick_color");
    descriptor.requires = Requires::Portal {
        iface: "Screenshot".into(),
        min_ver: 2,
    };
    let mut view = wield_core::AvailabilityView::default();
    view.portals.insert("Screenshot".into(), 2);
    let stub = Arc::new(StubPortal {
        outcome: ToolOutcome::Value {
            kind: ValueKind::Color,
            data: "#000000".into(),
        },
        seen: std::sync::Mutex::new(None),
    });
    let executor = Executor::new(BinaryResolver::from_env())
        .with_portal(stub.clone())
        .with_availability(view);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Value { .. }));
    assert_eq!(
        stub.seen.lock().unwrap().as_deref(),
        Some("screenshot.pick_color")
    );
}

#[tokio::test]
async fn built_descriptor_runs_through_registry_and_executor() {
    use wield_core::{
        ArgSpecBuilder, ArgType, ArgValue, ArgValueLiteral, BinaryResolver, Category,
        CommandSpecBuilder, DescriptorBuilder, ExecutionRequest, Executor, OutputDir, OutputSpec,
        Registry, Requires, ToolOutcome,
    };

    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "passthru", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let input = dir.path().join("in.data");
    std::fs::write(&input, b"OK").unwrap();

    let descriptor = DescriptorBuilder::new("data.pass", "Passthrough", Category::Convert)
        .keyword("copy")
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Input",
                ArgType::File {
                    filters: vec![],
                    multiple: false,
                },
            )
            .required(true)
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "format",
                "Format",
                ArgType::Enum {
                    options: vec!["data".into()],
                },
            )
            .default(ArgValueLiteral::Str("data".into()))
            .required(true)
            .build(),
        )
        .requires(Requires::Binary("passthru".into()))
        .output(OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("passthru")
                .arg("{input}")
                .arg("{output}")
                .timeout(Duration::from_secs(5)),
        )
        .build()
        .expect("descriptor should be valid");

    let mut registry = Registry::new();
    registry.register(descriptor.clone()).unwrap();
    assert_eq!(registry.search("copy").len(), 1);

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::File { ref path } if path.ends_with("in.data")));
}

#[tokio::test]
async fn batch_input_converts_each_file_and_reports_a_summary() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "batch-conv", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let first = dir.path().join("a.raw");
    let second = dir.path().join("b.raw");
    std::fs::write(&first, b"A").unwrap();
    std::fs::write(&second, b"B").unwrap();

    let mut descriptor = convert_descriptor("batch-conv");
    descriptor.args[0].arg_type = ArgType::File {
        filters: vec![],
        multiple: true,
    };

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert(
        "input".to_string(),
        ArgValue::Paths(vec![first.clone(), second.clone()]),
    );
    let (tx, mut rx) = mpsc::channel(64);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;

    match outcome {
        ToolOutcome::Report { title, lines } => {
            assert_eq!(title, "2 of 2 converted");
            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains("a.raw"));
            assert!(lines[1].contains("b.raw"));
        }
        other => panic!("expected Report, got {other:?}"),
    }
    assert_eq!(std::fs::read(dir.path().join("a.out")).unwrap(), b"A");
    assert_eq!(std::fs::read(dir.path().join("b.out")).unwrap(), b"B");

    let mut messages = vec![];
    while let Ok(progress) = rx.try_recv() {
        if let wield_core::outcome::Progress::Message(m) = progress {
            messages.push(m);
        }
    }
    assert!(messages.iter().any(|m| m.contains("1 of 2")));
    assert!(messages.iter().any(|m| m.contains("2 of 2")));
}

#[tokio::test]
async fn batch_failure_line_includes_the_stderr_hint() {
    let dir = tempfile::tempdir().unwrap();
    // Exits non-zero with stderr text hint_for_stderr recognizes, so the
    // Report line should carry both the exit detail and the hint.
    support::write_stub_script(
        dir.path(),
        "bad-conv",
        "#!/bin/sh\necho 'Invalid data found when processing input' 1>&2\nexit 1\n",
    );
    let input = dir.path().join("a.raw");
    std::fs::write(&input, b"A").unwrap();

    let mut descriptor = convert_descriptor("bad-conv");
    descriptor.args[0].arg_type = ArgType::File {
        filters: vec![],
        multiple: true,
    };

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Paths(vec![input]));
    let (tx, _rx) = mpsc::channel(64);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;

    match outcome {
        ToolOutcome::Report { title, lines } => {
            assert_eq!(title, "0 of 1 converted");
            assert_eq!(lines.len(), 1);
            assert!(lines[0].contains("a.raw"));
            assert!(
                lines[0].contains("corrupt"),
                "expected the stderr hint in the failure line, got: {}",
                lines[0]
            );
        }
        other => panic!("expected Report, got {other:?}"),
    }
}

#[tokio::test]
async fn batch_stops_before_the_next_file_once_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "slow-conv", "#!/bin/sh\nsleep 30\n");
    let first = dir.path().join("a.raw");
    let second = dir.path().join("b.raw");
    std::fs::write(&first, b"A").unwrap();
    std::fs::write(&second, b"B").unwrap();

    let mut descriptor = convert_descriptor("slow-conv");
    descriptor.args[0].arg_type = ArgType::File {
        filters: vec![],
        multiple: true,
    };
    if let Capability::Command(spec) = &mut descriptor.capability {
        spec.timeout = Duration::from_secs(30);
    }

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Paths(vec![first, second]));
    let (tx, _rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel_clone.cancel();
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        executor.run(ExecutionRequest { descriptor, args }, tx, cancel),
    )
    .await
    .expect("cancellation should resolve well before the command timeout");
    assert!(matches!(outcome, ToolOutcome::Cancelled));
}

mod support;

use std::collections::BTreeMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::args::ArgValue;
use wield_core::command::BinaryResolver;
use wield_core::descriptor::*;
use wield_core::executor::{ExecutionRequest, Executor};
use wield_core::outcome::{Stage, ToolOutcome};

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

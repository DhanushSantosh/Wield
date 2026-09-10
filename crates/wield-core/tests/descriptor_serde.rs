use std::time::Duration;
use wield_core::descriptor::*;

fn sample() -> Descriptor {
    Descriptor {
        id: ToolId::parse("image.convert").unwrap(),
        title: "Convert image".into(),
        keywords: vec!["image".into(), "resize".into()],
        category: Category::Convert,
        args: vec![
            ArgSpec {
                name: "input".into(),
                label: "Image".into(),
                help: None,
                arg_type: ArgType::File {
                    filters: vec![FileFilter {
                        label: "Images".into(),
                        extensions: vec!["png".into(), "jpg".into()],
                    }],
                    multiple: false,
                },
                default: None,
                required: true,
                when: None,
            },
            ArgSpec {
                name: "format".into(),
                label: "Output format".into(),
                help: Some("Target container".into()),
                arg_type: ArgType::Enum {
                    options: vec!["png".into(), "webp".into()],
                },
                default: Some(ArgValueLiteral::Str("webp".into())),
                required: true,
                when: None,
            },
            ArgSpec {
                name: "width".into(),
                label: "Width".into(),
                help: None,
                arg_type: ArgType::Int {
                    range: Some([1, 10_000]),
                    step: Some(1),
                },
                default: None,
                required: false,
                when: Some(When {
                    arg: "resize".into(),
                    in_values: vec![ArgValueLiteral::Bool(true)],
                }),
            },
        ],
        requires: Requires::Binary("magick".into()),
        output: OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        },
        capability: Capability::Command(CommandSpec {
            binary: "magick".into(),
            args: vec![
                "{input}".into(),
                CommandArg {
                    template: "-resize".into(),
                    when: Some(When {
                        arg: "resize".into(),
                        in_values: vec![ArgValueLiteral::Bool(true)],
                    }),
                },
                CommandArg {
                    template: "{width}x".into(),
                    when: Some(When {
                        arg: "resize".into(),
                        in_values: vec![ArgValueLiteral::Bool(true)],
                    }),
                },
                "{output}".into(),
            ],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(300),
            success: SuccessSpec::ExitZero,
        }),
    }
}

#[test]
fn descriptor_round_trips_through_json() {
    let original = sample();
    let json = serde_json::to_string_pretty(&original).unwrap();
    let parsed: Descriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn tool_id_rejects_non_namespaced() {
    assert!(ToolId::parse("convert").is_err());
    assert!(ToolId::parse("Image.Convert").is_err());
    assert!(ToolId::parse("image.convert").is_ok());
    assert!(ToolId::parse("pdf.tools.split").is_ok());
}

#[test]
fn when_field_serializes_as_in() {
    let when = When {
        arg: "resize".into(),
        in_values: vec![ArgValueLiteral::Bool(true)],
    };
    let json = serde_json::to_string(&when).unwrap();
    assert!(json.contains("\"in\""), "got {json}");
    assert!(!json.contains("in_values"), "got {json}");
}

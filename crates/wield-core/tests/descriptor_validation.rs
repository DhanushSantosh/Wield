use std::time::Duration;
use wield_core::descriptor::*;
use wield_core::validate::validate_descriptor;

fn base_command_descriptor() -> Descriptor {
    Descriptor {
        id: ToolId::parse("x.y").unwrap(),
        title: "t".into(),
        keywords: vec![],
        category: Category::Convert,
        args: vec![ArgSpec {
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
        }],
        requires: Requires::Binary("tool".into()),
        output: OutputSpec::File {
            name: "{input_stem}.out".into(),
            dir: OutputDir::SameAsInput,
        },
        capability: Capability::Command(CommandSpec {
            binary: "tool".into(),
            args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(60),
            success: SuccessSpec::ExitZero,
        }),
    }
}

#[test]
fn accepts_a_well_formed_descriptor() {
    assert!(validate_descriptor(&base_command_descriptor()).is_ok());
}

#[test]
fn rejects_duplicate_arg_names() {
    let mut descriptor = base_command_descriptor();
    descriptor.args.push(descriptor.args[0].clone());
    let errors = validate_descriptor(&descriptor).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.at.starts_with("args[1]") && error.message.contains("duplicate")));
}

#[test]
fn rejects_when_referencing_unknown_arg() {
    let mut descriptor = base_command_descriptor();
    descriptor.args.push(ArgSpec {
        name: "width".into(),
        label: "w".into(),
        help: None,
        arg_type: ArgType::Int {
            range: None,
            step: None,
        },
        default: None,
        required: false,
        when: Some(When {
            arg: "resize".into(),
            in_values: vec![ArgValueLiteral::Bool(true)],
        }),
    });
    let errors = validate_descriptor(&descriptor).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.at.contains("when.arg") && error.message.contains("resize")));
}

#[test]
fn rejects_unknown_template_placeholder() {
    let mut descriptor = base_command_descriptor();
    if let Capability::Command(ref mut command) = descriptor.capability {
        command.args.push("{bogus}".into());
    }
    let errors = validate_descriptor(&descriptor).unwrap_err();
    assert!(errors.iter().any(|error| error.message.contains("bogus")));
}

#[test]
fn rejects_enum_default_not_in_options() {
    let mut descriptor = base_command_descriptor();
    descriptor.args.push(ArgSpec {
        name: "format".into(),
        label: "f".into(),
        help: None,
        arg_type: ArgType::Enum {
            options: vec!["png".into()],
        },
        default: Some(ArgValueLiteral::Str("webp".into())),
        required: true,
        when: None,
    });
    let errors = validate_descriptor(&descriptor).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.at.contains("default") && error.message.contains("webp")));
}

#[test]
fn rejects_output_placeholder_in_output_name() {
    let mut descriptor = base_command_descriptor();
    descriptor.output = OutputSpec::File {
        name: "{output}.x".into(),
        dir: OutputDir::SameAsInput,
    };
    let errors = validate_descriptor(&descriptor).unwrap_err();
    assert!(errors
        .iter()
        .any(|error| error.at.starts_with("output") && error.message.contains("output")));
}

//! Descriptor validation.

use crate::descriptor::{
    ArgSpec, ArgType, ArgValueLiteral, Capability, CommandSpec, Descriptor, OutputSpec, When,
};
use crate::error::DescriptorError;
use crate::template::placeholders;
use std::collections::{HashMap, HashSet};

pub fn validate_descriptor(descriptor: &Descriptor) -> Result<(), Vec<DescriptorError>> {
    let mut errors = Vec::new();
    check_arg_names(&descriptor.args, &mut errors);
    check_arg_types(&descriptor.args, &mut errors);
    check_arg_when(&descriptor.args, &mut errors);
    check_capability_and_output(descriptor, &mut errors);

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn error(errors: &mut Vec<DescriptorError>, at: impl Into<String>, message: impl Into<String>) {
    errors.push(DescriptorError {
        at: at.into(),
        message: message.into(),
    });
}

fn check_arg_names(args: &[ArgSpec], errors: &mut Vec<DescriptorError>) {
    let mut seen = HashSet::new();
    for (index, arg) in args.iter().enumerate() {
        if arg.name.is_empty() {
            error(
                errors,
                format!("args[{index}].name"),
                "argument name must not be empty",
            );
        } else if !seen.insert(arg.name.as_str()) {
            error(
                errors,
                format!("args[{index}].name"),
                format!("duplicate argument name: {}", arg.name),
            );
        }
    }
}

fn check_arg_types(args: &[ArgSpec], errors: &mut Vec<DescriptorError>) {
    let mut multi_file_args = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        match &arg.arg_type {
            ArgType::Int { range, step } => {
                if let Some([low, high]) = range {
                    if low > high {
                        error(
                            errors,
                            format!("args[{index}].arg_type.range"),
                            "range minimum exceeds maximum",
                        );
                    }
                }
                if step.is_some_and(|value| value <= 0) {
                    error(
                        errors,
                        format!("args[{index}].arg_type.step"),
                        "step must be greater than zero",
                    );
                }
            }
            ArgType::Float {
                range: Some([low, high]),
            } if low > high => {
                error(
                    errors,
                    format!("args[{index}].arg_type.range"),
                    "range minimum exceeds maximum",
                );
            }
            ArgType::Enum { options } => {
                if options.is_empty() {
                    error(
                        errors,
                        format!("args[{index}].arg_type.options"),
                        "enum options must not be empty",
                    );
                }
                let mut seen = HashSet::new();
                if options.iter().any(|option| !seen.insert(option)) {
                    error(
                        errors,
                        format!("args[{index}].arg_type.options"),
                        "enum options contain duplicates",
                    );
                }
            }
            ArgType::File { multiple: true, .. } => {
                multi_file_args.push(index);
            }
            _ => {}
        }

        if let Some(default) = &arg.default {
            if !literal_matches_type(default, &arg.arg_type) {
                error(
                    errors,
                    format!("args[{index}].default"),
                    format!("default {default:?} is incompatible with argument type"),
                );
            } else if !literal_within_constraints(default, &arg.arg_type) {
                error(
                    errors,
                    format!("args[{index}].default"),
                    format!("default {default:?} is outside the allowed values"),
                );
            }
        }
    }

    if multi_file_args.len() > 1 {
        let indices = multi_file_args
            .iter()
            .map(|index| format!("args[{index}]"))
            .collect::<Vec<_>>()
            .join(", ");
        error(
            errors,
            "args",
            format!(
                "a descriptor may have at most one `multiple: true` File argument, found {}: {indices}",
                multi_file_args.len()
            ),
        );
    }
}

fn check_arg_when(args: &[ArgSpec], errors: &mut Vec<DescriptorError>) {
    let positions: HashMap<&str, usize> = args
        .iter()
        .enumerate()
        .map(|(index, arg)| (arg.name.as_str(), index))
        .collect();

    for (index, arg) in args.iter().enumerate() {
        let Some(when) = &arg.when else {
            continue;
        };
        match positions.get(when.arg.as_str()).copied() {
            None => error(
                errors,
                format!("args[{index}].when.arg"),
                format!("unknown argument: {}", when.arg),
            ),
            Some(position) if position >= index => error(
                errors,
                format!("args[{index}].when.arg"),
                format!("{} must reference an earlier argument", when.arg),
            ),
            Some(position) => check_when_values(
                when,
                &args[position].arg_type,
                format!("args[{index}].when.in"),
                errors,
            ),
        }
    }
}

fn check_capability_and_output(descriptor: &Descriptor, errors: &mut Vec<DescriptorError>) {
    match (&descriptor.capability, &descriptor.output) {
        (Capability::Command(command), OutputSpec::File { name, .. }) => {
            check_command_templates(descriptor, command, name, errors);
        }
        (Capability::Command(command), OutputSpec::Directory { name, .. }) => {
            check_command_templates(descriptor, command, name, errors);
        }
        (Capability::Command(_), _) => error(
            errors,
            "output",
            "Command capability requires File or Directory output in P2",
        ),
        (_, OutputSpec::File { .. } | OutputSpec::Directory { .. }) => error(
            errors,
            "output",
            "File or Directory output requires Command capability in P2",
        ),
        _ => {}
    }
}

fn check_command_templates(
    descriptor: &Descriptor,
    command: &CommandSpec,
    output_name: &str,
    errors: &mut Vec<DescriptorError>,
) {
    let names: HashSet<&str> = descriptor
        .args
        .iter()
        .map(|arg| arg.name.as_str())
        .collect();
    let positions: HashMap<&str, &ArgType> = descriptor
        .args
        .iter()
        .map(|arg| (arg.name.as_str(), &arg.arg_type))
        .collect();

    for (index, segment) in command.args.iter().enumerate() {
        check_template(
            &segment.template,
            format!("capability.command.args[{index}].template"),
            true,
            &names,
            &positions,
            errors,
        );
        if let Some(when) = &segment.when {
            match positions.get(when.arg.as_str()) {
                Some(arg_type) => check_when_values(
                    when,
                    arg_type,
                    format!("capability.command.args[{index}].when.in"),
                    errors,
                ),
                None => error(
                    errors,
                    format!("capability.command.args[{index}].when.arg"),
                    format!("unknown argument: {}", when.arg),
                ),
            }
        }
    }

    check_template(
        output_name,
        "output.name".to_owned(),
        false,
        &names,
        &positions,
        errors,
    );
}

fn check_template(
    template: &str,
    at: String,
    allow_output: bool,
    names: &HashSet<&str>,
    positions: &HashMap<&str, &ArgType>,
    errors: &mut Vec<DescriptorError>,
) {
    let tokens = match placeholders(template) {
        Ok(tokens) => tokens,
        Err(problem) => {
            error(errors, at, problem.to_string());
            return;
        }
    };

    for token in tokens {
        if matches!(token.as_str(), "output" | "output_dir") {
            if !allow_output {
                error(
                    errors,
                    &at,
                    "output placeholder is not allowed in output name",
                );
            }
            continue;
        }

        if matches!(token.as_str(), "input" | "input_stem" | "input_dir") {
            let valid_input = matches!(positions.get("input"), Some(ArgType::File { .. }));
            if !valid_input {
                error(
                    errors,
                    &at,
                    format!("{token} requires a file input argument"),
                );
            }
            continue;
        }

        if token == "format" {
            if !names.contains("format") {
                error(errors, &at, "format placeholder requires a format argument");
            }
            continue;
        }

        if !names.contains(token.as_str()) {
            error(errors, &at, format!("unknown placeholder: {token}"));
        }
    }
}

fn check_when_values(
    when: &When,
    arg_type: &ArgType,
    at: String,
    errors: &mut Vec<DescriptorError>,
) {
    for value in &when.in_values {
        if !literal_matches_type(value, arg_type) || !literal_within_constraints(value, arg_type) {
            error(
                errors,
                &at,
                format!(
                    "conditional value {value:?} is incompatible with {}",
                    when.arg
                ),
            );
        }
    }
}

fn literal_matches_type(value: &ArgValueLiteral, arg_type: &ArgType) -> bool {
    matches!(
        (value, arg_type),
        (
            ArgValueLiteral::Str(_),
            ArgType::Str | ArgType::Text | ArgType::Enum { .. }
        ) | (ArgValueLiteral::Int(_), ArgType::Int { .. })
            | (ArgValueLiteral::Float(_), ArgType::Float { .. })
            | (ArgValueLiteral::Bool(_), ArgType::Bool)
    )
}

fn literal_within_constraints(value: &ArgValueLiteral, arg_type: &ArgType) -> bool {
    match (value, arg_type) {
        (ArgValueLiteral::Str(value), ArgType::Enum { options }) => options.contains(value),
        (
            ArgValueLiteral::Int(value),
            ArgType::Int {
                range: Some([low, high]),
                ..
            },
        ) => value >= low && value <= high,
        (
            ArgValueLiteral::Float(value),
            ArgType::Float {
                range: Some([low, high]),
            },
        ) => value >= low && value <= high,
        _ => true,
    }
}

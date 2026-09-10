//! Command template rendering. See plan P2 Tasks 3 and 6.

use crate::args::{value_satisfies_literal, ArgMap, ArgValue};
use crate::descriptor::{CommandArg, OutputDir, OutputSpec};
use std::path::{Path, PathBuf};

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    #[error("malformed placeholder in {0}")]
    MalformedPlaceholder(String),
    #[error("unresolved placeholder in output name: {0}")]
    UnresolvedInOutputName(String),
    #[error("input argument is missing")]
    MissingInputArg,
    #[error("path is not valid UTF-8: {}", .0.display())]
    NonUtf8Path(PathBuf),
}

pub(crate) fn placeholders(template: &str) -> Result<Vec<String>, TemplateError> {
    let bytes = template.as_bytes();
    let mut names = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'{' {
            index += 1;
            continue;
        }

        let rest = &template[index + 1..];
        let Some(close_offset) = rest.find('}') else {
            return Err(TemplateError::MalformedPlaceholder(template.to_owned()));
        };
        let close = index + 1 + close_offset;
        let name = &template[index + 1..close];
        let mut chars = name.chars();
        let valid = matches!(chars.next(), Some(first) if first.is_ascii_lowercase() || first == '_')
            && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
        if !valid {
            return Err(TemplateError::MalformedPlaceholder(template.to_owned()));
        }
        names.push(name.to_owned());
        index = close + 1;
    }

    Ok(names)
}

pub fn render_output_name(
    name_template: &str,
    effective: &ArgMap,
) -> Result<String, TemplateError> {
    let tokens = placeholders(name_template)?;
    let mut rendered = name_template.to_owned();
    for token in tokens {
        if token == "output" {
            return Err(TemplateError::UnresolvedInOutputName(token));
        }
        let Some(value) = resolve(&token, effective, None)? else {
            return Err(TemplateError::UnresolvedInOutputName(token));
        };
        rendered = rendered.replace(&format!("{{{token}}}"), &value);
    }
    Ok(rendered)
}

pub fn compute_output_path(
    output: &OutputSpec,
    effective: &ArgMap,
) -> Result<Option<PathBuf>, TemplateError> {
    let OutputSpec::File { name, dir } = output else {
        return Ok(None);
    };
    let name = render_output_name(name, effective)?;
    let directory = match dir {
        OutputDir::Fixed(path) => path.clone(),
        OutputDir::SameAsInput => {
            let Some(ArgValue::Path(input)) = effective.get("input") else {
                return Err(TemplateError::MissingInputArg);
            };
            input.parent().unwrap_or_else(|| Path::new("")).to_path_buf()
        }
    };
    Ok(Some(directory.join(name)))
}

pub fn render_argv(
    args: &[CommandArg],
    effective: &ArgMap,
    output_path: Option<&Path>,
) -> Result<Vec<String>, TemplateError> {
    let mut rendered = Vec::new();
    for segment in args {
        if segment.when.as_ref().is_some_and(|when| {
            !effective.get(&when.arg).is_some_and(|value| {
                when.in_values
                    .iter()
                    .any(|literal| value_satisfies_literal(value, literal))
            })
        }) {
            continue;
        }

        let tokens = placeholders(&segment.template)?;
        let mut value = segment.template.clone();
        let mut unresolved = false;
        for token in tokens {
            let Some(replacement) = resolve(&token, effective, output_path)? else {
                unresolved = true;
                break;
            };
            value = value.replace(&format!("{{{token}}}"), &replacement);
        }
        if !unresolved {
            rendered.push(value);
        }
    }
    Ok(rendered)
}

fn resolve(
    name: &str,
    effective: &ArgMap,
    output_path: Option<&Path>,
) -> Result<Option<String>, TemplateError> {
    if name == "output" {
        let Some(path) = output_path else {
            return Err(TemplateError::UnresolvedInOutputName(name.to_owned()));
        };
        return path_string(path).map(Some);
    }

    if matches!(name, "input" | "input_stem" | "input_dir") {
        let Some(ArgValue::Path(input)) = effective.get("input") else {
            return Ok(None);
        };
        return match name {
            "input" => path_string(input).map(Some),
            "input_stem" => match input.file_stem() {
                Some(stem) => stem
                    .to_str()
                    .map(|value| Some(value.to_owned()))
                    .ok_or_else(|| TemplateError::NonUtf8Path(input.clone())),
                None => Ok(None),
            },
            "input_dir" => match input.parent() {
                Some(parent) => path_string(parent).map(Some),
                None => Ok(Some(String::new())),
            },
            _ => Ok(None),
        };
    }

    let Some(value) = effective.get(name) else {
        return Ok(None);
    };
    match value {
        ArgValue::Path(path) => path_string(path).map(Some),
        ArgValue::Paths(_) => Ok(None),
        _ => Ok(value.as_display_string()),
    }
}

fn path_string(path: &Path) -> Result<String, TemplateError> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| TemplateError::NonUtf8Path(path.to_path_buf()))
}

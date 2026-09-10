//! Runtime argument values and conditional visibility.

use crate::descriptor::{ArgSpec, ArgType, ArgValueLiteral};
use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArgValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Path(PathBuf),
    Paths(Vec<PathBuf>),
}

impl ArgValue {
    pub fn as_display_string(&self) -> Option<String> {
        match self {
            Self::Str(value) => Some(value.clone()),
            Self::Int(value) => Some(value.to_string()),
            Self::Float(value) => Some(value.to_string()),
            Self::Bool(value) => Some(value.to_string()),
            Self::Path(path) => path.to_str().map(ToOwned::to_owned),
            Self::Paths(_) => None,
        }
    }
}

pub type ArgMap = BTreeMap<String, ArgValue>;

pub fn literal_to_value(literal: &ArgValueLiteral) -> ArgValue {
    match literal {
        ArgValueLiteral::Str(value) => ArgValue::Str(value.clone()),
        ArgValueLiteral::Int(value) => ArgValue::Int(*value),
        ArgValueLiteral::Float(value) => ArgValue::Float(*value),
        ArgValueLiteral::Bool(value) => ArgValue::Bool(*value),
    }
}

pub fn value_satisfies_literal(value: &ArgValue, literal: &ArgValueLiteral) -> bool {
    match (value, literal) {
        (ArgValue::Str(left), ArgValueLiteral::Str(right)) => left == right,
        (ArgValue::Int(left), ArgValueLiteral::Int(right)) => left == right,
        (ArgValue::Float(left), ArgValueLiteral::Float(right)) => left == right,
        (ArgValue::Bool(left), ArgValueLiteral::Bool(right)) => left == right,
        _ => false,
    }
}

pub fn visible_args<'a>(specs: &'a [ArgSpec], values: &ArgMap) -> Vec<&'a ArgSpec> {
    let mut visible_names = HashSet::new();
    let mut visible = Vec::new();

    for spec in specs {
        let is_visible = match &spec.when {
            None => true,
            Some(when) => {
                visible_names.contains(when.arg.as_str())
                    && values.get(&when.arg).is_some_and(|value| {
                        when.in_values
                            .iter()
                            .any(|literal| value_satisfies_literal(value, literal))
                    })
            }
        };
        if is_visible {
            visible_names.insert(spec.name.as_str());
            visible.push(spec);
        }
    }

    visible
}

pub fn validate_args(specs: &[ArgSpec], input: &ArgMap) -> Result<ArgMap, Vec<ValidationError>> {
    let visible = visible_args(specs, input);
    let mut effective = ArgMap::new();
    let mut errors = Vec::new();

    for spec in visible {
        let value = input
            .get(&spec.name)
            .cloned()
            .or_else(|| spec.default.as_ref().map(literal_to_value));

        let Some(value) = value else {
            if spec.required {
                errors.push(ValidationError {
                    field: spec.name.clone(),
                    message: "required".to_owned(),
                });
            }
            continue;
        };

        if let Some(message) = validate_value(&value, &spec.arg_type) {
            errors.push(ValidationError {
                field: spec.name.clone(),
                message,
            });
        } else {
            effective.insert(spec.name.clone(), value);
        }
    }

    if errors.is_empty() {
        Ok(effective)
    } else {
        Err(errors)
    }
}

fn validate_value(value: &ArgValue, arg_type: &ArgType) -> Option<String> {
    match (value, arg_type) {
        (ArgValue::Path(_), ArgType::File { .. } | ArgType::Dir) => None,
        (ArgValue::Paths(_), ArgType::File { multiple: true, .. }) => None,
        (ArgValue::Str(_), ArgType::Str | ArgType::Text) => None,
        (ArgValue::Bool(_), ArgType::Bool) => None,
        (ArgValue::Str(value), ArgType::Enum { options }) => {
            if options.contains(value) {
                None
            } else {
                Some(format!("must be one of: {}", options.join(", ")))
            }
        }
        (ArgValue::Int(value), ArgType::Int { range, step }) => {
            if let Some([low, high]) = range {
                if value < low || value > high {
                    return Some(format!("must be between {low} and {high}"));
                }
            }
            if let Some(step) = step {
                let base = range.map_or(0_i128, |[low, _]| i128::from(low));
                if (i128::from(*value) - base) % i128::from(*step) != 0 {
                    return Some(format!("must align to step {step}"));
                }
            }
            None
        }
        (ArgValue::Float(value), ArgType::Float { range }) => {
            if let Some([low, high]) = range {
                if value < low || value > high {
                    return Some(format!("must be between {low} and {high}"));
                }
            }
            None
        }
        _ => Some("value has the wrong type".to_owned()),
    }
}

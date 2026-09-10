//! Runtime argument values and conditional visibility.

use crate::descriptor::{ArgSpec, ArgValueLiteral};
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

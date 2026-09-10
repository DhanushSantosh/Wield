//! Parse the tokens after `wield <id>` into an [`ArgMap`], guided by the
//! [`CliSurface`]. Descriptor validation (ranges, enum membership, required)
//! stays in the executor — this only handles shape and type coercion.

use std::collections::BTreeMap;
use wield_core::{ArgType, ArgValue};

use crate::surface::CliSurface;

#[derive(Debug, PartialEq)]
pub struct UsageError(pub String);

fn err(message: impl Into<String>) -> UsageError {
    UsageError(message.into())
}

fn coerce(name: &str, arg_type: &ArgType, raw: &str) -> Result<ArgValue, UsageError> {
    match arg_type {
        ArgType::File { .. } | ArgType::Dir => Ok(ArgValue::Path(raw.into())),
        ArgType::Int { .. } => raw
            .parse::<i64>()
            .map(ArgValue::Int)
            .map_err(|_| err(format!("--{name} expects a whole number, got {raw:?}"))),
        ArgType::Float { .. } => raw
            .parse::<f64>()
            .map(ArgValue::Float)
            .map_err(|_| err(format!("--{name} expects a number, got {raw:?}"))),
        ArgType::Bool => Ok(ArgValue::Bool(true)),
        ArgType::Str | ArgType::Text | ArgType::Enum { .. } => Ok(ArgValue::Str(raw.to_owned())),
    }
}

/// Parse `tokens` (everything after `wield <id>`) into an `ArgMap`.
pub fn parse_args(
    surface: &CliSurface<'_>,
    tokens: &[String],
) -> Result<BTreeMap<String, ArgValue>, UsageError> {
    let mut map = BTreeMap::new();
    let mut positional_filled = false;
    let mut multi: BTreeMap<String, Vec<std::path::PathBuf>> = BTreeMap::new();
    let mut index = 0;

    while index < tokens.len() {
        let token = &tokens[index];
        index += 1;

        if let Some(rest) = token.strip_prefix("--") {
            let (name, inline_value) = match rest.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (rest, None),
            };
            let Some(flag) = surface.flags.iter().find(|f| f.spec.name == name) else {
                return Err(err(format!("unknown option --{name}")));
            };
            let value_str = if flag.takes_value {
                match inline_value {
                    Some(v) => v,
                    None => {
                        let next = tokens
                            .get(index)
                            .ok_or_else(|| err(format!("--{name} expects a value")))?;
                        index += 1;
                        next.clone()
                    }
                }
            } else {
                if inline_value.is_some() {
                    return Err(err(format!("--{name} does not take a value")));
                }
                String::new()
            };

            match &flag.spec.arg_type {
                ArgType::File { multiple: true, .. } => {
                    multi
                        .entry(flag.spec.name.clone())
                        .or_default()
                        .push(value_str.into());
                }
                other => {
                    map.insert(flag.spec.name.clone(), coerce(name, other, &value_str)?);
                }
            }
        } else {
            let Some(positional) = surface.positional else {
                return Err(err(format!("unexpected argument {token:?}")));
            };
            match &positional.arg_type {
                ArgType::File { multiple: true, .. } => {
                    multi
                        .entry(positional.name.clone())
                        .or_default()
                        .push(token.into());
                }
                other => {
                    if positional_filled {
                        return Err(err(format!("unexpected argument {token:?}")));
                    }
                    positional_filled = true;
                    map.insert(
                        positional.name.clone(),
                        coerce(&positional.name, other, token)?,
                    );
                }
            }
        }
    }

    for (name, paths) in multi {
        map.insert(name, ArgValue::Paths(paths));
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::CliSurface;
    use wield_tools::builtin_registry;

    fn tokens(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_positional_and_flags() {
        let registry = builtin_registry();
        let surface = CliSurface::from_descriptor(registry.get("image.convert").unwrap());
        let map = parse_args(
            &surface,
            &tokens(&["a.png", "--format", "webp", "--width", "800"]),
        )
        .unwrap();
        assert_eq!(map.get("input"), Some(&ArgValue::Path("a.png".into())));
        assert_eq!(map.get("format"), Some(&ArgValue::Str("webp".into())));
        assert_eq!(map.get("width"), Some(&ArgValue::Int(800)));
    }

    #[test]
    fn accepts_equals_form() {
        let registry = builtin_registry();
        let surface = CliSurface::from_descriptor(registry.get("image.convert").unwrap());
        let map = parse_args(&surface, &tokens(&["a.png", "--format=png"])).unwrap();
        assert_eq!(map.get("format"), Some(&ArgValue::Str("png".into())));
    }

    #[test]
    fn rejects_unknown_flag_and_bad_int() {
        let registry = builtin_registry();
        let surface = CliSurface::from_descriptor(registry.get("image.convert").unwrap());
        assert!(parse_args(&surface, &tokens(&["a.png", "--bogus", "x"])).is_err());
        assert!(parse_args(&surface, &tokens(&["a.png", "--width", "wide"])).is_err());
    }

    #[test]
    fn rejects_second_positional() {
        let registry = builtin_registry();
        let surface = CliSurface::from_descriptor(registry.get("image.convert").unwrap());
        assert!(parse_args(&surface, &tokens(&["a.png", "b.png"])).is_err());
    }
}

//! Turn a [`Descriptor`] into a command-line surface: one positional argument
//! (the first required file/dir), everything else a `--flag`.

use wield_core::{ArgSpec, ArgType, Descriptor};

/// A `--flag` derived from an [`ArgSpec`].
pub struct FlagSpec<'a> {
    pub spec: &'a ArgSpec,
    /// `false` only for `ArgType::Bool` (bare `--flag`).
    pub takes_value: bool,
}

/// The CLI shape of one tool.
pub struct CliSurface<'a> {
    /// The single positional argument, if the tool has a required file/dir arg.
    pub positional: Option<&'a ArgSpec>,
    pub flags: Vec<FlagSpec<'a>>,
}

impl<'a> CliSurface<'a> {
    pub fn from_descriptor(descriptor: &'a Descriptor) -> Self {
        let positional = descriptor.args.iter().find(|arg| {
            arg.required && matches!(arg.arg_type, ArgType::File { .. } | ArgType::Dir)
        });
        let flags = descriptor
            .args
            .iter()
            .filter(|arg| positional.is_none_or(|p| !std::ptr::eq(*arg, p)))
            .map(|spec| FlagSpec {
                spec,
                takes_value: !matches!(spec.arg_type, ArgType::Bool),
            })
            .collect();
        Self { positional, flags }
    }
}

fn type_hint(arg_type: &ArgType) -> &'static str {
    match arg_type {
        ArgType::File { .. } => "<path>",
        ArgType::Dir => "<dir>",
        ArgType::Int { .. } => "<number>",
        ArgType::Float { .. } => "<number>",
        ArgType::Bool => "",
        ArgType::Enum { .. } => "<choice>",
        ArgType::Str | ArgType::Text => "<text>",
    }
}

/// The body printed for `wield <id> --help`.
pub fn help_text(descriptor: &Descriptor) -> String {
    let surface = CliSurface::from_descriptor(descriptor);
    let mut out = String::new();
    out.push_str(&format!(
        "{} — {}\n\n",
        descriptor.id.as_ref(),
        descriptor.title
    ));

    let positional_name = surface.positional.map(|p| p.name.to_uppercase());
    match &positional_name {
        Some(name) => out.push_str(&format!(
            "Usage: wield {} <{name}> [options]\n",
            descriptor.id.as_ref()
        )),
        None => out.push_str(&format!(
            "Usage: wield {} [options]\n",
            descriptor.id.as_ref()
        )),
    }

    if let Some(positional) = surface.positional {
        out.push_str(&format!(
            "\nArguments:\n  <{}>  {}\n",
            positional.name.to_uppercase(),
            positional.label
        ));
    }

    if !surface.flags.is_empty() {
        out.push_str("\nOptions:\n");
        for flag in &surface.flags {
            let hint = type_hint(&flag.spec.arg_type);
            let lead = if hint.is_empty() {
                format!("--{}", flag.spec.name)
            } else {
                format!("--{} {hint}", flag.spec.name)
            };
            let mut notes = Vec::new();
            notes.push(
                if flag.spec.required {
                    "required"
                } else {
                    "optional"
                }
                .to_string(),
            );
            if let Some(default) = &flag.spec.default {
                notes.push(format!("default {default:?}"));
            }
            if let ArgType::Enum { options } = &flag.spec.arg_type {
                notes.push(format!("one of: {}", options.join(", ")));
            }
            out.push_str(&format!("  {lead}  ({})\n", notes.join(", ")));
            if let Some(help) = &flag.spec.help {
                out.push_str(&format!("      {help}\n"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wield_tools::builtin_registry;

    #[test]
    fn image_convert_has_input_positional_and_three_flags() {
        let registry = builtin_registry();
        let descriptor = registry.get("image.convert").unwrap();
        let surface = CliSurface::from_descriptor(descriptor);
        assert_eq!(surface.positional.unwrap().name, "input");
        let names: Vec<_> = surface.flags.iter().map(|f| f.spec.name.as_str()).collect();
        assert_eq!(names, vec!["format", "width", "quality"]);
        assert!(surface.flags.iter().all(|f| f.takes_value));
    }

    #[test]
    fn color_pick_has_no_positional_and_no_flags() {
        let registry = builtin_registry();
        let descriptor = registry.get("color.pick").unwrap();
        let surface = CliSurface::from_descriptor(descriptor);
        assert!(surface.positional.is_none());
        assert!(surface.flags.is_empty());
    }

    #[test]
    fn help_text_names_the_positional_and_each_flag() {
        let registry = builtin_registry();
        let text = help_text(registry.get("image.convert").unwrap());
        assert!(text.contains("image.convert"));
        assert!(text.contains("--format"));
        assert!(text.contains("--width"));
        assert!(text.contains("required") && text.contains("optional"));
    }
}

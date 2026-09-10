//! Command template rendering. See plan P2 Tasks 3 and 6.

use std::path::PathBuf;

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

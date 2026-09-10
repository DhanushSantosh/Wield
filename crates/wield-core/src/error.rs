//! Structured error types for `wield-core`.

use std::fmt;

/// A single descriptor-validation failure. Collected into a list so the caller
/// sees every problem at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorError {
    /// Dotted location, e.g. `args[2].when.arg` or `id`.
    pub at: String,
    /// Human-readable explanation.
    pub message: String,
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

/// A single argument-validation failure, keyed by arg `name` so the UI can map
/// it back onto the generated form field.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

/// Crate-level error. Execution failures are reported as `ToolOutcome`, not this.
#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("descriptor invalid ({} problems)", .0.len())]
    Descriptor(Vec<DescriptorError>),

    #[error("argument validation failed ({} problems)", .0.len())]
    Validation(Vec<ValidationError>),

    #[error("template error: {0}")]
    Template(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

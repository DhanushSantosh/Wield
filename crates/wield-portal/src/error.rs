//! Portal error type and its mapping to `ToolOutcome`.

use wield_core::{Stage, ToolOutcome};

/// An error returned by a desktop portal operation.
#[derive(Debug)]
pub enum PortalError {
    /// The user dismissed the portal dialog.
    Cancelled,
    /// A D-Bus or portal transport/protocol failure.
    Transport(String),
    /// The portal returned a response that could not be interpreted.
    BadResponse(String),
}

impl std::fmt::Display for PortalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("portal request cancelled"),
            Self::Transport(detail) => write!(formatter, "portal transport failed: {detail}"),
            Self::BadResponse(detail) => {
                write!(formatter, "portal returned a bad response: {detail}")
            }
        }
    }
}

impl std::error::Error for PortalError {}

impl PortalError {
    /// Convert the error into the executor's public outcome model.
    pub fn into_outcome(self) -> ToolOutcome {
        match self {
            Self::Cancelled => ToolOutcome::Cancelled,
            Self::Transport(detail) => ToolOutcome::Failed {
                stage: Stage::Portal,
                detail,
                hint: Some("the desktop portal service may not be running".to_owned()),
            },
            Self::BadResponse(detail) => ToolOutcome::Failed {
                stage: Stage::Portal,
                detail,
                hint: None,
            },
        }
    }
}

impl From<zbus::Error> for PortalError {
    fn from(error: zbus::Error) -> Self {
        Self::Transport(error.to_string())
    }
}

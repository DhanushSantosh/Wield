//! Tool outcomes. See plan P2 Task 8.

use crate::descriptor::ValueKind;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Stage {
    Validation,
    Portal,
    Command,
    Output,
    Native,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ToolOutcome {
    Value {
        kind: ValueKind,
        data: String,
    },
    File {
        path: PathBuf,
    },
    Report {
        title: String,
        lines: Vec<String>,
    },
    Unavailable {
        reason: String,
        fix: Option<String>,
    },
    Failed {
        stage: Stage,
        detail: String,
        hint: Option<String>,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Progress {
    Started,
    Percent(u8),
    Message(String),
    Finished,
}

pub fn hint_for_stderr(stderr_tail: &str) -> Option<String> {
    if stderr_tail.contains("Invalid data found") {
        Some("input file may be corrupt or in a different format than its extension".to_owned())
    } else if stderr_tail.contains("no decode delegate")
        || stderr_tail.contains("no encode delegate")
    {
        Some("this image format needs a codec that isn't installed".to_owned())
    } else if stderr_tail.contains("Permission denied") {
        Some("open the file through the picker so Wield is granted access to it".to_owned())
    } else if stderr_tail.contains("No such file or directory") {
        Some("an input path no longer exists".to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_stderr_to_hint() {
        assert!(
            hint_for_stderr("x: Invalid data found when processing input")
                .unwrap()
                .contains("corrupt")
        );
        assert!(
            hint_for_stderr("convert: no decode delegate for this image format")
                .unwrap()
                .contains("codec")
        );
        assert_eq!(hint_for_stderr("some unrecognised error"), None);
    }

    #[test]
    fn outcome_serialises_stably() {
        let outcome = ToolOutcome::Failed {
            stage: Stage::Command,
            detail: "exit 1".into(),
            hint: None,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        assert!(json.contains("Failed") && json.contains("Command"));
    }
}

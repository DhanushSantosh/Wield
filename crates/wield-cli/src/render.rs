//! Turn a [`ToolOutcome`] into stdout / stderr text and a process exit code.

use wield_core::ToolOutcome;

pub struct Rendered {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

fn stdout(text: impl Into<String>, code: i32) -> Rendered {
    Rendered {
        stdout: text.into(),
        stderr: String::new(),
        code,
    }
}

fn stderr(text: impl Into<String>, code: i32) -> Rendered {
    Rendered {
        stdout: String::new(),
        stderr: text.into(),
        code,
    }
}

fn with_fix(message: &str, fix: Option<&str>) -> String {
    match fix {
        Some(fix) => format!("{message}\n  → {fix}"),
        None => message.to_owned(),
    }
}

pub fn render(outcome: ToolOutcome) -> Rendered {
    match outcome {
        ToolOutcome::Value { data, .. } => stdout(data, 0),
        ToolOutcome::File { path } => stdout(path.display().to_string(), 0),
        ToolOutcome::Report { title, lines } => stdout(
            std::iter::once(title)
                .chain(lines)
                .collect::<Vec<_>>()
                .join("\n"),
            0,
        ),
        ToolOutcome::Unavailable { reason, fix } => stderr(with_fix(&reason, fix.as_deref()), 1),
        ToolOutcome::Failed { detail, hint, .. } => stderr(with_fix(&detail, hint.as_deref()), 1),
        ToolOutcome::Cancelled => Rendered {
            stdout: String::new(),
            stderr: String::new(),
            code: 130,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wield_core::{Stage, ValueKind};

    #[test]
    fn value_and_file_go_to_stdout_with_code_0() {
        assert_eq!(
            render(ToolOutcome::Value {
                kind: ValueKind::Color,
                data: "#fff".into()
            })
            .code,
            0
        );
        let rendered = render(ToolOutcome::File {
            path: "/tmp/out.webp".into(),
        });
        assert_eq!(rendered.stdout.trim(), "/tmp/out.webp");
        assert_eq!(rendered.code, 0);
    }

    #[test]
    fn failed_and_unavailable_go_to_stderr_with_code_1() {
        let rendered = render(ToolOutcome::Failed {
            stage: Stage::Command,
            detail: "exited with 1".into(),
            hint: Some("input may be corrupt".into()),
        });
        assert!(
            rendered.stderr.contains("exited with 1")
                && rendered.stderr.contains("input may be corrupt")
        );
        assert_eq!(rendered.code, 1);
        assert_eq!(
            render(ToolOutcome::Unavailable {
                reason: "no magick".into(),
                fix: None
            })
            .code,
            1
        );
    }

    #[test]
    fn cancelled_is_silent_code_130() {
        let rendered = render(ToolOutcome::Cancelled);
        assert!(rendered.stdout.is_empty() && rendered.stderr.is_empty());
        assert_eq!(rendered.code, 130);
    }
}

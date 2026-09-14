//! Pure-data tool descriptor model.

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Descriptor {
    pub id: ToolId,
    pub title: String,
    pub keywords: Vec<String>,
    pub category: Category,
    pub args: Vec<ArgSpec>,
    pub requires: Requires,
    pub output: OutputSpec,
    pub capability: Capability,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ToolId(String);

impl ToolId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut segments = value.split('.');
        let valid_segment = |segment: &str| {
            let mut chars = segment.chars();
            matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
                && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        };

        let first = segments.next().unwrap_or_default();
        let rest: Vec<_> = segments.collect();
        if !valid_segment(first) || rest.is_empty() || rest.iter().any(|s| !valid_segment(s)) {
            return Err(format!("invalid namespaced tool id: {value}"));
        }
        Ok(Self(value.to_owned()))
    }
}

impl<'de> Deserialize<'de> for ToolId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for ToolId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    Capture,
    Convert,
    Desktop,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArgSpec {
    pub name: String,
    pub label: String,
    pub help: Option<String>,
    pub arg_type: ArgType,
    pub default: Option<ArgValueLiteral>,
    pub required: bool,
    pub when: Option<When>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArgType {
    File {
        filters: Vec<FileFilter>,
        multiple: bool,
    },
    Dir,
    Str,
    Int {
        range: Option<[i64; 2]>,
        step: Option<i64>,
    },
    Float {
        range: Option<[f64; 2]>,
    },
    Bool,
    Enum {
        options: Vec<String>,
    },
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileFilter {
    pub label: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct When {
    pub arg: String,
    #[serde(rename = "in")]
    pub in_values: Vec<ArgValueLiteral>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ArgValueLiteral {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Requires {
    None,
    Portal { iface: String, min_ver: u32 },
    Binary(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OutputSpec {
    Value(ValueKind),
    File {
        name: String,
        dir: OutputDir,
    },
    /// The command is given a fresh, empty scratch directory (via the
    /// `{output_dir}` template placeholder) instead of one exact output
    /// path, because the real number of files it produces isn't known
    /// until it actually runs (e.g. splitting a PDF into its pages).
    /// `name` is a template (resolved the same way `File.name` is) for
    /// the *prefix* each discovered file's final name is built from -
    /// the actual per-file numbering is `Executor::run_split`'s own
    /// concern, not a template placeholder.
    Directory {
        name: String,
        dir: OutputDir,
    },
    Report,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValueKind {
    Color,
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OutputDir {
    SameAsInput,
    Fixed(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    Command(CommandSpec),
    Portal { adapter: String },
    Native { id: NativeId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NativeId(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub binary: String,
    pub args: Vec<CommandArg>,
    pub progress: ProgressSpec,
    #[serde(with = "duration_secs")]
    pub timeout: Duration,
    pub success: SuccessSpec,
    pub combine_inputs: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandArg {
    pub template: String,
    pub when: Option<When>,
}

impl From<&str> for CommandArg {
    fn from(template: &str) -> Self {
        Self {
            template: template.to_owned(),
            when: None,
        }
    }
}

impl From<String> for CommandArg {
    fn from(template: String) -> Self {
        Self {
            template,
            when: None,
        }
    }
}

impl<'de> Deserialize<'de> for CommandArg {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            Bare(String),
            Full {
                template: String,
                when: Option<When>,
            },
        }

        Ok(match Repr::deserialize(deserializer)? {
            Repr::Bare(template) => Self::from(template),
            Repr::Full { template, when } => Self { template, when },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgressSpec {
    None,
    /// Parses ffmpeg's `-progress pipe:2 -nostats` stderr output against its
    /// own startup `Duration:` banner line - see `command.rs`'s
    /// `FfmpegDuration` for the parser. No `ffprobe` call needed; the total
    /// duration is already on the same stream.
    FfmpegDuration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SuccessSpec {
    ExitZero,
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs(u64::deserialize(deserializer)?))
    }
}

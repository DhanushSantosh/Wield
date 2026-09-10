//! Typed builders for tool descriptors. Built-in tools (P4) are authored with
//! these; they validate on `build()` so a malformed built-in fails fast.

use std::time::Duration;

use crate::descriptor::{
    ArgSpec, ArgType, ArgValueLiteral, Capability, Category, CommandArg, CommandSpec, Descriptor,
    NativeId, OutputSpec, ProgressSpec, Requires, SuccessSpec, ToolId, When,
};
use crate::error::DescriptorError;
use crate::validate::validate_descriptor;

/// Builds a single [`ArgSpec`].
#[derive(Debug, Clone)]
pub struct ArgSpecBuilder {
    spec: ArgSpec,
}

impl ArgSpecBuilder {
    pub fn new(name: &str, label: &str, arg_type: ArgType) -> Self {
        Self {
            spec: ArgSpec {
                name: name.to_owned(),
                label: label.to_owned(),
                help: None,
                arg_type,
                default: None,
                required: false,
                when: None,
            },
        }
    }

    pub fn help(mut self, help: &str) -> Self {
        self.spec.help = Some(help.to_owned());
        self
    }

    pub fn default(mut self, value: ArgValueLiteral) -> Self {
        self.spec.default = Some(value);
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.spec.required = required;
        self
    }

    pub fn when(mut self, arg: &str, in_values: Vec<ArgValueLiteral>) -> Self {
        self.spec.when = Some(When {
            arg: arg.to_owned(),
            in_values,
        });
        self
    }

    /// Visible only once `arg` has any value (the presence form of `When`).
    pub fn when_set(mut self, arg: &str) -> Self {
        self.spec.when = Some(When {
            arg: arg.to_owned(),
            in_values: Vec::new(),
        });
        self
    }

    pub fn build(self) -> ArgSpec {
        self.spec
    }
}

/// Builds a [`CommandSpec`]. Defaults: `progress = None`, `success = ExitZero`,
/// `timeout = 300s`.
#[derive(Debug, Clone)]
pub struct CommandSpecBuilder {
    binary: String,
    args: Vec<CommandArg>,
    timeout: Duration,
}

impl CommandSpecBuilder {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_owned(),
            args: Vec::new(),
            timeout: Duration::from_secs(300),
        }
    }

    pub fn arg(mut self, segment: impl Into<CommandArg>) -> Self {
        self.args.push(segment.into());
        self
    }

    /// A segment emitted only when `arg` holds one of `in_values`.
    pub fn arg_when(mut self, template: &str, arg: &str, in_values: Vec<ArgValueLiteral>) -> Self {
        self.args.push(CommandArg {
            template: template.to_owned(),
            when: Some(When {
                arg: arg.to_owned(),
                in_values,
            }),
        });
        self
    }

    /// A segment emitted only once `arg` has any value (the presence form).
    /// Use for paired option flags — give both segments the same `arg`.
    pub fn arg_when_set(mut self, template: &str, arg: &str) -> Self {
        self.args.push(CommandArg {
            template: template.to_owned(),
            when: Some(When {
                arg: arg.to_owned(),
                in_values: Vec::new(),
            }),
        });
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> CommandSpec {
        CommandSpec {
            binary: self.binary,
            args: self.args,
            progress: ProgressSpec::None,
            timeout: self.timeout,
            success: SuccessSpec::ExitZero,
        }
    }
}

/// Builds a [`Descriptor`]. `build()` runs [`validate_descriptor`].
#[derive(Debug, Clone)]
pub struct DescriptorBuilder {
    id: ToolId,
    title: String,
    category: Category,
    keywords: Vec<String>,
    args: Vec<ArgSpec>,
    requires: Requires,
    output: Option<OutputSpec>,
    capability: Option<Capability>,
}

impl DescriptorBuilder {
    /// # Panics
    /// Panics if `id` is not a valid namespaced tool id. Built-ins are authored
    /// in-repo, so an invalid id is a programming error, not runtime input.
    pub fn new(id: &str, title: &str, category: Category) -> Self {
        Self {
            id: ToolId::parse(id).expect("valid namespaced tool id"),
            title: title.to_owned(),
            category,
            keywords: Vec::new(),
            args: Vec::new(),
            requires: Requires::None,
            output: None,
            capability: None,
        }
    }

    pub fn keyword(mut self, keyword: &str) -> Self {
        self.keywords.push(keyword.to_owned());
        self
    }

    pub fn keywords(mut self, keywords: &[&str]) -> Self {
        self.keywords
            .extend(keywords.iter().map(|k| (*k).to_owned()));
        self
    }

    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }

    pub fn requires(mut self, requires: Requires) -> Self {
        self.requires = requires;
        self
    }

    pub fn output(mut self, output: OutputSpec) -> Self {
        self.output = Some(output);
        self
    }

    pub fn command(mut self, command: CommandSpecBuilder) -> Self {
        self.capability = Some(Capability::Command(command.build()));
        self
    }

    pub fn portal(mut self, adapter: &str) -> Self {
        self.capability = Some(Capability::Portal {
            adapter: adapter.to_owned(),
        });
        self
    }

    pub fn native(mut self, id: &str) -> Self {
        self.capability = Some(Capability::Native {
            id: NativeId(id.to_owned()),
        });
        self
    }

    pub fn build(self) -> Result<Descriptor, Vec<DescriptorError>> {
        let mut missing = Vec::new();
        if self.output.is_none() {
            missing.push(DescriptorError {
                at: "output".to_owned(),
                message: "output was not set".to_owned(),
            });
        }
        if self.capability.is_none() {
            missing.push(DescriptorError {
                at: "capability".to_owned(),
                message: "capability was not set (call .command()/.portal()/.native())".to_owned(),
            });
        }
        if !missing.is_empty() {
            return Err(missing);
        }

        let descriptor = Descriptor {
            id: self.id,
            title: self.title,
            keywords: self.keywords,
            category: self.category,
            args: self.args,
            requires: self.requires,
            output: self.output.expect("checked above"),
            capability: self.capability.expect("checked above"),
        };
        validate_descriptor(&descriptor)?;
        Ok(descriptor)
    }
}

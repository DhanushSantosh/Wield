//! `wield-core` — the Wield tool descriptor model, argument validation,
//! argv-template rendering, the `Command`-capability executor, and the tool
//! registry.
//!
//! Native execution remains stubbed here; it lands in P4.

pub mod args;
pub mod builder;
pub mod command;
pub mod descriptor;
pub mod error;
pub mod executor;
pub mod outcome;
pub mod portal;
pub mod registry;
pub mod template;
pub mod validate;

pub use args::{validate_args, ArgMap, ArgValue};
pub use builder::{ArgSpecBuilder, CommandSpecBuilder, DescriptorBuilder};
pub use command::BinaryResolver;
pub use descriptor::{
    ArgSpec, ArgType, ArgValueLiteral, Capability, Category, CommandArg, CommandSpec, Descriptor,
    FileFilter, NativeId, OutputDir, OutputSpec, ProgressSpec, Requires, SuccessSpec, ToolId,
    ValueKind, When,
};
pub use error::{CoreError, DescriptorError, ValidationError};
pub use executor::{AvailabilityView, ExecutionRequest, Executor};
pub use outcome::{Progress, Stage, ToolOutcome};
pub use portal::PortalRunner;
pub use registry::{Registry, RegistryError};
pub use template::{compute_output_path, render_argv, render_output_name, TemplateError};
pub use validate::validate_descriptor;

/// The `wield-core` crate version, from Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_non_empty() {
        assert!(!super::version().is_empty());
    }
}

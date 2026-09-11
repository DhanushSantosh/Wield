//! `wield` — the one-shot command-line surface, generated from the built-in
//! tool registry.

pub mod parse;
pub mod render;
pub mod surface;

use std::sync::Arc;
use wield_core::{
    validate_args, AvailabilityView, BinaryResolver, ExecutionRequest, Executor, Registry, Stage,
    ToolOutcome,
};
use wield_portal::PortalAdapterRunner;
use wield_tools::builtin_registry;

/// Entry point. `argv` includes `argv[0]`. Returns the process exit code:
/// `0` success, `1` the tool ran but failed / was unavailable, `2` a usage
/// error, `130` cancelled.
pub async fn run(argv: Vec<String>) -> i32 {
    let args: Vec<String> = argv.into_iter().skip(1).collect();
    let registry = builtin_registry();

    match args.first().map(String::as_str) {
        None => {
            print!("{}", tool_list(&registry));
            return 0;
        }
        Some("--version" | "-V") => {
            println!("wield {}", wield_core::version());
            return 0;
        }
        Some("--help" | "-h") => {
            print!("{}", usage(&registry));
            return 0;
        }
        _ => {}
    }

    let id = &args[0];
    let Some(descriptor) = registry.get(id) else {
        eprintln!("unknown tool: {id}\n");
        eprint!("{}", tool_list(&registry));
        return 2;
    };

    let rest = &args[1..];
    if rest.iter().any(|token| token == "--help" || token == "-h") {
        println!("{}", crate::surface::help_text(descriptor));
        return 0;
    }

    let cli_surface = crate::surface::CliSurface::from_descriptor(descriptor);
    let arg_map = match crate::parse::parse_args(&cli_surface, rest) {
        Ok(map) => map,
        Err(error) => {
            eprintln!("{}", error.0);
            return 2;
        }
    };

    // Reject invalid input before probing desktop portals. Besides being faster,
    // this keeps usage errors responsive when no portal service is available.
    if let Err(errors) = validate_args(&descriptor.args, &arg_map) {
        return write_outcome(ToolOutcome::Failed {
            stage: Stage::Validation,
            detail: errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
            hint: None,
        });
    }

    let resolver = BinaryResolver::from_env();
    let mut availability = AvailabilityView::probe_binaries(&resolver, registry.list());
    wield_portal::probe().await.apply_to(&mut availability);

    let executor = Executor::new(resolver)
        .with_portal(Arc::new(PortalAdapterRunner))
        .with_availability(availability);

    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(32);
    let drain = tokio::spawn(async move { while progress_rx.recv().await.is_some() {} });

    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: descriptor.clone(),
                args: arg_map,
            },
            progress_tx,
            tokio_util::sync::CancellationToken::new(),
        )
        .await;
    drain.abort();

    write_outcome(outcome)
}

fn write_outcome(outcome: ToolOutcome) -> i32 {
    let rendered = crate::render::render(outcome);
    if !rendered.stdout.is_empty() {
        println!("{}", rendered.stdout.trim_end());
    }
    if !rendered.stderr.is_empty() {
        eprintln!("{}", rendered.stderr.trim_end());
    }
    rendered.code
}

fn tool_list(registry: &Registry) -> String {
    let mut out = String::from("Available tools:\n");
    for tool in registry.list() {
        out.push_str(&format!("  {:<16}  {}\n", tool.id.as_ref(), tool.title));
    }
    out.push_str("\nRun `wield <tool> --help` for a tool's arguments.\n");
    out
}

fn usage(registry: &Registry) -> String {
    format!(
        "wield {} — portal-native Linux utility hub\n\nUsage:\n  wield <tool> [<file>] [options]\n  wield --version\n\n{}",
        wield_core::version(),
        tool_list(registry),
    )
}

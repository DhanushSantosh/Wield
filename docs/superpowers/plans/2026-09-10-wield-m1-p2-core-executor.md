# Wield M1 · Plan P2 — wield-core Descriptor Model & Command Executor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `wield-core` crate: the tool descriptor model, argument validation, argv-template rendering, and the async `Command`-capability executor (spawn, progress, timeout, cancellation, atomic output), plus the tool registry — all with a unit + stub-script integration test surface. No Portal execution, no Native execution, no UI.

**Architecture:** `wield-core` is a library with two halves. The **descriptor half** (pure data + validation) defines `Descriptor`, `ArgSpec`, `Capability`, `OutputSpec`, is `serde`-round-trippable (for the registry snapshot test and future user-supplied TOML), and is constructed through typed builders. The **executor half** takes a validated descriptor + an argument map and, for `Capability::Command`, resolves the binary, renders argv from the templated spec, spawns the child on Tokio with a cancellation token and timeout, streams `Progress` over an `mpsc` channel, writes output to a temp file that is atomically renamed into place only on success, and returns a single `ToolOutcome` enum. `Portal` and `Native` capability arms return a placeholder `Failed` outcome until P3/P4.

**Tech Stack:** Rust 2021, Tokio 1 (`process`, `time`, `sync`, `rt`), `tokio-util` `CancellationToken`, `libc` (process-group signalling), `fuzzy-matcher` (palette search), `serde` + `serde_json`, `thiserror`, `uuid`; `tempfile` for tests.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` — this plan implements §4 (descriptor & executor model), §7 (error handling, atomic output, cancellation), and the `wield-core` + executor-integration portions of §8 (testing).

---

## GEON amendment — 2026-09-10 (owner design decisions)

Locked with the owner before this plan was written:

1. **Async on Tokio.** The executor is `async`; subprocesses use `tokio::process`, cancellation uses `tokio_util::sync::CancellationToken`, progress uses `tokio::sync::mpsc`. No sync/thread variant.
2. **Segment-drop templates, author gates pairs.** `CommandSpec.args` is an ordered list of `CommandArg { template, when }`. A segment is emitted iff its optional `when` passes **and** every `{placeholder}` in its template resolves. Paired flags (e.g. `-resize` + `{width}x`) are gated by giving both segments the same `when`. No group/`{?:}` syntax.
3. **serde-ready descriptors, builders for construction.** All descriptor types derive `Serialize` + `Deserialize`. Built-ins (P4) are constructed via typed builders. Parsing user-supplied TOML descriptors is a later plan; P2 only makes the types ready.
4. **Native handler is a string id.** `Capability::Native { id: NativeId(String) }` — a key resolved against a handler registry built in P4. `Capability` stays pure data.

Additional constraints from GEON:

- **No git commits by the worker beyond the per-task commits below**, which land on a feature branch `feat/p2-core-executor` off `master`. GEON opens the PR and merges after review. (P1 is already committed + pushed; the repo is no longer at zero commits, so normal per-task commits on a branch are correct here — unlike P1.)
- **`cargo` is not on the default `PATH`.** Every shell that runs `cargo` must first: `export PATH="$HOME/.cargo/bin:$PATH"` (toolchain is rustup stable 1.98.1 at `~/.cargo`).
- **`npm run check`** must stay green at the end of every task (it runs `cargo test` across the workspace).

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p2-core-executor` off `master`). All paths below are relative to this root.
- **Crate:** all work is in `crates/wield-core/` except workspace dependency additions in the root `Cargo.toml`.
- **Rust edition:** 2021, from `[workspace.package]`.
- **Public API:** everything a later plan consumes is re-exported from `crates/wield-core/src/lib.rs` (see Task 14). Internal modules are `pub(crate)` unless a type crosses the crate boundary.
- **No `unwrap()` / `expect()` / `panic!` in library code** except in `#[cfg(test)]`. Fallible paths return `Result` or a `ToolOutcome` variant. The one deliberate exception: `Executor::run` wraps nothing in `catch_unwind` (that belongs at the Tauri command boundary, P5) — but it must not itself panic on any input reachable from validated args.
- **Determinism:** `Registry::snapshot()` output is stable across runs (sorted keys, no timestamps, no addresses).
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

All new files under `crates/wield-core/`.

**Created — `src/`:**

| File | Responsibility |
| --- | --- |
| `src/error.rs` | `CoreError` (thiserror) + the structured error lists `DescriptorError`, `ValidationError` |
| `src/descriptor.rs` | Pure-data descriptor types: `Descriptor`, `ToolId`, `Category`, `ArgSpec`, `ArgType`, `FileFilter`, `When`, `Requires`, `OutputSpec`, `OutputDir`, `ValueKind`, `Capability`, `CommandSpec`, `CommandArg`, `ProgressSpec`, `SuccessSpec`, `NativeId`. All `serde`. |
| `src/builder.rs` | `DescriptorBuilder`, `ArgSpecBuilder`, `CommandSpecBuilder` — the only sanctioned way to construct built-ins |
| `src/validate.rs` | `validate_descriptor(&Descriptor) -> Result<(), Vec<DescriptorError>>` |
| `src/args.rs` | `ArgValue`, `ArgMap`, `visible_args`, `validate_args` (types/ranges/required/enum/defaults, hidden-arg stripping) |
| `src/template.rs` | `render_argv`, `render_output_name`, derived-placeholder resolution, `TemplateError` |
| `src/command.rs` | `BinaryResolver`, `CommandRunner`, `OutputPlan`, `CommandResult` — spawn, progress, timeout, cancel, atomic rename |
| `src/outcome.rs` | `ToolOutcome`, `Stage`, `Progress`, `hint_for_stderr` |
| `src/executor.rs` | `Executor`, `ExecutionRequest`, `AvailabilityView` — the pipeline wiring |
| `src/registry.rs` | `Registry`, `RegistryError` — register/list/get/search/available/snapshot |

**Modified:**

- `crates/wield-core/src/lib.rs` — replace the P1 stub: module declarations + public re-exports. Keep `pub fn version()`.
- `crates/wield-core/Cargo.toml` — add dependencies (Task 1).
- `Cargo.toml` (root) — add `[workspace.dependencies]` entries (Task 1).

**Created — `tests/`:**

| File | Covers |
| --- | --- |
| `tests/descriptor_serde.rs` | round-trip serialize/deserialize of a full descriptor |
| `tests/descriptor_validation.rs` | every `DescriptorError` rejection case |
| `tests/arg_validation.rs` | `visible_args` + `validate_args` |
| `tests/argv_rendering.rs` | `render_argv` / `render_output_name` — the highest-value surface |
| `tests/command_runner.rs` | stub-script integration: success + atomic rename, non-zero exit, timeout kill, cancellation, temp cleanup, progress channel |
| `tests/executor_pipeline.rs` | full `Executor::run` through a stub converter script; Portal/Native placeholder outcomes |
| `tests/registry.rs` | register/dup-id/get/search/available/snapshot |
| `tests/support/mod.rs` | shared test helpers: write a stub script, build a sample descriptor |

**Out of P2 scope (named so the worker does not add them):** the `Ffmpeg` progress parser (M2), `ProgressSpec` variants beyond `None`, real `Portal`/`Native` execution (P3/P4), user-TOML descriptor loading, the real built-in registry + its snapshot (P4 — P2 snapshots a test fixture set), `SuccessSpec` variants beyond `ExitZero`, bundled-binary path resolution (P7 — P2 does `$PATH` only), `ValueKind` producers.

---

## Task 1: Crate dependencies + module skeleton

**Files:**
- Modify: `Cargo.toml` (root), `crates/wield-core/Cargo.toml`, `crates/wield-core/src/lib.rs`
- Create: `crates/wield-core/src/error.rs`

**Interfaces:**
- Produces: empty-but-declared modules `error`, `descriptor`, `builder`, `validate`, `args`, `template`, `command`, `outcome`, `executor`, `registry`; `CoreError` enum.

- [ ] **Step 1: Add workspace dependencies**

In root `Cargo.toml`, extend `[workspace.dependencies]`:

```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
uuid = { version = "1", features = ["v4", "serde"] }
tokio = { version = "1", features = ["rt", "rt-multi-thread", "process", "time", "sync", "io-util", "macros"] }
tokio-util = { version = "0.7", features = ["rt"] }
fuzzy-matcher = "0.3"
libc = "0.2"
tempfile = "3"
```

- [ ] **Step 2: Rewrite `crates/wield-core/Cargo.toml`**

```toml
[package]
name = "wield-core"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
tokio.workspace = true
tokio-util.workspace = true
fuzzy-matcher.workspace = true
libc.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio = { workspace = true, features = ["test-util"] }
```

- [ ] **Step 3: Create `src/error.rs`**

```rust
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
```

- [ ] **Step 4: Rewrite `src/lib.rs`**

```rust
//! `wield-core` — the Wield tool descriptor model, argument validation,
//! argv-template rendering, the `Command`-capability executor, and the tool
//! registry.
//!
//! Portal and Native execution are stubbed here; they land in P3 and P4.

pub mod args;
pub mod builder;
pub mod command;
pub mod descriptor;
pub mod error;
pub mod executor;
pub mod outcome;
pub mod registry;
pub mod template;
pub mod validate;

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
```

- [ ] **Step 5: Create stub module files**

Create each of `src/{descriptor,builder,validate,args,template,command,outcome,executor,registry}.rs` containing only a module doc comment line, e.g.:

```rust
//! Descriptor data types. See plan P2 Task 2.
```

- [ ] **Step 6: Verify it builds**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo build -p wield-core`
Expected: PASS (warnings about unused modules are fine).

- [ ] **Step 7: Commit**

```bash
git checkout -b feat/p2-core-executor
git add Cargo.toml crates/wield-core
git commit -m "chore(core): add P2 dependencies and module skeleton"
```

---

## Task 2: Descriptor data types

**Files:**
- Replace: `crates/wield-core/src/descriptor.rs`
- Test: `crates/wield-core/tests/descriptor_serde.rs`

**Interfaces:**
- Produces:
  - `struct Descriptor { id: ToolId, title: String, keywords: Vec<String>, category: Category, args: Vec<ArgSpec>, requires: Requires, output: OutputSpec, capability: Capability }`
  - `struct ToolId(String)` with `ToolId::parse(&str) -> Result<ToolId, String>` and `Display`
  - `enum Category { Capture, Convert, Desktop }`
  - `struct ArgSpec { name: String, label: String, help: Option<String>, arg_type: ArgType, default: Option<ArgValueLiteral>, required: bool, when: Option<When> }`
  - `enum ArgType { File { filters: Vec<FileFilter>, multiple: bool }, Dir, Str, Int { range: Option<[i64;2]>, step: Option<i64> }, Float { range: Option<[f64;2]> }, Bool, Enum { options: Vec<String> }, Text }`
  - `struct FileFilter { label: String, extensions: Vec<String> }`
  - `struct When { arg: String, in_values: Vec<ArgValueLiteral> }` (serde field name `in`)
  - `enum ArgValueLiteral { Str(String), Int(i64), Float(f64), Bool(bool) }` — the JSON/TOML-representable subset used for `default` and `When::in_values`
  - `enum Requires { None, Portal { iface: String, min_ver: u32 }, Binary(String) }`
  - `enum OutputSpec { Value(ValueKind), File { name: String, dir: OutputDir }, Report }`
  - `enum ValueKind { Color, Text }`
  - `enum OutputDir { SameAsInput, Fixed(std::path::PathBuf) }`
  - `enum Capability { Command(CommandSpec), Portal { adapter: String }, Native { id: NativeId } }`
  - `struct NativeId(String)`
  - `struct CommandSpec { binary: String, args: Vec<CommandArg>, progress: ProgressSpec, timeout: std::time::Duration, success: SuccessSpec }`
  - `struct CommandArg { template: String, when: Option<When> }` with `impl From<&str> for CommandArg`
  - `enum ProgressSpec { None }`
  - `enum SuccessSpec { ExitZero }`
- All types: `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]` (add `Eq`/`Hash` only where all fields support it — not on the `Float` arms).

- [ ] **Step 1: Write the failing test**

`crates/wield-core/tests/descriptor_serde.rs`:

```rust
use std::time::Duration;
use wield_core::descriptor::*;

fn sample() -> Descriptor {
    Descriptor {
        id: ToolId::parse("image.convert").unwrap(),
        title: "Convert image".into(),
        keywords: vec!["image".into(), "resize".into()],
        category: Category::Convert,
        args: vec![
            ArgSpec {
                name: "input".into(),
                label: "Image".into(),
                help: None,
                arg_type: ArgType::File {
                    filters: vec![FileFilter { label: "Images".into(), extensions: vec!["png".into(), "jpg".into()] }],
                    multiple: false,
                },
                default: None,
                required: true,
                when: None,
            },
            ArgSpec {
                name: "format".into(),
                label: "Output format".into(),
                help: Some("Target container".into()),
                arg_type: ArgType::Enum { options: vec!["png".into(), "webp".into()] },
                default: Some(ArgValueLiteral::Str("webp".into())),
                required: true,
                when: None,
            },
            ArgSpec {
                name: "width".into(),
                label: "Width".into(),
                help: None,
                arg_type: ArgType::Int { range: Some([1, 10_000]), step: Some(1) },
                default: None,
                required: false,
                when: Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] }),
            },
        ],
        requires: Requires::Binary("magick".into()),
        output: OutputSpec::File { name: "{input_stem}.{format}".into(), dir: OutputDir::SameAsInput },
        capability: Capability::Command(CommandSpec {
            binary: "magick".into(),
            args: vec![
                "{input}".into(),
                CommandArg { template: "-resize".into(), when: Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] }) },
                CommandArg { template: "{width}x".into(), when: Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] }) },
                "{output}".into(),
            ],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(300),
            success: SuccessSpec::ExitZero,
        }),
    }
}

#[test]
fn descriptor_round_trips_through_json() {
    let original = sample();
    let json = serde_json::to_string_pretty(&original).unwrap();
    let parsed: Descriptor = serde_json::from_str(&json).unwrap();
    assert_eq!(original, parsed);
}

#[test]
fn tool_id_rejects_non_namespaced() {
    assert!(ToolId::parse("convert").is_err());
    assert!(ToolId::parse("Image.Convert").is_err());
    assert!(ToolId::parse("image.convert").is_ok());
    assert!(ToolId::parse("pdf.tools.split").is_ok());
}

#[test]
fn when_field_serializes_as_in() {
    let w = When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] };
    let json = serde_json::to_string(&w).unwrap();
    assert!(json.contains("\"in\""), "got {json}");
    assert!(!json.contains("in_values"), "got {json}");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test descriptor_serde`
Expected: FAIL — types not defined.

- [ ] **Step 3: Implement `src/descriptor.rs`**

Write every type from the Interfaces block. Notes:

- `ToolId::parse`: accept `^[a-z][a-z0-9]*(\.[a-z][a-z0-9]*)+$` (at least one dot). Store the string. `#[derive(Serialize)]` as a transparent string via `#[serde(transparent)]`; implement `Deserialize` manually (or via `#[serde(try_from = "String")]`) so parsing runs on load. `impl std::fmt::Display`, `impl AsRef<str>`.
- `NativeId`: `#[serde(transparent)]` newtype, no validation.
- `When`: `#[serde(rename_all = "snake_case")]` is not enough for `in` — use `#[serde(rename = "in")]` on the `in_values` field.
- `CommandSpec::timeout`: `Duration` is not `serde` by default. Add a local module:

```rust
mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_secs())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs(u64::deserialize(d)?))
    }
}
```

and annotate `#[serde(with = "duration_secs")] pub timeout: Duration`.
- `CommandArg`: `impl From<&str>` and `impl From<String>` producing `{ template, when: None }`. Custom `Deserialize` that accepts **either** a bare string **or** the full `{ template, when }` map (use `#[serde(untagged)]` on a private helper enum, or `deserialize_with`). Serialize always as the full map (simplest; the round-trip test only checks equality, not shape).
- Enum tagging: default (externally tagged) is fine for `ArgType`, `Requires`, `OutputSpec`, `Capability`, `ArgValueLiteral`. Verify the round-trip test passes with whatever tagging you pick.
- Derive `Eq` + `Hash` on `ToolId`, `NativeId`, `Category` only.

- [ ] **Step 4: Run tests to verify they pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test descriptor_serde`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/descriptor.rs crates/wield-core/tests/descriptor_serde.rs
git commit -m "feat(core): descriptor data model with serde round-trip"
```

---

## Task 3: Descriptor validation

**Files:**
- Replace: `crates/wield-core/src/validate.rs`
- Test: `crates/wield-core/tests/descriptor_validation.rs`

**Interfaces:**
- Consumes: all of `descriptor` (Task 2), `DescriptorError` (Task 1).
- Produces: `pub fn validate_descriptor(d: &Descriptor) -> Result<(), Vec<DescriptorError>>`.
- Rules enforced (each yields a `DescriptorError` with a precise `at`):
  1. `args[i].name` non-empty, unique across the list.
  2. `args[i].when.arg` names another arg in the list (not itself); the referenced arg is declared **before** `args[i]` (no forward refs).
  3. Each value in `args[i].when.in_values` is type-compatible with the referenced arg's `ArgType` (e.g. `Bool` arg → only `Bool` literals; `Enum{options}` → `Str` literals that are in `options`; `Int` → `Int`).
  4. `args[i].default`, when present, is type-compatible with `args[i].arg_type` and (for `Enum`) is in `options`; for `Int`/`Float` with a `range`, within range.
  5. `ArgType::Int.range` / `Float.range`: `range[0] <= range[1]`. `Int.step`, if set, `> 0`.
  6. `ArgType::Enum.options` non-empty, no duplicates.
  7. If `capability` is `Capability::Command`, every `{placeholder}` appearing in any `CommandArg.template` **and** in `OutputSpec::File.name` is one of: a declared arg `name`, a derived placeholder (`input`, `input_stem`, `input_dir`, `format`), or `output` (only inside `CommandArg.template`, never in `OutputSpec::File.name`).
  8. Derived placeholders `input*` require an arg literally named `input` of `ArgType::File { multiple: false, .. }`; `{format}` requires an arg named `format`.
  9. `CommandArg[i].when` follows the same rules as arg `when` (references a declared arg, type-compatible literals) — but may reference **any** declared arg regardless of order.
  10. `OutputSpec::File` ⟺ `Capability::Command` for P2 (a `Command` tool must have `OutputSpec::File`; `OutputSpec::Value` / `Report` with `Command` is rejected in P2 — noted as a P2 limitation, not a permanent rule).

- [ ] **Step 1: Write the failing test**

`crates/wield-core/tests/descriptor_validation.rs`:

```rust
use std::time::Duration;
use wield_core::descriptor::*;
use wield_core::validate::validate_descriptor;

fn base_command_descriptor() -> Descriptor {
    Descriptor {
        id: ToolId::parse("x.y").unwrap(),
        title: "t".into(),
        keywords: vec![],
        category: Category::Convert,
        args: vec![ArgSpec {
            name: "input".into(),
            label: "in".into(),
            help: None,
            arg_type: ArgType::File { filters: vec![], multiple: false },
            default: None,
            required: true,
            when: None,
        }],
        requires: Requires::Binary("tool".into()),
        output: OutputSpec::File { name: "{input_stem}.out".into(), dir: OutputDir::SameAsInput },
        capability: Capability::Command(CommandSpec {
            binary: "tool".into(),
            args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None,
            timeout: Duration::from_secs(60),
            success: SuccessSpec::ExitZero,
        }),
    }
}

#[test]
fn accepts_a_well_formed_descriptor() {
    assert!(validate_descriptor(&base_command_descriptor()).is_ok());
}

#[test]
fn rejects_duplicate_arg_names() {
    let mut d = base_command_descriptor();
    d.args.push(d.args[0].clone());
    let errs = validate_descriptor(&d).unwrap_err();
    assert!(errs.iter().any(|e| e.at.starts_with("args[1]") && e.message.contains("duplicate")));
}

#[test]
fn rejects_when_referencing_unknown_arg() {
    let mut d = base_command_descriptor();
    d.args.push(ArgSpec {
        name: "width".into(), label: "w".into(), help: None,
        arg_type: ArgType::Int { range: None, step: None },
        default: None, required: false,
        when: Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] }),
    });
    let errs = validate_descriptor(&d).unwrap_err();
    assert!(errs.iter().any(|e| e.at.contains("when.arg") && e.message.contains("resize")));
}

#[test]
fn rejects_unknown_template_placeholder() {
    let mut d = base_command_descriptor();
    if let Capability::Command(ref mut c) = d.capability {
        c.args.push("{bogus}".into());
    }
    let errs = validate_descriptor(&d).unwrap_err();
    assert!(errs.iter().any(|e| e.message.contains("bogus")));
}

#[test]
fn rejects_enum_default_not_in_options() {
    let mut d = base_command_descriptor();
    d.args.push(ArgSpec {
        name: "format".into(), label: "f".into(), help: None,
        arg_type: ArgType::Enum { options: vec!["png".into()] },
        default: Some(ArgValueLiteral::Str("webp".into())),
        required: true, when: None,
    });
    let errs = validate_descriptor(&d).unwrap_err();
    assert!(errs.iter().any(|e| e.at.contains("default") && e.message.contains("webp")));
}

#[test]
fn rejects_output_placeholder_in_output_name() {
    let mut d = base_command_descriptor();
    d.output = OutputSpec::File { name: "{output}.x".into(), dir: OutputDir::SameAsInput };
    let errs = validate_descriptor(&d).unwrap_err();
    assert!(errs.iter().any(|e| e.at.starts_with("output") && e.message.contains("output")));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test descriptor_validation`
Expected: FAIL — `validate_descriptor` not defined.

- [ ] **Step 3: Implement `src/validate.rs`**

Implement `validate_descriptor` collecting **all** errors (do not early-return). Suggested internal structure:

```rust
pub fn validate_descriptor(d: &Descriptor) -> Result<(), Vec<DescriptorError>> {
    let mut errs = Vec::new();
    check_arg_names(&d.args, &mut errs);
    check_arg_when(&d.args, &mut errs);
    check_arg_defaults(&d.args, &mut errs);
    check_arg_type_bounds(&d.args, &mut errs);
    if let Capability::Command(cmd) = &d.capability {
        check_command_templates(d, cmd, &mut errs);
        check_output_is_file(d, &mut errs);
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}
```

Add a helper `fn placeholders(template: &str) -> Vec<String>` that scans `{name}` tokens (a `name` is `[a-z_][a-z0-9_]*`; a literal `{{` or unmatched `{` is ignored — but note `{{` escaping is **not** a feature, so treat any `{` not opening a valid token as a `DescriptorError` "malformed placeholder"). Reuse this exact function in `template.rs` (Task 6) — put it in `template.rs` as `pub(crate) fn placeholders` and `use` it here.

Type-compat helper: `fn literal_matches_type(lit: &ArgValueLiteral, ty: &ArgType) -> bool`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test descriptor_validation`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/validate.rs crates/wield-core/src/template.rs crates/wield-core/tests/descriptor_validation.rs
git commit -m "feat(core): descriptor validation"
```

---

## Task 4: Argument values and `when` visibility

**Files:**
- Replace: `crates/wield-core/src/args.rs` (visibility half)
- Test: `crates/wield-core/tests/arg_validation.rs` (visibility half)

**Interfaces:**
- Consumes: `descriptor` (Task 2).
- Produces:
  - `enum ArgValue { Str(String), Int(i64), Float(f64), Bool(bool), Path(PathBuf), Paths(Vec<PathBuf>) }` with `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`, plus `impl ArgValue { fn as_display_string(&self) -> Option<String> }` (returns `None` for `Paths`).
  - `type ArgMap = std::collections::BTreeMap<String, ArgValue>`
  - `fn literal_to_value(lit: &ArgValueLiteral) -> ArgValue`
  - `fn value_satisfies_literal(v: &ArgValue, lit: &ArgValueLiteral) -> bool`
  - `fn visible_args<'a>(specs: &'a [ArgSpec], values: &ArgMap) -> Vec<&'a ArgSpec>` — an arg is visible when `when` is `None`, or `values[when.arg]` exists and satisfies any literal in `when.in_values`. If `when.arg` itself is not visible or absent, the dependent arg is **not** visible.

- [ ] **Step 1: Write the failing test**

Add to `crates/wield-core/tests/arg_validation.rs`:

```rust
use std::collections::BTreeMap;
use wield_core::args::{visible_args, ArgValue};
use wield_core::descriptor::*;

fn specs() -> Vec<ArgSpec> {
    vec![
        ArgSpec { name: "resize".into(), label: "Resize".into(), help: None,
            arg_type: ArgType::Bool, default: Some(ArgValueLiteral::Bool(false)),
            required: false, when: None },
        ArgSpec { name: "width".into(), label: "Width".into(), help: None,
            arg_type: ArgType::Int { range: Some([1, 9999]), step: None },
            default: None, required: false,
            when: Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] }) },
    ]
}

#[test]
fn width_hidden_when_resize_false() {
    let mut v: BTreeMap<String, ArgValue> = BTreeMap::new();
    v.insert("resize".into(), ArgValue::Bool(false));
    let vis = visible_args(&specs(), &v);
    assert_eq!(vis.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["resize"]);
}

#[test]
fn width_visible_when_resize_true() {
    let mut v: BTreeMap<String, ArgValue> = BTreeMap::new();
    v.insert("resize".into(), ArgValue::Bool(true));
    let vis = visible_args(&specs(), &v);
    assert_eq!(vis.len(), 2);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test arg_validation`
Expected: FAIL — `visible_args` not defined.

- [ ] **Step 3: Implement the visibility half of `src/args.rs`**

Define `ArgValue`, `ArgMap`, `literal_to_value`, `value_satisfies_literal`, `visible_args`. `value_satisfies_literal` compares by variant (`Str`==`Str`, `Int`==`Int`, `Bool`==`Bool`, `Float` by exact `==`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test arg_validation`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/args.rs crates/wield-core/tests/arg_validation.rs
git commit -m "feat(core): arg values and when-visibility"
```

---

## Task 5: Argument validation

**Files:**
- Modify: `crates/wield-core/src/args.rs` (add `validate_args`)
- Modify: `crates/wield-core/tests/arg_validation.rs`

**Interfaces:**
- Consumes: `visible_args`, `ArgValue`, `ArgMap` (Task 4); `ValidationError` (Task 1).
- Produces: `pub fn validate_args(specs: &[ArgSpec], input: &ArgMap) -> Result<ArgMap, Vec<ValidationError>>`.
- Behaviour:
  1. Compute `visible_args(specs, input)`.
  2. Build the **effective map**: start empty; for each visible spec, take `input[name]` if present, else the spec's `default` (as `ArgValue` via `literal_to_value`), else nothing.
  3. For each visible spec: if `required` and the effective map has no value → `ValidationError { field: name, "required" }`.
  4. For each present effective value: check it matches `arg_type`:
     - `File { multiple: false }` → `ArgValue::Path`; `multiple: true` → `Path` or `Paths`.
     - `Dir` → `Path`.
     - `Str` / `Text` → `Str`.
     - `Int { range, step }` → `Int`; if `range` set, `lo <= n <= hi`; if `step` set, `(n - lo) % step == 0` (when `range` set) else `n % step == 0`.
     - `Float { range }` → `Float`; range check inclusive.
     - `Bool` → `Bool`.
     - `Enum { options }` → `Str` whose value ∈ `options`.
     - Mismatch or out-of-bounds → a `ValidationError` on that field.
  5. Values in `input` whose spec is **not visible** (or unknown) are dropped silently — they never reach the effective map.
  6. Return the effective map on success (this is what `render_argv` consumes).

- [ ] **Step 1: Write the failing tests**

Append to `crates/wield-core/tests/arg_validation.rs`:

```rust
use wield_core::args::validate_args;

#[test]
fn fills_defaults_and_drops_hidden() {
    let mut input = BTreeMap::new();
    input.insert("resize".into(), ArgValue::Bool(false));
    input.insert("width".into(), ArgValue::Int(800)); // hidden -> dropped
    let effective = validate_args(&specs(), &input).unwrap();
    assert_eq!(effective.get("resize"), Some(&ArgValue::Bool(false)));
    assert!(effective.get("width").is_none());
}

#[test]
fn rejects_out_of_range_int() {
    let mut input = BTreeMap::new();
    input.insert("resize".into(), ArgValue::Bool(true));
    input.insert("width".into(), ArgValue::Int(99999));
    let errs = validate_args(&specs(), &input).unwrap_err();
    assert_eq!(errs[0].field, "width");
}

#[test]
fn rejects_missing_required() {
    let specs = vec![ArgSpec {
        name: "input".into(), label: "in".into(), help: None,
        arg_type: ArgType::File { filters: vec![], multiple: false },
        default: None, required: true, when: None,
    }];
    let errs = validate_args(&specs, &BTreeMap::new()).unwrap_err();
    assert_eq!(errs[0].field, "input");
}

#[test]
fn rejects_enum_value_not_in_options() {
    let specs = vec![ArgSpec {
        name: "format".into(), label: "f".into(), help: None,
        arg_type: ArgType::Enum { options: vec!["png".into(), "webp".into()] },
        default: None, required: true, when: None,
    }];
    let mut input = BTreeMap::new();
    input.insert("format".into(), ArgValue::Str("gif".into()));
    let errs = validate_args(&specs, &input).unwrap_err();
    assert_eq!(errs[0].field, "format");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test arg_validation`
Expected: FAIL — `validate_args` not defined.

- [ ] **Step 3: Implement `validate_args`**

Collect all errors; do not early-return.

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test arg_validation`
Expected: PASS (6 tests total in the file).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/args.rs crates/wield-core/tests/arg_validation.rs
git commit -m "feat(core): argument validation with defaults and range checks"
```

---

## Task 6: argv-template rendering

**Files:**
- Replace: `crates/wield-core/src/template.rs`
- Test: `crates/wield-core/tests/argv_rendering.rs`

**Interfaces:**
- Consumes: `descriptor::{CommandArg, When, OutputSpec, OutputDir}`, `args::{ArgMap, ArgValue}`, `visible_args`/`value_satisfies_literal` as needed.
- Produces:
  - `pub(crate) fn placeholders(template: &str) -> Result<Vec<String>, TemplateError>` (shared with Task 3).
  - `pub fn render_output_name(name_template: &str, effective: &ArgMap) -> Result<String, TemplateError>` — resolves `{input_stem}`, `{format}`, `{<arg>}`; **not** `{output}`.
  - `pub fn compute_output_path(output: &OutputSpec, effective: &ArgMap) -> Result<Option<PathBuf>, TemplateError>` — for `OutputSpec::File { name, dir }`: render `name`, join onto `dir` (`SameAsInput` → parent of the `input` arg's path; `Fixed(p)` → `p`). Returns `Ok(None)` for non-`File` outputs.
  - `pub fn render_argv(args: &[CommandArg], effective: &ArgMap, output_path: Option<&Path>) -> Result<Vec<String>, TemplateError>`.
  - `enum TemplateError { MalformedPlaceholder(String), UnresolvedInOutputName(String), MissingInputArg, NonUtf8Path(PathBuf) }` (thiserror or manual Display; wrap into `CoreError::Template` at call sites).
- **Derived placeholder resolution** (used by `render_argv` and `render_output_name`):
  - `{input}` → the `input` arg's `ArgValue::Path` as a string.
  - `{input_stem}` → `Path::file_stem` of the `input` path.
  - `{input_dir}` → `Path::parent` of the `input` path (empty string if none).
  - `{format}` → the `format` arg's `Str` value.
  - `{output}` → the `output_path` argument passed to `render_argv` (error if a template uses `{output}` but `output_path` is `None`).
  - `{<name>}` → `effective[name].as_display_string()`.
- **Segment-drop rule** (`render_argv`): for each `CommandArg`:
  1. If `when` is `Some` and not satisfied by `effective` (same check as `visible_args`) → skip the segment.
  2. Else resolve every placeholder in `template`. If **any** placeholder has no value (arg absent from `effective`, or a derived placeholder whose backing arg is absent) → skip the segment.
  3. Else push the fully-substituted string as **one** argv element (no shell word-splitting — a value with spaces stays one element).
- Non-UTF-8 paths → `TemplateError::NonUtf8Path` (argv rendering needs `String`).

- [ ] **Step 1: Write the failing tests**

`crates/wield-core/tests/argv_rendering.rs`:

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use wield_core::args::ArgValue;
use wield_core::descriptor::CommandArg;
use wield_core::descriptor::{When, ArgValueLiteral};
use wield_core::template::{render_argv, render_output_name};

fn map(pairs: &[(&str, ArgValue)]) -> BTreeMap<String, ArgValue> {
    pairs.iter().cloned().map(|(k, v)| (k.to_string(), v)).collect()
}

#[test]
fn substitutes_input_and_output_and_derived() {
    let effective = map(&[
        ("input", ArgValue::Path(PathBuf::from("/home/a/pic 1.png"))),
        ("format", ArgValue::Str("webp".into())),
    ]);
    let args: Vec<CommandArg> = vec![
        "{input}".into(), "-quality".into(), "82".into(), "{output}".into(),
    ];
    let out = PathBuf::from("/home/a/pic 1.webp");
    let argv = render_argv(&args, &effective, Some(&out)).unwrap();
    assert_eq!(argv, vec![
        "/home/a/pic 1.png".to_string(),
        "-quality".into(),
        "82".into(),
        "/home/a/pic 1.webp".into(),
    ]);
}

#[test]
fn drops_segment_whose_optional_arg_is_absent() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let args: Vec<CommandArg> = vec![
        "{input}".into(),
        "-resize".into(),
        "{width}x".into(),   // width absent -> this element drops
    ];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(argv, vec!["/x/a.png".to_string(), "-resize".into()]);
}

#[test]
fn drops_both_paired_segments_via_matching_when() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let gate = Some(When { arg: "resize".into(), in_values: vec![ArgValueLiteral::Bool(true)] });
    let args = vec![
        CommandArg::from("{input}"),
        CommandArg { template: "-resize".into(), when: gate.clone() },
        CommandArg { template: "{width}x".into(), when: gate },
    ];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(argv, vec!["/x/a.png".to_string()]);
}

#[test]
fn render_output_name_uses_stem_and_format() {
    let effective = map(&[
        ("input", ArgValue::Path(PathBuf::from("/x/holiday.jpeg"))),
        ("format", ArgValue::Str("png".into())),
    ]);
    let name = render_output_name("{input_stem}.{format}", &effective).unwrap();
    assert_eq!(name, "holiday.png");
}

#[test]
fn output_placeholder_without_path_is_an_error() {
    let effective = map(&[("input", ArgValue::Path(PathBuf::from("/x/a.png")))]);
    let args: Vec<CommandArg> = vec!["{output}".into()];
    assert!(render_argv(&args, &effective, None).is_err());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test argv_rendering`
Expected: FAIL.

- [ ] **Step 3: Implement `src/template.rs`**

Keep `placeholders()` (added in Task 3) here and make it `pub(crate)`. Implement the rest per the Interfaces block.

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test argv_rendering`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/template.rs crates/wield-core/tests/argv_rendering.rs
git commit -m "feat(core): argv-template rendering with segment-drop"
```

---

## Task 7: Binary resolution

**Files:**
- Modify: `crates/wield-core/src/command.rs` (add `BinaryResolver`)
- Test: `crates/wield-core/tests/command_runner.rs` (binary-resolution section)

**Interfaces:**
- Produces:
  - `pub struct BinaryResolver { extra_dirs: Vec<PathBuf> }`
  - `impl BinaryResolver { pub fn from_env() -> Self; pub fn with_dirs(dirs: Vec<PathBuf>) -> Self; pub fn resolve(&self, binary: &str) -> Option<PathBuf> }`
  - Resolution order: if `binary` contains a `/`, treat as a path and return it iff it exists and is a file; else scan `extra_dirs` then `$PATH` entries, returning the first `<dir>/<binary>` that exists and is executable (`std::os::unix::fs::PermissionsExt`, any execute bit).
- `from_env()` reads `$PATH`; `with_dirs` is for tests / the future bundled-Flatpak path (extra_dirs take precedence).

- [ ] **Step 1: Write the failing test**

Add to `crates/wield-core/tests/command_runner.rs`:

```rust
mod support;
use wield_core::command::BinaryResolver;

#[test]
fn resolves_a_binary_on_a_custom_dir() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "faketool", "#!/bin/sh\necho hi\n");
    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    assert_eq!(resolver.resolve("faketool"), Some(script));
    assert_eq!(resolver.resolve("definitely-missing-xyz"), None);
}
```

`crates/wield-core/tests/support/mod.rs`:

```rust
#![allow(dead_code)]
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Write `body` to `<dir>/<name>`, chmod 0o755, return the path.
pub fn write_stub_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner resolves_a_binary`
Expected: FAIL.

- [ ] **Step 3: Implement `BinaryResolver` in `src/command.rs`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner resolves_a_binary`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/command.rs crates/wield-core/tests/command_runner.rs crates/wield-core/tests/support
git commit -m "feat(core): binary resolution via PATH and extra dirs"
```

---

## Task 8: Outcome types and stderr hints

**Files:**
- Replace: `crates/wield-core/src/outcome.rs`
- Test: unit tests inside `src/outcome.rs`

**Interfaces:**
- Produces:
  - `enum Stage { Validation, Portal, Command, Output, Native }` (`Serialize`, `Debug`, `Clone`, `PartialEq`, `Eq`)
  - `enum ToolOutcome { Value { kind: ValueKind, data: String }, File { path: PathBuf }, Report { title: String, lines: Vec<String> }, Unavailable { reason: String, fix: Option<String> }, Failed { stage: Stage, detail: String, hint: Option<String> }, Cancelled }` (`Serialize`, `Debug`, `Clone`, `PartialEq`)
  - `enum Progress { Started, Percent(u8), Message(String), Finished }` (`Serialize`, `Debug`, `Clone`, `PartialEq`)
  - `pub fn hint_for_stderr(stderr_tail: &str) -> Option<String>` — a small match table from the spec:
    - contains `"Invalid data found"` → `"input file may be corrupt or in a different format than its extension"`
    - contains `"no decode delegate"` or `"no encode delegate"` → `"this image format needs a codec that isn't installed"`
    - contains `"Permission denied"` → `"open the file through the picker so Wield is granted access to it"`
    - contains `"No such file or directory"` → `"an input path no longer exists"`
    - else `None`
- `ValueKind` is re-exported from `descriptor` — `use crate::descriptor::ValueKind;`.

- [ ] **Step 1: Write the failing test**

In `src/outcome.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_stderr_to_hint() {
        assert!(hint_for_stderr("x: Invalid data found when processing input").unwrap().contains("corrupt"));
        assert!(hint_for_stderr("convert: no decode delegate for this image format").unwrap().contains("codec"));
        assert_eq!(hint_for_stderr("some unrecognised error"), None);
    }

    #[test]
    fn outcome_serialises_stably() {
        let o = ToolOutcome::Failed { stage: Stage::Command, detail: "exit 1".into(), hint: None };
        let j = serde_json::to_string(&o).unwrap();
        assert!(j.contains("Failed") && j.contains("Command"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core outcome`
Expected: FAIL.

- [ ] **Step 3: Implement `src/outcome.rs`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core outcome`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/outcome.rs
git commit -m "feat(core): tool outcome taxonomy and stderr hints"
```

---

## Task 9: Command runner — spawn, success, atomic output

**Files:**
- Modify: `crates/wield-core/src/command.rs`
- Modify: `crates/wield-core/tests/command_runner.rs`

**Interfaces:**
- Consumes: `outcome::Progress` (Task 8), `descriptor::{ProgressSpec, SuccessSpec}`.
- Produces:
  - `pub struct OutputPlan { pub temp: PathBuf, pub final_path: PathBuf }`
  - `impl OutputPlan { pub fn for_final(final_path: PathBuf) -> Self }` — `temp` = same dir, name `.wield-tmp-<uuid>-<final_name>`.
  - `pub enum CommandResult { Success { output_file: Option<PathBuf> }, NonZeroExit { code: Option<i32>, stderr_tail: String }, Timeout, Cancelled, SpawnFailed { detail: String }, OutputMissing }`
  - `pub struct CommandRunner;`
  - `impl CommandRunner { pub async fn execute(spec: RunSpec<'_>) -> CommandResult }`
  - `pub struct RunSpec<'a> { pub binary: &'a Path, pub argv: &'a [String], pub cwd: Option<&'a Path>, pub output: Option<&'a OutputPlan>, pub progress_spec: &'a ProgressSpec, pub success: &'a SuccessSpec, pub timeout: Duration, pub progress: mpsc::Sender<Progress>, pub cancel: CancellationToken }`
- **Success path (this task):**
  1. Send `Progress::Started`.
  2. Spawn `tokio::process::Command::new(binary).args(argv).current_dir(cwd?)` with `stdout(Stdio::piped())`, `stderr(Stdio::piped())`, and `.process_group(0)` (via `std::os::unix::process::CommandExt`). Spawn error → `SpawnFailed`.
  3. Concurrently drain stdout+stderr to in-memory buffers (cap stderr retained tail at 8 KiB).
  4. `await` the child (Task 10 adds the `select!` with timeout/cancel; for this task just `child.wait().await`).
  5. On exit: if `SuccessSpec::ExitZero` and status success →
     - if `output` is `Some(plan)`: `fsync` `plan.temp` (open, `sync_all`), then `fs::rename(plan.temp, plan.final_path)`; return `Success { output_file: Some(final_path) }`. If `plan.temp` does not exist → `OutputMissing`.
     - if `output` is `None`: `Success { output_file: None }`.
  6. On non-zero exit → `NonZeroExit { code, stderr_tail }` and **delete `plan.temp` if present**.
  7. Always send `Progress::Finished` before returning (even on failure).

- [ ] **Step 1: Write the failing tests**

Append to `crates/wield-core/tests/command_runner.rs`:

```rust
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::command::{CommandRunner, CommandResult, OutputPlan, RunSpec};
use wield_core::descriptor::{ProgressSpec, SuccessSpec};
use std::time::Duration;

#[tokio::test]
async fn writes_output_atomically_on_success() {
    let dir = tempfile::tempdir().unwrap();
    // stub: write "$1" contents to the path given as the last argv element ({output})
    let script = support::write_stub_script(
        dir.path(), "conv",
        "#!/bin/sh\nprintf 'converted' > \"$2\"\n",
    );
    let final_path = dir.path().join("result.out");
    let plan = OutputPlan::for_final(final_path.clone());
    let (tx, mut rx) = mpsc::channel(16);
    let argv = vec!["ignored".to_string(), plan.temp.to_string_lossy().into_owned()];
    let res = CommandRunner::execute(RunSpec {
        binary: &script, argv: &argv, cwd: None, output: Some(&plan),
        progress_spec: &ProgressSpec::None, success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5), progress: tx, cancel: CancellationToken::new(),
    }).await;

    assert!(matches!(res, CommandResult::Success { output_file: Some(ref p) } if *p == final_path));
    assert_eq!(std::fs::read_to_string(&final_path).unwrap(), "converted");
    assert!(!plan.temp.exists(), "temp file must be gone after rename");

    let mut seen = vec![];
    while let Ok(p) = rx.try_recv() { seen.push(p); }
    assert_eq!(seen.first(), Some(&wield_core::outcome::Progress::Started));
    assert_eq!(seen.last(), Some(&wield_core::outcome::Progress::Finished));
}

#[tokio::test]
async fn nonzero_exit_reports_stderr_tail_and_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(
        dir.path(), "boom",
        "#!/bin/sh\necho 'no decode delegate for FOO' 1>&2\nexit 3\n",
    );
    let plan = OutputPlan::for_final(dir.path().join("x.out"));
    std::fs::write(&plan.temp, b"partial").unwrap();
    let (tx, _rx) = mpsc::channel(16);
    let argv: Vec<String> = vec![];
    let res = CommandRunner::execute(RunSpec {
        binary: &script, argv: &argv, cwd: None, output: Some(&plan),
        progress_spec: &ProgressSpec::None, success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5), progress: tx, cancel: CancellationToken::new(),
    }).await;

    match res {
        CommandResult::NonZeroExit { code, stderr_tail } => {
            assert_eq!(code, Some(3));
            assert!(stderr_tail.contains("no decode delegate"));
        }
        other => panic!("expected NonZeroExit, got {other:?}"),
    }
    assert!(!plan.temp.exists(), "temp must be cleaned on failure");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner atomically`
Expected: FAIL.

- [ ] **Step 3: Implement the success path in `src/command.rs`**

Use `tokio::io::AsyncReadExt` to drain pipes. `.process_group(0)` needs `use std::os::unix::process::CommandExt;` on the `tokio::process::Command` — Tokio re-exposes it via `Command::as_std_mut` or the `CommandExt` impl; if the trait is not implemented for `tokio::process::Command`, build a `std::process::Command`, call `.process_group(0)`, then `tokio::process::Command::from(std_cmd)`.

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner`
Expected: PASS (all command_runner tests so far).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/command.rs crates/wield-core/tests/command_runner.rs
git commit -m "feat(core): command runner spawn + atomic output"
```

---

## Task 10: Command runner — timeout and cancellation

**Files:**
- Modify: `crates/wield-core/src/command.rs`
- Modify: `crates/wield-core/tests/command_runner.rs`

**Interfaces:**
- Extends `CommandRunner::execute`: the `child.wait()` from Task 9 becomes:

```rust
tokio::select! {
    status = child.wait() => { /* Task 9 exit handling */ }
    _ = cancel.cancelled() => { terminate(&child, pgid).await; CommandResult::Cancelled }
    _ = tokio::time::sleep(timeout) => { terminate(&child, pgid).await; CommandResult::Timeout }
}
```
- `terminate(child, pgid)`: `libc::kill(-pgid, libc::SIGTERM)`; wait up to 3 s for exit (`tokio::time::timeout(Duration::from_secs(3), child.wait())`); if still alive `libc::kill(-pgid, libc::SIGKILL)` then `child.wait().await`. `pgid` = child pid (it is its own group leader via `process_group(0)`).
- On both `Timeout` and `Cancelled`: delete `output.temp` if present; still send `Progress::Finished`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn kills_on_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "slow", "#!/bin/sh\nsleep 30\n");
    let plan = OutputPlan::for_final(dir.path().join("x.out"));
    std::fs::write(&plan.temp, b"partial").unwrap();
    let (tx, _rx) = mpsc::channel(16);
    let argv: Vec<String> = vec![];
    let start = std::time::Instant::now();
    let res = CommandRunner::execute(RunSpec {
        binary: &script, argv: &argv, cwd: None, output: Some(&plan),
        progress_spec: &ProgressSpec::None, success: &SuccessSpec::ExitZero,
        timeout: Duration::from_millis(300), progress: tx, cancel: CancellationToken::new(),
    }).await;
    assert!(matches!(res, CommandResult::Timeout));
    assert!(start.elapsed() < Duration::from_secs(5));
    assert!(!plan.temp.exists());
}

#[tokio::test]
async fn cancels_promptly() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "slow2", "#!/bin/sh\nsleep 30\n");
    let (tx, _rx) = mpsc::channel(16);
    let cancel = CancellationToken::new();
    let argv: Vec<String> = vec![];
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        c2.cancel();
    });
    let res = CommandRunner::execute(RunSpec {
        binary: &script, argv: &argv, cwd: None, output: None,
        progress_spec: &ProgressSpec::None, success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(30), progress: tx, cancel,
    }).await;
    assert!(matches!(res, CommandResult::Cancelled));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner kills_on_timeout cancels_promptly`
Expected: FAIL.

- [ ] **Step 3: Implement `select!` + `terminate`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/command.rs crates/wield-core/tests/command_runner.rs
git commit -m "feat(core): command runner timeout + cancellation with SIGTERM/SIGKILL"
```

---

## Task 11: Progress plumbing

**Files:**
- Modify: `crates/wield-core/src/command.rs`
- Modify: `crates/wield-core/tests/command_runner.rs`

**Interfaces:**
- `ProgressSpec::None`: emit exactly `Started` (before spawn) and `Finished` (after the child resolves, any outcome). No `Percent`.
- Add `pub(crate) trait ProgressParser { fn parse_line(&mut self, line: &str) -> Option<Progress>; }` and `pub(crate) fn parser_for(spec: &ProgressSpec) -> Option<Box<dyn ProgressParser + Send>>` — returns `None` for `ProgressSpec::None`. When a parser is present, feed it each **stderr** line as it is drained and forward any `Some(Progress)` on the channel. (No parser exists yet; this is the seam M2's `Ffmpeg` parser plugs into.)
- Channel-full handling: use `progress.try_send(..)` and **drop** the update if the receiver is lagging — never block the child on progress.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn emits_started_then_finished_for_progress_none() {
    let dir = tempfile::tempdir().unwrap();
    let script = support::write_stub_script(dir.path(), "quick", "#!/bin/sh\nexit 0\n");
    let (tx, mut rx) = mpsc::channel(16);
    let argv: Vec<String> = vec![];
    let _ = CommandRunner::execute(RunSpec {
        binary: &script, argv: &argv, cwd: None, output: None,
        progress_spec: &ProgressSpec::None, success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5), progress: tx, cancel: CancellationToken::new(),
    }).await;
    let mut seen = vec![];
    while let Some(p) = rx.recv().await { seen.push(p); }
    assert_eq!(seen, vec![
        wield_core::outcome::Progress::Started,
        wield_core::outcome::Progress::Finished,
    ]);
}
```

- [ ] **Step 2: Run to verify failure / regression**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner emits_started`
Expected: FAIL until the ordering guarantee is enforced.

- [ ] **Step 3: Implement the parser seam + strict `Started`/`Finished`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test command_runner`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/command.rs crates/wield-core/tests/command_runner.rs
git commit -m "feat(core): progress channel plumbing and parser seam"
```

---

## Task 12: Executor pipeline

**Files:**
- Replace: `crates/wield-core/src/executor.rs`
- Test: `crates/wield-core/tests/executor_pipeline.rs`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `pub struct AvailabilityView { pub binaries: std::collections::HashSet<String>, pub portals: std::collections::HashMap<String, u32> }` with `impl AvailabilityView { pub fn probe_binaries(resolver: &BinaryResolver, descriptors: &[Descriptor]) -> Self }` (fills `binaries` with the `Requires::Binary` names that resolve; `portals` left empty in P2).
  - `pub struct ExecutionRequest { pub descriptor: Descriptor, pub args: ArgMap }`
  - `pub struct Executor { resolver: BinaryResolver }`
  - `impl Executor { pub fn new(resolver: BinaryResolver) -> Self; pub async fn run(&self, req: ExecutionRequest, progress: mpsc::Sender<Progress>, cancel: CancellationToken) -> ToolOutcome }`
- **Pipeline in `run`:**
  1. `validate_descriptor(&req.descriptor)` — on `Err` → `ToolOutcome::Failed { stage: Validation, detail: joined errors, hint: None }` (defensive; built-ins are pre-validated).
  2. `validate_args(&descriptor.args, &req.args)` — on `Err` → `Failed { stage: Validation, detail: joined field errors, hint: None }`. On `Ok(effective)` continue.
  3. `match &descriptor.requires`:
     - `Requires::Binary(b)` → `self.resolver.resolve(b)`; `None` → `ToolOutcome::Unavailable { reason: format!("{b} is not installed"), fix: Some(format!("install {b} or add it to your PATH")) }`.
     - `Requires::Portal { .. }` → in P2, `Unavailable { reason: "portal capability probing lands in P3".into(), fix: None }`.
     - `Requires::None` → continue.
  4. `match &descriptor.capability`:
     - `Capability::Command(spec)`:
       a. `compute_output_path(&descriptor.output, &effective)?` → `Option<PathBuf>`; wrap `TemplateError` → `Failed { stage: Output, .. }`.
       b. If `Some(final_path)`: `OutputPlan::for_final(final_path)`; the argv `{output}` will resolve to `plan.temp`.
       c. `render_argv(&spec.args, &effective, plan.as_ref().map(|p| p.temp.as_path()))` → `Failed { stage: Command, .. }` on error.
       d. `let binary = self.resolver.resolve(&spec.binary)` (may differ from `requires`); `None` → `Unavailable`.
       e. `cwd` = the `input` arg's parent dir if present, else `None`.
       f. `CommandRunner::execute(RunSpec { .. }).await`, then map `CommandResult` → `ToolOutcome`:
          - `Success { output_file: Some(p) }` → `File { path: p }`
          - `Success { output_file: None }` → `Failed { stage: Output, detail: "command produced no output", hint: None }` (a P2 `Command` tool always has `OutputSpec::File`)
          - `NonZeroExit { code, stderr_tail }` → `Failed { stage: Command, detail: format!("exited with {}", code.map(|c| c.to_string()).unwrap_or_else(|| "signal".into())), hint: hint_for_stderr(&stderr_tail) }`
          - `Timeout` → `Failed { stage: Command, detail: format!("timed out after {:?}", spec.timeout), hint: None }`
          - `Cancelled` → `ToolOutcome::Cancelled`
          - `SpawnFailed { detail }` → `Failed { stage: Command, detail, hint: Some(format!("could not start {}", spec.binary)) }`
          - `OutputMissing` → `Failed { stage: Output, detail: "expected output file was not created".into(), hint: None }`
     - `Capability::Portal { .. }` → `Failed { stage: Portal, detail: "portal execution is implemented in P3".into(), hint: None }`
     - `Capability::Native { .. }` → `Failed { stage: Native, detail: "native execution is implemented in P4".into(), hint: None }`

- [ ] **Step 1: Write the failing tests**

`crates/wield-core/tests/executor_pipeline.rs`:

```rust
mod support;

use std::collections::BTreeMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::args::ArgValue;
use wield_core::command::BinaryResolver;
use wield_core::descriptor::*;
use wield_core::executor::{Executor, ExecutionRequest};
use wield_core::outcome::{Stage, ToolOutcome};

fn convert_descriptor(binary: &str) -> Descriptor {
    Descriptor {
        id: ToolId::parse("image.convert").unwrap(),
        title: "Convert".into(), keywords: vec![], category: Category::Convert,
        args: vec![
            ArgSpec { name: "input".into(), label: "in".into(), help: None,
                arg_type: ArgType::File { filters: vec![], multiple: false },
                default: None, required: true, when: None },
            ArgSpec { name: "format".into(), label: "fmt".into(), help: None,
                arg_type: ArgType::Enum { options: vec!["out".into()] },
                default: Some(ArgValueLiteral::Str("out".into())), required: true, when: None },
        ],
        requires: Requires::Binary(binary.into()),
        output: OutputSpec::File { name: "{input_stem}.{format}".into(), dir: OutputDir::SameAsInput },
        capability: Capability::Command(CommandSpec {
            binary: binary.into(),
            args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None, timeout: Duration::from_secs(5),
            success: SuccessSpec::ExitZero,
        }),
    }
}

#[tokio::test]
async fn runs_a_command_tool_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let bin = support::write_stub_script(dir.path(), "cp-conv",
        "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let input = dir.path().join("photo.raw");
    std::fs::write(&input, b"PIXELS").unwrap();

    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    let exec = Executor::new(resolver);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input.clone()));

    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec.run(
        ExecutionRequest { descriptor: convert_descriptor("cp-conv"), args },
        tx, CancellationToken::new(),
    ).await;

    match outcome {
        ToolOutcome::File { path } => {
            assert_eq!(path, dir.path().join("photo.out"));
            assert_eq!(std::fs::read(&path).unwrap(), b"PIXELS");
        }
        other => panic!("expected File, got {other:?}"),
    }
}

#[tokio::test]
async fn missing_binary_is_unavailable() {
    let resolver = BinaryResolver::with_dirs(vec![]);
    let exec = Executor::new(resolver);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x.raw".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec.run(
        ExecutionRequest { descriptor: convert_descriptor("nope-missing"), args },
        tx, CancellationToken::new(),
    ).await;
    assert!(matches!(outcome, ToolOutcome::Unavailable { .. }));
}

#[tokio::test]
async fn portal_capability_is_placeholder_failure() {
    let mut d = convert_descriptor("sh");
    d.requires = Requires::None;
    d.capability = Capability::Portal { adapter: "screenshot.pick_color".into() };
    let exec = Executor::new(BinaryResolver::from_env());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec.run(ExecutionRequest { descriptor: d, args }, tx, CancellationToken::new()).await;
    assert!(matches!(outcome, ToolOutcome::Failed { stage: Stage::Portal, .. }));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test executor_pipeline`
Expected: FAIL.

- [ ] **Step 3: Implement `src/executor.rs`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test executor_pipeline`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/executor.rs crates/wield-core/tests/executor_pipeline.rs
git commit -m "feat(core): executor pipeline for Command capability"
```

---

## Task 13: Registry

**Files:**
- Replace: `crates/wield-core/src/registry.rs`
- Test: `crates/wield-core/tests/registry.rs`

**Interfaces:**
- Consumes: `descriptor`, `validate_descriptor`, `executor::AvailabilityView`.
- Produces:
  - `pub enum RegistryError { DuplicateId(String), Invalid { id: String, errors: Vec<DescriptorError> } }`
  - `pub struct Registry { tools: Vec<Descriptor> }`
  - `impl Registry`:
    - `pub fn new() -> Self`
    - `pub fn register(&mut self, d: Descriptor) -> Result<(), RegistryError>` — runs `validate_descriptor`; rejects a duplicate `id`.
    - `pub fn len` / `pub fn is_empty`
    - `pub fn list(&self) -> &[Descriptor]`
    - `pub fn get(&self, id: &str) -> Option<&Descriptor>`
    - `pub fn search(&self, query: &str) -> Vec<&Descriptor>` — if `query` is blank, return all in registration order; else score each tool with `fuzzy_matcher::skim::SkimMatcherV2` against `title` and each `keyword`, keep the max score, drop non-matches, sort by score desc then `id` asc.
    - `pub fn available<'a>(&'a self, view: &AvailabilityView) -> Vec<&'a Descriptor>` — keep tools whose `requires` is satisfied: `None` always; `Binary(b)` iff `view.binaries.contains(b)`; `Portal { iface, min_ver }` iff `view.portals.get(iface) >= Some(&min_ver)`.
    - `pub fn snapshot(&self) -> String` — `serde_json::to_string_pretty` of `list()` **after** sorting a clone by `id`; stable across runs.

- [ ] **Step 1: Write the failing tests**

`crates/wield-core/tests/registry.rs`:

```rust
use std::time::Duration;
use wield_core::descriptor::*;
use wield_core::executor::AvailabilityView;
use wield_core::registry::{Registry, RegistryError};

fn cmd_tool(id: &str, binary: &str, keywords: &[&str]) -> Descriptor {
    Descriptor {
        id: ToolId::parse(id).unwrap(),
        title: id.into(),
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        category: Category::Convert,
        args: vec![ArgSpec { name: "input".into(), label: "in".into(), help: None,
            arg_type: ArgType::File { filters: vec![], multiple: false },
            default: None, required: true, when: None }],
        requires: Requires::Binary(binary.into()),
        output: OutputSpec::File { name: "{input_stem}.o".into(), dir: OutputDir::SameAsInput },
        capability: Capability::Command(CommandSpec {
            binary: binary.into(), args: vec!["{input}".into(), "{output}".into()],
            progress: ProgressSpec::None, timeout: Duration::from_secs(5),
            success: SuccessSpec::ExitZero }),
    }
}

#[test]
fn rejects_duplicate_ids() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &[])).unwrap();
    let err = r.register(cmd_tool("image.convert", "magick", &[])).unwrap_err();
    assert!(matches!(err, RegistryError::DuplicateId(id) if id == "image.convert"));
}

#[test]
fn fuzzy_search_ranks_by_relevance() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &["picture", "resize"])).unwrap();
    r.register(cmd_tool("video.convert", "ffmpeg", &["movie"])).unwrap();
    let hits = r.search("img conv");
    assert_eq!(hits.first().unwrap().id.as_ref(), "image.convert");
    assert!(r.search("zzz nonsense").is_empty());
}

#[test]
fn available_filters_on_binaries() {
    let mut r = Registry::new();
    r.register(cmd_tool("image.convert", "magick", &[])).unwrap();
    r.register(cmd_tool("video.convert", "ffmpeg", &[])).unwrap();
    let mut view = AvailabilityView { binaries: Default::default(), portals: Default::default() };
    view.binaries.insert("magick".into());
    let avail: Vec<_> = r.available(&view).iter().map(|d| d.id.as_ref().to_string()).collect();
    assert_eq!(avail, vec!["image.convert"]);
}

#[test]
fn snapshot_is_deterministic() {
    let mut a = Registry::new();
    a.register(cmd_tool("b.two", "x", &[])).unwrap();
    a.register(cmd_tool("a.one", "y", &[])).unwrap();
    let mut b = Registry::new();
    b.register(cmd_tool("a.one", "y", &[])).unwrap();
    b.register(cmd_tool("b.two", "x", &[])).unwrap();
    assert_eq!(a.snapshot(), b.snapshot());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test registry`
Expected: FAIL.

- [ ] **Step 3: Implement `src/registry.rs`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test registry`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/registry.rs crates/wield-core/tests/registry.rs
git commit -m "feat(core): tool registry with fuzzy search and snapshot"
```

---

## Task 14: Public API, builders, and crate integration test

**Files:**
- Replace: `crates/wield-core/src/builder.rs`
- Modify: `crates/wield-core/src/lib.rs` (re-exports)
- Test: `crates/wield-core/tests/support/mod.rs` (add a descriptor helper), a new `#[tokio::test]` in `tests/executor_pipeline.rs`

**Interfaces:**
- Produces (builders — the sanctioned construction path for P4 built-ins):
  - `DescriptorBuilder::new(id: &str, title: &str, category: Category) -> Self` (panics only on an invalid `id`, since built-ins are authored in-repo — document this)
  - `.keyword(&str)`, `.keywords(&[&str])`
  - `.arg(ArgSpec)` and `ArgSpecBuilder` (`ArgSpecBuilder::new(name, label, arg_type).help(..).default(ArgValueLiteral).required(bool).when(arg, [literals]).build()`)
  - `.requires(Requires)`
  - `.output(OutputSpec)`
  - `.command(CommandSpecBuilder)` where `CommandSpecBuilder::new(binary).arg(impl Into<CommandArg>).arg_when(template, arg, [literals]).timeout(Duration).build()`
  - `.build() -> Result<Descriptor, Vec<DescriptorError>>` — runs `validate_descriptor` before returning.
- `lib.rs` re-exports, flat, so consumers write `wield_core::Descriptor`, `wield_core::Executor`, etc.:

```rust
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
pub use registry::{Registry, RegistryError};
pub use template::{compute_output_path, render_argv, render_output_name};
pub use validate::validate_descriptor;
```

- [ ] **Step 1: Write the failing test**

Add to `tests/executor_pipeline.rs`:

```rust
#[tokio::test]
async fn built_descriptor_runs_through_registry_and_executor() {
    use wield_core::{
        ArgType, Category, DescriptorBuilder, ArgSpecBuilder, CommandSpecBuilder,
        Requires, OutputSpec, OutputDir, Registry, Executor, BinaryResolver,
        ExecutionRequest, ArgValue, ToolOutcome, ArgValueLiteral,
    };
    use std::collections::BTreeMap;
    use std::time::Duration;

    let dir = tempfile::tempdir().unwrap();
    let bin = support::write_stub_script(dir.path(), "passthru", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let input = dir.path().join("in.data");
    std::fs::write(&input, b"OK").unwrap();

    let descriptor = DescriptorBuilder::new("data.pass", "Passthrough", Category::Convert)
        .keyword("copy")
        .arg(ArgSpecBuilder::new("input", "Input", ArgType::File { filters: vec![], multiple: false })
            .required(true).build())
        .arg(ArgSpecBuilder::new("format", "Format", ArgType::Enum { options: vec!["data".into()] })
            .default(ArgValueLiteral::Str("data".into())).required(true).build())
        .requires(Requires::Binary("passthru".into()))
        .output(OutputSpec::File { name: "{input_stem}.{format}".into(), dir: OutputDir::SameAsInput })
        .command(CommandSpecBuilder::new("passthru")
            .arg("{input}").arg("{output}").timeout(Duration::from_secs(5)))
        .build()
        .expect("descriptor should be valid");

    let mut reg = Registry::new();
    reg.register(descriptor.clone()).unwrap();
    assert_eq!(reg.search("copy").len(), 1);

    let exec = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input));
    let (tx, _rx) = tokio::sync::mpsc::channel(16);
    let outcome = exec.run(ExecutionRequest { descriptor, args }, tx, tokio_util::sync::CancellationToken::new()).await;
    assert!(matches!(outcome, ToolOutcome::File { ref path } if path.ends_with("in.data")));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test executor_pipeline built_descriptor`
Expected: FAIL — builders + re-exports missing.

- [ ] **Step 3: Implement `src/builder.rs` and the `lib.rs` re-exports**

- [ ] **Step 4: Run the full crate test suite**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core`
Expected: PASS — every test file.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-core/src/builder.rs crates/wield-core/src/lib.rs crates/wield-core/tests/executor_pipeline.rs
git commit -m "feat(core): descriptor builders and flat public API"
```

---

## Task 15: Workspace green + P2 wrap-up

**Files:**
- Modify: this plan file (check the boxes)

- [ ] **Step 1: Full workspace check**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: fmt clean, clippy clean, all tests pass (`wield-core` plus the P1 stubs in `wield-portal` / `wield-tools` / `wield-cli` / `wield-app`).

- [ ] **Step 2: `npm run check`**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && npm run check`
Expected: PASS (lint + typecheck + vitest + `cargo test`).

- [ ] **Step 3: Confirm `wield-cli` still runs**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo run -p wield-cli`
Expected: prints `wield 0.1.0` (P2 does not touch `wield-cli`; this is a regression check).

- [ ] **Step 4: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P2 complete" --body "wield-core landed on feat/p2-core-executor: descriptor model + validation + arg validation + argv rendering + Command executor (spawn/progress/timeout/cancel/atomic output) + registry. cargo test --workspace + clippy + npm run check all green. N commits. Ready for review + PR."
```

- [ ] **Step 5: Commit the checked-off plan**

```bash
git add docs/superpowers/plans/2026-09-10-wield-m1-p2-core-executor.md
git commit -m "docs: mark M1 P2 plan complete"
git push -u origin feat/p2-core-executor
```

GEON opens the PR, reviews, and merges to `master`. P3 (`wield-portal`) is written just-in-time after P2 merges.

---

## Self-Review

**Spec coverage (§4, §7, §8 `wield-core`):**

- §4 tool descriptor (id/title/keywords/category/args/requires/output/capability) → Task 2 ✓
- §4 `ArgSpec` (name/label/help/type/default/required/`when`) + all `type` variants → Tasks 2, 4, 5 ✓
- §4 `Command` capability (binary/args/output/progress/timeout/success), template placeholders incl. derived → Tasks 2, 6, 9 ✓
- §4 `Portal` / `Native` capability shape (not execution) → Task 2 (`adapter` string / `NativeId`) ✓; execution deferred → Task 12 placeholder ✓
- §4 executor pipeline: validate → check requires → render → execute w/ cancel+timeout → progress → outcome → (post-actions are UI, P6) → Task 12 ✓
- §4 registry: list / get / fuzzy-search / filter-by-availability / snapshot test → Task 13 ✓
- §4 built-ins via type-safe Rust builders → Task 14 ✓; user-TOML explicitly deferred ✓
- §7 outcome taxonomy (`Cancelled` / `Unavailable{reason,fix}` / `Failed{stage,detail,hint}`) + `stage` values → Task 8, 12 ✓
- §7 user-cancel is silent (`ToolOutcome::Cancelled`, no card) → Task 10, 12 ✓
- §7 arg validation before execution, field-keyed errors → Task 5, 12 ✓
- §7 stderr → hint table (ffmpeg / ImageMagick / sandbox-denial examples) → Task 8 ✓
- §7 atomic output (temp in dest dir → fsync → rename on success only; failed/cancelled leaves nothing) → Task 9, 10 ✓
- §7 cancellation (SIGTERM → 3s → SIGKILL, process group; temp discarded) → Task 10 ✓
- §7 native `catch_unwind` at the Tauri boundary → **out of P2 scope** (P5, the Tauri command layer) — noted, not a gap ✓
- §8 `wield-core` unit tests: argv rendering, arg schema, descriptor validation, outcome mapping, binary resolution, registry snapshot → Tasks 3, 5, 6, 7, 8, 13 ✓
- §8 executor integration with stub scripts (progress parsing seam, timeout kill, cancellation, atomic rename, temp cleanup) → Tasks 9, 10, 11, 12 ✓
- §8 "a few real conversions against bundled ffmpeg/ImageMagick" → **deferred to P4** (needs real descriptors + bundled binaries); P2 proves the mechanism with stubs — noted ✓

**Placeholder scan:** No "TBD"/"TODO" in task steps. Deferred items (Ffmpeg progress parser, Portal/Native execution, user-TOML, real-binary conversions, `catch_unwind`) are each named with their owning plan. Stub capability arms return a documented `Failed` with the plan number.

**Type consistency:** `ArgMap` = `BTreeMap<String, ArgValue>` used identically in Tasks 4/5/6/12/14. `validate_args` returns the effective `ArgMap` consumed by `render_argv` (Task 6) and the executor (Task 12). `ToolOutcome` / `Stage` / `Progress` defined in Task 8, consumed in 9–14. `CommandResult` (Task 9) → `ToolOutcome` mapping owned entirely by Task 12. `AvailabilityView` defined in Task 12, consumed by `Registry::available` in Task 13 (Task 13 depends on Task 12 — ordering is correct). `placeholders()` defined in Task 3 (`template.rs`), reused by Task 6 — Task 3's commit includes `template.rs`.

**Ordering check:** 1 → 2 → 3 (needs 2) → 4 (needs 2) → 5 (needs 4) → 6 (needs 2,4, and `placeholders` from 3) → 7 → 8 → 9 (needs 8) → 10 (needs 9) → 11 (needs 9) → 12 (needs 2,3,5,6,7,8,9,10,11) → 13 (needs 12) → 14 (needs 12,13) → 15. Consistent.

---

## Execution note

P2 is `wield-core` only. `wield-tools` (real descriptors), `wield-portal` (real adapters + probe), and the Tauri command layer are P3–P5. The registry snapshot in Task 13 guards a **test fixture** set; the snapshot of the real built-in registry is added in P4 when there are built-ins to snapshot.

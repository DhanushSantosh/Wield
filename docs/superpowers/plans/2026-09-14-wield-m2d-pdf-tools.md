# M2d: `pdf.tools` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `pdf.compress`, `pdf.merge`, and `pdf.split` — the three
operations the design spec's milestone table calls "`pdf.tools`" — as
three separate descriptors, adding the two genuinely new `wield-core`
mechanisms merge (N inputs → 1 output) and split (1 input → N outputs)
need.

**Architecture:** Two new, independent `wield-core` mechanisms, each
scoped as tightly as the problem allows: a `combine_inputs` flag on
`CommandSpec` that lets `render_argv` spread a `Paths` value into N argv
elements instead of `run_batch`'s per-file loop (for merge); a new
`OutputSpec::Directory` variant that reuses `CommandRunner::execute`'s
existing `output: None` path completely unchanged, with all new
directory-discovery-and-move logic living in `executor.rs` (for split,
mirroring exactly where `run_batch` already lives). `pdf.compress` needs
neither — it's the same shape every prior M2 tool has used.

**Tech Stack:** Rust (`wield-core`, `wield-tools`), `qpdf` 12.4.1,
`ghostscript` 10.08.0 (binary name `gs`) — both installed on the target
dev machine for this plan's live-verification task.

**Spec:** `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`

**Task risk note:** Tasks 1 and 2 are the highest-review-priority work in
this plan — real, new `wield-core` control-flow changes, not descriptor
additions. Task 4 depends on Task 1 landing first (needs `combine_inputs`
and the `Paths`-fallback in `resolve()`); Task 5 depends on Task 2
landing first (needs `OutputSpec::Directory` and `run_split`). Task 3 has
no such dependency. Tasks still run in strict order 1→2→3→4→5→6 for
simplicity — don't parallelize even though Task 3 technically could run
earlier.

## Global Constraints

- No new Rust or npm dependencies. `uuid` is already a workspace
  dependency (used in `command.rs`) — reusing it in `executor.rs` needs
  no `Cargo.toml` change.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace -- --test-threads=1`, and `npm run check` (run
  from the repo root) must all be clean before every commit.
- Typed builders only for tool descriptors (Tasks 3-5) —
  `DescriptorBuilder`/`ArgSpecBuilder`/`CommandSpecBuilder`.
- After Task 1 lands, every *later* task's `CommandSpec` construction —
  via the builder, in Tasks 3-5 — needs zero changes for the new
  `combine_inputs` field; the builder's own `new()` already defaults it
  to `false`, and only `pdf.merge` (Task 4) calls
  `.combine_inputs(true)` explicitly. The only places that need an
  explicit edit for the new field are the four test files listed in
  Task 1 Step 6, which construct `CommandSpec` via a raw struct literal
  instead of the builder.
- This branch (`feat/m2d-pdf-tools`) is forked from `master` (all of
  M2a/b/c are merged). The spec commit (`e5b5c14`) is already on it.

---

### Task 1: `combine_inputs` mode — spreads a `Paths` value into N argv elements

**Files:**
- Modify: `crates/wield-core/src/descriptor.rs`
- Modify: `crates/wield-core/src/builder.rs`
- Modify: `crates/wield-core/src/template.rs`
- Modify (mechanical ripple, Rust requires every field in a struct
  literal): `crates/wield-core/tests/registry.rs`,
  `crates/wield-core/tests/descriptor_validation.rs`,
  `crates/wield-core/tests/descriptor_serde.rs`,
  `crates/wield-core/tests/executor_pipeline.rs`
- Test: `crates/wield-core/tests/argv_rendering.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `CommandSpec.combine_inputs: bool` (new field, default
  `false` via the builder), `CommandSpecBuilder::combine_inputs(bool) -> Self`
  (new setter). `template::resolve()`'s `input`/`input_stem`/`input_dir`
  case now also matches `ArgValue::Paths` (falls back to the first
  element) — Task 4 relies on this for `pdf.merge`'s output naming.
  `template::render_argv()` now spreads a bare `{name}` placeholder whose
  value is `ArgValue::Paths` into multiple argv elements — Task 4 relies
  on this for `{input}` in `pdf.merge`'s command.

- [ ] **Step 1: Add the `combine_inputs` field to `CommandSpec`**

In `crates/wield-core/src/descriptor.rs`, find:

```rust
pub struct CommandSpec {
    pub binary: String,
    pub args: Vec<CommandArg>,
    pub progress: ProgressSpec,
    #[serde(with = "duration_secs")]
    pub timeout: Duration,
    pub success: SuccessSpec,
}
```

Add one field:

```rust
pub struct CommandSpec {
    pub binary: String,
    pub args: Vec<CommandArg>,
    pub progress: ProgressSpec,
    #[serde(with = "duration_secs")]
    pub timeout: Duration,
    pub success: SuccessSpec,
    pub combine_inputs: bool,
}
```

- [ ] **Step 2: Add the builder setter**

In `crates/wield-core/src/builder.rs`, find `CommandSpecBuilder`'s struct
definition and `new()`:

```rust
pub struct CommandSpecBuilder {
    binary: String,
    args: Vec<CommandArg>,
    timeout: Duration,
    progress: ProgressSpec,
}

impl CommandSpecBuilder {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_owned(),
            args: Vec::new(),
            timeout: Duration::from_secs(300),
            progress: ProgressSpec::None,
        }
    }

    pub fn progress(mut self, progress: ProgressSpec) -> Self {
        self.progress = progress;
        self
    }
```

Replace with:

```rust
pub struct CommandSpecBuilder {
    binary: String,
    args: Vec<CommandArg>,
    timeout: Duration,
    progress: ProgressSpec,
    combine_inputs: bool,
}

impl CommandSpecBuilder {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_owned(),
            args: Vec::new(),
            timeout: Duration::from_secs(300),
            progress: ProgressSpec::None,
            combine_inputs: false,
        }
    }

    pub fn progress(mut self, progress: ProgressSpec) -> Self {
        self.progress = progress;
        self
    }

    /// When `true`, `Executor::run_command` skips the normal per-file
    /// batch loop for a `multiple: true` File arg and instead lets a
    /// bare `{name}` placeholder referencing it spread into N argv
    /// elements in one command invocation - for tools like `pdf.merge`
    /// that genuinely combine several inputs into one output, rather
    /// than converting each independently.
    pub fn combine_inputs(mut self, combine_inputs: bool) -> Self {
        self.combine_inputs = combine_inputs;
        self
    }
```

Then find `build()`'s `CommandSpec { ... }` literal:

```rust
    pub fn build(self) -> CommandSpec {
```

Read the rest of that function (it constructs and returns a
`CommandSpec { binary: ..., args: ..., progress: ..., timeout: ...,
success: ... }`) and add `combine_inputs: self.combine_inputs,` as one
more field in that literal - match the existing field order and style
exactly.

- [ ] **Step 3: Extend `resolve()`'s input-family handling to accept `Paths`**

In `crates/wield-core/src/template.rs`, find:

```rust
    if matches!(name, "input" | "input_stem" | "input_dir") {
        let Some(ArgValue::Path(input)) = effective.get("input") else {
            return Ok(None);
        };
        return match name {
```

Replace the first two lines with:

```rust
    if matches!(name, "input" | "input_stem" | "input_dir") {
        let input = match effective.get("input") {
            Some(ArgValue::Path(input)) => input,
            Some(ArgValue::Paths(paths)) if !paths.is_empty() => &paths[0],
            _ => return Ok(None),
        };
        return match name {
```

(Everything after this - the `match name { "input" => ..., "input_stem" => ...,
"input_dir" => ..., _ => Ok(None) }` block - is unchanged; it already
uses the `input` binding, which now just has one more way to be produced.)

- [ ] **Step 4: Let `resolve()`'s `"output"` case also handle `"output_dir"`**

Still in `resolve()`, find:

```rust
    if name == "output" {
        let Some(path) = output_path else {
            return Err(TemplateError::UnresolvedInOutputName(name.to_owned()));
        };
        return path_string(path).map(Some);
    }
```

Change the condition:

```rust
    if matches!(name, "output" | "output_dir") {
        let Some(path) = output_path else {
            return Err(TemplateError::UnresolvedInOutputName(name.to_owned()));
        };
        return path_string(path).map(Some);
    }
```

(This is preparation for Task 2, which will pass a *directory* path as
`output_path` for `OutputSpec::Directory` descriptors, and reference it
via `{output_dir}` instead of `{output}` in their command. No descriptor
in this task uses `{output_dir}` yet - this just makes `resolve()` ready
for it.)

- [ ] **Step 5: Add spread resolution to `render_argv`**

Still in `crates/wield-core/src/template.rs`, find `render_argv`'s
signature and its loop body:

```rust
pub fn render_argv(
    args: &[CommandArg],
    effective: &ArgMap,
    output_path: Option<&Path>,
) -> Result<Vec<String>, TemplateError> {
    let mut rendered = Vec::new();
    for segment in args {
        if segment.when.as_ref().is_some_and(|when| {
            !effective.get(&when.arg).is_some_and(|value| {
                // Empty `in_values` is the presence form: satisfied by any value.
                when.in_values.is_empty()
                    || when
                        .in_values
                        .iter()
                        .any(|literal| value_satisfies_literal(value, literal))
            })
        }) {
            continue;
        }

        let tokens = placeholders(&segment.template)?;
```

Insert a new check between the `when`-gate's closing `}` and the
`let tokens = ...` line:

```rust
pub fn render_argv(
    args: &[CommandArg],
    effective: &ArgMap,
    output_path: Option<&Path>,
) -> Result<Vec<String>, TemplateError> {
    let mut rendered = Vec::new();
    for segment in args {
        if segment.when.as_ref().is_some_and(|when| {
            !effective.get(&when.arg).is_some_and(|value| {
                // Empty `in_values` is the presence form: satisfied by any value.
                when.in_values.is_empty()
                    || when
                        .in_values
                        .iter()
                        .any(|literal| value_satisfies_literal(value, literal))
            })
        }) {
            continue;
        }

        // A segment that is *exactly* one bare placeholder (nothing else
        // in the template string) referencing a `Paths`-valued arg
        // spreads into one argv element per path, in order, instead of
        // the normal single-value substitution below. Only ever fires
        // when a raw `Paths` value reaches this function at all, which
        // by construction only happens when `CommandSpec.combine_inputs`
        // is `true` (otherwise `Executor::run_command` already routed
        // it through `run_batch`'s per-file loop before render_argv ever
        // sees it) - inert for every other descriptor.
        if let Some(bare_name) = bare_placeholder(&segment.template) {
            if let Some(ArgValue::Paths(paths)) = effective.get(bare_name) {
                for path in paths {
                    rendered.push(path_string(path)?);
                }
                continue;
            }
        }

        let tokens = placeholders(&segment.template)?;
```

Leave the rest of the function body (the per-token resolution loop and
final `rendered.push(value)`) unchanged.

Add the new helper function anywhere else in the file (e.g. right after
`render_argv`, before `resolve`):

```rust
/// `Some(name)` when `template` is exactly one placeholder with nothing
/// else around it (e.g. `"{input}"`), `None` otherwise (e.g.
/// `"{output_dir}/page-%d.pdf"`, which has more than just the
/// placeholder, or `"-y"`, which has none).
fn bare_placeholder(template: &str) -> Option<&str> {
    let inner = template.strip_prefix('{')?.strip_suffix('}')?;
    if inner.contains('{') || inner.contains('}') {
        return None;
    }
    Some(inner)
}
```

- [ ] **Step 6: Fix the mechanical ripple — 4 test files construct `CommandSpec` via a raw struct literal**

`CommandSpec` gained a new field in Step 1; Rust requires every field to
be named in a struct literal (unlike the builder, which already defaults
it). Four test files build `CommandSpec { ... }` directly and will fail
to compile until each gets the new field. In every one of these four
files, find the line `success: SuccessSpec::ExitZero,` inside a
`CommandSpec { ... }` literal and add `combine_inputs: false,` directly
after it:

- `crates/wield-core/tests/registry.rs`
- `crates/wield-core/tests/descriptor_validation.rs`
- `crates/wield-core/tests/descriptor_serde.rs`
- `crates/wield-core/tests/executor_pipeline.rs`

(Each file has exactly one `CommandSpec { ... }` literal with this exact
line inside it - confirmed by reading all four before writing this
plan.)

- [ ] **Step 7: Write the new `render_argv` tests**

In `crates/wield-core/tests/argv_rendering.rs` (already has a `map()`
helper building a `BTreeMap<String, ArgValue>` from `&[(&str, ArgValue)]`,
and constructs `CommandArg` via `.into()` from `&str` literals - mirror
its existing `substitutes_input_and_output_and_derived` test exactly),
add:

```rust
#[test]
fn spreads_a_paths_value_across_a_bare_placeholder() {
    let effective = map(&[(
        "input",
        ArgValue::Paths(vec![
            PathBuf::from("/docs/a.pdf"),
            PathBuf::from("/docs/b.pdf"),
            PathBuf::from("/docs/c.pdf"),
        ]),
    )]);
    let args: Vec<CommandArg> = vec!["--empty".into(), "{input}".into(), "--".into()];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(
        argv,
        vec![
            "--empty".to_string(),
            "/docs/a.pdf".into(),
            "/docs/b.pdf".into(),
            "/docs/c.pdf".into(),
            "--".into(),
        ]
    );
}

#[test]
fn input_stem_falls_back_to_the_first_path_when_input_is_a_paths_value() {
    let effective = map(&[(
        "input",
        ArgValue::Paths(vec![
            PathBuf::from("/docs/report.pdf"),
            PathBuf::from("/docs/appendix.pdf"),
        ]),
    )]);
    let args: Vec<CommandArg> = vec!["{input_stem}".into()];
    let argv = render_argv(&args, &effective, None).unwrap();
    assert_eq!(argv, vec!["report".to_string()]);
}
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p wield-core`
Expected: PASS - both new tests, plus every existing test in the crate
(confirms Step 6's ripple fix was complete and Steps 3-5 didn't change
behavior for any existing single-value case).

- [ ] **Step 9: Regenerate the `wield-tools` snapshot**

**Found during execution, not anticipated when this plan was first
written — recorded here as the correction, not left as a surprise.**
`CommandSpec` derives `Serialize`; adding `combine_inputs` in Step 1
changes the serialized shape of *every* existing tool that has a
`Capability::Command` (`image.convert`, `video.convert`, `audio.extract`,
`document.convert` - not `color.pick`, a `Portal` capability), even
though this task never touches `wield-tools` source. The committed
snapshot doesn't know about the new field yet, so
`builtin_registry_matches_snapshot` fails the moment Step 10's full gate
runs `cargo test --workspace` - this step exists so that failure doesn't
happen. (The plan originally deferred all snapshot work to Task 3; that
was a real inconsistency against this task's own "full gate green
before commit" requirement, not something to route around by skipping
the gate or committing without a passing test suite.)

```bash
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools
cargo test -p wield-tools
```

Review the diff:

```bash
git diff crates/wield-tools/tests/snapshots/builtin_registry.json
```

Expected: every existing `Command`-capability entry gains exactly one
new line, `"combine_inputs": false`, inside its own `capability.Command`
object - confirm that's the *only* change to each of those four entries,
nothing else touched.

- [ ] **Step 10: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 11: Commit**

```bash
git add crates/wield-core/src/descriptor.rs \
  crates/wield-core/src/builder.rs \
  crates/wield-core/src/template.rs \
  crates/wield-core/tests/registry.rs \
  crates/wield-core/tests/descriptor_validation.rs \
  crates/wield-core/tests/descriptor_serde.rs \
  crates/wield-core/tests/executor_pipeline.rs \
  crates/wield-core/tests/argv_rendering.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json
git commit -m "feat(core): add combine_inputs mode — spread a Paths value across one argv placeholder

New CommandSpec.combine_inputs flag; when true, a bare {name} placeholder
referencing a multiple:true File arg spreads into N argv elements (one
per selected file) instead of going through run_batch's per-file loop.
Needed for pdf.merge (M2d): qpdf --empty --pages file1 file2 file3 --
output.pdf takes every input file as its own argv element, in one
invocation, not N separate ones. Also extends input/input_stem/input_dir
resolution to fall back to the first selected file when the arg holds a
Paths value instead of a single Path, and prepares resolve()'s \"output\"
case to also serve a new \"output_dir\" token (used by Task 2).

Snapshot regenerated (combine_inputs now appears on every existing
Command-capability tool) so the full workspace gate stays green on this
commit, not just wield-core's own tests.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 2: `OutputSpec::Directory` — an unknown-until-runtime number of outputs

**Files:**
- Modify: `crates/wield-core/src/descriptor.rs`
- Modify: `crates/wield-core/src/template.rs`
- Modify: `crates/wield-core/src/validate.rs`
- Modify: `crates/wield-core/src/executor.rs`
- Test: `crates/wield-core/tests/descriptor_validation.rs`
- Test: `crates/wield-core/tests/executor_pipeline.rs`

**Interfaces:**
- Consumes: `template::resolve()`'s `"output" | "output_dir"` handling
  from Task 1 Step 4.
- Produces: `OutputSpec::Directory { name: String, dir: OutputDir }` (new
  variant), `template::resolve_output_dir(&OutputDir, &ArgMap) -> Result<PathBuf, TemplateError>`
  (new public function, extracted from `compute_output_path`), a new
  `{output_dir}` template placeholder, `Executor`'s new private
  `run_split` method (dispatched automatically for any
  `OutputSpec::Directory` descriptor - Task 5's `pdf.split` relies on
  this, but nothing about this task is `pdf.split`-specific).

- [ ] **Step 1: Add the `OutputSpec::Directory` variant**

In `crates/wield-core/src/descriptor.rs`, find:

```rust
pub enum OutputSpec {
    Value(ValueKind),
    File { name: String, dir: OutputDir },
    Report,
}
```

Replace with:

```rust
pub enum OutputSpec {
    Value(ValueKind),
    File { name: String, dir: OutputDir },
    /// The command is given a fresh, empty scratch directory (via the
    /// `{output_dir}` template placeholder) instead of one exact output
    /// path, because the real number of files it produces isn't known
    /// until it actually runs (e.g. splitting a PDF into its pages).
    /// `name` is a template (resolved the same way `File.name` is) for
    /// the *prefix* each discovered file's final name is built from -
    /// the actual per-file numbering is `Executor::run_split`'s own
    /// concern, not a template placeholder.
    Directory { name: String, dir: OutputDir },
    Report,
}
```

- [ ] **Step 2: Extract `resolve_output_dir` from `compute_output_path` — with the same `Paths` fallback Task 1 gave `resolve()`**

**Corrected during execution, before any code was written for this
step** - found by tracing Task 4's `pdf.merge` (`OutputDir::SameAsInput`
+ a `multiple: true` input, so `effective.get("input")` is
`ArgValue::Paths`, never `ArgValue::Path`) against this exact function as
originally drafted. The original draft only matched
`Some(ArgValue::Path(input))` - for `pdf.merge` that would hit the `else`
branch and return `Err(MissingInputArg)` on *every* run, before `qpdf`
ever executes. This is the same class of gap Task 1 Step 3 already fixed
in `resolve()` - `resolve_output_dir` is a *different* function (until
this step, `compute_output_path`'s own private, inline `SameAsInput`
handling) that happens to need the identical fix. Extract it correctly
the first time rather than doing a "pure" extraction now and patching it
right after.

In `crates/wield-core/src/template.rs`, find:

```rust
pub fn compute_output_path(
    output: &OutputSpec,
    effective: &ArgMap,
) -> Result<Option<PathBuf>, TemplateError> {
    let OutputSpec::File { name, dir } = output else {
        return Ok(None);
    };
    let name = render_output_name(name, effective)?;
    let directory = match dir {
        OutputDir::Fixed(path) => path.clone(),
        OutputDir::SameAsInput => {
            let Some(ArgValue::Path(input)) = effective.get("input") else {
                return Err(TemplateError::MissingInputArg);
            };
            input
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf()
        }
    };
    Ok(Some(directory.join(name)))
}
```

Replace with:

```rust
pub fn compute_output_path(
    output: &OutputSpec,
    effective: &ArgMap,
) -> Result<Option<PathBuf>, TemplateError> {
    let OutputSpec::File { name, dir } = output else {
        return Ok(None);
    };
    let name = render_output_name(name, effective)?;
    let directory = resolve_output_dir(dir, effective)?;
    Ok(Some(directory.join(name)))
}

/// Resolves an `OutputDir` to a real directory path. Shared by
/// `compute_output_path` (single-file outputs) and
/// `Executor::run_split` (`OutputSpec::Directory` outputs) - both need
/// "where does the result live", just with something different joined
/// onto it afterward. `SameAsInput` accepts a `Paths` value the same
/// way `resolve()`'s `input`/`input_stem`/`input_dir` handling already
/// does (Task 1) - falls back to the first selected file. Needed for
/// `pdf.merge` (Task 4): its `input` is `multiple: true`, so
/// `effective.get("input")` is always `ArgValue::Paths`, never a bare
/// `Path` - without this fallback, `compute_output_path` would fail
/// with `MissingInputArg` on every single merge.
pub fn resolve_output_dir(dir: &OutputDir, effective: &ArgMap) -> Result<PathBuf, TemplateError> {
    match dir {
        OutputDir::Fixed(path) => Ok(path.clone()),
        OutputDir::SameAsInput => {
            let input = match effective.get("input") {
                Some(ArgValue::Path(input)) => input,
                Some(ArgValue::Paths(paths)) if !paths.is_empty() => &paths[0],
                _ => return Err(TemplateError::MissingInputArg),
            };
            Ok(input
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf())
        }
    }
}
```

Behavior-preserving for every *existing* descriptor: none of them ever
reach `compute_output_path` with a raw `Paths` value for "input" in the
first place (`run_batch` always substitutes a single `Path` per
iteration before `compute_output_path` runs) - the new match arm is
inert for `image.convert`/`video.convert`/`audio.extract`/
`document.convert`/`pdf.compress`, and only matters for `pdf.merge`
(Task 4), the one descriptor with both `combine_inputs: true` and
`OutputDir::SameAsInput`.

- [ ] **Step 3: Write a test for the `Paths` fallback**

In `crates/wield-core/tests/argv_rendering.rs` (already imports
`render_output_name`; add `compute_output_path` and `OutputDir`,
`OutputSpec` to the existing `wield_core::template`/
`wield_core::descriptor` import lines), add:

```rust
#[test]
fn compute_output_path_same_as_input_falls_back_to_the_first_path() {
    let effective = map(&[(
        "input",
        ArgValue::Paths(vec![
            PathBuf::from("/docs/a.pdf"),
            PathBuf::from("/docs/b.pdf"),
        ]),
    )]);
    let output = OutputSpec::File {
        name: "{input_stem}-merged.pdf".into(),
        dir: OutputDir::SameAsInput,
    };
    let path = compute_output_path(&output, &effective).unwrap();
    assert_eq!(path, Some(PathBuf::from("/docs/a-merged.pdf")));
}
```

- [ ] **Step 4: Run `template.rs`'s tests to confirm the refactor is behavior-preserving and the new fallback works**

Run: `cargo test -p wield-core --test argv_rendering`
Expected: PASS, no change from before Step 2.

- [ ] **Step 5: Validate `OutputSpec::Directory`**

In `crates/wield-core/src/validate.rs`, find:

```rust
fn check_capability_and_output(descriptor: &Descriptor, errors: &mut Vec<DescriptorError>) {
    match (&descriptor.capability, &descriptor.output) {
        (Capability::Command(command), OutputSpec::File { name, .. }) => {
            check_command_templates(descriptor, command, name, errors);
        }
        (Capability::Command(_), _) => error(
            errors,
            "output",
            "Command capability requires File output in P2",
        ),
        (_, OutputSpec::File { .. }) => error(
            errors,
            "output",
            "File output requires Command capability in P2",
        ),
        _ => {}
    }
}
```

Replace with:

```rust
fn check_capability_and_output(descriptor: &Descriptor, errors: &mut Vec<DescriptorError>) {
    match (&descriptor.capability, &descriptor.output) {
        (Capability::Command(command), OutputSpec::File { name, .. }) => {
            check_command_templates(descriptor, command, name, errors);
        }
        (Capability::Command(command), OutputSpec::Directory { name, .. }) => {
            check_command_templates(descriptor, command, name, errors);
        }
        (Capability::Command(_), _) => error(
            errors,
            "output",
            "Command capability requires File or Directory output in P2",
        ),
        (_, OutputSpec::File { .. } | OutputSpec::Directory { .. }) => error(
            errors,
            "output",
            "File or Directory output requires Command capability in P2",
        ),
        _ => {}
    }
}
```

(Order matters - the two specific arms must come before the two
catch-alls, since Rust matches top to bottom and the catch-alls would
otherwise shadow them.)

Still in `validate.rs`, find `check_template`'s handling of the
`"output"` token:

```rust
        if token == "output" {
            if !allow_output {
                error(
                    errors,
                    &at,
                    "output placeholder is not allowed in output name",
                );
            }
            continue;
        }
```

Change the condition to also accept `"output_dir"`:

```rust
        if matches!(token.as_str(), "output" | "output_dir") {
            if !allow_output {
                error(
                    errors,
                    &at,
                    "output placeholder is not allowed in output name",
                );
            }
            continue;
        }
```

- [ ] **Step 6: Write the validation test**

In `crates/wield-core/tests/descriptor_validation.rs`, find the existing
`base_command_descriptor()` helper (constructs a `Descriptor` with a
single `File { multiple: false, .. }` "input" arg, `Requires::Binary`,
`OutputSpec::File { name: "{input_stem}.out", dir: OutputDir::SameAsInput }`,
and a `Capability::Command` with args `["{input}".into(), "{output}".into()]`
- read it directly for the exact current field values before writing
this test, since Task 1 Step 6 already added `combine_inputs: false,` to
its `CommandSpec` literal). Add:

```rust
#[test]
fn accepts_a_directory_output_descriptor() {
    let mut descriptor = base_command_descriptor();
    descriptor.output = OutputSpec::Directory {
        name: "{input_stem}".into(),
        dir: OutputDir::SameAsInput,
    };
    if let Capability::Command(command) = &mut descriptor.capability {
        command.args = vec!["{input}".into(), "{output_dir}".into()];
    }
    assert!(validate_descriptor(&descriptor).is_ok());
}
```

- [ ] **Step 7: Run the validation test**

Run: `cargo test -p wield-core --test descriptor_validation`
Expected: PASS.

- [ ] **Step 8: Add `run_split` to the executor**

In `crates/wield-core/src/executor.rs`, find the import lines at the top:

```rust
use crate::args::{validate_args, ArgMap, ArgValue};
use crate::command::{BinaryResolver, CommandResult, CommandRunner, OutputPlan, RunSpec};
use crate::descriptor::{Capability, Descriptor, Requires};
use crate::outcome::{hint_for_stderr, Progress, Stage, ToolOutcome};
use crate::template::{compute_output_path, render_argv};
use crate::validate::validate_descriptor;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
```

Replace with:

```rust
use crate::args::{validate_args, ArgMap, ArgValue};
use crate::command::{BinaryResolver, CommandResult, CommandRunner, OutputPlan, RunSpec};
use crate::descriptor::{Capability, Descriptor, OutputSpec, Requires};
use crate::outcome::{hint_for_stderr, Progress, Stage, ToolOutcome};
use crate::template::{compute_output_path, render_argv, render_output_name, resolve_output_dir};
use crate::validate::validate_descriptor;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;
```

Find `run_command`'s current opening (the `batch_paths` dispatch check):

```rust
    async fn run_command(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        if let Some((arg_name, paths)) = batch_paths(effective) {
            return self
                .run_batch(
                    descriptor, command, effective, arg_name, &paths, progress, cancel,
                )
                .await;
        }
```

Replace with:

```rust
    async fn run_command(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        if let OutputSpec::Directory { name, dir } = &descriptor.output {
            return self
                .run_split(descriptor, command, effective, name, dir, progress, cancel)
                .await;
        }

        if !command.combine_inputs {
            if let Some((arg_name, paths)) = batch_paths(effective) {
                return self
                    .run_batch(
                        descriptor, command, effective, arg_name, &paths, progress, cancel,
                    )
                    .await;
            }
        }
```

Add the new method. Place it right after `run_batch` (which ends with
`ToolOutcome::Report { title: ..., lines }` followed by a closing `}` -
insert `run_split` right after that closing brace, still inside `impl
Executor`):

```rust
    #[allow(clippy::too_many_arguments)]
    async fn run_split(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        name_template: &str,
        dir: &crate::descriptor::OutputDir,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        let _ = descriptor;
        let destination = match resolve_output_dir(dir, effective) {
            Ok(path) => path,
            Err(error) => return failed(Stage::Output, error.to_string()),
        };
        let prefix = match render_output_name(name_template, effective) {
            Ok(name) => name,
            Err(error) => return failed(Stage::Output, error.to_string()),
        };
        let scratch = destination.join(format!(".wield-tmp-{}-split", Uuid::new_v4()));
        if let Err(error) = std::fs::create_dir_all(&scratch) {
            return failed(Stage::Output, error.to_string());
        }

        let argv = match render_argv(&command.args, effective, Some(&scratch)) {
            Ok(argv) => argv,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&scratch);
                return failed(Stage::Command, error.to_string());
            }
        };
        let Some(binary) = self.resolver.resolve(&command.binary) else {
            let _ = std::fs::remove_dir_all(&scratch);
            return unavailable_binary(&command.binary);
        };
        let cwd = match effective.get("input") {
            Some(ArgValue::Path(input)) => input.parent().map(ToOwned::to_owned),
            _ => None,
        };

        let result = CommandRunner::execute(RunSpec {
            binary: &binary,
            argv: &argv,
            cwd: cwd.as_deref(),
            output: None,
            progress_spec: &command.progress,
            success: &command.success,
            timeout: command.timeout,
            progress,
            cancel,
        })
        .await;

        match result {
            CommandResult::Success { .. } => {
                let mut entries: Vec<PathBuf> = match std::fs::read_dir(&scratch) {
                    Ok(entries) => entries
                        .filter_map(|entry| entry.ok().map(|e| e.path()))
                        .collect(),
                    Err(error) => {
                        let _ = std::fs::remove_dir_all(&scratch);
                        return failed(Stage::Output, error.to_string());
                    }
                };
                entries.sort();
                if entries.is_empty() {
                    let _ = std::fs::remove_dir_all(&scratch);
                    return failed(Stage::Output, "command produced no output files");
                }
                let mut lines = Vec::new();
                for (index, source) in entries.iter().enumerate() {
                    let final_path = destination.join(format!("{prefix}-{}.pdf", index + 1));
                    match std::fs::rename(source, &final_path) {
                        Ok(()) => lines.push(format!(
                            "\u{2713} page {} \u{2192} {}",
                            index + 1,
                            final_path.display()
                        )),
                        Err(error) => lines.push(format!("\u{2717} page {}: {error}", index + 1)),
                    }
                }
                let _ = std::fs::remove_dir_all(&scratch);
                ToolOutcome::Report {
                    title: format!("Split into {} files", lines.len()),
                    lines,
                }
            }
            CommandResult::NonZeroExit { code, stderr_tail } => {
                let _ = std::fs::remove_dir_all(&scratch);
                ToolOutcome::Failed {
                    stage: Stage::Command,
                    detail: format!(
                        "exited with {}",
                        code.map(|v| v.to_string())
                            .unwrap_or_else(|| "signal".to_owned())
                    ),
                    hint: hint_for_stderr(&stderr_tail),
                }
            }
            CommandResult::Timeout => {
                let _ = std::fs::remove_dir_all(&scratch);
                failed(
                    Stage::Command,
                    format!("timed out after {:?}", command.timeout),
                )
            }
            CommandResult::Cancelled => {
                let _ = std::fs::remove_dir_all(&scratch);
                ToolOutcome::Cancelled
            }
            CommandResult::SpawnFailed { detail } => {
                let _ = std::fs::remove_dir_all(&scratch);
                ToolOutcome::Failed {
                    stage: Stage::Command,
                    detail,
                    hint: Some(format!("could not start {}", command.binary)),
                }
            }
            CommandResult::OutputMissing => {
                // Unreachable in practice: finish_output only ever
                // produces OutputMissing when passed Some(plan);
                // run_split always passes output: None, so
                // CommandRunner::execute short-circuits to
                // Success{output_file: None} on the success path
                // instead. Handled anyway for exhaustiveness.
                let _ = std::fs::remove_dir_all(&scratch);
                failed(Stage::Output, "expected output file was not created")
            }
        }
    }
```

The `let _ = descriptor;` line silences a possible unused-parameter
warning - `descriptor` is kept in the signature for consistency with
`run_batch`'s shape (which also takes it), even though this method
doesn't currently need it. After Step 9's clippy run, check whether
clippy actually warns about it: if the gate is clean *without* that
line, delete it (an unused `fn` parameter isn't always warned on the way
an unused local binding is) - don't leave a no-op line if it isn't
needed, but don't guess either; check the real clippy output.

- [ ] **Step 9: Write the `run_split` integration tests**

In `crates/wield-core/tests/executor_pipeline.rs`, read the file's
existing `write_stub_script`/`convert_descriptor` helper conventions
directly before writing this (the exact helper signatures are already
established there from earlier M2 work - use them as given, don't
reinvent). Add a descriptor fixture and two tests:

```rust
fn split_descriptor(binary: &str) -> Descriptor {
    let mut descriptor = convert_descriptor(binary);
    descriptor.output = OutputSpec::Directory {
        name: "{input_stem}".into(),
        dir: OutputDir::SameAsInput,
    };
    if let Capability::Command(command) = &mut descriptor.capability {
        command.args = vec!["{output_dir}".into()];
    }
    descriptor
}

#[tokio::test]
async fn split_discovers_and_renames_every_produced_file() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(
        dir.path(),
        "split-stub",
        "#!/bin/sh\nmkdir -p \"$1\"\necho a > \"$1/x1\"\necho b > \"$1/x2\"\n",
    );
    let input = dir.path().join("report.raw");
    std::fs::write(&input, b"whatever").unwrap();

    let descriptor = split_descriptor("split-stub");
    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;

    match outcome {
        ToolOutcome::Report { title, lines } => {
            assert_eq!(title, "Split into 2 files");
            assert_eq!(lines.len(), 2);
        }
        other => panic!("expected Report, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(dir.path().join("report-1.pdf")).unwrap(),
        b"a\n"
    );
    assert_eq!(
        std::fs::read(dir.path().join("report-2.pdf")).unwrap(),
        b"b\n"
    );
    // The scratch directory must not survive a successful run.
    let leftover = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .any(|entry| entry.file_name().to_string_lossy().starts_with(".wield-tmp-"));
    assert!(!leftover, "scratch directory was not cleaned up");
}

#[tokio::test]
async fn split_producing_zero_files_is_a_failure_not_an_empty_report() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "empty-split-stub", "#!/bin/sh\nmkdir -p \"$1\"\n");
    let input = dir.path().join("report.raw");
    std::fs::write(&input, b"whatever").unwrap();

    let descriptor = split_descriptor("empty-split-stub");
    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path(input));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;

    assert!(
        matches!(outcome, ToolOutcome::Failed { .. }),
        "expected Failed, got {outcome:?}"
    );
    let leftover = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .any(|entry| entry.file_name().to_string_lossy().starts_with(".wield-tmp-"));
    assert!(!leftover, "scratch directory was not cleaned up");
}
```

The file's existing imports already cover everything used above -
confirmed directly: it has `use wield_core::descriptor::*;` (a wildcard),
so `OutputSpec`, `OutputDir`, `Capability`, and everything else from
`descriptor.rs` are already in scope, alongside the already-imported
`ArgValue`, `BTreeMap`, `Executor`, `BinaryResolver`, `ExecutionRequest`,
`ToolOutcome`, `CancellationToken`, `mpsc`. No import changes needed.

- [ ] **Step 10: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 11: Commit**

```bash
git add crates/wield-core/src/descriptor.rs \
  crates/wield-core/src/template.rs \
  crates/wield-core/src/validate.rs \
  crates/wield-core/src/executor.rs \
  crates/wield-core/tests/argv_rendering.rs \
  crates/wield-core/tests/descriptor_validation.rs \
  crates/wield-core/tests/executor_pipeline.rs
git commit -m "feat(core): add OutputSpec::Directory — an unknown-until-runtime file count

New variant for commands that produce a number of output files only
known once they've actually run (e.g. splitting a PDF into its pages).
Zero command.rs changes: CommandRunner::execute's output: Option<&OutputPlan>
already has a None branch that skips the single-file temp-then-rename
logic entirely and returns Success{output_file: None} - exactly what
this needs. All new logic lives in executor.rs's new run_split method,
the same place run_batch already lives: a fresh scratch directory
({output_dir}, resolved the same way {output} already is), discover
whatever files appeared after a successful run, rename each into its
final destination, report via the existing ToolOutcome::Report - zero
new frontend work, the same card M2a's batch summaries already render.

Also fixes a real gap found while extracting resolve_output_dir out of
compute_output_path: its SameAsInput branch only matched a single Path,
which would have made pdf.merge (Task 4, multiple:true input) fail with
MissingInputArg on every run - now falls back to the first selected
file, mirroring the same fix Task 1 already made to resolve()'s own
input/input_stem/input_dir handling.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 3: `pdf.compress` descriptor

**Files:**
- Create: `crates/wield-tools/src/pdf_compress.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated)
- Modify: `apps/wield/src-tauri/src/commands.rs` (mechanical ripple)
- Modify: `apps/wield/src-tauri/tests/commands.rs` (mechanical ripple)

**Interfaces:**
- Consumes: nothing new from Tasks 1-2 - this tool needs neither
  mechanism, it's the same one-input-one-output shape every prior M2
  tool has used.
- Produces: `pdf_compress::descriptor() -> Descriptor`, registered as
  `"pdf.compress"`.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/pdf_compress.rs`:

```rust
//! The `pdf.compress` built-in — shrink a PDF's file size via Ghostscript.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const QUALITY_PRESETS: &[&str] = &["screen", "ebook", "printer"];

/// Descriptor for `pdf.compress`. Runs `gs -sDEVICE=pdfwrite
/// -dCompatibilityLevel=1.4 -dPDFSETTINGS=/<preset> -dNOPAUSE -dBATCH
/// -sOutputFile=<output> <input>`. `quality` is one of Ghostscript's
/// three standard presets (`screen` smallest/lowest quality, `ebook`
/// medium, `printer` largest/highest quality) - verified live that all
/// three work; a real Enum of presets rather than a raw numeric range,
/// same reasoning as `audio.extract`'s bitrate choice. The output name
/// appends `-compressed` rather than reusing `{input_stem}.{format}` -
/// unlike every format-conversion tool so far, compress doesn't change
/// the extension, so there's nothing to naturally distinguish the
/// output from the source; overwriting the source PDF in place would be
/// a real, surprising data-loss risk for a "shrink my file" tool.
/// `ghostscript` is the package name; the binary itself is `gs`.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.compress", "Compress PDF", Category::Convert)
        .keywords(&["pdf", "compress", "shrink", "ghostscript", "size"])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "PDF",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "PDF".into(),
                        extensions: vec!["pdf".into()],
                    }],
                    multiple: false,
                },
            )
            .required(true)
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Quality",
                ArgType::Enum {
                    options: QUALITY_PRESETS.iter().map(|q| (*q).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("ebook".into()))
            .help("screen: smallest, lowest quality. ebook: balanced. printer: largest, best quality.")
            .build(),
        )
        .requires(Requires::Binary("gs".into()))
        .output(OutputSpec::File {
            name: "{input_stem}-compressed.pdf".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("gs")
                .arg("-sDEVICE=pdfwrite")
                .arg("-dCompatibilityLevel=1.4")
                .arg("-dPDFSETTINGS=/{quality}")
                .arg("-dNOPAUSE")
                .arg("-dBATCH")
                .arg("-sOutputFile={output}")
                .arg("{input}")
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.compress descriptor is valid")
}
```

- [ ] **Step 2: Register in the registry**

In `crates/wield-tools/src/lib.rs`, the module declarations currently
read (after Task 1/2, which don't touch this file):

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod video_convert;
```

Replace with (alphabetical, `pdf_compress` sorts between `image_convert`
and `video_convert`):

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod pdf_compress;
pub mod video_convert;
```

And in `builtin_registry()`, add the registration call between
`image_convert` and `video_convert`:

```rust
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
        .register(pdf_compress::descriptor())
        .expect("pdf.compress registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
```

- [ ] **Step 3: Write the render_argv test**

In `crates/wield-tools/tests/builtin_registry.rs`, add:

```rust
#[test]
fn pdf_compress_renders_the_ghostscript_argv() {
    let tool = builtin_registry().get("pdf.compress").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.pdf".into()));
    args.insert("quality".to_string(), ArgValue::Str("printer".into()));
    let out = std::path::PathBuf::from("/docs/a-compressed.pdf");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-sDEVICE=pdfwrite".to_string(),
            "-dCompatibilityLevel=1.4".into(),
            "-dPDFSETTINGS=/printer".into(),
            "-dNOPAUSE".into(),
            "-dBATCH".into(),
            "-sOutputFile=/docs/a-compressed.pdf".into(),
            "/docs/a.pdf".into(),
        ],
    );
}
```

Update `registry_has_exactly_the_expected_builtins` (currently `vec!["audio.extract",
"color.pick", "document.convert", "image.convert", "video.convert"]`) to:

```rust
        vec![
            "audio.extract",
            "color.pick",
            "document.convert",
            "image.convert",
            "pdf.compress",
            "video.convert"
        ]
```

- [ ] **Step 4: Regenerate the snapshot**

```bash
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools
cargo test -p wield-tools
```

- [ ] **Step 5: Review the snapshot diff**

```bash
git diff crates/wield-tools/tests/snapshots/builtin_registry.json
```

Expected: a pure insertion of one new `pdf.compress` object - Task 1's
own Step 9 already regenerated this snapshot to add `"combine_inputs": false`
to every existing `Command`-capability entry, so that ripple is already
settled by the time this task runs. Confirm every pre-existing entry
(`image.convert`, `video.convert`, `audio.extract`, `document.convert`,
and now their `combine_inputs` field too) is byte-for-byte unchanged in
this diff - only the new `pdf.compress` object should appear.

- [ ] **Step 6: Fix the mechanical ripple in the app crate**

In `apps/wield/src-tauri/src/commands.rs`, find
`list_tools_with_no_query_returns_registration_order`'s expected vector
and add `"pdf.compress"` in the same alphabetical position used above.

In `apps/wield/src-tauri/tests/commands.rs`, change both `assert_eq!(...len(), 5)`
lines to `6`.

- [ ] **Step 7: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 8: Commit**

```bash
git add crates/wield-tools/src/pdf_compress.rs \
  crates/wield-tools/src/lib.rs \
  crates/wield-tools/tests/builtin_registry.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json \
  apps/wield/src-tauri/src/commands.rs \
  apps/wield/src-tauri/tests/commands.rs
git commit -m "feat(tools): add pdf.compress (ghostscript)

Same shape as every prior M2 tool - one input, one output, no new
wield-core mechanism. Output name appends -compressed rather than
reusing {input_stem}.{format}, since compress doesn't change the
extension and there's nothing else to distinguish the output from the
source PDF.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 4: `pdf.merge` descriptor (depends on Task 1)

**Files:**
- Create: `crates/wield-tools/src/pdf_merge.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated)
- Modify: `apps/wield/src-tauri/src/commands.rs` (mechanical ripple)
- Modify: `apps/wield/src-tauri/tests/commands.rs` (mechanical ripple)

**Interfaces:**
- Consumes: `CommandSpecBuilder::combine_inputs(bool)` and the
  `render_argv` spread behavior, both from Task 1.
- Produces: `pdf_merge::descriptor() -> Descriptor`, registered as
  `"pdf.merge"`.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/pdf_merge.rs`:

```rust
//! The `pdf.merge` built-in — combine several PDFs into one via qpdf.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, Category, CommandSpecBuilder, Descriptor, DescriptorBuilder,
    FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

/// Descriptor for `pdf.merge`. Runs `qpdf --empty --pages <file1>
/// <file2> ... -- <output>` - `{input}` is a bare placeholder over a
/// `multiple: true` arg, so `combine_inputs(true)` makes it spread into
/// one argv element per selected file (see
/// `crates/wield-core/src/template.rs::render_argv`'s spread branch),
/// in one `qpdf` invocation, rather than `run_batch`'s usual per-file
/// loop. The `--` before `{output}` is required - verified live that
/// omitting it makes `qpdf` try to parse the output filename as a page
/// range and fail outright. There's no established Wield convention for
/// letting a user name a combined output, so the result is named after
/// the *first* selected file (`{input_stem}` now falls back to the
/// first path in a `Paths` value, from Task 1) with a `-merged` suffix.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.merge", "Merge PDFs", Category::Convert)
        .keywords(&["pdf", "merge", "combine", "join", "qpdf"])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "PDFs",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "PDF".into(),
                        extensions: vec!["pdf".into()],
                    }],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .requires(Requires::Binary("qpdf".into()))
        .output(OutputSpec::File {
            name: "{input_stem}-merged.pdf".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("qpdf")
                .arg("--empty")
                .arg("--pages")
                .arg("{input}")
                .arg("--")
                .arg("{output}")
                .combine_inputs(true)
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.merge descriptor is valid")
}
```

- [ ] **Step 2: Register in the registry**

In `crates/wield-tools/src/lib.rs`, `pdf_merge` sorts alphabetically
right before `pdf_compress`... check carefully: `pdf_compress` < `pdf_merge`
< `pdf_split` lexicographically (`c` < `m` < `s`), so the final module
order across this whole plan will be `pdf_compress, pdf_merge, pdf_split`.
Right now (after Task 3), only `pdf_compress` exists; add `pdf_merge`
directly after it:

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod pdf_compress;
pub mod pdf_merge;
pub mod video_convert;
```

Register directly after `pdf_compress`'s registration call in
`builtin_registry()`:

```rust
    registry
        .register(pdf_compress::descriptor())
        .expect("pdf.compress registers");
    registry
        .register(pdf_merge::descriptor())
        .expect("pdf.merge registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
```

- [ ] **Step 3: Write the render_argv test**

In `crates/wield-tools/tests/builtin_registry.rs`, add:

```rust
#[test]
fn pdf_merge_spreads_every_selected_file_before_the_separator() {
    let tool = builtin_registry().get("pdf.merge").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert(
        "input".to_string(),
        ArgValue::Paths(vec![
            "/docs/a.pdf".into(),
            "/docs/b.pdf".into(),
            "/docs/c.pdf".into(),
        ]),
    );
    let out = std::path::PathBuf::from("/docs/a-merged.pdf");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "--empty".to_string(),
            "--pages".into(),
            "/docs/a.pdf".into(),
            "/docs/b.pdf".into(),
            "/docs/c.pdf".into(),
            "--".into(),
            "/docs/a-merged.pdf".into(),
        ],
    );
}
```

Update `registry_has_exactly_the_expected_builtins`'s expected vector to
insert `"pdf.merge"` right after `"pdf.compress"`.

- [ ] **Step 4: Regenerate the snapshot, review the diff**

```bash
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools
cargo test -p wield-tools
git diff crates/wield-tools/tests/snapshots/builtin_registry.json
```

Expected: a pure insertion (`combine_inputs` has been on every entry
since Task 1's own snapshot regeneration, so there's no repeat of that
ripple here) - confirm only the new `pdf.merge` object was added, and
that its own `"combine_inputs": true` is present (this is the one entry
where that field is genuinely `true`, not the default `false` every
other tool has).

- [ ] **Step 5: Fix the mechanical ripple**

Same two files as every prior task - `apps/wield/src-tauri/src/commands.rs`'s
registration-order vector gets `"pdf.merge"` inserted in the right spot;
`apps/wield/src-tauri/tests/commands.rs`'s two count assertions go
`6` → `7`.

- [ ] **Step 6: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 7: Commit**

```bash
git add crates/wield-tools/src/pdf_merge.rs \
  crates/wield-tools/src/lib.rs \
  crates/wield-tools/tests/builtin_registry.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json \
  apps/wield/src-tauri/src/commands.rs \
  apps/wield/src-tauri/tests/commands.rs
git commit -m "feat(tools): add pdf.merge (qpdf, combine_inputs)

First real user of Task 1's combine_inputs mechanism - qpdf --empty
--pages file1 file2 file3 -- output.pdf takes every selected file as
its own argv element in one invocation, verified live including the
required -- separator (qpdf misparses the output path as a page range
without it).

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 5: `pdf.split` descriptor (depends on Task 2)

**Files:**
- Create: `crates/wield-tools/src/pdf_split.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated)
- Modify: `apps/wield/src-tauri/src/commands.rs` (mechanical ripple)
- Modify: `apps/wield/src-tauri/tests/commands.rs` (mechanical ripple)

**Interfaces:**
- Consumes: `OutputSpec::Directory` and the `{output_dir}` placeholder,
  both from Task 2.
- Produces: `pdf_split::descriptor() -> Descriptor`, registered as
  `"pdf.split"`.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/pdf_split.rs`:

```rust
//! The `pdf.split` built-in — one PDF's pages become one file each, via qpdf.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, Category, CommandSpecBuilder, Descriptor, DescriptorBuilder,
    FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

/// Descriptor for `pdf.split`. Runs `qpdf --split-pages <input>
/// <output_dir>/page-%d.pdf` - the number of files this produces isn't
/// known until it actually runs (it depends on the input's page count),
/// so this uses `OutputSpec::Directory` rather than `OutputSpec::File`:
/// `{output_dir}` resolves to a fresh, empty scratch directory instead
/// of one exact path, and `Executor::run_split` discovers whatever
/// files appeared afterward, renaming each to `{input_stem}-N.pdf` next
/// to the source. `qpdf`'s own `%d` is plain `printf`-style numbering,
/// not a Wield placeholder - it has no braces, so the template engine's
/// brace-scanning never touches it. The `{output_dir}/page-%d.pdf`
/// segment is NOT a bare placeholder (there's a suffix beyond just
/// `{output_dir}`), so it resolves through the normal single-value path
/// `resolve()` already handles for `"output_dir"`, not the spread path.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.split", "Split PDF", Category::Convert)
        .keywords(&["pdf", "split", "pages", "extract", "qpdf"])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "PDF",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "PDF".into(),
                        extensions: vec!["pdf".into()],
                    }],
                    multiple: false,
                },
            )
            .required(true)
            .build(),
        )
        .requires(Requires::Binary("qpdf".into()))
        .output(OutputSpec::Directory {
            name: "{input_stem}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("qpdf")
                .arg("--split-pages")
                .arg("{input}")
                .arg("{output_dir}/page-%d.pdf")
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.split descriptor is valid")
}
```

- [ ] **Step 2: Register in the registry**

In `crates/wield-tools/src/lib.rs`, add `pdf_split` right after
`pdf_merge` (alphabetical: `pdf_compress, pdf_merge, pdf_split`):

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod pdf_compress;
pub mod pdf_merge;
pub mod pdf_split;
pub mod video_convert;
```

Register right after `pdf_merge`'s call in `builtin_registry()`:

```rust
    registry
        .register(pdf_merge::descriptor())
        .expect("pdf.merge registers");
    registry
        .register(pdf_split::descriptor())
        .expect("pdf.split registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
```

- [ ] **Step 3: Write the render_argv test**

In `crates/wield-tools/tests/builtin_registry.rs`, add:

```rust
#[test]
fn pdf_split_renders_the_output_dir_pattern() {
    let tool = builtin_registry().get("pdf.split").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/report.pdf".into()));
    let scratch = std::path::PathBuf::from("/tmp/.wield-tmp-xyz-split");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&scratch)).unwrap(),
        vec![
            "--split-pages".to_string(),
            "/docs/report.pdf".into(),
            "/tmp/.wield-tmp-xyz-split/page-%d.pdf".into(),
        ],
    );
}
```

Update `registry_has_exactly_the_expected_builtins`'s expected vector to
insert `"pdf.split"` right after `"pdf.merge"`. This is the plan's final
tool addition, so the complete vector is now:

```rust
        vec![
            "audio.extract",
            "color.pick",
            "document.convert",
            "image.convert",
            "pdf.compress",
            "pdf.merge",
            "pdf.split",
            "video.convert"
        ]
```

- [ ] **Step 4: Regenerate the snapshot, review the diff**

```bash
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools
cargo test -p wield-tools
git diff crates/wield-tools/tests/snapshots/builtin_registry.json
```

Confirm only the new `pdf.split` object was added, and that its
`"output"` field serializes as the new `Directory` variant shape (not
`File`) - a quick visual check that the JSON actually says `"Directory"`
somewhere in that object, not a silent fallback to `File`.

- [ ] **Step 5: Fix the mechanical ripple**

`apps/wield/src-tauri/src/commands.rs`'s registration-order vector gets
`"pdf.split"` in the right spot; `apps/wield/src-tauri/tests/commands.rs`'s
two count assertions go `7` → `8` - this is the plan's final tool count.

- [ ] **Step 6: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 7: Commit**

```bash
git add crates/wield-tools/src/pdf_split.rs \
  crates/wield-tools/src/lib.rs \
  crates/wield-tools/tests/builtin_registry.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json \
  apps/wield/src-tauri/src/commands.rs \
  apps/wield/src-tauri/tests/commands.rs
git commit -m "feat(tools): add pdf.split (qpdf, OutputSpec::Directory)

First real user of Task 2's OutputSpec::Directory mechanism. qpdf's own
%d page-numbering has no braces, so it passes through Wield's
placeholder scanner untouched; {output_dir}/page-%d.pdf is a
non-bare placeholder (a suffix beyond just {output_dir}), so it
resolves through the normal single-value path, not the spread path -
the two new M2d mechanisms (Tasks 1 and 2) are independent and don't
interact within this one descriptor.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 6: Live verification and `docs/testing.md`/`docs/backlog.md` write-up

**No subagent dispatch for this task** — matches every prior M2 tool's
final task: this needs live desktop tooling only the orchestrator
session driving this plan has access to.

**Files:**
- Modify: `docs/testing.md`
- Modify: `docs/backlog.md`

**Interfaces:**
- Consumes: the real, installed `pdf.compress`/`pdf.merge`/`pdf.split`
  tools via the real desktop UI — everything built in Tasks 1-5.
- Produces: nothing further in this plan consumes this task's output;
  it's the plan's final deliverable.

- [ ] **Step 1: Build and install a fresh binary**

```bash
npm run build -w apps/wield -- --no-bundle
```

- [ ] **Step 2: `pdf.compress`**

Search "pdf" or "compress" in the palette, select `Compress PDF`, pick a
real multi-page test PDF, run with a couple of different `quality`
presets across separate runs. Confirm each result is a valid PDF
(`qpdf --show-npages` confirms the same page count as the source - a
genuine re-encode preserves page count even though it changes file
size/content encoding) and compare file sizes across presets
independently, not just trusting the result card.

- [ ] **Step 3: `pdf.merge`**

Select several real PDFs at once through the native multi-select picker,
run the merge. Confirm the result is `ToolOutcome::File`, and
independently confirm via `qpdf --show-npages` that the combined page
count equals the sum of every selected file's own page count, and (using
`qpdf --show-npages` on each individual page or another independent
check) that page order matches selection order.

- [ ] **Step 4: `pdf.split`**

Split a real multi-page PDF. Confirm the result is `ToolOutcome::Report`
with one line per page, that each produced file independently reports
exactly 1 page via `qpdf --show-npages`, and — since this exercises
Task 2's new directory-discovery-and-cleanup logic for real, not just in
an automated test — confirm no `.wield-tmp-*-split` directory or other
residue was left behind (list the destination directory before and
after, same verification method M2c's `soffice` wrapper cleanup used).

- [ ] **Step 5: Write up `docs/testing.md`**

Add a new section following the exact pattern of the existing M2a/b/c
sections (read at least one directly first for tone/structure). Cover
all three tools' live results and the `pdf.split` cleanup confirmation
specifically, since that's the one exercising genuinely new mechanism
code, not just a new descriptor on established rails.

- [ ] **Step 6: Update `docs/backlog.md`**

Add two items (find an appropriate existing section, e.g. under a
"Converter tooling (M2)" heading if one already exists from M2a/b/c's
own backlog entries, or create one):

```markdown
- **`pdf.split`'s discovered-file order is alphabetical, not numeric.**
  At 10+ pages, `qpdf`'s own `%d`-numbered output (`page-1.pdf`,
  `page-10.pdf`, `page-2.pdf`, ...) sorts lexicographically wrong before
  `Executor::run_split` renumbers them - each split file's *content* is
  still correct (exactly one real page each), but which page ends up
  labeled `-2` vs `-10` in the final filename could be off for larger
  documents. Not fixed for M2d: a numeric-aware sort is a small,
  legitimate follow-up, not core-mechanism work.
- **`pdf.compress` has no batch support**, unlike every other M2
  converter tool. It could reasonably take `multiple: true` (independent
  per-file compression, exactly `run_batch`'s existing shape) - left out
  of M2d's scope deliberately, to keep this milestone's real
  scope (two new wield-core mechanisms) from also absorbing a third,
  unrelated addition.
```

- [ ] **Step 7: Full gate one more time**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 8: Commit**

```bash
git add docs/testing.md docs/backlog.md
git commit -m "docs: verify pdf.compress/merge/split live, track deferred gaps

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

# Wield M1 · Plan P4 — wield-tools built-ins & the wield-cli one-shot — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the first two real Wield tools — `color.pick` (Portal) and `image.convert` (Command) — as validated built-in descriptors in `wield-tools`, guarded by a registry snapshot test, and build the `wield <tool> <args>` one-shot CLI that generates its argument surface from those descriptors and runs them through the executor.

**Architecture:** `wield-tools` exposes `builtin_registry() -> Registry` — each built-in is a `Descriptor` constructed with the P2 builders and validated on build. `wield-cli` resolves a tool id against that registry, turns the descriptor's `ArgSpec` list into a CLI surface (first required `file`/`dir` arg positional, everything else `--flag`), coerces tokens to `ArgValue`, and calls `Executor` (with `BinaryResolver`, a `PortalAdapterRunner`, and a live `probe()` result). It renders the single `ToolOutcome` to stdout/stderr with a meaningful exit code. One `wield-core` refinement is needed: `When` gains a "present" form (`in: []`) so a descriptor can drop a paired option flag (`-resize` + `{width}x`) together when its optional arg is unset — without a bool gate.

**Tech Stack:** Rust 2021, Tokio (`wield-cli` async entry), `wield-core` + `wield-portal` + `wield-tools` (all path deps), `serde_json` (Color payload / snapshot). Real `magick` (ImageMagick) for the `image.convert` integration test.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` — implements §3 (`wield-tools`, `wield-cli`), §4 (built-ins via type-safe builders; registry snapshot test), §6 (`color.pick`, `image.convert`), and §7 (outcome rendering).

---

## GEON amendment — 2026-09-10 (owner design decisions)

Locked with the owner before this plan was written:

1. **CLI arg mapping:** the first **required** `file`/`dir` `ArgSpec` is a positional argument; every other arg is `--name value` (bare `--name` for `bool`). Matches the spec example `wield image.convert foo.png --format webp`.
2. **Registry snapshot test:** a committed JSON fixture at `crates/wield-tools/tests/snapshots/builtin_registry.json`. The test compares `builtin_registry().snapshot()` to it; `UPDATE_SNAPSHOTS=1 cargo test -p wield-tools` regenerates it. No `insta`, no new deps.
3. **`image.convert` optional args:** `width` and `quality` are plain optional args — **no `when` gate**, no `resize` bool. Absent → their argv option pairs drop.

**Forced `wield-core` refinement (GEON decision, not owner-facing):** decision 3 requires the `image.convert` `CommandSpec` to drop *both* `-resize` and `{width}x` when `width` is unset. Segment-drop (P2) only drops the segment that *contains* the unresolved placeholder, leaving a dangling `-resize`. `When` can't currently express "this arg is set" for a wide-range int. So **`When` gains a presence form:** `When { arg, in_values: [] }` (serde `"in": []`) is satisfied iff `arg` has a value in the effective map. Empty `in` was previously meaningless. This is Task 1 and is additive — existing descriptors are unaffected.

Additional constraints from GEON:

- **Scope:** `wield-tools` = the two descriptors + `builtin_registry()` + snapshot test **only**. **No** `screen.ocr`, **no** `launcher.doctor`, **no** other converters (M2/M3/M4). `wield-cli` = the one-shot surface only — **no** daemon, **no** `ShowPalette`/D-Bus (P5). No Tauri changes.
- **Branch:** `feat/p4-tools-cli` off `master`, per-task commits, push at the end, do **not** open the PR (GEON does, after review).
- **`cargo` is not on the default `PATH`.** Every shell that runs `cargo` must first `export PATH="$HOME/.cargo/bin:$PATH"` (rustup stable 1.98.1). `magick` and `ffmpeg` are on `PATH`.
- **Mechanical-correction rule (as P2/P3):** obvious compile / import / lifetime / API-signature fixes to the snippets — apply and note in the Task 9 report. Stop-and-ask only for genuine design ambiguity.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p4-tools-cli` off `master`).
- **Crates touched:** `crates/wield-core` (Task 1 only), `crates/wield-tools`, `crates/wield-cli`, root `Cargo.toml` (no new workspace deps expected — everything is already there).
- **Rust edition:** 2021.
- **No `unwrap()` / `expect()` / `panic!` in library code** outside `#[cfg(test)]`, with the one documented exception carried from P2: `builtin_registry()` and each `fn descriptor()` may `.expect("<id> descriptor is valid")` — built-ins are authored in-repo and the snapshot/validation test guarantees the expects never fire. (Same rationale as `DescriptorBuilder::new`'s panic-on-bad-id.)
- **`wield-cli` exit codes:** `0` success, `1` tool ran but failed / unavailable, `2` usage error (unknown tool, bad args), `130` cancelled.
- **Determinism:** `builtin_registry().snapshot()` is byte-stable across runs and machines (it already sorts by id and emits pretty JSON — P2 Task 13).
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified — `crates/wield-core/` (Task 1 only):**

| File | Change |
| --- | --- |
| `src/args.rs` | `visible_args`: an arg with `when.in_values` empty is visible iff `values` contains `when.arg` |
| `src/template.rs` | segment `when` with empty `in_values`: emit iff `effective` contains `when.arg` |
| `src/validate.rs` | `check_when_values` / `check_arg_when`: empty `in_values` is **valid** (presence check); still reject `when.arg` self-reference / unknown / forward-ref |
| `src/builder.rs` | `ArgSpecBuilder::when_set(arg: &str)`; `CommandSpecBuilder::arg_when_set(template: &str, arg: &str)` |
| `tests/arg_validation.rs`, `tests/argv_rendering.rs`, `tests/descriptor_validation.rs` | presence-form cases |

**Created — `crates/wield-tools/`:**

| File | Responsibility |
| --- | --- |
| `Cargo.toml` | add deps: `wield-core` (already), nothing else |
| `src/lib.rs` | `pub fn builtin_registry() -> Registry`; `pub mod color_pick; pub mod image_convert;`; drop the P1 `descriptor_count` stub |
| `src/color_pick.rs` | `pub fn descriptor() -> Descriptor` — Portal, `screenshot.pick_color`, `Requires::Portal { iface: "Screenshot", min_ver: 2 }`, no args, `OutputSpec::Value(Color)` |
| `src/image_convert.rs` | `pub fn descriptor() -> Descriptor` — Command `magick`, args `input`/`format`/`width?`/`quality?`, `OutputSpec::File` |
| `tests/builtin_registry.rs` | every built-in validates; count == 2; snapshot equals the committed fixture |
| `tests/snapshots/builtin_registry.json` | committed fixture (generated in Task 4) |

**Created — `crates/wield-cli/`:**

| File | Responsibility |
| --- | --- |
| `Cargo.toml` | add deps: `wield-tools`, `wield-portal`, `tokio` (`rt`, `macros`), `tokio-util` |
| `src/main.rs` | `#[tokio::main(flavor = "current_thread")]` → `std::process::exit(wield_cli::run(std::env::args().collect()).await)` |
| `src/lib.rs` | `pub async fn run(argv: Vec<String>) -> i32` — the whole dispatch |
| `src/surface.rs` | `CliSurface::from_descriptor(&Descriptor)` — positional spec + flag specs; `help_text(&Descriptor) -> String` |
| `src/parse.rs` | `parse_args(&CliSurface, &[String]) -> Result<ArgMap, UsageError>` — token → `ArgValue` coercion |
| `src/render.rs` | `render(ToolOutcome) -> (String stdout, String stderr, i32 code)` |

**Out of P4 scope (do not add):** other converters, Native tools, the palette/tray/daemon, `wield --daemon`, config-file (user TOML) descriptors, progress UI beyond draining the channel, shell completions.

---

## Task 1: `wield-core` — `When` presence form + builder helpers

**Files:**
- Modify: `crates/wield-core/src/{args.rs,template.rs,validate.rs,builder.rs}`
- Modify: `crates/wield-core/tests/{arg_validation.rs,argv_rendering.rs,descriptor_validation.rs}`

**Interfaces:**
- Semantics: `When { arg, in_values }` — when `in_values` is non-empty, unchanged (value ∈ literals). When `in_values` is **empty**, the condition is satisfied iff the effective/among-provided map contains a value for `arg`.
- `visible_args`: for `when` with empty `in_values`, an arg is visible iff its `when.arg` is itself visible **and** present in `values`.
- `render_argv` segment `when` with empty `in_values`: emit the segment iff `effective` contains `when.arg` (and, as today, every placeholder resolves).
- `validate_descriptor`: empty `in_values` is legal. All other `when` rules stand (`when.arg` must exist; args `when` still can't forward-reference; `CommandArg.when` may reference any arg).
- New builder methods:
  - `ArgSpecBuilder::when_set(mut self, arg: &str) -> Self` — sets `when = Some(When { arg: arg.into(), in_values: vec![] })`.
  - `CommandSpecBuilder::arg_when_set(mut self, template: &str, arg: &str) -> Self` — pushes `CommandArg { template: template.into(), when: Some(When { arg: arg.into(), in_values: vec![] }) }`.

- [ ] **Step 1: Write the failing tests**

Append to `crates/wield-core/tests/arg_validation.rs`:

```rust
#[test]
fn when_set_hides_arg_until_its_dependency_is_present() {
    let specs = vec![
        ArgSpec { name: "width".into(), label: "w".into(), help: None,
            arg_type: ArgType::Int { range: None, step: None },
            default: None, required: false, when: None },
        ArgSpec { name: "keep_ratio".into(), label: "k".into(), help: None,
            arg_type: ArgType::Bool, default: None, required: false,
            when: Some(When { arg: "width".into(), in_values: vec![] }) },
    ];
    let mut without = BTreeMap::new();
    assert_eq!(visible_args(&specs, &without).len(), 1);
    without.insert("width".into(), ArgValue::Int(800));
    assert_eq!(visible_args(&specs, &without).len(), 2);
}
```

Append to `crates/wield-core/tests/argv_rendering.rs`:

```rust
#[test]
fn arg_when_set_drops_paired_option_when_absent() {
    use wield_core::descriptor::When;
    let gate = |arg: &str| Some(When { arg: arg.into(), in_values: vec![] });
    let args = vec![
        CommandArg::from("{input}"),
        CommandArg { template: "-resize".into(), when: gate("width") },
        CommandArg { template: "{width}x".into(), when: gate("width") },
        CommandArg::from("{output}"),
    ];
    let out = PathBuf::from("/x/a.png");

    let absent = map(&[("input", ArgValue::Path("/x/a.png".into()))]);
    assert_eq!(
        render_argv(&args, &absent, Some(&out)).unwrap(),
        vec!["/x/a.png".to_string(), "/x/a.png".to_string()],
    );

    let present = map(&[
        ("input", ArgValue::Path("/x/a.png".into())),
        ("width", ArgValue::Int(640)),
    ]);
    assert_eq!(
        render_argv(&args, &present, Some(&out)).unwrap(),
        vec!["/x/a.png".to_string(), "-resize".into(), "640x".into(), "/x/a.png".into()],
    );
}
```

Append to `crates/wield-core/tests/descriptor_validation.rs`:

```rust
#[test]
fn accepts_when_with_empty_in_values() {
    let mut d = base_command_descriptor();
    d.args.push(ArgSpec {
        name: "width".into(), label: "w".into(), help: None,
        arg_type: ArgType::Int { range: None, step: None },
        default: None, required: false, when: None,
    });
    d.args.push(ArgSpec {
        name: "keep_ratio".into(), label: "k".into(), help: None,
        arg_type: ArgType::Bool, default: None, required: false,
        when: Some(When { arg: "width".into(), in_values: vec![] }),
    });
    assert!(validate_descriptor(&d).is_ok());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test arg_validation --test argv_rendering --test descriptor_validation`
Expected: the three new tests FAIL.

- [ ] **Step 3: Implement**

- `args.rs` `visible_args`: replace the `Some(when)` branch's satisfaction check with:
  ```rust
  Some(when) => {
      visible_names.contains(when.arg.as_str())
          && match values.get(&when.arg) {
              None => false,
              Some(value) => {
                  when.in_values.is_empty()
                      || when.in_values.iter().any(|lit| value_satisfies_literal(value, lit))
              }
          }
  }
  ```
- `template.rs` segment `when` check: emit iff
  ```rust
  segment.when.as_ref().map_or(true, |when| match effective.get(&when.arg) {
      None => false,
      Some(value) => when.in_values.is_empty()
          || when.in_values.iter().any(|lit| value_satisfies_literal(value, lit)),
  })
  ```
  (keep the existing "all placeholders resolve" check after this.)
- `validate.rs`: `check_when_values` already no-ops on an empty list — confirm and leave. No rule change needed beyond confirming empty `in_values` is not separately rejected.
- `builder.rs`: add the two methods.

- [ ] **Step 4: Run the full wield-core suite**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core`
Expected: PASS — new tests plus every P2/P3 test.

- [ ] **Step 5: Commit**

```bash
git checkout -b feat/p4-tools-cli
git add crates/wield-core
git commit -m "feat(core): When presence form (in: []) and builder helpers"
```

---

## Task 2: `wield-tools` — `color.pick` descriptor

**Files:**
- Modify: `crates/wield-tools/src/lib.rs`
- Create: `crates/wield-tools/src/color_pick.rs`
- Test: `crates/wield-tools/tests/builtin_registry.rs` (start it)

**Interfaces:**
- `crates/wield-tools/src/color_pick.rs`:

  ```rust
  //! The `color.pick` built-in — pick a screen colour via the Screenshot portal.

  use wield_core::{Category, DescriptorBuilder, Descriptor, OutputSpec, Requires, ValueKind};

  pub fn descriptor() -> Descriptor {
      DescriptorBuilder::new("color.pick", "Pick a colour", Category::Capture)
          .keywords(&["colour", "color", "eyedropper", "pixel", "hex", "rgb"])
          .requires(Requires::Portal { iface: "Screenshot".into(), min_ver: 2 })
          .output(OutputSpec::Value(ValueKind::Color))
          .portal("screenshot.pick_color")
          .build()
          .expect("color.pick descriptor is valid")
  }
  ```
- `crates/wield-tools/src/lib.rs`:

  ```rust
  //! Built-in Wield tool descriptors.

  pub mod color_pick;
  pub mod image_convert;

  use wield_core::Registry;

  /// The registry of every built-in tool, validated.
  pub fn builtin_registry() -> Registry {
      let mut registry = Registry::new();
      registry
          .register(color_pick::descriptor())
          .expect("color.pick registers");
      registry
          .register(image_convert::descriptor())
          .expect("image.convert registers");
      registry
  }
  ```
  (Task 2 can leave `image_convert` as a stub `pub fn descriptor() -> Descriptor { unimplemented!() }` **only** if `builtin_registry` / its call is behind `#[cfg(test)]` until Task 3 — cleaner: do Task 3's `image_convert.rs` file skeleton here returning a `todo!()` and don't call `builtin_registry` from a test until Task 4. Simplest: in Task 2, `lib.rs` only declares `pub mod color_pick;` and `builtin_registry` registers just `color.pick`; Task 3 adds `image_convert` to both.)

  **Do the simplest:** Task 2 `lib.rs` = `pub mod color_pick;` + `builtin_registry()` registering only `color.pick`. Task 3 adds the second module + registration.

- [ ] **Step 1: Write the failing test**

`crates/wield-tools/tests/builtin_registry.rs`:

```rust
use wield_tools::builtin_registry;
use wield_core::{Capability, Requires};

#[test]
fn color_pick_is_a_valid_portal_tool() {
    let registry = builtin_registry();
    let tool = registry.get("color.pick").expect("color.pick present");
    assert!(matches!(tool.capability, Capability::Portal { .. }));
    assert!(matches!(tool.requires, Requires::Portal { min_ver: 2, .. }));
    assert!(tool.args.is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools --test builtin_registry`
Expected: FAIL.

- [ ] **Step 3: Implement `color_pick.rs` + `lib.rs` (color.pick only)**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-tools
git commit -m "feat(tools): color.pick built-in descriptor"
```

---

## Task 3: `wield-tools` — `image.convert` descriptor

**Files:**
- Create: `crates/wield-tools/src/image_convert.rs`
- Modify: `crates/wield-tools/src/lib.rs` (add module + registration)
- Test: `crates/wield-tools/tests/builtin_registry.rs`

**Interfaces:**
- `crates/wield-tools/src/image_convert.rs`:

  ```rust
  //! The `image.convert` built-in — format / resize / recompress via ImageMagick.

  use std::time::Duration;
  use wield_core::{
      ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
      DescriptorBuilder, FileFilter, OutputDir, OutputSpec, Requires,
  };

  const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tiff", "avif"];
  const FORMATS: &[&str] = &["png", "jpg", "webp", "gif", "bmp", "tiff", "avif"];

  pub fn descriptor() -> Descriptor {
      DescriptorBuilder::new("image.convert", "Convert image", Category::Convert)
          .keywords(&["image", "convert", "resize", "compress", "png", "jpeg", "webp", "format"])
          .arg(
              ArgSpecBuilder::new(
                  "input",
                  "Image",
                  ArgType::File {
                      filters: vec![FileFilter {
                          label: "Images".into(),
                          extensions: IMAGE_EXTS.iter().map(|s| (*s).to_owned()).collect(),
                      }],
                      multiple: false,
                  },
              )
              .required(true)
              .build(),
          )
          .arg(
              ArgSpecBuilder::new(
                  "format",
                  "Output format",
                  ArgType::Enum { options: FORMATS.iter().map(|s| (*s).to_owned()).collect() },
              )
              .required(true)
              .default(ArgValueLiteral::Str("png".into()))
              .build(),
          )
          .arg(
              ArgSpecBuilder::new("width", "Width (px)", ArgType::Int { range: Some([1, 20_000]), step: Some(1) })
                  .help("Resize to this width, keeping aspect ratio. Leave blank to keep the original size.")
                  .build(),
          )
          .arg(
              ArgSpecBuilder::new("quality", "Quality", ArgType::Int { range: Some([1, 100]), step: Some(1) })
                  .help("Compression quality for lossy formats (JPEG, WebP, AVIF). Leave blank for the format default.")
                  .build(),
          )
          .requires(Requires::Binary("magick".into()))
          .output(OutputSpec::File {
              name: "{input_stem}.{format}".into(),
              dir: OutputDir::SameAsInput,
          })
          .command(
              CommandSpecBuilder::new("magick")
                  .arg("{input}")
                  .arg_when_set("-resize", "width")
                  .arg_when_set("{width}x", "width")
                  .arg_when_set("-quality", "quality")
                  .arg_when_set("{quality}", "quality")
                  .arg("{output}")
                  .timeout(Duration::from_secs(120)),
          )
          .build()
          .expect("image.convert descriptor is valid")
  }
  ```
  Note: `magick` invocation is `magick <input> [-resize WxH] [-quality N] <output>`. The output format is taken from the `<output>` file extension (`{input_stem}.{format}`), which ImageMagick honours — no explicit format flag needed.

- [ ] **Step 1: Write the failing tests**

Append to `crates/wield-tools/tests/builtin_registry.rs`:

```rust
use std::collections::BTreeMap;
use wield_core::{args::ArgValue, template::render_argv, Capability};

#[test]
fn image_convert_renders_magick_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("image.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else { panic!("expected Command") };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/pics/a.png".into()));
    minimal.insert("format".to_string(), ArgValue::Str("webp".into()));
    let out = std::path::PathBuf::from("/pics/a.webp");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec!["/pics/a.png".to_string(), "/pics/a.webp".into()],
    );

    let mut full = minimal.clone();
    full.insert("width".to_string(), ArgValue::Int(1024));
    full.insert("quality".to_string(), ArgValue::Int(82));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "/pics/a.png".to_string(), "-resize".into(), "1024x".into(),
            "-quality".into(), "82".into(), "/pics/a.webp".into(),
        ],
    );
}

#[test]
fn registry_has_exactly_the_expected_builtins() {
    let ids: Vec<_> = builtin_registry().list().iter().map(|d| d.id.as_ref().to_string()).collect();
    assert_eq!(ids, vec!["color.pick", "image.convert"]);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools --test builtin_registry`
Expected: FAIL.

- [ ] **Step 3: Implement `image_convert.rs` + wire into `lib.rs`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-tools
git commit -m "feat(tools): image.convert built-in descriptor"
```

---

## Task 4: `wield-tools` — registry snapshot test

**Files:**
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Create: `crates/wield-tools/tests/snapshots/builtin_registry.json`

**Interfaces:**
- The test:

  ```rust
  #[test]
  fn builtin_registry_matches_snapshot() {
      let actual = builtin_registry().snapshot();
      let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots/builtin_registry.json");
      if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
          std::fs::write(path, &actual).unwrap();
          return;
      }
      let expected = std::fs::read_to_string(path)
          .expect("run `UPDATE_SNAPSHOTS=1 cargo test -p wield-tools` to create the snapshot");
      assert_eq!(actual, expected, "built-in registry changed — review the diff, then UPDATE_SNAPSHOTS=1 to accept");
  }
  ```

- [ ] **Step 1: Write the test (it fails — no snapshot file yet)**

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools --test builtin_registry builtin_registry_matches_snapshot`
Expected: FAIL — snapshot file missing.

- [ ] **Step 3: Generate the snapshot**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p crates/wield-tools/tests/snapshots
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools --test builtin_registry builtin_registry_matches_snapshot
```
Then **read** `crates/wield-tools/tests/snapshots/builtin_registry.json` and sanity-check it: two descriptors, `color.pick` before `image.convert` (sorted), the `image.convert` `CommandArg`s carry `when: { arg: "width"/"quality", in: [] }`, no absolute machine paths.

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-tools`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-tools/tests
git commit -m "test(tools): built-in registry snapshot"
```

---

## Task 5: `wield-cli` — CLI surface from a descriptor

**Files:**
- Modify: `crates/wield-cli/Cargo.toml`
- Create: `crates/wield-cli/src/surface.rs`
- Modify: `crates/wield-cli/src/lib.rs` (create it; `main.rs` stays until Task 8)
- Test: unit tests in `surface.rs`

**Interfaces:**
- `crates/wield-cli/Cargo.toml` `[dependencies]`: add

  ```toml
  wield-tools = { path = "../wield-tools" }
  wield-portal = { path = "../wield-portal" }
  tokio = { workspace = true }
  tokio-util.workspace = true
  ```
- `src/surface.rs`:

  ```rust
  use wield_core::{ArgSpec, ArgType, Descriptor};

  /// One `--flag` derived from an `ArgSpec`.
  pub struct FlagSpec<'a> {
      pub spec: &'a ArgSpec,
      pub takes_value: bool, // false only for ArgType::Bool
  }

  pub struct CliSurface<'a> {
      /// The one positional arg (first required file/dir), if any.
      pub positional: Option<&'a ArgSpec>,
      pub flags: Vec<FlagSpec<'a>>,
  }

  impl<'a> CliSurface<'a> {
      pub fn from_descriptor(descriptor: &'a Descriptor) -> Self {
          let positional = descriptor.args.iter().find(|a| {
              a.required && matches!(a.arg_type, ArgType::File { .. } | ArgType::Dir)
          });
          let flags = descriptor
              .args
              .iter()
              .filter(|a| positional.map_or(true, |p| !std::ptr::eq(*a, p)))
              .map(|spec| FlagSpec { spec, takes_value: !matches!(spec.arg_type, ArgType::Bool) })
              .collect();
          Self { positional, flags }
      }
  }

  /// `wield <id> --help` body.
  pub fn help_text(descriptor: &Descriptor) -> String;
  ```
  `help_text`: `Usage: wield <id> [<POSITIONAL>] [options]` then one line per arg — `--name <type>  (required|optional[, default X])  help`.

- [ ] **Step 1: Write the failing tests**

`crates/wield-cli/src/surface.rs` (tests module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wield_tools::builtin_registry;

    #[test]
    fn image_convert_has_input_positional_and_three_flags() {
        let reg = builtin_registry();
        let d = reg.get("image.convert").unwrap();
        let s = CliSurface::from_descriptor(d);
        assert_eq!(s.positional.unwrap().name, "input");
        let names: Vec<_> = s.flags.iter().map(|f| f.spec.name.as_str()).collect();
        assert_eq!(names, vec!["format", "width", "quality"]);
        assert!(s.flags.iter().all(|f| f.takes_value)); // none are bool
    }

    #[test]
    fn color_pick_has_no_positional_and_no_flags() {
        let reg = builtin_registry();
        let d = reg.get("color.pick").unwrap();
        let s = CliSurface::from_descriptor(d);
        assert!(s.positional.is_none());
        assert!(s.flags.is_empty());
    }

    #[test]
    fn help_text_names_the_positional_and_each_flag() {
        let reg = builtin_registry();
        let text = help_text(reg.get("image.convert").unwrap());
        assert!(text.contains("image.convert"));
        assert!(text.contains("--format"));
        assert!(text.contains("--width"));
        assert!(text.contains("required") && text.contains("optional"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-cli`
Expected: FAIL.

- [ ] **Step 3: Implement `surface.rs`; create `src/lib.rs` with `pub mod surface;`**

- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-cli`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-cli
git commit -m "feat(cli): CLI surface generation from a descriptor"
```

---

## Task 6: `wield-cli` — parse argv into an `ArgMap`

**Files:**
- Create: `crates/wield-cli/src/parse.rs`
- Modify: `crates/wield-cli/src/lib.rs` (`pub mod parse;`)
- Test: unit tests in `parse.rs`

**Interfaces:**
- `src/parse.rs`:

  ```rust
  use std::collections::BTreeMap;
  use wield_core::{ArgType, ArgValue};
  use crate::surface::CliSurface;

  #[derive(Debug, PartialEq)]
  pub struct UsageError(pub String);

  /// Parse the tokens after `wield <id>` into an `ArgMap`, using the surface to
  /// decide the positional and which flags take a value. Does **not** run
  /// descriptor validation — that stays in the executor / caller.
  pub fn parse_args(surface: &CliSurface<'_>, tokens: &[String]) -> Result<BTreeMap<String, ArgValue>, UsageError>;
  ```
  Rules:
  - A bare token (no leading `--`) fills the positional (error if there is no positional, or it is already set).
  - `--name value` for a value flag; `--name=value` also accepted; `--name` alone for a `Bool` flag → `ArgValue::Bool(true)`.
  - Unknown `--name` → `UsageError`.
  - Coercion by the flag's `ArgType`: `Int` → `token.parse::<i64>()` (error → `UsageError("--width expects a whole number")`), `Float` → `f64`, `Bool` → `true`, `File`/`Dir` → `ArgValue::Path`, `Str`/`Text`/`Enum` → `ArgValue::Str` (enum membership is checked later by `validate_args`).
  - `File { multiple: true }` positional/flag: repeated → `ArgValue::Paths` (not needed by P4 built-ins, but implement it).

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wield_tools::builtin_registry;

    fn surface_for(id: &str) -> (wield_core::Registry, ) { unreachable!() } // see note

    #[test]
    fn parses_positional_and_flags() {
        let reg = builtin_registry();
        let d = reg.get("image.convert").unwrap();
        let s = CliSurface::from_descriptor(d);
        let map = parse_args(&s, &["a.png".into(), "--format".into(), "webp".into(), "--width".into(), "800".into()]).unwrap();
        assert_eq!(map.get("input"), Some(&ArgValue::Path("a.png".into())));
        assert_eq!(map.get("format"), Some(&ArgValue::Str("webp".into())));
        assert_eq!(map.get("width"), Some(&ArgValue::Int(800)));
    }

    #[test]
    fn accepts_equals_form() {
        let reg = builtin_registry();
        let s = CliSurface::from_descriptor(reg.get("image.convert").unwrap());
        let map = parse_args(&s, &["a.png".into(), "--format=png".into()]).unwrap();
        assert_eq!(map.get("format"), Some(&ArgValue::Str("png".into())));
    }

    #[test]
    fn rejects_unknown_flag_and_bad_int() {
        let reg = builtin_registry();
        let s = CliSurface::from_descriptor(reg.get("image.convert").unwrap());
        assert!(parse_args(&s, &["a.png".into(), "--bogus".into(), "x".into()]).is_err());
        assert!(parse_args(&s, &["a.png".into(), "--width".into(), "wide".into()]).is_err());
    }

    #[test]
    fn rejects_second_positional() {
        let reg = builtin_registry();
        let s = CliSurface::from_descriptor(reg.get("image.convert").unwrap());
        assert!(parse_args(&s, &["a.png".into(), "b.png".into()]).is_err());
    }
}
```
(Remove the bogus `surface_for` stub; the tests build the surface inline as shown.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p wield-cli`
- [ ] **Step 3: Implement `parse.rs`**
- [ ] **Step 4: Run to verify pass** — `cargo test -p wield-cli`
- [ ] **Step 5: Commit**

```bash
git add crates/wield-cli
git commit -m "feat(cli): argv -> ArgMap parsing with coercion"
```

---

## Task 7: `wield-cli` — render a `ToolOutcome`

**Files:**
- Create: `crates/wield-cli/src/render.rs`
- Modify: `crates/wield-cli/src/lib.rs` (`pub mod render;`)
- Test: unit tests in `render.rs`

**Interfaces:**
- `src/render.rs`:

  ```rust
  use wield_core::ToolOutcome;

  pub struct Rendered {
      pub stdout: String,
      pub stderr: String,
      pub code: i32,
  }

  pub fn render(outcome: ToolOutcome) -> Rendered {
      match outcome {
          ToolOutcome::Value { data, .. } => out(data, 0),
          ToolOutcome::File { path } => out(path.display().to_string(), 0),
          ToolOutcome::Report { title, lines } => out(std::iter::once(title).chain(lines).collect::<Vec<_>>().join("\n"), 0),
          ToolOutcome::Unavailable { reason, fix } => err(with_fix(&reason, fix.as_deref()), 1),
          ToolOutcome::Failed { detail, hint, .. } => err(with_fix(&detail, hint.as_deref()), 1),
          ToolOutcome::Cancelled => Rendered { stdout: String::new(), stderr: String::new(), code: 130 },
      }
  }
  ```
  `with_fix(msg, Some(fix))` → `"{msg}\n  → {fix}"`; `with_fix(msg, None)` → `msg`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wield_core::{Stage, ValueKind};

    #[test]
    fn value_and_file_go_to_stdout_with_code_0() {
        assert_eq!(render(ToolOutcome::Value { kind: ValueKind::Color, data: "#fff".into() }).code, 0);
        let r = render(ToolOutcome::File { path: "/tmp/out.webp".into() });
        assert_eq!(r.stdout.trim(), "/tmp/out.webp");
        assert_eq!(r.code, 0);
    }

    #[test]
    fn failed_and_unavailable_go_to_stderr_with_code_1() {
        let r = render(ToolOutcome::Failed { stage: Stage::Command, detail: "exited with 1".into(), hint: Some("input may be corrupt".into()) });
        assert!(r.stderr.contains("exited with 1") && r.stderr.contains("input may be corrupt"));
        assert_eq!(r.code, 1);
        assert_eq!(render(ToolOutcome::Unavailable { reason: "no magick".into(), fix: None }).code, 1);
    }

    #[test]
    fn cancelled_is_silent_code_130() {
        let r = render(ToolOutcome::Cancelled);
        assert!(r.stdout.is_empty() && r.stderr.is_empty());
        assert_eq!(r.code, 130);
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p wield-cli`
- [ ] **Step 3: Implement `render.rs`**
- [ ] **Step 4: Run to verify pass** — `cargo test -p wield-cli`
- [ ] **Step 5: Commit**

```bash
git add crates/wield-cli
git commit -m "feat(cli): ToolOutcome rendering and exit codes"
```

---

## Task 8: `wield-cli` — `run()` wiring + `main`

**Files:**
- Modify: `crates/wield-cli/src/lib.rs` (add `pub async fn run`)
- Replace: `crates/wield-cli/src/main.rs`
- Test: `crates/wield-cli/tests/cli.rs`

**Interfaces:**
- `src/lib.rs`:

  ```rust
  pub mod parse;
  pub mod render;
  pub mod surface;

  use std::sync::Arc;
  use wield_core::{AvailabilityView, BinaryResolver, ExecutionRequest, Executor};
  use wield_portal::PortalAdapterRunner;
  use wield_tools::builtin_registry;

  /// Entry point. `argv` includes `argv[0]`. Returns the process exit code.
  pub async fn run(argv: Vec<String>) -> i32 {
      let args: Vec<String> = argv.into_iter().skip(1).collect();

      match args.first().map(String::as_str) {
          None => { print_tool_list(); return 0; }
          Some("--version" | "-V") => { println!("wield {}", wield_core::version()); return 0; }
          Some("--help" | "-h") => { print_usage(); return 0; }
          _ => {}
      }

      let id = &args[0];
      let registry = builtin_registry();
      let Some(descriptor) = registry.get(id) else {
          eprintln!("unknown tool: {id}");
          print_tool_list_to_stderr();
          return 2;
      };
      let rest = &args[1..];
      if rest.iter().any(|t| t == "--help" || t == "-h") {
          println!("{}", crate::surface::help_text(descriptor));
          return 0;
      }

      let surface = crate::surface::CliSurface::from_descriptor(descriptor);
      let arg_map = match crate::parse::parse_args(&surface, rest) {
          Ok(map) => map,
          Err(e) => { eprintln!("{}", e.0); return 2; }
      };

      // Availability: probe portals (best-effort), resolve binaries.
      let resolver = BinaryResolver::from_env();
      let mut availability = AvailabilityView::probe_binaries(&resolver, registry.list());
      wield_portal::probe().await.apply_to(&mut availability);

      let executor = Executor::new(resolver)
          .with_portal(Arc::new(PortalAdapterRunner))
          .with_availability(availability);

      let (tx, mut rx) = tokio::sync::mpsc::channel(32);
      let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });

      let outcome = executor
          .run(
              ExecutionRequest { descriptor: descriptor.clone(), args: arg_map },
              tx,
              tokio_util::sync::CancellationToken::new(),
          )
          .await;
      drain.abort();

      let rendered = crate::render::render(outcome);
      if !rendered.stdout.is_empty() { println!("{}", rendered.stdout.trim_end()); }
      if !rendered.stderr.is_empty() { eprintln!("{}", rendered.stderr.trim_end()); }
      rendered.code
  }
  ```
  `print_tool_list` / `print_usage`: list `registry.list()` as `  <id>  —  <title>`.
- `src/main.rs`:

  ```rust
  #[tokio::main(flavor = "current_thread")]
  async fn main() {
      let code = wield_cli::run(std::env::args().collect()).await;
      std::process::exit(code);
  }
  ```

- [ ] **Step 1: Write the failing tests**

`crates/wield-cli/tests/cli.rs`:

```rust
#[tokio::test]
async fn bare_invocation_lists_the_builtins() {
    // run() prints to real stdout; assert on the exit code and that it doesn't error.
    let code = wield_cli::run(vec!["wield".into()]).await;
    assert_eq!(code, 0);
}

#[tokio::test]
async fn version_flag() {
    assert_eq!(wield_cli::run(vec!["wield".into(), "--version".into()]).await, 0);
}

#[tokio::test]
async fn unknown_tool_is_usage_error() {
    assert_eq!(wield_cli::run(vec!["wield".into(), "no.such.tool".into()]).await, 2);
}

#[tokio::test]
async fn image_convert_end_to_end_against_magick() {
    if which::which("magick").is_err() {
        eprintln!("skipping: magick not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("src.png");
    // make a 2x2 png with magick itself
    std::process::Command::new("magick")
        .args(["-size", "2x2", "xc:red", input.to_str().unwrap()])
        .status().unwrap();

    let code = wield_cli::run(vec![
        "wield".into(), "image.convert".into(),
        input.to_str().unwrap().into(),
        "--format".into(), "webp".into(),
    ]).await;
    assert_eq!(code, 0);
    assert!(dir.path().join("src.webp").exists());
}

#[tokio::test]
async fn image_convert_missing_input_is_usage_error() {
    assert_eq!(
        wield_cli::run(vec!["wield".into(), "image.convert".into(), "--format".into(), "png".into()]).await,
        2
    );
}
```
Add `[dev-dependencies]` to `crates/wield-cli/Cargo.toml`: `tempfile.workspace = true`, `which = "6"` (or use `BinaryResolver::from_env().resolve("magick").is_none()` instead of the `which` crate — prefer that, no new dep). `tokio = { workspace = true, features = ["macros", "rt"] }`.

**Revise the magick check** to avoid the `which` dep:
```rust
if wield_core::BinaryResolver::from_env().resolve("magick").is_none() { return; }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p wield-cli`
- [ ] **Step 3: Implement `run()` + `main.rs`**
- [ ] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-cli`
Expected: PASS. Also run `cargo run -p wield-cli -- image.convert <some.png> --format png` manually once to eyeball the output.

- [ ] **Step 5: Commit**

```bash
git add crates/wield-cli
git commit -m "feat(cli): one-shot run() wiring, tool list, and end-to-end tests"
```

---

## Task 9: Workspace green + P4 wrap-up

**Files:**
- Modify: this plan file (status block)

- [ ] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: fmt clean, clippy clean, all tests pass.

- [ ] **Step 2: `npm run check`** — `export PATH="$HOME/.cargo/bin:$PATH" && npm run check` → PASS.

- [ ] **Step 3: Manual smoke**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo run -q -p wield-cli                     # lists color.pick + image.convert
cargo run -q -p wield-cli -- --version        # wield 0.1.0
cargo run -q -p wield-cli -- image.convert --help
magick -size 4x4 xc:blue /tmp/wield-smoke.png
cargo run -q -p wield-cli -- image.convert /tmp/wield-smoke.png --format webp --width 2
ls -l /tmp/wield-smoke.webp                   # exists
```

- [ ] **Step 4: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P4 complete" --body "wield-tools (color.pick + image.convert built-ins, builtin_registry, snapshot test) and wield-cli one-shot (surface generation, argv parsing, outcome rendering, run() wiring) landed on feat/p4-tools-cli. wield-core gained the When presence form (in: []). cargo test --workspace + clippy + npm run check green; image.convert end-to-end against real magick passes. N commits, branch pushed."
```

- [ ] **Step 5: Commit the checked-off plan + push**

```bash
git add docs/superpowers/plans/2026-09-10-wield-m1-p4-tools-cli.md
git commit -m "docs: mark M1 P4 plan complete"
git push -u origin feat/p4-tools-cli
```

GEON opens the PR, reviews, merges to `master`. P5 (Tauri shell — palette window, tray, `GlobalShortcuts`, D-Bus single instance, `list_tools`/`run_tool`/`cancel`/`capabilities` commands) is written just-in-time.

---

## Self-Review

**Spec coverage:**

- §3 `wield-tools` = "built-in tool descriptors (data) + the `Native` tool implementations" → descriptors done (Tasks 2, 3); `Native` impls (`screen.ocr`, `launcher.doctor`) are **M3/M4**, explicitly out of scope ✓
- §3 `wield-cli` = "CLI surface generated from the same registry … keeps the registry surface-agnostic" → Tasks 5–8 generate everything from `Descriptor`; no per-tool CLI code ✓
- §4 "Built-ins: type-safe Rust builders in `wield-tools` — no runtime parse risk" → `DescriptorBuilder` in both descriptor fns ✓
- §4 "Registry snapshot test guards accidental descriptor changes" → Task 4 ✓
- §4 `ArgSpec` `when` conditional visibility → extended with the presence form (Task 1), exercised by `image.convert` ✓
- §6 `color.pick` — Capture / Portal / `Screenshot.PickColor` / "copyable as hex / rgb / hsl" → Task 2 descriptor; the hex/rgb/hsl payload is produced by the P3 adapter ✓
- §6 `image.convert` — Convert / Command / ImageMagick / "format + resize + quality" → Task 3 ✓ (`when`-gated in the spec table; realised here as presence-gated option pairs per owner decision 3)
- §6 note "`PickColor` needs Screenshot portal v2+; older → tool greyed with reason" → `Requires::Portal { min_ver: 2 }` (Task 2) + the P3 executor `Unavailable` messaging ✓
- §7 outcome rendering (Value/File to stdout; Failed/Unavailable to stderr + hint; Cancelled silent) → Task 7 ✓
- §3 "CLI with no instance running (`wield image.convert file.png`) runs the tool one-shot" → Task 8 ✓
- spec §5 "resolves `Command` binaries … feeds … a first-run summary if something notable is missing" — the CLI does `probe_binaries` + `probe()` and the executor reports `Unavailable`; a *summary* screen is the shell's job (P5) ✓

**Placeholder scan:** the only `todo!()`/`unimplemented!()` mentioned is in a *discussion aside* about Task 2/3 sequencing, resolved by "do the simplest: Task 2 registers only color.pick". No placeholder survives in a step body. Deferred items name their milestone.

**Type consistency:** `builtin_registry() -> Registry` (Task 2) consumed by Tasks 3, 4, 5, 6, 8. `CliSurface::from_descriptor(&Descriptor) -> CliSurface` (Task 5) consumed by Tasks 6, 8. `parse_args(&CliSurface, &[String]) -> Result<BTreeMap<String,ArgValue>, UsageError>` (Task 6) consumed by Task 8. `render(ToolOutcome) -> Rendered { stdout, stderr, code }` (Task 7) consumed by Task 8. `When { arg, in_values: [] }` presence form (Task 1) used by `image_convert.rs` (Task 3) and validated by the snapshot (Task 4). `AvailabilityView::probe_binaries` + `PortalMap::apply_to` (P2/P3) composed in Task 8.

**Ordering:** 1 (core `When`) → 2 (color.pick) → 3 (image.convert, needs 1's `arg_when_set`) → 4 (snapshot, needs 2+3) → 5 (surface, needs 2+3 for tests) → 6 (parse, needs 5) → 7 (render) → 8 (run, needs 5+6+7 + P3's `PortalAdapterRunner`/`probe`) → 9. Consistent.

---

## Execution note

After P4 merges, `wield` is a working one-shot converter/colour-picker on the command line, and the built-in registry is snapshot-guarded. P5 builds the Tauri shell: it constructs the same `Executor` (resolver + `PortalAdapterRunner` + `probe()`), exposes `list_tools` / `run_tool` / `cancel` / `capabilities` as Tauri commands over `builtin_registry()`, and adds the palette window + tray + `GlobalShortcuts` registration + the `io.github.DhanushSantosh.Wield` single-instance D-Bus name.

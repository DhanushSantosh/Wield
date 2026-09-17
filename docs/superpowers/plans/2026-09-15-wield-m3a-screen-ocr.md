# M3a: `screen.ocr` + Real `Native` Capability Infrastructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first real `Capability::Native` execution path in `wield-core` and ship `screen.ocr` as its first tool — Screenshot portal capture → `tesseract` OCR → clipboard-ready text.

**Architecture:** A `NativeRunner` trait mirrors the existing `PortalRunner` trait exactly (same injection shape into `Executor`, same string-keyed dispatch pattern). A new `wield-native` crate (sibling to `wield-portal`, not folded into it) holds the dispatch + the `screen.ocr` pipeline. `tesseract` runs via `wield-core`'s already-descriptor-decoupled `CommandRunner`, writing to a self-managed scratch directory (no `wield-core` changes needed for stdout capture — `CommandRunner` currently discards captured stdout entirely, verified at `command.rs:230`, so this plan routes around that gap rather than extending it, since tesseract's normal file-output mode already does the job with zero core changes, verified live). A small, real `Requires::All(Vec<Requires>)` addition lets `screen.ocr` proactively gate on both the Screenshot portal *and* the `tesseract` binary, since no existing tool has ever needed two simultaneous requirements.

**Tech Stack:** Rust workspace (existing crates: `wield-core`, `wield-portal`, `wield-tools`; new crate: `wield-native`), `ashpd` (Screenshot portal), `tesseract` (subprocess, via `CommandRunner`), `tempfile`, `url`.

**Spec:** `docs/superpowers/specs/2026-09-15-wield-m3a-screen-ocr-design.md`

## Global Constraints

- `screen.ocr` ships English-only for this milestone — no language picker or download UI (owner decision, spec §5).
- Result delivery reuses the existing `ToolOutcome::Value` + `ResultView` "Copy" button pattern — no automatic/silent backend clipboard write, no new frontend work (owner decision, spec §4).
- `NativeRunner::run` takes no `progress` sender — matches `PortalRunner::run`'s signature exactly (spec §3.2). A single screenshot-sized `tesseract` call is sub-second; there is nothing meaningful to report mid-run.
- Every new `Requires`/`Capability::Native` addition to `wield-core` must be additive only — no existing descriptor's behavior changes (spec §3.3).
- `tesseract`'s timeout is 30 seconds (spec §3.5) — generous for one image, tight enough that a genuine hang surfaces quickly.
- Cleanup of the portal-created screenshot file is best-effort: log-and-ignore a deletion failure, never fail the tool run over a leftover temp file (spec §3.5 step 4).
- Empty (or whitespace-only) OCR output is a `Failed` outcome with a clear message, never a silent empty-string `Value` (spec §3.5 step 5).

---

### Task 1: `Requires::All` composite requirement

**Files:**
- Modify: `crates/wield-core/src/descriptor.rs` (the `Requires` enum, currently `None`/`Portal{..}`/`Binary(..)`)
- Modify: `crates/wield-core/src/executor.rs` (the inline `match &request.descriptor.requires` in `run()`, and `AvailabilityView::probe_binaries`)
- Modify: `crates/wield-core/src/registry.rs` (`Registry::available` — a **third**, independent exhaustive match over `Requires`, missed in the original design; caught by the Rust compiler's exhaustiveness check once `All` existed, and correctly stopped-on rather than improvised around per the plan's own etiquette — see Step 6)
- Modify: `apps/wield/src-tauri/src/capabilities.rs` (`is_available` — a **fourth** exhaustive match, one crate up; plus `report()`'s binary-collection loop, which has the same nested-`All` gap without being a compile error — see Step 7)
- Test: `crates/wield-core/tests/executor_pipeline.rs`, `crates/wield-core/tests/registry.rs`, `apps/wield/src-tauri/src/capabilities.rs` (new inline `#[cfg(test)]` module — this file had none before)

**Interfaces:**
- Produces: `Requires::All(Vec<Requires>)` variant; a free function `fn check_requires(requires: &Requires, resolver: &BinaryResolver, portals: &HashMap<String, u32>) -> Option<ToolOutcome>` (returns `Some(blocking_outcome)` if unmet, `None` if satisfied); a free function `fn available_binaries(requires: &Requires, resolver: &BinaryResolver) -> Vec<String>`; a free function `fn requires_satisfied(requires: &Requires, view: &AvailabilityView) -> bool` in `registry.rs`; `apps/wield/src-tauri/src/capabilities.rs`'s existing `is_available` gains an `All` arm and a new `fn binary_requirements(requires: &Requires) -> Vec<&String>` helper. All of these are used by later tasks' descriptors (`screen.ocr` in Task 5) without further changes here.

- [ ] **Step 1: Write the failing tests**

Add to `crates/wield-core/tests/executor_pipeline.rs`, near the existing `portal_requires_unmet_is_unavailable`/`portal_requires_met_runs_the_adapter` tests:

```rust
#[tokio::test]
async fn all_requires_blocks_on_the_first_unmet_entry() {
    let mut descriptor = convert_descriptor("nope-missing-binary");
    descriptor.requires = Requires::All(vec![
        Requires::Portal {
            iface: "Screenshot".into(),
            min_ver: 2,
        },
        Requires::Binary("nope-missing-binary".into()),
    ]);
    let mut view = wield_core::AvailabilityView::default();
    view.portals.insert("Screenshot".into(), 2);
    let executor = Executor::new(BinaryResolver::with_dirs(vec![])).with_availability(view);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x.raw".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Unavailable { .. }));
}

#[tokio::test]
async fn all_requires_met_runs_the_tool() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "cp-conv", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let input = dir.path().join("photo.raw");
    std::fs::write(&input, b"PIXELS").unwrap();

    let mut descriptor = convert_descriptor("cp-conv");
    descriptor.requires = Requires::All(vec![
        Requires::Portal {
            iface: "Screenshot".into(),
            min_ver: 2,
        },
        Requires::Binary("cp-conv".into()),
    ]);
    let mut view = wield_core::AvailabilityView::default();
    view.portals.insert("Screenshot".into(), 2);
    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]))
        .with_availability(view);
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
    assert!(matches!(outcome, ToolOutcome::File { .. }));
}

#[test]
fn probe_binaries_collects_a_binary_nested_inside_all() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "nested-bin", "#!/bin/sh\nexit 0\n");
    let mut descriptor = convert_descriptor("nested-bin");
    descriptor.requires = Requires::All(vec![
        Requires::Portal {
            iface: "Screenshot".into(),
            min_ver: 2,
        },
        Requires::Binary("nested-bin".into()),
    ]);
    let resolver = BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]);
    let view = wield_core::AvailabilityView::probe_binaries(&resolver, &[descriptor]);
    assert!(view.binaries.contains("nested-bin"));
}
```

Note: `write_stub_script` and `support` come from the file's existing `mod support;` — `probe_binaries_collects_a_binary_nested_inside_all` is a plain `#[test]` (not `#[tokio::test]`), matching `probe_binaries`'s own sync signature.

- [ ] **Step 2: Run the tests to verify they fail**

`cargo test` accepts exactly one positional `TESTNAME` filter (a substring
match), not several — run it as two commands instead:

```bash
cargo test -p wield-core --test executor_pipeline all_requires
cargo test -p wield-core --test executor_pipeline probe_binaries_collects_a_binary_nested_inside_all
```

(`all_requires` as a substring matches both
`all_requires_blocks_on_the_first_unmet_entry` and
`all_requires_met_runs_the_tool`.)
Expected: both FAIL to compile — `Requires::All` does not exist yet.

- [ ] **Step 3: Add the `All` variant**

In `crates/wield-core/src/descriptor.rs`, find:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Requires {
    None,
    Portal { iface: String, min_ver: u32 },
    Binary(String),
}
```

Replace with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Requires {
    None,
    Portal { iface: String, min_ver: u32 },
    Binary(String),
    /// Every entry must be satisfied. Added for tools needing more than one
    /// requirement at once (e.g. `screen.ocr`: the Screenshot portal *and*
    /// the `tesseract` binary) — no existing single-valued `Requires` could
    /// express that without leaving one half unable to proactively grey out
    /// in the palette the way `AvailabilityView::probe_binaries` already
    /// does for `Binary`.
    All(Vec<Requires>),
}
```

- [ ] **Step 4: Extract the pre-flight check into `check_requires`**

In `crates/wield-core/src/executor.rs`, find the `match &request.descriptor.requires { ... }` block inside `run()` (the block starting `Requires::Binary(binary) if self.resolver.resolve(binary).is_none() => {` and ending with `Requires::None | Requires::Binary(_) => {}`). Replace the whole `match` statement with:

```rust
if let Some(outcome) = check_requires(
    &request.descriptor.requires,
    &self.resolver,
    &self.availability.portals,
) {
    return outcome;
}
```

Then add this free function near the other free functions at the bottom of the file (next to `fn failed`, `fn unavailable_binary`):

```rust
fn check_requires(
    requires: &Requires,
    resolver: &BinaryResolver,
    portals: &HashMap<String, u32>,
) -> Option<ToolOutcome> {
    match requires {
        Requires::None => None,
        Requires::Binary(binary) => {
            if resolver.resolve(binary).is_none() {
                Some(unavailable_binary(binary))
            } else {
                None
            }
        }
        Requires::Portal { iface, min_ver } => match portals.get(iface) {
            Some(version) if version >= min_ver => None,
            Some(version) => Some(ToolOutcome::Unavailable {
                reason: format!(
                    "the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"
                ),
                fix: Some(format!(
                    "update your desktop environment to one that provides {iface} portal v{min_ver} or newer"
                )),
            }),
            None => Some(ToolOutcome::Unavailable {
                reason: format!("the {iface} desktop portal is not available"),
                fix: Some(
                    "this tool needs a desktop environment with XDG Desktop Portal support"
                        .to_owned(),
                ),
            }),
        },
        Requires::All(all) => all
            .iter()
            .find_map(|inner| check_requires(inner, resolver, portals)),
    }
}
```

This is a pure extraction for the `None`/`Binary`/`Portal` arms — same logic, same messages, just callable recursively for `All`.

- [ ] **Step 5: Make `probe_binaries` recurse into `All`**

In `crates/wield-core/src/executor.rs`, find `AvailabilityView::probe_binaries`:

```rust
    pub fn probe_binaries(resolver: &BinaryResolver, descriptors: &[Descriptor]) -> Self {
        let binaries = descriptors
            .iter()
            .filter_map(|descriptor| match &descriptor.requires {
                Requires::Binary(binary) if resolver.resolve(binary).is_some() => {
                    Some(binary.clone())
                }
                _ => None,
            })
            .collect();
        Self {
            binaries,
            portals: HashMap::new(),
        }
    }
```

Replace with:

```rust
    pub fn probe_binaries(resolver: &BinaryResolver, descriptors: &[Descriptor]) -> Self {
        let binaries = descriptors
            .iter()
            .flat_map(|descriptor| available_binaries(&descriptor.requires, resolver))
            .collect();
        Self {
            binaries,
            portals: HashMap::new(),
        }
    }
```

And add this free function next to `check_requires`:

```rust
fn available_binaries(requires: &Requires, resolver: &BinaryResolver) -> Vec<String> {
    match requires {
        Requires::Binary(binary) if resolver.resolve(binary).is_some() => vec![binary.clone()],
        Requires::All(all) => all
            .iter()
            .flat_map(|inner| available_binaries(inner, resolver))
            .collect(),
        _ => Vec::new(),
    }
}
```

- [ ] **Step 6: Run `cargo test -p wield-core` and fix the compile error it surfaces**

Run: `cargo test -p wield-core` (the whole crate, not just `executor_pipeline` — this is deliberate: it's how the next gap surfaces).

Expected: **fails to compile** with an E0004 non-exhaustive-match error at `crates/wield-core/src/registry.rs`, inside `Registry::available`. This is real, not a mistake in these steps — `Registry::available` is a *third*, independent site that exhaustively matches on `Requires` (separate from `check_requires` and `available_binaries`, both in `executor.rs`), missed when this plan was first written. It's what actually decides which tools the palette shows as available versus greyed-out, so it genuinely needs `Requires::All` semantics too, not just a placeholder arm to satisfy the compiler.

In `crates/wield-core/src/registry.rs`, find:

```rust
    /// The subset of tools whose `requires` is satisfied by `view`.
    pub fn available<'a>(&'a self, view: &AvailabilityView) -> Vec<&'a Descriptor> {
        self.tools
            .iter()
            .filter(|tool| match &tool.requires {
                Requires::None => true,
                Requires::Binary(binary) => view.binaries.contains(binary),
                Requires::Portal { iface, min_ver } => view
                    .portals
                    .get(iface)
                    .is_some_and(|version| version >= min_ver),
            })
            .collect()
    }
```

Replace with:

```rust
    /// The subset of tools whose `requires` is satisfied by `view`.
    pub fn available<'a>(&'a self, view: &AvailabilityView) -> Vec<&'a Descriptor> {
        self.tools
            .iter()
            .filter(|tool| requires_satisfied(&tool.requires, view))
            .collect()
    }
```

And add this free function near the bottom of the file (after the `impl Registry` block, next to nothing else currently there — it's the first free function in this file):

```rust
/// `All` requires every nested requirement to hold; every other variant is
/// exactly `Registry::available`'s original per-variant check, unchanged.
fn requires_satisfied(requires: &Requires, view: &AvailabilityView) -> bool {
    match requires {
        Requires::None => true,
        Requires::Binary(binary) => view.binaries.contains(binary),
        Requires::Portal { iface, min_ver } => view
            .portals
            .get(iface)
            .is_some_and(|version| version >= min_ver),
        Requires::All(all) => all.iter().all(|inner| requires_satisfied(inner, view)),
    }
}
```

Add a test to `crates/wield-core/tests/registry.rs`, matching the existing `available_filters_on_binaries` test's style (it already imports `Requires` via `use wield_core::descriptor::*;` and constructs descriptors via the file's own `cmd_tool` helper — for this test, build on that helper's output and override `requires` directly, the same way `executor_pipeline.rs`'s tests already do to its own `convert_descriptor` helper):

```rust
#[test]
fn available_requires_every_entry_of_an_all_requirement() {
    let mut r = Registry::new();
    let mut portal_and_binary = cmd_tool("screen.ocr", "tesseract", &[]);
    portal_and_binary.requires = Requires::All(vec![
        Requires::Portal {
            iface: "Screenshot".into(),
            min_ver: 2,
        },
        Requires::Binary("tesseract".into()),
    ]);
    r.register(portal_and_binary).unwrap();

    let mut view = AvailabilityView {
        binaries: Default::default(),
        portals: Default::default(),
    };
    view.binaries.insert("tesseract".into());
    // Portal missing from `view.portals` — only one of the two `All` entries is met.
    assert!(r.available(&view).is_empty());

    view.portals.insert("Screenshot".into(), 2);
    // Now both are met.
    let avail: Vec<_> = r
        .available(&view)
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(avail, vec!["screen.ocr"]);
}
```

Run: `cargo test -p wield-core`
Expected: PASS, everything — this is the point where the whole crate (not just `executor_pipeline.rs`) compiles and tests clean with `Requires::All` fully handled everywhere it's matched.

- [ ] **Step 7: Run `cargo build -p wield-app` and fix the fourth site it surfaces**

`wield-core` alone now compiles clean, but `Requires` is also matched exhaustively one layer up, in the Tauri app itself. Plain `cargo build` from the repo root will **not** catch this — `apps/wield/src-tauri` is a workspace `member` but deliberately excluded from `default-members` (a bare `cargo build` only builds default-members), confirmed live. Build the app crate explicitly instead:

```bash
cargo build -p wield-app
```

Expected: **fails to compile** with another E0004 at `apps/wield/src-tauri/src/capabilities.rs`, inside `is_available`. This function is what actually produces the palette's "greyed out with a reason" text (`ToolAvailability.reason`, shown directly in the UI) — a fourth independent site, missed for the same reason as `registry.rs`.

There's also a related, non-compile-error gap in the same file worth fixing alongside it: `report()`'s loop that populates `CapabilitiesReport.binaries` (a diagnostics map of "which binaries are installed") only checks `if let Requires::Binary(binary) = &descriptor.requires` — a *direct* top-level match, so it silently misses any binary named inside a `Requires::All` (exactly `screen.ocr`'s shape). Not a compile error, so nothing would force this one — worth catching now rather than shipping a diagnostics view that quietly omits `tesseract`.

In `apps/wield/src-tauri/src/capabilities.rs`, find:

```rust
/// Evaluate one descriptor requirement against startup capability data.
pub fn is_available(
    availability: &AvailabilityView,
    requires: &Requires,
) -> (bool, Option<String>) {
    match requires {
        Requires::None => (true, None),
        Requires::Binary(binary) if availability.binaries.contains(binary) => (true, None),
        Requires::Binary(binary) => (false, Some(format!("{binary} is not installed"))),
        Requires::Portal { iface, min_ver } => match availability.portals.get(iface) {
            Some(version) if version >= min_ver => (true, None),
            Some(version) => (
                false,
                Some(format!(
                    "the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"
                )),
            ),
            None => (
                false,
                Some(format!("the {iface} desktop portal is not available")),
            ),
        },
    }
}
```

Replace with (adding one `All` arm — blocks on and reports the first unmet entry, the same short-circuit order `wield-core`'s `check_requires` already uses, so the palette's reason text is consistent with the execution-time gate):

```rust
/// Evaluate one descriptor requirement against startup capability data.
/// `All` reports its first unmet entry, same order as `wield-core`'s own
/// `check_requires` - keeps this UI-facing reason consistent with the
/// execution-time gate.
pub fn is_available(
    availability: &AvailabilityView,
    requires: &Requires,
) -> (bool, Option<String>) {
    match requires {
        Requires::None => (true, None),
        Requires::Binary(binary) if availability.binaries.contains(binary) => (true, None),
        Requires::Binary(binary) => (false, Some(format!("{binary} is not installed"))),
        Requires::Portal { iface, min_ver } => match availability.portals.get(iface) {
            Some(version) if version >= min_ver => (true, None),
            Some(version) => (
                false,
                Some(format!(
                    "the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"
                )),
            ),
            None => (
                false,
                Some(format!("the {iface} desktop portal is not available")),
            ),
        },
        Requires::All(all) => all
            .iter()
            .map(|inner| is_available(availability, inner))
            .find(|(available, _)| !available)
            .unwrap_or((true, None)),
    }
}
```

Then find `report()`'s binary-collection loop:

```rust
            if let Requires::Binary(binary) = &descriptor.requires {
                binaries.insert(binary.clone(), state.availability.binaries.contains(binary));
            }
```

Replace with:

```rust
            for binary in binary_requirements(&descriptor.requires) {
                binaries.insert(binary.clone(), state.availability.binaries.contains(binary));
            }
```

And add this free function below `report()`:

```rust
/// Every `Binary` requirement named anywhere inside `requires`, including
/// nested inside `All` - `report()`'s diagnostics map needs all of them,
/// not just a direct top-level `Requires::Binary`.
fn binary_requirements(requires: &Requires) -> Vec<&String> {
    match requires {
        Requires::Binary(binary) => vec![binary],
        Requires::All(all) => all.iter().flat_map(binary_requirements).collect(),
        _ => Vec::new(),
    }
}
```

This file has no existing tests (checked: no inline `#[cfg(test)]`, no `apps/wield/src-tauri/tests/` file covering it) — add one, matching `state.rs`'s own inline `#[cfg(test)] mod tests` convention (the established pattern in *this* crate, unlike `wield-core`'s separate `tests/*.rs` files):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn availability_with(binaries: &[&str], portals: &[(&str, u32)]) -> AvailabilityView {
        let mut view = AvailabilityView::default();
        for binary in binaries {
            view.binaries.insert((*binary).to_owned());
        }
        for (iface, version) in portals {
            view.portals.insert((*iface).to_owned(), *version);
        }
        view
    }

    fn screen_ocr_requires() -> Requires {
        Requires::All(vec![
            Requires::Portal {
                iface: "Screenshot".into(),
                min_ver: 2,
            },
            Requires::Binary("tesseract".into()),
        ])
    }

    #[test]
    fn all_reports_the_first_unmet_entry() {
        let (available, reason) = is_available(&availability_with(&[], &[]), &screen_ocr_requires());
        assert!(!available);
        assert_eq!(
            reason.as_deref(),
            Some("the Screenshot desktop portal is not available")
        );
    }

    #[test]
    fn all_is_available_only_when_every_entry_is() {
        let view = availability_with(&["tesseract"], &[("Screenshot", 2)]);
        let (available, reason) = is_available(&view, &screen_ocr_requires());
        assert!(available);
        assert!(reason.is_none());
    }

    #[test]
    fn binary_requirements_collects_nested_entries() {
        // Bind first, not `binary_requirements(&screen_ocr_requires())` inline —
        // the inline form borrows into a temporary that's dropped at the end
        // of the statement (E0716).
        let requires = screen_ocr_requires();
        let names: Vec<&str> = binary_requirements(&requires)
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["tesseract"]);
    }
}
```

Run: `cargo build -p wield-app`
Expected: builds cleanly.

Run: `cargo test -p wield-app capabilities` (this crate's package name, confirmed in Task 5 later — filters to the three new tests)
Expected: PASS.

- [ ] **Step 8: Run the `executor_pipeline` tests once more to confirm**

Run: `cargo test -p wield-core --test executor_pipeline`
Expected: PASS, including the three tests from Step 1 and every pre-existing test in the file (the `None`/`Binary`/`Portal` behavior is unchanged, only relocated).

- [ ] **Step 9: Commit**

```bash
git add crates/wield-core/src/descriptor.rs crates/wield-core/src/executor.rs crates/wield-core/src/registry.rs crates/wield-core/tests/executor_pipeline.rs crates/wield-core/tests/registry.rs apps/wield/src-tauri/src/capabilities.rs
git commit -m "wield-core: add Requires::All for tools needing more than one requirement"
```

---

### Task 2: `NativeRunner` trait + `Executor::with_native` wiring

**Files:**
- Create: `crates/wield-core/src/native.rs`
- Modify: `crates/wield-core/src/lib.rs` (register the module, re-export `NativeRunner`, fix a stale doc comment)
- Modify: `crates/wield-core/src/executor.rs` (`Executor` gains a `native` field, `.with_native(...)`, and the `Capability::Native` match arm becomes a real dispatch instead of the hardcoded P4-era stub)
- Test: `crates/wield-core/tests/executor_pipeline.rs`

**Interfaces:**
- Consumes: nothing new from Task 1 — independent addition.
- Produces: `wield_core::native::NativeRunner` trait (also re-exported as `wield_core::NativeRunner`); `Executor::with_native(self, native: Arc<dyn NativeRunner>) -> Self`. Task 3's `wield-native` crate implements this trait; Task 5 wires the impl into `state.rs`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/wield-core/tests/executor_pipeline.rs`, near `portal_capability_is_placeholder_failure`/`injected_portal_runner_receives_the_adapter_key`:

```rust
fn native_descriptor(id: &str) -> Descriptor {
    let mut descriptor = convert_descriptor("sh");
    descriptor.requires = Requires::None;
    descriptor.output = OutputSpec::Value(ValueKind::Text);
    descriptor.capability = Capability::Native {
        id: NativeId(id.into()),
    };
    descriptor
}

struct StubNative {
    outcome: ToolOutcome,
    seen: std::sync::Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl wield_core::NativeRunner for StubNative {
    async fn run(&self, id: &str, _args: &wield_core::ArgMap, _cancel: CancellationToken) -> ToolOutcome {
        *self.seen.lock().unwrap() = Some(id.to_string());
        self.outcome.clone()
    }
}

#[tokio::test]
async fn native_capability_with_no_runner_configured_is_failed() {
    let executor = Executor::new(BinaryResolver::from_env());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: native_descriptor("screen.ocr"),
                args,
            },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(
        outcome,
        ToolOutcome::Failed {
            stage: Stage::Native,
            ..
        }
    ));
}

#[tokio::test]
async fn injected_native_runner_receives_the_native_id() {
    let stub = Arc::new(StubNative {
        outcome: ToolOutcome::Value {
            kind: ValueKind::Text,
            data: "recognized text".into(),
        },
        seen: std::sync::Mutex::new(None),
    });
    let executor = Executor::new(BinaryResolver::from_env()).with_native(stub.clone());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = executor
        .run(
            ExecutionRequest {
                descriptor: native_descriptor("screen.ocr"),
                args,
            },
            tx,
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(outcome, ToolOutcome::Value { .. }));
    assert_eq!(stub.seen.lock().unwrap().as_deref(), Some("screen.ocr"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

`cargo test` accepts only one positional `TESTNAME` filter (same note as
Task 1 Step 2) — these two names share no substring, so run the whole
file instead of filtering:

```bash
cargo test -p wield-core --test executor_pipeline
```

Expected: FAIL to compile — `NativeRunner`, `Executor::with_native` don't exist yet.

- [ ] **Step 3: Create the `NativeRunner` trait**

Create `crates/wield-core/src/native.rs`:

```rust
//! `Native` capability runner — dispatches a descriptor's `NativeId` to its
//! Rust implementation. Mirrors `portal.rs`'s `PortalRunner` exactly: same
//! injection shape into `Executor`, same string-keyed dispatch pattern.

use crate::args::ArgMap;
use crate::outcome::ToolOutcome;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait NativeRunner: Send + Sync {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome;
}
```

- [ ] **Step 4: Register the module and fix the stale doc comment**

In `crates/wield-core/src/lib.rs`, find:

```rust
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
```

Replace with:

```rust
//! `wield-core` — the Wield tool descriptor model, argument validation,
//! argv-template rendering, the `Command`/`Portal`/`Native`-capability
//! executor, and the tool registry.

pub mod args;
pub mod builder;
pub mod command;
pub mod descriptor;
pub mod error;
pub mod executor;
pub mod native;
pub mod outcome;
pub mod portal;
pub mod registry;
pub mod template;
pub mod validate;
```

Then find the `pub use portal::PortalRunner;` line further down and add a matching re-export directly after it:

```rust
pub use portal::PortalRunner;
pub use native::NativeRunner;
```

(Exact placement: alongside the other `pub use` lines already there — `args`, `builder`, `command`, `descriptor`, `error`, `executor`, `outcome`, `portal`, `registry`, `template`, `validate` re-exports. `native`'s goes next to `portal`'s since they're the same kind of thing.)

- [ ] **Step 5: Wire `Executor::with_native` and the real dispatch**

In `crates/wield-core/src/executor.rs`:

Add `native: Option<Arc<dyn crate::native::NativeRunner>>` to the `Executor` struct (next to the existing `portal: Option<Arc<dyn crate::portal::PortalRunner>>` field), initialize it to `None` in `Executor::new`, and add a builder method next to `with_portal`:

```rust
    pub fn with_native(mut self, native: Arc<dyn crate::native::NativeRunner>) -> Self {
        self.native = Some(native);
        self
    }
```

Also add `.field("native", &self.native.is_some())` to the manual `Debug` impl, next to the existing `.field("portal", &self.portal.is_some())` line.

Then replace the stub match arm:

```rust
            Capability::Native { .. } => ToolOutcome::Failed {
                stage: Stage::Native,
                detail: "native execution is implemented in P4".to_owned(),
                hint: None,
            },
```

with:

```rust
            Capability::Native { id } => match &self.native {
                Some(runner) => runner.run(&id.0, &effective, cancel).await,
                None => ToolOutcome::Failed {
                    stage: Stage::Native,
                    detail: "this build has no native runner configured".to_owned(),
                    hint: None,
                },
            },
```

(`id.0` is `NativeId`'s single tuple field, a `String` — matches how `Capability::Portal { adapter }` already passes `adapter` by reference into `runner.run(adapter, ...)`.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p wield-core --test executor_pipeline`
Expected: PASS, including the two new tests.

Also run: `cargo build -p wield-core` and `cargo test -p wield-core` to confirm the doc-comment/lib.rs edit didn't break anything else.

- [ ] **Step 7: Commit**

```bash
git add crates/wield-core/src/native.rs crates/wield-core/src/lib.rs crates/wield-core/src/executor.rs crates/wield-core/tests/executor_pipeline.rs
git commit -m "wield-core: real Native capability dispatch via a NativeRunner trait"
```

---

### Task 3: New `wield-native` crate scaffold

**Files:**
- Create: `crates/wield-native/Cargo.toml`
- Create: `crates/wield-native/src/lib.rs`
- Create: `crates/wield-native/src/runner.rs`
- Modify: root `Cargo.toml` (workspace `members`/`default-members`, and a new `url` entry in `[workspace.dependencies]`)

**Interfaces:**
- Consumes: `wield_core::NativeRunner` (Task 2).
- Produces: `wield_native::NativeToolRunner` (a `NativeRunner` impl that returns `Failed` for any unrecognized id — Task 5 adds the real `"screen.ocr"` arm once Task 4's pipeline function exists). `wield_native::tools` module (empty until Task 4).

- [ ] **Step 1: Add workspace wiring**

In the root `Cargo.toml`, find:

```toml
[workspace]
members = [
  "crates/wield-core",
  "crates/wield-portal",
  "crates/wield-tools",
  "crates/wield-cli",
  "apps/wield/src-tauri"
]
default-members = [
  "crates/wield-core",
  "crates/wield-portal",
  "crates/wield-tools",
  "crates/wield-cli"
]
```

Replace with:

```toml
[workspace]
members = [
  "crates/wield-core",
  "crates/wield-native",
  "crates/wield-portal",
  "crates/wield-tools",
  "crates/wield-cli",
  "apps/wield/src-tauri"
]
default-members = [
  "crates/wield-core",
  "crates/wield-native",
  "crates/wield-portal",
  "crates/wield-tools",
  "crates/wield-cli"
]
```

Then, in the same file's `[workspace.dependencies]` block, add one new line — alphabetically after `uuid` and before `tokio` isn't alphabetical in the existing file already (it's grouped loosely, not strictly sorted), so add it directly after the existing `uuid = { version = "1", features = ["v4", "serde"] }` line:

```toml
url = "2.5"
```

(Verified live: `url 2.5.8` already resolves in `Cargo.lock` transitively through `ashpd`/`zbus` — this pins the same major/minor explicitly as a direct dependency rather than relying on that transitive edge, per the design spec §8.)

- [ ] **Step 2: Create the crate manifest**

Create `crates/wield-native/Cargo.toml`:

```toml
[package]
name = "wield-native"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
wield-core = { path = "../wield-core" }
wield-portal = { path = "../wield-portal" }
async-trait.workspace = true
ashpd.workspace = true
tokio.workspace = true
tokio-util.workspace = true
tempfile.workspace = true
url.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

Note: `tempfile` is a real (non-dev) dependency here, not dev-only like every other crate in the workspace so far — `screen_ocr.rs`'s production code creates a real scratch directory for `tesseract`'s output (Task 4), it isn't test-only here.

`wield-portal` is a dependency so `wield-native` can reuse `PortalError`'s existing `Display`/`into_outcome` mapping rather than duplicating that small enum (design spec §8) — this is a one-directional edge (`wield-portal` has no dependency on `wield-native`), not a cycle.

- [ ] **Step 3: Create the crate root**

Create `crates/wield-native/src/lib.rs`:

```rust
//! `wield-native` — the `Native`-capability tool implementations. Implements
//! `wield_core::NativeRunner`, the same shape `wield_portal::PortalRunner`
//! uses for `Portal`-capability tools.

pub mod runner;
pub mod tools;

pub use runner::NativeToolRunner;
```

- [ ] **Step 4: Create the tools module placeholder**

Create `crates/wield-native/src/tools/mod.rs`:

```rust
//! One module per native tool, dispatched by id in `runner.rs`.
```

(Task 4 adds `pub mod screen_ocr;` here.)

- [ ] **Step 5: Create the dispatcher**

Create `crates/wield-native/src/runner.rs`:

```rust
//! Dispatches a descriptor's `NativeId` to its implementation. Mirrors
//! `wield-portal/src/adapters/mod.rs`'s `dispatch` function exactly.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, NativeRunner, Stage, ToolOutcome};

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeToolRunner;

#[async_trait]
impl NativeRunner for NativeToolRunner {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
        match id {
            other => ToolOutcome::Failed {
                stage: Stage::Native,
                detail: format!("unknown native tool: {other}"),
                hint: Some("this is a bug in the tool descriptor".to_owned()),
            },
        }
    }
}
```

(`args`/`cancel` are intentionally unused by this catch-all-only version — Task 5 adds the real `"screen.ocr" => tools::screen_ocr::run(args, cancel).await,` arm above the fallback, which will use both. Rust will warn on unused parameters here; that's expected and resolved by Task 5, not worth suppressing.)

- [ ] **Step 6: Write and run a test for the fallback**

Add to `crates/wield-native/src/runner.rs`, below the impl:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_id_is_a_clear_failure() {
        let runner = NativeToolRunner;
        let args = std::collections::BTreeMap::new();
        let outcome = runner.run("not.a.real.tool", &args, CancellationToken::new()).await;
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Native,
                ..
            }
        ));
    }
}
```

Run: `cargo test -p wield-native`
Expected: PASS.

- [ ] **Step 7: Confirm the whole workspace still builds**

Run: `cargo build` (from the repo root)
Expected: builds cleanly — `wield-native` compiles and is now a real workspace member, unused by anything else yet.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/wield-native
git commit -m "wield-native: new crate scaffold for Native-capability tools"
```

---

### Task 4: The `screen.ocr` OCR pipeline

**Files:**
- Create: `crates/wield-native/src/tools/screen_ocr.rs`
- Modify: `crates/wield-native/src/tools/mod.rs` (add `pub mod screen_ocr;`)

**Interfaces:**
- Consumes: `wield_portal::PortalError` (its `Display`/`into_outcome`), `wield_core::command::{CommandRunner, CommandResult, RunSpec}`, `wield_core::descriptor::{ProgressSpec, SuccessSpec}`, `wield_core::outcome::{Stage, ToolOutcome}`, `wield_core::ValueKind` (re-exported at the crate root — it's defined in `descriptor.rs`, not `outcome.rs`, so `wield_core::outcome::ValueKind` doesn't exist), `wield_core::ArgMap`.
- Produces: `pub async fn run(args: &ArgMap, cancel: CancellationToken) -> ToolOutcome` — the function Task 5 wires into `NativeToolRunner`'s dispatch.

- [ ] **Step 1: Write the failing tests**

Create `crates/wield-native/src/tools/screen_ocr.rs` with just its test module first (the real implementation comes in Step 3):

```rust
//! `screen.ocr` — Screenshot portal capture, then `tesseract` recognition.

#[cfg(test)]
mod tests {
    use super::*;
    use wield_core::{Stage, ToolOutcome, ValueKind};
    use wield_portal::PortalError;

    #[tokio::test]
    async fn cancel_token_wins_over_a_pending_capture() {
        let token = CancellationToken::new();
        token.cancel();
        let outcome = run_with(
            std::future::pending::<Result<PathBuf, PortalError>>(),
            |_path, _cancel| async { unreachable!("recognize must not run") },
            token,
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn dismissed_portal_dialog_is_cancelled() {
        let outcome = run_with(
            async { Err(PortalError::Cancelled) },
            |_path, _cancel| async { unreachable!("recognize must not run") },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn portal_transport_error_is_failed_native() {
        let outcome = run_with(
            async { Err(PortalError::Transport("boom".into())) },
            |_path, _cancel| async { unreachable!("recognize must not run") },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Portal,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn happy_path_trims_and_returns_the_recognized_text() {
        let outcome = run_with(
            async { Ok(PathBuf::from("/tmp/does-not-need-to-exist.png")) },
            |_path, _cancel| async { ToolOutcome::Value {
                kind: ValueKind::Text,
                data: "recognized text".into(),
            } },
            CancellationToken::new(),
        )
        .await;
        match outcome {
            ToolOutcome::Value { kind: ValueKind::Text, data } => {
                assert_eq!(data, "recognized text");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn empty_stdout_is_a_clear_failure_not_a_silent_empty_value() {
        let outcome = text_outcome("   \n  ");
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Native,
                ..
            }
        ));
    }

    #[test]
    fn non_empty_stdout_is_trimmed_into_a_value() {
        let outcome = text_outcome("  hello world  \n");
        match outcome {
            ToolOutcome::Value { kind: ValueKind::Text, data } => assert_eq!(data, "hello world"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p wield-native --lib`
Expected: FAIL to compile — `run_with`, `text_outcome`, `PathBuf`, `CancellationToken` aren't imported/defined in the module yet.

- [ ] **Step 3: Implement the pipeline**

Replace the top of `crates/wield-native/src/tools/screen_ocr.rs` (everything above `#[cfg(test)]`) with:

```rust
//! `screen.ocr` — Screenshot portal capture, then `tesseract` recognition.
//!
//! Capture and recognition are each behind their own small async function so
//! tests can substitute a fake for either independently, mirroring
//! `wield-portal/src/adapters/pick_color.rs`'s `pick_color_with`.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::args::ArgMap;
use wield_core::command::{CommandResult, CommandRunner, RunSpec};
use wield_core::descriptor::{ProgressSpec, SuccessSpec};
use wield_core::outcome::{Stage, ToolOutcome};
use wield_core::ValueKind;
use wield_portal::PortalError;

const TESSERACT_TIMEOUT_SECS: u64 = 30;

/// Entry point wired into `NativeToolRunner`'s dispatch (Task 5).
pub async fn run(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
    run_with(capture_screenshot(), recognize_text, cancel).await
}

async fn run_with<C, R, RFut>(capture: C, recognize: R, cancel: CancellationToken) -> ToolOutcome
where
    C: std::future::Future<Output = Result<PathBuf, PortalError>>,
    R: FnOnce(PathBuf, CancellationToken) -> RFut,
    RFut: std::future::Future<Output = ToolOutcome>,
{
    let screenshot_path = tokio::select! {
        biased;
        _ = cancel.cancelled() => return ToolOutcome::Cancelled,
        result = capture => match result {
            Ok(path) => path,
            Err(error) => return error.into_outcome(),
        },
    };

    let outcome = recognize(screenshot_path.clone(), cancel).await;
    // Best-effort: the portal owns this file, not us. A leftover temp
    // screenshot the user never sees is not worth failing the run over.
    let _ = std::fs::remove_file(&screenshot_path);
    outcome
}

async fn capture_screenshot() -> Result<PathBuf, PortalError> {
    let request = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(true)
        .modal(true)
        .send()
        .await
        .map_err(map_ashpd_error)?;
    let screenshot = request.response().map_err(map_ashpd_error)?;
    let url = url::Url::parse(screenshot.uri().as_str())
        .map_err(|error| PortalError::BadResponse(error.to_string()))?;
    url.to_file_path()
        .map_err(|_| PortalError::BadResponse(format!("unexpected screenshot URI: {url}")))
}

fn map_ashpd_error(error: ashpd::Error) -> PortalError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => PortalError::Cancelled,
        other => PortalError::Transport(other.to_string()),
    }
}

async fn recognize_text(image: PathBuf, cancel: CancellationToken) -> ToolOutcome {
    let scratch = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => {
            return ToolOutcome::Failed {
                stage: Stage::Native,
                detail: format!("could not create a scratch directory: {error}"),
                hint: None,
            }
        }
    };
    // tesseract appends `.txt` to whatever outputbase it's given.
    let output_base = scratch.path().join("ocr");
    let argv = vec![
        image.display().to_string(),
        output_base.display().to_string(),
        "-l".to_string(),
        "eng".to_string(),
    ];
    let (progress_tx, _progress_rx) = mpsc::channel(1);
    let result = CommandRunner::execute(RunSpec {
        binary: Path::new("tesseract"),
        argv: &argv,
        cwd: None,
        output: None,
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(TESSERACT_TIMEOUT_SECS),
        progress: progress_tx,
        cancel,
    })
    .await;

    match result {
        CommandResult::Success { .. } => {
            let text_path = output_base.with_extension("txt");
            let text = std::fs::read_to_string(&text_path).unwrap_or_default();
            let _ = std::fs::remove_file(&text_path);
            text_outcome(&text)
        }
        CommandResult::SpawnFailed { detail } => ToolOutcome::Unavailable {
            reason: format!("tesseract could not be started: {detail}"),
            fix: Some("install tesseract or add it to your PATH".to_owned()),
        },
        CommandResult::NonZeroExit { code, stderr_tail } => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: format!("tesseract exited with {code:?}: {stderr_tail}"),
            hint: None,
        },
        CommandResult::Timeout => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "tesseract timed out".to_owned(),
            hint: None,
        },
        CommandResult::Cancelled => ToolOutcome::Cancelled,
        CommandResult::OutputMissing => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "tesseract did not produce a text file".to_owned(),
            hint: None,
        },
    }
}

/// Maps raw (untrimmed) tesseract stdout-file content to the final outcome.
/// A blank result is a clear failure, not a silent empty-string success —
/// a "Copy" button that copies nothing is worse than an explicit message.
fn text_outcome(raw: &str) -> ToolOutcome {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "no text was found in the selected region".to_owned(),
            hint: Some("try a region with clearer text or more contrast".to_owned()),
        }
    } else {
        ToolOutcome::Value {
            kind: ValueKind::Text,
            data: trimmed.to_owned(),
        }
    }
}
```

In `crates/wield-native/src/tools/mod.rs`, add:

```rust
pub mod screen_ocr;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p wield-native --lib`
Expected: PASS — all six tests (`cancel_token_wins_over_a_pending_capture`, `dismissed_portal_dialog_is_cancelled`, `portal_transport_error_is_failed_native`, `happy_path_trims_and_returns_the_recognized_text`, `empty_stdout_is_a_clear_failure_not_a_silent_empty_value`, `non_empty_stdout_is_trimmed_into_a_value`).

- [ ] **Step 5: Live-verify against the real `tesseract` binary**

This step has no automated test — it's a direct sanity check that the argv/temp-file mechanics work against the real binary before wiring this into the app (the same live-verification discipline `command.rs`'s `FfmpegDuration` parser doc comment models). From the repo root:

```bash
mkdir -p /tmp/wield-ocr-check && \
convert -size 400x60 xc:white -font /usr/share/fonts/liberation/LiberationSans-Bold.ttf \
  -pointsize 28 -fill black -annotate +10+40 "Hello Wield OCR" /tmp/wield-ocr-check/in.png && \
tesseract /tmp/wield-ocr-check/in.png /tmp/wield-ocr-check/ocr -l eng && \
cat /tmp/wield-ocr-check/ocr.txt && \
rm -rf /tmp/wield-ocr-check
```

Expected output includes `Hello Wield OCR`. (If ImageMagick's `convert` isn't available in this environment, any other way of producing a `.png` with clear text works — the point is confirming `tesseract <image> <output_base> -l eng` really does write `<output_base>.txt`, which `recognize_text`'s `output_base.with_extension("txt")` depends on.)

- [ ] **Step 6: Commit**

```bash
git add crates/wield-native/src/tools/screen_ocr.rs crates/wield-native/src/tools/mod.rs
git commit -m "wield-native: screen.ocr pipeline (Screenshot portal capture + tesseract)"
```

---

### Task 5: Wire `screen.ocr` end-to-end

**Files:**
- Modify: `crates/wield-native/src/runner.rs` (real dispatch arm)
- Create: `crates/wield-tools/src/screen_ocr.rs`
- Modify: `crates/wield-tools/src/lib.rs` (register the module + the descriptor)
- Modify: `crates/wield-tools/Cargo.toml` (needs `wield-native`? — see note below; actually not needed, see Step 3)
- Modify: `apps/wield/src-tauri/src/state.rs` (both `.with_portal(...)` call sites gain `.with_native(...)`)
- Modify: `apps/wield/src-tauri/Cargo.toml` (new `wield-native` dependency)

**Interfaces:**
- Consumes: `wield_native::tools::screen_ocr::run` (Task 4), `wield_core::{Requires, DescriptorBuilder}` (Task 1 for `Requires::All`).
- Produces: the fully working `screen.ocr` tool, registered in `builtin_registry()` and reachable from the running app.

- [ ] **Step 1: Wire the real dispatch arm**

In `crates/wield-native/src/runner.rs`, replace:

```rust
        match id {
            other => ToolOutcome::Failed {
```

with:

```rust
        match id {
            "screen.ocr" => crate::tools::screen_ocr::run(args, cancel).await,
            other => ToolOutcome::Failed {
```

(This also resolves the "unused `args`/`cancel`" warning from Task 3, Step 5's note.)

Run: `cargo test -p wield-native` — expect the existing `unknown_id_is_a_clear_failure` test to still pass (it uses an id that still falls through to the `other` arm).

- [ ] **Step 2: Write the `screen.ocr` descriptor**

Create `crates/wield-tools/src/screen_ocr.rs`:

```rust
//! The `screen.ocr` built-in — capture a screen region via the Screenshot
//! portal, then recognize text in it via `tesseract`. `Native` capability:
//! the first built-in that isn't `Command` or `Portal`.

use wield_core::{Category, Descriptor, DescriptorBuilder, OutputSpec, Requires, ValueKind};

/// Descriptor for `screen.ocr`. Needs both the Screenshot portal (capture)
/// and the `tesseract` binary (recognition) — `Requires::All` lets the
/// palette grey this out proactively if either is missing, not just one.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("screen.ocr", "Extract text from screen", Category::Capture)
        .keywords(&["ocr", "text", "extract", "screenshot", "recognize"])
        .requires(Requires::All(vec![
            Requires::Portal {
                iface: "Screenshot".into(),
                min_ver: 2,
            },
            Requires::Binary("tesseract".into()),
        ]))
        .output(OutputSpec::Value(ValueKind::Text))
        .native("screen.ocr")
        .build()
        .expect("screen.ocr descriptor is valid")
}
```

- [ ] **Step 3: Register it in `builtin_registry()`**

In `crates/wield-tools/src/lib.rs`, add `pub mod screen_ocr;` to the module list (alongside `pdf_split`/`video_convert` — keep the list alphabetical, matching the existing order) and add its registration to `builtin_registry()`:

```rust
    registry
        .register(screen_ocr::descriptor())
        .expect("screen.ocr registers");
```

(Insert alphabetically among the existing `.register(...)` calls, matching the file's existing convention — after `pdf_split`, before `video_convert`.)

`wield-tools/Cargo.toml` needs **no change** — the descriptor only references `wield_core` types (`Requires::All`, `.native(...)`), never `wield-native` itself. `wield-tools` stays descriptor-only, per its established convention (design spec §3.1) — execution logic (which crate actually runs `screen.ocr`) is `state.rs`'s concern, wired directly to `wield-native`, not routed through `wield-tools`.

- [ ] **Step 4: Confirm the descriptor builds and registers cleanly**

Run: `cargo test -p wield-tools`
Expected: PASS — this exercises the workspace's existing descriptor snapshot/registration test, which now covers `screen.ocr` too since `builtin_registry()` includes it unconditionally.

- [ ] **Step 5: Wire `wield-native` into the app**

In `apps/wield/src-tauri/Cargo.toml`, find:

```toml
wield-core = { path = "../../../crates/wield-core" }
wield-tools = { path = "../../../crates/wield-tools" }
wield-portal = { path = "../../../crates/wield-portal" }
```

and add a fourth line:

```toml
wield-native = { path = "../../../crates/wield-native" }
```

In `apps/wield/src-tauri/src/state.rs`, `AppState::build()`:

```rust
        let executor = Executor::new(resolver)
            .with_portal(Arc::new(wield_portal::PortalAdapterRunner))
            .with_availability(availability.clone());
```

becomes:

```rust
        let executor = Executor::new(resolver)
            .with_portal(Arc::new(wield_portal::PortalAdapterRunner))
            .with_native(Arc::new(wield_native::NativeToolRunner))
            .with_availability(availability.clone());
```

And the `#[cfg(test)] pub(crate) async fn for_test(...)` helper gets the identical addition:

```rust
        let executor = Executor::new(resolver)
            .with_portal(Arc::new(wield_portal::PortalAdapterRunner))
            .with_native(Arc::new(wield_native::NativeToolRunner))
            .with_availability(availability.clone());
```

The third constructor (`empty_state()` in the test module, which builds a bare `Executor::new(...)` with no portal or native configured) is left untouched — it's deliberately the "nothing configured" case for `cancel_run` tests unrelated to execution.

- [ ] **Step 6: Run the full app-side test suite**

Run: `cargo test -p wield-app` (confirmed package name, from `apps/wield/src-tauri/Cargo.toml`'s `[package] name = "wield-app"`)
Expected: PASS, including the existing `build_produces_the_builtin_registry` test — extend it:

```rust
    #[tokio::test]
    async fn build_produces_the_builtin_registry() {
        let state = AppState::build().await;
        assert!(state.registry.get("image.convert").is_some());
        assert!(state.registry.get("color.pick").is_some());
        assert!(state.registry.get("screen.ocr").is_some());
    }
```

- [ ] **Step 7: Run the whole workspace's tests**

Plain `cargo test` only covers `default-members`, which excludes `apps/wield/src-tauri` (confirmed live during Task 1) — use `--workspace` explicitly so nothing is silently skipped:

```bash
cargo test --workspace
```

Expected: PASS, everything — this is the first point where all five crates (`wield-core`, `wield-native`, `wield-portal`, `wield-tools`, `apps/wield/src-tauri`) build and test together with `screen.ocr` fully wired.

- [ ] **Step 8: Commit**

```bash
git add crates/wield-native/src/runner.rs crates/wield-tools/src/screen_ocr.rs crates/wield-tools/src/lib.rs apps/wield/src-tauri/Cargo.toml apps/wield/src-tauri/src/state.rs
git commit -m "screen.ocr: wire the Native pipeline into the built-in registry and app state"
```

---

### Task 6: Docs + live walkthrough verification

**Files:**
- Modify: `docs/testing.md` (new M3a section, matching the existing per-milestone entries for M2a-d)

**Interfaces:** none — this task is verification and documentation only, no code changes.

- [ ] **Step 1: Rebuild and reinstall the real binary**

From the repo root:

```bash
npm run build -w apps/wield -- --no-bundle
```

Then follow the project's established rebuild/reinstall/relaunch cycle: confirm no stale `wield-app` process is running (`ps aux | grep wield-app | grep -v grep`), copy the fresh binary to `~/.local/bin/wield-app`, relaunch it, and confirm the new process owns the `io.github.DhanushSantosh.Wield` D-Bus name.

- [ ] **Step 2: Live walkthrough — happy path**

Per the standing UI-walkthrough-testing requirement (verify live via computer-use, not code-reasoning alone): open the palette, search for "screen.ocr" (or its title, "Extract text from screen"), select it, confirm the interactive Screenshot-portal region picker appears, select a region containing real on-screen text, confirm the recognized text appears as a `Value` result with a working "Copy" button, and confirm clicking Copy actually places the text on the clipboard (paste it somewhere to check).

- [ ] **Step 3: Live walkthrough — empty region**

Run `screen.ocr` again, this time selecting a region with no text in it (e.g. a blank part of the desktop background). Confirm the result is the "no text was found in the selected region" failure message, not a silent empty success.

- [ ] **Step 4: Live walkthrough — cancel**

Run `screen.ocr` again and dismiss the portal's region picker without selecting anything (Escape, or whatever the compositor's own cancel gesture is). Confirm Wield returns to its normal idle state without showing an error — a dismissed capture is `ToolOutcome::Cancelled`, not `Failed`.

- [ ] **Step 5: Write up the verification in `docs/testing.md`**

Add a new section to `docs/testing.md`, in the same style as the existing M2a-d sections (read the file first to match its exact heading level and prose style), covering: what was tested (the three walkthroughs above), what was verified live vs. only unit-tested (the `Requires::All` gating itself wasn't separately walked through live since there's no easy way to uninstall `tesseract` mid-session to trigger it — noted as a gap, not silently skipped), and the `tesseract 5.5.3` / `eng`+`osd` traineddata baseline this was verified against.

- [ ] **Step 6: Commit**

```bash
git add docs/testing.md
git commit -m "docs(testing): live-verify screen.ocr's happy path, empty region, and cancel"
```

---

## After all tasks: PR

Per this project's standing workflow, GEON opens the PR from `feat/m3a-screen-ocr` against `master` after reviewing every commit against this plan — do not merge without the user's explicit "merge it" for this specific PR, even under broad delegation.

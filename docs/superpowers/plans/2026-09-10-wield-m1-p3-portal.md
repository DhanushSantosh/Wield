# Wield M1 · Plan P3 — wield-portal: capability probe & the pick_color adapter — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `wield-portal` crate — a thin `ashpd`/`zbus` wrapper providing the startup capability probe (which XDG portals and versions are present) and the `screenshot.pick_color` adapter — and wire it into the `wield-core` executor through an injected `PortalRunner` trait, so a `Capability::Portal` tool runs end to end.

**Architecture:** `wield-core` gains a `PortalRunner` trait (async, object-safe via `async-trait`) and the `Executor` gains an optional `Arc<dyn PortalRunner>` plus an `AvailabilityView` it checks for `Requires::Portal`. `wield-core` still depends on nothing portal-specific. `wield-portal` depends on `wield-core`, implements `PortalRunner` as `PortalAdapterRunner` (a key → adapter dispatch), and provides `probe()` — raw `zbus` introspection of `org.freedesktop.portal.Desktop` returning an interface→version map. The one v1 adapter, `screenshot.pick_color`, calls `ashpd`'s Screenshot portal and maps the result to `ToolOutcome::Value { kind: Color, .. }`, with user-dismissed dialogs returning `ToolOutcome::Cancelled` (silent). Tests use a private `dbus-daemon` with a fake `org.freedesktop.portal.Desktop` service.

**Tech Stack:** Rust 2021, `ashpd` 0.13 (`default-features = false`, `tokio`), `zbus` 5 (`tokio`), `zbus_xml` 5 (introspection XML), `async-trait` 0.1, Tokio, `tokio-util` `CancellationToken`, `serde_json` (Color value payload); `dbus-daemon` + `zbus` server for tests.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` — implements §3 (`wield-portal` crate), §4 (`Portal` capability, adapter-by-key), §5 (capability probe), §7 (portal error handling, silent cancel), and the `wield-portal` + portal-path portions of §8.

## Status — complete (2026-09-10)

- Implemented Tasks 1–8 on `feat/p3-portal`; the private-bus PickColor mock is working, so the manual-matrix fallback was not used.
- Verified `cargo fmt --all`, strict workspace clippy, all workspace tests, ignored real-portal test registration, `npm run check`, and the `wield-cli` smoke test.
- Mechanical corrections applied: enabled ashpd's feature-gated `screenshot` API; made `probe_on` public so an external integration test can call it; explicitly exposed the XDG lowercase `version` property in the fake service; ran the invalid two-filter Cargo test example as a library test; and retained the generated lockfile feature update.

---

## GEON amendment — 2026-09-10 (owner design decisions)

Locked with the owner before this plan was written:

1. **Inject a `PortalRunner` trait.** `wield-core` defines `trait PortalRunner`; `Executor` holds an `Option<Arc<dyn PortalRunner>>` set via a builder method. `wield-portal` depends on `wield-core` and implements the trait. `wield-core` does **not** depend on `wield-portal` or `ashpd`/`zbus`. Object-safety via `async-trait`.
2. **Raw `zbus` introspection for the probe.** `probe()` calls `Introspect` on `org.freedesktop.portal.Desktop` and reads each `org.freedesktop.portal.*` interface's `version` property directly. No per-portal code path, no `ashpd` typed proxies for the probe.

Additional constraints from GEON:

- **Scope:** `wield-portal` crate + the `wield-core` injection seam only. The one adapter is `screenshot.pick_color`. **No** `screenshot.region`, **no** `inhibit.toggle` (those arrive with their tools in M3). **No** `wield-tools`, **no** real `color.pick` descriptor (P4), **no** Tauri wiring (P5). Tests use a fixture Portal descriptor.
- **Branch:** `feat/p3-portal` off `master`, per-task commits, push at the end, do **not** open the PR (GEON does, after review).
- **`cargo` is not on the default `PATH`.** Every shell that runs `cargo` must first `export PATH="$HOME/.cargo/bin:$PATH"` (rustup stable 1.98.1).
- **Mechanical-correction rule (as in P2):** obvious compile-error / import / lifetime / API-signature fixes to the snippets below — apply and note them in the Task 8 report. Stop-and-ask only for genuine design ambiguity.
- **`npm run check`** green at the end of every task that touches buildable code.
- The mock-portal Task 7 has a **documented fallback** — read it before starting Task 7.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p3-portal` off `master`).
- **Crates touched:** `crates/wield-core` (trait + executor seam), `crates/wield-portal` (everything else), root `Cargo.toml` (workspace deps).
- **Rust edition:** 2021, from `[workspace.package]`.
- **No `unwrap()` / `expect()` / `panic!` in library code** outside `#[cfg(test)]`. Portal failures become `ToolOutcome`, probe failures become an empty/partial map (never a panic — "Wield never blocks startup on a missing capability", spec §5).
- **User-cancel is silent** (spec §7): a dismissed portal dialog → `ToolOutcome::Cancelled`, never `Failed`.
- **`wield-core` stays portal-free:** no `ashpd`, `zbus`, or `wield-portal` in `crates/wield-core/Cargo.toml`. Only `async-trait` is added there.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified — `crates/wield-core/`:**

| File | Change |
| --- | --- |
| `Cargo.toml` | add `async-trait.workspace = true` |
| `src/portal.rs` | **new** — `PortalRunner` trait |
| `src/executor.rs` | `Executor` gains `portal: Option<Arc<dyn PortalRunner>>` + `availability: AvailabilityView`; builder methods `with_portal` / `with_availability`; `Requires::Portal` checks `availability`; `Capability::Portal` arm dispatches to the runner; manual `Debug` impl |
| `src/lib.rs` | `pub mod portal;` + `pub use portal::PortalRunner;` |
| `tests/executor_pipeline.rs` | add tests: injected stub runner routes; no runner → placeholder `Failed`; `Requires::Portal` unmet → `Unavailable` |

**Created — `crates/wield-portal/`:**

| File | Responsibility |
| --- | --- |
| `Cargo.toml` | deps: `wield-core` (path), `ashpd`, `zbus`, `zbus_xml`, `async-trait`, `tokio`, `tokio-util`, `serde_json` |
| `src/lib.rs` | module decls + public re-exports (`PortalAdapterRunner`, `probe`, `PortalMap`, `PortalError`) |
| `src/error.rs` | `PortalError` + `to_outcome` mapping (incl. the cancel→`Cancelled` rule) |
| `src/probe.rs` | `PortalMap`, `parse_portal_versions(xml) -> PortalMap`, `async fn probe() -> PortalMap` (session bus), `PortalMap::into_availability()` |
| `src/color.rs` | `Rgb` (0–255) + `hex()` / `rgb_string()` / `hsl_string()` / `value_payload() -> String` (JSON) |
| `src/adapters/mod.rs` | `pub(crate) async fn dispatch(adapter, args, cancel) -> ToolOutcome` |
| `src/adapters/pick_color.rs` | `pick_color(args, cancel)` + `pick_color_with(fut, cancel)` (the testable core) |
| `src/runner.rs` | `PortalAdapterRunner` implementing `wield_core::PortalRunner` |
| `tests/support/mod.rs` | `PrivateBus` (launch a private `dbus-daemon`), `serve_fake_portal(...)` helper |
| `tests/probe.rs` | unit: canned Introspect XML → `PortalMap`; integration: fake introspectable service → `probe()` |
| `tests/pick_color.rs` | unit: color math + `pick_color_with` outcome/cancel mapping; integration (Task 7): fake Screenshot portal → `pick_color()` → `Value` |
| `tests/runner.rs` | `PortalAdapterRunner`: known key routes, unknown key → `Failed { stage: Portal }` |

**Out of P3 scope (do not add):** `screenshot.region`, `inhibit.toggle`, any `wield-tools` descriptor, the real `color.pick`, Tauri commands, `ScreenCast`/PipeWire, the `GlobalShortcuts` registration (P5), a real portal in CI (manual matrix per spec §8).

---

## Task 1: Workspace deps + `wield-portal` skeleton

**Files:**
- Modify: `Cargo.toml` (root), `crates/wield-core/Cargo.toml`, `crates/wield-portal/Cargo.toml`, `crates/wield-portal/src/lib.rs`
- Create: `crates/wield-portal/src/{error,probe,color,runner}.rs`, `crates/wield-portal/src/adapters/{mod,pick_color}.rs`

**Interfaces:**
- Produces: a compiling `wield-portal` with empty modules; workspace deps available.

- [x] **Step 1: Add workspace dependencies**

Extend root `Cargo.toml` `[workspace.dependencies]`:

```toml
async-trait = "0.1"
ashpd = { version = "0.13", default-features = false, features = ["tokio"] }
zbus = { version = "5", default-features = false, features = ["tokio"] }
zbus_xml = "5"
```

(`serde_json`, `tokio`, `tokio-util` are already there from P2.)

- [x] **Step 2: `crates/wield-core/Cargo.toml` — add one dep**

Under `[dependencies]` add:

```toml
async-trait.workspace = true
```

- [x] **Step 3: `crates/wield-portal/Cargo.toml`**

```toml
[package]
name = "wield-portal"
version = "0.1.0"
edition.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
wield-core = { path = "../wield-core" }
async-trait.workspace = true
ashpd.workspace = true
zbus.workspace = true
zbus_xml.workspace = true
tokio.workspace = true
tokio-util.workspace = true
serde_json.workspace = true

[dev-dependencies]
tempfile.workspace = true
tokio = { workspace = true, features = ["test-util", "process"] }
```

- [x] **Step 4: Replace `crates/wield-portal/src/lib.rs`**

```rust
//! `wield-portal` — the startup capability probe and the XDG Desktop Portal
//! adapters Wield uses. Implements `wield_core::PortalRunner`.

pub mod adapters;
pub mod color;
pub mod error;
pub mod probe;
pub mod runner;

pub use error::PortalError;
pub use probe::{probe, PortalMap};
pub use runner::PortalAdapterRunner;
```

- [x] **Step 5: Create stub module files**

`src/error.rs`, `src/probe.rs`, `src/color.rs`, `src/runner.rs`, `src/adapters/mod.rs`, `src/adapters/pick_color.rs` — each just a `//! …` doc line. `src/adapters/mod.rs` also needs `pub mod pick_color;`.

- [x] **Step 6: Verify it builds**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo build -p wield-portal`
Expected: PASS (unused-module warnings are fine).

- [x] **Step 7: Commit**

```bash
git checkout -b feat/p3-portal
git add Cargo.toml crates/wield-core/Cargo.toml crates/wield-portal
git commit -m "chore(portal): add P3 dependencies and wield-portal skeleton"
```

---

## Task 2: `PortalRunner` trait + executor injection

**Files:**
- Create: `crates/wield-core/src/portal.rs`
- Modify: `crates/wield-core/src/executor.rs`, `crates/wield-core/src/lib.rs`
- Modify: `crates/wield-core/tests/executor_pipeline.rs`

**Interfaces:**
- Produces:
  - `crates/wield-core/src/portal.rs`:

    ```rust
    //! The portal-execution seam. `wield-portal` implements this; `wield-core`
    //! never depends on a portal library.

    use crate::args::ArgMap;
    use crate::outcome::ToolOutcome;
    use tokio_util::sync::CancellationToken;

    /// Runs a `Capability::Portal { adapter }` tool. Implementations must return
    /// `ToolOutcome::Cancelled` (never `Failed`) when the user dismisses a portal
    /// dialog, and `ToolOutcome::Failed { stage: Stage::Portal, .. }` for real
    /// errors.
    #[async_trait::async_trait]
    pub trait PortalRunner: Send + Sync {
        async fn run(
            &self,
            adapter: &str,
            args: &ArgMap,
            cancel: CancellationToken,
        ) -> ToolOutcome;
    }
    ```
  - `Executor` becomes:

    ```rust
    pub struct Executor {
        resolver: BinaryResolver,
        portal: Option<std::sync::Arc<dyn crate::portal::PortalRunner>>,
        availability: AvailabilityView,
    }

    impl Executor {
        pub fn new(resolver: BinaryResolver) -> Self { /* portal: None, availability: default */ }
        pub fn with_portal(mut self, portal: std::sync::Arc<dyn crate::portal::PortalRunner>) -> Self { self.portal = Some(portal); self }
        pub fn with_availability(mut self, availability: AvailabilityView) -> Self { self.availability = availability; self }
        pub async fn run(&self, request: ExecutionRequest, progress: mpsc::Sender<Progress>, cancel: CancellationToken) -> ToolOutcome { /* unchanged signature */ }
    }

    impl std::fmt::Debug for Executor {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("Executor")
                .field("resolver", &self.resolver)
                .field("portal", &self.portal.is_some())
                .field("availability", &self.availability)
                .finish()
        }
    }
    ```
    (Remove `#[derive(Debug)]` from `Executor`; keep `#[derive(Clone)]` — `Arc` and `AvailabilityView` are `Clone`.)
  - `lib.rs`: `pub mod portal;` and `pub use portal::PortalRunner;`.
- **`Requires::Portal { iface, min_ver }` arm** (replaces the P2 placeholder): consult `self.availability.portals`:

  ```rust
  Requires::Portal { iface, min_ver } => match self.availability.portals.get(iface) {
      Some(version) if version >= min_ver => {}
      Some(version) => {
          return ToolOutcome::Unavailable {
              reason: format!("the {iface} desktop portal is version {version}, but this tool needs version {min_ver}"),
              fix: Some(format!("update your desktop environment to one that provides {iface} portal v{min_ver} or newer")),
          };
      }
      None => {
          return ToolOutcome::Unavailable {
              reason: format!("the {iface} desktop portal is not available"),
              fix: Some("this tool needs a desktop environment with XDG Desktop Portal support".to_owned()),
          };
      }
  },
  ```
- **`Capability::Portal { adapter }` arm** (replaces the P2 placeholder):

  ```rust
  Capability::Portal { adapter } => match &self.portal {
      Some(runner) => runner.run(adapter, &effective, cancel).await,
      None => ToolOutcome::Failed {
          stage: Stage::Portal,
          detail: "this build has no portal runner configured".to_owned(),
          hint: None,
      },
  },
  ```
- The existing P2 test `portal_capability_is_placeholder_failure` still passes (it builds an `Executor::new(..)` with no portal → the `None` arm).

- [x] **Step 1: Write the failing tests**

Append to `crates/wield-core/tests/executor_pipeline.rs`:

```rust
use std::sync::Arc;
use wield_core::portal::PortalRunner;

struct StubPortal {
    outcome: ToolOutcome,
    seen: std::sync::Mutex<Option<String>>,
}

#[async_trait::async_trait]
impl PortalRunner for StubPortal {
    async fn run(
        &self,
        adapter: &str,
        _args: &wield_core::ArgMap,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> ToolOutcome {
        *self.seen.lock().unwrap() = Some(adapter.to_string());
        self.outcome.clone()
    }
}

fn portal_descriptor(adapter: &str) -> Descriptor {
    let mut d = convert_descriptor("sh");
    d.requires = Requires::None;
    d.output = OutputSpec::Value(ValueKind::Color);
    d.capability = Capability::Portal { adapter: adapter.into() };
    d
}

#[tokio::test]
async fn injected_portal_runner_receives_the_adapter_key() {
    let stub = Arc::new(StubPortal {
        outcome: ToolOutcome::Value { kind: ValueKind::Color, data: "#abcdef".into() },
        seen: std::sync::Mutex::new(None),
    });
    let exec = Executor::new(BinaryResolver::from_env()).with_portal(stub.clone());
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec
        .run(ExecutionRequest { descriptor: portal_descriptor("screenshot.pick_color"), args }, tx, CancellationToken::new())
        .await;
    assert!(matches!(outcome, ToolOutcome::Value { .. }));
    assert_eq!(stub.seen.lock().unwrap().as_deref(), Some("screenshot.pick_color"));
}

#[tokio::test]
async fn portal_requires_unmet_is_unavailable() {
    let mut d = portal_descriptor("screenshot.pick_color");
    d.requires = Requires::Portal { iface: "Screenshot".into(), min_ver: 2 };
    // availability left empty -> portal absent
    let exec = Executor::new(BinaryResolver::from_env())
        .with_portal(Arc::new(StubPortal {
            outcome: ToolOutcome::Cancelled,
            seen: std::sync::Mutex::new(None),
        }));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec.run(ExecutionRequest { descriptor: d, args }, tx, CancellationToken::new()).await;
    assert!(matches!(outcome, ToolOutcome::Unavailable { .. }));
}

#[tokio::test]
async fn portal_requires_met_runs_the_adapter() {
    let mut d = portal_descriptor("screenshot.pick_color");
    d.requires = Requires::Portal { iface: "Screenshot".into(), min_ver: 2 };
    let mut view = wield_core::AvailabilityView::default();
    view.portals.insert("Screenshot".into(), 2);
    let stub = Arc::new(StubPortal {
        outcome: ToolOutcome::Value { kind: ValueKind::Color, data: "#000000".into() },
        seen: std::sync::Mutex::new(None),
    });
    let exec = Executor::new(BinaryResolver::from_env())
        .with_portal(stub.clone())
        .with_availability(view);
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/tmp/x".into()));
    let (tx, _rx) = mpsc::channel(16);
    let outcome = exec.run(ExecutionRequest { descriptor: d, args }, tx, CancellationToken::new()).await;
    assert!(matches!(outcome, ToolOutcome::Value { .. }));
    assert_eq!(stub.seen.lock().unwrap().as_deref(), Some("screenshot.pick_color"));
}
```

(Add `async-trait` to `crates/wield-core`'s `[dev-dependencies]` too, or reference it via the dep already added in Task 1 Step 2 — it is a normal dep so `async_trait::async_trait` resolves in tests.)

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core --test executor_pipeline`
Expected: FAIL — `portal` module / `with_portal` missing.

- [x] **Step 3: Implement `src/portal.rs`, the `Executor` changes, and the `lib.rs` export**

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-core`
Expected: PASS — the 3 new tests plus every P2 test (including `portal_capability_is_placeholder_failure`).

- [x] **Step 5: Commit**

```bash
git add crates/wield-core
git commit -m "feat(core): PortalRunner trait and executor injection seam"
```

---

## Task 3: `PortalMap` and Introspect-XML parsing

**Files:**
- Replace: `crates/wield-portal/src/probe.rs`
- Replace: `crates/wield-portal/src/error.rs`
- Test: `crates/wield-portal/tests/probe.rs`

**Interfaces:**
- `src/error.rs`:

  ```rust
  //! Portal error type and its mapping to `ToolOutcome`.

  use wield_core::{Stage, ToolOutcome};

  #[derive(Debug)]
  pub enum PortalError {
      /// The user dismissed the portal dialog. Maps to a *silent* Cancelled.
      Cancelled,
      /// A D-Bus / portal transport or protocol failure.
      Transport(String),
      /// The portal returned a response we could not interpret.
      BadResponse(String),
  }

  impl std::fmt::Display for PortalError { /* ... */ }
  impl std::error::Error for PortalError {}

  impl PortalError {
      /// Convert to a `ToolOutcome`. `Cancelled` is silent; everything else is
      /// `Failed { stage: Portal, .. }` with a hint where we have one.
      pub fn into_outcome(self) -> ToolOutcome {
          match self {
              PortalError::Cancelled => ToolOutcome::Cancelled,
              PortalError::Transport(detail) => ToolOutcome::Failed {
                  stage: Stage::Portal,
                  detail,
                  hint: Some("the desktop portal service may not be running".to_owned()),
              },
              PortalError::BadResponse(detail) => ToolOutcome::Failed {
                  stage: Stage::Portal,
                  detail,
                  hint: None,
              },
          }
      }
  }

  impl From<zbus::Error> for PortalError {
      fn from(e: zbus::Error) -> Self { PortalError::Transport(e.to_string()) }
  }
  ```
- `src/probe.rs`:

  ```rust
  pub struct PortalMap(std::collections::HashMap<String, u32>);

  impl PortalMap {
      pub fn get(&self, iface_short: &str) -> Option<u32>;
      pub fn iter(&self) -> impl Iterator<Item = (&str, u32)>;
      pub fn is_empty(&self) -> bool;
      /// Feed the probe result into an `AvailabilityView` (keeps existing binaries).
      pub fn apply_to(&self, view: &mut wield_core::AvailabilityView);
  }

  /// Parse a `org.freedesktop.portal.Desktop` Introspect XML document into the
  /// map of short interface name (`Screenshot`, `Inhibit`, ...) → `version`
  /// property value. Interfaces without a readable `version` property are
  /// recorded with version `0` (present but unversioned). Non-portal interfaces
  /// (`org.freedesktop.DBus.*`, etc.) are ignored.
  pub fn parse_portal_versions(introspect_xml: &str) -> PortalMap;

  /// Connect to the session bus, introspect the portal object, and read each
  /// portal interface's `version` property. Never panics; a transport failure
  /// yields an empty map (spec §5: Wield never blocks startup on this).
  pub async fn probe() -> PortalMap;
  ```
- `parse_portal_versions` uses `zbus_xml::Node::from_reader` (or `from_str`) to get interfaces; the Introspect XML alone gives interface **names** but not `version` values — so `parse_portal_versions` records every `org.freedesktop.portal.*` interface it sees with version `0`, and `probe()` then upgrades each entry by reading the live `version` property. The unit test targets `parse_portal_versions` (names only); the integration test (Task 4) targets `probe()` (real versions).

- [x] **Step 1: Write the failing test**

`crates/wield-portal/tests/probe.rs`:

```rust
use wield_portal::probe::parse_portal_versions;

const SAMPLE: &str = r#"<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN" "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>
  <interface name="org.freedesktop.DBus.Introspectable">
    <method name="Introspect"><arg type="s" direction="out"/></method>
  </interface>
  <interface name="org.freedesktop.portal.Screenshot">
    <method name="PickColor"><arg type="s"/><arg type="a{sv}"/><arg type="o" direction="out"/></method>
    <property name="version" type="u" access="read"/>
  </interface>
  <interface name="org.freedesktop.portal.Inhibit">
    <property name="version" type="u" access="read"/>
  </interface>
</node>"#;

#[test]
fn parses_portal_interface_names() {
    let map = parse_portal_versions(SAMPLE);
    assert_eq!(map.get("Screenshot"), Some(0));
    assert_eq!(map.get("Inhibit"), Some(0));
    assert_eq!(map.get("Introspectable"), None);
    assert_eq!(map.get("GlobalShortcuts"), None);
}
```

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test probe`
Expected: FAIL.

- [x] **Step 3: Implement `src/error.rs` and `src/probe.rs`**

For `parse_portal_versions`: parse with `zbus_xml`, keep interface names starting `org.freedesktop.portal.`, strip that prefix, insert `(short, 0)`.

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test probe`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/wield-portal/src/probe.rs crates/wield-portal/src/error.rs crates/wield-portal/tests/probe.rs
git commit -m "feat(portal): PortalMap and Introspect-XML parsing"
```

---

## Task 4: Probe integration test — fake portal on a private bus

**Files:**
- Create: `crates/wield-portal/tests/support/mod.rs`
- Modify: `crates/wield-portal/tests/probe.rs`
- Possibly modify: `crates/wield-portal/src/probe.rs` (make `probe()` accept a `zbus::Connection` internally so tests can point it at the private bus)

**Interfaces:**
- `tests/support/mod.rs`:

  ```rust
  #![allow(dead_code)]
  use std::process::{Child, Command};

  /// A throwaway session bus backed by its own `dbus-daemon`.
  pub struct PrivateBus {
      address: String,
      child: Child,
  }

  impl PrivateBus {
      /// Launch `dbus-daemon --session` on a private socket. Skips the test
      /// (returns `None`) if `dbus-daemon` is not installed.
      pub fn launch() -> Option<Self>;
      pub fn address(&self) -> &str;
      /// A `zbus::Connection` to this bus.
      pub async fn connect(&self) -> zbus::Connection;
  }

  impl Drop for PrivateBus {
      fn drop(&mut self) { let _ = self.child.kill(); }
  }

  /// Serve a fake `org.freedesktop.portal.Desktop` on `conn` exposing the given
  /// portal interfaces, each reporting the given `version`. Returns once the
  /// name is owned.
  pub async fn serve_fake_portal(conn: &zbus::Connection, ifaces: &[(&str, u32)]);
  ```
- `probe()` gains a `pub(crate) async fn probe_on(conn: &zbus::Connection) -> PortalMap`; `probe()` becomes `probe_on(&session_connection).await`.
- `serve_fake_portal` registers, at `/org/freedesktop/portal/desktop`, one zbus `#[interface(name = "org.freedesktop.portal.<Short>")]` object per entry, each with a `#[zbus(property)] fn version(&self) -> u32`. Then requests the well-known name `org.freedesktop.portal.Desktop`.

- [x] **Step 1: Write the failing test**

Append to `crates/wield-portal/tests/probe.rs`:

```rust
mod support;

#[tokio::test]
async fn probe_reads_versions_from_a_fake_portal() {
    let Some(bus) = support::PrivateBus::launch() else {
        eprintln!("skipping: dbus-daemon not available");
        return;
    };
    let server = bus.connect().await;
    support::serve_fake_portal(&server, &[("Screenshot", 2), ("Inhibit", 3)]).await;

    let client = bus.connect().await;
    let map = wield_portal::probe::probe_on(&client).await;

    assert_eq!(map.get("Screenshot"), Some(2));
    assert_eq!(map.get("Inhibit"), Some(3));
    assert_eq!(map.get("GlobalShortcuts"), None);
}
```

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test probe`
Expected: FAIL (or skip line printed if `dbus-daemon` missing — but it is installed here).

- [x] **Step 3: Implement `tests/support/mod.rs` and `probe_on`**

`PrivateBus::launch`: `Command::new("dbus-daemon").args(["--session", "--nofork", "--print-address", &format!("--address=unix:path={}", sock)])`, read the printed address, poll until connectable. Kill on drop.

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test probe`
Expected: PASS (2 unit + 1 integration).

- [x] **Step 5: Commit**

```bash
git add crates/wield-portal/tests/support crates/wield-portal/tests/probe.rs crates/wield-portal/src/probe.rs
git commit -m "test(portal): probe integration against a fake portal service"
```

---

## Task 5: `pick_color` adapter — color math + outcome mapping

**Files:**
- Replace: `crates/wield-portal/src/color.rs`
- Replace: `crates/wield-portal/src/adapters/pick_color.rs`
- Test: unit tests in both files

**Interfaces:**
- `src/color.rs`:

  ```rust
  /// An 8-bit-per-channel colour.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct Rgb { pub r: u8, pub g: u8, pub b: u8 }

  impl Rgb {
      /// From `ashpd`'s 0.0..=1.0 float channels (values are clamped then rounded).
      pub fn from_unit(r: f64, g: f64, b: f64) -> Self;
      pub fn hex(&self) -> String;            // "#rrggbb"
      pub fn rgb_string(&self) -> String;     // "rgb(r, g, b)"
      pub fn hsl_string(&self) -> String;     // "hsl(h, s%, l%)" (h int deg, s/l int %)
      /// JSON object string used as `ToolOutcome::Value.data`.
      pub fn value_payload(&self) -> String;  // {"hex":..,"rgb":..,"hsl":..}
  }
  ```
- `src/adapters/pick_color.rs`:

  ```rust
  use tokio_util::sync::CancellationToken;
  use wield_core::{ArgMap, ToolOutcome, ValueKind};
  use crate::color::Rgb;
  use crate::error::PortalError;

  /// The real adapter: calls the Screenshot portal's PickColor.
  pub async fn pick_color(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
      pick_color_with(fetch_color_via_ashpd(), cancel).await
  }

  /// Testable core: given a future that yields the picked colour (or a
  /// `PortalError`) and a cancel token, produce the outcome.
  pub(crate) async fn pick_color_with<F>(fetch: F, cancel: CancellationToken) -> ToolOutcome
  where
      F: std::future::Future<Output = Result<Rgb, PortalError>>,
  {
      tokio::select! {
          _ = cancel.cancelled() => ToolOutcome::Cancelled,
          result = fetch => match result {
              Ok(rgb) => ToolOutcome::Value { kind: ValueKind::Color, data: rgb.value_payload() },
              Err(err) => err.into_outcome(),
          },
      }
  }

  async fn fetch_color_via_ashpd() -> Result<Rgb, PortalError> {
      // ashpd 0.13: Screenshot::pick_color returns a Color with red()/green()/blue() f64s.
      // ashpd::desktop::screenshot::ColorResponse (dismissed dialog) -> Err(PortalError::Cancelled).
      // Any other ashpd::Error -> PortalError::Transport / BadResponse.
      todo!("wire ashpd Screenshot.pick_color; see ashpd 0.13 docs")
  }
  ```
  Fill in `fetch_color_via_ashpd` using `ashpd::desktop::screenshot::Screenshot`. Map `ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled)` → `PortalError::Cancelled`; other `ashpd::Error` → `PortalError::Transport(e.to_string())`.

- [x] **Step 1: Write the failing tests**

In `src/color.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_unit_channels_to_hex_rgb_hsl() {
        let c = Rgb::from_unit(0.0, 0.5019608, 1.0); // 0, 128, 255
        assert_eq!(c, Rgb { r: 0, g: 128, b: 255 });
        assert_eq!(c.hex(), "#0080ff");
        assert_eq!(c.rgb_string(), "rgb(0, 128, 255)");
        assert_eq!(c.hsl_string(), "hsl(210, 100%, 50%)");
    }

    #[test]
    fn clamps_out_of_range_channels() {
        assert_eq!(Rgb::from_unit(-1.0, 2.0, 0.5), Rgb { r: 0, g: 255, b: 128 });
    }

    #[test]
    fn value_payload_is_json_with_all_three() {
        let p = Rgb { r: 255, g: 255, b: 255 }.value_payload();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        assert_eq!(v["hex"], "#ffffff");
        assert!(v["rgb"].is_string() && v["hsl"].is_string());
    }
}
```

In `src/adapters/pick_color.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn maps_a_picked_colour_to_a_value_outcome() {
        let out = pick_color_with(async { Ok(Rgb { r: 10, g: 20, b: 30 }) }, CancellationToken::new()).await;
        match out {
            ToolOutcome::Value { kind: ValueKind::Color, data } => {
                assert!(data.contains("#0a141e"));
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dismissed_dialog_is_silent_cancelled() {
        let out = pick_color_with(async { Err(PortalError::Cancelled) }, CancellationToken::new()).await;
        assert!(matches!(out, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn transport_error_is_failed_portal() {
        let out = pick_color_with(async { Err(PortalError::Transport("boom".into())) }, CancellationToken::new()).await;
        assert!(matches!(out, ToolOutcome::Failed { stage: wield_core::Stage::Portal, .. }));
    }

    #[tokio::test]
    async fn cancel_token_wins_over_a_pending_pick() {
        let token = CancellationToken::new();
        token.cancel();
        let out = pick_color_with(std::future::pending::<Result<Rgb, PortalError>>(), token).await;
        assert!(matches!(out, ToolOutcome::Cancelled));
    }
}
```

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal color pick_color`
Expected: FAIL.

- [x] **Step 3: Implement `color.rs` and the `pick_color_with` core**

Implement `fetch_color_via_ashpd` too, but it is not exercised by unit tests (Task 7 covers it).

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/wield-portal/src/color.rs crates/wield-portal/src/adapters/pick_color.rs
git commit -m "feat(portal): pick_color colour conversion and outcome mapping"
```

---

## Task 6: `PortalAdapterRunner` + dispatch + public API

**Files:**
- Replace: `crates/wield-portal/src/adapters/mod.rs`, `crates/wield-portal/src/runner.rs`
- Modify: `crates/wield-portal/src/lib.rs` (exports already listed in Task 1; confirm)
- Test: `crates/wield-portal/tests/runner.rs`

**Interfaces:**
- `src/adapters/mod.rs`:

  ```rust
  pub mod pick_color;

  use tokio_util::sync::CancellationToken;
  use wield_core::{ArgMap, Stage, ToolOutcome};

  /// Route an adapter key to its implementation. Unknown keys are a descriptor
  /// bug, surfaced as `Failed { stage: Portal }`.
  pub(crate) async fn dispatch(adapter: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
      match adapter {
          "screenshot.pick_color" => pick_color::pick_color(args, cancel).await,
          other => ToolOutcome::Failed {
              stage: Stage::Portal,
              detail: format!("unknown portal adapter: {other}"),
              hint: Some("this is a bug in the tool descriptor".to_owned()),
          },
      }
  }
  ```
- `src/runner.rs`:

  ```rust
  use async_trait::async_trait;
  use tokio_util::sync::CancellationToken;
  use wield_core::{ArgMap, PortalRunner, ToolOutcome};

  /// The `wield-portal` implementation of `wield_core::PortalRunner`.
  #[derive(Debug, Default, Clone, Copy)]
  pub struct PortalAdapterRunner;

  #[async_trait]
  impl PortalRunner for PortalAdapterRunner {
      async fn run(&self, adapter: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
          crate::adapters::dispatch(adapter, args, cancel).await
      }
  }
  ```

- [x] **Step 1: Write the failing test**

`crates/wield-portal/tests/runner.rs`:

```rust
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;
use wield_core::{PortalRunner, Stage, ToolOutcome};
use wield_portal::PortalAdapterRunner;

#[tokio::test]
async fn unknown_adapter_is_failed_portal() {
    let out = PortalAdapterRunner
        .run("nope.nope", &BTreeMap::new(), CancellationToken::new())
        .await;
    assert!(matches!(out, ToolOutcome::Failed { stage: Stage::Portal, .. }));
}

#[tokio::test]
async fn known_adapter_with_immediate_cancel_is_cancelled() {
    let token = CancellationToken::new();
    token.cancel();
    let out = PortalAdapterRunner
        .run("screenshot.pick_color", &BTreeMap::new(), token)
        .await;
    assert!(matches!(out, ToolOutcome::Cancelled));
}
```

(The second test relies on `pick_color`'s `tokio::select!` seeing the pre-cancelled token before it ever touches D-Bus — no portal needed.)

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test runner`
Expected: FAIL.

- [x] **Step 3: Implement `adapters/mod.rs` and `runner.rs`**

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal`
Expected: PASS.

- [x] **Step 5: Commit**

```bash
git add crates/wield-portal/src/adapters/mod.rs crates/wield-portal/src/runner.rs crates/wield-portal/src/lib.rs
git commit -m "feat(portal): PortalAdapterRunner implementing wield_core::PortalRunner"
```

---

## Task 7: End-to-end — fake Screenshot portal → `pick_color()` → executor

**Files:**
- Modify: `crates/wield-portal/tests/support/mod.rs` (add a fake Screenshot portal)
- Create: `crates/wield-portal/tests/pick_color.rs`

**FALLBACK — read first.** The fake `PickColor` requires implementing the XDG portal Request/Response dance (return an `o` request handle, then emit `Response(u, a{sv})` on `org.freedesktop.portal.Request` with `results["color"] = (ddd)`). If, after a genuine attempt, this cannot be made reliable within ~1–2 focused hours:
- keep the `#[ignore]`d real-portal test (below),
- rely on the Task 5 unit tests (`pick_color_with` mapping + cancellation) and the Task 4 probe integration test for automated coverage,
- add a line to `docs/testing.md` (create it if absent) under a "Portal manual matrix" heading: *"pick_color end-to-end is covered by the manual matrix (GNOME/KDE/sway) — the private-bus PickColor mock is deferred."*,
- note the fallback was taken in the Task 8 report.
This is acceptable: the spec already mandates a manual portal matrix, and `ashpd` itself is not under test (spec §8 "Out of scope: third-party CLI internals").

**Interfaces (if attempting the mock):**
- `tests/support/mod.rs` gains:

  ```rust
  /// Serve a fake `org.freedesktop.portal.Screenshot` whose `PickColor` returns
  /// `canned` after `delay`. Also serves the matching `org.freedesktop.portal.Request`.
  pub async fn serve_fake_pick_color(conn: &zbus::Connection, canned: (f64, f64, f64), delay: std::time::Duration);
  ```
- `tests/pick_color.rs`:

  ```rust
  mod support;
  use std::time::Duration;
  use tokio_util::sync::CancellationToken;
  use wield_core::{ToolOutcome, ValueKind};

  #[tokio::test]
  async fn pick_color_through_a_fake_portal_yields_a_value() {
      let Some(bus) = support::PrivateBus::launch() else { return; };
      let server = bus.connect().await;
      support::serve_fake_pick_color(&server, (0.0, 0.5019608, 1.0), Duration::from_millis(20)).await;

      // Point ashpd at the private bus for the duration of this test.
      std::env::set_var("DBUS_SESSION_BUS_ADDRESS", bus.address());

      let out = wield_portal::adapters::pick_color::pick_color(
          &Default::default(),
          CancellationToken::new(),
      ).await;

      match out {
          ToolOutcome::Value { kind: ValueKind::Color, data } => assert!(data.contains("#0080ff")),
          other => panic!("expected Value, got {other:?}"),
      }
  }

  #[tokio::test]
  #[ignore = "requires a real desktop portal; run manually"]
  async fn pick_color_against_the_real_portal() {
      let out = wield_portal::adapters::pick_color::pick_color(&Default::default(), CancellationToken::new()).await;
      assert!(matches!(out, ToolOutcome::Value { .. } | ToolOutcome::Cancelled));
  }
  ```
  Note: `pick_color::pick_color` must be `pub` (or add a `pub` test shim) for the test to call it. Adjust `adapters/mod.rs` / `pick_color.rs` visibility as needed and note it.
  `set_var` for `DBUS_SESSION_BUS_ADDRESS` makes this test **not** parallel-safe with other bus tests — put `#[serial_test::serial]` on it, adding `serial_test` as a dev-dep, **or** run this file's tests single-threaded via a `// @test-threads=1` convention and note it. Prefer `serial_test`.

- [x] **Step 1: Write the failing test** (the non-ignored one above)

- [x] **Step 2: Run to verify failure**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test pick_color`
Expected: FAIL.

- [x] **Step 3: Implement `serve_fake_pick_color`** (or take the fallback)

- [x] **Step 4: Run to verify pass**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-portal --test pick_color`
Expected: PASS (the mock test), plus 1 ignored.

- [x] **Step 5: Commit**

```bash
git add crates/wield-portal/tests
git commit -m "test(portal): end-to-end pick_color via a fake Screenshot portal"
```

(If the fallback was taken: `git add crates/wield-portal/tests docs/testing.md` and commit message `test(portal): pick_color unit + probe coverage; mock PickColor deferred to manual matrix`.)

---

## Task 8: Workspace green + P3 wrap-up

**Files:**
- Modify: this plan file (status block)

- [x] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p wield-portal -- --ignored --list   # confirm the real-portal test is registered
```
Expected: fmt clean, clippy clean, all non-ignored tests pass.

- [x] **Step 2: `npm run check`**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && npm run check`
Expected: PASS.

- [x] **Step 3: Regression — `wield-cli` still runs**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo run -p wield-cli`
Expected: prints `wield 0.1.0`.

- [x] **Step 4: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P3 complete" --body "wield-portal landed on feat/p3-portal: PortalRunner trait + executor injection in wield-core; zbus-introspection capability probe; screenshot.pick_color adapter (colour math + silent-cancel); PortalAdapterRunner. Probe + adapter + runner tests green; end-to-end PickColor mock <status: done | fell back to manual matrix>. cargo test --workspace + clippy + npm run check green. N commits, branch pushed."
```

- [x] **Step 5: Commit the checked-off plan + push**

```bash
git add docs/superpowers/plans/2026-09-10-wield-m1-p3-portal.md
git commit -m "docs: mark M1 P3 plan complete"
git push -u origin feat/p3-portal
```

GEON opens the PR, reviews, merges to `master`. P4 (`wield-tools` + `wield-cli` one-shot + the real `color.pick` / `image.convert` descriptors) is written just-in-time.

---

## Self-Review

**Spec coverage:**

- §3 `wield-portal` = "thin `ashpd` wrapper + startup capability probe + degradation reporting" → Tasks 3–6 ✓ (degradation = `PortalMap` → `AvailabilityView` → executor `Unavailable` messaging, Task 2 ✓)
- §4 `Portal` capability = "descriptor + named adapter; adapter is a small function in `wield-portal` referenced by key" → `PortalAdapterRunner::dispatch` Task 6 ✓
- §4 executor pipeline step 2 "check `requires` → portal probe result" → Task 2 `Requires::Portal` arm ✓
- §5 capability probe = "introspect `org.freedesktop.portal.Desktop` for interfaces + versions … feeds the palette (grey out + reason) … never blocks startup" → Tasks 3, 4 ✓; empty-map-on-failure ✓
- §5 adapters for v1: `screenshot.pick_color`, `screenshot.region`, `inhibit.toggle` → **P3 does `pick_color` only** (region/inhibit deferred to M3 with their tools — GEON amendment) ✓
- §7 "user-cancel is always silent" → `PortalError::Cancelled → ToolOutcome::Cancelled`, Tasks 3, 5 ✓
- §7 "never surface a raw D-Bus error string as the primary message" → `PortalError::Transport` carries the detail but with a plain-language `hint`; the raw string is the `detail` (shown behind an expand by the UI, P6) — acceptable, matches `Failed { detail, hint }` shape ✓
- §8 `wield-portal` unit: "capability probe: fake interface/version map → correct availability set" → Task 4 ✓
- §8 "adapter response → `ToolOutcome` mapping (ashpd call mocked)" → Task 5 `pick_color_with` ✓
- §8 "mock portal D-Bus service on a private bus returns canned PickColor" → Task 7 ✓ (with documented fallback to the spec's own manual matrix)
- §8 "manual matrix" → `#[ignore]`d real-portal test + `docs/testing.md` note ✓

**Placeholder scan:** the only `todo!()` is `fetch_color_via_ashpd` in a Task 5 *snippet*, explicitly filled in that task's Step 3. No `TODO`/`TBD` in step bodies. Deferred items (region/inhibit adapters, `wield-tools`, Tauri wiring, `GlobalShortcuts`) each name their owning plan.

**Type consistency:** `PortalRunner::run(&self, &str, &ArgMap, CancellationToken) -> ToolOutcome` — identical in `portal.rs` (Task 2), the `StubPortal` test impl (Task 2), and `PortalAdapterRunner` (Task 6). `PortalMap::get -> Option<u32>` used in Tasks 3, 4. `Rgb` produced by `color.rs` (Task 5), consumed by `pick_color_with` (Task 5). `PortalError::into_outcome` defined Task 3, used Task 5. `AvailabilityView` (from `wield-core`, P2) — `portals` field written by `PortalMap::apply_to` (Task 3), read by the executor `Requires::Portal` arm (Task 2).

**Ordering:** 1 → 2 (wield-core seam) → 3 (probe parse) → 4 (probe integration, needs 3 + support harness) → 5 (adapter, independent of 3/4) → 6 (runner, needs 5) → 7 (e2e, needs 4's harness + 6) → 8. Consistent.

---

## Execution note

After P3 merges, `wield-core` + `wield-portal` together can run a `Command` tool and a `Portal` tool, and report why any tool is unavailable. P4 adds `wield-tools` (the real `image.convert` / `color.pick` descriptors + built-in registry + its snapshot test) and the `wield-cli` one-shot surface. P5 builds the Tauri shell and constructs the `Executor` with `PortalAdapterRunner` + a real `probe()` result injected.

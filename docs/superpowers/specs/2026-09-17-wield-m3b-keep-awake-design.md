# Wield — M3b: `keep.awake` Design

## 1. Why this exists

The last tool in M3 (Capture), per `docs/superpowers/specs/2026-09-10-wield-design.md`
§6 — and the one that closes the milestone gating `1.0` ("`1.0` is tagged when
M1–M3 are shipped and stable"). `keep.awake` is a `Portal` capability backed
by `org.freedesktop.portal.Inhibit`, spec'd as "Stateful toggle; tray shows
active state."

Unlike every existing `Portal` tool (`color.pick`, `screen.ocr`'s capture
step), this isn't a one-shot request/response — it's a toggle whose effect
(the screen staying awake) persists independently of the function call that
turned it on. That shape doesn't obviously fit `PortalRunner::run`'s
"return one `ToolOutcome`, done" model, and the spec's own "tray shows
active state" requirement needs tray infrastructure that doesn't exist yet
(`tray.rs`'s menu is built once, statically, at startup — nothing in it
updates at runtime today).

## 2. What's already there, verified against the actual portal spec and code

- **`ashpd::desktop::inhibit::InhibitProxy`** already exists in the
  workspace's `ashpd` dependency (same crate `color.pick`/`screen.ocr`
  already use). `InhibitProxy::inhibit(identifier, flags, options) ->
  Result<Request<()>, Error>` is a one-shot call — but critically, it is
  **not** fire-and-forget in effect: the inhibitor it creates stays active
  until something closes the returned handle.
- **Verified against the actual XDG portal spec** (not just inferred from
  the Rust wrapper's shape): "To remove the inhibition, call
  `org.freedesktop.portal.Request.Close` on the returned handle." There is
  no separate `Uninhibit` method, and — important, and easy to get wrong —
  **the inhibitor does not auto-release if the calling process disconnects
  or crashes.** It stays in effect until explicitly closed.
  `ashpd::desktop::request::Request<T>::close(&self) -> Result<(), Error>`
  is exactly that call.
- **`tauri::menu::CheckMenuItem<R>`** already exists in the installed Tauri
  version (2.11.5), with `.set_checked(bool)` / `.is_checked()` — the
  primitive the tray needs, already available, not something to build.
- **The `hotkey_controller` field in `AppState`**
  (`apps/wield/src-tauri/src/state.rs`) is real prior art for "a
  portal-adjacent resource held across the app's lifetime, guarded by a
  `Mutex`, created/torn down outside of any single function call" — but it
  is only ever touched by dedicated Tauri commands that already have
  `State<AppState>` access. `keep.awake` runs through the *generic*
  `Registry → Executor → PortalRunner → adapter dispatch` pipeline every
  other tool uses, which has no channel into `AppState` at all — giving it
  one would be a real exception to "every tool is uniformly
  descriptor-driven," not something to introduce for one tool.
- **Live-verified on this dev machine**: the `Inhibit` portal interface is
  present at version 3 (`busctl --user introspect ... org.freedesktop.portal.Inhibit`
  → `.version 3`). `Inhibit()` and `InhibitFlags::Idle`/`Suspend` are both
  version-1 features per the real portal spec — `min_ver: 1` is the correct,
  conservative gate; nothing in this design needs anything newer.

## 3. Design

### 3.1 Descriptor (`wield-tools/src/keep_awake.rs`)

```rust
DescriptorBuilder::new("keep.awake", "Keep awake", Category::Capture)
    .keywords(&["awake", "sleep", "screensaver", "caffeine", "inhibit"])
    .requires(Requires::Portal {
        iface: "Inhibit".into(),
        min_ver: 1,
    })
    .output(OutputSpec::Report)
    .portal("inhibit.toggle")
    .build()
```

No arguments — a no-args tool runs immediately from the palette on
selection, exactly like `color.pick` already does (existing, free
behavior).

### 3.2 The adapter and its state (`wield-portal/src/adapters/inhibit_toggle.rs`)

Holds a small, self-contained module-level static tracking the currently-
held inhibit request — deliberately **not** in `AppState` (§2's reasoning).
Each invocation checks it and flips:

```rust
use crate::error::PortalError;
use ashpd::desktop::inhibit::{InhibitFlags, InhibitOptions, InhibitProxy};
use ashpd::desktop::Request;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, ToolOutcome};

static HELD: OnceLock<Mutex<Option<Request<()>>>> = OnceLock::new();

fn held() -> &'static Mutex<Option<Request<()>>> {
    HELD.get_or_init(|| Mutex::new(None))
}

/// Synchronous state query — the tray uses this to initialize and refresh
/// its checkmark without going through the async toggle path. `try_lock`
/// is deliberate: this only needs to be right when nothing is
/// mid-toggle, which is true the overwhelming majority of the time it's
/// called (startup, and shortly after a toggle's own event already fired
/// with the authoritative new value) - falling back to "not active" on the
/// rare contended case is an acceptable display-only race, not a
/// correctness issue worth a real lock wait for.
pub fn is_active() -> bool {
    held().try_lock().map(|guard| guard.is_some()).unwrap_or(false)
}

pub async fn toggle(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
    let mut guard = held().lock().await;
    if let Some(request) = guard.take() {
        // Turning off. Best-effort: even if the close call itself fails,
        // clearing our own handle is still correct - we can't meaningfully
        // hold or retry a handle we've already decided to drop.
        let _ = request.close().await;
        return ToolOutcome::Report {
            title: "Keep awake".to_owned(),
            lines: vec!["Turned off - normal sleep and screensaver behavior is restored.".to_owned()],
        };
    }

    let proxy = match InhibitProxy::new().await {
        Ok(proxy) => proxy,
        Err(error) => return PortalError::from(error).into_outcome(),
    };
    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => return ToolOutcome::Cancelled,
        result = proxy.inhibit(
            None,
            InhibitFlags::Idle | InhibitFlags::Suspend,
            InhibitOptions::default().set_reason("Wield: keep awake"),
        ) => result,
    };
    match result {
        Ok(request) => {
            *guard = Some(request);
            ToolOutcome::Report {
                title: "Keep awake".to_owned(),
                lines: vec!["Turned on - your screen won't sleep or lock until you toggle this off again.".to_owned()],
            }
        }
        Err(error) => PortalError::from(error).into_outcome(),
    }
}
```

Reuses `PortalError` directly — this adapter lives inside `wield-portal`
itself (unlike M3a's `wield-native`, which needed a cross-crate dependency
to reach it), so no new plumbing is needed to get the same
`Cancelled`/`Transport`/`BadResponse` mapping `pick_color.rs` already has.
`PortalError` currently has no `From<ashpd::Error>` impl (only
`From<zbus::Error>`) — adding one, mapping the same way
`pick_color.rs`'s existing `map_ashpd_error` free function already does
inline, is the one small addition this needs.

Registered in `wield-portal/src/adapters/mod.rs`'s dispatch alongside
`"screenshot.pick_color"`.

### 3.3 Tray reflects state (`tray.rs`)

`build_menu`'s per-tool loop special-cases `keep.awake`'s id to create a
`CheckMenuItem` (initialized from `inhibit_toggle::is_active()`) instead of
a plain `MenuItem`. This is a one-off special case for this one tool, not a
new general "every tool can push tray state" mechanism — no existing
`Descriptor` field marks a tool as stateful, and adding one for a single
current use case is exactly the kind of premature generalization this
project has consistently avoided (M3a's `Requires::All` addition was the
opposite case: real, immediate, multi-site need).

After the palette or tray runs `keep.awake` and gets its `ToolOutcome`
back, the app emits a small Tauri event (`"keep-awake://changed"`, carrying
the new boolean from `inhibit_toggle::is_active()`) from the Tauri command
that handles tool execution (`commands.rs`) — targeted to `keep.awake`'s id
specifically, not a general per-tool-run broadcast. `tray.rs` listens for
this event and calls `.set_checked(...)` on the `CheckMenuItem` handle it
kept from building the menu.

### 3.4 Quit safety

`tray.rs`'s `"quit"` handler, before calling `app.exit(0)`, checks
`inhibit_toggle::is_active()` and closes it first if true (best-effort — a
failed close must not block quitting). This is the one reliable place a
normal exit always passes through, and it directly closes the gap §2's
verified finding opened: without it, quitting Wield while keep-awake is
active would leave the screen unable to sleep with no way to fix it short
of logging out. A hard crash or `kill -9` still isn't covered — that's an
inherent limit of the portal's own design (no process-death auto-release),
not something client code can fully close, and is tracked in
`docs/backlog.md` as a known, honestly-documented gap rather than silently
assumed away.

### 3.5 Result shape: `Report`, not `Value`

`OutputSpec::Value(ValueKind::Text)` (reusing `color.pick`/`screen.ocr`'s
exact pattern) was considered and rejected: a toggle's status text ("on" /
"off") isn't something a user would want on their clipboard, and
`ResultView`'s `Value` card always renders a "Copy" button — offering to
copy "off" is a small, real UX oddity worth avoiding for zero cost.
`OutputSpec::Report`'s `{title, lines}` shape (already used by
`pdf.split`'s batch summary, no Copy button) fits directly: title `"Keep
awake"`, one line stating the new state, with the "on" case adding a short
reminder that it stays active until toggled off again.

## 4. Testing

- `wield-portal/src/adapters/inhibit_toggle.rs`: mirrors
  `pick_color.rs`'s injectable-fake pattern for `InhibitProxy::inhibit`/
  `Request::close` — first toggle turns on and stores state, second toggle
  turns off and clears it, `is_active()` reflects each transition,
  cancellation during a pending `inhibit()` call returns `Cancelled` without
  leaving stale state held.
- `apps/wield/src-tauri/src/tray.rs`: extends the existing
  `build_menu` tests (already using `tauri::test::mock_app()`) with a case
  confirming `keep.awake` renders as a `CheckMenuItem`, initialized
  unchecked; a case confirming the quit handler calls the close path when
  `is_active()` is true and skips it when false.
- Live verification (per the standing UI-walkthrough-testing requirement):
  toggle `keep.awake` from the palette, confirm the tray checkbox reflects
  it; toggle from the tray directly, confirm the palette's next run of the
  same tool sees the flipped state; quit Wield while active, confirm (via
  `busctl`/`hyprctl` or observing actual sleep behavior) the inhibitor is
  released rather than left dangling.

## 5. Out of scope for M3b (tracked separately)

- Recovering from a hard crash/`kill -9` while active (§3.4) —
  `docs/backlog.md`, an inherent portal-design limit, not a Wield gap to
  close.
- Generalizing "stateful tool" into a real `Descriptor` field — revisit only
  if a second stateful tool is ever added; one tool doesn't justify the
  abstraction yet.

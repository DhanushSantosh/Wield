# Testing Wield

## Palette latency

Every `show_palette` call measures the time from requesting `show()` through the
subsequent focus request. The shell records the result as a structured tracing
event named `palette shown` with an `elapsed_ms` field.

Daily log files live under `$XDG_STATE_HOME/wield/logs/`. When
`XDG_STATE_HOME` is unset, the fallback is
`$HOME/.local/state/wield/logs/`. Read the current `wield.log.YYYY-MM-DD` file
and filter for `palette shown` to inspect measurements.

P5a instruments the hidden, pre-warmed shell window. The latency acceptance
verdict belongs to P6, once the real palette UI and its render work are present.

## P5a completion smoke

On 2026-09-11, the ignored graphical launch test passed in the active Wayland/X11
session. A manual primary launch remained headless and appeared in `busctl
--user list` as `io.github.DhanushSantosh.Wield`. A second launch relayed
`ShowPalette` to the primary and exited with status 0.

The completion gate was:

```text
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p wield-app -- --ignored --list
cargo test -p wield-app --test launch -- --ignored
npm run check
```

All commands passed. One initial workspace test run transiently encountered
Linux `ETXTBSY` in the existing `wield-core` temporary-executable test; the
isolated retry and the complete workspace rerun both passed without code
changes.

Installed dependency APIs required two mechanical plan corrections: the Tauri
binary target is `wield-app` (and therefore uses
`CARGO_BIN_EXE_wield-app`), and zbus 5.19 reports a non-queued taken name as
`zbus::Error::NameTaken`. The single-instance test injects private bus
connections directly rather than mutating `DBUS_SESSION_BUS_ADDRESS` globally.

## Tray and global shortcut (P5b)

### `GlobalShortcuts` has no session-restore token in `ashpd` 0.13

Checked directly against the vendored `ashpd-0.13.13` source before writing P5b:
`InputCapture`, `RemoteDesktop`, and `ScreenCast` each expose a
`restore_token` on their session-creation options; `GlobalShortcuts`'s
`CreateSessionOptions` does not. Practical effect: **every launch re-shows
the desktop environment's shortcut-binding confirmation dialog** — there is
no way, with this `ashpd` version, to bind once and have subsequent launches
skip the dialog. This is a real upstream limitation, not a Wield bug. Revisit
when a newer `ashpd` adds restore-token support for this portal.

### Reading the bind outcome

`bind_show_palette` logs `GlobalShortcuts portal unavailable` (a `tracing::warn!`)
only on the `Unavailable` path; a successful bind is silent in the log (no
line to search for — its absence, plus a later `HotkeyState::Registered`
from `hotkey_status`, is the positive signal). On 2026-09-11, in this
environment, `org.freedesktop.portal.Desktop` advertises `GlobalShortcuts`
v1 and a real manual launch bound the shortcut with **no** `Unavailable`
warning in the log — i.e. `Bound`. A standalone check (`GlobalShortcuts::new()`
→ `create_session` → `bind_shortcuts`) also returned success outside the
shell. Whether that reflects a real portal backend or one that auto-approves
without an interactive confirmation dialog wasn't determined; either way, the
code path exercised is the real one, not a mock.

### Exit semantics

P5a's `run()` closure unconditionally called `api.prevent_exit()` on every
`RunEvent::ExitRequested`, which would have swallowed a genuine quit. P5b
replaces it with:

- Per-window `on_window_event` handlers on `palette` and `preferences` that
  intercept `WindowEvent::CloseRequested`, call `api.prevent_close()`, and
  hide the window instead of destroying it.
- An empty `app.run(|_app, _event| {})` — no app-level exit prevention at all.
- `commands::quit` / the tray's "Quit" item call `AppHandle::exit(0)` directly,
  which is Tauri's documented forceful-exit path and is not itself routed
  back through a preventable `RunEvent::ExitRequested` in the installed
  Tauri (2.11.5).

This was not exercised end-to-end against a live tray click in this
environment (no interactive session to click a real tray menu item) — the
manual smoke below confirms the tray builds and the shell stays headless and
reachable; treat "Quit actually terminates the process" as verified by
reading the Tauri API contract for `AppHandle::exit`, not by an automated or
interactive test here. If a future environment can click the tray, confirm
this and remove this caveat.

### P5b completion smoke

On 2026-09-11, a manual primary launch (`XDG_STATE_HOME` pointed at a scratch
directory) remained headless, appeared in `busctl --user list` as
`io.github.DhanushSantosh.Wield`, and its log showed the tray building
successfully (an `libayatana-appindicator` deprecation notice confirms an SNI
host was found and used — not a failure) and no `GlobalShortcuts portal
unavailable` warning (i.e. the shortcut bound). No tray-click / hotkey-press
interaction was performed (no interactive session available here).

### P6a: hotkey press did not surface the palette in one interactive session

On 2026-09-11, reviewing P6a with a real interactive desktop session
available (via `computer-use`), the shell was launched with the real
bundled UI and pressing `Super+W` was attempted to trigger the bound
`GlobalShortcuts` shortcut. The palette window did not appear — the
desktop's own window manager most likely intercepts `Super+W` before it
reaches the portal (a very common WM binding), rather than this being a
Wield bug: the backend log showed the bind succeeding with no
`Unavailable` warning, and the shell continued running normally (still
owned the D-Bus name afterward). Not chased further in this review —
window-manager shortcut conflicts are an environment concern the spec
already anticipates (§5's fallback path: the tray, or a manually-bound
command, when the portal hotkey doesn't reach the app). The `--ignored`
launch smoke test and every automated frontend/backend test still passed;
only the live end-to-end "press the hotkey and see the palette" interaction
is unverified in this session.

## P7: `kills_on_timeout` flaked repeatedly in real CI

On 2026-09-11, once P7's CI workflow gave this project's first real GitHub
Actions runner (not just this local dev sandbox), `wield-core`'s
`command_runner::kills_on_timeout` test flaked three times in a row on the
same branch — always the same failure, `assertion failed:
matches!(result, CommandResult::Timeout)`, meaning the spawned
`sleep 30` child reported as successfully *exited* well within the test's
300ms timeout window instead of being killed for overrunning it.

Two things were tried:

1. **Hardened `write_stub_script`** (the shared test helper that writes,
   chmods, and returns the path to a temp shell script) to explicitly
   `sync_all()` the file handle before returning, on the theory that a
   write/exec race on the runner's filesystem was letting a partially
   written (and therefore fast-exiting) script get executed. This did not
   fix it — the same failure recurred on the very next CI run, ruling this
   out as the primary cause. Left in place anyway (it is a real hardening,
   just not sufficient alone, and reverting it would be pure churn).
2. **`--test-threads=1`** on `cargo test --workspace` in `ci.yml`. This
   test binary spawns several short-lived child processes across its 6
   tests, which by default run concurrently; `kills_on_timeout` is the one
   test racing a *short* (300ms) timeout against a long-sleeping child in a
   suite otherwise full of fast-exiting ones — exactly the shape of
   workload where a rare async-runtime process-reaping mixup under heavy
   concurrent spawn/reap churn (a known category of issue, not specific to
   this codebase) could plausibly cause one child's exit notification to
   get misattributed to another. Serializing test execution removes that
   concurrent-spawn pressure entirely. This is the fix this project's own
   history already anticipated needing ("candidate for P7 CI tuning
   (--test-threads limit)", noted back in P5a) — applied here once a real
   CI runner finally gave it a chance to actually surface.

Never reproduced locally in this dev sandbox, in either failing or fixed
form, across 15 stress-test runs — consistent with this being specific to
the shared, contended CI runner environment, not a local reproduction path.

**Confirms the diagnosis:** `ci.yml`'s own `cargo test --workspace --
--test-threads=1` step passed clean on the very next run, but that same
run's `npm run check` step — which runs `cargo test` a *second* time via
`scripts/run-cargo.js` (root `package.json`'s `test` script), a separate,
un-serialized invocation this fix hadn't reached yet — flaked on
`cancels_promptly` instead (same file, same family, different specific
test). Fixed by adding `-- --test-threads=1` to both `package.json` script
definitions (`test` and `cargo:test`) that shell out to cargo, not just
`ci.yml`'s own direct step. If it recurs a third way, the next step is a
real root-cause investigation of `CommandRunner`'s `tokio::select!` race in
`crates/wield-core/src/command.rs` against process-reaping behavior
specifically, not another blind mitigation attempt.

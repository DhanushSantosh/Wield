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

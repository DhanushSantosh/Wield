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

## Wayland layer-shell positioning

Verified live on 2026-09-12 in this project's Hyprland session. The first
implementation followed the original design's four-edge anchoring proposal and
incorrectly filled the compositor's entire 1920×1040 usable area. This matches
gtk-layer-shell's installed API documentation: opposite anchors stretch a
surface in that direction and make GTK ignore its requested size. The preview
was stopped, the implementation was corrected to leave every edge unanchored,
and the design and plan were amended to record the verified semantics.

After rebuilding, Hyprland reported the palette as a 720×480 overlay-layer
surface at `(2520, 320)` on the focused `eDP-1` output. That output's usable
rectangle was `(1920, 40)` through `(3840, 1080)`, so the reported position is
exactly centered in both axes. The surface was absent from `hyprctl clients`,
confirming it was a layer surface rather than a regular Wayland toplevel. The
log contained `layer-shell positioning enabled`, confirming the capability
probe selected the intended path. Triggering Preferences through its exported
D-Bus menu item produced a 640×480 layer surface at `(2560, 320)`, also exactly
centered in the same usable rectangle.

Keyboard entry was not synthetically injected into the user's active desktop,
so focus is supported by the configured `KeyboardMode::OnDemand` path but not
claimed as live keystroke-verified here. Resize-while-centered was not tested:
the dynamic-resize feature remains on an unmerged sibling branch and this work
was explicitly based on `master`. KDE/KWin, Sway, other wlroots compositors,
GNOME's unsupported-protocol fallback, and X11 were not tested in this pass.

### `OnDemand` doesn't work on this Hyprland version; `Exclusive` has a real tradeoff

On 2026-09-13, real usage on the owner's desktop surfaced two problems the
initial verification above didn't catch (it never actually pressed a key or
clicked away): with `KeyboardMode::OnDemand`, neither Escape-to-hide nor
click-away-to-hide worked — the palette could only be closed by quitting the
whole app from the tray.

Investigated with `wtype`/`ydotool` (synthetic Wayland input — far more
reliable than manual clicking for isolating this) rather than more manual
testing:

- `OnDemand`: a synthetic `Escape` did nothing. Clicking directly inside the
  window first, then sending `Escape`, still did nothing. `hyprctl
  activewindow` confirmed the palette never became the focused window at
  all after `ShowPalette`. This matches a long-standing upstream report that
  Hyprland's `ON_DEMAND` handling doesn't behave as the protocol describes
  (<https://github.com/hyprwm/Hyprland/issues/2264>) — on this compositor
  version (0.56.2), it appears to just never grant focus, click or no click.
- `Exclusive`: confirmed Escape-to-hide now works reliably. But `hyprctl
  activewindow` stayed on the previously-active app even while the palette
  was shown and mapped, and a real click on another window while the
  palette was open did not register at all (the owner directly observed
  this: "i couldnt register a click at all couldnt even click the main
  foreground window"). This is effectively modal behavior for as long as
  the palette is mapped, not merely "guaranteed keyboard focus" as the
  protocol's naming might suggest.

**Decision (owner, 2026-09-13):** ship `Exclusive` and document the tradeoff,
rather than keep chasing a fully clean fix. Escape (or the tray) are the
supported ways to dismiss the palette; click-away-to-dismiss does not work
while it's shown. Revisit if Hyprland's `on_demand` handling improves, or if
a different mechanism — Wield detecting an outside click itself rather than
relying on the compositor's normal focus handoff — turns out to be worth the
added complexity.

### The palette intermittently failed to render at all — root-caused and fixed

Also on 2026-09-13, real usage surfaced something more severe than the
keyboard-focus tradeoff above: the palette sometimes didn't visually appear
at all after `ShowPalette` — `hyprctl layers` still reported it mapped at
the correct, centered geometry, but nothing was actually painted, letting
whatever was underneath show through untouched.

Confirmed with `grim` (a native Wayland screenshot tool — far more reliable
for this than manual observation or the general-purpose screenshot tooling
used earlier in this project's history): captured the exact screen region
the palette should occupy and got a picture of a completely unrelated
window instead. Initially suspected this was specific to the (at-the-time
untested) combination of the dynamic-resize feature with layer-shell — a
`set_size()` call not participating correctly in a layer-shell surface's
own configure/commit handshake. That turned out to be a real, separate bug
(fixed regardless — see the `palette::animate_to_height` commit — Tauri's
generic `set_size()` doesn't work correctly on a layer-shell surface;
gtk-layer-shell's own docs specify `set_size_request()` + `resize(1, 1)`
instead), but re-testing the *simplest possible* build (layer-shell only,
no resize code at all, identical binary re-launched fresh) still failed
non-deterministically — proving it wasn't resize-specific at all, just a
general layer-shell rendering flake.

This matches a real, documented gtk-layer-shell C-library function:
`gtk_layer_try_force_commit()`, whose own doc comment describes exactly
this scenario — "the surface is in a state where it does not receive frame
callbacks and the regular deferred commit mechanism is unavailable." That
function isn't exposed by either the safe `gtk-layer-shell` crate (0.8.2)
or its `-sys` bindings (0.7.2) — added to the C library after these
(already-unmaintained) Rust bindings were last updated. Declared it by
hand as a minimal `extern "C"` binding (`apps/wield/src-tauri/src/layer_shell.rs`,
`force_commit`) and call it right after `window.show()` in
`palette::show()`.

**Verified**: 5 consecutive fresh launches (new process, `ShowPalette`,
`grim` screenshot, every time) all rendered correctly after this fix, versus
frequent failures before it. Not an absolute guarantee — the underlying
flake's exact trigger condition was never fully understood, only worked
around via the mechanism the C library's own author built for exactly this
situation — but a meaningful, real, repeatably-verified improvement, not
a guess.

A second real bug was caught and fixed in the process of building this: the
first version of the `force_commit` call touched a raw GTK object directly
from `palette::show()` without dispatching through
`WebviewWindow::run_on_main_thread()` first. Since `show()` is invoked from
the D-Bus `ShowPalette` handler (not guaranteed to already be on the GTK
main thread) and raw GTK objects are not thread-safe, this caused the
`ShowPalette` D-Bus call to hang outright (`busctl` reported "Connection
timed out"). Fixed by wrapping the same way the resize code already
correctly does.

### A second, distinct non-rendering bug — deterministic this time, and unrelated to `force_commit`

Still on 2026-09-13, combining the `force_commit` fix above with the
dynamic-resize feature (`feat/dynamic-palette-resize`) for a joint test
turned up a second, worse bug: the palette failed to render on **every**
fresh launch (not intermittently) once resize was in the build, even with
`force_commit` in place.

Found a much faster and more precise signal than screenshots for this:
`hyprctl layers -j` reports an `alpha` field per layer surface. A working
palette reports `alpha: 1`; this one reported `alpha: 0` — and stayed at
`0` indefinitely (polled every 100ms for 2 seconds) rather than animating
through and settling on `1`, i.e. genuinely stuck, not mid-transition.

Root-caused by elimination, in order:

1. Suspected the resize animation itself (twelve rapid
   `set_size_request()` + `resize(1, 1)` calls in 180ms right after
   `show()`) was overwhelming the single `force_commit()` call. Added a
   second `force_commit()` after every resize step. **No change** —
   `alpha` still stuck at 0, 5/5 fresh launches.
2. Disabled the resize animation entirely (temporarily made
   `animate_to_height` a no-op) to isolate whether resize was involved at
   all. **Still no change** — `alpha` still stuck at 0, 5/5 fresh
   launches, with *no* resize code running whatsoever. This ruled out
   resize, and by extension `force_commit`, completely.
3. Re-tested the isolated `fix/layer-shell-keyboard-focus` branch (the one
   the original 5/5 `force_commit` verification above was done on) fresh,
   using this same `alpha` check instead of eyeballing screenshots: a
   clean 5/5 at `alpha: 1`. So something genuinely differed between that
   branch and the combined one — not a re-run of the same flake.
4. Diffed the two branches. One line stood out immediately in
   `apps/wield/src-tauri/tauri.conf.json`: the combined branch had flipped
   the palette window's `"resizable"` from `false` to `true` (presumably
   while building the resize feature, on the assumption a
   programmatically-resized window needs to be marked resizable — it
   doesn't; `gtk_window_resize()` works regardless). Flipped it back to
   `false` with everything else (resize code included, `force_commit`
   included) left untouched, rebuilt, and re-ran the stress test: **10/10
   fresh launches at `alpha: 1`**, correct final geometry
   (`720×216`, re-centered correctly), and a visual `grim` screenshot
   confirming the palette actually on screen.

So `resizable: true` on a `gtk-layer-shell` surface reliably breaks
rendering outright on this Hyprland version — a separate, deterministic
bug, not a rarer version of the `force_commit` flake, and not fixed by
`force_commit` at all. The per-step `force_commit` addition from step 1
above was reverted (`git stash drop`) once the real cause was found — it
wasn't wrong to try, but it wasn't the fix, and keeping unnecessary
workarounds around makes the next investigation harder, not easier.

**Lesson**: `hyprctl layers`' `alpha` field is a much more precise
diagnostic for "is this surface actually rendering" than a screenshot —
it's instant, scriptable, and distinguishes "still animating" from
"genuinely stuck" in a way a single screenshot can't. Prefer it over
`grim` for this specific class of bug going forward; keep `grim` for
confirming what's *visually* on screen once `alpha` says it should be.

### The palette rendered see-through — a user's own compositor config, not our code

On 2026-09-13, after the positioning/sizing fixes above, live feedback
("I don't see any difference, I just want the empty spaces on the sides
gone") didn't match what the code changes should have produced. A `grim`
crop of the actual rendered palette (not just a full-desktop screenshot,
which made this easy to miss) showed why: the card was rendering as a
near-fully-blurred, see-through ghost of itself — another app's media
player, album art included, was clearly visible *through* the palette's
supposedly solid `var(--bg)` background.

Root cause: `gtk-layer-shell` defaults every window to the literal
namespace `"gtk-layer-shell"` unless the app sets its own, and this dev
machine's Hyprland config (`~/.config/hypr/hyprland/rules.lua`, part of
the dots-hyprland/"illogical impulse" rice) has:

```lua
hl.layer_rule({ match = { namespace = "gtk-layer-shell" }, blur = true})
hl.layer_rule({ match = { namespace = "gtk-layer-shell" }, ignore_alpha = 0})
```

— rules clearly written for *some other* simple gtk-layer-shell-based
utility that wants full blur passthrough, not a considered choice about
Wield. Every app that never bothers to set its own namespace collides
with whatever a user's compositor config assumes about "generic
gtk-layer-shell apps."

Fixed by calling `set_namespace()` (present in the `LayerShell` trait,
previously unused) with a distinct name per window — `wield-palette`,
`wield-preferences` — in `layer_shell::configure()`. Confirmed via a
tight `grim` crop immediately after `ShowPalette`: solid, fully opaque
card, no ghosting, before touching anything compositor-side.

**Lesson**: a full-desktop screenshot can look "close enough" at a
glance and hide a real rendering bug; always crop tightly to the actual
surface being tested. And: never leave a layer-shell surface on the
library's default namespace in a real app — a user's own compositor
rules can already be targeting it for reasons that have nothing to do
with your app.

### Removing the card's side margin broke its rounded corners

Once the ghosting above was fixed, live feedback made clear the ~40px
per-side gap between the card and the window (`.app-shell`'s
`max-width` vs. the window's own width) read as dead space, not
intentional framing. Removing it entirely (card = window width) fixed
that, but broke the corners: `border-radius: 15px` on `.app-shell`
started rendering as a hard 90° corner instead of a curve, confirmed
with a `grim` crop zoomed into just one corner.

The window was never given Tauri's `"transparent": true` config flag.
Best-guess mechanism (not independently confirmed in the compositor's
own source, but consistent with every observation): GTK/Wayland
toolkits track an "opaque region" per surface as a rendering
optimization - the area a compositor can skip alpha-blending entirely.
With the card smaller than the window, part of the surface was never
painted with any opaque color at all, so the toolkit correctly inferred
a non-opaque region and blended it properly (rounded corners included).
Once the card's own solid background covered 100% of the window, the
toolkit likely inferred the *entire* surface as opaque, and the
rounded corner's own transparent cutout - which depends on real
per-pixel alpha blending - got clamped to opaque too.

Adding `"transparent": true` restored a curve, but a badly-aliased one:
zoomed in, the cutout area was a blocky, dithered blue-gray, not the
smooth flat blue confirmed to actually be there (hid the palette,
screenshotted the same exact pixels, compared directly). Something
about compositing real per-pixel alpha at the *exact* window edge, with
zero margin, produces poor-quality output even when it technically
isn't opaque anymore.

Landed a middle ground instead of chasing pixel-perfect edge-of-surface
alpha further: `max-width: calc(100% - 40px)` - a 20px margin per side,
just past the 15px corner radius so the curve has room to fully form.
Confirmed clean (smooth curve, no artifact, matching the quality of the
very first working screenshots) via the same hide-and-compare method.
20px reads as "this has rounded corners" rather than "there's empty
space here" - a real, load-bearing distinction that cost several rounds
of live feedback to actually pin down precisely.

**Lesson**: "no margin" and "a small margin" are not points on the same
continuum as far as rendering quality goes on this stack - zero margin
hit a real, qualitatively different code path (whole-surface-opaque
inference) than even a small nonzero one. When chasing a "make the gap
smaller" request, verify the *smallest* gap that still works before
assuming zero is just the limit of the same trend.

### The Preferences window had no way to close itself — merged into the palette instead

Also on 2026-09-13: opening Preferences (tray → "Preferences") left a
window on screen with no way to dismiss it. `KeyboardMode::Exclusive`
(the same tradeoff already accepted and documented for the palette)
meant clicking anywhere else did nothing, and unlike the palette,
Preferences had no Escape handler and no close button at all — nothing
in its own code path ever called `preferences::hide()`. The only way
out was killing the whole process.

Rather than give the Preferences window its own copy of every fix
already made for the palette (Escape handling, a close affordance,
`resizable: false` for the exact same alpha-stuck-at-0 flake docs above
already root-caused, and - discovered while fixing that - a *second*,
new bug where an unanchored non-resizable layer-shell window with no
active height constraint grew to the full monitor height instead of the
requested 480px), Preferences was folded into the palette's own view
state machine instead (`App.tsx`'s `View` union gained a `"settings"`
member, rendering `SettingsView`). It inherits the palette's already-
solid show/hide/resize/keyboard/layer-shell handling for free - there
is only ever one window to keep correct now, not two.

A gear icon in the search bar's input row opens it; an X button and
Escape both return to search, exactly like the existing `"form"` view.
The tray's "Preferences" item now emits a `tray://open-settings` event
and shows the palette, mirroring the existing tool-selection path,
instead of showing a second window that no longer exists.

The Settings content (Hotkey / Palette behaviour / System status) is
tall enough - the portal-support table alone lists twenty-plus rows -
that it needs its own scroll region (`max-height` + `overflow-y: auto`
on `.settings-view__body`) rather than letting the palette's own
`animate_to_height` grow to fit all of it; MAX_HEIGHT there is 640px,
well short of the full list.

**Lesson**: a second window means a second copy of every fix already
made for the first one, indefinitely - a real reason to prefer folding
a secondary UI into an already-hardened surface over giving it its own
window, when the UI doesn't specifically need one.

### CI failed on the `force_commit` fix: a hard link-time dependency the dev machine couldn't have caught

Opening PR #10, CI failed both jobs with `undefined symbol:
gtk_layer_try_force_commit` at the link step - `wield-app` itself
wouldn't link. The dev machine's local build was, and had always been,
completely unaffected.

Cause: `force_commit`'s `extern "C" { fn gtk_layer_try_force_commit(...); }`
block (see above) makes the symbol a *hard link-time requirement* -
the linker fails outright if it's not in whatever `libgtk-layer-shell`
the build machine has installed. The dev machine has 0.10.1, new enough
to export it. GitHub's `ubuntu-latest` runners install an older
`libgtk-layer-shell-dev` via `apt` (added for CI in PR #9's own fix,
before this function existed) that doesn't. This was never a CI
environment gap to patch (there's no newer package to install to) - the
symbol may just not exist on a given system, exactly as the function's
own doc comment already says.

Fixed by resolving the symbol at runtime instead of at link time:
`libloading::os::unix::Library::this()` (a handle to the current
process's own already-loaded symbol table - gtk-layer-shell is already
linked in via `gtk-layer-shell-sys`, just not necessarily *this*
symbol) plus `.get::<fn(...)>(b"gtk_layer_try_force_commit\0")`. Missing
now means `None`, handled identically to the existing "not a layer
window" no-op case - not a build failure, on *any* system, not only CI.
`libloading` pinned to 0.7 in `Cargo.toml` to match the version already
resolved transitively (via `tray-icon`'s own `dlopen2` dependency)
rather than compiling a second copy.

Confirmed after the fix: `cargo build --release` links clean, and a
fresh 3/3 launch-and-check-alpha spot-check on the dev machine still
shows the workaround actually firing (`alpha: 1` every time) - the
runtime lookup finds and calls the same symbol just as reliably as the
hard-linked version did there.

**Lesson**: a workaround built against "the C header documents this
function" is still only as portable as the *library version* that
exports it. A local build succeeding proves nothing about a symbol's
portability - only CI (or another machine) surfaces that gap. Prefer a
runtime lookup over `extern "C"` for any symbol whose presence isn't
guaranteed by the crate's own declared version.

## Click-away-to-dismiss: a second surface didn't work, a full-screen one did

On 2026-09-13/14, followed up on the "click-away-to-dismiss does not work"
tradeoff documented above. Two designs were tried; the first failed for a
reason worth recording in full, since it cost real implementation effort
before the failure was understood.

### First attempt: a separate "click-catcher" surface — built, reviewed, then found non-functional

The first design (spec'd, planned, and implemented across five reviewed
commits on `fix/click-catcher-design`) added a second, fully invisible,
full-output `gtk::Window` behind the palette (`Layer::Top`,
`KeyboardMode::None`) whose only job was to catch a click anywhere outside
the palette's own small box and call `palette::hide()`. It compiled clean,
passed every task review, and rendered correctly (`hyprctl layers` showed
it mapped at the right geometry with `alpha: 1`).

Live testing found it did not work at all: a realistic-timing `ydotool`
click squarely within the catcher's own mapped bounds, clearly outside the
palette, never dismissed the palette in either of two scenarios (the
same-monitor previously-focused window; a different, previously-unfocused
window on a different monitor). Root-caused with temporary
`tracing::info!` diagnostics added to the catcher's own click handler,
rebuilt, and reproduced live: **the handler's log line never printed** -
the click never reached the catcher's surface at all, not a logic bug in
what ran after.

Ruled out first, to isolate the real cause: general Exclusive-mode pointer
blocking wasn't it (a click landed squarely on the palette's own "Pick a
colour" row and correctly activated the tool - confirmed via a fresh
`org.freedesktop.portal.Request` proxy appearing in the log and the
color-picker capture overlay appearing on screen); the synthetic-input
mechanism itself wasn't it (baseline clicks with no Wield surface shown
correctly changed `hyprctl activewindow`, both same-monitor and
cross-monitor).

Root cause, confirmed via an upstream search rather than guessed: Hyprland
has an open, maintainer-unanswered bug report,
[hyprwm/Hyprland#14136](https://github.com/hyprwm/Hyprland/discussions/14136),
stating that a layer-shell surface holding `KeyboardMode::Exclusive`
captures *all* pointer button events on this compositor - regardless of
cursor position or the surface's own input region - unlike Sway and niri,
which correctly let pointer events fall through to whatever the cursor is
actually over. While the palette holds `Exclusive`, Hyprland was never
doing real hit-testing against any other surface, including a second
surface owned by the same process. No client-side configuration of a
second surface (anchors, layer, input region) could have worked around
this - the bug is in what the compositor delivers events *to*, not in
anything a client requests.

The whole click-catcher subsystem (`click_catcher.rs`, its `AppState`
plumbing, and its wiring into `palette::show`/`hide`) was removed rather
than left in place unused, once the pivot below replaced it.

### Second attempt: make the palette's own surface full-screen — this is what shipped

Since Hyprland only ever delivers pointer events to the surface holding
`KeyboardMode::Exclusive`, the fix that follows directly from the root
cause is to make *that* surface (the palette's own) cover the whole
output, rather than adding a second one. `layer_shell::configure` now
anchors the palette's `gtk::ApplicationWindow` to all four edges (full
output) instead of just the top edge; the small visible "card" the user
actually sees is positioned by CSS alone, as a child of a full-viewport,
fully transparent `.palette-backdrop` (`App.tsx` / `styles.css`). A click
anywhere on the output is now, structurally, a click on the palette's own
surface - ordinary in-page DOM hit-testing (already proven reliable: this
is exactly how clicking a result row already worked) decides whether it
landed on the backdrop (dismiss) or bubbled up from the card (do nothing).

This also retired the palette's Rust-driven window-resize animation
(`animate_to_height`/`resize_steps`, the `resize_palette` command): with
the OS window now permanently full-screen, there is no window left to
resize - gtk-layer-shell ignores requested sizes on a surface anchored to
all four edges by design, matching `layer_shell::configure`'s own
long-standing warning about this for the opposite case. The card's height
now animates with a plain CSS `transition: height`, driven by a
`ResizeObserver` on an inner content wrapper (not on the card element
itself, to avoid observing your own write) rather than an IPC round trip
per resize step.

**Verified live** (fresh build installed to `~/.local/bin/wield-app`,
`hyprctl layers -j` / `hyprctl activewindow -j` / `grim` / `ydotool` with
realistic per-key and per-click timing throughout):

- `hyprctl layers` shows a single `wield-palette` surface covering the
  full output (minus a ~45px strip at the top reserved by this desktop's
  own status bar's exclusive zone - harmless, well clear of where the
  card renders), `alpha: 1`.
- Clicking the real, previously-focused window underneath (tested against
  a real running app, not a disposable test window) correctly dismissed
  the palette - the exact case the original bug report described
  ("i couldnt register a click at all couldnt even click the main
  foreground window") and the case the click-catcher failed to fix.
- Clicking inside the card does not dismiss it, and normal interaction
  keeps working: a click on a result row activates it (confirmed via the
  color-picker capture), and typing into the search box filters results
  live while the palette stays mapped.
- Escape still hides the palette (or backs out one view level first, e.g.
  result → search), unaffected by any of the above.
- A 5-run fresh-launch stress test (kill, relaunch, show, check `alpha`)
  came back `alpha: 1` every time - `force_commit` is working for the
  full-screen surface exactly as reliably as it did for the smaller one.

**Known gap, found while testing this, not introduced by this change:**
clicking a genuinely different, unfocused monitor does *not* dismiss the
palette either, confirmed against two different real target windows
(ChatGPT and, separately, the Claude desktop app). This directly
contradicts an earlier note in this document (above) that claimed the
cross-monitor case "worked" via the existing blur listener - given what
#14136 actually says (Hyprland drops pointer events to *every* surface but
the Exclusive one, full stop, not just "other surfaces on the same
output"), that earlier finding was almost certainly a stale or
mistimed test predating this investigation, not a real, still-true
behavior. Closing this gap completely would mean creating a full-screen
backdrop surface *per connected output* (wlr-layer-shell ties one layer
surface to at most one `wl_output`), each independently forwarding a
dismiss - real, non-trivial added complexity (per-output surface
lifecycle, monitor hotplug) for a materially rarer interaction than the
same-monitor case this fix directly targets. Left as a documented
limitation rather than chased further: Escape or the tray remain the
reliable way to dismiss when the palette is shown on a monitor other than
the one you want to click.

**Rare timing hazard, observed once, not reproduced on demand:** a single
early test that clicked the search box and then immediately sent three
keystrokes with no delay ended up with the palette dismissed and the
typed text landing in a completely different application's own input box.
Every later, more deliberate repeat (click, confirm state, type one
character at a time, confirm state after each) worked correctly with no
issue. The likely explanation is a race with the pre-existing
`window.addEventListener("blur", ...)` hide-on-blur listener (not the new
backdrop-click code) rather than anything about this fix specifically -
matches this project's existing "give synthetic input realistic timing,
not instantaneous batches" lesson. Noted rather than chased further,
since it did not reproduce under normal interaction timing.

## M2a: `video.convert` — real ffmpeg progress and multi-file batch

On 2026-09-14, live-verified `video.convert` end-to-end on the real
desktop (ffmpeg `n9.0`, the same build the progress-parser design was
researched against) after the four implementation tasks landed. This is
the first M2 (Converter suite) tool, and the milestone that proves the two
mechanisms M2b-d reuse: real `ProgressSpec`-driven percentage progress, and
multi-file batch via `ArgValue::Paths`.

**Single-file conversion:** searched "video", selected `Convert video`,
picked a real generated test clip through the native multi-select file
picker (already wired for `multiple: true` with zero frontend code
changes needed - confirmed live, not just by reading the code), ran it
with default options (format `mp4`, resolution `original`). The result
card showed a real `ToolOutcome::Report` success (`"1 of 1 converted"`),
**not** `ToolOutcome::File` as this section originally (and incorrectly)
claimed — corrected 2026-09-14 after M2b's own live verification
independently caught the discrepancy. `video.convert`'s `input` is
`multiple: true`, so even a single selection through the picker is a
one-element JSON array; `coerce_json_args` turns that into
`ArgValue::Paths` regardless of length, and `Executor::run_command`
dispatches to `run_batch` unconditionally whenever any `ArgValue::Paths`
is present (confirmed by reading `batch_paths`/`run_command` directly —
there is no length check anywhere in that path). A `multiple: true`
descriptor can therefore never produce `ToolOutcome::File`, single
selection or not; the underlying conversion itself was always correct,
this was a documentation-only error. Verified independently via `ffprobe`
(not just trusting the UI): the output file's size and mtime changed,
duration was preserved exactly - a genuine re-encode, not a no-op copy.
(Converting `mp4` → `mp4` with `OutputDir::SameAsInput` overwrites the
source file in place - inherited, pre-existing behavior from
`image.convert`'s identical naming/output-dir pattern, not new to this
milestone; worth knowing, not a defect.)

**Multi-file batch - the core new mechanism, proven working end-to-end:**
selected 3 real test clips at once through the same picker (multi-select,
`Ctrl+A` in the native dialog's file-list view), ran the conversion. The
result card showed a real `ToolOutcome::Report`:

```
3 of 3 converted
✓ clip-1.mp4 → /tmp/m2a-test/clip-1.mp4
✓ clip-2.mp4 → /tmp/m2a-test/clip-2.mp4
✓ clip-3.mp4 → /tmp/m2a-test/clip-3.mp4
```

exactly matching the design's `"{succeeded} of {total} converted"` format.
Verified independently via `ffprobe` on all three output files: every one
had a changed mtime/size matching the batch run's timestamp and a
correctly-preserved duration - three genuine, separate ffmpeg
invocations, not one call operating on the first file only. This
confirms the full chain worked for real: native multi-select picker →
`ArgValue::Paths` (via `coerce_json_args`'s new branch) → `Executor`'s
batch detection → `run_batch` looping the *existing*, unmodified
single-file path once per file → `ToolOutcome::Report` aggregation → the
existing Result view rendering it correctly with zero new frontend code.

**Batch failure handling - found incidentally, real coverage nonetheless:**
during file-picker testing, a GTK location-bar autocomplete quirk (typing
a bare directory path while its own filename-completion popup was open,
then pressing Enter) caused the picker to return the *directory itself*
as if it were a one-item file selection - a testing-methodology artifact,
not a Wield bug. `video.convert` handled it correctly: ffmpeg rejected the
invalid input immediately (exit code 254), and the result came back as a
clean `Report` (`"0 of 1 converted"` with the failure reason on its own
line) rather than a crash or a stuck spinner - incidental but genuine
live coverage of the batch's per-file failure path, not just its
all-success path.

**Not independently reproduced live, with honest reasons why:**

- **Mid-batch cancellation via the real UI.** Repeated attempts to
  multi-select several longer (20s, 720p) test clips through the native
  picker kept running into the same GTK location-bar autocomplete
  behavior described above, which ate enough time that further pursuit
  stopped being worth it relative to what it would add. The underlying
  mechanism is not unverified, though: `Executor::run_batch`'s
  cancel-mid-batch logic has one dedicated automated test
  (`batch_stops_before_the_next_file_once_cancelled`, covering the
  mid-file path — the loop-top "stop before the next file" branch is
  verified by code review only, not a second automated test) and
  the underlying `CommandRunner`-level cancellation Escape triggers is the
  same, unchanged mechanism already live-verified for single-file runs
  earlier in this project's history (P5a) and again this session (the
  click-catcher work's own Escape-to-hide testing). This is a testing-tool
  friction, not an unverified code path.
- **The `Unavailable` path** (`ffmpeg` absent → tool shows greyed out
  with a reason). This would need renaming the system `/usr/bin/ffmpeg`,
  which requires `sudo` - not available non-interactively in this
  environment. Not chased further because this is the exact same,
  already-verified `Requires::Binary(...)` → `Unavailable` path
  `image.convert` already uses (live-verified in M1/P4) and covered by
  the existing `missing_binary_is_unavailable` automated test - `video.convert`
  sharing that code path, unmodified, is a structural guarantee, not an
  assumption.

**Three more paths were never run against real ffmpeg during live
verification, closed after the fact by this branch's final whole-branch
review** (not via the desktop UI - by running the exact `ffmpeg` argv
`video.convert`'s descriptor renders, against real ffmpeg `n9.0`):

- **`resolution` scaling.** Both live desktop runs used the default
  `original` (which emits none of the six `arg_when`-gated `-vf
  scale=-2:H` segments). Ran `-vf scale=-2:720` against a 640x480 test
  source: output was correctly 960x720, aspect preserved, exit 0.
- **`quality` (`-crf`).** Never set in either live run. Ran `-crf 23`
  alongside the resolution case above: exit 0, no ffmpeg complaint.
- **Non-`mp4` output formats.** Both live runs used the default `mp4`.
  Ran with `.webm` (libvpx-vp9) and `.mkv` (libx264) output extensions:
  both exit 0, both produce valid output.

No defect found in any of the three. Recorded here rather than left
implicit, since this doc is what a reader (including a future M2b-d
author) would reasonably expect to cover the whole descriptor, not just
its default option values.

## M2b: `audio.extract` — video source, audio source, and batch

On 2026-09-14, live-verified `audio.extract` against the real production
Tauri build and the system ffmpeg/ffprobe. The fixtures were generated in an
isolated `/tmp` directory with ffmpeg's `testsrc` and `sine` sources, then the
running app's real D-Bus `RunTool` entry point exercised the same registry,
argument coercion, executor, progress parser, output planning, and ffmpeg
command path used by the desktop UI.

**Video source:** a three-second MP4 containing H.264 video and AAC audio was
converted with the default MP3 format. Wield reported success and wrote an MP3
audio-only output. Independent ffprobe inspection found codec `mp3`, duration
`3.000000`, and no video stream; the source independently reported both H.264
and AAC streams with the same `3.000000` duration.

**Audio source:** a two-second PCM WAV was converted to FLAC. Wield reported
success, and ffprobe independently found a FLAC audio stream, 16 raw bits per
sample, and duration `2.000000`. This confirms `-vn` is harmless for an
audio-only input and the descriptor is not limited to extracting from video.

**Multi-file batch:** three one-second WAV inputs were converted to M4A with
the `192k` preset. The real outcome was `Report { title: "3 of 3 converted" }`
with one successful output line per input. ffprobe independently identified
all three outputs as AAC in M4A containers, each exactly `1.000000` second.

**One-item outcome correction:** the plan expected a one-file picker selection
to produce `ToolOutcome::File`. In the real application, a `multiple: true`
file field is represented as a JSON array even when it contains one path;
`coerce_json_args` turns that into `ArgValue::Paths`, so the generic batch path
correctly returns `Report { title: "1 of 1 converted" }`. A scalar string was
also checked and correctly rejected as `input must be an array of path
strings`. This is an expectation error in the live-test plan, not a conversion
failure; both one-file conversions above produced and independently verified
valid outputs.

**Conditional Bitrate field:** live screenshots of the real palette confirmed
that `format=mp3` renders the Bitrate selector and `format=flac` hides it. The
new ArgForm regression test also exercises the consequential state sequence
directly: select `320k` while MP3 is active, switch to FLAC, submit, and assert
that the payload is exactly `{ format: "flac" }`. Desktop input automation
could switch and visually confirm the live field states, but reliably clicking
the native picker after the full-screen layer-surface transition was too
fragile to claim a second end-to-end stale-value submission. The actual
filtering branch is therefore live-render-verified plus unit-submission-
verified, not falsely claimed as a complete synthetic pointer-driven run.

The branch test process and its full-screen transparent palette backdrop were
stopped after verification. The user's previously running installed Wield
instance was then restored headlessly.

## M2c: `document.convert` — pandoc path, PDF wrapper, and batch

On 2026-09-14, live-verified `document.convert` against the real production
Tauri build and the installed pandoc 3.10.2 and LibreOffice 26.8.0.3. Three
Markdown fixtures were created in an isolated `/tmp` directory, then the
running app's real D-Bus `RunTool` entry point exercised the same registry,
argument coercion, executor, output planning, and command path used by the
desktop UI. The palette was deliberately not shown during this pass because
its full-screen transparent layer surface would cover the user's workspace;
the native picker and visual result card were therefore not re-tested here.

**Pandoc direct path:** one Markdown input was converted to DOCX. The real
outcome was `Report { title: "1 of 1 converted" }`, consistent with the
one-item behavior of every `multiple: true` file argument. `file` independently
identified the result as `Microsoft Word 2007+`, and reading it back through
pandoc reproduced the heading, paragraph, and list content. This confirms the
result is a valid document rather than merely a file with a `.docx` suffix.

**LibreOffice PDF wrapper:** a different Markdown input was converted to PDF,
exercising the descriptor's `sh -c` PDF branch rather than pandoc's direct
output path. Wield again reported `1 of 1 converted`; `file` identified a PDF
1.7 document with one page, and `pdfinfo` identified LibreOffice Writer as the
creator and LibreOffice 26.8.0.3 as the producer. A directory listing before
and after the run confirmed that only the requested `beta.pdf` result was
added: the temporary copied Markdown input used to control soffice's forced
output name was removed by the wrapper's cleanup trap, with no `.wield-tmp-*`
or other intermediate file left behind.

**Multi-file batch:** the three Markdown fixtures were submitted together for
standalone HTML output. The real outcome was
`Report { title: "3 of 3 converted" }` with one successful line per input.
`file` independently identified all three results as HTML documents, and each
contained the expected standalone `<html>` element. This confirms the full
`ArgValue::Paths` → generic batch executor → one pandoc process per file →
aggregated Report path for the new descriptor.

No conversion defect was found. Other declared input/output combinations
(`html`, `docx`, `odt`, and `rst` inputs; `md` and `odt` outputs) were not each
run live because they use the same already-proven pandoc direct branch. The
descriptor's exact argv for both branches and its full serialized registry
shape remain covered by automated tests.

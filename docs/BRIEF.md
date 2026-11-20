# Agent Brief: Keyboard-First Window Tool for macOS (Rust)

> Project name: **Kevro** (crate, binary and paths use lowercase `kevro`). The brief has two parts: **Part 1** is the long-term vision, for context only. **Part 2** is the Phase 1 task.

---

# PART 1 — The Vision (context, do NOT build all of this now)

## What it is

Kevro is a small, always-running macOS menu-bar utility that lets me **drive my whole Mac's windows and apps from the keyboard**: launch or focus any app with one keypress, snap windows into layouts, move focus between windows by direction, and eventually organise windows into fast, animation-free virtual workspaces. Everything is configured in one plain-text file.

It's a personal, lightweight alternative to combining Raycast app hotkeys, Rectangle and AeroSpace, built in Rust and fitted to how I work.

## Why

- I want to use the mouse as little as possible, especially for repeated actions.
- macOS's built-in tools are weak here. Window tiling shortcuts clash with Mission Control, App Shortcuts can't launch apps, and **native Spaces can't be reordered or targeted from the keyboard**.
- I currently arrange full-screen apps in a set order in Mission Control (by dragging with the mouse) so that `Control + ←/→` lands me where I expect. The real goal is **reaching any app or window instantly without caring about order**.

## Feature roadmap (by difficulty)

| Phase | Features | Difficulty |
|---|---|---|
| **1 (now)** | App hotkeys (launch/focus), window snapping, focus window by direction | Easy, about a weekend |
| 2 | Move/swap windows by direction, cycling layouts, per-app rules (e.g. "Warp always opens on the right half") | Easy–medium |
| 3 | **AeroSpace-style virtual workspaces**: windows on "hidden" workspaces are moved off-screen to a corner, and switching moves them back. Hotkeys to jump to a workspace and send a window to a workspace. Apps auto-assigned to workspaces. | Medium. Weeks, plus ongoing edge-case fixing |
| ✗ Out of scope | **Native macOS Spaces control** (reordering Spaces, moving windows between real desktops, as yabai does) | Needs private APIs, code injected into Dock and SIP partly disabled. It breaks with most macOS updates. **Never do this.** |

### Known hard parts of Phase 3, so early design leaves room for them
- Tracking every window as it opens, closes, moves and minimises (`AXObserver` notifications)
- Apps that refuse to resize or snap back after being moved
- Multiple monitors, dialogs, sheets and pop-ups, native full-screen windows
- Crash safety: persisting state so windows are never stranded off-screen (e.g. a "restore all windows" command and recovery on startup)

### Reference
- **AeroSpace** (github.com/nikitabobko/AeroSpace, MIT licence, Swift) solves most Phase 3 problems. Read it for approaches; don't copy large chunks.

## Guiding principles
- **No SIP changes, no private APIs.** Public APIs only: Accessibility (AX), AppKit/NSWorkspace, CoreGraphics, Carbon hotkeys.
- **Keyboard-first, config-driven.** One TOML file, hot-reloadable, with no settings UI needed.
- **Never steal common shortcuts.** Defaults must not clash with macOS or major apps (details in Part 2).
- **Fast and invisible.** No Dock icon, minimal CPU and memory use, instant response.
- **Modular.** Each phase plugs into the same core (hotkey → action → window API).

## Environment
- macOS **Tahoe 26.x**, upgrading soon to **macOS 27**. Don't rely on version-specific behaviour, and keep Accessibility calls defensive.
- Apple Silicon assumed. Daily apps: VS Code, Google Chrome, Warp terminal, Azure portal in browser.
- My existing habits to preserve: `Control + ←/→` (switch Spaces), `Control + ↑` (Mission Control), `Control + ↓` (App Exposé), `Cmd + Tab`.

---

# PART 2 — Your Task Now: Phase 1

Build a working first version of Kevro in **Rust** with exactly these three features, plus the minimum plumbing needed to run it daily.

## Feature 1: App hotkeys (launch or focus)
- A configurable global hotkey for each app.
- Pressing the hotkey: if the app isn't running, **launch** it. If it's running, **activate** it and bring its windows forward. *(Optional stretch: if it's already frontmost, hide it.)*
- Use `NSWorkspace` (via `objc2-app-kit`) to launch and activate apps by bundle ID or name. You may fall back to running `open -a`, but prefer the native route.

## Feature 2: Window snapping (focused window)
Actions: `left_half`, `right_half`, `top_half`, `bottom_half`, `maximize` (fill the visible area, not native full-screen), `center` (keep size, centre it), `first_third`, `center_third`, `last_third`, `first_two_thirds`, `last_two_thirds`, `restore` (return to the size and position before the last snap).

Implementation notes:
- Get the frontmost app's PID → `AXUIElementCreateApplication(pid)` → `kAXFocusedWindowAttribute` → set `kAXPositionAttribute` / `kAXSizeAttribute`.
- Use the **visible frame** (`NSScreen.visibleFrame`, which excludes the menu bar and Dock) of the screen containing the window's centre.
- **Coordinate systems differ:** AX uses a top-left origin across the global display space, while NSScreen uses bottom-left. Write one well-tested conversion helper.
- Some apps clamp sizes. Use the common trick of setting size → position → size again, and tolerate small mismatches.
- Pressing `left_half` repeatedly *may* cycle ½ → ⅓ → ⅔ (optional, configurable).
- Remember the pre-snap frame per window for `restore`.

## Feature 3: Focus window by direction
- Actions: `focus_left`, `focus_right`, `focus_up`, `focus_down`.
- List the visible standard windows on the current screen and Space with `CGWindowListCopyWindowInfo` (on-screen only, layer 0) to get frames and owner PIDs.
- Pick the nearest window whose centre lies in the requested direction from the focused window's centre, weighting perpendicular distance so it feels natural.
- Focus it: activate the owning app, then `AXRaise` the matching AX window. Match CG windows to AX windows by PID plus frame or title.
- Must work across apps and across multiple monitors (directions cross screen edges).

## Plumbing
- **Global hotkeys:** the `global-hotkey` crate (uses Carbon `RegisterEventHotKey`, so no extra permission is needed for hotkeys).
- **Event loop:** hotkeys only fire with a main-thread run loop. Use `tao` (or `winit`), and keep all AppKit and AX calls on the main thread.
- **Accessibility permission:** on startup check `AXIsProcessTrustedWithOptions` with the prompt option. If permission isn't granted, show a clear message and keep app hotkeys working (they don't need it). Snapping and focus need it.
- **Menu bar icon** (`tray-icon` crate) with: Reload config, Open config, Accessibility status, Quit.
- **No Dock icon:** bundle as a `.app` with `LSUIElement = true` (`cargo-bundle` or a small script).
- **Launch at login:** a LaunchAgent plist written by an `install` subcommand or script, plus a matching `uninstall`.
- **Logging:** `tracing` writing to a log file under `~/Library/Logs/kevro/`.

## Config
Location: `~/.config/kevro/config.toml`. Create a commented default file on first run. Reload it from the menu bar and, ideally, on file change. Invalid config → keep the previous config and log or notify the error; **never crash**.

Example shape (adjust if needed, but keep it this simple):

```toml
[apps]
"alt+cmd+v" = "com.microsoft.VSCode"
"alt+cmd+c" = "com.google.Chrome"
"alt+cmd+w" = "dev.warp.Warp-Stable"

[snap]
"ctrl+alt+left"  = "left_half"
"ctrl+alt+right" = "right_half"
"ctrl+alt+up"    = "maximize"
"ctrl+alt+down"  = "restore"
"ctrl+alt+c"     = "center"
"ctrl+alt+d"     = "first_third"
"ctrl+alt+f"     = "center_third"
"ctrl+alt+g"     = "last_third"

[focus]
"ctrl+alt+cmd+left"  = "focus_left"
"ctrl+alt+cmd+right" = "focus_right"
"ctrl+alt+cmd+up"    = "focus_up"
"ctrl+alt+cmd+down"  = "focus_down"

[options]
cycle_halves_to_thirds = false
gap = 0            # pixels between windows and screen edges
```

**Shortcut rules for defaults:**
- Never bind plain `Control + arrows` (taken by Spaces and Mission Control).
- Avoid `Cmd + Option + arrows` (Chrome and VS Code use them for tab and editor switching).
- `Control + backtick` is VS Code's terminal toggle, so don't use it.
- If registering a hotkey fails because something else owns it, log a clear warning naming the key and keep running.

*Note: I may have set `Control + Option + arrow` shortcuts in System Settings → Keyboard → App Shortcuts for the built-in Move & Resize menu items. The README should tell me to remove those to avoid double-handling.*

## Suggested structure
```
src/
  main.rs        // startup, event loop, tray
  config.rs      // parse, validate, defaults, hot-reload
  hotkeys.rs     // register/unregister, map key IDs to actions
  actions.rs     // Action enum and dispatch
  apps.rs        // launch/activate (NSWorkspace)
  ax.rs          // safe wrappers around AXUIElement (focused window, get/set frame, raise)
  screens.rs     // NSScreen frames and coordinate conversion
  snap.rs        // layout maths (pure functions, unit-tested)
  focus.rs       // direction selection (pure functions, unit-tested) + window listing
```
Keep **layout maths and direction selection as pure functions with unit tests** (no macOS calls). Isolate all `unsafe` FFI in `ax.rs`, `apps.rs` and `screens.rs` behind safe wrappers. Leave a clear seam (e.g. a `WindowRegistry` trait or module stub) where Phase 3 workspaces will plug in, but **don't implement workspaces**.

## Acceptance criteria
1. `cargo build --release` succeeds with no warnings, and `cargo test` passes (snap maths, coordinate conversion, direction selection).
2. Each app hotkey launches the app when it's closed and focuses it when it's open (test with VS Code, Chrome and Warp).
3. Every snap action works on a Finder, Chrome and VS Code window, on the main display and (if present) an external display, and respects the menu bar and Dock.
4. `restore` returns the window to its exact pre-snap frame.
5. Focus-by-direction moves between at least 3 side-by-side windows from different apps, both directions.
6. Without Accessibility permission: app hotkeys still work, and snapping and focus show a helpful message instead of failing silently.
7. A bad config edit doesn't crash the app, and the previous bindings stay active.
8. Runs as a menu-bar-only `.app`, starts at login after `install`, and idles at about 0% CPU.

## Deliverables
- The Rust project (Cargo workspace or single crate) with the structure above.
- `README.md`: build, bundle, grant Accessibility, install at login, config reference, troubleshooting (permissions after rebuilding, since an unsigned rebuilt binary may need Accessibility re-granted, plus hotkey conflicts).
- A default `config.toml` with comments.
- A short `ROADMAP.md` that copies the Part 1 phases, so future sessions keep the vision.

## Out of scope for now
Virtual workspaces, moving windows between Spaces, any private API or SIP-related approach, a GUI settings window, code signing and notarisation (ad-hoc signing is fine).

## Working style
- Build in this order, getting each one working end to end before moving on: **hotkeys + event loop → app launching → AX permission + snapping → focus by direction → tray, config reload, bundling, login item.**
- If a crate is unmaintained or doesn't work on current macOS, choose an alternative and note why in the README.
- Ask me before adding heavy dependencies or changing the config format significantly.

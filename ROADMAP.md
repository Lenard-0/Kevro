# Kevro roadmap

Kevro's goal is to drive the whole Mac's windows and apps from the keyboard:
reach any app or window instantly, without caring about order. The full brief
is in [`docs/BRIEF.md`](docs/BRIEF.md).

| Phase | Features | Difficulty | Status |
|---|---|---|---|
| **1** | App hotkeys (launch/focus), window snapping, focus window by direction | Easy, about a weekend | Built, awaiting hands-on testing |
| 2 | Move/swap windows by direction, cycling layouts, per-app rules (e.g. "Warp always opens on the right half") | Easy–medium | Built, awaiting hands-on testing |
| 3 | **AeroSpace-style virtual workspaces**: windows on "hidden" workspaces are moved off-screen to a corner, and switching moves them back. Hotkeys to jump to a workspace and send a window to a workspace. Apps auto-assigned to workspaces. | Medium. Weeks, plus ongoing edge-case fixing | Not started |
| ✗ | **Native macOS Spaces control** (reordering Spaces, moving windows between real desktops, as yabai does) | Needs private APIs, code injected into Dock and SIP partly disabled. It breaks with most macOS updates. | **Never** |

## Known hard parts of Phase 3

- Tracking every window as it opens, closes, moves and minimises
  (`AXObserver` notifications)
- Apps that refuse to resize or snap back after being moved
- Multiple monitors, dialogs, sheets and pop-ups, native full-screen windows
- Crash safety: persisting state so windows are never stranded off-screen
  (e.g. a "restore all windows" command and recovery on startup)

Phase 2 already added a window-created `AXObserver` (`ax::WindowObserver`,
used by `src/rules.rs`), which Phase 3 can extend with destroyed, moved and
minimised notifications. `src/registry.rs` (`WindowRegistry`) is the seam for
the rest: it should become
the single source of truth for window state, fed by AXObserver and persisted
to disk.

## Principles

- **No SIP changes, no private APIs.** Public APIs only: Accessibility (AX),
  AppKit/NSWorkspace, CoreGraphics, Carbon hotkeys.
- **Keyboard-first, config-driven.** One TOML file, hot-reloadable, with no
  settings UI needed.
- **Never steal common shortcuts.** Defaults must not clash with macOS or
  major apps.
- **Fast and invisible.** No Dock icon, minimal CPU and memory use, instant
  response.
- **Modular.** Each phase plugs into the same core (hotkey → action → window
  API).

## Reference

- **AeroSpace** (github.com/nikitabobko/AeroSpace, MIT licence, Swift) solves
  most Phase 3 problems. Read it for approaches; don't copy large chunks.

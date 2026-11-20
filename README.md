# Kevro

Keyboard-first window and app control for macOS, written in Rust.

Kevro is a small menu-bar utility that lets you:

- **Launch or focus any app** with one hotkey
- **Snap the focused window** to halves, thirds, two-thirds, maximize or centre, and restore it
- **Move focus between windows by direction**, across apps and displays
- **Move or swap windows by direction**, including between displays
- **Cycle a window through your own list of layouts**
- **Per-app rules**, e.g. "Warp always opens on the right half"

Everything is configured in one TOML file. Kevro uses only public macOS APIs
(Accessibility, AppKit, CoreGraphics and Carbon hotkeys). It needs no SIP
changes, has no Dock icon, and idles at about 0% CPU.

> Status: Phases 1 and 2. See [`ROADMAP.md`](ROADMAP.md) for what's next and
> [`docs/BRIEF.md`](docs/BRIEF.md) for the original design brief.

## Requirements

- macOS 13 or later (developed on macOS 26/27, Apple Silicon)
- Rust 1.88+ (`rustup` recommended) and the Xcode command line tools

## Build and install

```bash
git clone https://github.com/Lenard-0/Kevro.git
cd Kevro
./scripts/bundle.sh            # builds dist/Kevro.app
```

Copy `dist/Kevro.app` to `/Applications` and open it. A split-window icon
appears in the menu bar, and Kevro writes a commented default config to
`~/.config/kevro/config.toml`.

### Grant Accessibility permission

App hotkeys work straight away. Snapping and focus need Accessibility
permission:

1. On first launch macOS shows **"Kevro" would like to control this computer
   using accessibility features**. Click **Open System Settings**. You can
   also click **Accessibility: not granted** in Kevro's menu.
2. In **Privacy & Security → Accessibility**, switch **Kevro** on.
3. Within about 2 seconds the menu shows **Accessibility: granted ✓**. No
   restart is needed.

Without permission, snap and focus hotkeys beep and log a message instead of
failing silently.

### Start at login

```bash
/Applications/Kevro.app/Contents/MacOS/kevro install
```

This writes `~/Library/LaunchAgents/io.github.lenard-0.kevro.plist` and
starts Kevro. launchd restarts Kevro if it crashes, but not after you choose
**Quit Kevro**. To remove it:

```bash
/Applications/Kevro.app/Contents/MacOS/kevro uninstall
```

## Default hotkeys

`⌃` Control, `⌥` Option, `⌘` Command, `⇧` Shift.

| Hotkey | Action |
|---|---|
| `⌃⌥⌘V` / `⌃⌥⌘C` / `⌃⌥⌘W` | VS Code / Chrome / Warp |
| `⌃⌥←` / `⌃⌥→` | Left / right half |
| `⌃⌥⇧↑` / `⌃⌥⇧↓` | Top / bottom half |
| `⌃⌥↑` | Maximize (fills the visible area, not native full screen) |
| `⌃⌥↓` | Restore the pre-snap size and position |
| `⌃⌥C` | Centre (keeps size) |
| `⌃⌥D` / `⌃⌥F` / `⌃⌥G` | First / centre / last third |
| `⌃⌥E` / `⌃⌥T` | First / last two-thirds |
| `⌃⌥⌘←` `→` `↑` `↓` | Focus the nearest window in that direction |
| `⌃⌥⇧←` / `⌃⌥⇧→` | Move the window to the left/right screen edge; again to jump to the next display |
| `⌃⌥⌘⇧←` `→` `↑` `↓` | Swap the window with the nearest window in that direction |

Layout cycles and per-app rules have no defaults; see the examples below.

The defaults leave `⌃←/→/↑/↓` (Spaces, Mission Control, App Exposé),
`⌘⌥←/→` (Chrome and VS Code tab switching), `⌃`` ` (VS Code terminal) and
`⌘Tab` alone. The app hotkeys use `⌃⌥⌘` rather than `⌥⌘` because `⌥⌘W`
closes all windows and `⌥⌘V`/`⌥⌘C` are Finder shortcuts.

> **If you used macOS's built-in Move & Resize shortcuts:** remove any
> `⌃⌥`+arrow entries you added under **System Settings → Keyboard → Keyboard
> Shortcuts → App Shortcuts** (the Window menu's "Move & Resize" items).
> Otherwise both macOS and Kevro will handle the same keys.

## Configuration

The file is `~/.config/kevro/config.toml`. Kevro reloads it within 2 seconds
of any change, or straight away from the menu with **Reload Config**. If the
file has an error, Kevro **keeps the previous bindings** and writes the error
to the log.

```toml
[apps]                       # launch, or focus if running
"ctrl+alt+cmd+v" = "com.microsoft.VSCode"   # bundle ID (preferred)
"ctrl+alt+cmd+s" = "Safari"                 # or app name

[snap]                       # move/resize the focused window
"ctrl+alt+left" = "left_half"

[focus]                      # focus nearest window in a direction
"ctrl+alt+cmd+left" = "focus_left"

[move]                       # move/swap the focused window
"ctrl+alt+shift+left" = "move_left"         # slide to edge, then next display
"ctrl+alt+cmd+shift+left" = "swap_left"     # trade places with the window there

[cycle]                      # each press: next layout in the list
"ctrl+alt+s" = ["left_half", "center_third", "right_half"]

[[rules]]                    # new windows of this app open in this layout
app = "dev.warp.Warp-Stable"
layout = "right_half"

[options]
cycle_halves_to_thirds = false  # repeat left/right half: ½ → ⅓ → ⅔
gap = 0                         # points between windows and screen edges
hide_if_frontmost = false       # app hotkey hides the app if already in front
```

- **Hotkeys:** modifiers `ctrl`, `alt`/`option`, `cmd`/`command`, `shift`,
  plus one key: `a`–`z`, `0`–`9`, `left`/`right`/`up`/`down`, `space`,
  `enter`, `tab`, `f1`–`f20`, `minus`, `equal`, `comma`, `period`, `slash`,
  `` ` ``, and so on. Case doesn't matter. A hotkey can only be bound once
  across all sections.
- **Snap actions:** `left_half`, `right_half`, `top_half`, `bottom_half`,
  `maximize`, `center`, `first_third`, `center_third`, `last_third`,
  `first_two_thirds`, `last_two_thirds`, `restore`.
- **Focus actions:** `focus_left`, `focus_right`, `focus_up`, `focus_down`.
- **Move actions:** `move_left`/`right`/`up`/`down` and `swap_left`/`right`/`up`/`down`.
  A window moved to another display keeps its proportions, so a left half stays a left half.
- **Cycles:** a list of snap actions (not `restore`). If the window isn't in
  any of them yet, the first is used.
- **Rules:** `app` is a bundle ID or name; `layout` is any snap action except
  `restore`. Only standard windows are touched (not dialogs, sheets or
  palettes). Rules apply to windows created while Kevro is running, including
  the first windows of an app launched after Kevro.

> **Upgrading:** Kevro only writes the default config if you don't have one,
> so an existing config won't gain the `[move]` bindings. Copy them from
> [`config/default.toml`](config/default.toml) if you want them.
- **Finding a bundle ID:** `osascript -e 'id of app "Visual Studio Code"'`

Snapping uses the visible area (excluding the menu bar and Dock) of the
display containing the window's centre. `restore` returns a window to where it
was before the first of a run of snaps.

## Command line

The binary is `Kevro.app/Contents/MacOS/kevro`, or `target/release/kevro`
when built with Cargo.

| Command | What it does |
|---|---|
| `kevro` | Run Kevro (what the app and login item do) |
| `kevro status` | Accessibility, config validity, login item and display layout |
| `kevro windows` | On-screen windows as Kevro sees them |
| `kevro action <name>` | Run one action and exit, e.g. `snap:left_half`, `focus:focus_right`, `move:left`, `swap:right`, `cycle:left_half,right_half`, `app:Safari` |
| `kevro install` / `uninstall` | Add or remove the login item |

Logs are written to `~/Library/Logs/kevro/kevro.log` (menu: **Open Log**).
Set `KEVRO_LOG=debug` for more detail.

## Troubleshooting

**Snapping stopped working after a rebuild.** Kevro is ad-hoc signed, and
macOS ties the Accessibility grant to the exact signature. A rebuilt app looks
like a different app, so the Kevro entry in System Settings stays switched on
but no longer applies. Reset it and grant it again:

```bash
tccutil reset Accessibility io.github.lenard-0.kevro
```

Then relaunch Kevro and switch it on again in **Privacy & Security →
Accessibility**. If the old entry is still listed, remove it with the **−**
button first.

**Permission is granted but the menu still says "not granted".** Running the
bare `kevro` binary from a terminal makes macOS check the *terminal's*
permission, not Kevro's. Run `Kevro.app` instead.

**A hotkey does nothing.** Check the log for `could not be registered`. That
means another app (Raycast, Rectangle, BetterTouchTool and so on) already owns
the combination. Pick a different one or free it in the other app. The rest of
your hotkeys keep working.

**An app hotkey opens the wrong app or nothing.** Use the bundle ID rather
than the name, and check the log for `not found`.

**A window doesn't snap exactly.** Some apps enforce minimum sizes or size
steps (terminals snap to character cells). Kevro tolerates small differences.

**Focus skips a window.** Focus only sees windows on the current Space(s), and
some apps' windows (for example certain Electron popovers) aren't visible to
Accessibility. `kevro windows` shows what Kevro can see.

## Implementation notes

- **Crates:** `global-hotkey` (Carbon `RegisterEventHotKey`, so hotkeys need
  no permission), `tao` (main-thread run loop), `tray-icon` (menu bar),
  `objc2-*` (AppKit, Accessibility and CoreGraphics bindings), `toml`/`serde`
  and `tracing`. There's no file-watcher dependency: the config's modified
  time is checked every 2 seconds, which costs effectively nothing.
- **App activation:** since macOS 14, `NSRunningApplication.activate` from a
  background app is often silently ignored. Kevro also opens the app through
  LaunchServices (the same as a Dock click). For focus-by-direction it sets
  the Accessibility `AXFrontmost` attribute.
- **Code layout:** the layout maths (`snap.rs`), direction picking
  (`focus.rs`) and coordinate conversion (`screens.rs`) are pure and
  unit-tested (`cargo test`). All `unsafe` FFI is in `ax.rs`, `apps.rs` and
  `screens.rs`. `registry.rs` is where Phase 3 workspaces will plug in.
- **Per-app rules** use an `AXObserver` per matching app (window-created
  events) and NSWorkspace launch/quit notifications, so nothing is polled per
  window. New windows are laid out 150 ms after they appear, which lets the
  app finish placing them first. Phase 3 will build on the same observer.
- Set `KEVRO_NO_PROMPT=1` to stop Kevro from showing the Accessibility dialog
  (useful for headless test runs).

## License

MIT. See [`LICENSE`](LICENSE).

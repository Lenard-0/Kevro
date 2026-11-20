//! The `Action` enum (everything a hotkey can trigger) and its dispatch.

use std::fmt;
use std::time::{Duration, Instant};

use crate::apps;
use crate::ax::{self, AxErr};
use crate::config::{self, Options};
use crate::focus::{self, Direction};
use crate::registry::WindowRegistry;
use crate::screens;
use crate::snap::{self, SnapAction};

/// Don't re-show the Accessibility prompt more often than this.
const PROMPT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Launch or focus an app, by bundle ID or app name.
    App(String),
    /// Move/resize the focused window.
    Snap(SnapAction),
    /// Focus the nearest window in a direction.
    Focus(Direction),
    /// Slide the focused window to a screen edge, or to the next display.
    Move(Direction),
    /// Swap the focused window's frame with the nearest window in a direction.
    Swap(Direction),
    /// Step the focused window through a list of layouts.
    Cycle(Vec<SnapAction>),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::App(target) => write!(f, "app:{target}"),
            Action::Snap(a) => write!(f, "snap:{}", a.name()),
            Action::Focus(d) => write!(f, "focus:{}", d.name()),
            Action::Move(d) => write!(f, "move:{}", d.name().trim_start_matches("focus_")),
            Action::Swap(d) => write!(f, "swap:{}", d.name().trim_start_matches("focus_")),
            Action::Cycle(steps) => {
                let names: Vec<_> = steps.iter().map(|s| s.name()).collect();
                write!(f, "cycle:{}", names.join(","))
            }
        }
    }
}

impl Action {
    /// Parse `app:<target>`, `snap:<name>`, `focus:<name>` or a bare snap/focus name.
    pub fn parse(spec: &str) -> Result<Action, String> {
        if let Some(target) = spec.strip_prefix("app:") {
            return Ok(Action::App(target.to_string()));
        }
        if let Some(list) = spec.strip_prefix("cycle:") {
            let names: Vec<String> = list.split(',').map(|s| s.trim().to_string()).collect();
            return config::parse_layouts(&names).map(Action::Cycle);
        }
        let as_move = spec
            .replacen("move:", "move_", 1)
            .replacen("swap:", "swap_", 1);
        if let Some(a) = config::parse_move(&as_move) {
            return Ok(a);
        }
        let name = spec
            .strip_prefix("snap:")
            .or_else(|| spec.strip_prefix("focus:"))
            .unwrap_or(spec);
        SnapAction::from_name(name)
            .map(Action::Snap)
            .or_else(|| Direction::from_name(name).map(Action::Focus))
            .ok_or_else(|| format!("unknown action \"{spec}\""))
    }

    fn needs_accessibility(&self) -> bool {
        !matches!(self, Action::App(_))
    }
}

/// Runs actions. Lives on the main thread for the life of the app.
#[derive(Default)]
pub struct Dispatcher {
    registry: WindowRegistry,
    last_prompt: Option<Instant>,
}

impl Dispatcher {
    pub fn dispatch(&mut self, action: &Action, options: &Options) {
        if action.needs_accessibility() && !ax::is_trusted() {
            self.explain_missing_permission(action);
            return;
        }
        let result = match action {
            Action::App(target) => {
                apps::launch_or_focus(target, options.hide_if_frontmost);
                Ok(())
            }
            Action::Snap(a) => self.snap(*a, options),
            Action::Focus(d) => self.focus(*d),
            Action::Move(d) => self.move_window(*d, options),
            Action::Swap(d) => self.swap(*d),
            Action::Cycle(steps) => self.cycle(steps, options),
        };
        if let Err(e) = result {
            tracing::warn!("{action}: {e}");
            beep();
        }
    }

    fn explain_missing_permission(&mut self, action: &Action) {
        tracing::warn!(
            "{action} needs Accessibility permission. Grant it in System Settings → Privacy & \
             Security → Accessibility, then try again. App hotkeys keep working without it."
        );
        beep();
        let due = self
            .last_prompt
            .is_none_or(|t| t.elapsed() >= PROMPT_INTERVAL);
        if due {
            self.last_prompt = Some(Instant::now());
            // Shows macOS's own dialog with an "Open System Settings" button.
            ax::is_trusted_with_prompt();
        }
    }

    fn snap(&mut self, action: SnapAction, options: &Options) -> Result<(), AxErr> {
        let window = ax::focused_window()?;
        if action == SnapAction::Restore {
            let Some(frame) = self.registry.take_pre_snap(&window) else {
                tracing::info!("restore: this window hasn't been snapped");
                return Ok(());
            };
            window.set_frame(frame)?;
            tracing::info!("restored window to {frame:?}");
            return Ok(());
        }
        let current = frame_of(&window)?;
        let action = if options.cycle_halves_to_thirds {
            match screen_visible(&current) {
                Some((_, visible)) => snap::cycle(action, visible, current, options.gap),
                None => action,
            }
        } else {
            action
        };
        self.layout_window(&window, action, options)
    }

    /// Put `window` into `layout` on the screen it's on, remembering its old
    /// frame for `restore`. `layout` must not be `Restore`.
    pub fn layout_window(
        &mut self,
        window: &ax::Window,
        layout: SnapAction,
        options: &Options,
    ) -> Result<(), AxErr> {
        let current = frame_of(window)?;
        let Some((i, visible)) = screen_visible(&current) else {
            tracing::warn!("no screens found");
            return Ok(());
        };
        let Some(target) = snap::target_frame(layout, visible, current, options.gap) else {
            return Ok(());
        };
        let actual = window.set_frame(target)?;
        self.registry.record_snap(window, current, actual);
        tracing::info!("{} on screen {i}: {current:?} -> {actual:?}", layout.name());
        Ok(())
    }

    fn cycle(&mut self, steps: &[SnapAction], options: &Options) -> Result<(), AxErr> {
        let window = ax::focused_window()?;
        let current = frame_of(&window)?;
        let Some((_, visible)) = screen_visible(&current) else {
            return Ok(());
        };
        let next = snap::next_in_cycle(steps, visible, current, options.gap);
        self.layout_window(&window, next, options)
    }

    fn move_window(&mut self, dir: Direction, options: &Options) -> Result<(), AxErr> {
        let window = ax::focused_window()?;
        let current = frame_of(&window)?;
        let screens = screens::all();
        let Some(i) = screens::screen_for(&current, &screens) else {
            return Ok(());
        };
        let visibles: Vec<_> = screens.iter().map(|s| s.visible).collect();
        let Some(target) = snap::move_toward(dir, current, &visibles, i, options.gap) else {
            tracing::info!("move: already at the edge and no display beyond");
            return Ok(());
        };
        let actual = window.set_frame(target)?;
        self.registry.record_snap(&window, current, actual);
        tracing::info!("move {dir:?}: {current:?} -> {actual:?}");
        Ok(())
    }

    fn swap(&mut self, dir: Direction) -> Result<(), AxErr> {
        let window = ax::focused_window()?;
        let current = frame_of(&window)?;
        let Some(target) = neighbour(window.pid(), current, dir) else {
            tracing::info!("swap: no window in that direction");
            return Ok(());
        };
        let Some(other) = ax::ax_window_for(&target) else {
            tracing::info!("swap: the window in that direction isn't accessible");
            return Ok(());
        };
        let other_frame = frame_of(&other)?;
        let a = window.set_frame(other_frame)?;
        let b = other.set_frame(current)?;
        self.registry.record_snap(&window, current, a);
        self.registry.record_snap(&other, other_frame, b);
        // Keep the moved window in front and focused.
        window.raise();
        tracing::info!("swap {dir:?}: {current:?} <-> {other_frame:?}");
        Ok(())
    }
}

fn frame_of(window: &ax::Window) -> Result<crate::geom::Rect, AxErr> {
    window.frame().ok_or(AxErr::Call("AXPosition/AXSize", -1))
}

/// The screen containing `frame`, and its visible area.
fn screen_visible(frame: &crate::geom::Rect) -> Option<(usize, crate::geom::Rect)> {
    let screens = screens::all();
    screens::screen_for(frame, &screens).map(|i| (i, screens[i].visible))
}

/// The on-screen window nearest to `from` in `dir`, excluding `from` itself.
fn neighbour(pid: i32, from: crate::geom::Rect, dir: Direction) -> Option<ax::CgWindow> {
    let candidates: Vec<ax::CgWindow> = ax::onscreen_windows()
        .into_iter()
        .filter(|w| !(w.pid == pid && w.frame.approx_eq(&from, 4.0)))
        .collect();
    let rects: Vec<_> = candidates.iter().map(|w| w.frame).collect();
    focus::pick(from, dir, &rects).map(|i| candidates[i])
}

impl Dispatcher {
    fn focus(&mut self, dir: Direction) -> Result<(), AxErr> {
        // Start from the focused window; if there isn't one (e.g. the
        // desktop is focused), from the frontmost window on screen.
        let source = ax::focused_window()
            .ok()
            .and_then(|w| Some((w.pid(), w.frame()?)))
            .or_else(|| ax::onscreen_windows().first().map(|w| (w.pid, w.frame)));
        let Some((src_pid, src_frame)) = source else {
            tracing::info!("{}: no windows on screen", dir.name());
            return Ok(());
        };
        let Some(target) = neighbour(src_pid, src_frame, dir) else {
            tracing::info!("{}: no window in that direction", dir.name());
            return Ok(());
        };
        let target = &target;
        match ax::ax_window_for(target) {
            Some(w) => {
                ax::focus_window(&w);
                tracing::info!(
                    "{}: focused pid {} \"{}\" at {:?}",
                    dir.name(),
                    target.pid,
                    w.title().unwrap_or_default(),
                    target.frame
                );
            }
            None => {
                // AX can't see this window (some apps hide windows from AX);
                // activating the app is the best we can do.
                tracing::info!(
                    "{}: no AX match for pid {}, activating app",
                    dir.name(),
                    target.pid
                );
                ax::activate_pid(target.pid);
            }
        }
        Ok(())
    }
}

fn beep() {
    objc2_app_kit::NSBeep();
}

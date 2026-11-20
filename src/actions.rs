//! The `Action` enum (everything a hotkey can trigger) and its dispatch.

use std::fmt;
use std::time::{Duration, Instant};

use crate::apps;
use crate::ax::{self, AxErr};
use crate::config::Options;
use crate::focus::Direction;
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
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::App(target) => write!(f, "app:{target}"),
            Action::Snap(a) => write!(f, "snap:{}", a.name()),
            Action::Focus(d) => write!(f, "focus:{}", d.name()),
        }
    }
}

impl Action {
    /// Parse `app:<target>`, `snap:<name>`, `focus:<name>` or a bare snap/focus name.
    pub fn parse(spec: &str) -> Result<Action, String> {
        if let Some(target) = spec.strip_prefix("app:") {
            return Ok(Action::App(target.to_string()));
        }
        let name = spec.strip_prefix("snap:").or_else(|| spec.strip_prefix("focus:")).unwrap_or(spec);
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
            Action::Focus(_) => {
                tracing::warn!("{action} is not implemented yet");
                Ok(())
            }
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
        let due = self.last_prompt.is_none_or(|t| t.elapsed() >= PROMPT_INTERVAL);
        if due {
            self.last_prompt = Some(Instant::now());
            // Shows macOS's own dialog with an "Open System Settings" button.
            ax::is_trusted_with_prompt();
        }
    }

    fn snap(&mut self, action: SnapAction, options: &Options) -> Result<(), AxErr> {
        let window = ax::focused_window()?;
        let current = window.frame().ok_or(AxErr::Call("AXPosition/AXSize", -1))?;

        if action == SnapAction::Restore {
            match self.registry.take_pre_snap(&window) {
                Some(frame) => {
                    window.set_frame(frame)?;
                    tracing::info!("restored window to {frame:?}");
                }
                None => tracing::info!("restore: this window hasn't been snapped"),
            }
            return Ok(());
        }

        let screens = screens::all();
        let Some(i) = screens::screen_for(&current, &screens) else {
            tracing::warn!("no screens found");
            return Ok(());
        };
        let visible = screens[i].visible;
        let action = if options.cycle_halves_to_thirds {
            snap::cycle(action, visible, current, options.gap)
        } else {
            action
        };
        let Some(target) = snap::target_frame(action, visible, current, options.gap) else {
            return Ok(());
        };
        let actual = window.set_frame(target)?;
        self.registry.record_snap(&window, current, actual);
        tracing::info!("{} on screen {i}: {current:?} -> {actual:?}", action.name());
        Ok(())
    }
}

fn beep() {
    objc2_app_kit::NSBeep();
}

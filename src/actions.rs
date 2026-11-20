//! The `Action` enum: everything a hotkey can trigger.

use std::fmt;

use crate::apps;
use crate::config::Options;
use crate::focus::Direction;
use crate::snap::SnapAction;

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
}

/// Run an action. Called on the main thread.
pub fn dispatch(action: &Action, options: &Options) {
    match action {
        Action::App(target) => apps::launch_or_focus(target, options.hide_if_frontmost),
        Action::Snap(_) | Action::Focus(_) => {
            tracing::warn!("{action} is not implemented yet");
        }
    }
}

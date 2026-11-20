//! The `Action` enum: everything a hotkey can trigger.

use std::fmt;

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

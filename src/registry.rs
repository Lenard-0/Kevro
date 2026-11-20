//! `WindowRegistry`: per-window state that Kevro remembers.
//!
//! Phase 1 only stores the pre-snap frame used by `restore`.
//!
//! **Phase 3 seam.** Virtual workspaces will grow this into the single source
//! of truth for every window: which workspace it belongs to, its on-screen
//! frame while hidden off-screen, and a persisted copy on disk so windows can
//! be recovered after a crash. AXObserver notifications (window created,
//! destroyed, moved, minimised) will feed it. Keep window bookkeeping here
//! rather than scattering it across actions.

use crate::ax::Window;
use crate::geom::Rect;
use crate::snap::MATCH_TOLERANCE;

/// Old entries are dropped beyond this many windows.
const MAX_ENTRIES: usize = 256;

#[derive(Debug)]
struct Entry {
    window: Window,
    /// The frame before the first snap in the current run of snaps.
    pre_snap: Rect,
    /// The frame the window had right after our last snap.
    last_set: Rect,
}

#[derive(Debug, Default)]
pub struct WindowRegistry {
    entries: Vec<Entry>,
}

impl WindowRegistry {
    fn index(&self, window: &Window) -> Option<usize> {
        self.entries.iter().position(|e| &e.window == window)
    }

    /// Record a snap from `before` to `after`.
    ///
    /// If the window still has the frame we last gave it, it is part of the
    /// same run of snaps, so the original pre-snap frame is kept. If the user
    /// has moved it since, `before` becomes the new pre-snap frame.
    pub fn record_snap(&mut self, window: &Window, before: Rect, after: Rect) {
        match self.index(window) {
            Some(i) => {
                let e = &mut self.entries[i];
                if !e.last_set.approx_eq(&before, MATCH_TOLERANCE) {
                    e.pre_snap = before;
                }
                e.last_set = after;
            }
            None => {
                if self.entries.len() >= MAX_ENTRIES {
                    self.entries.remove(0);
                }
                self.entries.push(Entry {
                    window: window.clone(),
                    pre_snap: before,
                    last_set: after,
                });
            }
        }
    }

    /// Forget and return the pre-snap frame for `window`, if any.
    pub fn take_pre_snap(&mut self, window: &Window) -> Option<Rect> {
        self.index(window).map(|i| self.entries.remove(i).pre_snap)
    }
}

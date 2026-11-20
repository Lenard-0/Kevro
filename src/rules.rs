//! Per-app rules: lay out new windows of matching apps automatically.
//!
//! Each running app that matches a rule gets an AXObserver for
//! "window created". New windows are laid out after a short delay, which
//! lets the app finish placing the window itself first. When a matching app
//! launches, its first windows may appear before the observer is attached,
//! so those are laid out too.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::actions::Dispatcher;
use crate::apps::{self, RunningApp};
use crate::ax::{self, Window, WindowObserver};
use crate::config::{Options, Rule};
use crate::snap::SnapAction;

/// Wait this long after a window appears before laying it out.
const SETTLE_DELAY: Duration = Duration::from_millis(150);

#[derive(Default)]
pub struct RuleEngine {
    observers: HashMap<i32, WindowObserver>,
    known_pids: Option<HashSet<i32>>,
    pending: Vec<(Instant, Window, SnapAction)>,
}

fn rule_for<'a>(rules: &'a [Rule], app: &RunningApp) -> Option<&'a Rule> {
    rules.iter().find(|r| app.matches(&r.app))
}

impl RuleEngine {
    /// Attach observers to running apps that match a rule and drop the rest.
    /// Call on startup, config reload, app launch/quit and periodically
    /// (to pick up a newly granted Accessibility permission).
    pub fn sync(&mut self, rules: &[Rule]) {
        let running = apps::running_apps();
        let pids: HashSet<i32> = running.iter().map(|a| a.pid).collect();
        // Apps that weren't running at the previous sync have just launched.
        // On the very first sync nothing counts as launched.
        let launched: HashSet<i32> = match &self.known_pids {
            Some(known) => pids.difference(known).copied().collect(),
            None => HashSet::new(),
        };
        self.known_pids = Some(pids);

        let mut wanted = HashSet::new();
        for app in &running {
            let Some(rule) = rule_for(rules, app) else {
                continue;
            };
            wanted.insert(app.pid);
            if let std::collections::hash_map::Entry::Vacant(slot) = self.observers.entry(app.pid) {
                match WindowObserver::new(app.pid) {
                    Ok(o) => {
                        tracing::info!("rules: watching {} for new windows", rule.app);
                        slot.insert(o);
                    }
                    Err(ax::AxErr::NotTrusted) => {}
                    Err(e) => tracing::debug!("rules: cannot observe {}: {e}", rule.app),
                }
            }
            if launched.contains(&app.pid) && self.observers.contains_key(&app.pid) {
                for w in ax::Element::application(app.pid).windows() {
                    self.queue(w, rule.layout);
                }
            }
        }
        self.observers.retain(|pid, _| wanted.contains(pid));
        self.pending.retain(|(_, w, _)| wanted.contains(&w.pid()));
    }

    /// Queue windows reported by the observers.
    pub fn on_new_windows(&mut self, rules: &[Rule], windows: Vec<Window>) {
        if windows.is_empty() {
            return;
        }
        let running = apps::running_apps();
        for w in windows {
            let rule = running
                .iter()
                .find(|a| a.pid == w.pid())
                .and_then(|a| rule_for(rules, a));
            if let Some(rule) = rule {
                self.queue(w, rule.layout);
            }
        }
    }

    fn queue(&mut self, window: Window, layout: SnapAction) {
        if self.pending.iter().any(|(_, w, _)| *w == window) {
            return;
        }
        self.pending
            .push((Instant::now() + SETTLE_DELAY, window, layout));
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.pending.iter().map(|(t, _, _)| *t).min()
    }

    /// Lay out windows whose delay has passed. Only standard, visible
    /// windows are touched: dialogs, sheets and palettes are left alone.
    pub fn run_due(&mut self, dispatcher: &mut Dispatcher, options: &Options) {
        let now = Instant::now();
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|(t, _, _)| *t <= now);
        self.pending = later;
        for (_, window, layout) in due {
            if !window.is_standard() || window.is_minimized() {
                continue;
            }
            tracing::info!(
                "rules: new window \"{}\" (pid {}) -> {}",
                window.title().unwrap_or_default(),
                window.pid(),
                layout.name()
            );
            if let Err(e) = dispatcher.layout_window(&window, layout, options) {
                tracing::warn!("rules: could not lay out window: {e}");
            }
        }
    }
}

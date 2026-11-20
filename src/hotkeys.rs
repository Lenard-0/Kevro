//! Register/unregister global hotkeys and map hotkey IDs to actions.

use std::collections::HashMap;

use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;

use crate::actions::Action;
use crate::config::Binding;

pub struct Hotkeys {
    manager: GlobalHotKeyManager,
    active: HashMap<u32, (HotKey, Binding)>,
}

impl Hotkeys {
    pub fn new() -> Result<Self, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        Ok(Hotkeys { manager, active: HashMap::new() })
    }

    /// Replace all registered hotkeys with `bindings`. Keys that fail to
    /// register (usually because another app owns them) are logged and skipped.
    /// Returns the number of hotkeys that registered successfully.
    pub fn apply(&mut self, bindings: &[Binding]) -> usize {
        for (_, (hotkey, binding)) in self.active.drain() {
            if let Err(e) = self.manager.unregister(hotkey) {
                tracing::warn!("could not unregister \"{}\": {e}", binding.key);
            }
        }
        for binding in bindings {
            match self.manager.register(binding.hotkey) {
                Ok(()) => {
                    self.active.insert(binding.hotkey.id(), (binding.hotkey, binding.clone()));
                }
                Err(e) => tracing::warn!(
                    "hotkey \"{}\" ({}) could not be registered, probably because another app \
                     already uses it: {e}",
                    binding.key,
                    binding.action
                ),
            }
        }
        self.active.len()
    }

    pub fn lookup(&self, id: u32) -> Option<(&str, &Action)> {
        self.active.get(&id).map(|(_, b)| (b.key.as_str(), &b.action))
    }
}

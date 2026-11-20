//! Menu bar icon and menu.

use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Accessibility,
    Reload,
    OpenConfig,
    OpenLog,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    accessibility: MenuItem,
    trusted: Option<bool>,
    ids: Vec<(MenuId, MenuAction)>,
}

impl Tray {
    /// Must be called on the main thread after the event loop has started.
    pub fn new() -> Result<Tray, String> {
        let menu = Menu::new();
        let title = MenuItem::new(format!("Kevro {}", env!("CARGO_PKG_VERSION")), false, None);
        let accessibility = MenuItem::new("Accessibility: checking…", false, None);
        let reload = MenuItem::new("Reload Config", true, None);
        let open_config = MenuItem::new("Open Config", true, None);
        let open_log = MenuItem::new("Open Log", true, None);
        let quit = MenuItem::new("Quit Kevro", true, None);
        menu.append_items(&[
            &title,
            &accessibility,
            &PredefinedMenuItem::separator(),
            &reload,
            &open_config,
            &open_log,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|e| e.to_string())?;

        let ids = vec![
            (accessibility.id().clone(), MenuAction::Accessibility),
            (reload.id().clone(), MenuAction::Reload),
            (open_config.id().clone(), MenuAction::OpenConfig),
            (open_log.id().clone(), MenuAction::OpenLog),
            (quit.id().clone(), MenuAction::Quit),
        ];

        let (rgba, size) = icon_rgba();
        let icon = Icon::from_rgba(rgba, size, size).map_err(|e| e.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon_templated(icon)
            .with_tooltip("Kevro")
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Tray { _icon: tray, accessibility, trusted: None, ids })
    }

    pub fn action_for(&self, event: &MenuEvent) -> Option<MenuAction> {
        self.ids.iter().find(|(id, _)| *id == event.id).map(|(_, a)| *a)
    }

    /// Update the Accessibility line. Cheap to call often.
    pub fn set_trusted(&mut self, trusted: bool) {
        if self.trusted == Some(trusted) {
            return;
        }
        self.trusted = Some(trusted);
        if trusted {
            self.accessibility.set_text("Accessibility: granted ✓");
            self.accessibility.set_enabled(false);
        } else {
            self.accessibility.set_text("Accessibility: not granted (click to fix)…");
            self.accessibility.set_enabled(true);
        }
    }
}

/// A 36×36 (18pt @2x) template glyph: a window outline split down the
/// middle with the left half filled. Black + alpha, so macOS tints it.
fn icon_rgba() -> (Vec<u8>, u32) {
    const S: u32 = 36;
    let (x0, y0, x1, y1) = (4, 7, 31, 28); // outline bounds, inclusive
    let border = 3;
    let mid = (x0 + x1) / 2;
    let mut rgba = vec![0u8; (S * S * 4) as usize];
    for y in 0..S {
        for x in 0..S {
            let inside = x >= x0 && x <= x1 && y >= y0 && y <= y1;
            let on_border = inside
                && (x < x0 + border || x > x1 - border || y < y0 + border || y > y1 - border);
            let left_fill = inside && x <= mid;
            let divider = inside && (mid..mid + 2).contains(&x);
            // Round the four outer corners off.
            let corner = (x == x0 || x == x1) && (y == y0 || y == y1);
            if (on_border || left_fill || divider) && !corner {
                rgba[((y * S + x) * 4 + 3) as usize] = 255;
            }
        }
    }
    (rgba, S)
}

#[cfg(test)]
mod tests {
    #[test]
    fn icon_has_expected_size_and_some_pixels() {
        let (rgba, s) = super::icon_rgba();
        assert_eq!(rgba.len(), (s * s * 4) as usize);
        let opaque = rgba.chunks(4).filter(|p| p[3] == 255).count();
        assert!(opaque > 100 && opaque < (s * s) as usize / 2);
    }
}

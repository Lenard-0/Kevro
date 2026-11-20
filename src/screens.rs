//! NSScreen frames and the AppKit → AX coordinate conversion.

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::NSRect;

use crate::geom::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// Whole display, AX coordinates.
    pub frame: Rect,
    /// Display minus menu bar and Dock, AX coordinates.
    pub visible: Rect,
}

/// Convert an AppKit rect (origin bottom-left of the primary display, y up)
/// to AX/CG coordinates (origin top-left of the primary display, y down).
///
/// `primary_height` is the height of the primary display (the one whose
/// AppKit frame origin is (0, 0)).
pub fn appkit_to_ax(r: Rect, primary_height: f64) -> Rect {
    Rect::new(r.x, primary_height - (r.y + r.h), r.w, r.h)
}

/// All screens in AX coordinates. The first is the primary display.
/// Returns an empty list if not called on the main thread.
pub fn all() -> Vec<Screen> {
    let Some(mtm) = MainThreadMarker::new() else {
        tracing::error!("screens::all called off the main thread");
        return Vec::new();
    };
    let screens = NSScreen::screens(mtm);
    let rects: Vec<(Rect, Rect)> =
        screens.iter().map(|s| (from_ns(s.frame()), from_ns(s.visibleFrame()))).collect();
    // The primary display is the one at the AppKit origin; NSScreen.screens
    // lists it first, but be defensive.
    let primary_height = rects
        .iter()
        .find(|(f, _)| f.x == 0.0 && f.y == 0.0)
        .or(rects.first())
        .map(|(f, _)| f.h)
        .unwrap_or(0.0);
    rects
        .into_iter()
        .map(|(f, v)| Screen {
            frame: appkit_to_ax(f, primary_height),
            visible: appkit_to_ax(v, primary_height),
        })
        .collect()
}

fn from_ns(r: NSRect) -> Rect {
    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height)
}

/// Index of the screen a window belongs to: the one containing its centre,
/// else the one it overlaps most, else the first (primary).
pub fn screen_for(window: &Rect, screens: &[Screen]) -> Option<usize> {
    if screens.is_empty() {
        return None;
    }
    let c = window.center();
    if let Some(i) = screens.iter().position(|s| s.frame.contains(c)) {
        return Some(i);
    }
    let (best, area) = screens
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.frame.intersection_area(window)))
        .fold((0, 0.0), |acc, x| if x.1 > acc.1 { x } else { acc });
    Some(if area > 0.0 { best } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY_H: f64 = 1117.0;

    #[test]
    fn primary_screen_maps_to_origin() {
        // 1728x1117 primary with a 37pt menu bar and 60pt Dock at the bottom.
        let frame = Rect::new(0.0, 0.0, 1728.0, 1117.0);
        let visible = Rect::new(0.0, 60.0, 1728.0, 1020.0);
        assert_eq!(appkit_to_ax(frame, PRIMARY_H), frame);
        // Visible area starts below the menu bar in AX coordinates.
        assert_eq!(appkit_to_ax(visible, PRIMARY_H), Rect::new(0.0, 37.0, 1728.0, 1020.0));
    }

    #[test]
    fn external_screen_above_primary() {
        // A 2560x1440 display placed above the primary in Displays settings.
        let ext = Rect::new(-400.0, 1117.0, 2560.0, 1440.0);
        assert_eq!(appkit_to_ax(ext, PRIMARY_H), Rect::new(-400.0, -1440.0, 2560.0, 1440.0));
    }

    #[test]
    fn external_screen_left_and_lower() {
        // A 1920x1080 display to the left, its bottom 200pt below the primary's.
        let ext = Rect::new(-1920.0, -200.0, 1920.0, 1080.0);
        assert_eq!(appkit_to_ax(ext, PRIMARY_H), Rect::new(-1920.0, 237.0, 1920.0, 1080.0));
    }

    #[test]
    fn conversion_round_trips() {
        let r = Rect::new(12.0, 345.0, 600.0, 400.0);
        // The conversion is its own inverse.
        assert_eq!(appkit_to_ax(appkit_to_ax(r, PRIMARY_H), PRIMARY_H), r);
    }

    fn two_screens() -> Vec<Screen> {
        let a = Rect::new(0.0, 0.0, 1000.0, 800.0);
        let b = Rect::new(1000.0, 0.0, 1000.0, 800.0);
        vec![Screen { frame: a, visible: a }, Screen { frame: b, visible: b }]
    }

    #[test]
    fn screen_for_uses_centre_then_overlap() {
        let s = two_screens();
        assert_eq!(screen_for(&Rect::new(100.0, 100.0, 200.0, 200.0), &s), Some(0));
        // Centre at x=1050: second screen.
        assert_eq!(screen_for(&Rect::new(800.0, 100.0, 500.0, 200.0), &s), Some(1));
        // Centre off all screens (below), but overlapping the second more.
        assert_eq!(screen_for(&Rect::new(1100.0, 700.0, 200.0, 400.0), &s), Some(1));
        // Entirely off-screen: primary.
        assert_eq!(screen_for(&Rect::new(5000.0, 5000.0, 10.0, 10.0), &s), Some(0));
        assert_eq!(screen_for(&Rect::default(), &[]), None);
    }
}

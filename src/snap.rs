//! Snap actions and layout maths. Pure functions only: no macOS calls.

use crate::geom::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SnapAction {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    Maximize,
    Center,
    FirstThird,
    CenterThird,
    LastThird,
    FirstTwoThirds,
    LastTwoThirds,
    Restore,
}

impl SnapAction {
    pub const ALL: [SnapAction; 12] = [
        SnapAction::LeftHalf,
        SnapAction::RightHalf,
        SnapAction::TopHalf,
        SnapAction::BottomHalf,
        SnapAction::Maximize,
        SnapAction::Center,
        SnapAction::FirstThird,
        SnapAction::CenterThird,
        SnapAction::LastThird,
        SnapAction::FirstTwoThirds,
        SnapAction::LastTwoThirds,
        SnapAction::Restore,
    ];

    pub fn name(self) -> &'static str {
        match self {
            SnapAction::LeftHalf => "left_half",
            SnapAction::RightHalf => "right_half",
            SnapAction::TopHalf => "top_half",
            SnapAction::BottomHalf => "bottom_half",
            SnapAction::Maximize => "maximize",
            SnapAction::Center => "center",
            SnapAction::FirstThird => "first_third",
            SnapAction::CenterThird => "center_third",
            SnapAction::LastThird => "last_third",
            SnapAction::FirstTwoThirds => "first_two_thirds",
            SnapAction::LastTwoThirds => "last_two_thirds",
            SnapAction::Restore => "restore",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }
}

/// Frames within this many points count as "the same" when deciding whether
/// a window is already in a layout (apps such as terminals round their size).
pub const MATCH_TOLERANCE: f64 = 12.0;

/// A layout as fractions of the visible area: (x, y, w, h), each in 0..=1.
fn fractions(action: SnapAction) -> Option<(f64, f64, f64, f64)> {
    const T: f64 = 1.0 / 3.0;
    Some(match action {
        SnapAction::LeftHalf => (0.0, 0.0, 0.5, 1.0),
        SnapAction::RightHalf => (0.5, 0.0, 0.5, 1.0),
        SnapAction::TopHalf => (0.0, 0.0, 1.0, 0.5),
        SnapAction::BottomHalf => (0.0, 0.5, 1.0, 0.5),
        SnapAction::Maximize => (0.0, 0.0, 1.0, 1.0),
        SnapAction::FirstThird => (0.0, 0.0, T, 1.0),
        SnapAction::CenterThird => (T, 0.0, T, 1.0),
        SnapAction::LastThird => (2.0 * T, 0.0, T, 1.0),
        SnapAction::FirstTwoThirds => (0.0, 0.0, 2.0 * T, 1.0),
        SnapAction::LastTwoThirds => (T, 0.0, 2.0 * T, 1.0),
        SnapAction::Center | SnapAction::Restore => return None,
    })
}

/// The target frame for `action` on a screen with visible area `visible`.
///
/// `gap` is applied in full at screen edges and halved between windows, so two
/// adjacent halves end up exactly `gap` apart. `Restore` returns `None`: it
/// depends on remembered state, not maths.
pub fn target_frame(action: SnapAction, visible: Rect, current: Rect, gap: f64) -> Option<Rect> {
    if action == SnapAction::Restore {
        return None;
    }
    if action == SnapAction::Center {
        let w = current.w.min(visible.w - 2.0 * gap).max(1.0);
        let h = current.h.min(visible.h - 2.0 * gap).max(1.0);
        let r = Rect::new(visible.x + (visible.w - w) / 2.0, visible.y + (visible.h - h) / 2.0, w, h);
        return Some(r.rounded());
    }
    let (fx, fy, fw, fh) = fractions(action)?;
    let at_edge = |v: f64| v.abs() < 1e-9 || (v - 1.0).abs() < 1e-9;
    let inset = |v: f64| if at_edge(v) { gap } else { gap / 2.0 };

    let left = visible.x + fx * visible.w + inset(fx);
    let right = visible.x + (fx + fw) * visible.w - inset(fx + fw);
    let top = visible.y + fy * visible.h + inset(fy);
    let bottom = visible.y + (fy + fh) * visible.h - inset(fy + fh);
    Some(Rect::new(left, top, (right - left).max(1.0), (bottom - top).max(1.0)).rounded())
}

/// With `cycle_halves_to_thirds`, pressing left/right half on a window that is
/// already in a step of the cycle moves to the next step:
/// ½ → ⅓ → ⅔ → ½. Any other action is returned unchanged.
pub fn cycle(action: SnapAction, visible: Rect, current: Rect, gap: f64) -> SnapAction {
    use SnapAction::*;
    let steps: &[SnapAction] = match action {
        LeftHalf => &[LeftHalf, FirstThird, FirstTwoThirds],
        RightHalf => &[RightHalf, LastThird, LastTwoThirds],
        _ => return action,
    };
    for (i, step) in steps.iter().enumerate() {
        if let Some(frame) = target_frame(*step, visible, current, gap)
            && frame.approx_eq(&current, MATCH_TOLERANCE)
        {
            return steps[(i + 1) % steps.len()];
        }
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1728x1117 laptop screen with a 37pt menu bar and 60pt Dock, in AX coords.
    const VIS: Rect = Rect::new(0.0, 37.0, 1728.0, 1020.0);
    const WIN: Rect = Rect::new(100.0, 100.0, 800.0, 600.0);

    fn t(a: SnapAction, gap: f64) -> Rect {
        target_frame(a, VIS, WIN, gap).unwrap()
    }

    #[test]
    fn names_round_trip() {
        for a in SnapAction::ALL {
            assert_eq!(SnapAction::from_name(a.name()), Some(a));
        }
        assert_eq!(SnapAction::from_name("nope"), None);
    }

    #[test]
    fn halves() {
        assert_eq!(t(SnapAction::LeftHalf, 0.0), Rect::new(0.0, 37.0, 864.0, 1020.0));
        assert_eq!(t(SnapAction::RightHalf, 0.0), Rect::new(864.0, 37.0, 864.0, 1020.0));
        assert_eq!(t(SnapAction::TopHalf, 0.0), Rect::new(0.0, 37.0, 1728.0, 510.0));
        assert_eq!(t(SnapAction::BottomHalf, 0.0), Rect::new(0.0, 547.0, 1728.0, 510.0));
        assert_eq!(t(SnapAction::Maximize, 0.0), VIS);
    }

    #[test]
    fn thirds_tile_without_overlap() {
        let a = t(SnapAction::FirstThird, 0.0);
        let b = t(SnapAction::CenterThird, 0.0);
        let c = t(SnapAction::LastThird, 0.0);
        assert_eq!(a, Rect::new(0.0, 37.0, 576.0, 1020.0));
        assert_eq!(a.right(), b.x);
        assert_eq!(b.right(), c.x);
        assert_eq!(c.right(), VIS.right());
        assert_eq!(t(SnapAction::FirstTwoThirds, 0.0), Rect::new(0.0, 37.0, 1152.0, 1020.0));
        assert_eq!(t(SnapAction::LastTwoThirds, 0.0), Rect::new(576.0, 37.0, 1152.0, 1020.0));
    }

    #[test]
    fn odd_sizes_and_offset_screens_still_tile() {
        // External screen to the left with an odd width.
        let vis = Rect::new(-1921.0, 237.0, 1921.0, 1055.0);
        let l = target_frame(SnapAction::LeftHalf, vis, WIN, 0.0).unwrap();
        let r = target_frame(SnapAction::RightHalf, vis, WIN, 0.0).unwrap();
        assert_eq!(l.x, vis.x);
        assert_eq!(l.right(), r.x);
        assert_eq!(r.right(), vis.right());
    }

    #[test]
    fn gaps_are_full_at_edges_and_half_between() {
        let l = t(SnapAction::LeftHalf, 10.0);
        let r = t(SnapAction::RightHalf, 10.0);
        assert_eq!(l, Rect::new(10.0, 47.0, 849.0, 1000.0));
        assert_eq!(r.x - l.right(), 10.0);
        assert_eq!(VIS.right() - r.right(), 10.0);
        assert_eq!(t(SnapAction::Maximize, 8.0), Rect::new(8.0, 45.0, 1712.0, 1004.0));
    }

    #[test]
    fn center_keeps_size_and_clamps() {
        let c = t(SnapAction::Center, 0.0);
        assert_eq!((c.w, c.h), (800.0, 600.0));
        assert_eq!(c, Rect::new(464.0, 247.0, 800.0, 600.0));
        let huge = Rect::new(0.0, 0.0, 5000.0, 5000.0);
        assert_eq!(target_frame(SnapAction::Center, VIS, huge, 0.0).unwrap(), VIS);
    }

    #[test]
    fn restore_has_no_target() {
        assert_eq!(target_frame(SnapAction::Restore, VIS, WIN, 0.0), None);
    }

    #[test]
    fn cycling() {
        use SnapAction::*;
        assert_eq!(cycle(LeftHalf, VIS, WIN, 0.0), LeftHalf);
        assert_eq!(cycle(LeftHalf, VIS, t(LeftHalf, 0.0), 0.0), FirstThird);
        assert_eq!(cycle(LeftHalf, VIS, t(FirstThird, 0.0), 0.0), FirstTwoThirds);
        assert_eq!(cycle(LeftHalf, VIS, t(FirstTwoThirds, 0.0), 0.0), LeftHalf);
        assert_eq!(cycle(RightHalf, VIS, t(RightHalf, 0.0), 0.0), LastThird);
        assert_eq!(cycle(RightHalf, VIS, t(LastThird, 0.0), 0.0), LastTwoThirds);
        // Terminal-style rounding: a few points short still counts as a match.
        let mut almost = t(LeftHalf, 0.0);
        almost.w -= 7.0;
        assert_eq!(cycle(LeftHalf, VIS, almost, 0.0), FirstThird);
        assert_eq!(cycle(Maximize, VIS, VIS, 0.0), Maximize);
    }
}

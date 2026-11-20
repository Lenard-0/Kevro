//! Snap actions and layout maths. Pure functions only: no macOS calls.

use crate::focus::{self, Direction};
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
        let r = Rect::new(
            visible.x + (visible.w - w) / 2.0,
            visible.y + (visible.h - h) / 2.0,
            w,
            h,
        );
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
    match action {
        LeftHalf => next_in_cycle(
            &[LeftHalf, FirstThird, FirstTwoThirds],
            visible,
            current,
            gap,
        ),
        RightHalf => next_in_cycle(
            &[RightHalf, LastThird, LastTwoThirds],
            visible,
            current,
            gap,
        ),
        _ => action,
    }
}

/// The layout after the one the window is currently in. If the window isn't
/// in any of `steps`, the first step. `steps` must not be empty.
pub fn next_in_cycle(steps: &[SnapAction], visible: Rect, current: Rect, gap: f64) -> SnapAction {
    for (i, step) in steps.iter().enumerate() {
        if let Some(frame) = target_frame(*step, visible, current, gap)
            && frame.approx_eq(&current, MATCH_TOLERANCE)
        {
            return steps[(i + 1) % steps.len()];
        }
    }
    steps[0]
}

/// Within this many points of an edge counts as "already at the edge".
const EDGE_TOLERANCE: f64 = 2.0;

/// Where `move_<dir>` puts a window.
///
/// If the window isn't touching that edge of its screen's visible area (plus
/// `gap`), it slides there, keeping its size. If it already is, it moves to
/// the next display in that direction, keeping its position and size as
/// proportions of the visible area (so a left-half window stays a left half).
/// Returns `None` if it's at the edge and there's no display beyond.
///
/// `visibles` are the visible areas of all screens; `cur` is the window's.
pub fn move_toward(
    dir: Direction,
    current: Rect,
    visibles: &[Rect],
    cur: usize,
    gap: f64,
) -> Option<Rect> {
    let vis = *visibles.get(cur)?;
    let w = current.w.min(vis.w - 2.0 * gap).max(1.0);
    let h = current.h.min(vis.h - 2.0 * gap).max(1.0);
    let (left, right) = (vis.x + gap, vis.right() - gap);
    let (top, bottom) = (vis.y + gap, vis.bottom() - gap);
    let near = |a: f64, b: f64| (a - b).abs() <= EDGE_TOLERANCE;

    let slid = match dir {
        Direction::Left => (!near(current.x, left)).then(|| Rect::new(left, current.y, w, h)),
        Direction::Right => {
            (!near(current.right(), right)).then(|| Rect::new(right - w, current.y, w, h))
        }
        Direction::Up => (!near(current.y, top)).then(|| Rect::new(current.x, top, w, h)),
        Direction::Down => {
            (!near(current.bottom(), bottom)).then(|| Rect::new(current.x, bottom - h, w, h))
        }
    };
    if let Some(r) = slid {
        return Some(r.rounded());
    }

    // At the edge: hop to the adjacent display, if any.
    let others: Vec<Rect> = visibles
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != cur)
        .map(|(_, r)| *r)
        .collect();
    let next = others[focus::pick(vis, dir, &others)?];
    let sx = next.w / vis.w;
    let sy = next.h / vis.h;
    let r = Rect::new(
        next.x + (current.x - vis.x) * sx,
        next.y + (current.y - vis.y) * sy,
        current.w * sx,
        current.h * sy,
    );
    // Keep it fully on the new display.
    let w = r.w.min(next.w);
    let h = r.h.min(next.h);
    let x = r.x.clamp(next.x, next.right() - w);
    let y = r.y.clamp(next.y, next.bottom() - h);
    Some(Rect::new(x, y, w, h).rounded())
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
        assert_eq!(
            t(SnapAction::LeftHalf, 0.0),
            Rect::new(0.0, 37.0, 864.0, 1020.0)
        );
        assert_eq!(
            t(SnapAction::RightHalf, 0.0),
            Rect::new(864.0, 37.0, 864.0, 1020.0)
        );
        assert_eq!(
            t(SnapAction::TopHalf, 0.0),
            Rect::new(0.0, 37.0, 1728.0, 510.0)
        );
        assert_eq!(
            t(SnapAction::BottomHalf, 0.0),
            Rect::new(0.0, 547.0, 1728.0, 510.0)
        );
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
        assert_eq!(
            t(SnapAction::FirstTwoThirds, 0.0),
            Rect::new(0.0, 37.0, 1152.0, 1020.0)
        );
        assert_eq!(
            t(SnapAction::LastTwoThirds, 0.0),
            Rect::new(576.0, 37.0, 1152.0, 1020.0)
        );
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
        assert_eq!(
            t(SnapAction::Maximize, 8.0),
            Rect::new(8.0, 45.0, 1712.0, 1004.0)
        );
    }

    #[test]
    fn center_keeps_size_and_clamps() {
        let c = t(SnapAction::Center, 0.0);
        assert_eq!((c.w, c.h), (800.0, 600.0));
        assert_eq!(c, Rect::new(464.0, 247.0, 800.0, 600.0));
        let huge = Rect::new(0.0, 0.0, 5000.0, 5000.0);
        assert_eq!(
            target_frame(SnapAction::Center, VIS, huge, 0.0).unwrap(),
            VIS
        );
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
        assert_eq!(
            cycle(LeftHalf, VIS, t(FirstThird, 0.0), 0.0),
            FirstTwoThirds
        );
        assert_eq!(cycle(LeftHalf, VIS, t(FirstTwoThirds, 0.0), 0.0), LeftHalf);
        assert_eq!(cycle(RightHalf, VIS, t(RightHalf, 0.0), 0.0), LastThird);
        assert_eq!(cycle(RightHalf, VIS, t(LastThird, 0.0), 0.0), LastTwoThirds);
        // Terminal-style rounding: a few points short still counts as a match.
        let mut almost = t(LeftHalf, 0.0);
        almost.w -= 7.0;
        assert_eq!(cycle(LeftHalf, VIS, almost, 0.0), FirstThird);
        assert_eq!(cycle(Maximize, VIS, VIS, 0.0), Maximize);
    }

    #[test]
    fn custom_cycle() {
        use SnapAction::*;
        let steps = [LeftHalf, CenterThird, RightHalf];
        // Not in any step: first.
        assert_eq!(next_in_cycle(&steps, VIS, WIN, 0.0), LeftHalf);
        assert_eq!(
            next_in_cycle(&steps, VIS, t(LeftHalf, 0.0), 0.0),
            CenterThird
        );
        assert_eq!(
            next_in_cycle(&steps, VIS, t(CenterThird, 0.0), 0.0),
            RightHalf
        );
        assert_eq!(next_in_cycle(&steps, VIS, t(RightHalf, 0.0), 0.0), LeftHalf);
        assert_eq!(next_in_cycle(&[Maximize], VIS, VIS, 0.0), Maximize);
    }

    // Primary (as in VIS) plus an external display to its right, lower and bigger.
    const EXT: Rect = Rect::new(1728.0, 0.0, 2560.0, 1415.0);

    #[test]
    fn move_slides_to_edge_first() {
        let v = [VIS, EXT];
        assert_eq!(
            move_toward(Direction::Left, WIN, &v, 0, 0.0),
            Some(Rect::new(0.0, 100.0, 800.0, 600.0))
        );
        assert_eq!(
            move_toward(Direction::Right, WIN, &v, 0, 0.0),
            Some(Rect::new(928.0, 100.0, 800.0, 600.0))
        );
        assert_eq!(
            move_toward(Direction::Up, WIN, &v, 0, 0.0),
            Some(Rect::new(100.0, 37.0, 800.0, 600.0))
        );
        assert_eq!(
            move_toward(Direction::Down, WIN, &v, 0, 10.0),
            Some(Rect::new(100.0, 447.0, 800.0, 600.0))
        );
    }

    #[test]
    fn move_at_edge_hops_display_proportionally() {
        let v = [VIS, EXT];
        let right_half = t(SnapAction::RightHalf, 0.0);
        let moved = move_toward(Direction::Right, right_half, &v, 0, 0.0).unwrap();
        // Right half of the primary becomes the right half of the external.
        assert_eq!(moved, Rect::new(3008.0, 0.0, 1280.0, 1415.0));
        // And back again.
        let back = move_toward(
            Direction::Left,
            Rect::new(1728.0, 0.0, 1280.0, 1415.0),
            &v,
            1,
            0.0,
        );
        assert_eq!(back, Some(Rect::new(0.0, 37.0, 864.0, 1020.0)));
    }

    #[test]
    fn move_at_edge_without_display_is_none() {
        let left_half = t(SnapAction::LeftHalf, 0.0);
        assert_eq!(
            move_toward(Direction::Left, left_half, &[VIS, EXT], 0, 0.0),
            None
        );
        assert_eq!(move_toward(Direction::Up, VIS, &[VIS], 0, 0.0), None);
    }
}

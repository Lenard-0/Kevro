//! Focus-by-direction: direction selection (pure, unit-tested).

use crate::geom::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    pub fn name(self) -> &'static str {
        match self {
            Direction::Left => "focus_left",
            Direction::Right => "focus_right",
            Direction::Up => "focus_up",
            Direction::Down => "focus_down",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ]
        .into_iter()
        .find(|d| d.name() == name)
    }
}

/// How much perpendicular offset counts against a candidate, relative to
/// distance in the requested direction. Higher = prefer windows "in line".
const PERPENDICULAR_WEIGHT: f64 = 2.0;

/// A candidate's centre must be at least this far in the requested direction.
const MIN_PROGRESS: f64 = 1.0;

/// Pick the window to focus when moving in `dir` from the window at `from`.
///
/// Considers only candidates whose centre lies in `dir` from `from`'s centre.
///
/// Candidates that overlap `from` on the perpendicular axis ("in line", e.g.
/// side-by-side windows of different heights) always beat ones that don't.
/// Within each group the score is distance along `dir` plus a weighted
/// perpendicular offset. Ties go to the earlier candidate (callers pass
/// front-to-back order, so the topmost window wins).
pub fn pick(from: Rect, dir: Direction, candidates: &[Rect]) -> Option<usize> {
    let (fx, fy) = from.center();
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let (cx, cy) = c.center();
            let (progress, perp) = match dir {
                Direction::Right => (cx - fx, gap_1d(from.y, from.bottom(), c.y, c.bottom())),
                Direction::Left => (fx - cx, gap_1d(from.y, from.bottom(), c.y, c.bottom())),
                Direction::Down => (cy - fy, gap_1d(from.x, from.right(), c.x, c.right())),
                Direction::Up => (fy - cy, gap_1d(from.x, from.right(), c.x, c.right())),
            };
            let in_line = perp == 0.0;
            (progress >= MIN_PROGRESS)
                .then_some((i, (!in_line, progress + PERPENDICULAR_WEIGHT * perp)))
        })
        .fold(
            None,
            |best: Option<(usize, (bool, f64))>, (i, score)| match best {
                Some((_, s)) if s <= score => best,
                _ => Some((i, score)),
            },
        )
        .map(|(i, _)| i)
}

/// Distance between two 1-D intervals; zero if they overlap.
fn gap_1d(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    (b0 - a1).max(a0 - b1).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn names_round_trip() {
        for d in [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ] {
            assert_eq!(Direction::from_name(d.name()), Some(d));
        }
    }

    #[test]
    fn three_side_by_side_windows() {
        // Thirds of a 1800pt screen.
        let a = r(0.0, 0.0, 600.0, 1000.0);
        let b = r(600.0, 0.0, 600.0, 1000.0);
        let c = r(1200.0, 0.0, 600.0, 1000.0);
        let all = [a, b, c];
        assert_eq!(pick(a, Direction::Right, &[b, c]), Some(0));
        assert_eq!(pick(b, Direction::Right, &[a, c]), Some(1));
        assert_eq!(pick(c, Direction::Left, &[a, b]), Some(1));
        assert_eq!(pick(b, Direction::Left, &[a, c]), Some(0));
        assert_eq!(pick(c, Direction::Right, &all[..2]), None);
        assert_eq!(pick(a, Direction::Up, &[b, c]), None);
    }

    #[test]
    fn prefers_in_line_over_diagonal() {
        let from = r(0.0, 400.0, 400.0, 200.0);
        let diagonal_near = r(450.0, 0.0, 300.0, 200.0); // up and right
        let in_line_far = r(900.0, 380.0, 300.0, 250.0); // straight right
        assert_eq!(
            pick(from, Direction::Right, &[diagonal_near, in_line_far]),
            Some(1)
        );
    }

    #[test]
    fn vertical_stack() {
        let top = r(0.0, 0.0, 1000.0, 500.0);
        let bottom = r(0.0, 500.0, 1000.0, 500.0);
        assert_eq!(pick(top, Direction::Down, &[bottom]), Some(0));
        assert_eq!(pick(bottom, Direction::Up, &[top]), Some(0));
        assert_eq!(pick(bottom, Direction::Down, &[top]), None);
    }

    #[test]
    fn crosses_monitors() {
        // Window on the right edge of the primary; one on an external display
        // to the right that sits 170pt lower.
        let from = r(960.0, 30.0, 960.0, 1050.0);
        let external = r(1920.0, 170.0, 960.0, 1080.0);
        assert_eq!(pick(from, Direction::Right, &[external]), Some(0));
        assert_eq!(pick(external, Direction::Left, &[from]), Some(0));
    }

    #[test]
    fn ignores_windows_with_same_centre_and_breaks_ties_by_order() {
        let from = r(0.0, 0.0, 500.0, 500.0);
        let same_centre = r(100.0, 100.0, 300.0, 300.0);
        assert_eq!(pick(from, Direction::Right, &[same_centre]), None);
        let a = r(600.0, 0.0, 500.0, 500.0);
        assert_eq!(pick(from, Direction::Right, &[a, a]), Some(0));
    }

    #[test]
    fn gap_1d_works() {
        assert_eq!(gap_1d(0.0, 10.0, 5.0, 20.0), 0.0);
        assert_eq!(gap_1d(0.0, 10.0, 15.0, 20.0), 5.0);
        assert_eq!(gap_1d(15.0, 20.0, 0.0, 10.0), 5.0);
    }
}

//! Focus-by-direction: direction selection (pure, unit-tested).

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
        [Direction::Left, Direction::Right, Direction::Up, Direction::Down]
            .into_iter()
            .find(|d| d.name() == name)
    }
}

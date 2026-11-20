//! Snap actions and layout maths. Pure functions only: no macOS calls.

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

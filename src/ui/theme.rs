use ratatui::style::Color;

use crate::slurm::JobState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    DarkNeon,
    Classic,
}

impl Theme {
    pub const ALL: [Theme; 2] = [Theme::DarkNeon, Theme::Classic];

    pub fn id(self) -> &'static str {
        match self {
            Theme::DarkNeon => "dark-neon",
            Theme::Classic => "classic",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::DarkNeon => "Dark Neon",
            Theme::Classic => "Classic",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "dark-neon" => Some(Theme::DarkNeon),
            "classic" => Some(Theme::Classic),
            _ => None,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::DarkNeon => Palette {
                background: Color::Rgb(20, 8, 28),
                surface: Color::Rgb(37, 15, 49),
                surface_alt: Color::Rgb(54, 23, 71),
                text: Color::Rgb(245, 229, 255),
                muted: Color::Rgb(180, 145, 199),
                accent: Color::Rgb(255, 74, 189),
                accent_alt: Color::Rgb(171, 92, 255),
                border: Color::Rgb(135, 67, 173),
                success: Color::Rgb(87, 224, 172),
                warning: Color::Rgb(255, 203, 107),
                danger: Color::Rgb(255, 99, 132),
                info: Color::Rgb(104, 179, 255),
            },
            Theme::Classic => Palette {
                background: Color::Black,
                surface: Color::Black,
                surface_alt: Color::DarkGray,
                text: Color::White,
                muted: Color::Gray,
                accent: Color::Cyan,
                accent_alt: Color::Blue,
                border: Color::Cyan,
                success: Color::Green,
                warning: Color::Yellow,
                danger: Color::Red,
                info: Color::Blue,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
}

impl Palette {
    pub fn job_state(self, state: JobState) -> Color {
        match state {
            JobState::Pending => self.warning,
            JobState::Running => self.success,
            JobState::Completed => self.info,
            JobState::Failed | JobState::Timeout | JobState::NodeFail | JobState::Boot => {
                self.danger
            }
            JobState::Cancelled | JobState::Preempted => self.accent,
            JobState::Other => self.text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_ids_round_trip() {
        for theme in Theme::ALL {
            assert_eq!(Theme::from_id(theme.id()), Some(theme));
        }
    }

    #[test]
    fn dark_neon_has_distinct_focus_and_surface_colors() {
        let palette = Theme::DarkNeon.palette();
        assert_ne!(palette.accent, palette.background);
        assert_ne!(palette.surface, palette.background);
    }
}

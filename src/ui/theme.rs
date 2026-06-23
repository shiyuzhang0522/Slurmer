use ratatui::style::Color;

use crate::slurm::JobState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    SakuraLight,
    DarkNeon,
    Classic,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::SakuraLight, Theme::DarkNeon, Theme::Classic];

    pub fn id(self) -> &'static str {
        match self {
            Theme::SakuraLight => "sakura-light",
            Theme::DarkNeon => "dark-neon",
            Theme::Classic => "classic",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Theme::SakuraLight => "Sakura Cream",
            Theme::DarkNeon => "Dark Neon",
            Theme::Classic => "Classic",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "sakura-light" => Some(Theme::SakuraLight),
            "dark-neon" => Some(Theme::DarkNeon),
            "classic" => Some(Theme::Classic),
            _ => None,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::SakuraLight => Palette {
                background: Color::Rgb(255, 249, 242),
                surface: Color::Rgb(255, 232, 238),
                surface_alt: Color::Rgb(247, 211, 226),
                text: Color::Rgb(83, 39, 61),
                muted: Color::Rgb(137, 91, 117),
                accent: Color::Rgb(213, 77, 132),
                accent_alt: Color::Rgb(143, 100, 184),
                border: Color::Rgb(191, 142, 188),
                success: Color::Rgb(42, 133, 104),
                warning: Color::Rgb(177, 112, 21),
                danger: Color::Rgb(191, 54, 78),
                info: Color::Rgb(62, 103, 168),
            },
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

    #[test]
    fn sakura_theme_is_available() {
        assert_eq!(Theme::from_id("sakura-light"), Some(Theme::SakuraLight));
        assert_ne!(
            Theme::SakuraLight.palette().background,
            Theme::SakuraLight.palette().text
        );
    }
}

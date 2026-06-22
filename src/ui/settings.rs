use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::theme::{Palette, Theme};

const REFRESH_INTERVALS: [u64; 6] = [5, 10, 15, 30, 60, 120];

pub enum SettingsAction {
    None,
    Close,
    Apply { theme: Theme, refresh_interval: u64 },
}

pub struct SettingsPopup {
    pub visible: bool,
    theme_index: usize,
    interval_index: usize,
    focus: usize,
    theme_state: ListState,
    interval_state: ListState,
}

impl SettingsPopup {
    pub fn new(theme: Theme, refresh_interval: u64) -> Self {
        let theme_index = Theme::ALL
            .iter()
            .position(|item| *item == theme)
            .unwrap_or(0);
        let interval_index = REFRESH_INTERVALS
            .iter()
            .position(|item| *item == refresh_interval)
            .unwrap_or(1);
        let mut theme_state = ListState::default();
        theme_state.select(Some(theme_index));
        let mut interval_state = ListState::default();
        interval_state.select(Some(interval_index));
        Self {
            visible: false,
            theme_index,
            interval_index,
            focus: 0,
            theme_state,
            interval_state,
        }
    }

    pub fn open(&mut self, theme: Theme, refresh_interval: u64) {
        *self = Self::new(theme, refresh_interval);
        self.visible = true;
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, palette: Palette) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title(Line::from(" Settings ").centered())
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.surface).fg(palette.text)),
            area,
        );
        let areas = Layout::default()
            .direction(Direction::Horizontal)
            .margin(2)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        let themes = Theme::ALL
            .iter()
            .map(|theme| ListItem::new(theme.label()))
            .collect::<Vec<_>>();
        let intervals = REFRESH_INTERVALS
            .iter()
            .map(|seconds| ListItem::new(format!("{seconds} seconds")))
            .collect::<Vec<_>>();

        let focus_style = Style::default()
            .fg(palette.accent)
            .add_modifier(Modifier::BOLD);
        frame.render_stateful_widget(
            List::new(themes)
                .block(
                    Block::default()
                        .title("Theme")
                        .borders(Borders::ALL)
                        .border_style(if self.focus == 0 {
                            focus_style
                        } else {
                            Style::default().fg(palette.border)
                        }),
                )
                .highlight_style(Style::default().bg(palette.surface_alt).fg(palette.accent)),
            areas[0],
            &mut self.theme_state,
        );
        frame.render_stateful_widget(
            List::new(intervals)
                .block(
                    Block::default()
                        .title("Auto refresh")
                        .borders(Borders::ALL)
                        .border_style(if self.focus == 1 {
                            focus_style
                        } else {
                            Style::default().fg(palette.border)
                        }),
                )
                .highlight_style(Style::default().bg(palette.surface_alt).fg(palette.accent)),
            areas[1],
            &mut self.interval_state,
        );
        let help = Paragraph::new("←/→ field  ↑/↓ choose  Enter apply  Esc close")
            .style(Style::default().fg(palette.muted));
        let help_area = Rect::new(
            area.x.saturating_add(2),
            area.bottom().saturating_sub(2),
            area.width.saturating_sub(4),
            1,
        );
        frame.render_widget(help, help_area);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsAction {
        match key.code {
            KeyCode::Esc => SettingsAction::Close,
            KeyCode::Left | KeyCode::Right | KeyCode::Tab => {
                self.focus = 1 - self.focus;
                SettingsAction::None
            }
            KeyCode::Up => {
                if self.focus == 0 {
                    self.theme_index = self.theme_index.saturating_sub(1);
                    self.theme_state.select(Some(self.theme_index));
                } else {
                    self.interval_index = self.interval_index.saturating_sub(1);
                    self.interval_state.select(Some(self.interval_index));
                }
                SettingsAction::None
            }
            KeyCode::Down => {
                if self.focus == 0 {
                    self.theme_index = (self.theme_index + 1).min(Theme::ALL.len() - 1);
                    self.theme_state.select(Some(self.theme_index));
                } else {
                    self.interval_index =
                        (self.interval_index + 1).min(REFRESH_INTERVALS.len() - 1);
                    self.interval_state.select(Some(self.interval_index));
                }
                SettingsAction::None
            }
            KeyCode::Enter => SettingsAction::Apply {
                theme: Theme::ALL[self.theme_index],
                refresh_interval: REFRESH_INTERVALS[self.interval_index],
            },
            _ => SettingsAction::None,
        }
    }
}

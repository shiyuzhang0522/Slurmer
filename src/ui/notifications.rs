use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::Rect,
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use super::{
    text_view::{sanitize_terminal_text, wrap_text},
    theme::Palette,
};

#[derive(Default)]
pub struct NotificationView {
    pub visible: bool,
    scroll: usize,
    viewport: usize,
}

impl NotificationView {
    pub fn open(&mut self) {
        self.visible = true;
        self.scroll = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('N') => self.visible = false,
            KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.viewport),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(self.viewport),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = usize::MAX,
            _ => {}
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        statuses: Vec<String>,
        palette: Palette,
    ) {
        frame.render_widget(Clear, area);
        let mut lines = vec![
            "One email per watched parent job / array in this session.".to_string(),
            "UI watches stop when Slurmer exits. Use --watch in tmux/nohup to keep watching."
                .to_string(),
            "Mailer acceptance does not confirm inbox delivery.".to_string(),
            String::new(),
        ];
        if statuses.is_empty() {
            lines.push("No watches yet. Close this panel, highlight a job, then press n.".into());
        } else {
            lines.extend(statuses);
        }
        let text = sanitize_terminal_text(&lines.join("\n"));
        let wrapped = wrap_text(&text, area.width.saturating_sub(2).max(1) as usize);
        self.viewport = area.height.saturating_sub(2).max(1) as usize;
        self.scroll = self.scroll.min(wrapped.len().saturating_sub(self.viewport));
        let content: Vec<_> = wrapped
            .into_iter()
            .skip(self.scroll)
            .take(self.viewport)
            .map(|row| Line::from(row.text))
            .collect();
        frame.render_widget(
            Paragraph::new(content)
                .style(Style::default().bg(palette.surface).fg(palette.text))
                .block(
                    Block::default()
                        .title(" Email status · ↑/↓ PgUp/Dn · Esc close ")
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(palette.border)),
                ),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::Theme;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn end_reaches_last_watch_on_small_terminals() {
        let mut view = NotificationView::default();
        view.open();
        view.handle_key(KeyEvent::from(KeyCode::End));
        let mut terminal = Terminal::new(TestBackend::new(50, 12)).unwrap();
        terminal
            .draw(|frame| {
                view.render(
                    frame,
                    frame.area(),
                    (0..20).map(|id| format!("Job {id}: waiting")).collect(),
                    Theme::OrangeCream.palette(),
                )
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("Job 19: waiting"));
        view.handle_key(KeyEvent::from(KeyCode::Esc));
        assert!(!view.visible);
    }
}

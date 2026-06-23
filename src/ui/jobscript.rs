use std::{collections::HashMap, process::Command};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::{
    text_view::{page_metrics, sanitize_terminal_text, wrap_text, WrappedRow},
    theme::Palette,
};

pub struct JobScript {
    pub visible: bool,
    pub job_id: Option<String>,
    pub job_name: Option<String>,
    pub content: String,
    pub scroll_position: usize,
    pub script_path: Option<String>,
    viewport_rows: usize,
    wrapped_rows: Vec<WrappedRow>,
    wrap_width: usize,
}

impl JobScript {
    pub fn new() -> Self {
        Self {
            visible: false,
            job_id: None,
            job_name: None,
            content: String::new(),
            scroll_position: 0,
            script_path: None,
            viewport_rows: 1,
            wrapped_rows: Vec::new(),
            wrap_width: 0,
        }
    }

    pub fn show(&mut self, job_id: String, job_name: String) {
        self.change_job(job_id, job_name);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn change_job(&mut self, job_id: String, job_name: String) {
        self.job_id = Some(job_id);
        self.job_name = Some(job_name);
        self.script_path = None;
        self.scroll_position = 0;
        self.fetch_script_content();
        self.rewrap();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_position = (self.scroll_position + 1).min(self.max_offset());
    }

    pub fn page_up(&mut self) {
        self.scroll_position = self
            .scroll_position
            .saturating_sub(self.viewport_rows.max(1));
    }

    pub fn page_down(&mut self) {
        self.scroll_position =
            (self.scroll_position + self.viewport_rows.max(1)).min(self.max_offset());
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, palette: Palette) {
        if !self.visible {
            return;
        }

        frame.render_widget(Clear, area);
        let width = area.width.saturating_sub(2).max(1) as usize;
        self.viewport_rows = area.height.saturating_sub(2).max(1) as usize;
        if self.wrap_width != width {
            self.wrap_width = width;
            self.rewrap();
        }
        self.scroll_position = self.scroll_position.min(self.max_offset());

        let visible = self
            .wrapped_rows
            .iter()
            .skip(self.scroll_position)
            .take(self.viewport_rows)
            .map(|row| Line::raw(row.text.clone()))
            .collect::<Vec<_>>();
        let (page, pages, start, end) = page_metrics(
            self.scroll_position,
            self.viewport_rows,
            self.wrapped_rows.len(),
        );
        let source_start = self
            .wrapped_rows
            .get(self.scroll_position)
            .map(|row| row.source_line)
            .unwrap_or(0);
        let source_end = self
            .wrapped_rows
            .get(end.saturating_sub(1))
            .map(|row| row.source_line)
            .unwrap_or(0);
        let job = format!(
            "{}/{}",
            self.job_name.as_deref().unwrap_or("unknown"),
            self.job_id.as_deref().unwrap_or("unknown")
        );
        let title = format!(
            " Script {job} · Page {page}/{pages} · Rows {start}-{end} · Lines {source_start}-{source_end} "
        );

        let paragraph = Paragraph::new(Text::from(visible))
            .style(Style::default().fg(palette.text).bg(palette.background))
            .block(
                Block::default()
                    .title(title)
                    .title_bottom(" ↑/↓ scroll · PgUp/PgDn page · Shift+↑/↓ job · q close ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(palette.border)),
            );
        frame.render_widget(paragraph, area);
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Char('q')) => self.hide(),
            (_, KeyCode::Up) => self.scroll_up(),
            (_, KeyCode::Down) => self.scroll_down(),
            (_, KeyCode::PageUp) | (KeyModifiers::CONTROL, KeyCode::Char('u')) => self.page_up(),
            (_, KeyCode::PageDown) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.page_down()
            }
            (_, KeyCode::Home) => self.scroll_position = 0,
            (_, KeyCode::End) => self.scroll_position = self.max_offset(),
            _ => {}
        }
    }

    fn max_offset(&self) -> usize {
        self.wrapped_rows
            .len()
            .saturating_sub(self.viewport_rows.max(1))
    }

    fn rewrap(&mut self) {
        let clean = sanitize_terminal_text(&self.content);
        self.wrapped_rows = wrap_text(&clean, self.wrap_width.max(1));
        self.scroll_position = self.scroll_position.min(self.max_offset());
    }

    fn fetch_script_content(&mut self) {
        let Some(job_id) = &self.job_id else {
            self.content.clear();
            return;
        };
        let output = Command::new("scontrol")
            .args(["show", "job", job_id, "-o"])
            .output();
        let Ok(output) = output else {
            self.content = "Failed to execute scontrol command".to_string();
            return;
        };
        if !output.status.success() {
            self.content = "Error retrieving job information".to_string();
            return;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let values = parse_scontrol_output(&output_str);
        let Some(script_path) = values.get("Command") else {
            self.content = "No script found for this job. It may be wrapped.".to_string();
            return;
        };
        self.script_path = Some(script_path.clone());
        self.content = std::fs::read_to_string(script_path)
            .unwrap_or_else(|_| format!("Failed to read script from path: {script_path}"));
    }
}

fn parse_scontrol_output(output: &str) -> HashMap<String, String> {
    output
        .split_whitespace()
        .filter_map(|part| part.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_pages_use_viewport_height() {
        let mut view = JobScript::new();
        view.viewport_rows = 3;
        view.wrap_width = 20;
        view.content = "1\n2\n3\n4\n5\n6\n7".to_string();
        view.rewrap();
        view.page_down();
        assert_eq!(view.scroll_position, 3);
        view.page_down();
        assert_eq!(view.scroll_position, 4);
    }
}

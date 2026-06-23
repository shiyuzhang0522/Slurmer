use std::{collections::HashMap, path::PathBuf, process::Command, time::Duration};

use crossbeam::channel::{unbounded, Receiver};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::utils::file_watcher::{FileWatcherError, FileWatcherHandle};

use super::{
    text_view::{page_metrics, sanitize_terminal_text, wrap_text, WrappedRow},
    theme::Palette,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTab {
    StdOut,
    StdErr,
}

impl LogTab {
    fn toggle(&mut self) {
        *self = match self {
            LogTab::StdOut => LogTab::StdErr,
            LogTab::StdErr => LogTab::StdOut,
        };
    }

    fn as_str(self) -> &'static str {
        match self {
            LogTab::StdOut => "stdout",
            LogTab::StdErr => "stderr",
        }
    }
}

pub struct LogView {
    pub visible: bool,
    pub job_id: Option<String>,
    pub current_tab: LogTab,
    pub content: String,
    pub scroll_position: usize,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    file_watcher: Option<FileWatcherHandle>,
    file_receiver: Option<Receiver<Result<String, FileWatcherError>>>,
    refresh_interval: Duration,
    file_status: LogFileStatus,
    viewport_rows: usize,
    wrap_width: usize,
    wrapped_rows: Vec<WrappedRow>,
    follow_live: bool,
    content_dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFileStatus {
    NotFound,
    Waiting,
    Loaded,
    Error,
}

impl LogView {
    pub fn new() -> Self {
        Self {
            visible: false,
            job_id: None,
            current_tab: LogTab::StdOut,
            content: String::new(),
            scroll_position: 0,
            stdout_path: None,
            stderr_path: None,
            file_watcher: None,
            file_receiver: None,
            refresh_interval: Duration::from_secs(2),
            file_status: LogFileStatus::NotFound,
            viewport_rows: 1,
            wrap_width: 0,
            wrapped_rows: Vec::new(),
            follow_live: true,
            content_dirty: true,
        }
    }

    pub fn show(&mut self, job_id: String) {
        self.change_job(job_id);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        if let Some(watcher) = &mut self.file_watcher {
            watcher.set_file_path(None);
        }
    }

    pub fn change_job(&mut self, job_id: String) {
        self.job_id = Some(job_id);
        self.stdout_path = None;
        self.stderr_path = None;
        self.content.clear();
        self.wrapped_rows.clear();
        self.scroll_position = 0;
        self.follow_live = true;
        self.content_dirty = true;
        self.file_status = LogFileStatus::NotFound;
        self.fetch_log_paths();

        if self.file_watcher.is_none() {
            let (sender, receiver) = unbounded();
            self.file_watcher = Some(FileWatcherHandle::new(sender, self.refresh_interval));
            self.file_receiver = Some(receiver);
        }
        self.update_watched_file();
    }

    pub fn toggle_tab(&mut self) {
        self.current_tab.toggle();
        self.content.clear();
        self.wrapped_rows.clear();
        self.scroll_position = 0;
        self.follow_live = true;
        self.content_dirty = true;
        self.update_watched_file();
    }

    fn update_watched_file(&mut self) {
        if let Some(watcher) = &mut self.file_watcher {
            let path = match self.current_tab {
                LogTab::StdOut => self.stdout_path.clone(),
                LogTab::StdErr => self.stderr_path.clone(),
            };
            match path {
                Some(path) if !path.is_empty() => {
                    watcher.set_file_path(Some(PathBuf::from(path)));
                    self.file_status = LogFileStatus::Waiting;
                }
                _ => {
                    watcher.set_file_path(None);
                    self.file_status = LogFileStatus::NotFound;
                }
            }
        }
        self.check_refresh();
    }

    pub fn check_refresh(&mut self) {
        let mut latest = None;
        if let Some(receiver) = self.file_receiver.as_ref() {
            while let Ok(result) = receiver.try_recv() {
                latest = Some(result);
            }
        }
        if let Some(result) = latest {
            match result {
                Ok(content) => {
                    self.content = content;
                    self.file_status = LogFileStatus::Loaded;
                    self.content_dirty = true;
                }
                Err(error) => {
                    self.content = format!("Error watching file: {error}");
                    self.file_status = LogFileStatus::Error;
                    self.content_dirty = true;
                }
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.follow_live = false;
        self.scroll_position = self.scroll_position.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_position = (self.scroll_position + 1).min(self.max_offset());
        self.follow_live = self.scroll_position == self.max_offset();
    }

    pub fn page_up(&mut self) {
        self.follow_live = false;
        self.scroll_position = self
            .scroll_position
            .saturating_sub(self.viewport_rows.max(1));
    }

    pub fn page_down(&mut self) {
        self.scroll_position =
            (self.scroll_position + self.viewport_rows.max(1)).min(self.max_offset());
        self.follow_live = self.scroll_position == self.max_offset();
    }

    pub fn resume_live(&mut self) {
        self.follow_live = true;
        self.scroll_position = self.max_offset();
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
            self.content_dirty = true;
        }
        if self.content_dirty {
            self.rewrap();
            self.content_dirty = false;
        }
        if self.follow_live {
            self.scroll_position = self.max_offset();
        } else {
            self.scroll_position = self.scroll_position.min(self.max_offset());
        }

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
        let mode = if self.follow_live { "LIVE" } else { "PAUSED" };
        let title = format!(
            " Job {} · {} · Page {page}/{pages} · Rows {start}-{end} · Lines {source_start}-{source_end} · ",
            self.job_id.as_deref().unwrap_or("unknown"),
            self.current_tab.as_str()
        );

        let text = match self.file_status {
            LogFileStatus::NotFound => Text::from(Line::raw(format!(
                "No {} log file found for job {}",
                self.current_tab.as_str(),
                self.job_id.as_deref().unwrap_or("unknown")
            ))),
            LogFileStatus::Waiting if self.content.is_empty() => {
                Text::from(Line::raw("Waiting for log output…"))
            }
            LogFileStatus::Error if self.content.is_empty() => {
                Text::from(Line::raw("Unable to access this job log."))
            }
            _ => Text::from(
                self.wrapped_rows
                    .iter()
                    .skip(self.scroll_position)
                    .take(self.viewport_rows)
                    .map(|row| Line::raw(row.text.clone()))
                    .collect::<Vec<_>>(),
            ),
        };

        let badge_color = if self.follow_live {
            palette.success
        } else {
            palette.warning
        };
        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(palette.text).bg(palette.background))
            .block(
                Block::default()
                    .title(Line::from(vec![
                        Span::raw(title),
                        Span::styled(
                            mode,
                            Style::default()
                                .fg(badge_color)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]))
                    .title_bottom(
                        " ↑/↓ scroll · PgUp/PgDn page · End live · o stdout/stderr · Shift+↑/↓ job · q close ",
                    )
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(palette.border)),
            );
        frame.render_widget(paragraph, area);
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (_, KeyCode::Char('o')) => self.toggle_tab(),
            (_, KeyCode::Char('q')) => self.hide(),
            (_, KeyCode::Up) => self.scroll_up(),
            (_, KeyCode::Down) => self.scroll_down(),
            (_, KeyCode::PageUp) | (KeyModifiers::CONTROL, KeyCode::Char('u')) => self.page_up(),
            (_, KeyCode::PageDown) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.page_down()
            }
            (_, KeyCode::Home) => {
                self.follow_live = false;
                self.scroll_position = 0;
            }
            (_, KeyCode::End) => self.resume_live(),
            _ => {}
        }
    }

    fn rewrap(&mut self) {
        let clean = sanitize_terminal_text(&self.content);
        self.wrapped_rows = wrap_text(&clean, self.wrap_width.max(1));
        self.scroll_position = self.scroll_position.min(self.max_offset());
    }

    fn max_offset(&self) -> usize {
        self.wrapped_rows
            .len()
            .saturating_sub(self.viewport_rows.max(1))
    }

    fn fetch_log_paths(&mut self) {
        let Some(job_id) = &self.job_id else {
            self.file_status = LogFileStatus::NotFound;
            return;
        };
        let output = Command::new("scontrol")
            .args(["show", "job", job_id, "-o"])
            .output();
        let Ok(output) = output else {
            self.file_status = LogFileStatus::Error;
            return;
        };
        if !output.status.success() {
            self.file_status = LogFileStatus::Error;
            return;
        }

        let values = parse_scontrol_output(&String::from_utf8_lossy(&output.stdout));
        self.stdout_path = values.get("StdOut").cloned();
        self.stderr_path = values.get("StdErr").cloned();
        let has_path = match self.current_tab {
            LogTab::StdOut => self
                .stdout_path
                .as_deref()
                .is_some_and(|path| !path.is_empty()),
            LogTab::StdErr => self
                .stderr_path
                .as_deref()
                .is_some_and(|path| !path.is_empty()),
        };
        self.file_status = if has_path {
            LogFileStatus::Waiting
        } else {
            LogFileStatus::NotFound
        };
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
    fn scrolling_up_pauses_and_end_resumes_live_mode() {
        let mut view = LogView::new();
        view.viewport_rows = 2;
        view.wrap_width = 20;
        view.content = "1\n2\n3\n4".to_string();
        view.rewrap();
        view.resume_live();
        assert!(view.follow_live);
        view.scroll_up();
        assert!(!view.follow_live);
        view.resume_live();
        assert!(view.follow_live);
        assert_eq!(view.scroll_position, view.max_offset());
    }

    #[test]
    fn tab_switch_restores_live_follow() {
        let mut view = LogView::new();
        view.follow_live = false;
        view.toggle_tab();
        assert!(view.follow_live);
        assert_eq!(view.scroll_position, 0);
    }
}

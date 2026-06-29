use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Clear, Row, Table, TableState},
    Frame,
};

use crate::{
    search::fuzzy_score,
    slurm::sacct::{AccountingJob, AccountingStateFilter, SacctOptions},
};

use super::{text_view::page_metrics, theme::Palette};

const DAY_CHOICES: [u16; 3] = [1, 7, 30];

pub enum HistoryAction {
    Close,
    Refresh,
    None,
}

pub struct HistoryView {
    pub visible: bool,
    pub options: SacctOptions,
    pub jobs: Vec<AccountingJob>,
    all_jobs: Vec<AccountingJob>,
    state: TableState,
    search_mode: bool,
    search_query: String,
    viewport_rows: usize,
}

impl HistoryView {
    pub fn new(user: Option<String>) -> Self {
        let options = SacctOptions {
            user,
            ..SacctOptions::default()
        };
        Self {
            visible: false,
            options,
            jobs: Vec::new(),
            all_jobs: Vec::new(),
            state: TableState::default(),
            search_mode: false,
            search_query: String::new(),
            viewport_rows: 1,
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.search_mode = false;
    }

    pub fn update_jobs(&mut self, jobs: Vec<AccountingJob>) {
        let selected_id = self.selected_job().map(|job| job.id.clone());
        self.all_jobs = jobs;
        self.apply_search();
        self.state.select(
            selected_id
                .and_then(|id| self.jobs.iter().position(|job| job.id == id))
                .or_else(|| (!self.jobs.is_empty()).then_some(0)),
        );
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect, palette: Palette) {
        if !self.visible {
            return;
        }

        frame.render_widget(Clear, area);
        self.viewport_rows = area.height.saturating_sub(3).max(1) as usize;

        let headers = [
            "ID", "Name", "State", "Exit", "Elapsed", "Start", "End", "Part", "Acct", "CPU",
            "ReqMem", "MaxRSS",
        ];
        let header = Row::new(headers.iter().map(|header| {
            Cell::from(*header).style(
                Style::default()
                    .fg(palette.text)
                    .bg(palette.surface_alt)
                    .add_modifier(Modifier::BOLD),
            )
        }))
        .height(1);

        let rows = self.jobs.iter().map(|job| {
            let state_color = state_color(&job.state, palette);
            Row::new(vec![
                Cell::from(job.id.clone()),
                Cell::from(truncate(&job.name, 24)),
                Cell::from(job.state.clone()),
                Cell::from(job.exit_code.clone()),
                Cell::from(job.elapsed.clone()),
                Cell::from(short_time(&job.start)),
                Cell::from(short_time(&job.end)),
                Cell::from(job.partition.clone()),
                Cell::from(job.account.clone()),
                Cell::from(
                    job.cpus
                        .map(|cpus| cpus.to_string())
                        .unwrap_or_else(|| "-".into()),
                ),
                Cell::from(job.requested_memory.clone().unwrap_or_else(|| "-".into())),
                Cell::from(job.max_rss.clone().unwrap_or_else(|| "-".into())),
            ])
            .style(Style::default().fg(state_color).bg(palette.background))
            .height(1)
        });

        let count = self.jobs.len();
        let selected = self.state.selected().unwrap_or(0);
        let page_offset = (selected / self.viewport_rows.max(1)) * self.viewport_rows.max(1);
        let (page, pages, start, end) = page_metrics(page_offset, self.viewport_rows, count);
        let mut title = format!(
            " History · {}d · {} · {count} jobs · Page {page}/{pages} · Rows {start}-{end} ",
            self.options.days,
            self.options.state_filter.label()
        );
        if !self.search_query.is_empty() {
            title.push_str(&format!(
                "· /{} [{}/{}] ",
                self.search_query,
                self.jobs.len(),
                self.all_jobs.len()
            ));
        }
        if self.search_mode {
            title.push_str("· search ");
        }

        let table = Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Min(16),
                Constraint::Length(14),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(16),
                Constraint::Length(16),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(5),
                Constraint::Length(8),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .title(title)
                .title_bottom(
                    " ↑/↓ scroll · PgUp/PgDn page · / search · f state · t range · r refresh · q close ",
                )
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border))
                .style(Style::default().bg(palette.background)),
        )
        .row_highlight_style(
            Style::default()
                .bg(palette.surface)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" ◆ ");

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> HistoryAction {
        if self.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_query.clear();
                    self.apply_search();
                    self.state.select((!self.jobs.is_empty()).then_some(0));
                }
                KeyCode::Enter => self.search_mode = false,
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.apply_search();
                    self.state.select((!self.jobs.is_empty()).then_some(0));
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    self.search_query.push(character);
                    self.apply_search();
                    self.state.select((!self.jobs.is_empty()).then_some(0));
                }
                _ => {}
            }
            return HistoryAction::None;
        }

        match (key.modifiers, key.code) {
            (_, KeyCode::Esc) | (_, KeyCode::Char('q')) => HistoryAction::Close,
            (_, KeyCode::Char('/')) => {
                self.search_mode = true;
                HistoryAction::None
            }
            (_, KeyCode::Char('r')) => HistoryAction::Refresh,
            (_, KeyCode::Char('f')) => {
                self.options.state_filter = self.options.state_filter.next();
                HistoryAction::Refresh
            }
            (_, KeyCode::Char('t')) => {
                self.options.days = next_days(self.options.days);
                HistoryAction::Refresh
            }
            (_, KeyCode::Up) => {
                self.previous();
                HistoryAction::None
            }
            (_, KeyCode::Down) => {
                self.next();
                HistoryAction::None
            }
            (_, KeyCode::PageUp) | (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
                self.page_up();
                HistoryAction::None
            }
            (_, KeyCode::PageDown) | (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
                self.page_down();
                HistoryAction::None
            }
            (_, KeyCode::Home) => {
                self.state.select((!self.jobs.is_empty()).then_some(0));
                HistoryAction::None
            }
            (_, KeyCode::End) => {
                self.state
                    .select((!self.jobs.is_empty()).then_some(self.jobs.len() - 1));
                HistoryAction::None
            }
            _ => HistoryAction::None,
        }
    }

    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.jobs = self.all_jobs.clone();
            return;
        }

        let mut scored = self
            .all_jobs
            .iter()
            .filter_map(|job| {
                history_score(&self.search_query, job).map(|score| (score, job.clone()))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.0.cmp(&left.0));
        self.jobs = scored.into_iter().map(|(_, job)| job).collect();
    }

    fn selected_job(&self) -> Option<&AccountingJob> {
        self.state.selected().and_then(|index| self.jobs.get(index))
    }

    fn next(&mut self) {
        if self.jobs.is_empty() {
            return;
        }
        let index = self.state.selected().unwrap_or(0);
        self.state
            .select(Some((index + 1).min(self.jobs.len() - 1)));
    }

    fn previous(&mut self) {
        if self.jobs.is_empty() {
            return;
        }
        let index = self.state.selected().unwrap_or(0);
        self.state.select(Some(index.saturating_sub(1)));
    }

    fn page_down(&mut self) {
        if self.jobs.is_empty() {
            return;
        }
        let index = self.state.selected().unwrap_or(0);
        self.state.select(Some(
            (index + self.viewport_rows.max(1)).min(self.jobs.len() - 1),
        ));
    }

    fn page_up(&mut self) {
        if self.jobs.is_empty() {
            return;
        }
        let index = self.state.selected().unwrap_or(0);
        self.state
            .select(Some(index.saturating_sub(self.viewport_rows.max(1))));
    }
}

fn history_score(query: &str, job: &AccountingJob) -> Option<i64> {
    [
        job.id.as_str(),
        job.name.as_str(),
        job.user.as_str(),
        job.state.as_str(),
        job.partition.as_str(),
        job.account.as_str(),
    ]
    .iter()
    .filter_map(|field| fuzzy_score(query, field))
    .max()
}

fn next_days(current: u16) -> u16 {
    let index = DAY_CHOICES
        .iter()
        .position(|days| *days == current)
        .unwrap_or(1);
    DAY_CHOICES[(index + 1) % DAY_CHOICES.len()]
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn short_time(value: &str) -> String {
    if value.len() >= 16 && value.as_bytes().get(10) == Some(&b'T') {
        value[..16].to_string()
    } else {
        value.to_string()
    }
}

fn state_color(state: &str, palette: Palette) -> ratatui::style::Color {
    match state {
        "COMPLETED" => palette.success,
        "FAILED" | "NODE_FAIL" | "BOOT_FAIL" | "TIMEOUT" => palette.danger,
        "CANCELLED" | "PREEMPTED" => palette.warning,
        _ => palette.text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, name: &str, state: &str) -> AccountingJob {
        AccountingJob {
            id: id.to_string(),
            name: name.to_string(),
            state: state.to_string(),
            user: "shelley".to_string(),
            ..AccountingJob::default()
        }
    }

    #[test]
    fn day_filter_cycles_through_supported_ranges() {
        assert_eq!(next_days(1), 7);
        assert_eq!(next_days(7), 30);
        assert_eq!(next_days(30), 1);
    }

    #[test]
    fn search_filters_history_jobs() {
        let mut view = HistoryView::new(Some("shelley".to_string()));
        view.update_jobs(vec![
            job("1", "gpu-train", "FAILED"),
            job("2", "align", "COMPLETED"),
        ]);
        view.search_query = "gpu".to_string();
        view.apply_search();
        assert_eq!(view.jobs.len(), 1);
        assert_eq!(view.jobs[0].id, "1");
    }
}

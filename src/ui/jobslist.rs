use std::collections::HashSet;

use ratatui::{
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::search::job_score;
use crate::slurm::Job;
use crate::ui::columns::{JobColumn, SortColumn};
use crate::ui::theme::Palette;

/// Struct to manage the jobs list view
pub struct JobsList {
    pub state: TableState,
    pub jobs: Vec<Job>,
    all_jobs: Vec<Job>,
    pub selected_jobs: HashSet<String>,
    search_query: String,
    pub sort_column: usize,
    pub sort_ascending: bool,
    viewport_rows: usize,
}

impl JobsList {
    pub fn new() -> Self {
        Self {
            state: TableState::default(),
            jobs: Vec::new(),
            all_jobs: Vec::new(),
            selected_jobs: HashSet::new(),
            search_query: String::new(),
            sort_column: 0, // Default sort by job ID
            sort_ascending: true,
            viewport_rows: 1,
        }
    }

    /// Update the list of jobs
    pub fn update_jobs(&mut self, jobs: Vec<Job>) {
        let selected_id = self.selected_job().map(|job| job.id.clone());
        self.all_jobs = jobs;
        self.selected_jobs
            .retain(|id| self.all_jobs.iter().any(|job| &job.id == id));
        self.apply_search();

        if let Some(id) = selected_id {
            if let Some(index) = self.jobs.iter().position(|job| job.id == id) {
                self.state.select(Some(index));
                return;
            }
        }
        self.state.select((!self.jobs.is_empty()).then_some(0));
    }

    pub fn set_search_query(&mut self, query: String) {
        let selected_id = self.selected_job().map(|job| job.id.clone());
        self.search_query = query;
        self.apply_search();
        self.state.select(
            selected_id
                .and_then(|id| self.jobs.iter().position(|job| job.id == id))
                .or_else(|| (!self.jobs.is_empty()).then_some(0)),
        );
    }

    pub fn total_count(&self) -> usize {
        self.all_jobs.len()
    }

    fn apply_search(&mut self) {
        if self.search_query.is_empty() {
            self.jobs = self.all_jobs.clone();
            return;
        }
        let mut scored = self
            .all_jobs
            .iter()
            .filter_map(|job| job_score(&self.search_query, job).map(|score| (score, job.clone())))
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.0.cmp(&left.0));
        self.jobs = scored.into_iter().map(|(_, job)| job).collect();
    }

    /// Toggle job selection
    pub fn toggle_select(&mut self) {
        if let Some(selected) = self.state.selected() {
            if let Some(job) = self.jobs.get(selected) {
                if !self.selected_jobs.remove(&job.id) {
                    self.selected_jobs.insert(job.id.clone());
                }
            }
        }
    }

    /// Judge if all jobs are selected
    pub fn all_selected(&self) -> bool {
        !self.jobs.is_empty()
            && self
                .jobs
                .iter()
                .all(|job| self.selected_jobs.contains(&job.id))
    }

    /// Select all jobs
    pub fn select_all(&mut self) {
        self.selected_jobs
            .extend(self.jobs.iter().map(|job| job.id.clone()));
    }

    /// Clear all selections
    pub fn clear_selection(&mut self) {
        self.selected_jobs.clear();
    }

    /// Update sort configuration based on SortColumn settings
    pub fn update_sort(&mut self, columns: &[JobColumn], sort_columns: &[SortColumn]) {
        if let Some(first_sort) = sort_columns.first() {
            // Find the index of the column in the displayed columns list
            let column_index = columns
                .iter()
                .position(|col| {
                    std::mem::discriminant(col) == std::mem::discriminant(&first_sort.column)
                })
                .unwrap_or(0);

            self.sort_column = column_index;
            self.sort_ascending =
                matches!(first_sort.order, crate::ui::columns::SortOrder::Ascending);
            // No need to sort jobs as sorting is handled by squeue
        }
    }

    /// Navigate to next job
    /// Returns true if selection changed, false otherwise
    pub fn next(&mut self) -> bool {
        if self.jobs.is_empty() {
            return false;
        }

        let old_selection = self.state.selected();
        let i = match old_selection {
            Some(i) => {
                if i >= self.jobs.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        old_selection != Some(i)
    }

    /// Navigate to previous job
    /// Returns true if selection changed, false otherwise
    pub fn previous(&mut self) -> bool {
        if self.jobs.is_empty() {
            return false;
        }

        let old_selection = self.state.selected();
        let i = match old_selection {
            Some(i) => {
                if i == 0 {
                    self.jobs.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        old_selection != Some(i)
    }

    pub fn page_down(&mut self) -> bool {
        self.move_by_page(false)
    }

    pub fn page_up(&mut self) -> bool {
        self.move_by_page(true)
    }

    fn move_by_page(&mut self, upward: bool) -> bool {
        if self.jobs.is_empty() {
            return false;
        }
        let old = self.state.selected().unwrap_or(0);
        let step = self.viewport_rows.max(1);
        let next = if upward {
            old.saturating_sub(step)
        } else {
            (old + step).min(self.jobs.len() - 1)
        };
        self.state.select(Some(next));
        old != next
    }

    /// Draw the jobs list widget
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        columns: &[JobColumn],
        sort_columns: &[SortColumn],
        palette: Palette,
    ) {
        self.viewport_rows = area.height.saturating_sub(3).max(1) as usize;
        // Update sorting if needed based on sort_columns
        if !sort_columns.is_empty() {
            self.update_sort(columns, sort_columns);
        }

        // Check if columns are empty, show warning if so
        if columns.is_empty() {
            let warning = Paragraph::new("No columns selected. Press 'c' to configure columns.")
                .style(Style::default().fg(palette.warning).bg(palette.background))
                .block(
                    Block::default()
                        .title("Warning")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(palette.border)),
                );
            frame.render_widget(warning, area);
            return;
        }

        // Create headers based on selected columns
        let headers: Vec<&str> = columns.iter().map(|col| col.title()).collect();

        // Create header cells with appropriate styling
        let header_cells = headers.iter().enumerate().map(|(_i, &h)| {
            // Check if this column is in the sort list
            let is_sort_column = sort_columns.iter().any(|sc| sc.column.title() == h);
            let sort_indicator = if is_sort_column {
                match sort_columns
                    .iter()
                    .find(|sc| sc.column.title() == h)
                    .map(|sort| sort.order)
                {
                    Some(crate::ui::columns::SortOrder::Ascending) => " ↑",
                    Some(crate::ui::columns::SortOrder::Descending) => " ↓",
                    None => "",
                }
            } else {
                ""
            };

            let header_style = if is_sort_column {
                Style::default()
                    .fg(palette.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD)
            };

            Cell::from(format!("{}{}", h, sort_indicator)).style(header_style)
        });

        let header = Row::new(header_cells)
            .style(Style::default().bg(palette.surface_alt))
            .height(1);

        // Create rows for each job
        let rows = self.jobs.iter().map(|job| {
            let is_selected = self.selected_jobs.contains(&job.id);
            let color = palette.job_state(job.state);

            let style = if is_selected {
                Style::default()
                    .fg(palette.background)
                    .bg(color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color).bg(palette.background)
            };

            // Create cells based on selected columns
            let cells: Vec<Cell> = columns
                .iter()
                .map(|col| {
                    let content = match col {
                        JobColumn::Id => job.id.clone(),
                        JobColumn::Name => {
                            // Truncate name if too long
                            if job.name.chars().count() > 30 {
                                format!("{}...", job.name.chars().take(27).collect::<String>())
                            } else {
                                job.name.clone()
                            }
                        }
                        JobColumn::User => job.user.clone(),
                        JobColumn::State => job.state.to_string(),
                        JobColumn::Partition => job.partition.clone(),
                        JobColumn::QoS => job.qos.clone(),
                        JobColumn::Nodes => job.nodes.to_string(),
                        JobColumn::Node => job.node.clone().unwrap_or_else(|| "-".to_string()),
                        JobColumn::CPUs => job.cpus.to_string(),
                        JobColumn::Time => job.time.clone(),
                        JobColumn::Memory => job.memory.clone(),
                        JobColumn::Account => {
                            job.account.clone().unwrap_or_else(|| "-".to_string())
                        }
                        JobColumn::Priority => job
                            .priority
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        JobColumn::WorkDir => {
                            job.work_dir.clone().unwrap_or_else(|| "-".to_string())
                        }
                        JobColumn::SubmitTime => {
                            job.submit_time.clone().unwrap_or_else(|| "-".to_string())
                        }
                        JobColumn::StartTime => {
                            job.start_time.clone().unwrap_or_else(|| "-".to_string())
                        }
                        JobColumn::EndTime => {
                            job.end_time.clone().unwrap_or_else(|| "-".to_string())
                        }
                        JobColumn::PReason => job
                            .pending_reason
                            .clone()
                            .unwrap_or_else(|| "-".to_string()),
                    };
                    Cell::from(content)
                })
                .collect();

            Row::new(cells).style(style).height(1)
        });

        // Calculate total available width
        // let available_width = area.width.saturating_sub(2); // Subtract 2 for borders

        // Get constraints for columns using the default_width method from JobColumn
        let constraints: Vec<Constraint> = columns
            .iter()
            .map(|col| {
                // Use the default_width from JobColumn, but with some specific overrides
                // for better display in the jobs list context
                match col {
                    // Override specific columns that need different constraints in the jobs list
                    JobColumn::Name => Constraint::Min(15),
                    JobColumn::WorkDir => Constraint::Min(20),
                    // For time-related columns, we use a slightly longer constraint
                    JobColumn::SubmitTime | JobColumn::StartTime | JobColumn::EndTime => {
                        Constraint::Length(19)
                    }
                    // Use the default_width for all other columns
                    _ => col.default_width(),
                }
            })
            .collect();

        // Create the table
        let job_count = self.jobs.len();
        let selected = self.state.selected().unwrap_or(0);
        let page_offset = (selected / self.viewport_rows.max(1)) * self.viewport_rows.max(1);
        let (page, pages, start, end) =
            crate::ui::text_view::page_metrics(page_offset, self.viewport_rows, job_count);
        let title = if job_count == 0 {
            "0 Jobs".to_string()
        } else {
            format!("{job_count} Jobs · Page {page}/{pages} · Rows {start}-{end}")
        };
        let table = Table::new(rows, constraints)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(palette.border))
                    .style(Style::default().bg(palette.background)),
            )
            .row_highlight_style(
                Style::default()
                    .bg(palette.surface)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(" ◆ ");

        // Render the table
        frame.render_stateful_widget(table, area, &mut self.state);
    }

    /// Get the currently selected job, if any
    pub fn selected_job(&self) -> Option<&Job> {
        self.state.selected().and_then(|i| self.jobs.get(i))
    }

    /// Get all selected jobs
    pub fn get_selected_jobs(&self) -> Vec<String> {
        self.selected_jobs.iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, name: &str) -> Job {
        Job {
            id: id.to_string(),
            name: name.to_string(),
            ..Job::default()
        }
    }

    #[test]
    fn selections_survive_refresh_by_id() {
        let mut list = JobsList::new();
        list.update_jobs(vec![job("1", "one"), job("2", "two")]);
        list.state.select(Some(1));
        list.toggle_select();
        list.update_jobs(vec![job("2", "two"), job("1", "one")]);
        assert_eq!(list.get_selected_jobs(), vec!["2".to_string()]);
    }

    #[test]
    fn fuzzy_search_filters_and_ranks_jobs() {
        let mut list = JobsList::new();
        list.update_jobs(vec![job("1", "great-purple-unit"), job("2", "gpu-worker")]);
        list.set_search_query("gpu".to_string());
        assert_eq!(list.jobs.len(), 2);
        assert_eq!(list.jobs[0].id, "2");
    }

    #[test]
    fn page_navigation_clamps_to_bounds() {
        let mut list = JobsList::new();
        list.viewport_rows = 3;
        list.update_jobs((0..8).map(|id| job(&id.to_string(), "job")).collect());
        list.page_down();
        assert_eq!(list.state.selected(), Some(3));
        list.page_down();
        list.page_down();
        assert_eq!(list.state.selected(), Some(7));
        list.page_up();
        assert_eq!(list.state.selected(), Some(4));
    }
}

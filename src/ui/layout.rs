use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::Duration;

use super::theme::Palette;

/// Defines the main layout of the application
pub fn draw_main_layout(frame: &mut Frame) -> Vec<Rect> {
    let size = frame.area();

    // Create the main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header area with status
            Constraint::Min(10),   // Main content area
            Constraint::Length(3), // Footer area with controls
        ])
        .split(size);

    let main_chunk = chunks[1];

    vec![chunks[0], main_chunk, chunks[2]]
}

/// Draws the application header with status information
pub fn draw_header(
    frame: &mut Frame,
    area: Rect,
    status_text: &str,
    time_since_refresh: Duration,
    refresh_interval: u64,
    search: Option<(&str, usize, usize)>,
    palette: Palette,
) {
    // Split the header area into title and status
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20), // Title
            Constraint::Percentage(80), // Status
        ])
        .split(area);

    // Render the title part
    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled("SLURMER", Style::default().fg(palette.accent).bold()),
        Span::raw(" - "),
        Span::styled("HPC job console", Style::default().fg(palette.accent_alt)),
    ])]))
    .style(Style::default().bg(palette.surface).fg(palette.text))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(palette.border)),
    );

    frame.render_widget(title, header_chunks[0]);

    // Render the status part
    let mut status_info = format!(
        "{} | Refresh: {}s ago (auto: {}s)",
        status_text,
        time_since_refresh.as_secs(),
        refresh_interval
    );
    if let Some((query, matches, total)) = search {
        status_info.push_str(&format!(" | /{query} [{matches}/{total}]"));
    }

    let status = Paragraph::new(status_info)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border)),
        )
        .style(Style::default().bg(palette.surface).fg(palette.text));

    frame.render_widget(status, header_chunks[1]);
}

/// Draws the application footer with help text and status
pub fn draw_footer(
    frame: &mut Frame,
    area: Rect,
    job_stat: (usize, usize, usize),
    palette: Palette,
) {
    // Controls help (lower part of footer)
    let color_style = Style::default().fg(palette.accent);
    let text_hashmap = [
        ("Esc", "Quit"),
        ("/", "Search"),
        ("s", "Settings"),
        ("↑/↓", "Navigate"),
        ("PgUp/Dn", "Page"),
        ("Space", "Select"),
        ("Enter", "Script"),
        ("f", "Filter"),
        ("c", "Columns"),
        ("v", "Log"),
        ("h", "History"),
        ("a", "SelectAll"),
        ("r", "Refresh"),
        ("x", "Cancel"),
    ];

    let mut footer_text: Vec<Span> = text_hashmap
        .iter()
        .flat_map(|(key, description)| {
            vec![
                Span::styled(*key, color_style),
                Span::raw(": "),
                Span::raw(*description),
                Span::raw(" "),
            ]
        })
        .collect();

    footer_text.push(Span::styled(
        "Job Stat: ",
        Style::default().fg(palette.accent),
    ));
    footer_text.push(Span::styled(
        format!("P[ {} ] ", job_stat.0),
        Style::default().fg(palette.warning),
    ));
    footer_text.push(Span::styled(
        format!("R[ {} ] ", job_stat.1),
        Style::default().fg(palette.success),
    ));
    footer_text.push(Span::styled(
        format!("Other[ {} ]", job_stat.2),
        Style::default().fg(palette.info),
    ));

    let footer = Paragraph::new(Line::from(footer_text))
        .style(Style::default().bg(palette.surface).fg(palette.text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(palette.border)),
        );

    frame.render_widget(footer, area);
}

/// Creates a popup area in the center of the screen
pub fn centered_popup_area(frame_size: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(frame_size);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

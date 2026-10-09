use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
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
            Constraint::Length(if size.width >= 110 { 5 } else { 6 }), // Wrapped controls
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
            Constraint::Length(22), // Title
            Constraint::Min(0),     // Status
        ])
        .split(area);

    // Render the title part
    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled("(o) SLURMER", Style::default().fg(palette.accent).bold()),
        Span::raw(" "),
        Span::styled("HPC", Style::default().fg(palette.accent_alt)),
    ])]))
    .style(Style::default().bg(palette.surface).fg(palette.text))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
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
                .border_type(BorderType::Rounded)
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
        ("n", "Email"),
        ("N", "Email status"),
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
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(palette.surface).fg(palette.text))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::Theme;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn notification_shortcuts_fit_standard_terminal() {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                let areas = draw_main_layout(frame);
                let palette = Theme::OrangeCream.palette();
                draw_header(frame, areas[0], "Ready", Duration::ZERO, 10, None, palette);
                draw_footer(frame, areas[2], (2, 3, 1), palette);
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("n: Email"));
        assert!(text.contains("N: Email status"));
        assert!(text.contains("Cancel"));
        assert!(text.contains("Other[ 1 ]"));
    }
}

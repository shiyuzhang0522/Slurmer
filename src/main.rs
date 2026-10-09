use color_eyre::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;

mod app;
mod config;
mod notifications;
mod search;
mod slurm;
mod ui;
mod utils;

use app::App;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => {}
        [flag] if flag == "--help" || flag == "-h" => {
            println!("Slurmer\n\n  slurmer                Open the terminal UI\n  slurmer --watch JOB_ID Watch one job / entire array and email on completion\n\nRecipient: SLURMER_EMAIL (default: {})\nWatch mode polls every 30s and requires squeue, sacct and sendmail/mail.", notifications::DEFAULT_EMAIL);
            return Ok(());
        }
        [flag, id] if flag == "--watch" => {
            let email = notifications::recipient()?;
            return notifications::watch(id, &email, |message| eprintln!("{message}"));
        }
        _ => {
            return Err(color_eyre::eyre::eyre!(
                "Unknown arguments. Run slurmer --help."
            ))
        }
    }
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        let mut app = App::new()?;
        app.run(&mut terminal)
    })();

    let restore_result = (|| -> Result<()> {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        Ok(())
    })();

    result?;
    restore_result
}

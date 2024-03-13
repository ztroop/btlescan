#[macro_use]
extern crate lazy_static;
use crate::viewer::viewer;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use scan::bluetooth_scan;
use std::{error::Error, io};
use tokio::sync::mpsc;

mod company_codes;
mod scan;
mod structs;
mod viewer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move {
        if let Err(e) = bluetooth_scan(tx).await {
            eprintln!("Error during Bluetooth scan: {}", e);
        }
    });

    if let Err(e) = viewer(&mut terminal, rx).await {
        eprintln!("Error running application: {}", e);
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    Ok(())
}

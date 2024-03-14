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
use std::{
    error::Error,
    io,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::mpsc;

mod company_codes;
mod scan;
mod structs;
mod utils;
mod viewer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, rx) = mpsc::channel(100);
    let pause_signal = Arc::new(AtomicBool::new(false));
    let pause_signal_clone = Arc::clone(&pause_signal);

    tokio::spawn(async move {
        if let Err(e) = bluetooth_scan(tx, pause_signal_clone).await {
            eprintln!("Error during Bluetooth scan: {}", e);
        }
    });

    if let Err(e) = viewer(&mut terminal, rx, pause_signal).await {
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

use crate::viewer::viewer;
use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::{error::Error, io};

mod app;
mod company_codes;
mod scan;
#[cfg(feature = "server")]
mod server;
mod structs;
mod utils;
mod viewer;
mod widgets;

/// Restores raw mode and alternate screen on drop. Used to guard the setup phase
/// before TerminalGuard exists, so early failures (e.g. Terminal::new) still clean up.
struct SetupGuard;

impl Drop for SetupGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste
        );
    }
}

/// Ensures terminal state is restored on drop (including on panic).
struct TerminalGuard {
    terminal: Option<Terminal<CrosstermBackend<io::Stdout>>>,
}

impl TerminalGuard {
    fn new(terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Self {
        Self {
            terminal: Some(terminal),
        }
    }

    fn get_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        self.terminal.as_mut().expect("terminal is Some until Drop")
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if let Some(terminal) = &mut self.terminal {
            let _ = execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
            );
            let _ = terminal.show_cursor();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _setup = SetupGuard;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    let mut guard = TerminalGuard::new(terminal);

    guard.get_mut().clear()?;

    let mut app = app::App::new();
    app.scan();
    viewer(guard.get_mut(), &mut app).await
    // TerminalGuard's Drop restores terminal state on return or panic
}

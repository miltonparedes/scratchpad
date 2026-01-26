mod app;
mod ui;

pub use app::App;

use std::io;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::models::Config;
use crate::open::open_path_nonblocking;
use crate::storage::Storage;

pub fn run(config: Config, session_id: Option<&str>) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let storage = Storage::new(config.clone());
    let mut app = App::new(storage, config);

    let res = run_app(&mut terminal, &mut app, session_id);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    session_id: Option<&str>,
) -> Result<()> {
    app.refresh_sessions()?;
    if let Some(id) = session_id {
        app.select_session_by_prefix(id);
    }

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Event::Key(key) = event::read()? {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(());
            }

            match app.handle_key(key) {
                app::Action::Quit => return Ok(()),
                app::Action::Continue => {}
                app::Action::RunAgent(session_id, agent) => {
                    disable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        LeaveAlternateScreen,
                        DisableMouseCapture
                    )?;
                    terminal.show_cursor()?;

                    let session_dir = app.storage.session_dir(&session_id);
                    let status = std::process::Command::new(agent.command())
                        .current_dir(&session_dir)
                        .status();

                    enable_raw_mode()?;
                    execute!(
                        terminal.backend_mut(),
                        EnterAlternateScreen,
                        EnableMouseCapture
                    )?;

                    if let Err(e) = status {
                        app.set_error(format!("Failed to run agent: {}", e));
                    }

                    app.refresh_sessions()?;
                }
                app::Action::OpenExternal(path) => {
                    if let Err(e) = open_path_nonblocking(&path, app.config.viewer.as_deref()) {
                        app.set_error(format!("Failed to open: {}", e));
                    }
                }
            }
        }
    }
}

//! The schedule UI: owns the terminal, event loop, state, input and view.

mod app;
mod input;
mod view;

use std::io;
use std::time::Duration;

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;

use crate::session_data::SessionData;

type Tui = Terminal<CrosstermBackend<std::io::Stdout>>;

pub fn run(data: SessionData) -> io::Result<()> {
    let mut app = app::App::new(data);
    let mut terminal = ratatui::init();
    let result = run_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run_loop(terminal: &mut Tui, app: &mut app::App) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| view::draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    input::handle_key(app, key);
                }
            }
        }

        app.tick();
    }
    Ok(())
}

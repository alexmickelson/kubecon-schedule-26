//! Vim-style key mapping. Keeps terminal events out of the display state.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;

const DETAIL_PAGE: isize = 12;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    if app.detail_open {
        handle_detail_key(app, key);
    } else {
        handle_grid_key(app, key);
    }
}

fn handle_grid_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => app.move_session(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_session(-1),
        KeyCode::Char('h') | KeyCode::Left => app.move_column(-1),
        KeyCode::Char('l') | KeyCode::Right => app.move_column(1),
        KeyCode::Char('d') if ctrl => app.page(1),
        KeyCode::Char('u') if ctrl => app.page(-1),
        KeyCode::PageDown => app.page(1),
        KeyCode::PageUp => app.page(-1),
        KeyCode::Char('g') => app.select_first(),
        KeyCode::Char('G') => app.select_last(),
        KeyCode::Enter => app.open_detail(),
        KeyCode::Char('t') => app.focus_now(),
        _ => {}
    }
}

fn handle_detail_key(app: &mut App, key: KeyEvent) {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left | KeyCode::Enter => {
            app.detail_open = false;
        }
        KeyCode::Char('j') | KeyCode::Down => app.scroll_detail(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_detail(-1),
        KeyCode::Char('d') if ctrl => app.scroll_detail(DETAIL_PAGE),
        KeyCode::Char('u') if ctrl => app.scroll_detail(-DETAIL_PAGE),
        KeyCode::PageDown => app.scroll_detail(DETAIL_PAGE),
        KeyCode::PageUp => app.scroll_detail(-DETAIL_PAGE),
        KeyCode::Char('g') => app.detail_scroll = 0,
        KeyCode::Char('G') => app.detail_scroll = u16::MAX,
        KeyCode::Char('t') => {
            app.detail_open = false;
            app.focus_now();
        }
        _ => {}
    }
}

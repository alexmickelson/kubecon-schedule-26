//! Vim-style key mapping. Keeps terminal events out of the display state.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;

const DETAIL_PAGE_LINES: isize = 12;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit_requested = true;
        return;
    }

    if app.session_detail_open {
        handle_session_detail_key(app, key);
    } else {
        handle_schedule_grid_key(app, key);
    }
}

fn handle_schedule_grid_key(app: &mut App, key: KeyEvent) {
    let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => app.quit_requested = true,
        KeyCode::Char('j') | KeyCode::Down => app.select_adjacent_session(1),
        KeyCode::Char('k') | KeyCode::Up => app.select_adjacent_session(-1),
        KeyCode::Char('h') | KeyCode::Left => app.select_adjacent_room(-1),
        KeyCode::Char('l') | KeyCode::Right => app.select_adjacent_room(1),
        KeyCode::Char('d') if control_pressed => app.scroll_half_page(1),
        KeyCode::Char('u') if control_pressed => app.scroll_half_page(-1),
        KeyCode::PageDown => app.scroll_half_page(1),
        KeyCode::PageUp => app.scroll_half_page(-1),
        KeyCode::Char('g') => app.select_first_session_in_room(),
        KeyCode::Char('G') => app.select_last_session_in_room(),
        KeyCode::Enter => app.open_session_detail(),
        KeyCode::Char('t') => app.select_current_or_next_session(),
        _ => {}
    }
}

fn handle_session_detail_key(app: &mut App, key: KeyEvent) {
    let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => app.quit_requested = true,
        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left | KeyCode::Enter => {
            app.session_detail_open = false;
        }
        KeyCode::Char('j') | KeyCode::Down => app.scroll_session_detail(1),
        KeyCode::Char('k') | KeyCode::Up => app.scroll_session_detail(-1),
        KeyCode::Char('d') if control_pressed => app.scroll_session_detail(DETAIL_PAGE_LINES),
        KeyCode::Char('u') if control_pressed => app.scroll_session_detail(-DETAIL_PAGE_LINES),
        KeyCode::PageDown => app.scroll_session_detail(DETAIL_PAGE_LINES),
        KeyCode::PageUp => app.scroll_session_detail(-DETAIL_PAGE_LINES),
        KeyCode::Char('g') => app.session_detail_scroll_line = 0,
        KeyCode::Char('G') => app.session_detail_scroll_line = u16::MAX,
        KeyCode::Char('t') => {
            app.session_detail_open = false;
            app.select_current_or_next_session();
        }
        _ => {}
    }
}

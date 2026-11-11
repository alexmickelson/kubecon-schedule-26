//! Vim-style key mapping. Keeps terminal events out of the display state.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::actions;
use super::app::App;

const DETAIL_PAGE_LINES: isize = 12;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit_requested = true;
        return;
    }

    if app.search.is_some() {
        handle_search_key(app, key);
    } else if app.session_detail_open {
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
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Char('N') => app.select_next_day(),
        KeyCode::Char('P') => app.select_previous_day(),
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

fn handle_search_key(app: &mut App, key: KeyEvent) {
    let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => app.cancel_search(),
        KeyCode::Enter => app.confirm_search(),
        KeyCode::Backspace => app.search_pop_char(),
        KeyCode::Up => app.select_search_match(-1),
        KeyCode::Down => app.select_search_match(1),
        KeyCode::Char('p') if control_pressed => app.select_search_match(-1),
        KeyCode::Char('n') if control_pressed => app.select_search_match(1),
        KeyCode::Char(character)
            if !control_pressed && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.search_push_char(character);
        }
        _ => {}
    }
}

fn handle_session_detail_key(app: &mut App, key: KeyEvent) {
    let control_pressed = key.modifiers.contains(KeyModifiers::CONTROL);
    if !matches!(key.code, KeyCode::Char('y') | KeyCode::Char('o')) {
        app.session_detail_action_message = None;
    }
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc | KeyCode::Char('h') | KeyCode::Left | KeyCode::Enter => {
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
        KeyCode::Char('y') => {
            let message = match app.focused_session_url() {
                Some(url) => match actions::copy_to_clipboard(&url) {
                    Ok(()) => "URL copied to clipboard".to_string(),
                    Err(error) => format!("Copy failed: {error}"),
                },
                None => "No session URL available".to_string(),
            };
            app.session_detail_action_message = Some(message);
        }
        KeyCode::Char('o') => {
            let message = match app.focused_session_url() {
                Some(url) => match actions::open_in_browser(&url) {
                    Ok(()) => "Opened session in browser".to_string(),
                    Err(error) => format!("Open failed: {error}"),
                },
                None => "No session URL available".to_string(),
            };
            app.session_detail_action_message = Some(message);
        }
        KeyCode::Char('t') => {
            app.session_detail_open = false;
            app.select_current_or_next_session();
        }
        _ => {}
    }
}

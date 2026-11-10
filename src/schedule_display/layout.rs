//! Grid geometry and text measurement. Time rows grow to fit the tallest
//! wrapped session title in that row, so every talk's title stays readable.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::app::App;
use crate::session_data::Session;

/// Lines occupied by a session's title and track/subtrack metadata in a room
/// column.
pub fn session_content_line_count(session: &Session, room_column_width: usize) -> usize {
    let title_wrap_width = room_column_width.saturating_sub(2).max(1);
    let title_line_count = wrap_text_to_width(&session.title, title_wrap_width)
        .len()
        .max(1);
    let metadata_line_count = session
        .track_and_subtrack()
        .map(|metadata| wrap_text_to_width(&metadata, title_wrap_width).len())
        .unwrap_or(0);
    title_line_count + metadata_line_count
}

/// Recompute time row heights and their grid line offsets. Cached per room
/// column width and current time row.
pub fn measure_time_row_line_heights(app: &mut App, room_column_width: usize) {
    let current_time_row = app.time_row_containing_now();
    if app.measured_room_column_width == Some(room_column_width)
        && app.measured_current_time_row == current_time_row
        && app.time_row_line_heights.len() == app.time_row_starts.len()
    {
        return;
    }

    let mut time_row_line_heights = vec![1usize; app.time_row_starts.len()];

    for room_column in &app.room_columns {
        for &session_index in &room_column.session_indices_by_start {
            let session = &app.schedule.sessions[session_index];
            let session_start_time_row = app
                .time_row_starts
                .partition_point(|time_row_start| time_row_start.as_str() < session.start.as_str());
            let cell_line_count = session_content_line_count(session, room_column_width);
            if let Some(time_row_line_height) =
                time_row_line_heights.get_mut(session_start_time_row)
            {
                *time_row_line_height = (*time_row_line_height).max(cell_line_count);
            }
        }
    }

    // The current time row gets one extra line for the now indicator.
    if let Some(current_time_row) = current_time_row {
        if let Some(time_row_line_height) = time_row_line_heights.get_mut(current_time_row) {
            *time_row_line_height += 1;
        }
    }

    let mut time_row_line_offsets = Vec::with_capacity(app.time_row_starts.len() + 1);
    let mut total_grid_lines = 0;
    time_row_line_offsets.push(0);
    for time_row_line_height in &time_row_line_heights {
        total_grid_lines += *time_row_line_height;
        time_row_line_offsets.push(total_grid_lines);
    }

    app.time_row_line_heights = time_row_line_heights;
    app.time_row_line_offsets = time_row_line_offsets;
    app.measured_room_column_width = Some(room_column_width);
    app.measured_current_time_row = current_time_row;
}

/// Greedy word wrap by terminal display width.
pub fn wrap_text_to_width(text: &str, width: usize) -> Vec<String> {
    let max_line_width = width.max(1);
    let mut wrapped_lines = Vec::new();

    for paragraph in text.split('\n') {
        let mut current_line = String::new();
        let mut current_line_width = 0;

        for word in paragraph.split(' ') {
            let word_width = text_display_width(word);

            if current_line.is_empty() {
                if word_width <= max_line_width {
                    current_line.push_str(word);
                    current_line_width = word_width;
                } else {
                    push_split_long_word(
                        &mut wrapped_lines,
                        &mut current_line,
                        &mut current_line_width,
                        word,
                        max_line_width,
                    );
                }
                continue;
            }

            if current_line_width + 1 + word_width <= max_line_width {
                current_line.push(' ');
                current_line.push_str(word);
                current_line_width += 1 + word_width;
            } else {
                wrapped_lines.push(std::mem::take(&mut current_line));
                current_line_width = 0;
                if word_width <= max_line_width {
                    current_line.push_str(word);
                    current_line_width = word_width;
                } else {
                    push_split_long_word(
                        &mut wrapped_lines,
                        &mut current_line,
                        &mut current_line_width,
                        word,
                        max_line_width,
                    );
                }
            }
        }

        if !current_line.is_empty() || paragraph.is_empty() {
            wrapped_lines.push(current_line);
        }
    }

    if wrapped_lines.is_empty() {
        wrapped_lines.push(String::new());
    }
    wrapped_lines
}

fn push_split_long_word(
    wrapped_lines: &mut Vec<String>,
    current_line: &mut String,
    current_line_width: &mut usize,
    long_word: &str,
    max_line_width: usize,
) {
    for character in long_word.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if *current_line_width + character_width > max_line_width && !current_line.is_empty() {
            wrapped_lines.push(std::mem::take(current_line));
            *current_line_width = 0;
        }
        current_line.push(character);
        *current_line_width += character_width;
    }
}

pub fn text_display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn pad_text_to_width(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(text_display_width(text)))
    )
}

pub fn truncate_text_to_width(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text_display_width(text) <= width {
        return text.to_string();
    }

    let max_text_width = width.saturating_sub(1);
    let mut shortened_text = String::new();
    let mut used_width = 0;
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used_width + character_width > max_text_width {
            break;
        }
        shortened_text.push(character);
        used_width += character_width;
    }
    shortened_text.push('…');
    shortened_text
}

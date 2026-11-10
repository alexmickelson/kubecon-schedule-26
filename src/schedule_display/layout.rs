//! Grid geometry and text measurement. Time rows grow to fit the tallest
//! wrapped session title in that row, so every talk's title stays readable.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::app::App;

/// Recompute row heights and their line offsets. Cached per column width.
pub fn update_rows(app: &mut App, column_width: usize) {
    if app.layout_width == Some(column_width) && app.row_heights.len() == app.rows.len() {
        return;
    }

    let content_width = column_width.saturating_sub(2).max(1);
    let mut heights = vec![1usize; app.rows.len()];

    for column in &app.columns {
        for &index in &column.sessions {
            let session = &app.data.sessions[index];
            let start_row = app
                .rows
                .partition_point(|row| row.as_str() < session.start.as_str());
            let height = wrap(&session.title, content_width).len().max(1);
            if let Some(slot) = heights.get_mut(start_row) {
                *slot = (*slot).max(height);
            }
        }
    }

    let mut offsets = Vec::with_capacity(app.rows.len() + 1);
    let mut total = 0;
    offsets.push(0);
    for height in &heights {
        total += *height;
        offsets.push(total);
    }

    app.row_heights = heights;
    app.row_offsets = offsets;
    app.layout_width = Some(column_width);
}

/// Greedy word wrap by terminal display width.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();

    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_width = 0;

        for word in paragraph.split(' ') {
            let word_width = display_width(word);

            if line.is_empty() {
                if word_width <= width {
                    line.push_str(word);
                    line_width = word_width;
                } else {
                    push_split_word(&mut out, &mut line, &mut line_width, word, width);
                }
                continue;
            }

            if line_width + 1 + word_width <= width {
                line.push(' ');
                line.push_str(word);
                line_width += 1 + word_width;
            } else {
                out.push(std::mem::take(&mut line));
                line_width = 0;
                if word_width <= width {
                    line.push_str(word);
                    line_width = word_width;
                } else {
                    push_split_word(&mut out, &mut line, &mut line_width, word, width);
                }
            }
        }

        if !line.is_empty() || paragraph.is_empty() {
            out.push(line);
        }
    }

    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn push_split_word(
    out: &mut Vec<String>,
    line: &mut String,
    line_width: &mut usize,
    word: &str,
    width: usize,
) {
    for character in word.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if *line_width + character_width > width && !line.is_empty() {
            out.push(std::mem::take(line));
            *line_width = 0;
        }
        line.push(character);
        *line_width += character_width;
    }
}

pub fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub fn pad_right(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(display_width(text)))
    )
}

pub fn truncate(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if display_width(text) <= width {
        return text.to_string();
    }

    let limit = width.saturating_sub(1);
    let mut shortened = String::new();
    let mut used = 0;
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if used + character_width > limit {
            break;
        }
        shortened.push(character);
        used += character_width;
    }
    shortened.push('…');
    shortened
}

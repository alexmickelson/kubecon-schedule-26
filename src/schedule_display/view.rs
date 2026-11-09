//! All rendering. Widgets only read `App`; the only state written here is
//! scroll bookkeeping that depends on the available terminal size.

use chrono::Datelike;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::app::App;
use crate::session_data::{compact_room, Session, Speaker, Status};

const ACCENT: Color = Color::Rgb(97, 175, 239);
const LIVE: Color = Color::Rgb(152, 195, 121);
const DIM: Color = Color::Rgb(92, 99, 112);
const FAINT: Color = Color::Rgb(55, 60, 72);
const SELECT_BG: Color = Color::Rgb(38, 79, 120);

/// Width of the time gutter: now marker + "HH:MM" + space.
const GUTTER: usize = 7;
const MIN_COLUMN_WIDTH: usize = 24;
const MAX_COLUMN_WIDTH: usize = 40;

pub fn draw(frame: &mut Frame, app: &mut App) {
    if app.detail_open {
        draw_detail(frame, app);
        return;
    }

    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    draw_grid(frame, app, chunks[1], chunks[2]);
    draw_footer(frame, app, chunks[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width as usize;
    let clock = format!(
        "{} {} {}  {} {} ",
        app.now.format("%a"),
        app.now.format("%b"),
        app.now.day(),
        app.now.format("%H:%M:%S"),
        app.now.format("%Z"),
    );
    let left = truncate(
        &format!(" {}", app.data.event.name),
        width.saturating_sub(display_width(&clock)),
    );
    let padding = width.saturating_sub(display_width(&left) + display_width(&clock));

    let line1 = Line::from(vec![
        Span::styled(
            left,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(padding)),
        Span::styled(clock, Style::default().fg(Color::Gray)),
    ]);

    let day = &app.data.days[app.day_index];
    let session_count: usize = app.columns.iter().map(|column| column.sessions.len()).sum();
    let live_count = app
        .columns
        .iter()
        .flat_map(|column| column.sessions.iter())
        .filter(|&&index| app.data.sessions[index].status(app.now, app.tz) == Status::Live)
        .count();
    let mut line2 = format!(
        " {}  ·  {}  ·  {} sessions",
        day.label, app.data.event.city, session_count
    );
    if live_count > 0 {
        line2.push_str(&format!("  ·  {live_count} live"));
    }

    frame.render_widget(
        Paragraph::new(vec![
            line1,
            Line::from(Span::styled(line2, Style::default().fg(DIM))),
        ]),
        area,
    );
}

fn draw_grid(frame: &mut Frame, app: &mut App, header_area: Rect, body_area: Rect) {
    if app.columns.is_empty() || app.rows.is_empty() {
        frame.render_widget(
            Paragraph::new("No sessions scheduled.")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            body_area,
        );
        return;
    }

    let width = body_area.width as usize;
    let usable = width.saturating_sub(GUTTER);
    let max_columns = (usable / (MIN_COLUMN_WIDTH + 1)).max(1);
    let view_columns = max_columns.min(app.columns.len());
    let column_width = (usable / view_columns)
        .saturating_sub(1)
        .clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
    let view_rows = body_area.height as usize;

    app.view_rows = view_rows;
    app.view_cols = view_columns;
    update_offsets(app, view_rows, view_columns);

    draw_column_headers(frame, app, header_area, column_width, view_columns);
    draw_grid_body(frame, app, body_area, column_width, view_columns);
}

fn draw_column_headers(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    column_width: usize,
    view_columns: usize,
) {
    let mut names = vec![Span::raw(" ".repeat(GUTTER))];
    let mut rule = "─".repeat(GUTTER);

    for offset in 0..view_columns {
        let col = app.col_offset + offset;
        if col >= app.columns.len() {
            break;
        }

        names.push(Span::styled("│", Style::default().fg(DIM)));
        let name = compact_room(&app.columns[col].name);
        let style = if col == app.selected_col() {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::Gray)
        };
        names.push(Span::styled(
            pad_right(&truncate(&name, column_width), column_width),
            style,
        ));

        rule.push('┼');
        rule.push_str(&"─".repeat(column_width));
    }

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(names),
            Line::from(Span::styled(rule, Style::default().fg(DIM))),
        ]),
        area,
    );
}

fn draw_grid_body(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    column_width: usize,
    view_columns: usize,
) {
    let current_row = app.current_row();
    let mut lines: Vec<Line> = Vec::with_capacity(area.height as usize);

    for screen_row in 0..area.height as usize {
        let row = app.row_offset + screen_row;
        if row >= app.rows.len() {
            break;
        }

        let mut spans = Vec::new();
        let is_current = current_row == Some(row);
        let label_style = if is_current {
            Style::default().fg(LIVE).add_modifier(Modifier::BOLD)
        } else if current_row.is_some_and(|current| row < current) {
            Style::default().fg(DIM)
        } else {
            Style::default().fg(Color::Gray)
        };
        let marker = if is_current { "▶" } else { " " };
        spans.push(Span::styled(
            format!("{marker}{:>5} ", app.row_label(row)),
            label_style,
        ));

        for offset in 0..view_columns {
            let col = app.col_offset + offset;
            if col >= app.columns.len() {
                break;
            }
            spans.push(Span::styled("│", Style::default().fg(DIM)));

            let cell = app.columns[col].cells.get(row).copied().flatten();
            match cell {
                Some(index) => {
                    let session = &app.data.sessions[index];
                    let (start_row, _) = app.session_span(index);
                    let selected = index == app.selected;
                    let status = session.status(app.now, app.tz);

                    let (text, style) = if row == start_row {
                        let glyph = if status == Status::Live { "▶" } else { " " };
                        let title = truncate(&session.title, column_width.saturating_sub(2));
                        let base = match status {
                            Status::Live => Style::default().fg(LIVE).add_modifier(Modifier::BOLD),
                            Status::Upcoming => Style::default().fg(Color::White),
                            Status::Past => Style::default().fg(DIM),
                        };
                        (format!("{glyph} {title}"), base)
                    } else {
                        (" ┆".to_string(), Style::default().fg(FAINT))
                    };

                    let style = if selected {
                        style.bg(SELECT_BG).add_modifier(Modifier::BOLD)
                    } else {
                        style
                    };
                    spans.push(Span::styled(pad_right(&text, column_width), style));
                }
                None => spans.push(Span::raw(" ".repeat(column_width))),
            }
        }

        lines.push(Line::from(spans));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn update_offsets(app: &mut App, view_rows: usize, view_columns: usize) {
    if app.rows.is_empty() || app.columns.is_empty() || view_rows == 0 || view_columns == 0 {
        return;
    }

    let (selected_start, selected_end) = app.session_span(app.selected);
    let selected_col = app.selected_col();

    if app.center_selection {
        app.row_offset = selected_start.saturating_sub(view_rows / 3);
        app.col_offset = selected_col.saturating_sub(view_columns / 2);
        app.center_selection = false;
    }

    // Keep the viewport put when a long session still occupies it; only scroll
    // when the session is completely above or below the visible rows.
    if selected_end <= app.row_offset {
        app.row_offset = selected_start;
    } else if selected_start >= app.row_offset + view_rows {
        app.row_offset = selected_start + 1 - view_rows;
    }

    if selected_col < app.col_offset {
        app.col_offset = selected_col;
    } else if selected_col >= app.col_offset + view_columns {
        app.col_offset = selected_col + 1 - view_columns;
    }

    app.row_offset = app.row_offset.min(app.rows.len().saturating_sub(view_rows));
    app.col_offset = app
        .col_offset
        .min(app.columns.len().saturating_sub(view_columns));
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let left = " h/l room · j/k session · g/G · Ctrl-d/u · Enter details · t now · q quit ";
    let right = if app.columns.is_empty() {
        String::new()
    } else {
        let first = app.col_offset + 1;
        let last = (app.col_offset + app.view_cols).min(app.columns.len());
        format!("rooms {first}-{last}/{} ", app.columns.len())
    };
    let padding = (area.width as usize).saturating_sub(display_width(left) + display_width(&right));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, Style::default().fg(DIM)),
            Span::raw(" ".repeat(padding)),
            Span::styled(right, Style::default().fg(DIM)),
        ])),
        area,
    );
}

fn draw_detail(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            " Session details ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(chunks[0]);
    frame.render_widget(block, chunks[0]);

    let Some(session) = app.session() else {
        frame.render_widget(
            Paragraph::new("No session selected.").style(Style::default().fg(DIM)),
            inner,
        );
        return;
    };

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let lines = detail_lines(app, session);
    // One extra line of slack: our wrap estimate is approximate, and it is
    // better to allow scrolling a line too far than to hide the last line.
    let total = wrapped_line_count(&lines, inner.width as usize) + 1;
    app.detail_total = total;
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });

    let max_scroll = total.saturating_sub(1).min(u16::MAX as usize) as u16;
    app.detail_scroll = app.detail_scroll.min(max_scroll);
    frame.render_widget(paragraph.scroll((app.detail_scroll, 0)), inner);

    frame.render_widget(
        Paragraph::new(" Esc/h back · j/k scroll · Ctrl-d/u page · g/G top/bottom ")
            .style(Style::default().fg(DIM))
            .alignment(Alignment::Center),
        chunks[1],
    );
}

fn detail_lines(app: &App, session: &Session) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(Span::styled(
        session.title.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        status_text(app, session),
        status_style(app, session),
    )));
    lines.push(Line::default());

    let day_label = &app.data.days[app.day_index].label;
    let tz_name = session
        .start_at(app.tz)
        .map(|time| time.format("%Z").to_string())
        .unwrap_or_default();
    let when = format!(
        "{} · {}–{} · {} min ({})",
        day_label,
        session.start_hm(app.tz),
        session.end_hm(app.tz),
        session.duration_minutes(app.tz),
        tz_name
    );
    lines.push(field("When", when));
    lines.push(field("Room", session.room.clone()));

    if let Some(track) = session.track() {
        lines.push(field("Track", track.to_string()));
    }
    if let Some(level) = session.level() {
        lines.push(field("Level", level.to_string()));
    }
    let projects = session.projects();
    if !projects.is_empty() {
        lines.push(field("Projects", projects.join(", ")));
    }
    let subtopics = session.subtopics();
    if !subtopics.is_empty() {
        lines.push(field("Subtopics", subtopics.join(", ")));
    }
    let tags = session.tags();
    if !tags.is_empty() {
        lines.push(field("Tags", tags.join(", ")));
    }

    lines.push(Line::default());
    lines.push(section("About"));
    match session.description.as_deref() {
        Some(description) if !description.trim().is_empty() => {
            push_paragraph(&mut lines, description, Style::default().fg(Color::Gray));
        }
        _ => lines.push(Line::from(Span::styled(
            "No description.",
            Style::default().fg(DIM),
        ))),
    }

    let speakers: Vec<&Speaker> = session
        .speakers
        .iter()
        .filter_map(|id| app.data.speakers.get(id))
        .collect();
    if !speakers.is_empty() {
        lines.push(Line::default());
        lines.push(section("Speakers"));
        for (index, speaker) in speakers.iter().enumerate() {
            let mut header = vec![Span::styled(
                speaker.name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )];
            if let Some(tagline) = &speaker.tagline {
                header.push(Span::styled(
                    format!("  {tagline}"),
                    Style::default().fg(DIM),
                ));
            }
            lines.push(Line::from(header));

            for link in &speaker.links {
                let label = link.title.as_deref().unwrap_or("Link");
                lines.push(Line::from(Span::styled(
                    format!("  {label}: {}", link.url),
                    Style::default().fg(ACCENT),
                )));
            }
            if let Some(bio) = &speaker.bio {
                push_paragraph(&mut lines, bio, Style::default().fg(Color::Gray));
            }
            if index + 1 < speakers.len() {
                lines.push(Line::default());
            }
        }
    }

    lines
}

fn status_text(app: &App, session: &Session) -> String {
    match session.status(app.now, app.tz) {
        Status::Live => match session.end_at(app.tz) {
            Some(end) => format!("● LIVE · ends in {}", humanize(end - app.now)),
            None => "● LIVE".to_string(),
        },
        Status::Upcoming => match session.start_at(app.tz) {
            Some(start) => format!("Starts in {}", humanize(start - app.now)),
            None => "Upcoming".to_string(),
        },
        Status::Past => match session.end_at(app.tz) {
            Some(end) => format!("Ended {} ago", humanize(app.now - end)),
            None => "Ended".to_string(),
        },
    }
}

fn status_style(app: &App, session: &Session) -> Style {
    match session.status(app.now, app.tz) {
        Status::Live => Style::default().fg(LIVE).add_modifier(Modifier::BOLD),
        Status::Upcoming => Style::default().fg(Color::Yellow),
        Status::Past => Style::default().fg(DIM),
    }
}

fn humanize(delta: chrono::Duration) -> String {
    let minutes = delta.num_minutes().max(0);
    if minutes == 0 {
        "<1m".to_string()
    } else if minutes < 60 {
        format!("{minutes}m")
    } else if minutes < 60 * 24 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{}d {}h", minutes / (60 * 24), (minutes % (60 * 24)) / 60)
    }
}

fn field(label: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<10}"),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        ),
        Span::styled(value, Style::default().fg(Color::Gray)),
    ])
}

fn section(label: &str) -> Line<'static> {
    Line::from(Span::styled(
        label.to_string(),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
}

fn push_paragraph(lines: &mut Vec<Line<'static>>, text: &str, style: Style) {
    for line in text.lines() {
        lines.push(Line::from(Span::styled(line.trim_end().to_string(), style)));
    }
}

/// Approximate the number of terminal rows used by wrapped plain text, so the
/// detail pane can clamp its scroll offset. Styling does not affect wrapping.
fn wrapped_line_count(lines: &[Line<'static>], width: usize) -> usize {
    let width = width.max(1);
    lines
        .iter()
        .map(|line| {
            let text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            wrap_line_count(&text, width)
        })
        .sum::<usize>()
        .max(1)
}

fn wrap_line_count(text: &str, width: usize) -> usize {
    if text.is_empty() {
        return 1;
    }
    let mut rows = 0;
    let mut current = 0;
    for word in text.split(' ') {
        let length = display_width(word);
        if current == 0 {
            current = length;
        } else if current + 1 + length <= width {
            current += 1 + length;
        } else {
            rows += 1;
            current = length;
        }
        while current > width {
            rows += 1;
            current -= width;
        }
    }
    rows + 1
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn pad_right(text: &str, width: usize) -> String {
    format!(
        "{text}{}",
        " ".repeat(width.saturating_sub(display_width(text)))
    )
}

fn truncate(text: &str, width: usize) -> String {
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

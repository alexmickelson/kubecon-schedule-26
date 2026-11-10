//! All rendering. Widgets only read `App`; the only state written here is
//! scroll/layout bookkeeping that depends on the available terminal size.

use chrono::Datelike;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;

use super::app::{App, ViewAnchor};
use super::layout;
use crate::session_data::{compact_room, Session, Speaker, Status};

const ACCENT: Color = Color::Rgb(97, 175, 239);
const LIVE: Color = Color::Rgb(152, 195, 121);
const DIM: Color = Color::Rgb(92, 99, 112);
const FAINT: Color = Color::Rgb(55, 60, 72);
const SELECTED_SESSION_BACKGROUND: Color = Color::Rgb(38, 79, 120);
const SUBTRACK: Color = Color::Rgb(122, 130, 144);
const SELECTED_SUBTRACK: Color = Color::Rgb(176, 188, 204);

/// Width of the time gutter: now marker + "12:00 PM" + space.
const TIME_GUTTER_WIDTH: usize = 10;
const MIN_ROOM_COLUMN_WIDTH: usize = 20;
const MAX_ROOM_COLUMN_WIDTH: usize = 32;

pub fn draw(frame: &mut Frame, app: &mut App) {
    if app.session_detail_open {
        draw_session_detail(frame, app);
        return;
    }

    let screen_area = frame.area();
    let (visible_room_count, room_column_width) =
        grid_metrics(screen_area.width as usize, app.room_columns.len());
    app.first_visible_room_index = room_column_offset_for_selection(app, visible_room_count);
    let room_header_height = wrapped_room_header_height(
        app,
        app.first_visible_room_index,
        visible_room_count,
        room_column_width,
    );
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(room_header_height as u16),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(screen_area);

    draw_event_header(frame, app, panes[0]);
    draw_schedule_grid(
        frame,
        app,
        panes[1],
        panes[2],
        visible_room_count,
        room_column_width,
    );
    draw_key_help_footer(frame, app, panes[3]);
}

fn grid_metrics(screen_width: usize, room_count: usize) -> (usize, usize) {
    if room_count == 0 {
        return (0, MIN_ROOM_COLUMN_WIDTH);
    }

    let grid_width_after_gutter = screen_width.saturating_sub(TIME_GUTTER_WIDTH);
    let max_visible_rooms = (grid_width_after_gutter / (MIN_ROOM_COLUMN_WIDTH + 1)).max(1);
    let visible_room_count = max_visible_rooms.min(room_count);
    let room_column_width = (grid_width_after_gutter / visible_room_count)
        .saturating_sub(1)
        .clamp(MIN_ROOM_COLUMN_WIDTH, MAX_ROOM_COLUMN_WIDTH);
    (visible_room_count, room_column_width)
}

fn room_column_offset_for_selection(app: &App, visible_room_count: usize) -> usize {
    if visible_room_count == 0 || app.room_columns.is_empty() {
        return 0;
    }

    let selected_room_column = app.selected_room_column();
    let mut first_visible_room_index = if app.pending_view_anchor.is_some() {
        selected_room_column.saturating_sub(visible_room_count / 2)
    } else {
        app.first_visible_room_index
    };

    if selected_room_column < first_visible_room_index {
        first_visible_room_index = selected_room_column;
    } else if selected_room_column >= first_visible_room_index + visible_room_count {
        first_visible_room_index = selected_room_column + 1 - visible_room_count;
    }

    first_visible_room_index.min(app.room_columns.len().saturating_sub(visible_room_count))
}

fn wrapped_room_header_height(
    app: &App,
    first_visible_room_index: usize,
    visible_room_count: usize,
    room_column_width: usize,
) -> usize {
    let max_room_name_line_count = (first_visible_room_index
        ..first_visible_room_index + visible_room_count)
        .filter_map(|room_column_index| app.room_columns.get(room_column_index))
        .map(|room_column| {
            let room_name = compact_room(&room_column.room_name);
            layout::wrap_text_to_width(&room_name, room_column_width)
                .len()
                .max(1)
        })
        .max()
        .unwrap_or(1);
    max_room_name_line_count + 1
}

fn draw_event_header(frame: &mut Frame, app: &App, area: Rect) {
    let screen_width = area.width as usize;
    let clock_text = format!(
        "{} {} {}  {} {} ",
        app.now.format("%a"),
        app.now.format("%b"),
        app.now.day(),
        app.now.format("%H:%M:%S"),
        app.now.format("%Z"),
    );
    let event_title = layout::truncate_text_to_width(
        &format!(" {}", app.schedule.event.name),
        screen_width.saturating_sub(layout::text_display_width(&clock_text)),
    );
    let gap_width = screen_width.saturating_sub(
        layout::text_display_width(&event_title) + layout::text_display_width(&clock_text),
    );

    let event_title_line = Line::from(vec![
        Span::styled(
            event_title,
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap_width)),
        Span::styled(clock_text, Style::default().fg(Color::Gray)),
    ]);

    let event_day = &app.schedule.days[app.day_index];
    let session_count: usize = app
        .room_columns
        .iter()
        .map(|room_column| room_column.session_indices_by_start.len())
        .sum();
    let live_session_count = app
        .room_columns
        .iter()
        .flat_map(|room_column| room_column.session_indices_by_start.iter())
        .filter(|&&session_index| {
            app.schedule.sessions[session_index].status(app.now, app.tz) == Status::Live
        })
        .count();
    let mut event_summary_line = format!(
        " {}  ·  {}  ·  {} sessions",
        event_day.label, app.schedule.event.city, session_count
    );
    if live_session_count > 0 {
        event_summary_line.push_str(&format!("  ·  {live_session_count} live"));
    }

    frame.render_widget(
        Paragraph::new(vec![
            event_title_line,
            Line::from(Span::styled(event_summary_line, Style::default().fg(DIM))),
        ]),
        area,
    );
}

fn draw_schedule_grid(
    frame: &mut Frame,
    app: &mut App,
    header_area: Rect,
    body_area: Rect,
    visible_room_count: usize,
    room_column_width: usize,
) {
    if app.room_columns.is_empty() || app.time_row_starts.is_empty() {
        frame.render_widget(
            Paragraph::new("No sessions scheduled.")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            body_area,
        );
        return;
    }

    let viewport_height = body_area.height as usize;
    app.viewport_height = viewport_height;
    app.visible_room_count = visible_room_count;

    layout::measure_time_row_line_heights(app, room_column_width);
    position_schedule_viewport(app, viewport_height, visible_room_count, room_column_width);

    draw_room_headers(
        frame,
        app,
        header_area,
        room_column_width,
        visible_room_count,
    );
    draw_time_rows(frame, app, body_area, room_column_width, visible_room_count);
}

fn draw_room_headers(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    room_column_width: usize,
    visible_room_count: usize,
) {
    let room_name_line_count = (area.height as usize).saturating_sub(1);
    let mut visible_headers = Vec::new();

    for visible_column_index in 0..visible_room_count {
        let room_column_index = app.first_visible_room_index + visible_column_index;
        if room_column_index >= app.room_columns.len() {
            break;
        }

        let room_name = compact_room(&app.room_columns[room_column_index].room_name);
        let wrapped_room_name_lines = layout::wrap_text_to_width(&room_name, room_column_width);
        let header_style = if room_column_index == app.selected_room_column() {
            Style::default()
                .fg(ACCENT)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(Color::Gray)
        };
        visible_headers.push((wrapped_room_name_lines, header_style));
    }

    let mut header_lines = Vec::with_capacity(room_name_line_count + 1);
    for room_name_line_index in 0..room_name_line_count {
        let mut room_header_spans = vec![Span::raw(" ".repeat(TIME_GUTTER_WIDTH))];
        for (wrapped_room_name_lines, header_style) in &visible_headers {
            room_header_spans.push(Span::styled("│", Style::default().fg(DIM)));
            let room_name_line = wrapped_room_name_lines
                .get(room_name_line_index)
                .map(String::as_str)
                .unwrap_or("");
            room_header_spans.push(Span::styled(
                layout::pad_text_to_width(room_name_line, room_column_width),
                *header_style,
            ));
        }
        header_lines.push(Line::from(room_header_spans));
    }

    let mut header_rule = "─".repeat(TIME_GUTTER_WIDTH);
    for _ in &visible_headers {
        header_rule.push('┼');
        header_rule.push_str(&"─".repeat(room_column_width));
    }
    header_lines.push(Line::from(Span::styled(
        header_rule,
        Style::default().fg(DIM),
    )));

    frame.render_widget(Paragraph::new(header_lines), area);
}

fn draw_time_rows(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    room_column_width: usize,
    visible_room_count: usize,
) {
    let viewport_height = area.height as usize;
    let current_time_row = app.time_row_containing_now();
    let mut rendered_grid_lines: Vec<Line> = Vec::with_capacity(viewport_height);

    let start_time_row = app.time_row_at_grid_line(app.first_visible_grid_line);
    let mut time_row = start_time_row;
    let mut line_in_time_row = app
        .first_visible_grid_line
        .saturating_sub(app.first_grid_line_of_time_row(start_time_row));
    let mut line_on_screen = 0;

    while line_on_screen < viewport_height && time_row < app.time_row_starts.len() {
        let time_row_line_height = app
            .time_row_line_heights
            .get(time_row)
            .copied()
            .unwrap_or(1)
            .max(1);

        while line_in_time_row < time_row_line_height && line_on_screen < viewport_height {
            rendered_grid_lines.push(render_time_row_line(
                app,
                time_row,
                line_in_time_row,
                room_column_width,
                visible_room_count,
                current_time_row,
            ));
            line_in_time_row += 1;
            line_on_screen += 1;
        }

        time_row += 1;
        line_in_time_row = 0;
    }

    frame.render_widget(Paragraph::new(rendered_grid_lines), area);
}

fn render_time_row_line(
    app: &App,
    time_row: usize,
    line_in_time_row: usize,
    room_column_width: usize,
    visible_room_count: usize,
    current_time_row: Option<usize>,
) -> Line<'static> {
    let mut spans = Vec::new();

    let is_current_time_row = current_time_row == Some(time_row);

    if is_current_time_row && line_in_time_row == 0 {
        spans.push(Span::styled(
            format!("▶{:>8} ", app.time_row_label(time_row)),
            Style::default().fg(LIVE).add_modifier(Modifier::BOLD),
        ));
        for visible_column_index in 0..visible_room_count {
            let room_column_index = app.first_visible_room_index + visible_column_index;
            if room_column_index >= app.room_columns.len() {
                break;
            }
            spans.push(Span::styled("│", Style::default().fg(DIM)));
            spans.push(Span::styled(
                "┈".repeat(room_column_width),
                Style::default().fg(LIVE),
            ));
        }
        return Line::from(spans);
    }

    let time_gutter_style = if is_current_time_row {
        Style::default().fg(LIVE).add_modifier(Modifier::BOLD)
    } else if current_time_row.is_some_and(|current| time_row < current) {
        Style::default().fg(DIM)
    } else {
        Style::default().fg(Color::Gray)
    };

    if line_in_time_row == 0 {
        let now_marker = if is_current_time_row { "▶" } else { " " };
        spans.push(Span::styled(
            format!("{now_marker}{:>8} ", app.time_row_label(time_row)),
            time_gutter_style,
        ));
    } else {
        spans.push(Span::raw(" ".repeat(TIME_GUTTER_WIDTH)));
    }

    let content_line_in_time_row = if is_current_time_row {
        line_in_time_row - 1
    } else {
        line_in_time_row
    };

    for visible_column_index in 0..visible_room_count {
        let room_column_index = app.first_visible_room_index + visible_column_index;
        if room_column_index >= app.room_columns.len() {
            break;
        }
        spans.push(Span::styled("│", Style::default().fg(DIM)));

        let session_index_at_time_row = app.room_columns[room_column_index]
            .session_index_by_time_row
            .get(time_row)
            .copied()
            .flatten();
        match session_index_at_time_row {
            Some(session_index) => {
                let session = &app.schedule.sessions[session_index];
                let (session_start_time_row, _) = app.time_row_span(session_index);
                let is_selected_session = session_index == app.selected_session_index;
                let session_status = session.status(app.now, app.tz);

                if time_row == session_start_time_row {
                    let wrapped_title_lines = layout::wrap_text_to_width(
                        &session.title,
                        room_column_width.saturating_sub(2),
                    );
                    let title_line_count = wrapped_title_lines.len();
                    let wrapped_subtrack_lines = session
                        .subtrack()
                        .filter(|subtrack| !subtrack.is_empty())
                        .map(|subtrack| {
                            layout::wrap_text_to_width(
                                subtrack,
                                room_column_width.saturating_sub(2),
                            )
                        })
                        .unwrap_or_default();

                    let (cell_text, cell_style) = if content_line_in_time_row < title_line_count {
                        let title_line_text = &wrapped_title_lines[content_line_in_time_row];
                        let status_prefix = if content_line_in_time_row == 0 {
                            if session_status == Status::Live {
                                "▶ "
                            } else {
                                "  "
                            }
                        } else {
                            "  "
                        };
                        let title_style = if is_selected_session {
                            Style::default()
                                .fg(Color::White)
                                .bg(SELECTED_SESSION_BACKGROUND)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            match session_status {
                                Status::Live => {
                                    Style::default().fg(LIVE).add_modifier(Modifier::BOLD)
                                }
                                Status::Upcoming => Style::default().fg(Color::White),
                                Status::Past => Style::default().fg(DIM),
                            }
                        };
                        (format!("{status_prefix}{title_line_text}"), title_style)
                    } else if let Some(subtrack_line) =
                        wrapped_subtrack_lines.get(content_line_in_time_row - title_line_count)
                    {
                        let subtrack_style = if is_selected_session {
                            Style::default()
                                .fg(SELECTED_SUBTRACK)
                                .bg(SELECTED_SESSION_BACKGROUND)
                        } else if session_status == Status::Past {
                            Style::default().fg(FAINT)
                        } else {
                            Style::default().fg(SUBTRACK)
                        };
                        (format!("  {subtrack_line}"), subtrack_style)
                    } else {
                        let blank_style = if is_selected_session {
                            Style::default().bg(SELECTED_SESSION_BACKGROUND)
                        } else {
                            Style::default()
                        };
                        ("  ".to_string(), blank_style)
                    };
                    spans.push(Span::styled(
                        layout::pad_text_to_width(&cell_text, room_column_width),
                        cell_style,
                    ));
                } else {
                    let continuation_style = if is_selected_session {
                        Style::default()
                            .fg(Color::White)
                            .bg(SELECTED_SESSION_BACKGROUND)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(FAINT)
                    };
                    spans.push(Span::styled(
                        layout::pad_text_to_width(" ┆", room_column_width),
                        continuation_style,
                    ));
                }
            }
            None => spans.push(Span::raw(" ".repeat(room_column_width))),
        }
    }

    Line::from(spans)
}

fn position_schedule_viewport(
    app: &mut App,
    viewport_height: usize,
    visible_room_count: usize,
    room_column_width: usize,
) {
    if app.time_row_starts.is_empty()
        || app.room_columns.is_empty()
        || viewport_height == 0
        || visible_room_count == 0
    {
        return;
    }

    let (selected_first_grid_line, selected_end_grid_line) =
        app.grid_line_span(app.selected_session_index);
    let selected_room_column = app.selected_room_column();
    let keep_bottom_margin = std::mem::take(&mut app.pending_selected_session_bottom_margin);

    if let Some(view_anchor) = app.pending_view_anchor.take() {
        let anchor_grid_line = match view_anchor {
            ViewAnchor::CurrentTime => app
                .time_row_containing_now()
                .map(|time_row| app.first_grid_line_of_time_row(time_row))
                .unwrap_or(selected_first_grid_line),
            ViewAnchor::SelectedSession => selected_first_grid_line,
        };
        app.first_visible_grid_line = anchor_grid_line.saturating_sub(viewport_height / 3);
        app.first_visible_room_index = selected_room_column.saturating_sub(visible_room_count / 2);
    }

    // Long sessions keep the viewport in place while any part of their span
    // is still visible; only scroll when they are fully above/below.
    if selected_end_grid_line <= app.first_visible_grid_line {
        app.first_visible_grid_line = selected_first_grid_line;
    } else if selected_first_grid_line >= app.first_visible_grid_line + viewport_height {
        app.first_visible_grid_line = selected_first_grid_line + 1 - viewport_height;
    }

    if keep_bottom_margin {
        let selected_content_line_count = app
            .selected_session()
            .map(|session| {
                let session_start_time_row = app.time_row_for_start(&session.start);
                let now_indicator_line_count =
                    usize::from(app.time_row_containing_now() == Some(session_start_time_row));
                layout::session_content_line_count(session, room_column_width)
                    + now_indicator_line_count
            })
            .unwrap_or(1);
        // Keep about a third of the viewport below the selection, but never
        // less than the selected title's height.
        let bottom_margin = (viewport_height / 3)
            .max(selected_content_line_count)
            .min(viewport_height);
        let selected_bottom_grid_line = selected_first_grid_line + bottom_margin;
        let viewport_bottom_grid_line = app.first_visible_grid_line + viewport_height;
        if selected_bottom_grid_line > viewport_bottom_grid_line {
            app.first_visible_grid_line = selected_bottom_grid_line - viewport_height;
        }
    }

    let total_grid_lines = app.grid_line_count();
    app.first_visible_grid_line = app
        .first_visible_grid_line
        .min(total_grid_lines.saturating_sub(viewport_height));

    if selected_room_column < app.first_visible_room_index {
        app.first_visible_room_index = selected_room_column;
    } else if selected_room_column >= app.first_visible_room_index + visible_room_count {
        app.first_visible_room_index = selected_room_column + 1 - visible_room_count;
    }
    app.first_visible_room_index = app
        .first_visible_room_index
        .min(app.room_columns.len().saturating_sub(visible_room_count));
}

fn draw_key_help_footer(frame: &mut Frame, app: &App, area: Rect) {
    let key_help_text =
        " h/l room · j/k session · g/G · Ctrl-d/u · Enter details · t now · q quit ";
    let room_range_text = if app.room_columns.is_empty() {
        String::new()
    } else {
        let first_visible_room_number = app.first_visible_room_index + 1;
        let last_visible_room_number =
            (app.first_visible_room_index + app.visible_room_count).min(app.room_columns.len());
        format!(
            "rooms {first_visible_room_number}-{last_visible_room_number}/{} ",
            app.room_columns.len()
        )
    };
    let gap_width = (area.width as usize).saturating_sub(
        layout::text_display_width(key_help_text) + layout::text_display_width(&room_range_text),
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(key_help_text, Style::default().fg(DIM)),
            Span::raw(" ".repeat(gap_width)),
            Span::styled(room_range_text, Style::default().fg(DIM)),
        ])),
        area,
    );
}

fn draw_session_detail(frame: &mut Frame, app: &mut App) {
    let screen_area = frame.area();
    let panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(screen_area);

    let detail_border = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            " Session details ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let detail_text_area = detail_border.inner(panes[0]);
    frame.render_widget(detail_border, panes[0]);

    let Some(session) = app.selected_session() else {
        frame.render_widget(
            Paragraph::new("No session selected.").style(Style::default().fg(DIM)),
            detail_text_area,
        );
        return;
    };

    if detail_text_area.width == 0 || detail_text_area.height == 0 {
        return;
    }

    let rendered_detail_lines = session_detail_lines(app, session);
    // One extra line of slack: our wrap estimate is approximate, and it is
    // better to allow scrolling a line too far than to hide the last line.
    let estimated_detail_line_count =
        estimated_wrapped_line_count(&rendered_detail_lines, detail_text_area.width as usize) + 1;
    let max_detail_scroll_line = estimated_detail_line_count
        .saturating_sub(detail_text_area.height as usize)
        .min(u16::MAX as usize);
    app.session_detail_max_scroll_line = max_detail_scroll_line;
    app.session_detail_scroll_line = app
        .session_detail_scroll_line
        .min(max_detail_scroll_line as u16);
    let detail_paragraph = Paragraph::new(rendered_detail_lines).wrap(Wrap { trim: false });
    frame.render_widget(
        detail_paragraph.scroll((app.session_detail_scroll_line, 0)),
        detail_text_area,
    );

    frame.render_widget(
        Paragraph::new(" Esc/h back · j/k scroll · Ctrl-d/u page · g/G top/bottom ")
            .style(Style::default().fg(DIM))
            .alignment(Alignment::Center),
        panes[1],
    );
}

fn session_detail_lines(app: &App, session: &Session) -> Vec<Line<'static>> {
    let mut rendered_lines = Vec::new();

    rendered_lines.push(Line::from(Span::styled(
        session.title.clone(),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    rendered_lines.push(Line::from(Span::styled(
        session_status_text(app, session),
        session_status_style(app, session),
    )));
    rendered_lines.push(Line::default());

    let event_day_label = &app.schedule.days[app.day_index].label;
    let timezone_abbreviation = session
        .start_at(app.tz)
        .map(|start_time| start_time.format("%Z").to_string())
        .unwrap_or_default();
    let session_time_range = format!(
        "{} · {}–{} · {} min ({})",
        event_day_label,
        session.start_hm(app.tz),
        session.end_hm(app.tz),
        session.duration_minutes(app.tz),
        timezone_abbreviation
    );
    rendered_lines.push(detail_field_row("When", session_time_range));
    rendered_lines.push(detail_field_row("Room", session.room.clone()));

    if let Some(track) = session.track() {
        rendered_lines.push(detail_field_row("Track", track.to_string()));
    }
    if let Some(level) = session.level() {
        rendered_lines.push(detail_field_row("Level", level.to_string()));
    }
    let projects = session.projects();
    if !projects.is_empty() {
        rendered_lines.push(detail_field_row("Projects", projects.join(", ")));
    }
    if let Some(subtrack) = session.subtrack() {
        rendered_lines.push(detail_field_row("Subtrack", subtrack.to_string()));
    }
    let tags = session.tags();
    if !tags.is_empty() {
        rendered_lines.push(detail_field_row("Tags", tags.join(", ")));
    }

    rendered_lines.push(Line::default());
    rendered_lines.push(detail_section_heading("About"));
    match session.description.as_deref() {
        Some(description) if !description.trim().is_empty() => {
            push_wrapped_paragraph(
                &mut rendered_lines,
                description,
                Style::default().fg(Color::Gray),
            );
        }
        _ => rendered_lines.push(Line::from(Span::styled(
            "No description.",
            Style::default().fg(DIM),
        ))),
    }

    let session_speakers: Vec<&Speaker> = session
        .speakers
        .iter()
        .filter_map(|speaker_id| app.schedule.speakers.get(speaker_id))
        .collect();
    if !session_speakers.is_empty() {
        rendered_lines.push(Line::default());
        rendered_lines.push(detail_section_heading("Speakers"));
        for (speaker_index, speaker) in session_speakers.iter().enumerate() {
            let mut speaker_header_spans = vec![Span::styled(
                speaker.name.clone(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )];
            if let Some(tagline) = &speaker.tagline {
                speaker_header_spans.push(Span::styled(
                    format!("  {tagline}"),
                    Style::default().fg(DIM),
                ));
            }
            rendered_lines.push(Line::from(speaker_header_spans));

            for link in &speaker.links {
                let link_label = link.title.as_deref().unwrap_or("Link");
                rendered_lines.push(Line::from(Span::styled(
                    format!("  {link_label}: {}", link.url),
                    Style::default().fg(ACCENT),
                )));
            }
            if let Some(bio) = &speaker.bio {
                push_wrapped_paragraph(&mut rendered_lines, bio, Style::default().fg(Color::Gray));
            }
            if speaker_index + 1 < session_speakers.len() {
                rendered_lines.push(Line::default());
            }
        }
    }

    rendered_lines
}

fn session_status_text(app: &App, session: &Session) -> String {
    match session.status(app.now, app.tz) {
        Status::Live => match session.end_at(app.tz) {
            Some(end_time) => format!("● LIVE · ends in {}", format_duration(end_time - app.now)),
            None => "● LIVE".to_string(),
        },
        Status::Upcoming => match session.start_at(app.tz) {
            Some(start_time) => format!("Starts in {}", format_duration(start_time - app.now)),
            None => "Upcoming".to_string(),
        },
        Status::Past => match session.end_at(app.tz) {
            Some(end_time) => {
                format!("Ended {} ago", format_duration(app.now - end_time))
            }
            None => "Ended".to_string(),
        },
    }
}

fn session_status_style(app: &App, session: &Session) -> Style {
    match session.status(app.now, app.tz) {
        Status::Live => Style::default().fg(LIVE).add_modifier(Modifier::BOLD),
        Status::Upcoming => Style::default().fg(Color::Yellow),
        Status::Past => Style::default().fg(DIM),
    }
}

fn format_duration(delta: chrono::Duration) -> String {
    let delta_minutes = delta.num_minutes().max(0);
    if delta_minutes == 0 {
        "<1m".to_string()
    } else if delta_minutes < 60 {
        format!("{delta_minutes}m")
    } else if delta_minutes < 60 * 24 {
        format!("{}h {:02}m", delta_minutes / 60, delta_minutes % 60)
    } else {
        format!(
            "{}d {}h",
            delta_minutes / (60 * 24),
            (delta_minutes % (60 * 24)) / 60
        )
    }
}

fn detail_field_row(field_label: &str, field_value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{field_label:<10}"),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        ),
        Span::styled(field_value, Style::default().fg(Color::Gray)),
    ])
}

fn detail_section_heading(section_heading: &str) -> Line<'static> {
    Line::from(Span::styled(
        section_heading.to_string(),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
}

fn push_wrapped_paragraph(
    rendered_lines: &mut Vec<Line<'static>>,
    paragraph_text: &str,
    paragraph_style: Style,
) {
    for paragraph_line in paragraph_text.lines() {
        rendered_lines.push(Line::from(Span::styled(
            paragraph_line.trim_end().to_string(),
            paragraph_style,
        )));
    }
}

/// Approximate the number of terminal rows used by wrapped plain text, so the
/// detail pane can clamp its scroll offset. Styling does not affect wrapping.
fn estimated_wrapped_line_count(lines: &[Line<'static>], width: usize) -> usize {
    let width = width.max(1);
    lines
        .iter()
        .map(|line| {
            let plain_text: String = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            let word_wrapped_line_count = layout::wrap_text_to_width(&plain_text, width).len();
            let dense_line_count = layout::text_display_width(&plain_text)
                .div_ceil(width)
                .max(1);
            word_wrapped_line_count.max(dense_line_count)
        })
        .sum::<usize>()
        .max(1)
}

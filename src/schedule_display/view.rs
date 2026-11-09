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
use crate::session_data::{Session, Speaker, Status};

const ACCENT: Color = Color::Rgb(97, 175, 239);
const LIVE: Color = Color::Rgb(152, 195, 121);
const ROOM: Color = Color::Rgb(198, 160, 246);
const DIM: Color = Color::Rgb(92, 99, 112);
const SELECT_BG: Color = Color::Rgb(38, 79, 120);

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
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, app, chunks[0]);
    draw_list(frame, app, chunks[1]);
    draw_footer(frame, chunks[2]);
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
    let live_count = app
        .day_indices
        .iter()
        .filter(|&&index| app.data.sessions[index].status(app.now, app.tz) == Status::Live)
        .count();
    let mut line2 = format!(
        " {}  ·  {}  ·  {} sessions",
        day.label,
        app.data.event.city,
        app.day_indices.len()
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

fn draw_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM))
        .title(Span::styled(
            " Schedule ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total = app.day_indices.len();
    if total == 0 {
        frame.render_widget(
            Paragraph::new("No sessions scheduled.")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }

    let height = inner.height as usize;
    app.list_height = height;
    if height == 0 || inner.width == 0 {
        return;
    }
    update_scroll(app, height, total);

    let end = (app.scroll + height).min(total);
    let rows: Vec<Line> = (app.scroll..end)
        .map(|position| session_row(app, position, inner.width as usize))
        .collect();
    frame.render_widget(Paragraph::new(rows), inner);
}

/// Keep the selected row visible. After `focus_now` (launch or `t`) place it
/// roughly one third from the top so the current time has context above it.
fn update_scroll(app: &mut App, height: usize, total: usize) {
    if app.selected >= total {
        app.selected = total - 1;
    }

    if app.center_selection {
        app.scroll = app.selected.saturating_sub(height / 3);
        app.center_selection = false;
    }

    if app.selected < app.scroll {
        app.scroll = app.selected;
    } else if app.selected >= app.scroll + height {
        app.scroll = app.selected + 1 - height;
    }

    app.scroll = app.scroll.min(total.saturating_sub(height));
}

fn session_row(app: &App, position: usize, width: usize) -> Line<'static> {
    // Keep one blank column between the content and the right border.
    let width = width.saturating_sub(1);
    let session = &app.data.sessions[app.day_indices[position]];
    let status = session.status(app.now, app.tz);
    let selected = position == app.selected;

    let prefix = if selected { "▌ " } else { "  " };
    let prefix_style = if selected {
        Style::default().fg(ACCENT)
    } else {
        Style::default()
    };

    let time = format!("{}-{}", session.start_hm(app.tz), session.end_hm(app.tz));
    let (glyph, time_style, title_style) = match status {
        Status::Live => (
            "▶ ",
            Style::default().fg(LIVE),
            Style::default().fg(LIVE).add_modifier(Modifier::BOLD),
        ),
        Status::Upcoming => (
            "  ",
            Style::default().fg(Color::Gray),
            Style::default().fg(Color::White),
        ),
        Status::Past => ("  ", Style::default().fg(DIM), Style::default().fg(DIM)),
    };
    let room_style = if status == Status::Past {
        Style::default().fg(DIM)
    } else {
        Style::default().fg(ROOM)
    };

    // prefix (2) + time (11) + space (1) + glyph (2), then title, gap, room.
    let fixed = 2 + 11 + 1 + 2;
    let available = width.saturating_sub(fixed + 1);
    let room_width = (available / 3).clamp(8, 30).min(available);
    let title_width = width.saturating_sub(fixed + room_width + 1).max(1);

    let title = truncate(&session.title, title_width);
    let room = truncate(&session.short_room(), room_width);

    let mut line = Line::from(vec![
        Span::styled(prefix, prefix_style),
        Span::styled(format!("{time} "), time_style),
        Span::styled(glyph, Style::default().fg(LIVE)),
        Span::styled(pad_right(&title, title_width), title_style),
        Span::raw(" "),
        Span::styled(pad_left(&room, room_width), room_style),
    ]);

    if selected {
        line = line.style(Style::default().bg(SELECT_BG).add_modifier(Modifier::BOLD));
    }
    line
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let hints = " j/k move · g/G top/bottom · Ctrl-d/u page · Enter details · t now · q quit ";
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(hints, Style::default().fg(DIM)))),
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
            format!("{label:<9}"),
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

fn pad_left(text: &str, width: usize) -> String {
    format!(
        "{}{text}",
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

//! Display state for the room-column grid: the selected session, visible
//! offsets, and time-aware focus. No rendering and no terminal I/O here.

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::session_data::{Session, SessionData, Status};

pub struct Column {
    /// Full room name from `Day::rooms`, in website grid order.
    pub name: String,
    /// Global session indices in this room, in start order.
    pub sessions: Vec<usize>,
    /// Row -> session occupying that cell (for long sessions, every row of
    /// their span points at them).
    pub cells: Vec<Option<usize>>,
}

pub struct App {
    pub data: SessionData,
    pub tz: Tz,
    pub now: DateTime<Tz>,
    pub day_index: usize,
    /// Unique session start timestamps for the day; the grid's time rows.
    pub rows: Vec<String>,
    pub columns: Vec<Column>,
    /// Global index into `data.sessions`.
    pub selected: usize,
    pub row_offset: usize,
    pub col_offset: usize,
    /// Set after `t`/launch so the view centers the selection.
    pub center_selection: bool,
    pub view_rows: usize,
    pub view_cols: usize,
    pub detail_open: bool,
    pub detail_scroll: u16,
    pub detail_total: usize,
    pub should_quit: bool,
}

impl App {
    pub fn new(data: SessionData) -> Self {
        let tz = data.timezone();
        let now = Utc::now().with_timezone(&tz);
        let day_index = choose_day(&data, now.date_naive());

        let mut app = Self {
            data,
            tz,
            now,
            day_index,
            rows: Vec::new(),
            columns: Vec::new(),
            selected: 0,
            row_offset: 0,
            col_offset: 0,
            center_selection: false,
            view_rows: 20,
            view_cols: 5,
            detail_open: false,
            detail_scroll: 0,
            detail_total: 0,
            should_quit: false,
        };
        app.rebuild_grid();
        app.focus_now();
        app
    }

    pub fn session(&self) -> Option<&Session> {
        self.data.sessions.get(self.selected)
    }

    pub fn selected_col(&self) -> usize {
        self.session().map(|session| session.col).unwrap_or(0)
    }

    pub fn selected_row(&self) -> usize {
        self.session()
            .map(|session| self.row_of_start(&session.start))
            .unwrap_or(0)
    }

    pub fn row_of_start(&self, start: &str) -> usize {
        self.rows
            .partition_point(|row| row.as_str() < start)
            .min(self.rows.len().saturating_sub(1))
    }

    /// Inclusive start row, exclusive end row of a session's grid span.
    pub fn session_span(&self, index: usize) -> (usize, usize) {
        if self.rows.is_empty() {
            return (0, 0);
        }
        let Some(session) = self.data.sessions.get(index) else {
            return (0, 0);
        };
        let start = self.row_of_start(&session.start);
        let end = self
            .rows
            .partition_point(|row| row.as_str() < session.end.as_str())
            .max(start + 1)
            .min(self.rows.len());
        (start, end)
    }

    pub fn row_label(&self, row: usize) -> &str {
        self.rows
            .get(row)
            .map(|start| {
                if start.len() >= 16 {
                    &start[11..16]
                } else {
                    start.as_str()
                }
            })
            .unwrap_or("--:--")
    }

    /// The time row containing "now", if the day has started.
    pub fn current_row(&self) -> Option<usize> {
        let now = self.now.format("%Y-%m-%dT%H:%M:%S").to_string();
        self.rows
            .partition_point(|row| row.as_str() <= now.as_str())
            .checked_sub(1)
    }

    /// Build rows from session starts, then columns in website grid order,
    /// then mark every cell covered by each session.
    fn rebuild_grid(&mut self) {
        let rooms = self.data.days[self.day_index].rooms.clone();
        let indices = self.data.sessions_on(&self.data.days[self.day_index]);

        let mut starts: Vec<String> = indices
            .iter()
            .map(|&index| self.data.sessions[index].start.clone())
            .collect();
        starts.sort();
        starts.dedup();
        self.rows = starts;

        self.columns = rooms
            .into_iter()
            .map(|name| Column {
                name,
                sessions: Vec::new(),
                cells: vec![None; self.rows.len()],
            })
            .collect();

        for &index in &indices {
            let col = self.data.sessions[index].col;
            if let Some(column) = self.columns.get_mut(col) {
                column.sessions.push(index);
            }
        }

        let row_count = self.rows.len();
        for column in &mut self.columns {
            let sessions = column.sessions.clone();
            for index in sessions {
                let session = &self.data.sessions[index];
                let start = self
                    .rows
                    .partition_point(|row| row.as_str() < session.start.as_str());
                let end = self
                    .rows
                    .partition_point(|row| row.as_str() < session.end.as_str())
                    .max(start + 1)
                    .min(row_count);
                if start < row_count {
                    for cell in &mut column.cells[start..end] {
                        *cell = Some(index);
                    }
                }
            }
        }
    }

    /// Focus the live session that started most recently (so all-day
    /// registration does not win over an actual talk), else the next one,
    /// else the last session of the day.
    pub fn focus_now(&mut self) {
        self.now = Utc::now().with_timezone(&self.tz);
        self.refresh_day();

        let now = self.now;
        let tz = self.tz;
        let mut live: Option<(usize, DateTime<Tz>)> = None;
        let mut next: Option<(usize, DateTime<Tz>)> = None;

        for column in &self.columns {
            for &index in &column.sessions {
                let session = &self.data.sessions[index];
                match session.status(now, tz) {
                    Status::Live => {
                        if let Some(start) = session.start_at(tz) {
                            if live.map_or(true, |(_, best)| start > best) {
                                live = Some((index, start));
                            }
                        }
                    }
                    Status::Upcoming => {
                        if let Some(start) = session.start_at(tz) {
                            if next.map_or(true, |(_, best)| start < best) {
                                next = Some((index, start));
                            }
                        }
                    }
                    Status::Past => {}
                }
            }
        }

        if let Some((index, _)) = live.or(next) {
            self.selected = index;
        } else if let Some(index) = self
            .columns
            .iter()
            .rev()
            .find(|column| !column.sessions.is_empty())
            .and_then(|column| column.sessions.last())
        {
            self.selected = *index;
        }

        self.center_selection = true;
        self.detail_scroll = 0;
    }

    pub fn tick(&mut self) {
        let now = Utc::now().with_timezone(&self.tz);
        let date_changed = now.date_naive() != self.now.date_naive();
        self.now = now;

        if date_changed && self.refresh_day() {
            self.focus_now();
        }
    }

    fn refresh_day(&mut self) -> bool {
        let today = self.now.date_naive().format("%Y-%m-%d").to_string();
        if self
            .data
            .days
            .get(self.day_index)
            .is_some_and(|day| day.date == today)
        {
            return false;
        }

        let Some(index) = self.data.days.iter().position(|day| day.date == today) else {
            return false;
        };

        self.day_index = index;
        self.selected = 0;
        self.row_offset = 0;
        self.col_offset = 0;
        self.detail_open = false;
        self.rebuild_grid();
        true
    }

    // ------------------------------------------------------------------
    // navigation
    // ------------------------------------------------------------------

    /// j/k: next/previous session in the current room column.
    pub fn move_session(&mut self, delta: isize) {
        let col = self.selected_col();
        let sessions = match self.columns.get(col) {
            Some(column) if !column.sessions.is_empty() => column.sessions.clone(),
            _ => return,
        };
        let position = sessions
            .iter()
            .position(|&index| index == self.selected)
            .unwrap_or(0) as isize;
        let next = (position + delta).clamp(0, sessions.len() as isize - 1) as usize;
        self.selected = sessions[next];
        self.detail_scroll = 0;
    }

    /// h/l: adjacent room column, staying near the same point in time.
    pub fn move_column(&mut self, delta: isize) {
        if self.columns.is_empty() {
            return;
        }
        let current = self.selected_col();
        let target = (current as isize + delta).clamp(0, self.columns.len() as isize - 1) as usize;
        if target == current {
            return;
        }
        let time = self.column_anchor_time();
        if let Some(index) = self.nearest_session(target, &time) {
            self.selected = index;
            self.detail_scroll = 0;
        }
    }

    /// Time used when moving horizontally. A short session anchors on its own
    /// start; a long/all-day session anchors on the middle of the viewport so
    /// changing rooms keeps the user near the time they were looking at.
    fn column_anchor_time(&self) -> String {
        if self.rows.is_empty() {
            return String::new();
        }
        let (start, end) = self.session_span(self.selected);
        let anchor = if end.saturating_sub(start) > (self.view_rows / 2).max(1) {
            (self.row_offset + self.view_rows / 2).min(self.rows.len() - 1)
        } else {
            start
        };
        self.rows[anchor].clone()
    }

    /// The session in `col` that is ongoing at `time`, else the next one,
    /// else the last one.
    fn nearest_session(&self, col: usize, time: &str) -> Option<usize> {
        let sessions = &self.columns.get(col)?.sessions;
        if sessions.is_empty() {
            return None;
        }

        let position =
            sessions.partition_point(|&index| self.data.sessions[index].start.as_str() <= time);
        for &index in sessions[..position].iter().rev() {
            if self.data.sessions[index].end.as_str() > time {
                return Some(index);
            }
        }
        if let Some(&index) = sessions.get(position) {
            return Some(index);
        }
        sessions.last().copied()
    }

    /// Ctrl-d/Ctrl-u: move the time window by half a page, keeping the room.
    pub fn page(&mut self, direction: isize) {
        if self.rows.is_empty() {
            return;
        }
        let step = (self.view_rows / 2).max(1) as isize;
        let target = (self.selected_row() as isize + direction * step)
            .clamp(0, self.rows.len() as isize - 1) as usize;
        let time = self.rows[target].clone();
        let col = self.selected_col();
        if let Some(index) = self.nearest_session(col, &time) {
            self.selected = index;
            self.detail_scroll = 0;
        }
    }

    pub fn select_first(&mut self) {
        let col = self.selected_col();
        if let Some(&index) = self
            .columns
            .get(col)
            .and_then(|column| column.sessions.first())
        {
            self.selected = index;
            self.center_selection = true;
            self.detail_scroll = 0;
        }
    }

    pub fn select_last(&mut self) {
        let col = self.selected_col();
        if let Some(&index) = self
            .columns
            .get(col)
            .and_then(|column| column.sessions.last())
        {
            self.selected = index;
            self.center_selection = true;
            self.detail_scroll = 0;
        }
    }

    pub fn open_detail(&mut self) {
        if self.session().is_some() {
            self.detail_open = true;
            self.detail_scroll = 0;
        }
    }

    pub fn scroll_detail(&mut self, delta: isize) {
        let max = self.detail_total.saturating_sub(1).min(u16::MAX as usize) as isize;
        self.detail_scroll = (self.detail_scroll as isize + delta).clamp(0, max) as u16;
    }
}

/// Today if it is an event day, otherwise the most recent event day that has
/// started, otherwise the first day.
fn choose_day(data: &SessionData, today: chrono::NaiveDate) -> usize {
    let today = today.format("%Y-%m-%d").to_string();
    let mut best = 0;
    for (index, day) in data.days.iter().enumerate() {
        if day.date <= today {
            best = index;
        } else {
            break;
        }
    }
    best
}

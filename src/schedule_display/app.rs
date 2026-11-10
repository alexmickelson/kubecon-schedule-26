//! Display state for the room-column grid: the focused cell, visible offsets,
//! and time-aware focus. No rendering and no terminal I/O here.

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::session_data::{compact_room, Session, SessionData, Status};

pub struct RoomColumn {
    pub room_name: String,
    /// Indices into `schedule.sessions`, in start order.
    pub session_indices_by_start: Vec<usize>,
    /// Every time row covered by a long session points at that session.
    pub session_index_by_time_row: Vec<Option<usize>>,
}

pub struct SessionSearch {
    pub query: String,
    pub matches: Vec<usize>,
    pub selected_match: usize,
    /// Room column and time row to return to when search is cancelled.
    focus_before_search: (usize, usize),
}

#[derive(Clone, Copy)]
pub enum ViewAnchor {
    /// Scroll so the current time is near the top third.
    CurrentTime,
    /// Scroll so the focused time row is near the top third.
    FocusedRow,
}

pub struct App {
    pub schedule: SessionData,
    pub tz: Tz,
    pub now: DateTime<Tz>,
    pub day_index: usize,
    pub time_row_starts: Vec<String>,
    pub room_columns: Vec<RoomColumn>,
    /// The room column and time row under the grid cursor.
    pub focused_room_column: usize,
    pub focused_time_row: usize,
    pub time_row_line_heights: Vec<usize>,
    /// Prefix sum of `time_row_line_heights`; one longer than the heights.
    pub time_row_line_offsets: Vec<usize>,
    pub measured_room_column_width: Option<usize>,
    /// Current time row the cached height measurements were made for.
    pub measured_current_time_row: Option<usize>,
    pub first_visible_grid_line: usize,
    pub first_visible_room_index: usize,
    /// Consumed by the view to center the current time or the focused row.
    pub pending_view_anchor: Option<ViewAnchor>,
    /// Set when moving down so the viewport keeps room below the focused row.
    pub pending_focus_bottom_margin: bool,
    pub search: Option<SessionSearch>,
    pub viewport_height: usize,
    pub visible_room_count: usize,
    pub session_detail_open: bool,
    pub session_detail_scroll_line: u16,
    /// Set by the view after measuring the rendered detail lines.
    pub session_detail_max_scroll_line: usize,
    pub quit_requested: bool,
}

impl App {
    pub fn new(schedule: SessionData) -> Self {
        let tz = schedule.timezone();
        let now = Utc::now().with_timezone(&tz);
        let day_index = event_day_for_date(&schedule, now.date_naive());

        let mut app = Self {
            schedule,
            tz,
            now,
            day_index,
            time_row_starts: Vec::new(),
            room_columns: Vec::new(),
            focused_room_column: 0,
            focused_time_row: 0,
            time_row_line_heights: Vec::new(),
            time_row_line_offsets: vec![0],
            measured_room_column_width: None,
            measured_current_time_row: None,
            first_visible_grid_line: 0,
            first_visible_room_index: 0,
            pending_view_anchor: None,
            pending_focus_bottom_margin: false,
            search: None,
            viewport_height: 20,
            visible_room_count: 5,
            session_detail_open: false,
            session_detail_scroll_line: 0,
            session_detail_max_scroll_line: 0,
            quit_requested: false,
        };
        app.build_room_columns();
        app.select_current_or_next_session();
        app
    }

    pub fn focused_session_index(&self) -> Option<usize> {
        self.session_index_at(self.focused_room_column, self.focused_time_row)
    }

    pub fn focused_session(&self) -> Option<&Session> {
        self.focused_session_index()
            .and_then(|session_index| self.schedule.sessions.get(session_index))
    }

    /// Put the cursor on a session's room and start time.
    pub fn focus_session(&mut self, session_index: usize) {
        let Some(session) = self.schedule.sessions.get(session_index) else {
            return;
        };
        self.focused_room_column = session.room_column;
        self.focused_time_row = self.time_row_for_start(&session.start);
    }

    fn session_index_at(&self, room_column_index: usize, time_row: usize) -> Option<usize> {
        self.room_columns
            .get(room_column_index)?
            .session_index_by_time_row
            .get(time_row)
            .copied()
            .flatten()
    }

    pub fn time_row_for_start(&self, start: &str) -> usize {
        self.time_row_starts
            .partition_point(|time_row_start| time_row_start.as_str() < start)
            .min(self.time_row_starts.len().saturating_sub(1))
    }

    /// Inclusive first time row, exclusive end time row of a session span.
    pub fn time_row_span(&self, session_index: usize) -> (usize, usize) {
        if self.time_row_starts.is_empty() {
            return (0, 0);
        }
        let Some(session) = self.schedule.sessions.get(session_index) else {
            return (0, 0);
        };
        let first_time_row = self.time_row_for_start(&session.start);
        let end_time_row = self
            .time_row_starts
            .partition_point(|time_row_start| time_row_start.as_str() < session.end.as_str())
            .max(first_time_row + 1)
            .min(self.time_row_starts.len());
        (first_time_row, end_time_row)
    }

    /// Total number of rendered grid lines across every time row.
    pub fn grid_line_count(&self) -> usize {
        self.time_row_line_offsets.last().copied().unwrap_or(0)
    }

    pub fn first_grid_line_of_time_row(&self, time_row: usize) -> usize {
        self.time_row_line_offsets
            .get(time_row)
            .copied()
            .unwrap_or_else(|| self.grid_line_count())
    }

    pub fn time_row_at_grid_line(&self, grid_line: usize) -> usize {
        if self.time_row_starts.is_empty() {
            return 0;
        }
        self.time_row_line_offsets
            .partition_point(|&offset| offset <= grid_line)
            .saturating_sub(1)
            .min(self.time_row_starts.len() - 1)
    }

    pub fn time_row_label(&self, time_row: usize) -> String {
        let Some(start) = self.time_row_starts.get(time_row) else {
            return "--:--".to_string();
        };
        chrono::NaiveDateTime::parse_from_str(start, "%Y-%m-%dT%H:%M:%S")
            .map(|local_time| local_time.format("%-I:%M %p").to_string())
            .unwrap_or_else(|_| "--:--".to_string())
    }

    /// The time row containing "now", if the day has started.
    pub fn time_row_containing_now(&self) -> Option<usize> {
        let now = self.now.format("%Y-%m-%dT%H:%M:%S").to_string();
        self.time_row_starts
            .partition_point(|time_row_start| time_row_start.as_str() <= now.as_str())
            .checked_sub(1)
    }

    /// Build time rows from session starts, then room columns in website grid
    /// order, then mark every time row covered by each session.
    fn build_room_columns(&mut self) {
        let room_names = self.schedule.days[self.day_index].rooms.clone();
        let session_indices = self
            .schedule
            .session_indices_on(&self.schedule.days[self.day_index]);

        let mut time_row_starts: Vec<String> = session_indices
            .iter()
            .map(|&session_index| self.schedule.sessions[session_index].start.clone())
            .collect();
        time_row_starts.sort();
        time_row_starts.dedup();
        self.time_row_starts = time_row_starts;

        self.room_columns = room_names
            .into_iter()
            .map(|room_name| RoomColumn {
                room_name,
                session_indices_by_start: Vec::new(),
                session_index_by_time_row: vec![None; self.time_row_starts.len()],
            })
            .collect();

        for &session_index in &session_indices {
            let room_column = self.schedule.sessions[session_index].room_column;
            if let Some(room_column) = self.room_columns.get_mut(room_column) {
                room_column.session_indices_by_start.push(session_index);
            }
        }

        let time_row_count = self.time_row_starts.len();
        for room_column in &mut self.room_columns {
            let session_indices = room_column.session_indices_by_start.clone();
            for session_index in session_indices {
                let session = &self.schedule.sessions[session_index];
                let first_time_row = self.time_row_starts.partition_point(|time_row_start| {
                    time_row_start.as_str() < session.start.as_str()
                });
                let end_time_row = self
                    .time_row_starts
                    .partition_point(|time_row_start| {
                        time_row_start.as_str() < session.end.as_str()
                    })
                    .max(first_time_row + 1)
                    .min(time_row_count);
                if first_time_row < time_row_count {
                    for occupant in
                        &mut room_column.session_index_by_time_row[first_time_row..end_time_row]
                    {
                        *occupant = Some(session_index);
                    }
                }
            }
        }

        self.reset_time_row_measurements();
    }

    fn reset_time_row_measurements(&mut self) {
        self.time_row_line_heights = vec![1; self.time_row_starts.len()];
        self.time_row_line_offsets = (0..=self.time_row_starts.len()).collect();
        self.measured_room_column_width = None;
        self.measured_current_time_row = None;
        self.first_visible_grid_line = 0;
    }

    /// Focus the live session that started most recently (so all-day
    /// registration does not win over an actual talk), else the next one,
    /// else the last session of the day.
    pub fn select_current_or_next_session(&mut self) {
        self.now = Utc::now().with_timezone(&self.tz);
        self.follow_current_date();

        let current_time = self.now;
        let timezone = self.tz;
        let mut live_session: Option<(usize, DateTime<Tz>)> = None;
        let mut next_session: Option<(usize, DateTime<Tz>)> = None;

        for room_column in &self.room_columns {
            for &session_index in &room_column.session_indices_by_start {
                let session = &self.schedule.sessions[session_index];
                match session.status(current_time, timezone) {
                    Status::Live => {
                        if let Some(start) = session.start_at(timezone) {
                            if live_session.map_or(true, |(_, latest_start)| start > latest_start) {
                                live_session = Some((session_index, start));
                            }
                        }
                    }
                    Status::Upcoming => {
                        if let Some(start) = session.start_at(timezone) {
                            if next_session
                                .map_or(true, |(_, earliest_start)| start < earliest_start)
                            {
                                next_session = Some((session_index, start));
                            }
                        }
                    }
                    Status::Past => {}
                }
            }
        }

        let fallback_session_index = self
            .room_columns
            .iter()
            .rev()
            .find(|room_column| !room_column.session_indices_by_start.is_empty())
            .and_then(|room_column| room_column.session_indices_by_start.last())
            .copied();
        if let Some(session_index) = live_session
            .or(next_session)
            .map(|(session_index, _)| session_index)
            .or(fallback_session_index)
        {
            self.focus_session(session_index);
        }

        self.pending_view_anchor = Some(ViewAnchor::CurrentTime);
        self.pending_focus_bottom_margin = false;
        self.session_detail_scroll_line = 0;
    }

    pub fn refresh_clock(&mut self) {
        let current_time = Utc::now().with_timezone(&self.tz);
        let date_changed = current_time.date_naive() != self.now.date_naive();
        self.now = current_time;

        if date_changed && self.follow_current_date() {
            self.select_current_or_next_session();
        }
    }

    fn follow_current_date(&mut self) -> bool {
        let today = self.now.date_naive().format("%Y-%m-%d").to_string();
        if self
            .schedule
            .days
            .get(self.day_index)
            .is_some_and(|day| day.date == today)
        {
            return false;
        }

        let Some(event_day_index) = self.schedule.days.iter().position(|day| day.date == today)
        else {
            return false;
        };

        self.switch_day(event_day_index);
        true
    }

    fn switch_day(&mut self, day_index: usize) {
        self.day_index = day_index;
        self.focused_room_column = 0;
        self.focused_time_row = 0;
        self.first_visible_grid_line = 0;
        self.first_visible_room_index = 0;
        self.pending_view_anchor = None;
        self.pending_focus_bottom_margin = false;
        self.session_detail_open = false;
        self.session_detail_scroll_line = 0;
        self.search = None;
        self.build_room_columns();
    }

    /// N/P: move through the event days without leaving today's focus behind.
    pub fn select_next_day(&mut self) {
        self.change_day(1);
    }

    pub fn select_previous_day(&mut self) {
        self.change_day(-1);
    }

    fn change_day(&mut self, delta: isize) {
        let day_count = self.schedule.days.len() as isize;
        let target_day_index =
            (self.day_index as isize + delta).clamp(0, day_count.saturating_sub(1)) as usize;
        if target_day_index == self.day_index {
            return;
        }

        self.switch_day(target_day_index);
        let today = self.now.date_naive().format("%Y-%m-%d").to_string();
        if self.schedule.days[self.day_index].date == today {
            self.select_current_or_next_session();
            return;
        }

        let first_session_index = self
            .schedule
            .session_indices_on(&self.schedule.days[self.day_index])
            .first()
            .copied();
        if let Some(session_index) = first_session_index {
            self.focus_session(session_index);
            self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
        }
    }

    // ------------------------------------------------------------------
    // search
    // ------------------------------------------------------------------

    pub fn start_search(&mut self) {
        let focused_session_index = self.focused_session_index();
        let matches = self.current_day_session_indices();
        let selected_match = focused_session_index
            .and_then(|focused_session_index| {
                matches
                    .iter()
                    .position(|&session_index| session_index == focused_session_index)
            })
            .unwrap_or(0);
        self.search = Some(SessionSearch {
            query: String::new(),
            matches,
            selected_match,
            focus_before_search: (self.focused_room_column, self.focused_time_row),
        });
    }

    pub fn search_push_char(&mut self, character: char) {
        if let Some(search) = &mut self.search {
            search.query.push(character);
        }
        self.refresh_search_matches();
    }

    pub fn search_pop_char(&mut self) {
        if let Some(search) = &mut self.search {
            search.query.pop();
        }
        self.refresh_search_matches();
    }

    pub fn select_search_match(&mut self, delta: isize) {
        let Some(search) = &mut self.search else {
            return;
        };
        if search.matches.is_empty() {
            return;
        }
        let match_count = search.matches.len() as isize;
        search.selected_match =
            (search.selected_match as isize + delta).rem_euclid(match_count) as usize;
        let session_index = search.matches[search.selected_match];
        self.focus_session(session_index);
        self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
        self.pending_focus_bottom_margin = false;
        self.session_detail_scroll_line = 0;
    }

    pub fn confirm_search(&mut self) {
        self.search = None;
    }

    pub fn cancel_search(&mut self) {
        if let Some(search) = self.search.take() {
            (self.focused_room_column, self.focused_time_row) = search.focus_before_search;
            self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
            self.pending_focus_bottom_margin = false;
            self.session_detail_scroll_line = 0;
        }
    }

    fn refresh_search_matches(&mut self) {
        let Some(search) = &self.search else {
            return;
        };
        let query = search.query.clone();
        let focus_before_search = search.focus_before_search;
        let previous_focused_session = self.focused_session_index();
        let matches: Vec<usize> = self
            .current_day_session_indices()
            .into_iter()
            .filter(|&session_index| self.session_matches_search(session_index, &query))
            .collect();
        let session_at_initial_focus =
            self.session_index_at(focus_before_search.0, focus_before_search.1);
        let preferred_session = if query.trim().is_empty() {
            session_at_initial_focus
        } else {
            previous_focused_session
        };
        let selected_match = preferred_session
            .and_then(|preferred_session| {
                matches
                    .iter()
                    .position(|&session_index| session_index == preferred_session)
            })
            .unwrap_or(0);

        if let Some(session_index) = matches.get(selected_match).copied() {
            self.focus_session(session_index);
        } else {
            (self.focused_room_column, self.focused_time_row) = focus_before_search;
        }

        let search = self.search.as_mut().unwrap();
        search.matches = matches;
        search.selected_match = selected_match;
        self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
        self.pending_focus_bottom_margin = false;
        self.session_detail_scroll_line = 0;
    }

    fn current_day_session_indices(&self) -> Vec<usize> {
        self.schedule
            .session_indices_on(&self.schedule.days[self.day_index])
    }

    fn session_matches_search(&self, session_index: usize, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return true;
        }
        let session = &self.schedule.sessions[session_index];
        let text_matches = |text: &str| text.to_lowercase().contains(&query);

        text_matches(&session.title)
            || text_matches(&compact_room(&session.room))
            || session
                .categories
                .iter()
                .any(|category| text_matches(&category.item))
            || session.speakers.iter().any(|speaker_id| {
                self.schedule
                    .speakers
                    .get(speaker_id)
                    .is_some_and(|speaker| text_matches(&speaker.name))
            })
    }

    // ------------------------------------------------------------------
    // navigation
    // ------------------------------------------------------------------

    /// j/k: next/previous session in the focused room column.
    pub fn select_adjacent_session(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let session_indices = match self.room_columns.get(self.focused_room_column) {
            Some(room_column) if !room_column.session_indices_by_start.is_empty() => {
                room_column.session_indices_by_start.clone()
            }
            _ => return,
        };

        let next_session_index = if let Some(focused_session_index) = self.focused_session_index() {
            let current_position = session_indices
                .iter()
                .position(|&session_index| session_index == focused_session_index)
                .unwrap_or(0) as isize;
            let next_position =
                (current_position + delta).clamp(0, session_indices.len() as isize - 1) as usize;
            (next_position != current_position as usize).then(|| session_indices[next_position])
        } else {
            let Some(focused_time_start) = self.time_row_starts.get(self.focused_time_row).cloned()
            else {
                return;
            };
            if delta > 0 {
                session_indices.iter().copied().find(|&session_index| {
                    self.schedule.sessions[session_index].start.as_str()
                        > focused_time_start.as_str()
                })
            } else {
                session_indices
                    .iter()
                    .rev()
                    .copied()
                    .find(|&session_index| {
                        self.schedule.sessions[session_index].start.as_str()
                            < focused_time_start.as_str()
                    })
            }
        };

        if let Some(session_index) = next_session_index {
            self.focus_session(session_index);
            self.pending_focus_bottom_margin = delta > 0;
            self.session_detail_scroll_line = 0;
        }
    }

    /// h/l: move the focus one room column, keeping the same time row. Empty
    /// cells are valid focus targets, so sparse timeslots still scroll.
    pub fn select_adjacent_room(&mut self, delta: isize) {
        if delta == 0 || self.room_columns.is_empty() {
            return;
        }
        let target_room_column = (self.focused_room_column as isize + delta.signum())
            .clamp(0, self.room_columns.len() as isize - 1)
            as usize;
        if target_room_column == self.focused_room_column {
            return;
        }
        self.focused_room_column = target_room_column;
        self.pending_focus_bottom_margin = false;
        self.session_detail_scroll_line = 0;
    }

    /// The session in `room_column_index` that is ongoing at `time`, else the
    /// next one, else the last one.
    fn session_active_or_next_in_room(
        &self,
        room_column_index: usize,
        time: &str,
    ) -> Option<usize> {
        let session_indices = &self
            .room_columns
            .get(room_column_index)?
            .session_indices_by_start;
        if session_indices.is_empty() {
            return None;
        }

        let first_later_position = session_indices.partition_point(|&session_index| {
            self.schedule.sessions[session_index].start.as_str() <= time
        });
        for &session_index in session_indices[..first_later_position].iter().rev() {
            if self.schedule.sessions[session_index].end.as_str() > time {
                return Some(session_index);
            }
        }
        if let Some(&session_index) = session_indices.get(first_later_position) {
            return Some(session_index);
        }
        session_indices.last().copied()
    }

    /// Ctrl-d/Ctrl-u: move the time window by half a page, keeping the room.
    pub fn scroll_half_page(&mut self, direction: isize) {
        let total_grid_lines = self.grid_line_count();
        if total_grid_lines == 0 || self.viewport_height == 0 {
            return;
        }

        let half_page_lines = (self.viewport_height / 2).max(1) as isize;
        let max_first_line = total_grid_lines.saturating_sub(self.viewport_height);
        let target_first_line = (self.first_visible_grid_line as isize
            + direction * half_page_lines)
            .clamp(0, max_first_line as isize) as usize;
        if target_first_line == self.first_visible_grid_line {
            return;
        }
        self.first_visible_grid_line = target_first_line;

        // Focus whatever is near the middle of the newly positioned window.
        let anchor_grid_line =
            (self.first_visible_grid_line + self.viewport_height / 2).min(total_grid_lines - 1);
        let time_row = self.time_row_at_grid_line(anchor_grid_line);
        let anchor_time = self.time_row_starts[time_row].clone();
        if let Some(session_index) =
            self.session_active_or_next_in_room(self.focused_room_column, &anchor_time)
        {
            self.focus_session(session_index);
        } else {
            self.focused_time_row = time_row;
        }
        self.session_detail_scroll_line = 0;
    }

    pub fn select_first_session_in_room(&mut self) {
        if let Some(&session_index) = self
            .room_columns
            .get(self.focused_room_column)
            .and_then(|room_column| room_column.session_indices_by_start.first())
        {
            self.focus_session(session_index);
            self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
            self.pending_focus_bottom_margin = false;
            self.session_detail_scroll_line = 0;
        }
    }

    pub fn select_last_session_in_room(&mut self) {
        if let Some(&session_index) = self
            .room_columns
            .get(self.focused_room_column)
            .and_then(|room_column| room_column.session_indices_by_start.last())
        {
            self.focus_session(session_index);
            self.pending_view_anchor = Some(ViewAnchor::FocusedRow);
            self.pending_focus_bottom_margin = false;
            self.session_detail_scroll_line = 0;
        }
    }

    pub fn open_session_detail(&mut self) {
        if self.focused_session().is_some() {
            self.session_detail_open = true;
            self.session_detail_scroll_line = 0;
        }
    }

    pub fn scroll_session_detail(&mut self, delta: isize) {
        let max_scroll_line = self.session_detail_max_scroll_line.min(u16::MAX as usize) as isize;
        self.session_detail_scroll_line =
            (self.session_detail_scroll_line as isize + delta).clamp(0, max_scroll_line) as u16;
    }
}

/// Today if it is an event day, otherwise the most recent event day that has
/// started, otherwise the first day.
fn event_day_for_date(schedule: &SessionData, today: chrono::NaiveDate) -> usize {
    let today = today.format("%Y-%m-%d").to_string();
    let mut matching_day = 0;
    for (event_day_index, day) in schedule.days.iter().enumerate() {
        if day.date <= today {
            matching_day = event_day_index;
        } else {
            break;
        }
    }
    matching_day
}

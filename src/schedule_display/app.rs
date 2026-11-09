//! Display state: which day/session is focused, where we are scrolled, and
//! whether the detail view is open. No rendering and no terminal I/O here.

use chrono::{DateTime, Utc};
use chrono_tz::Tz;

use crate::session_data::{Session, SessionData, Status};

pub struct App {
    pub data: SessionData,
    pub tz: Tz,
    pub now: DateTime<Tz>,
    pub day_index: usize,
    /// Indices into `data.sessions` for the displayed day, in start order.
    pub day_indices: Vec<usize>,
    /// Index into `day_indices`.
    pub selected: usize,
    /// First visible row in the schedule list.
    pub scroll: usize,
    /// Set after `t`/launch so the view can place "now" near the top third.
    pub center_selection: bool,
    pub list_height: usize,
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
        let day_indices = data.sessions_on(&data.days[day_index]);

        let mut app = Self {
            data,
            tz,
            now,
            day_index,
            day_indices,
            selected: 0,
            scroll: 0,
            center_selection: false,
            list_height: 20,
            detail_open: false,
            detail_scroll: 0,
            detail_total: 0,
            should_quit: false,
        };
        app.focus_now();
        app
    }

    pub fn session(&self) -> Option<&Session> {
        self.day_indices
            .get(self.selected)
            .map(|&index| &self.data.sessions[index])
    }

    /// Put the selection on the live session that started most recently
    /// (avoids all-day booths/registration winning over an actual talk),
    /// otherwise on the next session, otherwise on the last one.
    pub fn focus_now(&mut self) {
        self.now = Utc::now().with_timezone(&self.tz);
        self.refresh_day();

        let now = self.now;
        let tz = self.tz;
        let mut live: Option<(usize, DateTime<Tz>)> = None;
        let mut next: Option<(usize, DateTime<Tz>)> = None;

        for (position, &index) in self.day_indices.iter().enumerate() {
            let session = &self.data.sessions[index];
            match session.status(now, tz) {
                Status::Live => {
                    if let Some(start) = session.start_at(tz) {
                        if live.map_or(true, |(_, best)| start > best) {
                            live = Some((position, start));
                        }
                    }
                }
                Status::Upcoming => {
                    if let Some(start) = session.start_at(tz) {
                        if next.map_or(true, |(_, best)| start < best) {
                            next = Some((position, start));
                        }
                    }
                }
                Status::Past => {}
            }
        }

        self.selected = live
            .map(|(position, _)| position)
            .or_else(|| next.map(|(position, _)| position))
            .unwrap_or_else(|| self.day_indices.len().saturating_sub(1));
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

    /// If the calendar day rolled over to another event day, switch to it.
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
        self.day_indices = self.data.sessions_on(&self.data.days[index]);
        self.selected = 0;
        self.scroll = 0;
        self.detail_open = false;
        true
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.day_indices.is_empty() {
            return;
        }
        let last = self.day_indices.len() - 1;
        let next = (self.selected as isize + delta).clamp(0, last as isize) as usize;
        if next != self.selected {
            self.selected = next;
            self.detail_scroll = 0;
        }
    }

    pub fn select_first(&mut self) {
        if !self.day_indices.is_empty() {
            self.selected = 0;
            self.detail_scroll = 0;
        }
    }

    pub fn select_last(&mut self) {
        if !self.day_indices.is_empty() {
            self.selected = self.day_indices.len() - 1;
            self.detail_scroll = 0;
        }
    }

    pub fn page(&mut self, direction: isize) {
        let step = (self.list_height / 2).max(1) as isize;
        self.move_selection(direction * step);
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

//! Deserialised, read-only view of `data/sessions.json`.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SessionData {
    pub event: Event,
    pub days: Vec<Day>,
    #[serde(default)]
    pub speakers: HashMap<String, Speaker>,
    pub sessions: Vec<Session>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub name: String,
    pub city: String,
    pub timezone: String,
}

#[derive(Debug, Deserialize)]
pub struct Day {
    pub date: String,
    pub label: String,
    /// Grid column order for this day, exactly as the website renders it.
    pub rooms: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Speaker {
    pub name: String,
    pub tagline: Option<String>,
    pub bio: Option<String>,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    pub title: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct Session {
    pub title: String,
    pub description: Option<String>,
    pub start: String,
    pub end: String,
    pub day: String,
    pub room: String,
    /// Zero-based grid column (index into `Day::rooms`).
    pub col: usize,
    #[serde(default)]
    pub categories: Vec<Category>,
    #[serde(default)]
    pub speakers: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Category {
    pub category: String,
    pub item: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Upcoming,
    Live,
    Past,
}

impl SessionData {
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let data: SessionData = serde_json::from_str(&raw)
            .map_err(|error| format!("cannot parse {}: {error}", path.display()))?;

        if data.days.is_empty() {
            return Err(format!("{} contains no days", path.display()));
        }
        if data.sessions.is_empty() {
            return Err(format!("{} contains no sessions", path.display()));
        }
        Ok(data)
    }

    pub fn timezone(&self) -> Tz {
        self.event
            .timezone
            .parse::<Tz>()
            .unwrap_or(chrono_tz::America::Denver)
    }

    /// Indices into `sessions` for one day, ordered by start time then room.
    pub fn sessions_on(&self, day: &Day) -> Vec<usize> {
        let mut indices: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| session.day == day.date)
            .map(|(index, _)| index)
            .collect();
        indices.sort_by(|&a, &b| {
            let left = &self.sessions[a];
            let right = &self.sessions[b];
            left.start
                .cmp(&right.start)
                .then(left.col.cmp(&right.col))
                .then_with(|| left.title.cmp(&right.title))
        });
        indices
    }
}

impl Session {
    pub fn start_at(&self, tz: Tz) -> Option<DateTime<Tz>> {
        parse_local(&self.start, tz)
    }

    pub fn end_at(&self, tz: Tz) -> Option<DateTime<Tz>> {
        parse_local(&self.end, tz)
    }

    pub fn status(&self, now: DateTime<Tz>, tz: Tz) -> Status {
        match (self.start_at(tz), self.end_at(tz)) {
            (Some(start), Some(end)) if now >= start && now < end => Status::Live,
            (Some(start), _) if now < start => Status::Upcoming,
            _ => Status::Past,
        }
    }

    pub fn duration_minutes(&self, tz: Tz) -> i64 {
        match (self.start_at(tz), self.end_at(tz)) {
            (Some(start), Some(end)) => (end - start).num_minutes().max(0),
            _ => 0,
        }
    }

    pub fn start_hm(&self, tz: Tz) -> String {
        self.start_at(tz)
            .map(|time| time.format("%H:%M").to_string())
            .unwrap_or_else(|| "--:--".to_string())
    }

    pub fn end_hm(&self, tz: Tz) -> String {
        self.end_at(tz)
            .map(|time| time.format("%H:%M").to_string())
            .unwrap_or_else(|| "--:--".to_string())
    }

    pub fn category(&self, name: &str) -> Option<&str> {
        self.categories
            .iter()
            .find(|category| category.category == name)
            .map(|category| category.item.as_str())
    }

    pub fn category_all(&self, name: &str) -> Vec<&str> {
        self.categories
            .iter()
            .filter(|category| category.category == name)
            .map(|category| category.item.as_str())
            .collect()
    }

    pub fn track(&self) -> Option<&str> {
        self.category("All Sessions")
    }

    pub fn level(&self) -> Option<&str> {
        self.category("Audience Level")
    }

    pub fn projects(&self) -> Vec<&str> {
        self.category_all("CNCF Projects")
    }

    pub fn subtopics(&self) -> Vec<&str> {
        self.category_all("SubTopic")
    }

    pub fn tags(&self) -> Vec<&str> {
        self.categories
            .iter()
            .filter(|category| {
                !matches!(
                    category.category.as_str(),
                    "All Sessions" | "Audience Level" | "SubTopic" | "CNCF Projects"
                )
            })
            .map(|category| category.item.as_str())
            .collect()
    }
}

/// "Salt Palace | Level 2 | 250 A-C" -> "SP · Level 2 · 250 A-C".
pub fn compact_room(room: &str) -> String {
    let parts: Vec<&str> = room
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return room.trim().to_string();
    }

    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            if index == 0 {
                match *part {
                    "Salt Palace" | "Salt Palace Convention Center" => "SP".to_string(),
                    "Hyatt Regency" => "HR".to_string(),
                    other => other.to_string(),
                }
            } else {
                (*part).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn parse_local(value: &str, tz: Tz) -> Option<DateTime<Tz>> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .and_then(|naive| tz.from_local_datetime(&naive).earliest())
}

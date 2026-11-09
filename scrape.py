#!/usr/bin/env python3
"""One-time scrape of the KubeCon + CloudNativeCon NA schedule.

The LF schedule page is rendered from Sessionize (API code ``svi82w6c``).
This script pulls the two Sessionize views the page uses and writes a single
normalised JSON file that the Rust TUI reads at runtime:

    https://sessionize.com/api/v2/svi82w6c/view/All
    https://sessionize.com/api/v2/svi82w6c/view/GridSmart

Usage:
    ./scrape.py                 # writes data/sessions.json
    ./scrape.py --out other.json
    ./scrape.py --timezone America/Denver

Only the Python standard library is used. Rust makes no network requests.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from zoneinfo import ZoneInfo

API_CODE = "svi82w6c"
API_URL = "https://sessionize.com/api/v2/{code}/view/{view}"
SOURCE_URL = (
    "https://events.linuxfoundation.org/"
    "kubecon-cloudnativecon-north-america/program/schedule/"
)
EVENT_NAME = "KubeCon + CloudNativeCon North America 2026"
EVENT_CITY = "Salt Lake City, Utah"
DEFAULT_TIMEZONE = "America/Denver"

VIEWS = ("All", "GridSmart")


def fetch_json(url: str, attempts: int = 3) -> dict | list:
    headers = {
        "User-Agent": "kubecon-tui/0.1 (one-time schedule scrape)",
        "Accept": "application/json",
    }
    last_error: Exception | None = None
    for attempt in range(1, attempts + 1):
        try:
            request = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(request, timeout=90) as response:
                return json.loads(response.read().decode("utf-8"))
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
            last_error = exc
            if attempt < attempts:
                time.sleep(2 * attempt)
    raise SystemExit(f"error: could not fetch {url}: {last_error}")


def clean_text(value: str | None) -> str | None:
    if not value:
        return None
    text = value.replace("\r\n", "\n").replace("\r", "\n").strip()
    return text or None


def day_label(date: dt.date) -> str:
    return f"{date.strftime('%A')}, {date.strftime('%B')} {date.day}"


def build_payload(all_data: dict, grid_data: list, tz_name: str) -> dict:
    today = dt.datetime.now(ZoneInfo(tz_name)).date()

    category_items: dict[int, tuple[str, str]] = {}
    for category in all_data.get("categories", []):
        for item in category.get("items", []):
            category_items[item["id"]] = (category["title"], item["name"])

    room_sort = {
        room["id"]: room.get("sort", 9999) for room in all_data.get("rooms", [])
    }

    speakers: dict[str, dict] = {}
    for speaker in all_data.get("speakers", []):
        speakers[speaker["id"]] = {
            "name": speaker.get("fullName")
            or " ".join(filter(None, [speaker.get("firstName"), speaker.get("lastName")])),
            "tagline": clean_text(speaker.get("tagLine")),
            "bio": clean_text(speaker.get("bio")),
            "links": [
                {"title": link.get("title"), "url": link["url"]}
                for link in speaker.get("links", [])
                if link.get("url")
            ],
        }

    all_sessions = {session["id"]: session for session in all_data.get("sessions", [])}

    days: list[dict] = []
    sessions: list[dict] = []
    seen: set[str] = set()

    # Newest scrape wins: days that are already over are not history we want.
    for grid_day in sorted(grid_data, key=lambda day: day["date"]):
        date = dt.date.fromisoformat(grid_day["date"][:10])
        if date < today:
            continue

        for slot in grid_day.get("timeSlots", []):
            for room_slot in slot.get("rooms", []):
                raw = room_slot.get("session") or {}
                session_id = str(raw.get("id", ""))
                if not session_id or session_id in seen:
                    continue
                seen.add(session_id)

                full = all_sessions.get(session_id, raw)
                room_id = full.get("roomId") or raw.get("roomId") or room_slot.get("id")
                room_name = (
                    room_slot.get("name")
                    or full.get("room")
                    or all_data_room_name(all_data, room_id)
                    or "Room TBD"
                )

                categories = []
                for item_id in full.get("categoryItems", raw.get("categoryItems", [])):
                    category, item = category_items.get(item_id, (None, None))
                    if category and item:
                        categories.append({"category": category, "item": item})

                start = full.get("startsAt") or raw.get("startsAt")
                end = full.get("endsAt") or raw.get("endsAt")
                if not start:
                    start = f"{date.isoformat()}T{slot.get('slotStart', '00:00:00')}"
                if not end:
                    end = start

                speaker_ids = full.get("speakers") or raw.get("speakers") or []

                sessions.append(
                    {
                        "id": session_id,
                        "title": (full.get("title") or raw.get("title") or "").strip(),
                        "description": clean_text(
                            full.get("description") or raw.get("description")
                        ),
                        "start": start,
                        "end": end,
                        "day": date.isoformat(),
                        "room": room_name.strip(),
                        "room_sort": room_sort.get(room_id, 9999),
                        "categories": categories,
                        "speakers": [sid for sid in speaker_ids if sid in speakers],
                    }
                )

        days.append({"date": date.isoformat(), "label": day_label(date)})

    sessions.sort(key=lambda s: (s["day"], s["start"], s["room_sort"], s["title"].lower()))
    used_speakers = {speaker_id for session in sessions for speaker_id in session["speakers"]}

    return {
        "event": {
            "name": EVENT_NAME,
            "city": EVENT_CITY,
            "timezone": tz_name,
            "source": SOURCE_URL,
            "scraped_at": dt.datetime.now(dt.timezone.utc)
            .astimezone()
            .isoformat(timespec="seconds"),
        },
        "days": days,
        "speakers": {
            key: value for key, value in speakers.items() if key in used_speakers
        },
        "sessions": sessions,
    }


def all_data_room_name(all_data: dict, room_id: int | None) -> str | None:
    for room in all_data.get("rooms", []):
        if room["id"] == room_id:
            return room["name"]
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Scrape the KubeCon NA schedule once.")
    parser.add_argument(
        "-o",
        "--out",
        type=Path,
        default=Path(__file__).resolve().parent / "data" / "sessions.json",
    )
    parser.add_argument("--timezone", default=DEFAULT_TIMEZONE)
    parser.add_argument("--all", type=Path, help="use a pre-downloaded All JSON")
    parser.add_argument("--grid", type=Path, help="use a pre-downloaded GridSmart JSON")
    args = parser.parse_args()

    ZoneInfo(args.timezone)

    if args.all:
        all_data = json.loads(args.all.read_text())
    else:
        url = API_URL.format(code=API_CODE, view="All")
        print(f"fetching {url}", file=sys.stderr)
        all_data = fetch_json(url)

    if args.grid:
        grid_data = json.loads(args.grid.read_text())
    else:
        url = API_URL.format(code=API_CODE, view="GridSmart")
        print(f"fetching {url}", file=sys.stderr)
        grid_data = fetch_json(url)

    payload = build_payload(all_data, grid_data, args.timezone)

    if not payload["sessions"]:
        raise SystemExit("error: scrape produced no sessions (is the event over?)")

    args.out.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.out.with_suffix(args.out.suffix + ".tmp")
    temporary.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n")
    temporary.replace(args.out)

    print(
        f"wrote {args.out}: {len(payload['sessions'])} sessions, "
        f"{len(payload['speakers'])} speakers, {len(payload['days'])} days",
        file=sys.stderr,
    )
    for day in payload["days"]:
        count = sum(1 for session in payload["sessions"] if session["day"] == day["date"])
        print(f"  {day['date']}  {day['label']:<24} {count:>3} sessions", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

# kubecon-tui

Time-aware terminal schedule for **KubeCon + CloudNativeCon North America 2026**
(Salt Lake City, November 9–12).

It shows the current day as a room-by-room grid in the same column order as
the website's schedule grid, dims sessions that have already ended, highlights
what is live, and scrolls to the current time on launch. Session titles wrap,
time rows expand as needed, and each session's subtrack appears in a subdued
color beneath its title.

## Setup

The toolchain and Python are provided by the Nix flake:

```sh
direnv allow       # loads the dev shell via nix-direnv
cargo build
cargo run
```

Without direnv:

```sh
nix develop --command cargo run
```

## Run the packaged flake

```sh
nix run .            # packaged binary with the committed schedule data
nix build .#default  # ./result/bin/kubecon-tui
```

The package installs `data/sessions.json` under `share/kubecon-tui` and makes
it the binary's default data path. Pass `--data PATH` to override it.

## Data

`data/sessions.json` is a one-time scrape of the schedule the LF event page
renders from Sessionize (API code `svi82w6c`). It is committed to the repo, so
the TUI works offline and the Rust binary never makes a network request.

The scrape preserves each day's room-column order and every session's
column index from that grid. `scrape.py` generated it (Python standard
library only):

```sh
./scrape.py              # refreshes data/sessions.json
./scrape.py --help
```

Days that are already over at scrape time are skipped, so re-running this
after the event will intentionally produce nothing.

## Keys

| Key | Action |
| --- | --- |
| `j` / `k`, up/down | next / previous session in the current room |
| `h` / `l`, left/right | previous / next room column |
| `g` / `G` | first / last session in the current room |
| `Ctrl-d` / `Ctrl-u`, PgDn/PgUp | move half a time-page |
| `Enter` | open session details |
| `Esc`, `h` | back to the schedule |
| `j` / `k` in details | scroll the details |
| `t` | jump to the live/next session and re-center |
| `q`, `Ctrl-c` | quit |

## Layout

```
src/
  main.rs                    loads and parses the JSON, hands it to the UI
  session_data.rs            data model, day filtering, time/status helpers
  schedule_display/
    mod.rs                   terminal setup and event loop
    app.rs                   time rows, room columns, selection, time-aware state
    layout.rs                wrapped-title measurement and time row line heights
    input.rs                 vim key mapping
    view.rs                  schedule grid, room headers, session details
data/sessions.json           committed one-time scrape
scrape.py                    one-time Sessionize scraper
flake.nix / .envrc           Nix dev shell and direnv integration
```

Use `--data PATH` to point at a different JSON file.

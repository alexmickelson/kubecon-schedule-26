# kubecon-tui

Time-aware terminal schedule for **KubeCon + CloudNativeCon North America 2026**
(Salt Lake City, November 9–12).

It shows the current day's schedule, dims sessions that have already ended,
highlights what is live, and scrolls to the current time on launch.

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

## Data

`data/sessions.json` is a one-time scrape of the schedule the LF event page
renders from Sessionize (API code `svi82w6c`). It is committed to the repo, so
the TUI works offline and the Rust binary never makes a network request.

`scrape.py` generated it (Python standard library only):

```sh
./scrape.py              # refreshes data/sessions.json
./scrape.py --help
```

Days that are already over at scrape time are skipped, so re-running this
after the event will intentionally produce nothing.

## Keys

| Key | Action |
| --- | --- |
| `j` / `k`, arrows | move through the schedule |
| `g` / `G` | first / last session |
| `Ctrl-d` / `Ctrl-u`, PgDn/PgUp | half page |
| `Enter`, `l` | open session details |
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
    app.rs                   selection, scrolling, time-aware state
    input.rs                 vim key mapping
    view.rs                  rendering
data/sessions.json           committed one-time scrape
scrape.py                    one-time Sessionize scraper
flake.nix / .envrc           Nix dev shell and direnv integration
```

Use `--data PATH` to point at a different JSON file.

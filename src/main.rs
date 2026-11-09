mod schedule_display;
mod session_data;

use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_DATA_PATH: &str = "data/sessions.json";

fn main() -> ExitCode {
    let mut path = PathBuf::from(DEFAULT_DATA_PATH);

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--data" => match args.next() {
                Some(value) => path = PathBuf::from(value),
                None => {
                    eprintln!("kubecon-tui: --data needs a path");
                    return ExitCode::FAILURE;
                }
            },
            other => {
                eprintln!("kubecon-tui: unknown argument '{other}'");
                print_help();
                return ExitCode::FAILURE;
            }
        }
    }

    // main owns loading: read + parse the scraped JSON, then hand it to the UI.
    let data = match session_data::SessionData::load(&path) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("kubecon-tui: {error}");
            return ExitCode::FAILURE;
        }
    };

    match schedule_display::run(data) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("kubecon-tui: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "kubecon-tui — time-aware KubeCon + CloudNativeCon schedule\n\n\
         Usage: kubecon-tui [--data PATH]\n\n\
         Options:\n  \
         --data PATH   session JSON (default: data/sessions.json)\n  \
         -h, --help    show this help\n\n\
         Keys:\n  \
         j/k, arrows   move\n  \
         g/G           first/last\n  \
         Ctrl-d/u      half page\n  \
         Enter, l      session details\n  \
         Esc, h        back to schedule\n  \
         t             jump to now\n  \
         q             quit"
    );
}

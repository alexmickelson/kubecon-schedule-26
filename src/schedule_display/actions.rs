//! Side effects launched from the session detail page: clipboard and browser.

use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut errors = Vec::new();
    for (program, arguments) in clipboard_commands() {
        match send_to_program(program, &arguments, text) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    Err(errors.join("; "))
}

fn clipboard_commands() -> Vec<(&'static str, Vec<&'static str>)> {
    if cfg!(target_os = "macos") {
        vec![("pbcopy", Vec::new())]
    } else if cfg!(target_os = "windows") {
        vec![("clip.exe", Vec::new()), ("clip", Vec::new())]
    } else {
        vec![
            ("wl-copy", Vec::new()),
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
            ("clip.exe", Vec::new()),
        ]
    }
}

fn send_to_program(program: &str, arguments: &[&str], text: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => format!("{program} not found"),
            _ => format!("could not run {program}: {error}"),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        if let Err(error) = stdin.write_all(text.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} could not accept text: {error}"));
        }
    }

    match child.wait() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("{program} exited with {status}")),
        Err(error) => Err(format!("could not wait for {program}: {error}")),
    }
}

pub fn open_in_browser(url: &str) -> Result<(), String> {
    let mut errors = Vec::new();
    for (program, arguments) in browser_commands() {
        match spawn_program(program, &arguments, url) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    Err(errors.join("; "))
}

fn browser_commands() -> Vec<(&'static str, Vec<&'static str>)> {
    if cfg!(target_os = "macos") {
        vec![("open", Vec::new())]
    } else if cfg!(target_os = "windows") {
        vec![("cmd", vec!["/C", "start", ""])]
    } else {
        vec![
            ("xdg-open", Vec::new()),
            ("gio", vec!["open"]),
            ("sensible-browser", Vec::new()),
        ]
    }
}

fn spawn_program(program: &str, arguments: &[&str], url: &str) -> Result<(), String> {
    let mut child = Command::new(program)
        .args(arguments)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| match error.kind() {
            ErrorKind::NotFound => format!("{program} not found"),
            _ => format!("could not run {program}: {error}"),
        })?;

    // Do not block the TUI on a browser process that may stay alive.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

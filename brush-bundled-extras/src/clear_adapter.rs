//! In-tree `clear` implementation — write ANSI clear-screen + cursor-home.
//!
//! Per the post-Cycle 1 follow-up of
//! `docs/planning/bundled-extras-coverage-expansion.md`. uutils 0.8.0
//! does not ship a `clear` crate at all; this is the smallest possible
//! implementation that does what 99% of agent-shell users want
//! (`clear` to wipe the visible buffer).
//!
//! The escape sequence `\x1b[H\x1b[2J\x1b[3J` does:
//! - `\x1b[H` — move cursor to home (top-left)
//! - `\x1b[2J` — erase entire visible screen
//! - `\x1b[3J` — erase scrollback buffer (xterm + Win Terminal honor)
//!
//! Modern Windows terminals (Win10+ cmd, Windows Terminal, mintty,
//! `ConEmu`, all third-party terminals) honor ANSI escapes by default.
//! For ancient pre-Win10 cmd this won't clear anything visibly, but
//! also won't error — agents looking for `command not found` won't get it.

use std::ffi::OsString;
use std::io::{self, Write};

pub(crate) fn clear_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    for arg in argv.iter().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "--version" => {
                println!("clear (brush-bundled-extras) 0.1.8");
                return 0;
            }
            "-x" => { /* GNU clear's "don't clear scrollback" — we still emit but no-op */ }
            _ => { /* tolerate unknown args silently like GNU clear */ }
        }
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let _ = out.write_all(b"\x1b[H\x1b[2J\x1b[3J");
    let _ = out.flush();
    0
}

fn print_help() {
    println!(
        "Usage: clear [OPTIONS]\n\
         \n\
         Clear the terminal screen using ANSI escape sequences.\n\
         \n\
         Options:\n  \
           -x          don't clear scrollback (no-op in this implementation)\n  \
           --help      show this help\n  \
           --version   show version\n"
    );
}

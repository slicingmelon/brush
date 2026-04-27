//! `which` adapter — thin CLI on the crates.io [`which`] crate.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 1.
//! The `which` crate handles PATH walking, PATHEXT (Windows), and
//! permission checks; we only ship the CLI surface.
//!
//! Supported flags:
//!
//! | Flag | Behavior |
//! |---|---|
//! | `-a` | Print all matches on PATH, not just the first |
//! | `-s` | Silent — exit only, don't print |

use std::ffi::OsString;
use std::io::{self, BufWriter, Write};

pub(crate) fn which_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("which: {e}");
            1
        }
    }
}

fn run(argv: &[String]) -> Result<i32, String> {
    let mut all = false;
    let mut silent = false;
    let mut targets: Vec<String> = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-a" | "--all" => all = true,
            "-s" | "--silent" => silent = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            "--version" => {
                println!("which (brush-bundled-extras) 0.1.4");
                return Ok(0);
            }
            s if s.starts_with('-') && s != "-" => return Err(format!("unknown option: {s}")),
            _ => targets.push(arg.clone()),
        }
        i += 1;
    }

    if targets.is_empty() {
        return Err("missing operand".to_string());
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut any_missing = false;
    for name in &targets {
        if all {
            match which::which_all(name) {
                Ok(iter) => {
                    let mut found = false;
                    for path in iter {
                        found = true;
                        if !silent {
                            writeln!(out, "{}", path.display()).map_err(|e| e.to_string())?;
                        }
                    }
                    if !found {
                        any_missing = true;
                    }
                }
                Err(_) => any_missing = true,
            }
        } else {
            match which::which(name) {
                Ok(path) => {
                    if !silent {
                        writeln!(out, "{}", path.display()).map_err(|e| e.to_string())?;
                    }
                }
                Err(_) => any_missing = true,
            }
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(i32::from(any_missing))
}

fn print_help() {
    println!(
        "Usage: which [OPTIONS] NAME...\n\
         \n\
         Locate a command on PATH.\n\
         \n\
         Options:\n  \
           -a, --all       print all matches, not just the first\n  \
           -s, --silent    don't print, just exit with status\n  \
           --help          show this help\n  \
           --version       show version\n"
    );
}

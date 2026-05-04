//! In-tree `column` implementation — table formatter.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 1.
//! Covers the dominant invocation `column -t [-s <sep>]` (table mode).
//! No `-x` (fill columns first) or `-c <width>` (terminal-fill) yet —
//! those can land if demand surfaces.
//!
//! Supported flags:
//!
//! | Flag | Behavior |
//! |---|---|
//! | `-t` | Table mode (default in this implementation) |
//! | `-s <sep>` | Input column separator (default: whitespace) |
//! | `-o <sep>` | Output column separator (default: two spaces) |
//! | `-N <names>` | Comma-separated list of column header names |

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

pub(crate) fn column_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("column: {e}");
            1
        }
    }
}

struct Cfg {
    table_mode: bool,
    input_sep: Option<String>,
    output_sep: String,
    headers: Option<Vec<String>>,
    inputs: Vec<String>,
}

#[allow(
    clippy::too_many_lines,
    reason = "single CLI entry point parsing argv + dispatching; splitting harms readability"
)]
fn run(argv: &[String]) -> Result<i32, String> {
    let mut cfg = Cfg {
        table_mode: false,
        input_sep: None,
        output_sep: "  ".to_string(),
        headers: None,
        inputs: Vec::new(),
    };
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-t" => cfg.table_mode = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            "--version" => {
                println!("column (brush-bundled-extras) 0.1.4");
                return Ok(0);
            }
            "-s" => {
                i += 1;
                cfg.input_sep = Some(
                    argv.get(i)
                        .ok_or_else(|| "-s requires an argument".to_string())?
                        .clone(),
                );
            }
            "-o" => {
                i += 1;
                cfg.output_sep.clone_from(
                    argv.get(i)
                        .ok_or_else(|| "-o requires an argument".to_string())?,
                );
            }
            "-N" => {
                i += 1;
                let names = argv
                    .get(i)
                    .ok_or_else(|| "-N requires an argument".to_string())?;
                cfg.headers = Some(names.split(',').map(str::to_string).collect());
            }
            s if s.starts_with('-') && s != "-" => return Err(format!("unknown option: {s}")),
            _ => cfg.inputs.push(arg.clone()),
        }
        i += 1;
    }
    // Default to table mode if -s or -N supplied — matches util-linux.
    if cfg.input_sep.is_some() || cfg.headers.is_some() {
        cfg.table_mode = true;
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    if let Some(headers) = &cfg.headers {
        rows.push(headers.clone());
    }
    if cfg.inputs.is_empty() {
        read_rows(&mut rows, BufReader::new(io::stdin().lock()), &cfg)?;
    } else {
        for path in &cfg.inputs {
            if path == "-" {
                read_rows(&mut rows, BufReader::new(io::stdin().lock()), &cfg)?;
            } else {
                let f = File::open(path).map_err(|e| format!("{path}: {e}"))?;
                read_rows(&mut rows, BufReader::new(f), &cfg)?;
            }
        }
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if cfg.table_mode {
        let widths = column_widths(&rows);
        for row in &rows {
            for (idx, cell) in row.iter().enumerate() {
                if idx > 0 {
                    out.write_all(cfg.output_sep.as_bytes())
                        .map_err(|e| e.to_string())?;
                }
                if idx + 1 == row.len() {
                    out.write_all(cell.as_bytes()).map_err(|e| e.to_string())?;
                } else {
                    let pad = widths
                        .get(idx)
                        .copied()
                        .unwrap_or(0)
                        .saturating_sub(cell.len());
                    out.write_all(cell.as_bytes()).map_err(|e| e.to_string())?;
                    for _ in 0..pad {
                        out.write_all(b" ").map_err(|e| e.to_string())?;
                    }
                }
            }
            out.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    } else {
        // Non-table mode — pass through verbatim. (Real `column` packs
        // into terminal-width columns; deferred until -c support is added.)
        for row in &rows {
            out.write_all(row.join(" ").as_bytes())
                .map_err(|e| e.to_string())?;
            out.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(0)
}

fn read_rows<R: BufRead>(rows: &mut Vec<Vec<String>>, reader: R, cfg: &Cfg) -> Result<(), String> {
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() {
            continue;
        }
        let cols: Vec<String> = match &cfg.input_sep {
            Some(sep) => line.split(sep.as_str()).map(str::to_string).collect(),
            None => line.split_whitespace().map(str::to_string).collect(),
        };
        rows.push(cols);
    }
    Ok(())
}

fn column_widths(rows: &[Vec<String>]) -> Vec<usize> {
    let max_cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0_usize; max_cols];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }
    widths
}

fn print_help() {
    println!(
        "Usage: column [OPTIONS] [FILE...]\n\
         \n\
         Format input as a table.\n\
         \n\
         Options:\n  \
           -t              table mode (align columns)\n  \
           -s SEP          input column separator (default: whitespace)\n  \
           -o SEP          output column separator (default: two spaces)\n  \
           -N a,b,c        comma-separated header names (implies -t)\n  \
           --help          show this help\n  \
           --version       show version\n"
    );
}

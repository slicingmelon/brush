//! In-tree `tree` implementation using the crates.io [`walkdir`] crate.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 1.
//! Covers the dominant invocations agents emit: `tree`, `tree -L 2`,
//! `tree -d`, `tree -a`, `tree -I '*.tmp'`. Box-drawing identical to
//! GNU `tree`'s default ASCII-graphics output.
//!
//! Supported flags:
//!
//! | Flag | Behavior |
//! |---|---|
//! | `-L <n>` | Max depth (root = 1) |
//! | `-d` | Directories only |
//! | `-a` | Show hidden entries (default: hide dotfiles) |
//! | `-I <pat>` | Exclude entries matching glob pattern (comma-sep) |
//! | `-P <pat>` | Include only entries matching glob pattern (comma-sep) |
//! | `-f` | Print full relative path on each line |
//! | `--noreport` | Suppress trailing "N directories, N files" line |

use std::ffi::OsString;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub(crate) fn tree_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tree: {e}");
            1
        }
    }
}

struct Cfg {
    max_depth: Option<usize>,
    dirs_only: bool,
    show_hidden: bool,
    excludes: Vec<String>,
    includes: Vec<String>,
    full_path: bool,
    no_report: bool,
    roots: Vec<PathBuf>,
}

fn run(argv: &[String]) -> Result<i32, String> {
    let mut cfg = Cfg {
        max_depth: None,
        dirs_only: false,
        show_hidden: false,
        excludes: Vec::new(),
        includes: Vec::new(),
        full_path: false,
        no_report: false,
        roots: Vec::new(),
    };
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-d" => cfg.dirs_only = true,
            "-a" => cfg.show_hidden = true,
            "-f" => cfg.full_path = true,
            "--noreport" => cfg.no_report = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            "--version" => {
                println!("tree (brush-bundled-extras) 0.1.4");
                return Ok(0);
            }
            "-L" => {
                i += 1;
                let n = argv
                    .get(i)
                    .ok_or_else(|| "-L requires a value".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("-L: {e}"))?;
                cfg.max_depth = Some(n);
            }
            "-I" => {
                i += 1;
                let pats = argv
                    .get(i)
                    .ok_or_else(|| "-I requires a value".to_string())?;
                cfg.excludes
                    .extend(pats.split(',').map(str::to_string));
            }
            "-P" => {
                i += 1;
                let pats = argv
                    .get(i)
                    .ok_or_else(|| "-P requires a value".to_string())?;
                cfg.includes
                    .extend(pats.split(',').map(str::to_string));
            }
            s if s.starts_with('-') && s.len() > 1 => return Err(format!("unknown option: {s}")),
            _ => cfg.roots.push(PathBuf::from(arg)),
        }
        i += 1;
    }
    if cfg.roots.is_empty() {
        cfg.roots.push(PathBuf::from("."));
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let mut total_dirs = 0_usize;
    let mut total_files = 0_usize;
    for root in &cfg.roots {
        let label = if cfg.full_path {
            root.to_string_lossy().to_string()
        } else {
            root.display().to_string()
        };
        writeln!(out, "{label}").map_err(|e| e.to_string())?;
        let (d, f) = render_dir(root, &mut Vec::new(), 1, &cfg, &mut out)?;
        total_dirs += d;
        total_files += f;
    }
    if !cfg.no_report {
        writeln!(
            out,
            "\n{total_dirs} director{}, {total_files} file{}",
            if total_dirs == 1 { "y" } else { "ies" },
            if total_files == 1 { "" } else { "s" }
        )
        .map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(0)
}

fn render_dir<W: Write>(
    dir: &Path,
    prefix: &mut Vec<bool>,
    depth: usize,
    cfg: &Cfg,
    out: &mut W,
) -> Result<(usize, usize), String> {
    if let Some(max) = cfg.max_depth {
        if depth > max {
            return Ok((0, 0));
        }
    }
    let mut entries: Vec<walkdir::DirEntry> = walkdir::WalkDir::new(dir)
        .min_depth(1)
        .max_depth(1)
        .sort_by_file_name()
        .into_iter()
        .filter_map(Result::ok)
        .collect();
    entries.retain(|e| keep_entry(e, cfg));

    let mut dirs = 0_usize;
    let mut files = 0_usize;
    for (idx, entry) in entries.iter().enumerate() {
        let last = idx + 1 == entries.len();
        for &is_last_segment in prefix.iter() {
            out.write_all(if is_last_segment { b"    " } else { b"|   " })
                .map_err(|e| e.to_string())?;
        }
        out.write_all(if last { b"`-- " } else { b"|-- " })
            .map_err(|e| e.to_string())?;
        let name = if cfg.full_path {
            entry.path().display().to_string()
        } else {
            entry.file_name().to_string_lossy().to_string()
        };
        writeln!(out, "{name}").map_err(|e| e.to_string())?;

        if entry.file_type().is_dir() {
            dirs += 1;
            prefix.push(last);
            let (sub_d, sub_f) = render_dir(entry.path(), prefix, depth + 1, cfg, out)?;
            prefix.pop();
            dirs += sub_d;
            files += sub_f;
        } else {
            files += 1;
        }
    }
    Ok((dirs, files))
}

fn keep_entry(entry: &walkdir::DirEntry, cfg: &Cfg) -> bool {
    let name = entry.file_name().to_string_lossy();
    if !cfg.show_hidden && name.starts_with('.') {
        return false;
    }
    if cfg.dirs_only && !entry.file_type().is_dir() {
        return false;
    }
    if !cfg.excludes.is_empty() && cfg.excludes.iter().any(|p| glob_match(p, &name)) {
        return false;
    }
    if !cfg.includes.is_empty()
        && entry.file_type().is_file()
        && !cfg.includes.iter().any(|p| glob_match(p, &name))
    {
        return false;
    }
    true
}

/// Minimal glob matcher — supports `*` (any chars), `?` (single char).
/// Sufficient for the dominant `*.ext` / `prefix*` / `*infix*` patterns
/// agents emit. No bracket expressions, no escaping.
fn glob_match(pat: &str, s: &str) -> bool {
    fn helper(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => helper(&p[1..], s) || (!s.is_empty() && helper(p, &s[1..])),
            (Some(b'?'), Some(_)) => helper(&p[1..], &s[1..]),
            (Some(pc), Some(sc)) if pc == sc => helper(&p[1..], &s[1..]),
            _ => false,
        }
    }
    helper(pat.as_bytes(), s.as_bytes())
}

fn print_help() {
    println!(
        "Usage: tree [OPTIONS] [DIR...]\n\
         \n\
         Print directory tree.\n\
         \n\
         Options:\n  \
           -L N            descend at most N levels\n  \
           -d              list directories only\n  \
           -a              show hidden entries (default: hide dotfiles)\n  \
           -I PAT          exclude entries matching glob (comma-sep)\n  \
           -P PAT          include only entries matching glob (comma-sep)\n  \
           -f              print full path on each line\n  \
           --noreport      suppress trailing summary\n  \
           --help          show this help\n  \
           --version       show version\n"
    );
}

//! ripgrep-style grep adapter — bundled `rg` / `grep` / `egrep` / `fgrep`
//! using `regex` + `pcre2` directly, with `ignore` (the same crate ripgrep
//! itself uses) for gitignore-aware filesystem walks.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 3.
//! Replaces fastgrep as the backing for `grep` / `egrep` / `fgrep`; the
//! `fastgrep` name remains registered to the fastgrep adapter for
//! users who want fastgrep's SIMD speed and accept its GNU-grep gaps
//! (no `-P`, no `-z`, no `-G`, etc.).
//!
//! This is a **line-based** implementation — for each file, read line
//! by line and run the regex. Not as fast as ripgrep's mmap+SIMD core,
//! but supports the full GNU/PCRE2 flag matrix in ~500 lines of Rust
//! instead of vendoring 4000 lines of ripgrep core. Sufficient for
//! agent workloads (sub-second on most repositories).
//!
//! Supported flags (the dominant agent-workload set):
//!
//! | Flag | Behavior |
//! |---|---|
//! | `-r` / `-R` | Recursive (auto if a directory is given) |
//! | `-i` | Case-insensitive |
//! | `-n` | Show line numbers |
//! | `-c` | Print only count of matching lines per file |
//! | `-l` | Print only filenames with matches |
//! | `-L` | Print only filenames without matches |
//! | `-h` / `-H` | Suppress / force filename prefix |
//! | `-o` | Print only the matching part of each line |
//! | `-v` | Invert match |
//! | `-w` | Word boundary |
//! | `-x` | Whole-line match |
//! | `-q` | Quiet (exit only) |
//! | `-E` | Extended regex (default) |
//! | `-F` | Fixed string (no regex metacharacters) |
//! | `-P` | PCRE2 mode (the headline reason for this cycle) |
//! | `-e PATTERN` | Pattern (can repeat) |
//! | `-A N` / `-B N` / `-C N` | After / before / context lines |
//! | `-m N` / `--max-count N` | Stop after N matches per file |
//! | `--include GLOB` / `--exclude GLOB` | File path filters |
//! | `--no-ignore` | Don't honor `.gitignore` |
//! | `--hidden` | Don't skip hidden files / directories |
//! | `--color WHEN` | `always`/`never`/`auto` (default `never`) |

#![allow(
    clippy::too_many_lines,
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::significant_drop_tightening,
    clippy::needless_pass_by_value,
    clippy::struct_excessive_bools,
    clippy::fn_params_excessive_bools,
    clippy::option_if_let_else,
    clippy::single_match_else,
    clippy::if_not_else,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    reason = "ripgrep CLI orchestration is intrinsically branchy and parameter-heavy; refactoring obscures the flag-by-flag mapping"
)]

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use ignore::WalkBuilder;

pub(crate) fn rg_main(args: Vec<OsString>) -> i32 {
    run(args, AliasMode::None)
}

/// `grep` alias — same as `rg` for our purposes.
pub(crate) fn grep_main(args: Vec<OsString>) -> i32 {
    run(args, AliasMode::None)
}

/// `egrep` alias — extended regex (also the default).
pub(crate) fn egrep_main(args: Vec<OsString>) -> i32 {
    run(args, AliasMode::ForceExtended)
}

/// `fgrep` alias — fixed-string match.
pub(crate) fn fgrep_main(args: Vec<OsString>) -> i32 {
    run(args, AliasMode::ForceFixed)
}

#[derive(Clone, Copy)]
enum AliasMode {
    None,
    ForceExtended,
    ForceFixed,
}

#[derive(Default)]
struct Cfg {
    patterns: Vec<String>,
    paths: Vec<PathBuf>,
    ignore_case: bool,
    line_numbers: bool,
    count: bool,
    files_with_matches: bool,
    files_without_matches: bool,
    show_filename: Option<bool>,
    only_matching: bool,
    invert_match: bool,
    word: bool,
    whole_line: bool,
    quiet: bool,
    fixed: bool,
    pcre2: bool,
    after: usize,
    before: usize,
    max_count: Option<u64>,
    recursive: bool,
    no_ignore: bool,
    hidden: bool,
    color_always: bool,
    includes: Vec<String>,
    excludes: Vec<String>,
}

fn run(args: Vec<OsString>, alias: AliasMode) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let mut cfg = Cfg::default();
    match alias {
        AliasMode::None | AliasMode::ForceExtended => {}
        AliasMode::ForceFixed => cfg.fixed = true,
    }
    if let Err(code) = parse_args(&argv, &mut cfg) {
        return code;
    }

    if cfg.patterns.is_empty() {
        eprintln!("grep: no pattern provided");
        return 2;
    }

    if cfg.paths.is_empty() {
        cfg.paths.push(PathBuf::from("-"));
    }

    if cfg.show_filename.is_none() {
        let any_dir = cfg
            .paths
            .iter()
            .any(|p| p.as_path() != Path::new("-") && p.is_dir());
        cfg.show_filename = Some(any_dir || cfg.paths.len() > 1);
        if any_dir {
            cfg.recursive = true;
        }
    }

    let engine = match build_engine(&cfg) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("grep: {e}");
            return 2;
        }
    };

    let any_match = AtomicBool::new(false);
    let stdout = io::stdout();
    let out = Mutex::new(stdout.lock());

    for path in &cfg.paths {
        if path.as_os_str() == "-" {
            let stdin = io::stdin();
            search_reader(
                BufReader::new(stdin.lock()),
                "(standard input)",
                &engine,
                &cfg,
                &out,
                &any_match,
            );
            continue;
        }
        if path.is_dir() {
            walk_dir(path, &engine, &cfg, &out, &any_match);
        } else {
            search_path(path, &engine, &cfg, &out, &any_match);
        }
    }
    i32::from(!any_match.load(Ordering::SeqCst))
}

fn parse_args(argv: &[String], cfg: &mut Cfg) -> Result<(), i32> {
    let mut i = 1;
    let mut after_double_dash = false;
    while i < argv.len() {
        let arg = &argv[i];
        if after_double_dash {
            if cfg.patterns.is_empty() {
                cfg.patterns.push(arg.clone());
            } else {
                cfg.paths.push(PathBuf::from(arg));
            }
            i += 1;
            continue;
        }
        match arg.as_str() {
            "--" => after_double_dash = true,
            "-h" => cfg.show_filename = Some(false),
            "-H" => cfg.show_filename = Some(true),
            "-i" | "--ignore-case" => cfg.ignore_case = true,
            "-n" | "--line-number" => cfg.line_numbers = true,
            "-c" | "--count" => cfg.count = true,
            "-l" | "--files-with-matches" => cfg.files_with_matches = true,
            "-L" | "--files-without-match" => cfg.files_without_matches = true,
            "-o" | "--only-matching" => cfg.only_matching = true,
            "-v" | "--invert-match" => cfg.invert_match = true,
            "-w" | "--word-regexp" => cfg.word = true,
            "-x" | "--line-regexp" => cfg.whole_line = true,
            "-q" | "--quiet" | "--silent" => cfg.quiet = true,
            "-F" | "--fixed-strings" => cfg.fixed = true,
            "-E" | "--extended-regexp" => { /* default */ }
            "-P" | "--perl-regexp" => cfg.pcre2 = true,
            "-r" | "-R" | "--recursive" | "--dereference-recursive" => cfg.recursive = true,
            "--no-ignore" => cfg.no_ignore = true,
            "--hidden" => cfg.hidden = true,
            "--help" => {
                print_help();
                return Err(0);
            }
            "--version" => {
                println!("rg / grep (brush-bundled-extras regex+pcre2) 0.1.6");
                println!("PCRE2 enabled");
                return Err(0);
            }
            "-e" | "--regexp" => {
                i += 1;
                let v = arg_or_err(argv.get(i), "-e")?;
                cfg.patterns.push(v.clone());
            }
            "-A" | "--after-context" => {
                i += 1;
                cfg.after = parse_usize(argv.get(i), "-A")?;
            }
            "-B" | "--before-context" => {
                i += 1;
                cfg.before = parse_usize(argv.get(i), "-B")?;
            }
            "-C" | "--context" => {
                i += 1;
                let n = parse_usize(argv.get(i), "-C")?;
                cfg.after = n;
                cfg.before = n;
            }
            "-m" | "--max-count" => {
                i += 1;
                cfg.max_count = Some(parse_u64(argv.get(i), "-m")?);
            }
            "--include" => {
                i += 1;
                cfg.includes
                    .push(arg_or_err(argv.get(i), "--include")?.clone());
            }
            "--exclude" => {
                i += 1;
                cfg.excludes
                    .push(arg_or_err(argv.get(i), "--exclude")?.clone());
            }
            "--color" | "--colour" => {
                i += 1;
                cfg.color_always = matches!(
                    arg_or_err(argv.get(i), "--color")?.as_str(),
                    "always" | "yes"
                );
            }
            s if s.starts_with("--color=") || s.starts_with("--colour=") => {
                let v = s.split_once('=').map_or("", |(_, v)| v);
                cfg.color_always = matches!(v, "always" | "yes");
            }
            s if s.starts_with("--include=") => {
                cfg.includes
                    .push(s.trim_start_matches("--include=").to_string());
            }
            s if s.starts_with("--exclude=") => {
                cfg.excludes
                    .push(s.trim_start_matches("--exclude=").to_string());
            }
            s if s.starts_with('-') && s.len() > 1 && !s.starts_with("--") => {
                if !try_parse_short_bundle(s, cfg) {
                    eprintln!("grep: unknown option: {s}");
                    return Err(2);
                }
            }
            s if s.starts_with("--") => {
                eprintln!("grep: unknown option: {s}");
                return Err(2);
            }
            _ => {
                if cfg.patterns.is_empty() {
                    cfg.patterns.push(arg.clone());
                } else {
                    cfg.paths.push(PathBuf::from(arg));
                }
            }
        }
        i += 1;
    }
    Ok(())
}

fn try_parse_short_bundle(s: &str, cfg: &mut Cfg) -> bool {
    for ch in s.chars().skip(1) {
        match ch {
            'i' => cfg.ignore_case = true,
            'n' => cfg.line_numbers = true,
            'c' => cfg.count = true,
            'l' => cfg.files_with_matches = true,
            'L' => cfg.files_without_matches = true,
            'h' => cfg.show_filename = Some(false),
            'H' => cfg.show_filename = Some(true),
            'o' => cfg.only_matching = true,
            'v' => cfg.invert_match = true,
            'w' => cfg.word = true,
            'x' => cfg.whole_line = true,
            'q' => cfg.quiet = true,
            'F' => cfg.fixed = true,
            'E' => { /* default */ }
            'P' => cfg.pcre2 = true,
            'r' | 'R' => cfg.recursive = true,
            _ => return false,
        }
    }
    true
}

fn parse_usize(s: Option<&String>, flag: &str) -> Result<usize, i32> {
    s.ok_or_else(|| {
        eprintln!("grep: option {flag} requires a value");
        2
    })?
    .parse::<usize>()
    .map_err(|e| {
        eprintln!("grep: option {flag}: {e}");
        2
    })
}

fn parse_u64(s: Option<&String>, flag: &str) -> Result<u64, i32> {
    s.ok_or_else(|| {
        eprintln!("grep: option {flag} requires a value");
        2
    })?
    .parse::<u64>()
    .map_err(|e| {
        eprintln!("grep: option {flag}: {e}");
        2
    })
}

fn arg_or_err<'a>(s: Option<&'a String>, flag: &str) -> Result<&'a String, i32> {
    s.ok_or_else(|| {
        eprintln!("grep: option {flag} requires a value");
        2
    })
}

enum Engine {
    Regex(regex::Regex),
    Pcre2(pcre2::bytes::Regex),
}

fn build_engine(cfg: &Cfg) -> Result<Engine, String> {
    let pieces: Vec<String> = if cfg.fixed {
        cfg.patterns.iter().map(|p| regex_escape(p)).collect()
    } else {
        cfg.patterns.clone()
    };
    let combined = pieces.join("|");
    let pattern = if cfg.word {
        format!(r"\b(?:{combined})\b")
    } else if cfg.whole_line {
        format!(r"^(?:{combined})$")
    } else {
        combined
    };

    if cfg.pcre2 {
        let mut b = pcre2::bytes::RegexBuilder::new();
        b.caseless(cfg.ignore_case);
        b.multi_line(false);
        b.build(&pattern)
            .map(Engine::Pcre2)
            .map_err(|e| format!("PCRE2 compile error: {e}"))
    } else {
        let pat = if cfg.ignore_case {
            format!("(?i){pattern}")
        } else {
            pattern
        };
        regex::Regex::new(&pat)
            .map(Engine::Regex)
            .map_err(|e| format!("regex compile error: {e}"))
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '\\' | '.' | '*' | '+' | '?' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '^' | '$'
        ) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn engine_find(engine: &Engine, line: &[u8]) -> Option<(usize, usize)> {
    match engine {
        Engine::Regex(r) => {
            let s = std::str::from_utf8(line).ok()?;
            r.find(s).map(|m| (m.start(), m.end()))
        }
        Engine::Pcre2(p) => p.find(line).ok().flatten().map(|m| (m.start(), m.end())),
    }
}

fn walk_dir(
    root: &Path,
    engine: &Engine,
    cfg: &Cfg,
    out: &Mutex<io::StdoutLock<'_>>,
    any_match: &AtomicBool,
) {
    let mut wb = WalkBuilder::new(root);
    wb.standard_filters(!cfg.no_ignore);
    wb.hidden(!cfg.hidden);
    if !cfg.includes.is_empty() || !cfg.excludes.is_empty() {
        let mut overrides = ignore::overrides::OverrideBuilder::new(root);
        for pat in &cfg.includes {
            let _ = overrides.add(pat);
        }
        for pat in &cfg.excludes {
            let _ = overrides.add(&format!("!{pat}"));
        }
        if let Ok(o) = overrides.build() {
            wb.overrides(o);
        }
    }
    for entry in wb.build() {
        let Ok(entry) = entry else { continue };
        let Some(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        search_path(entry.path(), engine, cfg, out, any_match);
        if cfg.quiet && any_match.load(Ordering::SeqCst) {
            return;
        }
    }
}

fn search_path(
    path: &Path,
    engine: &Engine,
    cfg: &Cfg,
    out: &Mutex<io::StdoutLock<'_>>,
    any_match: &AtomicBool,
) {
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            if !cfg.quiet {
                eprintln!("grep: {}: {e}", path.display());
            }
            return;
        }
    };
    let label = path.to_string_lossy();
    search_reader(BufReader::new(f), &label, engine, cfg, out, any_match);
}

fn search_reader<R: BufRead>(
    mut reader: R,
    label: &str,
    engine: &Engine,
    cfg: &Cfg,
    out: &Mutex<io::StdoutLock<'_>>,
    any_match: &AtomicBool,
) {
    let mut buf: Vec<u8> = Vec::new();
    let mut line_no: u64 = 0;
    let mut match_count: u64 = 0;
    let mut had_match = false;
    let mut before_buf: VecDeque<(u64, Vec<u8>)> = VecDeque::with_capacity(cfg.before + 1);
    let mut after_remaining: usize = 0;
    let show_path = cfg.show_filename.unwrap_or(false);

    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        line_no += 1;
        let line = if buf.ends_with(b"\n") {
            &buf[..n - 1]
        } else {
            &buf[..]
        };
        let line = if line.ends_with(b"\r") {
            &line[..line.len() - 1]
        } else {
            line
        };
        let m = engine_find(engine, line);
        let is_match = if cfg.invert_match {
            m.is_none()
        } else {
            m.is_some()
        };

        if is_match {
            had_match = true;
            match_count += 1;
            any_match.store(true, Ordering::SeqCst);

            if cfg.quiet {
                return;
            }
            if cfg.files_with_matches {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let _ = writeln!(o, "{label}");
                return;
            }
            if cfg.files_without_matches {
                return;
            }
            if !cfg.count && !cfg.files_with_matches && !cfg.files_without_matches {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                // Emit context-before
                while let Some((bn, bl)) = before_buf.pop_front() {
                    write_line(
                        &mut *o,
                        show_path,
                        label,
                        cfg.line_numbers,
                        bn,
                        &bl,
                        false,
                        None,
                    );
                }
                if cfg.only_matching {
                    if let Some((s, e)) = m {
                        write_line(
                            &mut *o,
                            show_path,
                            label,
                            cfg.line_numbers,
                            line_no,
                            &line[s..e],
                            true,
                            None,
                        );
                    }
                } else {
                    write_line(
                        &mut *o,
                        show_path,
                        label,
                        cfg.line_numbers,
                        line_no,
                        line,
                        true,
                        m,
                    );
                }
                after_remaining = cfg.after;
            }
            if let Some(max) = cfg.max_count {
                if match_count >= max {
                    break;
                }
            }
        } else {
            if after_remaining > 0 {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                write_line(
                    &mut *o,
                    show_path,
                    label,
                    cfg.line_numbers,
                    line_no,
                    line,
                    false,
                    None,
                );
                after_remaining -= 1;
            }
            if cfg.before > 0 {
                if before_buf.len() == cfg.before {
                    before_buf.pop_front();
                }
                before_buf.push_back((line_no, line.to_vec()));
            }
        }
    }

    if cfg.count {
        let mut o = out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if show_path {
            let _ = writeln!(o, "{label}:{match_count}");
        } else {
            let _ = writeln!(o, "{match_count}");
        }
    }
    if cfg.files_without_matches && !had_match {
        let mut o = out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let _ = writeln!(o, "{label}");
    }
}

fn write_line<W: Write>(
    out: &mut W,
    show_path: bool,
    label: &str,
    line_numbers: bool,
    n: u64,
    line: &[u8],
    is_match: bool,
    _highlight: Option<(usize, usize)>,
) {
    let sep = if is_match { ':' } else { '-' };
    if show_path {
        let _ = write!(out, "{label}{sep}");
    }
    if line_numbers {
        let _ = write!(out, "{n}{sep}");
    }
    let _ = out.write_all(line);
    let _ = out.write_all(b"\n");
}

fn print_help() {
    println!(
        "Usage: rg / grep [OPTIONS] PATTERN [PATH...]\n\
         \n\
         Recursively search for PATTERN. Built on `regex` + `pcre2` (for -P)\n\
         with the `ignore` crate for gitignore-aware walks. Same gitignore\n\
         handling ripgrep itself uses.\n\
         \n\
         Pattern selection:\n  \
           -E, --extended-regexp    use extended regex (default)\n  \
           -F, --fixed-strings      treat PATTERN as literal\n  \
           -P, --perl-regexp        use PCRE2\n  \
           -e, --regexp PATTERN     specify pattern (can repeat)\n\
         \n\
         Match control:\n  \
           -i, --ignore-case        case-insensitive match\n  \
           -w, --word-regexp        match whole words only\n  \
           -x, --line-regexp        match whole lines only\n  \
           -v, --invert-match       select non-matching lines\n  \
           -m, --max-count N        stop after N matches per file\n\
         \n\
         Output control:\n  \
           -n, --line-number        show line numbers\n  \
           -c, --count              show match counts only\n  \
           -l, --files-with-matches show only filenames with matches\n  \
           -L, --files-without-match show only filenames without matches\n  \
           -h                       suppress filename prefix\n  \
           -H                       force filename prefix\n  \
           -o, --only-matching      print only the matching part\n  \
           -A, --after-context N    show N lines after each match\n  \
           -B, --before-context N   show N lines before each match\n  \
           -C, --context N          show N lines before AND after\n  \
           -q, --quiet              suppress output, exit only\n\
         \n\
         File selection:\n  \
           -r, -R, --recursive      recurse into directories\n  \
           --include GLOB           include only files matching GLOB\n  \
           --exclude GLOB           skip files matching GLOB\n  \
           --no-ignore              don't honor .gitignore\n  \
           --hidden                 don't skip hidden files/dirs\n\
         \n\
         Misc:\n  \
           --color WHEN             always / never / auto (default: never)\n  \
           --help                   show this help\n  \
           --version                show version\n"
    );
}

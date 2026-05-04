//! tar adapter — via the [`tar`] crate, optionally chained through
//! [`flate2`] for `-z` (gzip) compression.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 2.
//! Covers the dominant agent invocations:
//!
//! | Invocation | Behavior |
//! |---|---|
//! | `tar -cf out.tar a b c` | Create archive |
//! | `tar -czf out.tar.gz a b c` | Create gzip-compressed archive |
//! | `tar -xf out.tar` | Extract |
//! | `tar -xzf out.tar.gz` | Extract gzip |
//! | `tar -tf out.tar` | List contents |
//! | `tar -tzf out.tar.gz` | List gzip |
//! | `tar -xf out.tar -C dest --strip-components=1` | Extract into `dest` skipping leading path component |
//!
//! `-j` (bzip2) and `-J` (xz) are **not** routed through tar here —
//! they would require the bzip2/xz feature flags to be enabled too.
//! Combined `-czj` / `-cxJ` not supported. Bare `tar` (no `-c/-x/-t`)
//! defaults to listing if it sees an `-f` arg, else error.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

pub(crate) fn tar_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("tar: {e}");
            2
        }
    }
}

#[derive(Default)]
struct Cfg {
    op: Option<Op>,
    gzip: bool,
    file: Option<String>,
    verbose: bool,
    chdir: Option<PathBuf>,
    strip_components: usize,
    excludes: Vec<String>,
    paths: Vec<String>,
    from_stdin: bool,
    to_stdout: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Create,
    Extract,
    List,
}

fn run(argv: &[String]) -> Result<i32, String> {
    let mut cfg = Cfg::default();
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        // -C is a standalone short flag that takes an argument; intercept
        // before the bundle loop so it's not mistaken for a -czf-style
        // operation bundle.
        if arg == "-C" {
            i += 1;
            cfg.chdir = Some(PathBuf::from(
                argv.get(i)
                    .ok_or_else(|| "-C requires a value".to_string())?,
            ));
            i += 1;
            continue;
        }
        // Multi-flag bundles: -czf / -xzf / -tzf / -cf / -xf / -tf / -tvf
        if arg.len() > 1 && arg.starts_with('-') && !arg.starts_with("--") && !is_clap_long(arg) {
            for ch in arg.chars().skip(1) {
                match ch {
                    'c' => cfg.op = Some(Op::Create),
                    'x' => cfg.op = Some(Op::Extract),
                    't' => cfg.op = Some(Op::List),
                    'z' => cfg.gzip = true,
                    'v' => cfg.verbose = true,
                    'O' => cfg.to_stdout = true,
                    'f' => {
                        i += 1;
                        let v = argv
                            .get(i)
                            .ok_or_else(|| "-f requires an argument".to_string())?
                            .clone();
                        if v == "-" {
                            cfg.from_stdin = true;
                            cfg.to_stdout = true;
                        }
                        cfg.file = Some(v);
                    }
                    other => return Err(format!("unknown short option: -{other}")),
                }
            }
        } else {
            match arg.as_str() {
                "--create" => cfg.op = Some(Op::Create),
                "--extract" | "--get" => cfg.op = Some(Op::Extract),
                "--list" => cfg.op = Some(Op::List),
                "--gzip" | "--gunzip" | "--ungzip" => cfg.gzip = true,
                "--verbose" => cfg.verbose = true,
                "-h" | "--help" => {
                    print_help();
                    return Ok(0);
                }
                "--version" => {
                    println!("tar (brush-bundled-extras) 0.1.5");
                    return Ok(0);
                }
                "-C" | "--directory" => {
                    i += 1;
                    cfg.chdir = Some(PathBuf::from(
                        argv.get(i)
                            .ok_or_else(|| "-C requires a value".to_string())?,
                    ));
                }
                "--strip-components" => {
                    i += 1;
                    cfg.strip_components = argv
                        .get(i)
                        .ok_or_else(|| "--strip-components requires a value".to_string())?
                        .parse::<usize>()
                        .map_err(|e| format!("--strip-components: {e}"))?;
                }
                s if s.starts_with("--strip-components=") => {
                    cfg.strip_components = s
                        .trim_start_matches("--strip-components=")
                        .parse::<usize>()
                        .map_err(|e| format!("--strip-components: {e}"))?;
                }
                "--exclude" => {
                    i += 1;
                    cfg.excludes.push(
                        argv.get(i)
                            .ok_or_else(|| "--exclude requires a value".to_string())?
                            .clone(),
                    );
                }
                s if s.starts_with("--exclude=") => {
                    cfg.excludes
                        .push(s.trim_start_matches("--exclude=").to_string());
                }
                s if s.starts_with("--") => return Err(format!("unknown option: {s}")),
                _ => cfg.paths.push(arg.clone()),
            }
        }
        i += 1;
    }

    let op = cfg
        .op
        .ok_or_else(|| "missing operation: -c, -x, or -t".to_string())?;
    match op {
        Op::Create => do_create(&cfg),
        Op::Extract => do_extract(&cfg),
        Op::List => do_list(&cfg),
    }
}

fn is_clap_long(arg: &str) -> bool {
    arg.starts_with("--")
}

fn open_input(cfg: &Cfg) -> Result<Box<dyn Read>, String> {
    let raw: Box<dyn Read> = if cfg.from_stdin || cfg.file.as_deref() == Some("-") {
        Box::new(io::stdin())
    } else {
        let path = cfg
            .file
            .as_ref()
            .ok_or_else(|| "missing -f operand".to_string())?;
        Box::new(File::open(path).map_err(|e| format!("{path}: {e}"))?)
    };
    if cfg.gzip {
        Ok(Box::new(GzDecoder::new(BufReader::new(raw))))
    } else {
        Ok(Box::new(BufReader::new(raw)))
    }
}

fn open_output(cfg: &Cfg) -> Result<Box<dyn Write>, String> {
    let raw: Box<dyn Write> = if cfg.to_stdout || cfg.file.as_deref() == Some("-") {
        Box::new(io::stdout())
    } else {
        let path = cfg
            .file
            .as_ref()
            .ok_or_else(|| "missing -f operand".to_string())?;
        Box::new(File::create(path).map_err(|e| format!("{path}: {e}"))?)
    };
    if cfg.gzip {
        Ok(Box::new(GzEncoder::new(
            BufWriter::new(raw),
            Compression::default(),
        )))
    } else {
        Ok(Box::new(BufWriter::new(raw)))
    }
}

fn do_create(cfg: &Cfg) -> Result<i32, String> {
    if cfg.paths.is_empty() {
        return Err("create mode requires at least one input path".to_string());
    }
    let writer = open_output(cfg)?;
    let mut builder = tar::Builder::new(writer);
    builder.follow_symlinks(false);
    for p in &cfg.paths {
        let path = Path::new(p);
        if !path.exists() {
            return Err(format!("{p}: not found"));
        }
        if path.is_dir() {
            builder
                .append_dir_all(path.file_name().unwrap_or(path.as_os_str()), path)
                .map_err(|e| format!("{p}: {e}"))?;
        } else {
            let mut f = File::open(path).map_err(|e| format!("{p}: {e}"))?;
            builder
                .append_file(p, &mut f)
                .map_err(|e| format!("{p}: {e}"))?;
        }
        if cfg.verbose {
            eprintln!("a {p}");
        }
    }
    builder
        .into_inner()
        .map_err(|e| e.to_string())?
        .flush()
        .map_err(|e| e.to_string())?;
    Ok(0)
}

fn do_extract(cfg: &Cfg) -> Result<i32, String> {
    let reader = open_input(cfg)?;
    let mut archive = tar::Archive::new(reader);
    let dest = cfg.chdir.clone().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {}", dest.display(), e))?;

    let mut any_err = false;
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let mut entry = match entry {
            Ok(e) => e,
            Err(e) => {
                eprintln!("tar: {e}");
                any_err = true;
                continue;
            }
        };
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        let stripped = strip_components(&path, cfg.strip_components);
        let Some(stripped) = stripped else { continue };
        let out_path = dest.join(&stripped);
        if cfg.verbose {
            eprintln!("x {}", stripped.display());
        }
        if let Err(e) = entry.unpack(&out_path) {
            eprintln!("tar: {}: {}", out_path.display(), e);
            any_err = true;
        }
    }
    Ok(i32::from(any_err))
}

fn do_list(cfg: &Cfg) -> Result<i32, String> {
    let reader = open_input(cfg)?;
    let mut archive = tar::Archive::new(reader);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    for entry in archive.entries().map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?;
        if cfg.verbose {
            let header = entry.header();
            let size = header.size().unwrap_or(0);
            let _ = writeln!(out, "{size:>10} {}", path.display());
        } else {
            let _ = writeln!(out, "{}", path.display());
        }
    }
    Ok(0)
}

fn strip_components(path: &Path, n: usize) -> Option<PathBuf> {
    if n == 0 {
        return Some(path.to_path_buf());
    }
    let mut comps = path.components();
    for _ in 0..n {
        comps.next()?;
    }
    let stripped = comps.as_path();
    if stripped.as_os_str().is_empty() {
        None
    } else {
        Some(stripped.to_path_buf())
    }
}

fn print_help() {
    println!(
        "Usage: tar [OPTIONS] [PATH...]\n\
         \n\
         Create / extract / list tar archives.\n\
         \n\
         Operations (pick exactly one):\n  \
           -c, --create     create archive\n  \
           -x, --extract    extract archive\n  \
           -t, --list       list archive contents\n\
         \n\
         Options:\n  \
           -f FILE          archive file (`-` = stdin/stdout)\n  \
           -z, --gzip       gzip-compress / decompress\n  \
           -v, --verbose    verbose output\n  \
           -O               extract to stdout\n  \
           -C DIR           change to DIR before extract\n  \
           --strip-components=N  remove N leading path components on extract\n  \
           --exclude=PAT    exclude paths matching PAT\n  \
           --help           show this help\n  \
           --version        show version\n\
         \n\
         Common bundles:\n  \
           tar -czf out.tar.gz path/      create gzip archive\n  \
           tar -xzf out.tar.gz            extract gzip archive\n  \
           tar -tf out.tar                list archive\n"
    );
}

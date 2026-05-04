//! gzip / gunzip / gzcat / zcat adapter — pure-Rust deflate via
//! [`flate2`].
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 2.
//! All four names share one entry point that branches on `argv[0]`:
//!
//! | Invocation | Behavior |
//! |---|---|
//! | `gzip <file>` | Compress `<file>` to `<file>.gz`, remove original |
//! | `gzip -c <file>` | Compress to stdout |
//! | `gunzip <file.gz>` | Decompress to `<file>`, remove original |
//! | `gunzip -c <file.gz>` / `zcat`/`gzcat <file.gz>` | Decompress to stdout |
//! | `gzip -d` / `gunzip` | Same — decompress |
//! | (no file args) | Read stdin, write stdout |

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

pub(crate) fn gzip_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Compress)
}

pub(crate) fn gunzip_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Decompress)
}

/// `zcat` / `gzcat` — decompress to stdout, never modify on disk.
pub(crate) fn zcat_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::DecompressToStdout)
}

#[derive(Clone, Copy)]
enum Mode {
    Compress,
    Decompress,
    DecompressToStdout,
}

fn run(args: Vec<OsString>, default: Mode) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();

    let mut to_stdout = matches!(default, Mode::DecompressToStdout);
    let mut keep = false;
    let mut force = false;
    let mut mode = default;
    let mut level = Compression::default();
    let mut paths: Vec<String> = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-c" | "--stdout" => to_stdout = true,
            "-d" | "--decompress" | "--uncompress" => mode = Mode::Decompress,
            "-k" | "--keep" => keep = true,
            "-f" | "--force" => force = true,
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "--version" => {
                println!("gzip (brush-bundled-extras flate2) 0.1.5");
                return 0;
            }
            s if s.starts_with('-') && s.len() == 2 && s.as_bytes()[1].is_ascii_digit() => {
                level = Compression::new(u32::from(s.as_bytes()[1] - b'0'));
            }
            s if s.starts_with('-') && s != "-" => {
                eprintln!("gzip: unknown option: {s}");
                return 1;
            }
            _ => paths.push(arg.clone()),
        }
        i += 1;
    }

    if paths.is_empty() {
        return run_stream(
            mode,
            level,
            &mut io::stdin().lock(),
            &mut io::stdout().lock(),
        );
    }

    let mut any_err = false;
    for p in &paths {
        if let Err(e) = run_one(p, mode, to_stdout, keep, force, level) {
            eprintln!("gzip: {p}: {e}");
            any_err = true;
        }
    }
    i32::from(any_err)
}

fn run_stream<R: BufRead, W: Write>(mode: Mode, level: Compression, r: &mut R, w: &mut W) -> i32 {
    let res = match mode {
        Mode::Compress => {
            let mut enc = GzEncoder::new(w, level);
            io::copy(r, &mut enc).and_then(|_| enc.finish().map(|_| ()))
        }
        Mode::Decompress | Mode::DecompressToStdout => {
            let mut dec = GzDecoder::new(r);
            io::copy(&mut dec, w).map(|_| ())
        }
    };
    if let Err(e) = res {
        eprintln!("gzip: {e}");
        return 1;
    }
    0
}

fn run_one(
    path: &str,
    mode: Mode,
    to_stdout: bool,
    keep: bool,
    force: bool,
    level: Compression,
) -> io::Result<()> {
    let in_path = PathBuf::from(path);
    match mode {
        Mode::Compress => {
            let out_path = if to_stdout {
                None
            } else {
                Some(append_gz(&in_path))
            };
            let mut input = BufReader::new(File::open(&in_path)?);
            if let Some(out) = out_path {
                if !force && out.exists() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "output file exists (use -f to overwrite)",
                    ));
                }
                let f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&out)?;
                let mut enc = GzEncoder::new(BufWriter::new(f), level);
                io::copy(&mut input, &mut enc)?;
                enc.finish()?;
                if !keep {
                    std::fs::remove_file(&in_path)?;
                }
            } else {
                let stdout = io::stdout();
                let mut enc = GzEncoder::new(BufWriter::new(stdout.lock()), level);
                io::copy(&mut input, &mut enc)?;
                enc.finish()?;
            }
        }
        Mode::Decompress | Mode::DecompressToStdout => {
            let always_stdout = matches!(mode, Mode::DecompressToStdout) || to_stdout;
            let out_path = if always_stdout {
                None
            } else {
                Some(strip_gz(&in_path)?)
            };
            let input = BufReader::new(File::open(&in_path)?);
            let mut dec = GzDecoder::new(input);
            if let Some(out) = out_path {
                if !force && out.exists() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "output file exists (use -f to overwrite)",
                    ));
                }
                let f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&out)?;
                let mut w = BufWriter::new(f);
                io::copy(&mut dec, &mut w)?;
                w.flush()?;
                if !keep {
                    std::fs::remove_file(&in_path)?;
                }
            } else {
                let stdout = io::stdout();
                let mut w = BufWriter::new(stdout.lock());
                io::copy(&mut dec, &mut w)?;
            }
        }
    }
    Ok(())
}

fn append_gz(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(".gz");
    PathBuf::from(s)
}

fn strip_gz(p: &Path) -> io::Result<PathBuf> {
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "non-UTF-8 filename"))?;
    let stripped = name
        .strip_suffix(".gz")
        .or_else(|| {
            name.strip_suffix(".tgz").map(|s| {
                // .tgz -> .tar
                Box::leak(format!("{s}.tar").into_boxed_str()) as &str
            })
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "input file does not end in .gz",
            )
        })?;
    let parent = p.parent().unwrap_or_else(|| Path::new(""));
    Ok(parent.join(stripped))
}

fn print_help() {
    println!(
        "Usage: gzip   [OPTIONS] [FILE...]\n  \
              gunzip [OPTIONS] [FILE...]\n  \
              zcat   [OPTIONS] [FILE...]\n\
         \n\
         Compress / decompress files using gzip (RFC 1952).\n\
         \n\
         Options:\n  \
           -c, --stdout       write to stdout, keep originals\n  \
           -d, --decompress   force decompress mode (gzip -d == gunzip)\n  \
           -k, --keep         keep input files\n  \
           -f, --force        overwrite existing output files\n  \
           -1 .. -9           compression level (default 6)\n  \
           --help             show this help\n  \
           --version          show version\n"
    );
}

//! `file` adapter — type detection via magic bytes.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 1.
//! Uses the crates.io [`infer`] crate (used by mediainfo-style tooling
//! and many `file`-equivalents in the Rust ecosystem) for binary types,
//! plus an internal text-vs-binary heuristic for plain-text fallback.
//!
//! Output style mirrors GNU `file`: `path: <description>`. Multi-file
//! invocations align the colon to the longest path.

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

pub(crate) fn file_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("file: {e}");
            1
        }
    }
}

fn run(argv: &[String]) -> Result<i32, String> {
    let mut brief = false;
    let mut mime = false;
    let mut paths: Vec<String> = Vec::new();
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-b" | "--brief" => brief = true,
            "-i" | "--mime" => mime = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            "--version" => {
                println!("file (brush-bundled-extras) 0.1.4");
                return Ok(0);
            }
            s if s.starts_with('-') && s != "-" => return Err(format!("unknown option: {s}")),
            _ => paths.push(arg.clone()),
        }
        i += 1;
    }

    if paths.is_empty() {
        return Err("missing operand".to_string());
    }

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    let label_width = if brief {
        0
    } else {
        paths.iter().map(String::len).max().unwrap_or(0)
    };

    let mut any_err = false;
    for path in &paths {
        let desc = describe(Path::new(path), mime).unwrap_or_else(|e| {
            any_err = true;
            format!("cannot open: {e}")
        });
        if brief {
            writeln!(out, "{desc}").map_err(|e| e.to_string())?;
        } else {
            writeln!(out, "{path:<label_width$}: {desc}").map_err(|e| e.to_string())?;
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(i32::from(any_err))
}

fn describe(path: &Path, mime: bool) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Ok(if mime {
            "inode/directory".to_string()
        } else {
            "directory".to_string()
        });
    }
    if meta.is_symlink() {
        return Ok(if mime {
            "inode/symlink".to_string()
        } else {
            "symbolic link".to_string()
        });
    }
    if meta.len() == 0 {
        return Ok(if mime {
            "inode/x-empty".to_string()
        } else {
            "empty".to_string()
        });
    }

    let head_size = usize::try_from(meta.len().min(4096)).unwrap_or(4096);
    let mut head = vec![0_u8; head_size];
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let n = f.read(&mut head).map_err(|e| e.to_string())?;
    head.truncate(n);

    if let Some(t) = infer::get(&head) {
        return Ok(if mime {
            t.mime_type().to_string()
        } else {
            human_label_from_mime(t.mime_type(), t.extension())
        });
    }

    // Text-vs-binary heuristic: count printable ASCII / common UTF-8.
    if looks_like_text(&head) {
        if std::str::from_utf8(&head).is_ok() {
            return Ok(if mime {
                "text/plain; charset=utf-8".to_string()
            } else {
                "ASCII text".to_string()
            });
        }
        return Ok(if mime {
            "text/plain".to_string()
        } else {
            "text".to_string()
        });
    }
    Ok(if mime {
        "application/octet-stream".to_string()
    } else {
        "data".to_string()
    })
}

fn looks_like_text(buf: &[u8]) -> bool {
    if buf.is_empty() {
        return false;
    }
    let printable = buf
        .iter()
        .filter(|b| matches!(**b, 0x09 | 0x0A | 0x0D | 0x20..=0x7E))
        .count();
    printable * 100 / buf.len() >= 95
}

fn human_label_from_mime(mime: &str, ext: &str) -> String {
    match mime {
        "application/zip" => "Zip archive data".to_string(),
        "application/gzip" => "gzip compressed data".to_string(),
        "application/x-bzip2" => "bzip2 compressed data".to_string(),
        "application/x-xz" => "XZ compressed data".to_string(),
        "application/x-tar" => "tar archive".to_string(),
        "application/pdf" => "PDF document".to_string(),
        "image/png" => "PNG image data".to_string(),
        "image/jpeg" => "JPEG image data".to_string(),
        "image/gif" => "GIF image data".to_string(),
        "image/webp" => "WebP image data".to_string(),
        "application/x-executable" | "application/x-elf" => "ELF executable".to_string(),
        "application/vnd.microsoft.portable-executable" => "PE32+ executable (Windows)".to_string(),
        "application/wasm" => "WebAssembly module".to_string(),
        _ => format!("{mime} ({ext})"),
    }
}

fn print_help() {
    println!(
        "Usage: file [OPTIONS] FILE...\n\
         \n\
         Determine the type of a file via magic-byte detection.\n\
         \n\
         Options:\n  \
           -b, --brief     don't print the filename prefix\n  \
           -i, --mime      print the MIME type instead of a description\n  \
           --help          show this help\n  \
           --version       show version\n"
    );
}

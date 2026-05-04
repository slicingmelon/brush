//! unzip / zipinfo adapter — via the [`zip`] crate.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 2.
//! Two names share one entry point branching on `argv[0]`:
//!
//! | Invocation | Behavior |
//! |---|---|
//! | `unzip <archive>` | Extract archive into current directory |
//! | `unzip -d <dir> <archive>` | Extract into `<dir>` |
//! | `unzip -l <archive>` | List archive contents |
//! | `unzip -p <archive> [members]` | Extract members to stdout |
//! | `zipinfo <archive>` | Long-form list of archive contents |
//!
//! Archive creation is **not** included — `zip` (the create side)
//! has many flag combinations and is uncommon in agent shell flows;
//! defer to follow-up if requested.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

pub(crate) fn unzip_main(args: Vec<OsString>) -> i32 {
    run(args, Variant::Unzip)
}

pub(crate) fn zipinfo_main(args: Vec<OsString>) -> i32 {
    run(args, Variant::Zipinfo)
}

#[derive(Clone, Copy, PartialEq)]
enum Variant {
    Unzip,
    Zipinfo,
}

fn run(args: Vec<OsString>, variant: Variant) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let mut list = matches!(variant, Variant::Zipinfo);
    let mut to_stdout = false;
    let mut overwrite = false;
    let mut quiet = false;
    let mut dest_dir: Option<PathBuf> = None;
    let mut archive: Option<String> = None;
    let mut members: Vec<String> = Vec::new();

    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-l" => list = true,
            "-p" => to_stdout = true,
            "-o" => overwrite = true,
            "-q" => quiet = true,
            "-h" | "--help" => {
                print_help(variant);
                return 0;
            }
            "--version" => {
                println!("unzip (brush-bundled-extras zip 5.x) 0.1.5");
                return 0;
            }
            "-d" => {
                i += 1;
                let Some(d) = argv.get(i) else {
                    eprintln!("unzip: -d requires a directory");
                    return 1;
                };
                dest_dir = Some(PathBuf::from(d));
            }
            s if s.starts_with('-') && s != "-" => {
                eprintln!("unzip: unknown option: {s}");
                return 1;
            }
            _ => {
                if archive.is_none() {
                    archive = Some(arg.clone());
                } else {
                    members.push(arg.clone());
                }
            }
        }
        i += 1;
    }

    let Some(arc) = archive else {
        eprintln!("unzip: missing archive operand");
        return 1;
    };

    let f = match File::open(&arc) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("unzip: {arc}: {e}");
            return 1;
        }
    };
    let mut zip = match zip::ZipArchive::new(f) {
        Ok(z) => z,
        Err(e) => {
            eprintln!("unzip: {arc}: {e}");
            return 1;
        }
    };

    if list {
        return print_listing(&mut zip, variant == Variant::Zipinfo);
    }

    if to_stdout {
        return extract_to_stdout(&mut zip, &members);
    }

    let target = dest_dir.unwrap_or_else(|| PathBuf::from("."));
    extract_all(&mut zip, &target, overwrite, quiet, &members)
}

fn print_listing<R: Read + io::Seek>(zip: &mut zip::ZipArchive<R>, long: bool) -> i32 {
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let n = zip.len();
    if long {
        let _ = writeln!(out, "Length      Date    Time    Name");
        let _ = writeln!(out, "---------   ----    ----    ----");
    } else {
        let _ = writeln!(out, "  Length      Date    Time    Name");
        let _ = writeln!(out, "---------  ---------- -----   ----");
    }
    let mut total = 0_u64;
    for i in 0..n {
        let entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("unzip: {e}");
                return 1;
            }
        };
        let name = entry.name();
        let size = entry.size();
        total += size;
        let _ = writeln!(out, "{size:>9}                      {name}");
    }
    let _ = writeln!(out, "---------                     -------");
    let _ = writeln!(out, "{total:>9}                     {n} files");
    0
}

fn extract_to_stdout<R: Read + io::Seek>(zip: &mut zip::ZipArchive<R>, members: &[String]) -> i32 {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let n = zip.len();
    let mut any_err = false;
    for i in 0..n {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("unzip: {e}");
                any_err = true;
                continue;
            }
        };
        if !members.is_empty() && !members.iter().any(|m| m == entry.name()) {
            continue;
        }
        if entry.is_dir() {
            continue;
        }
        if let Err(e) = io::copy(&mut entry, &mut out) {
            eprintln!("unzip: {e}");
            any_err = true;
        }
    }
    i32::from(any_err)
}

fn extract_all<R: Read + io::Seek>(
    zip: &mut zip::ZipArchive<R>,
    dest: &Path,
    overwrite: bool,
    quiet: bool,
    members: &[String],
) -> i32 {
    if let Err(e) = fs::create_dir_all(dest) {
        eprintln!("unzip: {}: {}", dest.display(), e);
        return 1;
    }
    let mut any_err = false;
    let n = zip.len();
    for i in 0..n {
        let mut entry = match zip.by_index(i) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("unzip: {e}");
                any_err = true;
                continue;
            }
        };
        if !members.is_empty() && !members.iter().any(|m| m == entry.name()) {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            eprintln!("unzip: skipping unsafe entry {}", entry.name());
            any_err = true;
            continue;
        };
        let out_path = dest.join(rel);
        if entry.is_dir() {
            if let Err(e) = fs::create_dir_all(&out_path) {
                eprintln!("unzip: {}: {}", out_path.display(), e);
                any_err = true;
            }
            continue;
        }
        if let Some(parent) = out_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("unzip: {}: {}", parent.display(), e);
                any_err = true;
                continue;
            }
        }
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        if !overwrite {
            opts.create_new(true);
        }
        let f = match opts.open(&out_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("unzip: {}: {}", out_path.display(), e);
                any_err = true;
                continue;
            }
        };
        let mut writer = BufWriter::new(f);
        if let Err(e) = io::copy(&mut entry, &mut writer) {
            eprintln!("unzip: {}: {}", out_path.display(), e);
            any_err = true;
            continue;
        }
        if !quiet {
            println!("  inflating: {}", out_path.display());
        }
    }
    i32::from(any_err)
}

fn print_help(variant: Variant) {
    if variant == Variant::Zipinfo {
        println!(
            "Usage: zipinfo ARCHIVE.zip\n\
             \n\
             List the contents of a zip archive (long form).\n"
        );
    } else {
        println!(
            "Usage: unzip [OPTIONS] ARCHIVE.zip [members...]\n\
             \n\
             Extract a zip archive.\n\
             \n\
             Options:\n  \
               -d DIR        extract into DIR (default: current dir)\n  \
               -l            list archive contents\n  \
               -p            extract members to stdout\n  \
               -o            overwrite existing files\n  \
               -q            quiet (suppress per-file output)\n  \
               --help        show this help\n  \
               --version     show version\n"
        );
    }
}

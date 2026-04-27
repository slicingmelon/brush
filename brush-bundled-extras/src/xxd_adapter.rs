//! In-tree `xxd` implementation — hex dumper.
//!
//! Per `docs/planning/bundled-extras-coverage-expansion.md` Cycle 1.
//! No upstream crate; the BSD `xxd` is a single-file utility small
//! enough to write cleaner than vendor.
//!
//! Supported flags (subset of BSD `xxd`):
//!
//! | Flag | Behavior |
//! |---|---|
//! | `-r` | Reverse: read hex dump, write binary |
//! | `-c <cols>` | Bytes per output line (default 16) |
//! | `-g <size>` | Group bytes (default 2) |
//! | `-s <off>` | Skip `off` bytes of input |
//! | `-l <len>` | Stop after `len` bytes |
//! | `-p` | Postscript / plain hex dump (no offsets, no ASCII) |
//! | `-i` | C include style (array of bytes) |
//! | `-u` | Uppercase hex |

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};

pub(crate) fn xxd_main(args: Vec<OsString>) -> i32 {
    let argv: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    match run(&argv) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("xxd: {e}");
            1
        }
    }
}

#[derive(Default)]
struct Cfg {
    reverse: bool,
    cols: usize,
    group: usize,
    skip: u64,
    length: Option<u64>,
    postscript: bool,
    c_include: bool,
    uppercase: bool,
    input: Option<String>,
}

fn run(argv: &[String]) -> Result<i32, String> {
    let mut cfg = Cfg {
        cols: 16,
        group: 2,
        ..Cfg::default()
    };
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "-r" => cfg.reverse = true,
            "-p" => cfg.postscript = true,
            "-i" => cfg.c_include = true,
            "-u" => cfg.uppercase = true,
            "-h" | "--help" => {
                print_help();
                return Ok(0);
            }
            "--version" => {
                println!("xxd (brush-bundled-extras) 0.1.4");
                return Ok(0);
            }
            "-c" => {
                i += 1;
                cfg.cols = parse_usize(argv.get(i), "-c")?;
            }
            "-g" => {
                i += 1;
                cfg.group = parse_usize(argv.get(i), "-g")?;
            }
            "-s" => {
                i += 1;
                cfg.skip = parse_u64(argv.get(i), "-s")?;
            }
            "-l" => {
                i += 1;
                cfg.length = Some(parse_u64(argv.get(i), "-l")?);
            }
            s if s.starts_with('-') && s != "-" => {
                return Err(format!("unknown option: {s}"));
            }
            _ => positional.push(arg.clone()),
        }
        i += 1;
    }
    if positional.len() > 1 {
        return Err("at most one input file".to_string());
    }
    cfg.input = positional.into_iter().next();

    if cfg.cols == 0 {
        cfg.cols = 16;
    }
    if cfg.group == 0 {
        cfg.group = cfg.cols;
    }

    let mut buf = Vec::new();
    read_input(cfg.input.as_deref(), cfg.skip, cfg.length, &mut buf)?;

    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    if cfg.reverse {
        write_reverse(&buf, &mut out)?;
    } else if cfg.c_include {
        write_c_include(cfg.input.as_deref(), &buf, cfg.cols, cfg.uppercase, &mut out)?;
    } else if cfg.postscript {
        write_postscript(&buf, cfg.cols, cfg.uppercase, &mut out)?;
    } else {
        write_canonical(&buf, &cfg, &mut out)?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(0)
}

fn parse_usize(s: Option<&String>, flag: &str) -> Result<usize, String> {
    s.ok_or_else(|| format!("option {flag} requires a value"))?
        .parse::<usize>()
        .map_err(|e| format!("option {flag}: {e}"))
}

fn parse_u64(s: Option<&String>, flag: &str) -> Result<u64, String> {
    let v = s.ok_or_else(|| format!("option {flag} requires a value"))?;
    let trimmed = v.trim_start_matches("0x").trim_start_matches("0X");
    if trimmed.len() != v.len() {
        u64::from_str_radix(trimmed, 16).map_err(|e| format!("option {flag}: {e}"))
    } else {
        v.parse::<u64>().map_err(|e| format!("option {flag}: {e}"))
    }
}

fn read_input(
    name: Option<&str>,
    skip: u64,
    length: Option<u64>,
    out: &mut Vec<u8>,
) -> Result<(), String> {
    let data = match name {
        None | Some("-") => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| format!("stdin: {e}"))?;
            buf
        }
        Some(path) => {
            let mut f = File::open(path).map_err(|e| format!("{path}: {e}"))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            buf
        }
    };
    let start = usize::try_from(skip).unwrap_or(usize::MAX).min(data.len());
    let end = match length {
        Some(n) => start
            .saturating_add(usize::try_from(n).unwrap_or(usize::MAX))
            .min(data.len()),
        None => data.len(),
    };
    out.extend_from_slice(&data[start..end]);
    Ok(())
}

fn write_canonical<W: Write>(buf: &[u8], cfg: &Cfg, out: &mut W) -> Result<(), String> {
    let hex_chars: &[u8] = if cfg.uppercase {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    for (line_idx, chunk) in buf.chunks(cfg.cols).enumerate() {
        let offset = cfg.skip + (line_idx as u64) * (cfg.cols as u64);
        write!(out, "{offset:08x}: ").map_err(|e| e.to_string())?;
        let mut hex_written = 0_usize;
        for (i, b) in chunk.iter().enumerate() {
            let hi = hex_chars[(b >> 4) as usize];
            let lo = hex_chars[(b & 0x0f) as usize];
            out.write_all(&[hi, lo]).map_err(|e| e.to_string())?;
            hex_written += 2;
            if (i + 1) % cfg.group == 0 && i + 1 != chunk.len() {
                out.write_all(b" ").map_err(|e| e.to_string())?;
                hex_written += 1;
            }
        }
        let total_hex_width = cfg.cols * 2 + cfg.cols.saturating_sub(1) / cfg.group;
        for _ in hex_written..total_hex_width {
            out.write_all(b" ").map_err(|e| e.to_string())?;
        }
        out.write_all(b"  ").map_err(|e| e.to_string())?;
        for b in chunk {
            let c = if (0x20..0x7f).contains(b) { *b } else { b'.' };
            out.write_all(&[c]).map_err(|e| e.to_string())?;
        }
        out.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_postscript<W: Write>(
    buf: &[u8],
    cols: usize,
    upper: bool,
    out: &mut W,
) -> Result<(), String> {
    let hex_chars: &[u8] = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    for chunk in buf.chunks(cols) {
        for b in chunk {
            out.write_all(&[hex_chars[(b >> 4) as usize], hex_chars[(b & 0x0f) as usize]])
                .map_err(|e| e.to_string())?;
        }
        out.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn write_c_include<W: Write>(
    name: Option<&str>,
    buf: &[u8],
    cols: usize,
    upper: bool,
    out: &mut W,
) -> Result<(), String> {
    let var_base = match name {
        Some(n) if n != "-" => n
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(n)
            .replace(['.', '-'], "_"),
        _ => "stdin".to_string(),
    };
    writeln!(out, "unsigned char {var_base}[] = {{").map_err(|e| e.to_string())?;
    let prefix = if upper { "0X" } else { "0x" };
    let hex_chars: &[u8] = if upper {
        b"0123456789ABCDEF"
    } else {
        b"0123456789abcdef"
    };
    for (i, b) in buf.iter().enumerate() {
        if i % cols == 0 {
            out.write_all(b"  ").map_err(|e| e.to_string())?;
        }
        write!(
            out,
            "{prefix}{}{}",
            hex_chars[(b >> 4) as usize] as char,
            hex_chars[(b & 0x0f) as usize] as char
        )
        .map_err(|e| e.to_string())?;
        if i + 1 != buf.len() {
            out.write_all(b", ").map_err(|e| e.to_string())?;
        }
        if (i + 1) % cols == 0 {
            out.write_all(b"\n").map_err(|e| e.to_string())?;
        }
    }
    if !buf.len().is_multiple_of(cols) {
        out.write_all(b"\n").map_err(|e| e.to_string())?;
    }
    writeln!(out, "}};").map_err(|e| e.to_string())?;
    writeln!(out, "unsigned int {var_base}_len = {};", buf.len()).map_err(|e| e.to_string())?;
    Ok(())
}

fn write_reverse<W: Write>(input: &[u8], out: &mut W) -> Result<(), String> {
    // Accept both canonical (offset: hex hex  ascii) and postscript
    // (just hex). Strategy: walk the input, for each line strip an
    // optional "<hex>:" prefix, then collect hex pairs up to either
    // end-of-line or two consecutive spaces (which marks the
    // hex/ascii boundary in canonical mode).
    let text = std::str::from_utf8(input).map_err(|e| format!("invalid utf-8: {e}"))?;
    for line in text.lines() {
        let after_offset = line.split_once(':').map_or(line, |(_, rest)| rest);
        let hex_part = after_offset
            .split_once("  ")
            .map_or(after_offset, |(h, _)| h);
        let mut nibbles: Vec<u8> = Vec::new();
        for c in hex_part.chars() {
            if let Some(n) = c.to_digit(16) {
                // n is 0..16 by construction; fits trivially in u8.
                nibbles.push(u8::try_from(n).unwrap_or(0));
            }
        }
        for pair in nibbles.chunks_exact(2) {
            out.write_all(&[(pair[0] << 4) | pair[1]])
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn print_help() {
    println!(
        "Usage: xxd [OPTIONS] [INFILE]\n\
         \n\
         Make a hexdump (or do reverse with -r).\n\
         \n\
         Options:\n  \
           -r          reverse: convert hex back to binary\n  \
           -c COLS     bytes per line (default 16)\n  \
           -g SIZE     group size in bytes (default 2)\n  \
           -s OFF      skip OFF bytes from start (decimal or 0x...)\n  \
           -l LEN      stop after LEN bytes\n  \
           -p          postscript / plain hex dump\n  \
           -i          C include style\n  \
           -u          uppercase hex\n  \
           --help      show this help\n  \
           --version   show version\n"
    );
}

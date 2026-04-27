//! Adapter wrappers for non-uutils-coreutils utilities bundled into
//! brush-shell.
//!
//! ## Why this crate exists
//!
//! `brush-coreutils-builtins` ships utilities from `uutils/coreutils`,
//! which all expose the same `uumain` shape:
//!
//! ```ignore
//! fn uumain(args: impl Iterator<Item = OsString>) -> i32
//! ```
//!
//! Other utilities we want to bundle (findutils, diffutils, procps;
//! eventually grep/sed/awk) live in different upstream repos and use
//! different API shapes — argv slices instead of iterators, `&str`
//! instead of `OsString`, dependency-injection objects, etc. This crate
//! is the *adapter layer*: per-utility wrapper functions translate
//! between brush's [`BundledFn`] signature and whatever the upstream
//! exposes.
//!
//! See [`docs/planning/coreutils-coverage-expansion.md`](../../docs/planning/coreutils-coverage-expansion.md)
//! Cycle 2 for the rationale behind one mega-crate vs per-upstream
//! crates.
//!
//! ## Argv conversion
//!
//! Many upstreams take `&[&str]` rather than the platform's native
//! `OsString`. We convert via [`OsString::to_string_lossy`]: bytes that
//! are valid UTF-8 round-trip cleanly, but invalid sequences are
//! replaced with U+FFFD. This loses fidelity for non-UTF-8 OS paths —
//! rare in practice but worth knowing. Each adapter documents whether
//! its upstream is sensitive to this.

#![allow(
    clippy::missing_errors_doc,
    reason = "registry function returns a HashMap directly; error semantics are at the BundledFn boundary"
)]

use std::collections::HashMap;
use std::ffi::OsString;

#[cfg(feature = "extras.grep")]
mod grep_adapter;

#[cfg(feature = "extras.which")]
mod which_adapter;
#[cfg(feature = "extras.tree")]
mod tree_adapter;
#[cfg(feature = "extras.xxd")]
mod xxd_adapter;
#[cfg(feature = "extras.column")]
mod column_adapter;
#[cfg(feature = "extras.file")]
mod file_adapter;

#[cfg(feature = "extras.tar")]
mod tar_adapter;
#[cfg(feature = "extras.gzip")]
mod gzip_adapter;
#[cfg(feature = "extras.bzip2")]
mod bzip2_adapter;
#[cfg(feature = "extras.xz")]
mod xz_adapter;
#[cfg(feature = "extras.zip")]
mod zip_adapter;

/// Signature of a bundled command's entry point.
///
/// Same shape as `brush-coreutils-builtins::BundledFn`. Re-declared
/// here to avoid a dependency on that crate; consumers (brush-shell)
/// merge the two registries by `HashMap::extend`.
pub type BundledFn = fn(args: Vec<OsString>) -> i32;

/// Returns the set of bundled commands enabled by feature flags.
///
/// Keyed by utility name (e.g., `"find"`); values are adapter functions
/// that consume a full argv (with `argv[0]` as the command name) and
/// return an exit code.
#[allow(
    clippy::implicit_hasher,
    reason = "registry uses the default hasher; callers build with HashMap::new()"
)]
#[must_use]
pub fn bundled_commands() -> HashMap<String, BundledFn> {
    #[allow(
        unused_mut,
        reason = "every insertion is cfg'd on a feature; with none enabled, nothing mutates the map"
    )]
    let mut m = HashMap::<String, BundledFn>::new();

    #[cfg(feature = "extras.find")]
    {
        m.insert("find".to_string(), find_adapter as BundledFn);
    }

    #[cfg(feature = "extras.xargs")]
    {
        m.insert("xargs".to_string(), xargs_adapter as BundledFn);
    }

    #[cfg(feature = "extras.sed")]
    {
        m.insert("sed".to_string(), sed_adapter as BundledFn);
    }

    #[cfg(feature = "extras.awk")]
    {
        m.insert("awk".to_string(), awk_adapter as BundledFn);
    }

    #[cfg(feature = "extras.grep")]
    {
        // `grep` / `fastgrep` dispatch directly to the upstream CLI;
        // `egrep` / `fgrep` are GNU-style aliases that prepend `-E`
        // (extended regex) / `-F` (fixed string) before delegating —
        // matching the historical behavior agents and shell scripts
        // expect. See `docs/planning/bundled-extras-coverage-expansion.md`
        // Cycle 0a. fastgrep's `GNU_GREP_COMPAT.md` confirms `-E` and
        // `-F` are both supported.
        m.insert("grep".to_string(), grep_adapter::grep_main as BundledFn);
        m.insert("fastgrep".to_string(), grep_adapter::grep_main as BundledFn);
        m.insert("egrep".to_string(), grep_adapter::egrep_main as BundledFn);
        m.insert("fgrep".to_string(), grep_adapter::fgrep_main as BundledFn);
    }

    // Cycle 1 utility quick-wins (bundled-extras-coverage-expansion).
    #[cfg(feature = "extras.which")]
    {
        m.insert("which".to_string(), which_adapter::which_main as BundledFn);
    }
    #[cfg(feature = "extras.tree")]
    {
        m.insert("tree".to_string(), tree_adapter::tree_main as BundledFn);
    }
    #[cfg(feature = "extras.xxd")]
    {
        m.insert("xxd".to_string(), xxd_adapter::xxd_main as BundledFn);
    }
    #[cfg(feature = "extras.column")]
    {
        m.insert("column".to_string(), column_adapter::column_main as BundledFn);
    }
    #[cfg(feature = "extras.file")]
    {
        m.insert("file".to_string(), file_adapter::file_main as BundledFn);
    }

    // Cycle 2 compression family (bundled-extras-coverage-expansion).
    #[cfg(feature = "extras.tar")]
    {
        m.insert("tar".to_string(), tar_adapter::tar_main as BundledFn);
    }
    #[cfg(feature = "extras.gzip")]
    {
        // `gzip` / `gunzip` / `gzcat` / `zcat` all share the gzip
        // adapter, branching on argv[0] for compress vs decompress.
        m.insert("gzip".to_string(), gzip_adapter::gzip_main as BundledFn);
        m.insert("gunzip".to_string(), gzip_adapter::gunzip_main as BundledFn);
        m.insert("zcat".to_string(), gzip_adapter::zcat_main as BundledFn);
        m.insert("gzcat".to_string(), gzip_adapter::zcat_main as BundledFn);
    }
    #[cfg(feature = "extras.bzip2")]
    {
        m.insert("bzip2".to_string(), bzip2_adapter::bzip2_main as BundledFn);
        m.insert("bunzip2".to_string(), bzip2_adapter::bunzip2_main as BundledFn);
        m.insert("bzcat".to_string(), bzip2_adapter::bzcat_main as BundledFn);
    }
    #[cfg(feature = "extras.xz")]
    {
        m.insert("xz".to_string(), xz_adapter::xz_main as BundledFn);
        m.insert("unxz".to_string(), xz_adapter::unxz_main as BundledFn);
        m.insert("xzcat".to_string(), xz_adapter::xzcat_main as BundledFn);
    }
    #[cfg(feature = "extras.zip")]
    {
        m.insert("unzip".to_string(), zip_adapter::unzip_main as BundledFn);
        m.insert("zipinfo".to_string(), zip_adapter::zipinfo_main as BundledFn);
    }

    m
}

/// Adapter for `findutils::find::find_main`.
///
/// Upstream signature:
/// `find_main(args: &[&str], deps: &impl Dependencies) -> i32`
///
/// Adapter behavior:
/// 1. Convert `Vec<OsString>` → `Vec<String>` via
///    [`OsString::to_string_lossy`]. This is lossy on non-UTF-8 bytes —
///    `find` is used heavily with file paths, which on Windows are
///    UCS-2 and on Unix are arbitrary bytes; truly non-UTF-8 paths will
///    be substituted with U+FFFD before reaching find. In practice this
///    is rare for sane filenames. If lossless behavior is needed,
///    consumers can fall through to a system `find` on PATH.
/// 2. Build a `Vec<&str>` slice over the owned strings.
/// 3. Construct `findutils::find::StandardDependencies` (the
///    real-IO/clock/fs implementation).
/// 4. Call `find_main` and return its exit code.
#[cfg(feature = "extras.find")]
fn find_adapter(args: Vec<OsString>) -> i32 {
    let owned: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let strs: Vec<&str> = owned.iter().map(String::as_str).collect();
    let deps = findutils::find::StandardDependencies::new();
    findutils::find::find_main(&strs, &deps)
}

/// Adapter for `findutils::xargs::xargs_main`.
///
/// Upstream signature: `xargs_main(args: &[&str]) -> i32` (no
/// dependency-injection object needed).
///
/// Lossy `OsString` → `&str` conversion as in [`find_adapter`]; xargs
/// reads command names and args from stdin, which is independent of
/// argv, so the lossiness only affects flags like `-I` or `-d`'s
/// argument values — almost always ASCII-safe.
#[cfg(feature = "extras.xargs")]
fn xargs_adapter(args: Vec<OsString>) -> i32 {
    let owned: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let strs: Vec<&str> = owned.iter().map(String::as_str).collect();
    findutils::xargs::xargs_main(&strs)
}

/// Adapter for `awk_rs::Interpreter` (pegasusheavy/awk-rs v0.1.0).
///
/// Upstream exposes `Lexer`/`Parser`/`Interpreter` as a public lib but
/// no `run(args)` entrypoint, so this adapter mirrors the CLI driver in
/// upstream `src/main.rs::run()` line-for-line. Intent: stay drop-in
/// equivalent to the standalone `awk-rs` binary, no behavioral
/// divergence.
///
/// `args[0]` carries the bundled name (`"awk"`), set in
/// `bundled.rs::maybe_dispatch`. Upstream's main.rs operates on
/// `&args[1..]`, so we slice the same way.
///
/// Lossy `OsString` → `String` conversion follows the find/xargs
/// precedent — awk programs and `-v var=val` assignments are typically
/// ASCII-safe; non-UTF-8 input file paths get U+FFFD substitution
/// before reaching the interpreter (which then opens via `File::open`,
/// where the lossy path may fail to resolve).
#[cfg(feature = "extras.awk")]
fn awk_adapter(args: Vec<OsString>) -> i32 {
    let owned: Vec<String> = args
        .into_iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let rest: &[String] = owned.get(1..).unwrap_or(&[]);
    match awk_run(rest) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("awk: {e}");
            2
        }
    }
}

/// Argv-driven entrypoint mirroring `awk-rs/src/main.rs::run()`.
/// Returns the awk exit code on success, an error string on failure
/// (printed by [`awk_adapter`] as `awk: <msg>` before exit 2).
#[cfg(feature = "extras.awk")]
#[allow(
    clippy::too_many_lines,
    reason = "ports upstream main.rs::run() argv parser line-for-line; refactoring would diverge from upstream"
)]
fn awk_run(args: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
    use awk_rs::{Interpreter, Lexer, Parser};
    use std::fs::{self, File};
    use std::io::{self, BufReader};

    let mut field_separator = String::from(" ");
    let mut program_source: Option<String> = None;
    let mut input_files: Vec<String> = Vec::new();
    let mut variables: Vec<(String, String)> = Vec::new();
    let mut posix_mode = false;
    let mut traditional_mode = false;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--help" || arg == "-h" {
            print_awk_help();
            return Ok(0);
        }

        if arg == "--version" {
            println!("awk-rs 0.1.0");
            return Ok(0);
        }

        if arg == "--posix" || arg == "-P" {
            posix_mode = true;
            traditional_mode = false;
            i += 1;
            continue;
        }

        if arg == "--traditional" || arg == "--compat" || arg == "-c" {
            traditional_mode = true;
            posix_mode = false;
            i += 1;
            continue;
        }

        if arg == "-F" {
            i += 1;
            let fs_arg = args.get(i).ok_or("option -F requires an argument")?;
            field_separator = fs_arg.clone();
        } else if let Some(fs) = arg.strip_prefix("-F") {
            field_separator = fs.to_string();
        } else if arg == "-v" {
            i += 1;
            let var_assign = args.get(i).ok_or("option -v requires an argument")?;
            if let Some((name, value)) = var_assign.split_once('=') {
                variables.push((name.to_string(), value.to_string()));
            } else {
                return Err(format!("invalid variable assignment: {var_assign}").into());
            }
        } else if arg == "-f" {
            i += 1;
            let script_file = args.get(i).ok_or("option -f requires an argument")?;
            program_source = Some(fs::read_to_string(script_file)?);
        } else if arg == "--" {
            i += 1;
            input_files.extend(args[i..].iter().cloned());
            break;
        } else if arg.starts_with('-') && arg != "-" {
            return Err(format!("unknown option: {arg}").into());
        } else if program_source.is_none() {
            program_source = Some(arg.clone());
        } else {
            input_files.push(arg.clone());
        }

        i += 1;
    }

    let program_source = program_source.ok_or("no program provided")?;

    let mut lexer = Lexer::new(&program_source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let program = parser.parse()?;

    let mut interpreter = Interpreter::new(&program);
    interpreter.set_posix_mode(posix_mode);
    interpreter.set_traditional_mode(traditional_mode);
    interpreter.set_fs(&field_separator);

    let mut argv = vec![String::from("awk")];
    argv.extend(input_files.iter().cloned());
    interpreter.set_args(argv);

    for (name, value) in &variables {
        interpreter.set_variable(name, value);
    }

    let stdout = io::stdout();
    let mut output = stdout.lock();

    let exit_code = if input_files.is_empty() {
        interpreter.set_filename("");
        let stdin = io::stdin();
        let inputs = vec![BufReader::new(stdin.lock())];
        interpreter.run(inputs, &mut output)?
    } else {
        let mut exit_code = 0;
        for filename in &input_files {
            interpreter.set_filename(filename);
            if filename == "-" {
                let stdin = io::stdin();
                let inputs = vec![BufReader::new(stdin.lock())];
                exit_code = interpreter.run(inputs, &mut output)?;
            } else {
                let file = File::open(filename)?;
                let inputs = vec![BufReader::new(file)];
                exit_code = interpreter.run(inputs, &mut output)?;
            }
        }
        exit_code
    };

    Ok(exit_code)
}

#[cfg(feature = "extras.awk")]
fn print_awk_help() {
    println!(
        r#"Usage: awk [OPTIONS] 'program' [file ...]
       awk [OPTIONS] -f progfile [file ...]

A 100% POSIX-compatible AWK implementation in Rust with gawk extensions.

Options:
  -F fs            Set the field separator to fs
  -v var=val       Assign value to variable before execution
  -f progfile      Read the AWK program from file
  -P, --posix      Strict POSIX mode (disable gawk extensions)
  -c, --traditional Traditional AWK mode (disable gawk extensions)
  --version        Print version information
  --help           Print this help message
"#
    );
}

/// Adapter for `sed::sed::uumain` (uutils/sed v0.1.1).
///
/// Upstream signature is the standard uutils `uumain` shape:
/// `uumain(args: impl IntoIterator<Item = OsString>) -> i32`. The
/// `argv[0]` slot already carries the bundled name (`"sed"`) by the
/// time this adapter runs — set in `bundled.rs::maybe_dispatch` —
/// which is what uucore's `util_name()` lazily reads, so no
/// special `set_utility_is_second_arg()` handling is needed.
///
/// SIGPIPE/localization wiring (the work
/// `brush-coreutils-builtins::register!` does for uutils crates) is
/// intentionally omitted to keep the adapter dep-light: the bundled
/// dispatch path always runs sed inside a fresh `brush --invoke-bundled`
/// subprocess, so pipe-close behavior is delegated to the host OS rather
/// than masked by the runtime helpers. If sed's `translate!()` output
/// turns up untranslated in the wild, lift the `prepare_uutil_runtime`
/// helper into this crate (requires adding a uucore dep — currently a
/// transitive of `sed` itself).
#[cfg(feature = "extras.sed")]
fn sed_adapter(args: Vec<OsString>) -> i32 {
    sed::sed::uumain(args.into_iter())
}

//! Adapter for `awnion/fastgrep` v0.1.8 — registered as both `grep`
//! and `fastgrep` in [`crate::bundled_commands`].
//!
//! Upstream exposes the search engine as a public lib (`pub mod cli /
//! output / pattern / searcher / threadpool / trigram / walker`) but
//! the orchestration lives in `src/bin/grep.rs::main()` and is not a
//! `pub fn`. This module ports that orchestration line-for-line as
//! [`grep_main`], with three small adaptations:
//!
//! 1. `Cli::try_parse_from(args)` instead of `Cli::parse()`, so we
//!    consume the [`Vec<OsString>`] handed to us by the bundled-
//!    dispatch path rather than reading `std::env::args_os()`.
//! 2. `ExitCode` returns are flattened to `i32` (`SUCCESS` → 0,
//!    `from(N)` → `N`) to match brush's `BundledFn` signature.
//! 3. Upstream's one `expect("failed to spawn walker thread")` is
//!    replaced with explicit error handling (brush forbids
//!    `clippy::expect_used`).
//!
//! Version output uses the hardcoded crate version string `"0.1.8"`
//! and skips the upstream `env!("GIT_SHA")` reference (brush-bundled-
//! extras has no `build.rs` setting that env var).
//!
//! See `docs/planning/posixutils-rs-integration.md` Cycle 0b-revised.

#![allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    clippy::significant_drop_tightening,
    reason = "module ports upstream fastgrep src/bin/grep.rs::main() shape; refactoring would diverge from upstream"
)]

use std::ffi::OsString;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::Parser;
use fastgrep::cli::{Cli, ResolvedConfig};
use fastgrep::output::{
    OutputConfig, TAB_FIELD_WIDTH_STDIN, format_result, write_json_size_limit_warning,
};
use fastgrep::pattern::CompiledPattern;
use fastgrep::searcher::{
    search_file_streaming, search_file_streaming_reuse, search_reader,
    search_reader_streaming_labeled,
};
use fastgrep::threadpool::ThreadPool;
use fastgrep::trigram::{INDEX_VERSION, TrigramIndex, evict_if_needed};
use fastgrep::walker::{SkippedFile, walk};
use kanal::bounded;

/// `egrep` alias — pre-pends `-E` after `argv[0]` and delegates to
/// [`grep_main`]. GNU `egrep` is `grep -E` (extended regex); fastgrep
/// honors `-E`. Per `docs/planning/bundled-extras-coverage-expansion.md`
/// Cycle 0a.
pub(crate) fn egrep_main(args: Vec<OsString>) -> i32 {
    grep_main(prepend_flag_after_argv0(args, "-E"))
}

/// `fgrep` alias — pre-pends `-F` after `argv[0]` and delegates to
/// [`grep_main`]. GNU `fgrep` is `grep -F` (fixed-string match);
/// fastgrep honors `-F`. Per `docs/planning/bundled-extras-coverage-expansion.md`
/// Cycle 0a.
pub(crate) fn fgrep_main(args: Vec<OsString>) -> i32 {
    grep_main(prepend_flag_after_argv0(args, "-F"))
}

/// Insert `flag` immediately after `args[0]` (the bundled command
/// name). Empty input returns `[flag]` so the alias is still meaningful
/// in degenerate dispatch (shouldn't happen — `bundled.rs` always sets
/// `args[0]`).
fn prepend_flag_after_argv0(args: Vec<OsString>, flag: &str) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len() + 1);
    let mut iter = args.into_iter();
    if let Some(first) = iter.next() {
        out.push(first);
    }
    out.push(OsString::from(flag));
    out.extend(iter);
    out
}

/// Bundled-dispatch entry point for `grep` / `fastgrep`. Mirrors
/// [upstream `src/bin/grep.rs::main`](https://github.com/awnion/fastgrep/blob/main/src/bin/grep.rs).
pub(crate) fn grep_main(args: Vec<OsString>) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            // clap surfaces `--help` and `--version` as errors with
            // kind ::DisplayHelp / ::DisplayVersion (exit 0); real
            // parse errors are everything else (exit 2). Mirror that
            // convention rather than calling `e.exit()` (which would
            // bypass brush's centralized exit-site contract).
            let exit = match e.kind() {
                clap::error::ErrorKind::DisplayHelp
                | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
            let _ = e.print();
            return exit;
        }
    };

    if cli.version {
        println!("grep (fastgrep) 0.1.8 [index v{INDEX_VERSION}]");
        println!("https://crates.io/crates/fastgrep");
        println!();
        println!("features: simd, trigram-index, parallel, ai-agent-optimized");
        return 0;
    }

    let config = cli.resolve();

    let pattern = match CompiledPattern::compile(&config) {
        Ok(p) => Arc::new(p),
        Err(e) => {
            eprintln!("grep: {e}");
            return 2;
        }
    };

    let no_messages = config.no_messages;

    let output_config = OutputConfig {
        mode: config.output_mode,
        color: config.color,
        line_number: config.line_number,
        files_with_matches: config.files_with_matches,
        files_without_match: config.files_without_match,
        count: config.count,
        quiet: config.quiet,
        max_count: config.max_count,
        multi_file: config.multi_file,
        max_line_len: config.max_line_len,
        only_matching: config.only_matching,
        after_context: config.after_context,
        before_context: config.before_context,
        byte_offset: config.byte_offset,
        ignore_binary: config.ignore_binary,
        group_separator: config.group_separator.clone(),
        initial_tab: config.initial_tab,
        null: config.null,
        text: config.text,
    };

    if config.stdin {
        return run_stdin(
            &pattern,
            &output_config,
            config.invert_match,
            no_messages,
            config.label.as_deref(),
        );
    }

    // Check for nonexistent paths (matches GNU grep behavior)
    let mut has_path_error = false;
    for path in &config.paths {
        if !path.exists() {
            has_path_error = true;
            if !no_messages {
                eprintln!("grep: {}: No such file or directory", path.display());
            }
        }
    }

    // Fast path: single file, no recursion — skip thread pool/channel overhead
    if config.paths.len() == 1 && !config.recursive {
        let path = &config.paths[0];
        if path.is_file() {
            return run_single_file(
                path,
                &pattern,
                &output_config,
                config.invert_match,
                no_messages,
            );
        }
        if has_path_error {
            return 2;
        }
    }

    let result = run_files(config, pattern, output_config, no_messages);
    // If there were path errors, exit code 2 takes precedence over "no match" (1)
    if has_path_error && result != 0 { 2 } else { result }
}

fn run_single_file(
    path: &std::path::Path,
    pattern: &CompiledPattern,
    output_config: &OutputConfig,
    invert_match: bool,
    no_messages: bool,
) -> i32 {
    let stdout = std::io::stdout().lock();
    let mut writer = BufWriter::new(stdout);

    let count =
        match search_file_streaming(path, pattern, invert_match, output_config, &mut writer) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::BrokenPipe && !no_messages {
                    eprintln!("grep: {}: {e}", path.display());
                }
                return 2;
            }
        };

    let _ = writer.flush();
    if count > 0 { 0 } else { 1 }
}

fn run_stdin(
    pattern: &CompiledPattern,
    output_config: &OutputConfig,
    invert_match: bool,
    no_messages: bool,
    label: Option<&str>,
) -> i32 {
    let mut stdin = std::io::stdin().lock();

    // Default stdin label to "(standard input)" when -H is active (matches GNU grep)
    let effective_label =
        if output_config.multi_file { Some(label.unwrap_or("(standard input)")) } else { label };
    let label_path = effective_label.map(std::path::PathBuf::from);
    let effective_config = output_config;

    let has_context = effective_config.before_context > 0 || effective_config.after_context > 0;

    // Use streaming path for context or only-matching modes
    if has_context || effective_config.only_matching {
        let stdout = std::io::stdout().lock();
        let mut writer = BufWriter::new(stdout);
        let label_bytes = effective_label.map(str::as_bytes);
        let count = match search_reader_streaming_labeled(
            &mut stdin,
            pattern,
            invert_match,
            effective_config,
            &mut writer,
            label_bytes,
        ) {
            Ok(c) => c,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::BrokenPipe && !no_messages {
                    eprintln!("grep: (stdin): {e}");
                }
                return 2;
            }
        };
        let _ = writer.flush();
        return if count > 0 { 0 } else { 1 };
    }

    let need_ranges = effective_config.requires_match_ranges()
        && !effective_config.files_with_matches
        && !effective_config.count
        && !effective_config.quiet;
    let count_only =
        effective_config.count || effective_config.files_with_matches || effective_config.quiet;
    let mut result =
        match search_reader(&mut stdin, pattern, invert_match, need_ranges, count_only) {
            Ok(r) => r,
            Err(e) => {
                if !no_messages {
                    eprintln!("grep: (stdin): {e}");
                }
                return 2;
            }
        };

    if let Some(ref lp) = label_path {
        result.path.clone_from(lp);
    }

    let found_match = !result.matches.is_empty();

    if !effective_config.quiet {
        let stdout = std::io::stdout().lock();
        let mut writer = BufWriter::new(stdout);
        if let Err(e) = format_result(&result, effective_config, &mut writer, TAB_FIELD_WIDTH_STDIN)
            && e.kind() != std::io::ErrorKind::BrokenPipe
            && !no_messages
        {
            eprintln!("grep: write error: {e}");
        }
        let _ = writer.flush();
    }

    if found_match { 0 } else { 1 }
}

fn run_files(
    config: ResolvedConfig,
    pattern: Arc<CompiledPattern>,
    output_config: OutputConfig,
    no_messages: bool,
) -> i32 {
    let no_index = config.no_index;
    let invert_match = config.invert_match;
    let threads = config.threads;
    let max_file_size = config.max_file_size;

    // --- Trigram index: load and compute candidate filter ---
    let search_root = config
        .paths
        .first()
        .and_then(|p| if config.recursive { std::fs::canonicalize(p).ok() } else { None });

    let (candidate_filter, index_loaded) = if !no_index && let Some(ref root) = search_root {
        let trigrams = pattern.required_trigrams();
        if let Some(index) = TrigramIndex::load(root) {
            if !trigrams.is_empty() && !index.needs_rebuild() {
                let mut candidates = index.candidate_files(&trigrams);
                let total = index.file_count();
                // Skip filtering when trigrams are too common (>= 90% of files match)
                if total > 0 && candidates.len() * 10 >= total * 9 {
                    (None, true)
                } else {
                    for stale in index.stale_files() {
                        candidates.insert(stale);
                    }
                    (Some(candidates), true)
                }
            } else {
                (None, true)
            }
        } else {
            (None, false)
        }
    } else {
        (None, false)
    };

    let candidate_filter = candidate_filter.map(Arc::new);
    let should_build_index = !no_index && search_root.is_some() && !index_loaded;

    let (path_tx, path_rx) = bounded::<PathBuf>(256);

    let (walked_send, walked_recv) = if should_build_index {
        let (s, r) = kanal::unbounded::<PathBuf>();
        (Some(s), Some(r))
    } else {
        (None, None)
    };

    let skipped_files: Arc<Mutex<Vec<SkippedFile>>> = Arc::new(Mutex::new(Vec::new()));
    let skipped_for_walker = Arc::clone(&skipped_files);

    let filter_for_walker = candidate_filter;
    let walker_handle = match std::thread::Builder::new()
        .name("fg-walker".into())
        .spawn(move || {
            let (tx_inner, rx_inner) = bounded::<PathBuf>(256);
            std::thread::scope(|s| {
                let config_ref = &config;
                let skipped_ref = &skipped_for_walker;
                s.spawn(|| {
                    let walk_threads = (config_ref.threads / 4).clamp(2, 4);
                    walk(config_ref, tx_inner, walk_threads, skipped_ref);
                });
                for p in rx_inner {
                    if let Some(ref filter) = filter_for_walker
                        && !filter.contains(&p)
                    {
                        continue;
                    }
                    if let Some(ref wtx) = walked_send {
                        let _ = wtx.send(p.clone());
                    }
                    let _ = path_tx.send(p);
                }
            });
            // Drop walked_send to close the channel
            drop(walked_send);
        }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("grep: failed to spawn walker thread: {e}");
            return 2;
        }
    };

    // Shared stdout writer behind a mutex — workers flush per-file buffers here.
    let shared_writer = Arc::new(Mutex::new(BufWriter::new(std::io::stdout())));
    let found_match = Arc::new(AtomicBool::new(false));

    let pool = ThreadPool::new(threads, "fg-search", {
        let pattern = Arc::clone(&pattern);
        let shared_writer = Arc::clone(&shared_writer);
        let found_match = Arc::clone(&found_match);
        let output_config = output_config.clone();
        move || {
            let pattern = Arc::clone(&pattern);
            let shared_writer = Arc::clone(&shared_writer);
            let found_match = Arc::clone(&found_match);
            let output_config = output_config.clone();
            let mut read_buf = Vec::with_capacity(256 * 1024);
            let mut out_buf: Vec<u8> = Vec::with_capacity(64 * 1024);
            while let Ok(path) = path_rx.recv() {
                out_buf.clear();
                match search_file_streaming_reuse(
                    &path,
                    &pattern,
                    invert_match,
                    &output_config,
                    &mut out_buf,
                    &mut read_buf,
                ) {
                    Ok(count) => {
                        if count > 0 {
                            found_match.store(true, Ordering::Relaxed);
                        }
                        let should_flush = !output_config.quiet
                            && (count > 0
                                || output_config.count
                                || output_config.files_without_match);
                        if should_flush
                            && let Ok(mut w) = shared_writer.lock()
                            && let Err(e) = w.write_all(&out_buf)
                            && e.kind() == std::io::ErrorKind::BrokenPipe
                        {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::BrokenPipe && !no_messages {
                            eprintln!("grep: {}: {e}", path.display());
                        }
                    }
                }
            }
        }
    });

    pool.join();
    if let Ok(mut w) = shared_writer.lock() {
        let _ = w.flush();
    }

    walker_handle.join().ok();

    if should_build_index
        && let Some(ref root) = search_root
        && let Some(rx) = walked_recv
    {
        let paths: Vec<PathBuf> = std::iter::from_fn(|| rx.try_recv().ok().flatten()).collect();
        if !paths.is_empty() {
            let index = TrigramIndex::build(root, &paths);
            let _ = index.save();
            evict_if_needed();
        }
    }

    if !no_messages
        && let Ok(skipped) = skipped_files.lock()
        && !skipped.is_empty()
    {
        if output_config.is_json() {
            let stderr = std::io::stderr().lock();
            let mut writer = BufWriter::new(stderr);
            for sf in skipped.iter() {
                let _ =
                    write_json_size_limit_warning(&mut writer, &sf.path, sf.size, max_file_size);
            }
            let _ = writer.flush();
        } else {
            eprintln!();
            eprintln!("WARNING: {} file(s) skipped due to size limit:", skipped.len());
            for sf in skipped.iter() {
                let size_mb = sf.size as f64 / (1024.0 * 1024.0);
                eprintln!("  - {} ({:.1} MB)", sf.path.display(), size_mb);
            }
            eprintln!();
            eprintln!("These files may cause grep to hang. To search them anyway, re-run with:");
            eprintln!("  FASTGREP_NO_LIMIT=1 grep ...");
            eprintln!("Or adjust the threshold: --max-file-size=<BYTES> (current: 100 MiB)");
        }
    }

    if found_match.load(Ordering::Relaxed) { 0 } else { 1 }
}

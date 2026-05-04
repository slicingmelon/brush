//! ripgrep-style grep adapter — bundled `rg` / `ripgrep` / `grep` /
//! `egrep` / `fgrep` using `regex` + `pcre2` directly, with `ignore`
//! (the same crate ripgrep itself uses) for gitignore-aware filesystem
//! walks plus `globset` for the `-g` / `--glob` filter system.
//!
//! ## Why this isn't real ripgrep
//!
//! `BurntSushi`'s `ripgrep` crate on crates.io ships as a binary only —
//! its `crates/core/` is not exposed as a library, confirmed in
//! [`BurntSushi`/ripgrep#2509](https://github.com/BurntSushi/ripgrep/discussions/2509).
//! There is no `pub fn rg_main(args)` to delegate to, the way
//! `uutils/sed` exposes `sed::sed::uumain(args)`.
//!
//! The fork-side options for embedding ripgrep behavior are:
//!
//! 1. **Vendor `crates/core/`** as a workspace member exposing
//!    `pub fn rg_main(args)` (highest fidelity, ~5k lines of code to
//!    track).
//! 2. **Re-implement using `BurntSushi`'s published library family**
//!    (`grep`, `grep-searcher`, `grep-regex`, `grep-pcre2`,
//!    `grep-printer`, `ignore`, `globset`, `walkdir`).
//! 3. **Roll a thin line-based adapter** over `regex` + `pcre2` +
//!    `ignore` + `globset` — what this module does. Keeps the binary
//!    size and dependency surface bounded; gives up some of ripgrep's
//!    speed and the more exotic flag semantics.
//!
//! Layer 2 of the [`bundled-extras-cli-fidelity`][p] plan is the
//! vendoring path. This module is Layer 1: full flag *recognition*
//! via `clap`-derive so agents don't get "unknown option" errors,
//! plus a more-complete behavior surface than the previous hand-rolled
//! parser.
//!
//! [p]: ../../docs/planning/bundled-extras-cli-fidelity.md
//!
//! ## Mode-aware behavior
//!
//! The same engine drives four registered names; behavior branches at
//! the entry function level on [`Mode`]:
//!
//! | Mode | `gitignore` honored? | smart-case | auto-recurse on dir? | `-s` |
//! |---|---|---|---|---|
//! | [`Mode::Rg`] | yes | yes | yes | `--case-sensitive` |
//! | [`Mode::Grep`] | no | no | no (errors w/o `-r`) | `--no-messages` |
//! | [`Mode::Egrep`] | no (= `grep -E`) | no | no | `--no-messages` |
//! | [`Mode::Fgrep`] | no (= `grep -F`) | no | no | `--no-messages` |
//!
//! GNU-grep–only shortcuts handled by [`preprocess_argv`]:
//! `-NUM` → `-C NUM`, `-y` → `-i`, `-s` → `--no-messages` (only in
//! grep modes; in rg mode `-s` keeps its ripgrep meaning).

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
    clippy::module_name_repetitions,
    reason = "ripgrep CLI orchestration is intrinsically branchy and parameter-heavy; refactoring obscures the flag-by-flag mapping"
)]

use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use ignore::WalkBuilder;
use ignore::types::TypesBuilder;

const ADAPTER_VERSION: &str = "0.2.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Rg,
    Grep,
    Egrep,
    Fgrep,
}

impl Mode {
    const fn cmd_name(self) -> &'static str {
        match self {
            Self::Rg => "rg",
            Self::Grep => "grep",
            Self::Egrep => "egrep",
            Self::Fgrep => "fgrep",
        }
    }

    const fn is_grep_family(self) -> bool {
        matches!(self, Self::Grep | Self::Egrep | Self::Fgrep)
    }
}

pub(crate) fn rg_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Rg)
}

/// `grep` — GNU grep semantics with PCRE2 via `-P`.
pub(crate) fn grep_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Grep)
}

/// `egrep` — GNU `grep -E` (extended regex, the default in our engine).
pub(crate) fn egrep_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Egrep)
}

/// `fgrep` — GNU `grep -F` (fixed-string match).
pub(crate) fn fgrep_main(args: Vec<OsString>) -> i32 {
    run(args, Mode::Fgrep)
}

// =====================================================================
// CLI
// =====================================================================
//
// One clap-derive struct serves all four registered names; mode-aware
// dispatch happens before the parser via [`preprocess_argv`] (handling
// the `-s` and `-NUM` GNU-vs-rg differences) and after the parser via
// [`Cfg::from_cli`] (defaults).
//
// When extending: prefer adding both short and long. If a short flag
// would conflict between GNU and ripgrep, define it for the rg meaning
// and rewrite it in [`preprocess_argv`] for the grep family.

#[derive(clap::Parser, Debug)]
#[command(
    name = "rg",
    version = ADAPTER_VERSION,
    about = "Recursively search for patterns. Bundled regex+PCRE2+ignore implementation.",
    long_about = None,
    disable_help_flag = true,
    disable_version_flag = true,
    override_usage = "rg [OPTIONS] PATTERN [PATH...]\n       rg [OPTIONS] -e PATTERN [-e PATTERN ...] [PATH...]\n       rg [OPTIONS] -f PATTERNFILE [PATH...]",
)]
struct Cli {
    /// Show help and exit. ripgrep / GNU grep both use long-only `--help`;
    /// the short `-h` is reserved for `--no-filename`.
    #[arg(long = "help", action = clap::ArgAction::Help)]
    help_flag: Option<bool>,

    // ---- Pattern selection ---------------------------------------
    #[arg(short = 'e', long = "regexp", value_name = "PATTERN")]
    pattern_e: Vec<String>,

    #[arg(short = 'f', long = "file", value_name = "PATTERNFILE")]
    pattern_file: Vec<PathBuf>,

    #[arg(short = 'F', long = "fixed-strings")]
    fixed: bool,

    #[arg(short = 'E', long = "extended-regexp")]
    extended: bool,

    /// Basic regex (treated as ERE by our engine; agents rarely use BRE)
    #[arg(short = 'G', long = "basic-regexp")]
    basic: bool,

    #[arg(short = 'P', long = "perl-regexp")]
    pcre2: bool,

    #[arg(long = "engine", value_name = "ENGINE", default_value = "default")]
    engine: String,

    // ---- Match control -------------------------------------------
    #[arg(short = 'i', long = "ignore-case")]
    ignore_case: bool,

    #[arg(short = 'S', long = "smart-case")]
    smart_case: bool,

    #[arg(long = "case-sensitive")]
    case_sensitive: bool,

    #[arg(short = 'w', long = "word-regexp")]
    word: bool,

    #[arg(short = 'x', long = "line-regexp")]
    line_regexp: bool,

    #[arg(short = 'v', long = "invert-match")]
    invert: bool,

    #[arg(short = 'm', long = "max-count", value_name = "NUM")]
    max_count: Option<u64>,

    #[arg(short = 'U', long = "multiline")]
    multiline: bool,

    #[arg(long = "multiline-dotall")]
    multiline_dotall: bool,

    #[arg(long = "no-unicode")]
    no_unicode: bool,

    #[arg(long = "no-pcre2-unicode")]
    no_pcre2_unicode: bool,

    // ---- Output control ------------------------------------------
    #[arg(short = 'n', long = "line-number")]
    line_number: bool,

    #[arg(short = 'N', long = "no-line-number")]
    no_line_number: bool,

    #[arg(long = "column")]
    column: bool,

    #[arg(short = 'c', long = "count")]
    count: bool,

    #[arg(long = "count-matches")]
    count_matches: bool,

    #[arg(short = 'l', long = "files-with-matches")]
    files_with_matches: bool,

    #[arg(short = 'L', long = "files-without-match")]
    files_without_match: bool,

    /// Suppress filename prefix (GNU: -h)
    #[arg(short = 'h', long = "no-filename")]
    no_filename: bool,

    /// Force filename prefix (GNU: -H)
    #[arg(short = 'H', long = "with-filename")]
    with_filename: bool,

    #[arg(short = 'o', long = "only-matching")]
    only_matching: bool,

    #[arg(long = "passthru", visible_alias = "passthrough")]
    passthru: bool,

    #[arg(short = 'A', long = "after-context", value_name = "NUM")]
    after_context: Option<usize>,

    #[arg(short = 'B', long = "before-context", value_name = "NUM")]
    before_context: Option<usize>,

    #[arg(short = 'C', long = "context", value_name = "NUM")]
    context: Option<usize>,

    #[arg(long = "context-separator", value_name = "SEP")]
    context_separator: Option<String>,

    #[arg(long = "no-context-separator")]
    no_context_separator: bool,

    #[arg(short = 'b', long = "byte-offset")]
    byte_offset: bool,

    #[arg(long = "trim")]
    trim: bool,

    #[arg(long = "vimgrep")]
    vimgrep: bool,

    #[arg(long = "json")]
    json: bool,

    #[arg(long = "no-heading")]
    no_heading: bool,

    #[arg(long = "heading")]
    heading: bool,

    #[arg(long = "label", value_name = "LABEL")]
    label: Option<String>,

    #[arg(short = 'q', long = "quiet", visible_alias = "silent")]
    quiet: bool,

    #[arg(short = '0', long = "null")]
    null: bool,

    /// GNU grep `--null` short
    #[arg(short = 'Z', hide = true)]
    null_z: bool,

    #[arg(long = "null-data")]
    null_data: bool,

    /// ripgrep: search inside compressed files. We accept the flag
    /// (no error) but currently don't decompress.
    #[arg(short = 'z', long = "search-zip")]
    search_zip: bool,

    #[arg(short = 'T', long = "initial-tab")]
    initial_tab: bool,

    #[arg(long = "color", visible_alias = "colour", value_name = "WHEN")]
    color: Option<String>,

    #[arg(long = "no-color")]
    no_color: bool,

    #[arg(long = "stats")]
    stats: bool,

    #[arg(long = "debug")]
    debug: bool,

    /// ripgrep: report adapter version (separate from `--version`)
    #[arg(long = "version", short = 'V')]
    version_flag: bool,

    // ---- File / path selection -----------------------------------
    #[arg(
        short = 'r',
        long = "recursive",
        visible_alias = "dereference-recursive"
    )]
    recursive: bool,

    /// `-R` is the same as `-r` for our purposes
    #[arg(short = 'R', hide = true)]
    recursive_r: bool,

    #[arg(short = 'a', long = "text")]
    text: bool,

    #[arg(long = "binary")]
    binary: bool,

    #[arg(long = "no-binary")]
    no_binary: bool,

    #[arg(long = "binary-files", value_name = "TYPE")]
    binary_files: Option<String>,

    #[arg(long = "include", value_name = "GLOB")]
    include: Vec<String>,

    #[arg(long = "exclude", value_name = "GLOB")]
    exclude: Vec<String>,

    #[arg(long = "exclude-dir", value_name = "GLOB")]
    exclude_dir: Vec<String>,

    #[arg(long = "exclude-from", value_name = "FILE")]
    exclude_from: Vec<PathBuf>,

    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    glob: Vec<String>,

    #[arg(long = "iglob", value_name = "GLOB")]
    iglob: Vec<String>,

    #[arg(long = "ignore-file", value_name = "PATH")]
    ignore_file: Vec<PathBuf>,

    #[arg(short = 't', long = "type", value_name = "TYPE")]
    type_: Vec<String>,

    #[arg(long = "type-not", value_name = "TYPE")]
    type_not: Vec<String>,

    #[arg(long = "type-list")]
    type_list: bool,

    #[arg(long = "type-add", value_name = "SPEC")]
    type_add: Vec<String>,

    #[arg(long = "type-clear", value_name = "TYPE")]
    type_clear: Vec<String>,

    #[arg(long = "no-ignore")]
    no_ignore: bool,

    #[arg(long = "no-ignore-vcs")]
    no_ignore_vcs: bool,

    #[arg(long = "no-ignore-global")]
    no_ignore_global: bool,

    #[arg(long = "no-ignore-parent")]
    no_ignore_parent: bool,

    #[arg(long = "no-ignore-dot")]
    no_ignore_dot: bool,

    #[arg(long = "no-ignore-exclude")]
    no_ignore_exclude: bool,

    #[arg(long = "no-ignore-files")]
    no_ignore_files: bool,

    #[arg(long = "no-ignore-messages")]
    no_ignore_messages: bool,

    #[arg(long = "hidden", visible_alias = "no-hidden")]
    hidden: bool,

    /// follow symlinks
    #[arg(long = "follow", visible_alias = "dereference")]
    follow: bool,

    #[arg(long = "max-depth", value_name = "NUM")]
    max_depth: Option<usize>,

    #[arg(long = "max-filesize", value_name = "SIZE")]
    max_filesize: Option<String>,

    #[arg(long = "one-file-system")]
    one_file_system: bool,

    /// GNU grep `--devices=ACTION` (read|skip)
    #[arg(short = 'D', long = "devices", value_name = "ACTION")]
    devices: Option<String>,

    /// GNU grep `--directories=ACTION` (read|skip|recurse)
    #[arg(short = 'd', long = "directories", value_name = "ACTION")]
    directories: Option<String>,

    /// `--no-messages` (GNU `-s`; rewritten in `preprocess_argv` for grep mode)
    #[arg(long = "no-messages")]
    no_messages: bool,

    // ---- Performance / runtime -----------------------------------
    #[arg(short = 'j', long = "threads", value_name = "NUM")]
    threads: Option<usize>,

    #[arg(long = "mmap")]
    mmap: bool,

    #[arg(long = "no-mmap")]
    no_mmap: bool,

    #[arg(long = "encoding", value_name = "ENC")]
    encoding: Option<String>,

    #[arg(long = "pre", value_name = "COMMAND")]
    pre: Option<String>,

    #[arg(long = "pre-glob", value_name = "GLOB")]
    pre_glob: Vec<String>,

    #[arg(long = "no-config")]
    no_config: bool,

    #[arg(long = "line-buffered")]
    line_buffered: bool,

    #[arg(long = "block-buffered")]
    block_buffered: bool,

    #[arg(long = "sort", value_name = "SORTBY")]
    sort: Option<String>,

    #[arg(long = "sortr", value_name = "SORTBY")]
    sort_reverse: Option<String>,

    /// All positional arguments — first is pattern (unless -e/-f given),
    /// rest are paths
    #[arg(allow_hyphen_values = false, trailing_var_arg = true)]
    positional: Vec<OsString>,
}

// =====================================================================
// Argv preprocessing — handle GNU-vs-rg ambiguities
// =====================================================================

/// Rewrite mode-specific short flags before clap sees them.
///
/// We only rewrite where a single token has different meanings between
/// GNU grep and ripgrep, or where GNU has a shortcut clap can't express
/// natively (`-NUM`).
fn preprocess_argv(args: Vec<OsString>, mode: Mode) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len());
    let mut iter = args.into_iter();
    if let Some(name) = iter.next() {
        out.push(name);
    }
    let mut after_double_dash = false;
    for arg in iter {
        if after_double_dash {
            out.push(arg);
            continue;
        }
        let s = arg.to_string_lossy();
        if s == "--" {
            after_double_dash = true;
            out.push(arg);
            continue;
        }

        if mode.is_grep_family() {
            // GNU `-s` = `--no-messages`; rg `-s` = `--case-sensitive`.
            // Our clap schema does not define `-s` (we expose
            // `--case-sensitive` long-only) so this rewrite is unambiguous
            // for the grep family.
            if s == "-s" {
                out.push(OsString::from("--no-messages"));
                continue;
            }
            // GNU `-y` is documented as `-i` synonym
            if s == "-y" {
                out.push(OsString::from("-i"));
                continue;
            }
            // GNU `-NUM` shortcut for `-C NUM` (e.g. `-3` means 3 lines context)
            if let Some(rest) = s.strip_prefix('-')
                && !rest.is_empty()
                && rest.chars().all(|c| c.is_ascii_digit())
            {
                out.push(OsString::from("-C"));
                out.push(OsString::from(rest.to_string()));
                continue;
            }
        } else {
            // rg-mode rewrites
            // `-s` in rg means --case-sensitive — clap doesn't have a `-s`
            // short, so rewrite it
            if s == "-s" {
                out.push(OsString::from("--case-sensitive"));
                continue;
            }
        }
        out.push(arg);
    }
    out
}

// =====================================================================
// Resolved configuration (post-clap, mode-aware defaults applied)
// =====================================================================

#[derive(Default, Debug)]
struct Cfg {
    patterns: Vec<String>,
    paths: Vec<PathBuf>,
    ignore_case: bool,
    smart_case: bool,
    line_numbers: bool,
    column: bool,
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
    follow: bool,
    max_depth: Option<usize>,
    color_always: bool,
    null_separator: bool,
    initial_tab: bool,
    byte_offset: bool,
    trim: bool,
    no_messages: bool,
    passthru: bool,
    multiline: bool,
    multiline_dotall: bool,
    text_binary: bool,
    heading: bool,
    no_heading: bool,
    label: Option<String>,
    includes: Vec<String>,
    excludes: Vec<String>,
    exclude_dirs: Vec<String>,
    exclude_from: Vec<PathBuf>,
    globs: Vec<String>,
    iglobs: Vec<String>,
    ignore_files: Vec<PathBuf>,
    types: Vec<String>,
    types_not: Vec<String>,
    type_add: Vec<String>,
    type_clear: Vec<String>,
    directories_action: DirectoriesAction,
    devices_action: DevicesAction,
    binary_files: BinaryFiles,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum DirectoriesAction {
    #[default]
    Read, // GNU default: error on dir without -r
    Skip,
    Recurse,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum DevicesAction {
    #[default]
    Read,
    Skip,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum BinaryFiles {
    #[default]
    Auto,
    Text,
    WithoutMatch,
    Binary,
}

impl Cfg {
    fn from_cli(cli: Cli, mode: Mode) -> Result<Self, i32> {
        let mut cfg = Self::default();

        // Patterns: -e and -f take precedence over positional
        cfg.patterns.extend(cli.pattern_e);
        for pf in &cli.pattern_file {
            let content = std::fs::read_to_string(pf).map_err(|e| {
                eprintln!("{}: {}: {e}", mode.cmd_name(), pf.display());
                2
            })?;
            for line in content.lines() {
                cfg.patterns.push(line.to_string());
            }
        }

        // Positional: pattern (if no -e/-f) + paths
        let mut pos_iter = cli.positional.into_iter();
        if cfg.patterns.is_empty() {
            if let Some(p) = pos_iter.next() {
                cfg.patterns.push(p.to_string_lossy().into_owned());
            }
        }
        for path in pos_iter {
            cfg.paths.push(PathBuf::from(path));
        }

        if cfg.patterns.is_empty() {
            eprintln!("{}: no pattern provided", mode.cmd_name());
            return Err(2);
        }

        // Mode-driven flag overrides for egrep/fgrep
        cfg.fixed = cli.fixed || mode == Mode::Fgrep;
        // -E / -G are no-ops because our engine is RE2/ERE-equivalent;
        // we keep them recognized so agents don't error out.
        let _ = cli.extended;
        let _ = cli.basic;
        cfg.pcre2 = cli.pcre2 || cli.engine.eq_ignore_ascii_case("pcre2");

        // Match control
        cfg.ignore_case = cli.ignore_case;
        // smart-case: rg-mode default; grep-mode opt-in only
        cfg.smart_case = if cli.case_sensitive {
            false
        } else if cli.smart_case {
            true
        } else {
            // rg defaults to smart-case OFF too — it's only with -S.
            // Our previous adapter never had smart-case at all, so leave OFF
            // unless explicitly requested.
            false
        };
        cfg.word = cli.word;
        cfg.whole_line = cli.line_regexp;
        cfg.invert_match = cli.invert;
        cfg.max_count = cli.max_count;
        cfg.multiline = cli.multiline;
        cfg.multiline_dotall = cli.multiline_dotall;

        // Output control
        cfg.line_numbers = cli.line_number && !cli.no_line_number;
        cfg.column = cli.column;
        cfg.count = cli.count || cli.count_matches;
        cfg.files_with_matches = cli.files_with_matches;
        cfg.files_without_matches = cli.files_without_match;
        cfg.only_matching = cli.only_matching;
        cfg.passthru = cli.passthru;
        cfg.byte_offset = cli.byte_offset;
        cfg.trim = cli.trim;
        cfg.quiet = cli.quiet;
        cfg.heading = cli.heading;
        cfg.no_heading = cli.no_heading;
        cfg.label = cli.label;
        cfg.no_messages = cli.no_messages;
        cfg.initial_tab = cli.initial_tab;
        cfg.null_separator = cli.null || cli.null_z || cli.null_data;
        cfg.show_filename = if cli.no_filename {
            Some(false)
        } else if cli.with_filename {
            Some(true)
        } else {
            None
        };

        // Context
        if let Some(c) = cli.context {
            cfg.after = c;
            cfg.before = c;
        }
        if let Some(a) = cli.after_context {
            cfg.after = a;
        }
        if let Some(b) = cli.before_context {
            cfg.before = b;
        }

        // Color
        cfg.color_always = matches!(
            cli.color.as_deref(),
            Some("always" | "yes" | "force") | None if cli.color.is_some()
        ) && !cli.no_color;

        // File selection
        // ripgrep auto-recurses on dirs; GNU grep does not (errors w/o -r).
        cfg.recursive = cli.recursive || cli.recursive_r;
        cfg.includes = cli.include;
        cfg.excludes = cli.exclude;
        cfg.exclude_dirs = cli.exclude_dir;
        cfg.exclude_from = cli.exclude_from;
        cfg.globs = cli.glob;
        cfg.iglobs = cli.iglob;
        cfg.ignore_files = cli.ignore_file;
        cfg.types = cli.type_;
        cfg.types_not = cli.type_not;
        cfg.type_add = cli.type_add;
        cfg.type_clear = cli.type_clear;
        cfg.no_ignore = cli.no_ignore
            || cli.no_ignore_vcs
            || cli.no_ignore_global
            || cli.no_ignore_parent
            || cli.no_ignore_dot
            || cli.no_ignore_exclude
            || cli.no_ignore_files
            || mode.is_grep_family();
        cfg.hidden = cli.hidden || mode.is_grep_family();
        cfg.follow = cli.follow;
        cfg.max_depth = cli.max_depth;

        cfg.directories_action = match cli.directories.as_deref() {
            Some("read") => DirectoriesAction::Read,
            Some("skip") => DirectoriesAction::Skip,
            Some("recurse") => DirectoriesAction::Recurse,
            None => {
                if mode.is_grep_family() {
                    DirectoriesAction::Read
                } else {
                    DirectoriesAction::Recurse
                }
            }
            _ => DirectoriesAction::Read,
        };
        cfg.devices_action = match cli.devices.as_deref() {
            Some("skip") => DevicesAction::Skip,
            _ => DevicesAction::Read,
        };
        cfg.binary_files = match cli.binary_files.as_deref() {
            Some("text") => BinaryFiles::Text,
            Some("without-match") => BinaryFiles::WithoutMatch,
            Some("binary") => BinaryFiles::Binary,
            _ => {
                if cli.text || cli.binary {
                    BinaryFiles::Text
                } else {
                    BinaryFiles::Auto
                }
            }
        };
        cfg.text_binary =
            cli.text || matches!(cfg.binary_files, BinaryFiles::Text | BinaryFiles::Binary);

        // ignored-but-accepted flags (no-op): vimgrep, json, stats, debug,
        // mmap/no-mmap, encoding, pre, pre-glob, no-config, line-buffered,
        // block-buffered, sort, sortr, threads, no-unicode, no-pcre2-unicode,
        // search-zip, no-binary, no-context-separator, context-separator,
        // one-file-system, max-filesize, ignore-messages
        let _ = (
            cli.vimgrep,
            cli.json,
            cli.stats,
            cli.debug,
            cli.mmap,
            cli.no_mmap,
            cli.encoding,
            cli.pre,
            cli.pre_glob,
            cli.no_config,
            cli.line_buffered,
            cli.block_buffered,
            cli.sort,
            cli.sort_reverse,
            cli.threads,
            cli.no_unicode,
            cli.no_pcre2_unicode,
            cli.search_zip,
            cli.no_binary,
            cli.no_context_separator,
            cli.context_separator,
            cli.one_file_system,
            cli.max_filesize,
            cli.no_ignore_messages,
        );

        // Default paths: stdin
        if cfg.paths.is_empty() {
            cfg.paths.push(PathBuf::from("-"));
        }

        // Default show_filename: yes if any path is a directory or
        // multiple paths
        if cfg.show_filename.is_none() {
            let any_dir = cfg
                .paths
                .iter()
                .any(|p| p.as_path() != Path::new("-") && p.is_dir());
            cfg.show_filename = Some(any_dir || cfg.paths.len() > 1);
            // rg-mode auto-recurses on directories; grep-mode does not
            if any_dir && !mode.is_grep_family() {
                cfg.recursive = true;
            }
        }

        Ok(cfg)
    }
}

// =====================================================================
// Entry-point dispatch
// =====================================================================

fn run(args: Vec<OsString>, mode: Mode) -> i32 {
    let argv = preprocess_argv(args, mode);
    let cli = match Cli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => {
            let exit = match e.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
            let _ = e.print();
            return exit;
        }
    };

    // --version handled manually so we can show the right banner per mode
    if cli.version_flag {
        print_version(mode);
        return 0;
    }
    // --type-list is independent of pattern, handle before Cfg validation
    if cli.type_list {
        print_type_list();
        return 0;
    }

    let cfg = match Cfg::from_cli(cli, mode) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let engine = match build_engine(&cfg) {
        Ok(e) => e,
        Err(e) => {
            if !cfg.no_messages {
                eprintln!("{}: {e}", mode.cmd_name());
            }
            return 2;
        }
    };

    let any_match = AtomicBool::new(false);
    let stdout = io::stdout();
    let out = Mutex::new(stdout.lock());

    for path in &cfg.paths {
        if path.as_os_str() == "-" {
            let stdin = io::stdin();
            let label = cfg.label.as_deref().unwrap_or("(standard input)");
            search_reader(
                BufReader::new(stdin.lock()),
                label,
                &engine,
                &cfg,
                &out,
                &any_match,
                mode,
            );
            continue;
        }
        if path.is_dir() {
            match cfg.directories_action {
                DirectoriesAction::Skip => {}
                DirectoriesAction::Read => {
                    if cfg.recursive {
                        walk_dir(path, &engine, &cfg, &out, &any_match, mode);
                    } else if !cfg.no_messages {
                        eprintln!("{}: {}: Is a directory", mode.cmd_name(), path.display());
                    }
                }
                DirectoriesAction::Recurse => {
                    walk_dir(path, &engine, &cfg, &out, &any_match, mode);
                }
            }
        } else {
            search_path(path, &engine, &cfg, &out, &any_match, mode);
        }
    }
    i32::from(!any_match.load(Ordering::SeqCst))
}

fn print_version(mode: Mode) {
    let canonical = mode.cmd_name();
    println!("{canonical} (brush-bundled-extras regex+pcre2+ignore+globset) {ADAPTER_VERSION}");
    println!(
        "PCRE2 enabled · gitignore-aware via `ignore` crate · `-t TYPE` via ripgrep type defs"
    );
    println!("Bundled inside brush — for full ripgrep, install BurntSushi/ripgrep externally.");
}

fn print_type_list() {
    let mut tb = TypesBuilder::new();
    tb.add_defaults();
    let types = tb.definitions();
    for def in types {
        let globs: Vec<&str> = def.globs().iter().map(String::as_str).collect();
        println!("{}: {}", def.name(), globs.join(", "));
    }
}

// =====================================================================
// Engine
// =====================================================================

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

    // Smart-case: only force case-insensitive if pattern has no uppercase
    let force_icase =
        cfg.ignore_case || (cfg.smart_case && !pattern.chars().any(|c| c.is_uppercase()));

    if cfg.pcre2 {
        let mut b = pcre2::bytes::RegexBuilder::new();
        b.caseless(force_icase);
        // multiline-dotall: in PCRE2, that's `(?s)` + `(?m)` modifiers via flags
        if cfg.multiline_dotall {
            b.dotall(true);
        }
        if cfg.multiline {
            b.multi_line(true);
        }
        b.build(&pattern)
            .map(Engine::Pcre2)
            .map_err(|e| format!("PCRE2 compile error: {e}"))
    } else {
        let mut prefix = String::new();
        if force_icase {
            prefix.push_str("(?i)");
        }
        if cfg.multiline {
            prefix.push_str("(?m)");
        }
        if cfg.multiline_dotall {
            prefix.push_str("(?s)");
        }
        let pat = format!("{prefix}{pattern}");
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

/// All non-overlapping matches in `line`, in order. Used by `-o` /
/// `--only-matching` so multi-match lines emit one row per match (real
/// ripgrep / GNU grep behavior).
fn engine_find_all(engine: &Engine, line: &[u8]) -> Vec<(usize, usize)> {
    match engine {
        Engine::Regex(r) => {
            let Ok(s) = std::str::from_utf8(line) else {
                return Vec::new();
            };
            r.find_iter(s).map(|m| (m.start(), m.end())).collect()
        }
        Engine::Pcre2(p) => {
            let mut out = Vec::new();
            let mut pos = 0;
            while pos <= line.len() {
                match p.find_at(line, pos) {
                    Ok(Some(m)) => {
                        let (s, e) = (m.start(), m.end());
                        out.push((s, e));
                        // Step forward to avoid infinite loop on zero-width matches.
                        pos = if e == s { e + 1 } else { e };
                    }
                    _ => break,
                }
            }
            out
        }
    }
}

// =====================================================================
// Filesystem walking
// =====================================================================

fn walk_dir(
    root: &Path,
    engine: &Engine,
    cfg: &Cfg,
    out: &Mutex<io::StdoutLock<'_>>,
    any_match: &AtomicBool,
    mode: Mode,
) {
    let mut wb = WalkBuilder::new(root);
    wb.standard_filters(!cfg.no_ignore);
    wb.hidden(!cfg.hidden);
    wb.follow_links(cfg.follow);
    if let Some(depth) = cfg.max_depth {
        wb.max_depth(Some(depth));
    }
    for ig in &cfg.ignore_files {
        wb.add_custom_ignore_filename(ig);
    }

    // Type filter via `ignore::types::TypesBuilder` — same defs ripgrep uses
    if !cfg.types.is_empty()
        || !cfg.types_not.is_empty()
        || !cfg.type_add.is_empty()
        || !cfg.type_clear.is_empty()
    {
        let mut tb = TypesBuilder::new();
        tb.add_defaults();
        for c in &cfg.type_clear {
            tb.clear(c);
        }
        for s in &cfg.type_add {
            if let Err(e) = tb.add_def(s) {
                if !cfg.no_messages {
                    eprintln!("{}: --type-add: {e}", mode.cmd_name());
                }
                return;
            }
        }
        for t in &cfg.types {
            tb.select(t);
        }
        for t in &cfg.types_not {
            tb.negate(t);
        }
        match tb.build() {
            Ok(types) => {
                wb.types(types);
            }
            Err(e) => {
                if !cfg.no_messages {
                    eprintln!("{}: --type: {e}", mode.cmd_name());
                }
                return;
            }
        }
    }

    // Glob filter via overrides
    let has_overrides = !cfg.includes.is_empty()
        || !cfg.excludes.is_empty()
        || !cfg.exclude_dirs.is_empty()
        || !cfg.globs.is_empty()
        || !cfg.iglobs.is_empty();
    if has_overrides {
        let mut overrides = ignore::overrides::OverrideBuilder::new(root);
        for pat in &cfg.includes {
            let _ = overrides.add(pat);
        }
        for pat in &cfg.excludes {
            let _ = overrides.add(&format!("!{pat}"));
        }
        for pat in &cfg.exclude_dirs {
            let _ = overrides.add(&format!("!{pat}"));
        }
        for pat in &cfg.globs {
            // ripgrep -g semantics: `!pat` excludes, `pat` includes
            let _ = overrides.add(pat);
        }
        for pat in &cfg.iglobs {
            // case-insensitive glob — best-effort: prepend `*` to match
            let _ = overrides.case_insensitive(true);
            let _ = overrides.add(pat);
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
        search_path(entry.path(), engine, cfg, out, any_match, mode);
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
    mode: Mode,
) {
    // Binary detection runs against a *separate* `File::open`. Sharing
    // a handle via `try_clone` on Windows can advance the underlying
    // file position, leaving the search-side BufReader pointing past
    // the data; using two opens keeps the search reader at byte 0.
    if matches!(cfg.binary_files, BinaryFiles::Auto) && !cfg.text_binary {
        if let Ok(mut g) = File::open(path) {
            let mut peek = [0u8; 8192];
            if let Ok(n) = g.read(&mut peek) {
                if peek[..n].contains(&0) {
                    // Skip without printing — matches ripgrep + grep default.
                    return;
                }
            }
        }
    }

    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            if !cfg.no_messages {
                eprintln!("{}: {}: {e}", mode.cmd_name(), path.display());
            }
            return;
        }
    };

    let label = path.to_string_lossy();
    search_reader(BufReader::new(f), &label, engine, cfg, out, any_match, mode);
}

// =====================================================================
// Per-reader search + output
// =====================================================================

fn search_reader<R: BufRead>(
    mut reader: R,
    label: &str,
    engine: &Engine,
    cfg: &Cfg,
    out: &Mutex<io::StdoutLock<'_>>,
    any_match: &AtomicBool,
    mode: Mode,
) {
    let mut buf: Vec<u8> = Vec::new();
    let mut line_no: u64 = 0;
    let mut byte_pos: u64 = 0;
    let mut match_count: u64 = 0;
    let mut had_match = false;
    let mut before_buf: VecDeque<(u64, u64, Vec<u8>)> = VecDeque::with_capacity(cfg.before + 1);
    let mut after_remaining: usize = 0;
    let show_path = cfg.show_filename.unwrap_or(false);
    let _ = mode;

    loop {
        buf.clear();
        let n = match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        line_no += 1;
        let line_start_byte = byte_pos;
        byte_pos += n as u64;
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
            if !cfg.count {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                while let Some((bn, bb, bl)) = before_buf.pop_front() {
                    write_line(&mut *o, show_path, label, cfg, bn, bb, &bl, false, None);
                }
                if cfg.only_matching {
                    // Real ripgrep / GNU grep emit ALL non-overlapping
                    // matches on a line, each on its own row. Loop here
                    // rather than printing only `m`.
                    for (s, e) in engine_find_all(engine, line) {
                        write_line(
                            &mut *o,
                            show_path,
                            label,
                            cfg,
                            line_no,
                            line_start_byte + s as u64,
                            &line[s..e],
                            true,
                            Some((0, e - s)),
                        );
                    }
                } else {
                    write_line(
                        &mut *o,
                        show_path,
                        label,
                        cfg,
                        line_no,
                        line_start_byte,
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
            if cfg.passthru && !cfg.count {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                write_line(
                    &mut *o,
                    show_path,
                    label,
                    cfg,
                    line_no,
                    line_start_byte,
                    line,
                    false,
                    None,
                );
            } else if after_remaining > 0 {
                let mut o = out
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                write_line(
                    &mut *o,
                    show_path,
                    label,
                    cfg,
                    line_no,
                    line_start_byte,
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
                before_buf.push_back((line_no, line_start_byte, line.to_vec()));
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
    cfg: &Cfg,
    n: u64,
    byte_offset: u64,
    line: &[u8],
    is_match: bool,
    highlight: Option<(usize, usize)>,
) {
    let sep = if is_match { ':' } else { '-' };
    let null_sep = if cfg.null_separator { '\0' } else { sep };
    if show_path {
        let _ = write!(out, "{label}{null_sep}");
    }
    if cfg.line_numbers {
        let _ = write!(out, "{n}{sep}");
    }
    if cfg.column {
        let col = highlight.map_or(0, |(s, _)| s + 1);
        let _ = write!(out, "{col}{sep}");
    }
    if cfg.byte_offset {
        let _ = write!(out, "{byte_offset}{sep}");
    }
    if cfg.initial_tab {
        let _ = out.write_all(b"\t");
    }
    let written: &[u8] = if cfg.trim {
        let mut start = 0;
        while start < line.len() && (line[start] == b' ' || line[start] == b'\t') {
            start += 1;
        }
        &line[start..]
    } else {
        line
    };
    let _ = out.write_all(written);
    let _ = out.write_all(b"\n");
}

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

/// Signature of a bundled command's entry point — same shape as
/// `brush-coreutils-builtins::BundledFn`. Re-declared here to avoid a
/// dependency on that crate; consumers (brush-shell) merge the two
/// registries by `HashMap::extend`.
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

//! MSYS-style ↔ native Windows path conversion (Layer 0 of the
//! `path-conversion-msys.md` plan).
//!
//! This is the single-path translator core that the `cygpath` builtin
//! and (in later cycles) the bundled-tools dispatch and outgoing-env
//! translator are built on top of.
//!
//! **Algorithm reference**: clean-room reimplementation informed by
//! [`stdlib-js/utils-convert-path`][stdlib] (Apache-2.0). No code copied
//! from MSYS2 / Cygwin (those are GPL).
//!
//! [stdlib]: https://github.com/stdlib-js/utils-convert-path
//!
//! Forms accepted on input (`<L>` = single ASCII letter, case-insensitive):
//!   * POSIX device root:    `/<L>`, `/<L>/...`
//!   * Cygdrive prefix:      `/cygdrive/<L>`, `/cygdrive/<L>/...`
//!   * Windows device root:  `<L>:`, `<L>:\...`, `<L>:/...`
//!   * Backslash-rooted MSYS: `\<L>`, `\<L>\...` (rarer but accepted)
//!   * Plain relative paths: `foo/bar`, `..\foo`, etc. — separators
//!     normalized to the target form; no device root added.
//!
//! Forms intentionally **not** translated:
//!   * UNC paths (`//server/share`, `\\server\share`)
//!   * Extended-length paths (`\\?\C:\...`)
//!   * DOS short names (`C:\PROGRA~1\`)

use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Output form for [`convert`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathForm {
    /// Native Windows form: `C:\foo\bar`. Drive letter uppercased.
    Win32,
    /// Mixed form: `C:/foo/bar`. Drive letter uppercased; forward slashes.
    Mixed,
    /// POSIX / MSYS form: `/c/foo/bar`. Drive letter lowercased.
    Posix,
}

/// Strips a leading `/cygdrive` (or `\cygdrive`) prefix if it is followed
/// by a single drive letter and a separator (or end-of-string). Otherwise
/// returns the input unchanged.
fn strip_cygdrive_prefix(s: &str) -> &str {
    let after = s
        .strip_prefix("/cygdrive")
        .or_else(|| s.strip_prefix(r"\cygdrive"));
    let Some(after) = after else { return s };

    // Must be followed by a separator + drive letter pattern, or we leave
    // the original alone (e.g. `/cygdrivelong/foo` is not a cygdrive form).
    let mut chars = after.chars();
    let Some(sep) = chars.next() else {
        // Just `/cygdrive` with no follow-on — leave alone.
        return s;
    };
    if !is_sep(sep) {
        return s;
    }
    let Some(drive) = chars.next() else {
        // `/cygdrive/` — not a drive reference.
        return s;
    };
    if !drive.is_ascii_alphabetic() {
        return s;
    }
    // Validate the third char: must be a separator or end-of-string.
    match chars.next() {
        None => after, // `/cygdrive/c`
        Some(c) if is_sep(c) => after,
        _ => s, // `/cygdrive/cd/...` is not a single-letter drive
    }
}

const fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Parses a leading POSIX device root (`/<L>` followed by separator or
/// end-of-string). Returns `(drive_letter, tail_after_separator)`.
fn parse_posix_device_root(s: &str) -> Option<(char, &str)> {
    let rest = s.strip_prefix('/').or_else(|| s.strip_prefix('\\'))?;
    let mut chars = rest.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() {
        return None;
    }
    let after_drive = chars.as_str();
    if after_drive.is_empty() {
        return Some((drive, ""));
    }
    // Next char must be a separator (otherwise it's e.g. `/cd` — a
    // directory name that happens to start with a letter).
    let sep = after_drive.chars().next()?;
    if !is_sep(sep) {
        return None;
    }
    Some((drive, &after_drive[sep.len_utf8()..]))
}

/// Parses a leading Windows device root (`<L>:` followed by separator
/// or end-of-string). Returns `(drive_letter, tail_after_separator)`.
fn parse_win_device_root(s: &str) -> Option<(char, &str)> {
    let mut chars = s.chars();
    let drive = chars.next()?;
    if !drive.is_ascii_alphabetic() {
        return None;
    }
    if chars.next()? != ':' {
        return None;
    }
    let after_colon = chars.as_str();
    if after_colon.is_empty() {
        return Some((drive, ""));
    }
    // Consume one or more separators (collapses `C:\\foo` → `C:\foo` in
    // win32 mode; matches stdlib's `[\\\/]+` regex).
    let mut bytes_consumed = 0usize;
    for c in after_colon.chars() {
        if is_sep(c) {
            bytes_consumed += c.len_utf8();
        } else {
            break;
        }
    }
    if bytes_consumed == 0 {
        // `C:foo` — drive-relative path. Not a device root in our sense.
        return None;
    }
    Some((drive, &after_colon[bytes_consumed..]))
}

/// Replaces all path separators in `s` with `target_sep` (one of `/` or
/// `\`). Returns a borrowed cow if no replacement was needed.
fn normalize_separators(s: &str, target_sep: char) -> Cow<'_, str> {
    let other = if target_sep == '/' { '\\' } else { '/' };
    if s.contains(other) {
        Cow::Owned(s.replace(other, &target_sep.to_string()))
    } else {
        Cow::Borrowed(s)
    }
}

/// Converts a path string to the requested form. Always succeeds; returns
/// the input unchanged (borrowed) when no conversion is needed.
///
/// Drive-letter case follows the conventional cygpath behavior:
///   * Win32 / Mixed → uppercase
///   * Posix         → lowercase
pub fn convert(s: &str, target: PathForm) -> Cow<'_, str> {
    match target {
        PathForm::Win32 => to_win32(s),
        PathForm::Mixed => to_mixed(s),
        PathForm::Posix => to_posix(s),
    }
}

fn to_win32(s: &str) -> Cow<'_, str> {
    let stripped = strip_cygdrive_prefix(s);

    if let Some((drive, tail)) = parse_posix_device_root(stripped) {
        let drive_upper = drive.to_ascii_uppercase();
        if tail.is_empty() {
            return Cow::Owned(format!("{drive_upper}:\\"));
        }
        let tail_native = tail.replace('/', "\\");
        return Cow::Owned(format!("{drive_upper}:\\{tail_native}"));
    }

    if let Some((drive, tail)) = parse_win_device_root(stripped) {
        let drive_upper = drive.to_ascii_uppercase();
        if tail.is_empty() {
            return Cow::Owned(format!("{drive_upper}:\\"));
        }
        let tail_native = tail.replace('/', "\\");
        return Cow::Owned(format!("{drive_upper}:\\{tail_native}"));
    }

    normalize_separators(stripped, '\\')
}

fn to_mixed(s: &str) -> Cow<'_, str> {
    let stripped = strip_cygdrive_prefix(s);

    if let Some((drive, tail)) = parse_posix_device_root(stripped) {
        let drive_upper = drive.to_ascii_uppercase();
        if tail.is_empty() {
            return Cow::Owned(format!("{drive_upper}:/"));
        }
        let tail_fwd = tail.replace('\\', "/");
        return Cow::Owned(format!("{drive_upper}:/{tail_fwd}"));
    }

    if let Some((drive, tail)) = parse_win_device_root(stripped) {
        let drive_upper = drive.to_ascii_uppercase();
        if tail.is_empty() {
            return Cow::Owned(format!("{drive_upper}:/"));
        }
        let tail_fwd = tail.replace('\\', "/");
        return Cow::Owned(format!("{drive_upper}:/{tail_fwd}"));
    }

    normalize_separators(stripped, '/')
}

fn to_posix(s: &str) -> Cow<'_, str> {
    let stripped = strip_cygdrive_prefix(s);

    if let Some((drive, tail)) = parse_win_device_root(stripped) {
        let drive_lower = drive.to_ascii_lowercase();
        if tail.is_empty() {
            return Cow::Owned(format!("/{drive_lower}"));
        }
        let tail_fwd = tail.replace('\\', "/");
        return Cow::Owned(format!("/{drive_lower}/{tail_fwd}"));
    }

    if let Some((drive, tail)) = parse_posix_device_root(stripped) {
        // Already POSIX — but ensure separators / drive case match.
        let drive_lower = drive.to_ascii_lowercase();
        if tail.is_empty() {
            // `/c` → `/c`; `/C` → `/c`.
            if drive == drive_lower && stripped.len() == 2 && stripped.starts_with('/') {
                return Cow::Borrowed(stripped);
            }
            return Cow::Owned(format!("/{drive_lower}"));
        }
        let tail_fwd = tail.replace('\\', "/");
        // If nothing changed, return the (possibly stripped-cygdrive) input
        // borrowed. Otherwise allocate.
        if drive == drive_lower
            && tail == tail_fwd
            && stripped.starts_with('/')
            && std::ptr::eq(stripped.as_ptr(), s.as_ptr())
        {
            return Cow::Borrowed(s);
        }
        return Cow::Owned(format!("/{drive_lower}/{tail_fwd}"));
    }

    normalize_separators(stripped, '/')
}

/// Splits `s` on `:` or `;` and converts each component to `target`.
/// The output uses `;` between components when `target` is `Win32` or
/// `Mixed` (Windows-native PATH separator) and `:` when `target` is
/// `Posix` (POSIX PATH separator).
///
/// Empty components (e.g. produced by a leading or trailing separator)
/// are preserved.
pub fn convert_path_list(s: &str, target: PathForm) -> Cow<'_, str> {
    // Detect the input separator. Prefer `;` if present (Windows PATH);
    // otherwise fall back to `:`. Mixed-separator input is uncommon and
    // we match MSYS2's behavior of using whichever appears.
    let in_sep = if s.contains(';') { ';' } else { ':' };
    let out_sep = match target {
        PathForm::Win32 | PathForm::Mixed => ';',
        PathForm::Posix => ':',
    };

    // Single-component fast path.
    if !s.contains(in_sep) {
        return convert(s, target);
    }

    let mut out = String::with_capacity(s.len());
    let mut first = true;
    for component in s.split(in_sep) {
        if !first {
            out.push(out_sep);
        }
        first = false;
        if component.is_empty() {
            continue;
        }
        // For posix output of a Windows-native path list, a bare drive
        // letter colon (`C:`) inside `;`-separated input would otherwise
        // confuse `:` re-joining; switching the output sep handles that.
        out.push_str(&convert(component, target));
    }
    Cow::Owned(out)
}

/// Conservative heuristic: does `s` look like a path string that *would*
/// benefit from translation? Used by Cycle 2 (bundled-tools dispatch)
/// and Cycle 4 (external-arg translation) to skip non-path argv elements.
///
/// Returns true only when `s` clearly carries a recognizable drive-root
/// form. Plain relative paths (e.g. `foo/bar`) return false — they
/// don't need translation between MSYS and native, and false-positives
/// here are the MSYS2 footgun this whole feature exists to avoid.
///
/// **Excluded**:
///   * Empty strings
///   * Option flags (start with `-`)
///   * URL-shaped strings (contain `://`)
///   * `key=value` shapes where `=` precedes any separator (e.g.
///     `--data=foo` would already be excluded by the leading-`-` rule;
///     this catches bare `KEY=value` env-style assignments)
pub fn looks_like_path(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if s.starts_with('-') {
        return false;
    }
    if s.contains("://") {
        return false;
    }
    // `key=value`: `=` precedes any separator.
    if let Some(eq_pos) = s.find('=') {
        let before_eq = &s[..eq_pos];
        if !before_eq.contains(['/', '\\']) {
            return false;
        }
    }

    let stripped = strip_cygdrive_prefix(s);
    parse_posix_device_root(stripped).is_some() || parse_win_device_root(stripped).is_some()
}

/// Translates an MSYS / Git-Bash / Cygwin style absolute path into a
/// native Windows path. Returns `None` if `path` is not in one of those
/// forms. Preserves the legacy contract: callers (`Shell::absolute_path`,
/// `commands.rs` external-exec) get `Some(_)` only when they should
/// substitute, leaving native paths and relative paths untouched.
///
/// Recognized forms (`<L>` is a single ASCII letter, treated case-
/// insensitively):
///   `/<L>`              → `<L>:\`
///   `/<L>/...`          → `<L>:\...`
///   `/cygdrive/<L>`     → `<L>:\`
///   `/cygdrive/<L>/...` → `<L>:\...`
///
/// Backslashes are accepted as path separators on input. Other leading-
/// slash paths (e.g. `/dev/null`, `/tmp/foo`) are left alone — those
/// aren't drive references, and platform-specific handlers like
/// `try_open_special_file` take care of the ones that need to.
///
/// This exists because Claude Code, Git Bash, MSYS2, and similar tooling
/// routinely hand brush MSYS-style absolute paths (e.g. as redirection
/// targets), but `Path::is_absolute` returns `false` for them on Windows,
/// causing `absolute_path` to drive-root-join them and produce mojibake
/// like `C:/c/Users/...`.
pub fn try_translate_msys_path(path: &Path) -> Option<PathBuf> {
    let s = path.to_str()?;

    // Only the POSIX/cygdrive forms are "translatable" in the legacy
    // sense — already-native and Windows-mixed inputs are returned None
    // so callers leave them alone.
    let stripped = strip_cygdrive_prefix(s);
    let (drive, tail) = parse_posix_device_root(stripped)?;
    let drive_upper = drive.to_ascii_uppercase();
    if tail.is_empty() {
        return Some(PathBuf::from(format!("{drive_upper}:\\")));
    }
    let tail_native = tail.replace('/', "\\");
    Some(PathBuf::from(format!("{drive_upper}:\\{tail_native}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- try_translate_msys_path: legacy contract ----

    #[test]
    fn translate_msys_drive_root() {
        assert_eq!(
            try_translate_msys_path(Path::new("/c")),
            Some(PathBuf::from(r"C:\"))
        );
        assert_eq!(
            try_translate_msys_path(Path::new("/C")),
            Some(PathBuf::from(r"C:\"))
        );
        assert_eq!(
            try_translate_msys_path(Path::new("/c/")),
            Some(PathBuf::from(r"C:\"))
        );
    }

    #[test]
    fn translate_msys_typical_paths() {
        assert_eq!(
            try_translate_msys_path(Path::new("/c/Users/foo")),
            Some(PathBuf::from(r"C:\Users\foo"))
        );
        assert_eq!(
            try_translate_msys_path(Path::new("/d/some/deep/path")),
            Some(PathBuf::from(r"D:\some\deep\path"))
        );
        // The exact failure-mode path that motivated the original fix.
        assert_eq!(
            try_translate_msys_path(Path::new(
                "/c/Users/P_SURU~1/AppData/Local/Temp/claude-3c0e-cwd"
            )),
            Some(PathBuf::from(
                r"C:\Users\P_SURU~1\AppData\Local\Temp\claude-3c0e-cwd"
            ))
        );
    }

    #[test]
    fn translate_msys_cygdrive_form() {
        assert_eq!(
            try_translate_msys_path(Path::new("/cygdrive/c/Users/foo")),
            Some(PathBuf::from(r"C:\Users\foo"))
        );
        assert_eq!(
            try_translate_msys_path(Path::new("/cygdrive/z")),
            Some(PathBuf::from(r"Z:\"))
        );
    }

    #[test]
    fn translate_msys_rejects_non_drive_paths() {
        assert_eq!(try_translate_msys_path(Path::new("/dev/null")), None);
        assert_eq!(try_translate_msys_path(Path::new("/tmp/foo")), None);
        assert_eq!(try_translate_msys_path(Path::new("/usr/bin/bash")), None);
        assert_eq!(try_translate_msys_path(Path::new("/cd")), None);
        assert_eq!(try_translate_msys_path(Path::new("/cd/foo")), None);
    }

    #[test]
    fn translate_msys_rejects_native_and_relative() {
        // Already-native paths are not "translatable" — let callers pass
        // them through unchanged.
        assert_eq!(try_translate_msys_path(Path::new(r"C:\Users\foo")), None);
        assert_eq!(try_translate_msys_path(Path::new("C:/Users/foo")), None);
        // Relative paths.
        assert_eq!(try_translate_msys_path(Path::new("foo/bar")), None);
        assert_eq!(try_translate_msys_path(Path::new("")), None);
    }

    // ---- convert(): always-succeeds API ----

    #[test]
    fn convert_posix_to_win32() {
        assert_eq!(convert("/c/foo/bar", PathForm::Win32), "C:\\foo\\bar");
        assert_eq!(convert("/C/foo/bar", PathForm::Win32), "C:\\foo\\bar");
        assert_eq!(convert("/c", PathForm::Win32), "C:\\");
        assert_eq!(convert("/c/", PathForm::Win32), "C:\\");
        assert_eq!(
            convert("/cygdrive/d/Users/foo", PathForm::Win32),
            "D:\\Users\\foo"
        );
    }

    #[test]
    fn convert_posix_to_mixed() {
        assert_eq!(convert("/c/foo/bar", PathForm::Mixed), "C:/foo/bar");
        assert_eq!(convert("/c", PathForm::Mixed), "C:/");
        assert_eq!(
            convert("/cygdrive/c/Users/foo", PathForm::Mixed),
            "C:/Users/foo"
        );
    }

    #[test]
    fn convert_posix_to_posix_passthrough() {
        // Already-posix inputs come back borrowed unchanged.
        let out = convert("/c/foo/bar", PathForm::Posix);
        assert_eq!(out, "/c/foo/bar");
        assert!(matches!(out, Cow::Borrowed(_)));
        // Cygdrive-prefixed inputs get the prefix stripped.
        assert_eq!(convert("/cygdrive/c/foo", PathForm::Posix), "/c/foo");
    }

    #[test]
    fn convert_posix_uppercases_to_lowercase_when_targeting_posix() {
        assert_eq!(convert("/C/foo/bar", PathForm::Posix), "/c/foo/bar");
        assert_eq!(convert("/Z", PathForm::Posix), "/z");
    }

    #[test]
    fn convert_win32_to_posix() {
        assert_eq!(convert(r"C:\foo\bar", PathForm::Posix), "/c/foo/bar");
        assert_eq!(convert(r"D:\foo\bar", PathForm::Posix), "/d/foo/bar");
        assert_eq!(convert(r"C:\", PathForm::Posix), "/c");
        assert_eq!(convert("C:/foo/bar", PathForm::Posix), "/c/foo/bar");
    }

    #[test]
    fn convert_win32_to_mixed() {
        assert_eq!(convert(r"C:\foo\bar", PathForm::Mixed), "C:/foo/bar");
        assert_eq!(convert(r"d:\foo\bar", PathForm::Mixed), "D:/foo/bar");
    }

    #[test]
    fn convert_win32_to_win32_normalizes_drive_case() {
        assert_eq!(convert(r"c:\foo\bar", PathForm::Win32), "C:\\foo\\bar");
        // Mixed-form (`c:/foo`) → fully native.
        assert_eq!(convert("c:/foo/bar", PathForm::Win32), "C:\\foo\\bar");
    }

    #[test]
    fn convert_relative_paths_only_swaps_separators() {
        assert_eq!(convert("foo/bar", PathForm::Win32), "foo\\bar");
        assert_eq!(convert(r"foo\bar", PathForm::Posix), "foo/bar");
        assert_eq!(convert("./foo/bar", PathForm::Win32), ".\\foo\\bar");
        assert_eq!(convert("../foo/bar", PathForm::Win32), "..\\foo\\bar");
        // No separator at all → unchanged borrowed.
        let out = convert("foo", PathForm::Win32);
        assert_eq!(out, "foo");
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn convert_collapses_multi_separator_after_drive() {
        // Stdlib treats `C:\\\\foo\bar` as `C:\foo\bar` post-normalization
        // because the regex `[\\\/]+` consumes all separators after the
        // colon.
        assert_eq!(convert(r"C:\\foo\bar", PathForm::Posix), "/c/foo/bar");
        assert_eq!(convert(r"C:\\foo\bar", PathForm::Mixed), "C:/foo/bar");
    }

    #[test]
    fn convert_empty_input() {
        let out = convert("", PathForm::Win32);
        assert_eq!(out, "");
        assert!(matches!(out, Cow::Borrowed(_)));
    }

    #[test]
    fn convert_does_not_treat_drive_relative_as_root() {
        // `C:foo` (no separator) is a drive-relative path in DOS — leave
        // it alone, just normalize separators.
        assert_eq!(convert("C:foo", PathForm::Win32), "C:foo");
        assert_eq!(convert("C:foo/bar", PathForm::Win32), "C:foo\\bar");
    }

    // ---- convert_path_list ----

    #[test]
    fn convert_path_list_posix_to_win32() {
        assert_eq!(
            convert_path_list("/c/a:/d/b", PathForm::Win32),
            "C:\\a;D:\\b"
        );
    }

    #[test]
    fn convert_path_list_win32_to_posix() {
        assert_eq!(
            convert_path_list(r"C:\a;D:\b", PathForm::Posix),
            "/c/a:/d/b"
        );
    }

    #[test]
    fn convert_path_list_preserves_empty_components() {
        // Trailing/leading empty entries (common with PATH ending in `:`)
        // are preserved.
        let out = convert_path_list("/c/a:/d/b:", PathForm::Win32);
        assert_eq!(out, "C:\\a;D:\\b;");
    }

    #[test]
    fn convert_path_list_single_component_fast_path() {
        assert_eq!(convert_path_list("/c/foo", PathForm::Win32), "C:\\foo");
    }

    // ---- looks_like_path ----

    #[test]
    fn looks_like_path_accepts_drive_rooted() {
        assert!(looks_like_path("/c/foo"));
        assert!(looks_like_path("/c"));
        assert!(looks_like_path("/cygdrive/c/foo"));
        assert!(looks_like_path(r"C:\foo"));
        assert!(looks_like_path("c:/foo"));
        assert!(looks_like_path(r"C:\"));
    }

    #[test]
    fn looks_like_path_rejects_relative_and_non_path() {
        // Relative paths return false — they don't *need* translation.
        assert!(!looks_like_path("foo/bar"));
        assert!(!looks_like_path("./foo"));
        // Plain words.
        assert!(!looks_like_path("hello"));
        // Non-drive POSIX paths.
        assert!(!looks_like_path("/tmp/foo"));
        assert!(!looks_like_path("/dev/null"));
    }

    #[test]
    fn looks_like_path_rejects_flags_and_urls() {
        assert!(!looks_like_path(""));
        assert!(!looks_like_path("-x"));
        assert!(!looks_like_path("--data=foo"));
        assert!(!looks_like_path("https://example.com/c/foo"));
        assert!(!looks_like_path("file:///c/foo"));
    }

    #[test]
    fn looks_like_path_rejects_env_assignment() {
        assert!(!looks_like_path("FOO=bar"));
        assert!(!looks_like_path("KEY=value"));
        // But assignment whose value happens to *contain* a path is
        // not what this function decides — that's caller's policy.
        // We do exclude `KEY=/c/foo` because the `=` precedes any
        // separator, signaling an env-assignment shape.
        assert!(!looks_like_path("KEY=/c/foo"));
    }

    #[test]
    fn strip_cygdrive_only_when_drive_letter_follows() {
        assert_eq!(strip_cygdrive_prefix("/cygdrive/c/foo"), "/c/foo");
        assert_eq!(strip_cygdrive_prefix("/cygdrive/c"), "/c");
        // Not cygdrive form — leave alone.
        assert_eq!(strip_cygdrive_prefix("/cygdrivelong"), "/cygdrivelong");
        assert_eq!(strip_cygdrive_prefix("/cygdrive"), "/cygdrive");
        assert_eq!(strip_cygdrive_prefix("/cygdrive/"), "/cygdrive/");
        assert_eq!(
            strip_cygdrive_prefix("/cygdrive/cd/foo"),
            "/cygdrive/cd/foo"
        );
    }
}

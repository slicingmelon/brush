//! Cross-platform facade over the MSYS-style ↔ native path translator.
//!
//! On Windows the implementation lives in [`crate::sys::windows::path_conv`].
//! On other platforms, the API is provided as no-op passthroughs so that
//! callers (e.g. the `cygpath` builtin) can be written once and compile
//! everywhere — they just won't change anything off Windows.

#[cfg(windows)]
pub use super::windows::path_conv::*;

#[cfg(not(windows))]
mod non_windows {
    use std::borrow::Cow;
    use std::path::{Path, PathBuf};

    /// Output form for [`convert`].
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PathForm {
        /// Native Windows form (no-op on non-Windows).
        Win32,
        /// Mixed form (Windows drive letter + forward slashes).
        Mixed,
        /// POSIX form.
        Posix,
    }

    /// Returns the input unchanged. Path conversion is a no-op on
    /// non-Windows platforms.
    pub fn convert(s: &str, _target: PathForm) -> Cow<'_, str> {
        Cow::Borrowed(s)
    }

    /// Returns the input unchanged. Path-list conversion is a no-op on
    /// non-Windows platforms.
    pub fn convert_path_list(s: &str, _target: PathForm) -> Cow<'_, str> {
        Cow::Borrowed(s)
    }

    /// Always returns false on non-Windows — there is no MSYS form to
    /// translate to/from.
    pub const fn looks_like_path(_s: &str) -> bool {
        false
    }

    /// Always returns `None` on non-Windows.
    pub const fn try_translate_msys_path(_path: &Path) -> Option<PathBuf> {
        None
    }
}

#[cfg(not(windows))]
pub use non_windows::*;

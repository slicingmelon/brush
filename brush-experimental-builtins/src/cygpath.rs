//! `cygpath` — convert paths between POSIX (MSYS/Cygwin) and Windows
//! forms. Mirrors the surface of Cygwin's `cygpath(1)` for the flag
//! subset that brush actually needs.
//!
//! On non-Windows platforms, conversion is a no-op (paths pass through
//! unchanged). This keeps scripts portable: `path=$(cygpath -w "$x")`
//! works on both Windows and Linux, just without any transformation on
//! Linux.
//!
//! See [`docs/planning/path-conversion-msys.md`](../../docs/planning/path-conversion-msys.md)
//! Cycle 1 — this is the explicit, no-heuristic surface that lets
//! agents and scripts request conversion without depending on the
//! later auto-translation layers.

use brush_core::sys::path_conv::{self, PathForm};
use brush_core::{ExecutionResult, builtins};
use clap::Parser;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;

/// (*EXPERIMENTAL, fork-only*) Convert paths between POSIX (MSYS/Cygwin)
/// and Windows forms.
///
/// One mode flag may be supplied; without one, POSIX is the default.
/// Multiple paths can be supplied as positional arguments — each is
/// printed on its own line on stdout in the requested form.
#[derive(Parser)]
#[command(
    no_binary_name = false,
    after_help = "On non-Windows platforms, paths are printed unchanged."
)]
pub(crate) struct CygpathCommand {
    /// Output POSIX form (e.g. `/c/foo`). Default if no mode is given.
    #[arg(short = 'u', long = "unix", conflicts_with_all = ["windows", "mixed"])]
    unix: bool,

    /// Output Windows form (e.g. `C:\foo`).
    #[arg(short = 'w', long = "windows", conflicts_with_all = ["unix", "mixed"])]
    windows: bool,

    /// Output mixed form: Windows drive letter + forward slashes (e.g. `C:/foo`).
    #[arg(short = 'm', long = "mixed", conflicts_with_all = ["unix", "windows"])]
    mixed: bool,

    /// Force the result to be absolute. Relative inputs are joined to
    /// the shell's current working directory before conversion.
    #[arg(short = 'a', long = "absolute")]
    absolute: bool,

    /// Treat each input as a `:`/`;`-separated path list. Each
    /// component is converted; the output uses `;` for Windows / mixed
    /// targets and `:` for POSIX.
    #[arg(short = 'p', long = "path")]
    path_list: bool,

    /// Path(s) to convert.
    #[arg(required = true)]
    paths: Vec<String>,
}

impl builtins::Command for CygpathCommand {
    type Error = brush_core::Error;

    async fn execute<SE: brush_core::ShellExtensions>(
        &self,
        context: brush_core::ExecutionContext<'_, SE>,
    ) -> Result<ExecutionResult, Self::Error> {
        let target = if self.windows {
            PathForm::Win32
        } else if self.mixed {
            PathForm::Mixed
        } else {
            // -u or no mode flag.
            PathForm::Posix
        };

        let mut stdout = context.stdout();
        for input in &self.paths {
            let prepared = if self.absolute {
                make_absolute(input, context.shell.working_dir())
            } else {
                Cow::Borrowed(input.as_str())
            };

            let converted = if self.path_list {
                path_conv::convert_path_list(&prepared, target)
            } else {
                path_conv::convert(&prepared, target)
            };

            writeln!(stdout, "{converted}")?;
        }

        Ok(ExecutionResult::success())
    }
}

/// If `input` is already absolute (drive-rooted on Windows or
/// platform-absolute), return it unchanged. Otherwise join it onto
/// `cwd` and return the result as a string. The resulting form is
/// whatever cwd is in (typically native Windows on Windows, POSIX on
/// Unix); the caller is expected to feed the result through
/// [`path_conv::convert`] so that the final output matches the
/// requested target form.
fn make_absolute<'a>(input: &'a str, cwd: &Path) -> Cow<'a, str> {
    // Drive-rooted POSIX or Windows form already counts as absolute for
    // our purposes.
    if path_conv::looks_like_path(input) {
        return Cow::Borrowed(input);
    }
    // Native absolute as Path::is_absolute sees it (e.g. on Unix,
    // `/foo/bar`; on Windows, `C:\...` already caught above).
    if Path::new(input).is_absolute() {
        return Cow::Borrowed(input);
    }
    Cow::Owned(cwd.join(input).to_string_lossy().into_owned())
}

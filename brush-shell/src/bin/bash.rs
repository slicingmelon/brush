//! Fork: `bash`-named alias binary for `brush`.
//!
//! Identical entry point to `brush`; the binary detects its invocation name
//! at runtime (see `brush_shell::productinfo::invoked_name`) so banners and
//! `$0` reflect the alias. Lets `brush` be installed as a drop-in Git-Bash
//! replacement (e.g. via Claude Code's `CLAUDE_CODE_GIT_BASH_PATH`) without
//! a manual rename step.

fn main() {
    brush_shell::entry::run();
}

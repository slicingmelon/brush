# Fork Changelog

Changes specific to this fork of [reubeno/brush](https://github.com/reubeno/brush).
Upstream changes are tracked in [`CHANGELOG.md`](./CHANGELOG.md).

## Unreleased

### 🐛 Bug Fixes

#### `fix(windows): translate MSYS / Git-Bash style paths in `absolute_path``

When brush is used as the shell behind tools that hand it MSYS / Git-Bash
style paths (Claude Code, MSYS2, Cygwin, Git Bash itself), absolute paths
like `/c/Users/foo` were being treated as relative because Windows'
`Path::is_absolute()` requires a drive letter. They were then joined with
the cwd, and Windows' drive-rooted-join semantics produced mojibake like
`C:/c/Users/foo`.

Concretely: Claude Code's Bash tool wraps every command with a
`pwd > /c/Users/<user>/AppData/Local/Temp/claude-XXXX-cwd` redirect to
track the cwd. Every command emitted:

```
error: failed to redirect to C:/c/Users/<user>/AppData/Local/Temp/claude-XXXX-cwd: \
       The system cannot find the path specified. (os error 3)
```

Fix: added a Windows-only `try_translate_msys_path` helper in
`brush-core/src/sys/windows/fs.rs` (no-op stubs on other platforms) and
wired it into `Shell::absolute_path` so the translation happens once at
the source. Every redirect, file open, and `cd` benefits.

Recognized input forms (case-insensitive drive letter; `/` and `\`
accepted on input):

| Input                       | Translated                |
|-----------------------------|---------------------------|
| `/c`                        | `C:\`                     |
| `/c/`                       | `C:\`                     |
| `/c/Users/foo`              | `C:\Users\foo`            |
| `/cygdrive/c/Users/foo`     | `C:\Users\foo`            |

Non-drive leading-slash paths (`/dev/null`, `/tmp/foo`, `/usr/bin/bash`,
`/cd`) are intentionally **not** translated — those aren't drive references
and the `/dev/null` case is handled by `try_open_special_file`.

Native Windows paths (`C:\…`, `C:/…`) and relative paths are left alone.

Also adjacent fix in `try_open_special_file` on Windows: bare `/dev/null`
was being rejected because `Path::is_absolute()` returns false for it on
Windows, so redirections like `> /dev/null` from a raw MSYS path could fall
through. The check now also accepts MSYS-rooted (leading `/` or `\`) paths.

**Files changed**

- `brush-core/src/shell/fs.rs` — call translator first in `absolute_path`
- `brush-core/src/sys/windows/fs.rs` — `try_translate_msys_path` + `/dev/null` fix + tests
- `brush-core/src/sys/unix/fs.rs` — no-op stub
- `brush-core/src/sys/stubs/fs.rs` — no-op stub (also covers wasm via re-export)

**Tests added** (in `brush-core/src/sys/windows/fs.rs`)

- `translate_msys_drive_root`
- `translate_msys_typical_paths`
- `translate_msys_cygdrive_form`
- `translate_msys_rejects_non_drive_paths`
- `translate_msys_rejects_native_and_relative`
- `try_open_special_file_accepts_raw_dev_null`

### 🛠️ CLI argument parsing

#### `fix(cli): bash semantics for `-c` when followed by another flag`

Bash's `-c` consumes the *first non-option argument* as the command string,
not necessarily the next argv element. So `bash -c -l 'echo hi'` is valid:
`-l` is a flag, `'echo hi'` is the command. Claude Code's Bash tool uses
exactly this form.

Clap's short-option parsing requires the value to be adjacent to the flag,
so the fix rewrites argv before parsing: locate the pending `-c` group and,
if the next token is another option, pull the first subsequent non-option
argument into the slot right after `-c`.

Examples (rewritten before clap parsing):

| Input                          | Rewritten as                   |
|--------------------------------|--------------------------------|
| `-c -l 'echo hi'`              | `-c 'echo hi' -l`              |
| `-c -l 'echo' a b`             | `-c 'echo' -l a b`             |
| `-ec -l 'echo'`                | `-ec 'echo' -l`                |
| `-c 'echo' …`                  | unchanged (already adjacent)   |
| `-c -l --foo`                  | unchanged (no non-option)      |
| `-c -- echo`                   | unchanged (handled by `--`)    |

**Files changed**

- `brush-shell/src/entry.rs` — `pull_c_value_adjacent` + tests

## Installing the fork over the upstream binary

If you use brush as your Git Bash replacement (e.g. via Claude Code's
`CLAUDE_CODE_GIT_BASH_PATH` env var pointing at `~/.cargo/bin/bash.exe`),
install the fork build over the existing binary:

```sh
cargo install --path brush-shell --bin brush --force
# then rename/copy ~/.cargo/bin/brush.exe → ~/.cargo/bin/bash.exe
```

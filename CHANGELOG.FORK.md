# Fork Changelog

Changes specific to this fork of [reubeno/brush](https://github.com/reubeno/brush).
Upstream changes are tracked in [`CHANGELOG.md`](./CHANGELOG.md).

## Unreleased

### ✨ Features

#### `feat(coreutils): add 26 missing uutils/coreutils utilities + `[` alias`

This is Cycle 1 of [`docs/planning/coreutils-coverage-expansion.md`](./docs/planning/coreutils-coverage-expansion.md).
The goal of that plan is closing the gap between the 80 uutils utilities
the fork shipped at MVP and the ~107 utilities `uutils/coreutils 0.8.0`
actually provides — every command we missed is a `command not found` for
shell scripts the fork was meant to host.

After Phase 0 reconciled the to-add list against the canonical
[`uutils/coreutils@0.8.0/Cargo.toml`](https://github.com/uutils/coreutils/blob/0.8.0/Cargo.toml),
**26 utilities + 1 alias** are wired into `brush-coreutils-builtins`:

| Group                                  | Utilities                                                                                                                                          | Cargo gate                              |
|----------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------|
| Cross-platform (Tier 1; was missing)   | `pathchk`                                                                                                                                          | `[dependencies]` (unconditional)        |
| Unix-only (`feat_require_unix_core`)   | `chgrp`, `chmod`, `chown`, `chroot`, `groups`, `id`, `install`, `kill`*, `logname`, `mkfifo`, `mknod`, `nice`, `nohup`, `stat`, `stty`, `timeout`, `tty` | `[target.'cfg(unix)'.dependencies]`     |
| Unix-only (`feat_require_unix_utmpx`)  | `pinky`, `uptime`, `users`, `who`                                                                                                                  | `[target.'cfg(unix)'.dependencies]`     |
| Unix-only (`feat_require_unix_hostid`) | `hostid`                                                                                                                                           | `[target.'cfg(unix)'.dependencies]`     |
| Linux-only (LD_PRELOAD or libselinux)  | `stdbuf`, `chcon`, `runcon`                                                                                                                        | `[target.'cfg(target_os = "linux")'.dependencies]` |
| Alias                                  | `[`  (calls `uu_test::uumain`)                                                                                                                     | enabled by `coreutils.test`             |

\* `kill` collides with brush's native `kill` builtin.
[`brush-shell/src/bundled.rs::register_shims`](./brush-shell/src/bundled.rs)
uses `register_builtin_if_unset`, so the native version wins for the
shell-builtin lookup path. The bundled-dispatch fast path
(`brush --invoke-bundled kill ...`) routes to uutils, which is the
expected behavior for direct invocation.

**New aggregate features in `brush-coreutils-builtins/Cargo.toml`**:

- `coreutils.all` (existing) — every utility that builds on every Tier-1
  target. Now includes `pathchk` (the only Tier-1 cross-platform utility
  the fork was missing).
- `coreutils.all-unix` (new) — `coreutils.all` + the Unix-only set above.
  Builds only on Unix targets (the dep crates live in a `cfg(unix)`
  target table). Enabling on Windows triggers a Cargo dep-resolution
  error — by design.
- `coreutils.all-linux` (new) — `coreutils.all-unix` + the Linux-only set.
  Builds only on Linux.

**New feature flags on `brush-shell/Cargo.toml`** (opt-in, layered):

- `experimental-bundled-coreutils` (existing) — enables
  `coreutils.all`. Behavior unchanged for Windows users; on Unix this
  now also includes `pathchk` and the `[` alias.
- `experimental-bundled-coreutils-unix-extras` (new) — adds the
  Unix-only set on top.
- `experimental-bundled-coreutils-linux-extras` (new) — adds the
  Linux-only set on top of `-unix-extras`.

**Installing the full Unix set** (replaces the bare-experimental install):

```sh
# Linux:
cargo install --locked --path brush-shell --force \
  --features experimental-builtins,experimental-bundled-coreutils-linux-extras

# macOS / other Unix:
cargo install --locked --path brush-shell --force \
  --features experimental-builtins,experimental-bundled-coreutils-unix-extras

# Windows: the bare experimental flag is still the right choice — uutils'
# Unix-only crates (id, chmod, ...) are gated to cfg(unix) by upstream,
# and there is no Windows port of these utilities to bundle.
cargo install --locked --path brush-shell --force \
  --features experimental-builtins,experimental-bundled-coreutils
```

**Why some utilities are still missing on Windows**: uutils itself ships
`id`, `stat`, `chmod`, etc. only for Unix-class targets — they use POSIX
APIs (`getuid()`, mode bits, signal numbers) that Windows lacks
equivalents for. The fork follows uutils' gating: the Cargo deps for
these utilities live in `[target.'cfg(unix)'.dependencies]`, so on
Windows the dep is not in the graph and no compile is attempted.
Windows users wanting `id` etc. should fall through to `PATH` — the
Git Bash MSYS2 binaries are still resolvable.

**What was removed from the original to-add candidate set during Phase 0**:

- `hashsum` — not a separate `uu_*` dependency in 0.8.0; uutils ships
  it only as a multi-call binary alias from the `coreutils` driver, with
  no `uu_hashsum` crate to register here.

**Binary-size impact** (dev unstripped on Windows, with
`experimental-bundled-coreutils`): 42.8 MB (with `pathchk` + `[` alias).
Cycle 1's incremental cost is bounded — Windows gains one new utility
(`pathchk`), Unix gains 26 + the alias. Release-mode size measurement
deferred — not a regression on Windows.

**Files changed**

- `brush-coreutils-builtins/Cargo.toml` — 26 new `coreutils.<name>`
  features; new `coreutils.all-unix` and `coreutils.all-linux`
  aggregates; new `[target.'cfg(unix)'.dependencies]` and
  `[target.'cfg(target_os = "linux")'.dependencies]` blocks
- `brush-coreutils-builtins/src/lib.rs` — `register!` lines for all 26
  utilities (target-cfg-gated as appropriate) + `[` alias
- `brush-shell/Cargo.toml` — two new opt-in feature flags
  (`-unix-extras`, `-linux-extras`)

**Smoke-tested** on Windows (`cargo build -p brush-shell --features
experimental-bundled-coreutils`, exit 0):

```text
$ ./target/debug/brush.exe -c 'type pathchk; type "["; pathchk hello.txt && echo ok=$?; [ 1 = 1 ] && echo ok'
pathchk is a shell builtin
[ is a shell builtin
ok=0
ok
```

**Deferred to follow-up** (tracked in
[`docs/planning/coreutils-coverage-expansion.md`](./docs/planning/coreutils-coverage-expansion.md)):

- Phase 1.5 — YAML smoke tests for the new utilities. The harness
  builds brush via `cargo_bin!("brush")` without specifying features, so
  bundled-coreutils tests would need test-harness feature-flag plumbing
  (a separate concern).
- Phase 1.6 — `xtask coverage-check` to detect future drift between our
  registry and upstream `uutils/coreutils` releases.

#### `feat(bundled): plumb pipeline pgid through ExecutionContext`

Adds `process_group_id: Option<i32>` to `brush_core::commands::ExecutionContext`
and threads it through every construction site (`execute_via_builtin_in_owned_shell`,
`execute_via_builtin_in_parent_shell`, `execute_via_function`, `execute_via_external`,
plus `Shell::invoke_function`). Lets the bundled-coreutils shim
([`brush-shell/src/bundled.rs`](./brush-shell/src/bundled.rs)) read the enclosing
pipeline's pgid back and apply it to the child `SimpleCommand` it spawns.

**Effective only where `process_group` is more than a stub** — Linux/macOS via `nix`.
Windows is a silent no-op until the job-control epic lands
(see [`docs/planning/bundled-coreutils-pipelines.md`](./docs/planning/bundled-coreutils-pipelines.md)
Cycle 3).

This is Cycle 1 of the bundled-coreutils-pipelines plan. **By itself it does not
change observable behavior** — the shim still returns `Completed`, so the pipeline
spawn loop never harvests the bundled leader's pgid to apply to the rest of the
pipeline. That is resolved in Cycle 2 (pipeline parallelism), which converts the
shim's return shape from a finished result into a spawn handle.

**Public API change**: `ExecutionContext` is `pub` and not `#[non_exhaustive]`, so
the new field is technically observable to downstream brush-core consumers
constructing `ExecutionContext` literals. In practice the struct is only meant
for builtin authors, and the field defaults to `None` for all builtins that don't
re-exec. Flagged here for SemVer transparency.

**Files changed**

- `brush-core/src/commands.rs` — new field + 4 plumbing sites
- `brush-core/src/shell/funcs.rs` — set `None` on the function-invocation context
- `brush-shell/src/bundled.rs` — read context pgid onto child `SimpleCommand`

### 🐛 Bug Fixes

#### `fix(windows): suppress console-window flash on child process spawn`

When brush is launched by a host that has no attached console (Claude Code's
Bash tool, editor terminals, automation harnesses), every child process it
spawns triggered a brief dark console window: Windows allocates a fresh
console for a console-subsystem child whose parent has none. The flash was
especially visible with bundled coreutils enabled, since each `cat`, `ls`,
`tr`, etc. re-invokes brush as a shim child — every line of output, multiple
flashes.

Fix: pass `CREATE_NO_WINDOW` (`0x0800_0000`) via `creation_flags` on the
single Windows spawn point in `brush-core/src/sys/tokio_process.rs`. stdio
handles still inherit through `STARTUPINFO`, so pipelines, redirections, and
captured output are unaffected — only the console *window* allocation is
suppressed. The flag is gated on `cfg(windows)`; Unix is untouched.

A future enhancement may detect interactive child invocations (e.g. `vim`,
`less`) and skip the flag for those, but for non-interactive shell-tool
usage — which is what brush-as-Git-Bash-replacement is mainly for —
unconditional suppression is the right default.

**Files changed**

- `brush-core/src/sys/tokio_process.rs` — apply `CREATE_NO_WINDOW` on Windows

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

### 🛠️ Build / install ergonomics

#### `feat(install): produce a `bash` binary alongside `brush``

The fork now ships a second binary named `bash` (`bash.exe` on Windows) built
from the same source as `brush`. This eliminates the manual
`cp brush.exe → bash.exe` step that was previously required when using brush
as a Git Bash replacement (e.g. via Claude Code's `CLAUDE_CODE_GIT_BASH_PATH`
env var, or anywhere a tool spawns `bash` by name).

`cargo install --git https://github.com/slicingmelon/brush brush-shell` now
deposits **both** `brush` and `bash` into `~/.cargo/bin/`.

The `bash` binary is byte-identical-in-behavior to `brush` — it simply
detects its invocation name at runtime via `std::env::current_exe()` and
adjusts the product banner accordingly:

| Invocation       | `--version` output                           |
|------------------|----------------------------------------------|
| `brush --version`| `brush version 0.3.0 (...) - https://...`    |
| `bash --version` | `bash (brush) version 0.3.0 (...) - https://...` |

The `(brush)` suffix on aliased invocations keeps the underlying
implementation discoverable to users debugging "why is `bash --version` not
GNU bash?".

**Files changed**

- `brush-shell/Cargo.toml` — declare two `[[bin]]` targets (`brush`, `bash`)
  pointing at `src/bin/brush.rs` and `src/bin/bash.rs`
- `brush-shell/src/bin/brush.rs` — entry shim (replaces former `src/main.rs`)
- `brush-shell/src/bin/bash.rs` — alias entry shim
- `brush-shell/src/productinfo.rs` — `invoked_name()` + `display_name()`
  helpers; `get_product_display_str()` now uses them
- `brush-shell/src/args.rs` — switch `--version` from
  `clap::ArgAction::Version` to `SetTrue` so we can format the banner
  ourselves at runtime
- `brush-shell/src/entry.rs` — handle `--version` manually after parse,
  printing `productinfo::get_product_display_str()` and exiting

## Installing the fork over the upstream binary

If you use brush as your Git Bash replacement (e.g. via Claude Code's
`CLAUDE_CODE_GIT_BASH_PATH` env var pointing at `~/.cargo/bin/bash.exe`),
just install the fork — both `brush.exe` and `bash.exe` are deposited into
`~/.cargo/bin/` automatically:

```sh
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force
```

Or from a local clone:

```sh
cargo install --locked --path brush-shell --force
```

No rename step required.

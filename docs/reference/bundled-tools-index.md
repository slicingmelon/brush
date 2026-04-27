# Bundled-Tools Index

> Inventory of every command this **fork** of brush registers when built
> with the recommended Windows install line, plus the gap analysis
> against Git-for-Windows. Last revision: 2026-04-26.
>
> **Recommended Windows install** (from [`README.md`](../../README.md)):
>
> ```bash
> cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force \
>   --features experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras
> ```
>
> Requires **rustc ≥ 1.92** because the umbrella `experimental-bundled-extras`
> pulls in `awnion/fastgrep`, whose MSRV is 1.92. The brush workspace
> itself stays at MSRV 1.88.0 — only this one feature flag carries the
> bumped requirement. Fall-back install lines (no `grep`) are documented
> in the README.

## How resolution works (short version)

When you type a command at a brush prompt, brush walks this order:

1. **Native shell builtins** — registered from
   [`brush-builtins`](../../brush-builtins/src/lib.rs) (`echo`, `cd`,
   `printf`, `pwd`, `test`, `[`, `true`, `false`, …).
2. **Bundled commands (uutils + extras)** — registered as shim builtins
   that dispatch to the same `brush.exe` re-invoked with
   `--invoke-bundled`. See
   [`brush-shell/src/bundled.rs:316`](../../brush-shell/src/bundled.rs:316).
   On name collisions with step 1, **brush's native builtin wins**
   (`register_builtin_if_unset`) — so `echo`, `printf`, `pwd`, `test`,
   `true`, `false`, `kill` stay native.
3. **PATH external** — fall through to whatever Windows can find on
   `PATH`. In the user's environment that's mostly Windows-native tools
   (`git`, `curl`, `ssh`, `gpg`) plus anything in `~/.cargo/bin`.

That's why `command -v sed` shows `sed` (no path — bundled shim) but
`command -v git` shows `C:\Software\GitForWindows\cmd\git`. Brush
**does not** reach into `C:\Software\GitForWindows\usr\bin` unless the
user explicitly adds it to `PATH` — that directory is the MSYS world
bash sees, not what brush sees.

## Section A — Native shell builtins (`brush-builtins`)

Always present (no feature flag needed under the workspace defaults).
Source: [`brush-builtins/src/lib.rs`](../../brush-builtins/src/lib.rs)
+ siblings.

| Builtin | Platform | Notes |
|---------|----------|-------|
| `alias` | all | |
| `bg` | all | job control |
| `bind` | all | reedline keybindings |
| `break` | all | |
| `builtin` | all | force-builtin dispatch |
| `caller` | all | |
| `cd` | all | |
| `:` (colon) | all | |
| `command` | all | |
| `complete` / `compgen` / `compopt` | all | programmable completion |
| `continue` | all | |
| `declare` | all | (also registered as `local`, `typeset`, `readonly` — declaration variants) |
| `dirs` | all | |
| `.` (dot) / `source` | all | |
| `echo` | all | wins over uutils' `uu_echo` |
| `enable` | all | enable/disable builtins |
| `eval` | all | |
| `exec` | **Unix only** | gated `cfg(unix)` in `lib.rs:41` |
| `exit` | all | |
| `export` | all | |
| `false` | all | wins over uutils' `uu_false` |
| `fc` | all | |
| `fg` | all | job control |
| `getopts` | all | |
| `hash` | all | |
| `help` | all | |
| `history` | all | |
| `jobs` | all | |
| `kill` | **Unix only** | brush-builtins gates `cfg(unix)`; uutils' `uu_kill` is also Unix-only — Windows brush has **no** `kill` builtin and falls through to PATH (none in user's PATH) |
| `let` | all | |
| `mapfile` / `readarray` | all | |
| `popd` | all | |
| `printf` | all | gated `any(unix, windows)`; wins over uutils' `uu_printf` |
| `pushd` | all | |
| `pwd` | all | wins over uutils' `uu_pwd` |
| `read` | all | |
| `return` | all | |
| `set` | all | |
| `shift` | all | |
| `shopt` | all | |
| `suspend` | **Unix only** | |
| `test` / `[` | all | wins over uutils' `uu_test` |
| `times` | all | |
| `trap` | all | |
| `true` | all | wins over uutils' `uu_true` |
| `type` | all | (covers Git-Bash's `which` for most uses) |
| `ulimit` | **Unix only** | |
| `umask` | **Unix only** | |
| `unalias` | all | |
| `unset` | all | |
| `wait` | all | job control |

Disabled-on-Windows count: 5 (`exec`, `kill`, `suspend`, `ulimit`, `umask`).

## Section B — Experimental builtins (`experimental-builtins` flag)

Source: [`brush-experimental-builtins/src/lib.rs`](../../brush-experimental-builtins/src/lib.rs).

| Builtin | Feature | Notes |
|---------|---------|-------|
| `save` | `builtin.save` | brush-only extension (not in bash) |

## Section C — uutils/coreutils 0.8.0 (`experimental-bundled-coreutils`)

Source:
[`brush-coreutils-builtins/Cargo.toml`](../../brush-coreutils-builtins/Cargo.toml).
On Windows the `experimental-bundled-coreutils` flag enables only the
cross-platform set (the `coreutils.all` aggregate). The `-unix-extras`
and `-linux-extras` aggregates are gated to Unix and Linux respectively
and would fail to build on Windows.

### Cross-platform set — bundled on Windows

> **Reading note**: `*(masked)*` / `*(masked by native)*` next to a name
> means the uutils crate is compiled into the binary but brush's own
> native builtin (from `brush-builtins`) wins on dispatch — see
> [§Section A](#section-a--native-shell-builtins-brush-builtins) and the
> resolution-order explanation at the top of this doc. Reach the masked
> uutils version with `command -p <name>` or by disabling the native
> builtin via `enable -n <name>`.

`arch`, `b2sum`, `base32`, `base64`, `basename`, `basenc`, `cat`,
`cksum`, `comm`, `cp`, `csplit`, `cut`, `date`, `dd`, `df`, `dir`,
`dircolors`, `dirname`, `du`, `echo` *(masked by native)*, `env`,
`expand`, `expr`, `factor`, `false` *(masked)*, `fmt`, `fold`, `head`,
`hostname`, `join`, `link`, `ln`, `ls`, `md5sum`, `mkdir`, `mktemp`,
`more`, `mv`, `nl`, `nproc`, `numfmt`, `od`, `paste`, `pathchk`, `pr`,
`printenv`, `printf` *(masked)*, `ptx`, `pwd` *(masked)*, `readlink`,
`realpath`, `rm`, `rmdir`, `seq`, `sha1sum`, `sha224sum`, `sha256sum`,
`sha384sum`, `sha512sum`, `shred`, `shuf`, `sleep`, `sort`, `split`,
`sum`, `sync`, `tac`, `tail`, `tee`, `test` *(masked)*, `touch`, `tr`,
`true` *(masked)*, `truncate`, `tsort`, `uname`, `unexpand`, `uniq`,
`unlink`, `vdir`, `wc`, `whoami`, `yes`.

**83 utilities** when counted by per-utility `coreutils.<name>` feature
keys in the `coreutils.all` aggregate
([`brush-coreutils-builtins/Cargo.toml:37-121`](../../brush-coreutils-builtins/Cargo.toml)).
Six are shadowed by native brush builtins on Windows: `echo`, `false`,
`printf`, `pwd`, `test`, `true`. (On Unix `kill` makes seven — the
native `kill` builtin is `cfg(unix)` so on Windows there's nothing to
shadow `uu_kill`, but `uu_kill` is itself in `coreutils.all-unix`, so
the collision only exists on Unix builds.) The shadowed crates still
compile and ship in the binary — reach them via `command -p` or by
disabling the native builtin with `enable -n`.

### Unix-only extras (`experimental-bundled-coreutils-unix-extras`)

Not enabled on Windows builds. Listed for completeness — these are the
deltas if you build on Linux/macOS:

`chgrp`, `chmod`, `chown`, `chroot`, `groups`, `hostid`, `id`,
`install`, `kill`, `logname`, `mkfifo`, `mknod`, `nice`, `nohup`,
`pinky`, `stat`, `stty`, `timeout`, `tty`, `uptime`, `users`, `who`.

### Linux-only extras (`experimental-bundled-coreutils-linux-extras`)

Adds: `chcon`, `runcon`, `stdbuf` (the last via LD_PRELOAD cdylib).

## Section D — Bundled extras (`experimental-bundled-extras`)

Source:
[`brush-bundled-extras/Cargo.toml`](../../brush-bundled-extras/Cargo.toml)
+ [`src/lib.rs`](../../brush-bundled-extras/src/lib.rs).

| Command | Upstream | Version | Per-flag opt-in | MSRV |
|---------|----------|---------|-----------------|------|
| `find` | `uutils/findutils` | 0.8.0 | `experimental-bundled-extras-findutils` | 1.88 |
| `xargs` | `uutils/findutils` | 0.8.0 | `experimental-bundled-extras-findutils` | 1.88 |
| `sed` | `uutils/sed` | 0.1.1 | `experimental-bundled-extras-uutils-sed` | 1.88 |
| `awk` | `pegasusheavy/awk-rs` | 0.1.0 | `experimental-bundled-extras-awk-rs` | 1.85 |
| `grep` | `awnion/fastgrep` | 0.1.8 | `experimental-bundled-extras-fastgrep` | **1.92** |
| `fastgrep` | (alias of `grep`) | — | `experimental-bundled-extras-fastgrep` | **1.92** |
| `egrep` | (alias of `grep` with `-E` pre-pended) | — | `experimental-bundled-extras-fastgrep` | **1.92** |
| `fgrep` | (alias of `grep` with `-F` pre-pended) | — | `experimental-bundled-extras-fastgrep` | **1.92** |
| `which` | crates.io `which` | 6 | `experimental-bundled-extras-utils` | 1.88 |
| `tree` | in-tree (uses `walkdir`) | — | `experimental-bundled-extras-utils` | 1.88 |
| `xxd` | in-tree (no deps) | — | `experimental-bundled-extras-utils` | 1.88 |
| `column` | in-tree (no deps) | — | `experimental-bundled-extras-utils` | 1.88 |
| `file` | crates.io `infer` | 0.16 | `experimental-bundled-extras-utils` | 1.88 |

`grep` / `fastgrep` / `egrep` / `fgrep` all resolve to the same fastgrep
adapter. `grep` and `fastgrep` dispatch raw; `egrep` / `fgrep` insert
`-E` / `-F` immediately after `argv[0]` to mirror GNU `egrep` / `fgrep`
semantics. See `brush-bundled-extras/src/lib.rs` and the
`grep_adapter::{egrep_main, fgrep_main}` wrappers. Per
[`docs/planning/bundled-extras-coverage-expansion.md`](../planning/bundled-extras-coverage-expansion.md)
Cycle 0a, ripgrep is planned to take over `grep` / `egrep` / `fgrep`
in Cycle 3 — `fastgrep` will retain its own name then.

## Section E — What this install does **NOT** include

### From bash itself (POSIX/bash builtins not implemented)

Per [`docs/reference/compatibility.md`](compatibility.md): `coproc`,
`select`, signal traps + the corresponding `trap` extensions, and a
few edge cases. Refer to that doc rather than reproducing it here.

### From Git-for-Windows `usr\bin` — real gaps

Reference listing at the top of this conversation; verified against
ground-truth `dir` output. These are commands that exist in Git-Bash's
`/usr/bin` but **brush does not bundle** *and* are not on the user's
Windows `PATH`. So if you type them at a brush prompt, you get
"command not found". (If your PATH adds `C:\Software\GitForWindows\usr\bin`,
brush will of course resolve them.)

#### Compression / archives — significant gaps

| Tool family | Missing commands |
|---|---|
| tar | `tar` |
| bzip2 | `bzip2`, `bunzip2`, `bzcat`, `bzcmp`, `bzdiff`, `bzegrep`, `bzfgrep`, `bzgrep`, `bzip2recover`, `bzless`, `bzmore` |
| gzip | `gzip`, `gunzip`, `gzexe`, `zcat`, `zcmp`, `zdiff`, `zegrep`, `zfgrep`, `zforce`, `zgrep`, `zless`, `znew` |
| xz | `xz`, `xzcat`, `xzdec`, `unxz`, `lzmadec`, `lzmainfo`, `xzcmp`, `xzdiff`, `xzegrep`, `xzfgrep`, `xzgrep`, `xzless`, `xzmore` |
| zip | `unzip`, `unzipsfx`, `funzip`, `zipgrep`, `zipinfo` |

`tar` is the loudest absence — common enough in scripts to be worth a
bundling decision. uutils does not ship one; `cargo install ouch` or a
crates.io `tar`-binary search is the obvious next step. *Out of scope
for this branch.*

#### Diff / patch — diffutils deferred

`cmp`, `diff`, `diff3`, `sdiff`, `patch`. `uutils/diffutils` is being
tracked in
[`docs/planning/coreutils-coverage-expansion.md`](../planning/coreutils-coverage-expansion.md)
Cycle 3 (deferred awaiting upstream `pub mod diff;`).

#### Editors — no native equivalent

`vi`, `vim`, `vimdiff`, `view`, `rvim`, `rview`, `ex`, `nano`, `rnano`,
`vimtutor`. brush is non-interactive code; bundling an editor is out of
scope. Users who want one should `winget install` or rely on
`$EDITOR` resolving via Windows PATH.

#### Cygwin/MSYS internals — irrelevant by design

`cygcheck`, `cygpath`, `cygwin-console-helper`, `mintty`, `mintheme`,
`mount`, `umount`, `mkpasswd`, `mkgroup`, `passwd`, `getfacl`,
`setfacl`, `chattr`, `lsattr`, `regtool`, `setmetamode`, `pldd`,
`gencat`, `getconf`, `rebase`, `rebaseall`, `ldd`, `ldh`, `gkill`,
`minidumper`, `gmondump`, `winpty-*`, `msys-*.dll`. These exist
because Git-Bash *is* an MSYS distribution and needs the cygwin
runtime; brush is native Windows and has nothing equivalent (or wants
nothing equivalent). **Not a gap.**

#### Already on Windows PATH — non-issue

Resolved via PATH from the user's Windows env (verified in conversation):
`git` (`C:\Software\GitForWindows\cmd\git`), `curl`, `gpg`, `ssh`,
`scp`, `sftp`, `ssh-keygen`, `ssh-agent`, `openssl`, `perl`,
`python` (Software\Python-3.12.10), `tig` is in usr/bin only — partial.

#### Genuinely useful gaps (no Windows PATH coverage either)

| Command | What it does | Likely action |
|---|---|---|
| `tar` | archive | high-value bundle candidate |
| `bzip2`/`gzip`/`xz`/`zstd` family | compression | medium-value; `flate2`/`bzip2`/`xz2` crates exist |
| ~~`column`~~ | columnar formatter | **CLOSED 2026-04-28** — bundled in-tree (Cycle 1 of `bundled-extras-coverage-expansion.md`) |
| `getopt` | shellopts (different from `getopts`) | low-value |
| `iconv` | text encoding | medium-value; deferred to Cycle 4 |
| ~~`file`~~ | type detection | **CLOSED 2026-04-28** — bundled via `infer` crate (Cycle 1 of `bundled-extras-coverage-expansion.md`) |
| `less` | pager | high-value but interactive |
| ~~`which`~~ | command lookup | **CLOSED 2026-04-28** — bundled via `which` crate (Cycle 1 of `bundled-extras-coverage-expansion.md`) |
| ~~`xxd`~~ | hex dump | **CLOSED 2026-04-28** — bundled in-tree (Cycle 1 of `bundled-extras-coverage-expansion.md`) |
| ~~`tree`~~ | directory listing | **CLOSED 2026-04-28** — bundled in-tree using `walkdir` (Cycle 1 of `bundled-extras-coverage-expansion.md`) |
| `gawk` | gnu awk variant | covered by bundled `awk` |
| ~~`egrep`/`fgrep`~~ | grep aliases | **CLOSED 2026-04-28** — both now registered as bundled aliases of fastgrep with `-E`/`-F` pre-pended (Cycle 0a of `bundled-extras-coverage-expansion.md`) |
| `ps` | process listing | high-value but tricky cross-platform |
| `kill` | signal sender | bundled-extras candidate (uutils' is Unix-only); on Windows the use case is "kill PID" which `taskkill.exe` covers |
| `tee` | already bundled (uutils) ✓ | — |
| `column`/`ts`/`tsort` | various | `tsort` already bundled |
| `mktemp` | already bundled (uutils) ✓ | — |
| `tac` | already bundled (uutils) ✓ | — |

_(The `egrep`/`fgrep` gap was closed in Cycle 0a of
[`docs/planning/bundled-extras-coverage-expansion.md`](../planning/bundled-extras-coverage-expansion.md).)_

### From Git-for-Windows `mingw64\bin` — almost all DLLs and Avalonia GUI

Most entries are `*.dll`, `Avalonia.*` (the GUI for Git Credential
Manager), `Microsoft.*.dll`, or git's helper executables (already
covered above). The only standalone CLI tools worth noting:

| Tool | Status |
|---|---|
| `curl.exe` | already on Windows PATH ✓ |
| `openssl.exe` | already on Windows PATH ✓ |
| `gettext.exe`, `envsubst.exe` | not bundled, not on PATH — minor gap |
| `brotli.exe` | not bundled, not on PATH — minor gap |
| `bunzip2`/`bzip2`/`bzcat` | duplicated in `usr\bin` (covered above) |
| `pdftotext.exe`, `odt2txt.exe`, `antiword.exe` | document conversion — niche, out of scope |
| `psl.exe` (public suffix list), `c_rehash`, `dirmngr*`, `gpg-*` | crypto/dns helpers — out of scope |
| `tclsh.exe`, `wish.exe` | Tcl/Tk runtime for `gitk` — out of scope |

Summary: `mingw64\bin` adds **no** unique CLI gaps over `usr\bin`.
The interesting deltas are bundled there because Git-for-Windows ships
the GUI credential manager from that path.

## Maintenance

When this list goes stale (bumped uutils version, new bundled crate,
removed flag), regenerate by:

1. Running through the three `Cargo.toml`s in
   [`brush-coreutils-builtins`](../../brush-coreutils-builtins/Cargo.toml),
   [`brush-bundled-extras`](../../brush-bundled-extras/Cargo.toml), and
   [`brush-experimental-builtins`](../../brush-experimental-builtins/Cargo.toml).
2. Diffing this file's tables against them.
3. Re-running `dir C:\Software\GitForWindows\usr\bin` and
   `dir C:\Software\GitForWindows\mingw64\bin` if Git-for-Windows has
   been updated.

CI does not enforce this index. It exists for human reference.

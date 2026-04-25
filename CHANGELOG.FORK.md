# Fork Changelog

Changes specific to this fork of [reubeno/brush](https://github.com/reubeno/brush).
Upstream changes are tracked in [`CHANGELOG.md`](./CHANGELOG.md).

# Unreleased

> Per-component version bumps planned for the next release:
>
> | Crate                  | Previous | New     | Why                                                                  |
> |------------------------|----------|---------|----------------------------------------------------------------------|
> | `brush-core`           | 0.4.1    | 0.4.2   | Conditional `CREATE_NO_WINDOW` — fix a v0.3.1 regression where bundled coreutils produced no output when brush ran interactively from a real Windows console. |
> | `brush-bundled-extras` | 0.1.0    | 0.1.2   | Cycle 0a — wire `uutils/sed = "0.1.1"` via `sed_adapter` (`extras.sed` / `extras.uutils-sed-all` features). Cycle 0c-revised — wire `pegasusheavy/awk-rs = "0.1.0"` via `awk_adapter` (`extras.awk` / `extras.awk-rs-all` features). Both per `posixutils-rs-integration.md`. |
> | `brush-shell`          | 0.3.1    | 0.3.3   | New `experimental-bundled-extras-uutils-sed` (Cycle 0a) and `experimental-bundled-extras-awk-rs` (Cycle 0c-revised) feature flags; `bundled.rs` cfg-gate extended to merge the extras registry when only one of them is enabled. |

### 📋 Process / Decisions

#### `docs(planning): record Cycle 0b-revised gate outcome (Windows-build + MSRV both passed)`

Cycle 0b-revised of [`docs/planning/posixutils-rs-integration.md`](./docs/planning/posixutils-rs-integration.md)
prescribes two MANDATORY pre-merge gates before the production grep
adapter can land:

1. **MSRV gate** — fastgrep declares `rust-version = "1.92"` while
   brush is at `1.88.0`. Plan offered four resolution options
   (workspace MSRV bump, feature-conditional MSRV, upstream PR, or
   fall-through to fallback).
2. **Windows-build smoke gate** — fastgrep upstream CI is Ubuntu +
   macOS only; Windows-buildability and Windows-correctness needed
   independent verification before brush could ship a feature flag
   that depends on the crate.

**Outcome — both gates PASSED.** Decision: option (b)
feature-conditional MSRV (workspace stays at 1.88.0; the new flag
documents `rustc ≥ 1.92` as a per-flag requirement). `cargo install
fastgrep --version 0.1.8` built cleanly on Windows 11 in 35s
producing a working `grep.exe`. Runtime smoke checks (`--version`,
`-rn`, stdin pipe, no-match exit code, EPIPE under `| head`, trigram
cache directory creation) all behaved correctly. Cycle 0b-fallback
(vendor from posix-tools-for-windows) is **not triggered** — the
production adapter follows in the next commit on this branch.

Full decision log entry in the planning doc.

### ✨ Features

#### `feat(bundled): ship awk via pegasusheavy/awk-rs crates.io dep`

Cycle 0c-revised of [`docs/planning/posixutils-rs-integration.md`](./docs/planning/posixutils-rs-integration.md).
**Second gap-filler from that plan to land.**

`awk` is now available as a bundled builtin behind a new feature flag.
The implementation is a clean crates.io dep on
[`pegasusheavy/awk-rs`](https://crates.io/crates/awk-rs) v0.1.0 — no
vendoring. Upstream advertises 100% POSIX compatibility (with optional
gawk extensions), 639 tests at 86% library coverage, Criterion
benchmarks, and **CI matrix tests Windows-latest + macOS + Linux**.
Dual MIT / Apache-2.0. Light deps (`regex` + `thiserror`).

Upstream's lib exposes `Lexer` / `Parser` / `Interpreter` directly but
no single `run(args)` entrypoint, so the brush adapter ports the CLI
driver from upstream `src/main.rs::run()` line-for-line. Intent: stay
drop-in equivalent to the standalone `awk-rs` binary, no behavioral
divergence. Supports the standard set of POSIX awk flags:

| Flag | Purpose |
|---|---|
| `-F fs` (and `-Ffs`) | Field separator |
| `-v var=val` | Pre-execution variable assignment |
| `-f progfile` | Read AWK program from file |
| `-P` / `--posix` | Strict POSIX mode (disable gawk extensions) |
| `-c` / `--traditional` / `--compat` | Traditional AWK mode |
| `--` | End of options (rest are input files) |
| `-` (as input file) | Read from stdin |

Wiring matches the find/xargs/sed adapter precedent:

| Layer | What landed |
|---|---|
| `brush-bundled-extras/Cargo.toml` | `awk-rs = { version = "0.1.0", optional = true }`; new features `extras.awk` and `extras.awk-rs-all`; `extras.all` aggregate now layers in `extras.awk-rs-all`. |
| `brush-bundled-extras/src/lib.rs` | New `awk_adapter` + `awk_run` (port of upstream main.rs::run) + `print_awk_help`. `args[0]` carries `"awk"` per brush dispatch convention; adapter slices `&args[1..]` to mirror upstream's `&env::args()[1..]`. |
| `brush-shell/Cargo.toml` | New `experimental-bundled-extras-awk-rs` feature flag pulling `extras.awk-rs-all`. |
| `brush-shell/src/bundled.rs` | `cfg(any(...))` gate around the bundled-extras registry merge extended to include `experimental-bundled-extras-awk-rs`. |

**Smoke verification on Windows** (rustc 1.88.0 host build):

| Command | Output |
|---|---|
| `brush -c "echo 'a b c' \| awk '{print \$2}'"` (DoD) | `b` |
| `brush -c "awk 'BEGIN{for(i=1;i<=10;i++) sum+=i; print sum}'"` (DoD) | `55` |
| `brush -c "type awk"` | `awk is a shell builtin` |
| `brush -c "awk --version"` | `awk-rs 0.1.0` |
| `brush -c "awk '{print FILENAME, NR}' brush-bundled-extras/Cargo.toml"` | rows tagged with `<path> <n>` |
| `brush -c "printf 'a:1\nb:2\nc:3\n' \| awk -F: '{print \$1, \$2 * 10}'"` | `a 10` / `b 20` / `c 30` |
| `brush -c "awk -v x=5 'BEGIN{print x+10}'"` | `15` |
| `brush -c "printf 'red\nblue\ngreen\n' \| awk 'length(\$0) > 3'"` | `blue` / `green` |
| `brush -c "printf 'apple 5\nbanana 3\ncherry 8\n' \| awk '{ s += \$2 } END { print s }'"` | `16` |

**Cross-platform caveat** — the `find … | xargs awk '…'` DoD case from
the plan does not work on Windows out of the box. Reason is unrelated
to awk-rs: `xargs` `execvp`s its target command, which doesn't see
brush's bundled builtins, and Windows has no `awk.exe` on PATH unless
the user installed Git Bash MSYS2. On Linux/macOS where `awk` is
typically present on PATH, the pipeline works as expected. Same
limitation applies to `find -exec`, `parallel`, etc.

**Maturity caveat**: awk-rs v0.1.0 is a fresh single-author + dependabot
crate. Upstream README claim of "100% POSIX-compatible" is supported by
639 tests at 86% library coverage and Criterion benches showing ~1.6×
gawk on a 100k-line sum, but production-grade awk scripts (multi-file
hold space, complex regex backreferences, locale edge cases) should be
exercised before relying on this. The crate's listed repository
(`github.com/pegasusheavy/awk-rs`) returns 404 — the source previously
lived at `github.com/quinnjr/rawk` and was migrated; the published
crates.io tarball is the authoritative source today.

**Files changed**

- `brush-bundled-extras/Cargo.toml` — add `awk-rs` optional dep + features
- `brush-bundled-extras/src/lib.rs` — `awk_adapter` + `awk_run` + `print_awk_help` + registration
- `brush-shell/Cargo.toml` — `experimental-bundled-extras-awk-rs` flag
- `brush-shell/src/bundled.rs` — extend cfg gate

#### `feat(bundled): ship sed via uutils/sed crates.io dep`

Cycle 0a of [`docs/planning/posixutils-rs-integration.md`](./docs/planning/posixutils-rs-integration.md).
**First gap-filler from the posixutils-rs-integration plan to land.**

`sed` is now available as a bundled builtin behind a new feature flag.
The implementation is a clean crates.io dep on
[`uutils/sed`](https://github.com/uutils/sed) v0.1.1 — no vendoring,
no behavioral overrides. uucore version (`0.8.0`) matches
brush-coreutils-builtins exactly, so the dep graph stays single-version.
MSRV (`1.88`) matches brush's workspace MSRV (`1.88.0`), so no MSRV
friction.

Wiring follows the existing `find`/`xargs` adapter precedent:

| Layer | What landed |
|---|---|
| `brush-bundled-extras/Cargo.toml` | `sed = { version = "0.1.1", optional = true }`; new features `extras.sed` and `extras.uutils-sed-all`; `extras.all` now layers in `extras.uutils-sed-all`. |
| `brush-bundled-extras/src/lib.rs` | New `sed_adapter` calls `sed::sed::uumain(args.into_iter())`. SIGPIPE/localization init from `uucore::bin!` is intentionally omitted — bundled dispatch always runs sed in a fresh `brush --invoke-bundled` subprocess. |
| `brush-shell/Cargo.toml` | New `experimental-bundled-extras-uutils-sed` feature flag pulling `extras.uutils-sed-all`. |
| `brush-shell/src/bundled.rs` | `cfg(any(...))` gate around the bundled-extras registry merge extended to include `experimental-bundled-extras-uutils-sed`. |

**Smoke verification on Windows** (rustc 1.88.0 host build):

| Command | Output |
|---|---|
| `brush -c "echo a \| sed s/a/b/"` | `b` |
| `brush -c "echo hello \| sed s/h/H/"` | `Hello` |
| `brush -c "type sed"` | `sed is a shell builtin` |
| `brush -c "printf 'foo\nbar\nbaz\n' \| sed -n '2p'"` | `bar` |
| `brush -c "printf 'one\ntwo\nthree\n' \| sed 's/o/O/g'"` | `One` / `twO` / `three` |
| `brush -c "sed --version"` | `sed 0.1.1` |

**Maturity caveat**: uutils/sed is at 0.1.1 — pre-feature-complete.
POSIX sed has a large surface (hold space, branching, label commands,
multi-line `N`/`P`/`D`); upstream may not cover all of it yet. Real-world
sed scripts should be tested before relying on this in production. The
fork tracks upstream for v0.2.0+ as the next re-evaluation point.

**Files changed**

- `brush-bundled-extras/Cargo.toml` — add `sed` optional dep + features
- `brush-bundled-extras/src/lib.rs` — `sed_adapter` + registration
- `brush-shell/Cargo.toml` — `experimental-bundled-extras-uutils-sed` flag
- `brush-shell/src/bundled.rs` — extend cfg gate

### 🐛 Bug Fixes

#### `fix(windows): only suppress console window when brush has no console`

Regression introduced by `0299f3a` (in v0.3.1). That fix applied
`CREATE_NO_WINDOW` (`0x0800_0000`) unconditionally on every Windows
child spawn to suppress a console-window flash for non-console hosts
(Claude Code's Bash tool, editor terminals, automation harnesses).

Symptom of the regression: when brush was launched **inside a real
Windows console** (cmd / pwsh / Windows Terminal / mintty), bundled
coreutils produced no visible output. The shell prompt rendered fine
and inline builtins (`echo`, `pwd`, command substitution) worked, but
`ls`, `cat`, `wc`, pipelines like `seq 1 5 | sort -r`, etc. silently
returned with no stdout.

Root cause: `CREATE_NO_WINDOW` doesn't just suppress new-console
allocation — it also detaches the child from the parent's console.
When stdio is inherited via `STARTUPINFO`, those handles are now
console handles the child process is no longer attached to and
cannot write to. Bundled coreutils re-exec brush as a shim child, so
every bundled command went through this path and lost its output.
The original `0299f3a` changelog claim that "stdio handles still
inherit through `STARTUPINFO`, so pipelines and captured output are
unaffected" was true only for pipe/file stdio, not for console-handle
stdio.

Fix: gate `CREATE_NO_WINDOW` on the parent having no attached console
itself — `GetConsoleWindow() == NULL`. Result is cached in a
`OnceLock<bool>`, so the spawn path stays a single syscall on first
use and a load thereafter. No new dependencies — single
`extern "system"` declaration for `GetConsoleWindow` from `kernel32`.

| Host scenario                              | Before fix      | After fix              |
|--------------------------------------------|-----------------|------------------------|
| brush in cmd / pwsh / Windows Terminal     | no output       | output prints          |
| brush -c "..." from Claude Code Bash tool  | output via pipe (worked) | output via pipe (still works), no flash |
| brush spawned by editor terminal           | output via pipe (worked) | output via pipe (still works), no flash |

Both intents — "no flash for non-console hosts" and "interactive
output from a real console" — are now satisfied.

**Files changed**

- `brush-core/src/sys/tokio_process.rs` — `host_has_attached_console()` helper + conditional flag

# v0.3.1 - 2026-04-25

> Per-component version bumps in this release:
>
> | Crate                      | Previous | New     | Why                                                                                                                                                |
> |----------------------------|----------|---------|----------------------------------------------------------------------------------------------------------------------------------------------------|
> | `brush-core`               | 0.4.0    | 0.4.1   | Pgid plumbing through `ExecutionContext`; new `BundledDispatch` struct + `Registration::bundled_dispatch` field; `Registration` is now `#[non_exhaustive]`; new `raw_builtin()` factory; `special()` is no longer `const fn`; `execute_via_bundled` method on `SimpleCommand`; Windows console-flash suppression on child spawn; MSYS path translator for `/c/...`-style paths. |
> | `brush-shell`              | 0.3.0    | 0.3.1   | New `experimental-bundled-coreutils-{unix,linux}-extras` and `experimental-bundled-extras{,-findutils}` feature flags; `bundled.rs` shim now uses `BundledDispatch` + factory pattern; `install_default_providers()` merges the new `brush-bundled-extras` registry; `bash` binary alias produced alongside `brush`; `-c` adjacency rewriting fix; bash-version banner; `--norc`/`--noprofile`/`--no-config` interactive defaults. |
> | `brush-coreutils-builtins` | 0.1.0    | 0.1.1   | 26 missing utilities + `[` test alias added (Phase 0 reconciled against canonical `uutils/coreutils@0.8.0`); new aggregate features `coreutils.all-unix`, `coreutils.all-linux`; target-gated dep blocks for Unix-only and Linux-only `uu_*` crates. |
> | `brush-bundled-extras`     | —        | 0.1.0   | New crate. Adapter wrappers for non-uutils-coreutils utilities. Currently ships `find` + `xargs` from `uutils/findutils@0.8.0` (Cycle 2 of `coreutils-coverage-expansion.md`). |
>
> Crates not bumped (no source changes on this branch): `brush-builtins`,
> `brush-experimental-builtins`, `brush-parser`, `brush-interactive`,
> `brush-test-harness`, `xtask`, `fuzz`, top-level `brush`.

### ♻️ Refactors

#### `refactor(bundled): route bundled dispatch through external-spawn machinery`

Cycle 2 of [`docs/planning/bundled-coreutils-pipelines.md`](./docs/planning/bundled-coreutils-pipelines.md).
**Architectural cleanup** — the cycle was originally framed as a
parallelism unlock, but empirical measurement showed pre-Path-A
pipelines were already parallel (via `tokio::task::spawn_blocking` +
`ExecutionSpawnResult::StartedTask` for owned-shell builtins). Path A
still landed because it delivers real, if smaller, wins:

- **Bundled commands now route through the same machinery as ordinary
  PATH commands.** `SimpleCommand::execute` recognizes a new
  `Registration::bundled_dispatch` field and short-circuits to a new
  `execute_via_bundled` method that calls the existing
  `execute_via_external` path. The shim's spawn-and-wait indirection is
  replaced by direct external-spawn dispatch.
- **No `spawn_blocking` thread per pipeline stage.** Bundled stages are
  spawned directly via `sys::process::spawn`, returning
  `ExecutionSpawnResult::StartedProcess` — same shape as PATH commands.
  Small win for long pipelines.
- **Pgid handling is uniform with external commands.** Bundled stages
  now honor `cmd.process_group_id` via the same code path
  (`commands.rs::execute_external_command`) that PATH commands use,
  instead of via a shim-specific copy.
- **Single `ExecutionSpawnResult` variant for bundled and external
  commands.** Previously bundled returned `StartedTask` (tokio join
  handle) and external returned `StartedProcess` (OS child). Now both
  are `StartedProcess` — removing a divergence the orchestrator code
  had to handle.

**Public API additions to `brush-core`** (SemVer-relevant):

- New `BundledDispatch` struct in `brush-core::builtins`, carrying an
  executable path and an opaque dispatch flag. Set on a
  `Registration` via `with_bundled_dispatch()`.
- New `Registration::bundled_dispatch: Option<BundledDispatch>` field.
- `Registration` is now `#[non_exhaustive]` so future field additions
  don't break downstream consumers.
- New `raw_builtin(execute_func, content_func)` factory function for
  consumers (like `brush-shell::bundled`) that don't fit the
  trait-based factories.
- `Registration::special()` is no longer `const fn` (the
  `BundledDispatch::exe_path: PathBuf` adds a non-const-evaluatable
  destructor). Semantic behavior is unchanged.

**Path B prototype skipped**: the original plan called for prototyping
both Path A (new field on `Registration`) and Path B (changing
`CommandExecuteFunc` return type to `ExecutionSpawnResult`) and
choosing by measurement. The decision rule was gated on parallelism
gain. Once empirical measurement showed there was no parallelism gap
to close, Path B's much larger SemVer break (modifying the type alias
all ~50 builtin sites consume) had no payoff. Correctly skipped.

**Linux pgid integration test added** at
`brush-shell/tests/bundled_pgid.rs`, gated to `cfg(target_os = "linux")`.
Runs `cat /proc/self/stat | sh -c 'ps -o pgid= -p $$'` and asserts that
bundled `cat`'s pgid (read from `/proc/self/stat` field 5) equals the
`sh` stage's pgid (printed by `ps`). End-to-end check that Cycle 1
plumbing + Cycle 2 routing compose correctly. CI validates on Linux
runners; Windows compiles past it via the file-level cfg gate.

**Empirical timings on Windows** (debug build vs installed release):

| Workload                                   | Pre-Path-A (release) | Path A (debug) |
|--------------------------------------------|----------------------|----------------|
| `sleep 2 \| sleep 2 \| sleep 2`            | 2.4s                 | 2.8s           |
| `seq 1 5_000_000 \| wc -l`                 | 0.39s                | 0.87s (debug)  |

Both achieve parallelism; differences are dominated by build mode, not
orchestration shape.

**Files changed**

- `brush-core/src/builtins.rs` — `BundledDispatch` struct,
  `Registration::bundled_dispatch` field, `#[non_exhaustive]`,
  `raw_builtin()` factory, `with_bundled_dispatch()` builder,
  `special()` loses `const fn`.
- `brush-core/src/commands.rs` — `SimpleCommand::execute` branches on
  `bundled_dispatch.is_some()`; new `execute_via_bundled()` method
  builds the spawn argv and delegates to `execute_via_external`.
- `brush-shell/src/bundled.rs` — `shim_registration` attaches
  `BundledDispatch` via `with_bundled_dispatch`. Uses new
  `raw_builtin()` factory (struct-literal blocked by
  `#[non_exhaustive]`). The inline `shim_execute` is now an
  unreachable defensive fallback. Old TODO block removed (both TODOs
  resolved).
- `brush-shell/tests/bundled_pgid.rs` — new Linux pgid integration test.

### ✨ Features

#### `feat(extras): bundle find / xargs from uutils/findutils via adapter wrapper`

Cycle 2 of [`docs/planning/coreutils-coverage-expansion.md`](./docs/planning/coreutils-coverage-expansion.md).
Closes the gap between brush's bundled coreutils set and the next-most-asked-for
utilities: `find` and `xargs` from
[`uutils/findutils@0.8.0`](https://github.com/uutils/findutils/tree/0.8.0).

**New crate**: `brush-bundled-extras`. Houses adapter wrappers for every
non-uutils-coreutils utility we ship. Pairs with the existing layout:

```text
brush-builtins                — native shell builtins (cd, eval, trap, ...)
brush-experimental-builtins   — experimental natives (save)
brush-coreutils-builtins      — direct uutils-coreutils bundling
brush-bundled-extras          — adapter-wrapped non-coreutils utilities  ← NEW
```

The new crate exists because findutils' API does **not** match uutils-coreutils'
`uumain` shape:

```rust
// uutils/coreutils — what brush-coreutils-builtins ships:
uumain(args: impl Iterator<Item = OsString>) -> i32

// uutils/findutils — what we have to adapt:
findutils::find::find_main(args: &[&str], deps: &StandardDependencies) -> i32
findutils::xargs::xargs_main(args: &[&str]) -> i32
```

Per-utility adapter functions in `brush-bundled-extras/src/lib.rs` translate:

- argv `Vec<OsString>` → `Vec<String>` via `OsString::to_string_lossy()`,
  then `&[&str]`. **Lossy on non-UTF-8 OS args** — invalid UTF-8 sequences
  are replaced with U+FFFD. In practice rare for sane file names; if
  lossless behavior is required, fall through to a system `find`/`xargs`
  on PATH.
- For `find`: construct `findutils::find::StandardDependencies::new()`
  (real-IO/clock/fs implementation) and pass it through.

**Why one crate for all non-coreutils extras**: the original plan called
for one crate per upstream repo (`brush-findutils-builtins`,
`brush-diffutils-builtins`, ...). Cycle 2 pre-flight invalidated that
default. The plan's load-bearing argument for separate crates was
"`uucore` version-skew tolerance", but version coexistence is a Cargo
*dependency-graph* concern, not a *crate-layout* one — Cargo pulls both
`uucore` versions whether the deps live in one crate or many. With
that argument gone, separate-crate is just over-fragmentation. The
mega-crate also future-fits ripgrep/sed/awk integration whose upstreams
don't naturally fit a "per-uutils-source" pattern. See plan Decision Log
2026-04-25.

**uucore version skew**: `findutils 0.8.0` pins `uucore = "0.0.30"`;
`brush-coreutils-builtins` pins `uucore = "0.8.0"`. Cargo resolves both,
duplicating uucore-side code in the binary. Accepted cost — bounded; not
a regression vs. shipping no `find` at all.

**New feature flags on `brush-shell/Cargo.toml`** (opt-in, independent of
the coreutils flags):

- `experimental-bundled-extras` — top-level aggregate. Currently enables
  findutils only (Cycle 2); future cycles add diffutils/procps and
  potentially grep/sed/awk to this set.
- `experimental-bundled-extras-findutils` — explicit findutils-only
  opt-in.

**Installing with find / xargs included**:

```sh
cargo install --locked --path brush-shell --force \
  --features experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras
```

(Add `-coreutils-unix-extras` or `-coreutils-linux-extras` from Cycle 1
on Unix/Linux for the broader utility set.)

**Smoke-tested** on Windows (`cargo build -p brush-shell --features
experimental-bundled-extras`, exit 0):

```text
$ ./target/debug/brush.exe -c 'type find; type xargs; find . -maxdepth 1 -name "*.toml"'
find is a shell builtin
xargs is a shell builtin
.\Cargo.toml
.\cliff.toml
.\clippy.toml
.\deny.toml
.\release-plz.toml
.\rustfmt.toml

$ ./target/debug/brush.exe -c \
    'printf "%s\n" .\\Cargo.toml .\\cliff.toml | xargs -n1 cmd /c echo'
.Cargo.toml
.cliff.toml
```

**Known limitations** (upstream behavior, not adapter bugs):

- **`xargs` cannot invoke shell builtins.** xargs spawns subprocesses via
  the OS `fork`/`exec` (or `CreateProcess` on Windows), which doesn't see
  brush's builtins. So `xargs echo` fails to find `echo` even though it's
  a brush builtin — pass a real binary instead. Same applies to
  `find -exec`. This is a hard limit of how `find`/`xargs` work, not
  fork-able to fix in our adapter.
- **`find` panics on broken pipe on Windows.** When a downstream stage
  (e.g., `find | head`) closes early, findutils 0.8.0 panics in
  `printer.rs:67` on a Windows EPIPE because it `unwrap()`s a
  `Result::Err`. Same family of issue as the coreutils EPIPE noise on
  Windows (no SIGPIPE → uutils' EPIPE handling is brittle on Windows).
  Tracked separately; needs an upstream fix in findutils. Workaround:
  on Windows, write find's output to a file and pipe the file, instead
  of streaming through `head`.
- **Argv lossiness on non-UTF-8 OS args.** Documented above; affects
  paths with surrogate UCS-2 on Windows or arbitrary bytes on Unix.
  Substituted with U+FFFD before reaching findutils. Use a system find
  for fully lossless paths.

**Files changed**

- `brush-bundled-extras/Cargo.toml` — new crate; optional `findutils`
  dep; per-utility features + aggregates.
- `brush-bundled-extras/src/lib.rs` — `find_adapter`, `xargs_adapter`,
  `bundled_commands()`.
- `Cargo.toml` — workspace member registration.
- `brush-shell/Cargo.toml` — `brush-bundled-extras` dep (optional);
  `experimental-bundled-extras` and `-findutils` feature flags.
- `brush-shell/src/bundled.rs` — `install_default_providers()` merges
  `brush-bundled-extras::bundled_commands()` into the registry under
  the new feature flags.

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

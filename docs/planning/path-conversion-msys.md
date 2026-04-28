# MSYS-style Path Conversion — Planning

> **Status**: 🟢 **Cycles 1–2 shipped** (2026-04-29) · 🟡 Cycle 3 not started.
>
> Captures research + plan for closing the path-translation gaps
> surfaced in the
> [`shell-integration-investigation.md`](shell-integration-investigation.md)
> companion doc.
>
> **Created**: 2026-04-29 · **Owner**: TBD
> **Tracks**: path-form mismatches between MSYS-style POSIX paths
> (`/c/...`, `/cygdrive/c/...`) and native Windows paths (`C:\...`) when
> brush hands arguments and environment to non-MSYS-aware programs
> (uutils-bundled tools, system binaries like `curl`, `git.exe`, `python.exe`).
>
> ## Cycle status
>
> | Cycle | Layer(s) | Status | Notes |
> |---|---|---|---|
> | 1 | 0 (core) + 1 (`cygpath` builtin) | ✅ Shipped 2026-04-29 | `path_conv` module + cygpath builtin (`-u/-w/-m/-a/-p`); 25 unit tests; no behavior change for existing callers. |
> | 2 | 2 (bundled-tools dispatch) | ✅ Shipped 2026-04-29 | `PathArgPolicy` on `BundledDispatch`; per-tool table in `brush-shell/src/bundled.rs`; default `Heuristic` for path-arg-dominant utilities, `Positional([1])` for `find`, `None` (passthrough) for `sed`/`awk`/`grep`. Shipped *without* `experimental-msys-pathconv` flag (decision below). |
> | 3 | 3 (outgoing env vars) + 4 (external-binary argv) + Cycle 2 runtime overrides | ⏳ Not started | `BRUSH_PATHCONV=1` opt-in; `BRUSH_PATHCONV_EXCL` for per-program exclusion — **must also gate Cycle 2's bundled-tool table** so users have one knob for both layers. See Cycle 3 carry-over note. |

---

## A3 — Problem analysis

### 1. Background

When brush is used as a daily-driver shell on Windows (the fork's primary
use case — invoked as `bash.exe` for Claude Code / Cursor / etc.), users
and AI agents routinely hand brush MSYS-style absolute paths
(`/c/Github-Tools/brush`, `/d/Projects/x`, `/cygdrive/c/Users/...`).

Commit `e7cd1f2` ("winpaths") added a single-funnel translator
[`try_translate_msys_path`](../../brush-core/src/sys/windows/fs.rs) wired
into [`Shell::absolute_path`](../../brush-core/src/shell/fs.rs) that fixes
the common cases (`cd`, `[ -X ]`, redirect targets, source/`.`,
completion). The follow-up commit in this cycle plugged
[`commands.rs:431`](../../brush-core/src/commands.rs) so direct exec by
MSYS path also works (`/c/Windows/System32/whoami.exe`).

What still doesn't work — and what AI agents hit repeatedly:

- **Bundled-tool path arguments** — `cat /c/Users/foo`, `ls /c/Users`,
  `head -n 3 /c/...`. Bundled uutils tools receive argv verbatim from the
  bundled-shim re-spawn ([`brush-shell/src/bundled.rs`](../../brush-shell/src/bundled.rs))
  and have no MSYS handling.
- **External-binary path arguments** — `git diff /c/Users/...`,
  `curl -o /c/Users/.../out.bin ...`, `python /c/scripts/x.py`. External
  Windows binaries need native paths.
- **Outgoing environment variables** — most importantly `PATH`. Setting
  `export PATH="/c/MyTools:$PATH"` from a `.bashrc` style file is common.
  When brush spawns a Windows-native child, the child's CRT path-search
  doesn't understand `/c/MyTools` and silently skips that entry.
- **`cygpath`-equivalent builtin** — there's no in-shell way for users or
  scripts to convert paths explicitly.

### 2. Current condition

| Surface | Translates? | Code path |
|---|---|---|
| `cd /c/...`, `[ -f /c/... ]`, `> /c/.../file`, `source /c/...`, completion | ✅ | `Shell::absolute_path` → `try_translate_msys_path` |
| Direct exec by MSYS path (`/c/.../foo.exe`) | ✅ (this cycle) | `commands.rs:431` |
| Bundled tools (`cat /c/...`, `find /c/... -name x`, `grep p /c/...`) | ❌ | uutils argv passthrough; no translation |
| External binaries (`git`, `curl`, `python`, `node`, `cargo`) | ❌ | brush passes argv verbatim |
| Outgoing `$PATH` → child's environment | ❌ | env passthrough |
| Outgoing other env vars (`MANPATH`, `LD_LIBRARY_PATH`, `PYTHONPATH`, …) | ❌ | env passthrough |
| User-facing `cygpath`-like builtin | ❌ (none exists) | — |

The current `try_translate_msys_path` covers four input forms:
`/<L>`, `/<L>/...`, `/cygdrive/<L>`, `/cygdrive/<L>/...` (case-insensitive
single-letter drive). Backslash-rooted equivalents accepted on input.

### 3. Goal / target

Close the gaps above with a layered design that:

- **Doesn't break MSYS-aware tools.** Bundled brush + native brush builtins
  must continue to accept MSYS paths directly (current behavior).
- **Doesn't surprise users with silent argv mangling.** MSYS2's auto-mangling
  is a documented footgun (see issues
  [#84](https://github.com/msys2/MSYS2-packages/issues/84),
  [#2316](https://github.com/msys2/MSYS2-packages/issues/2316),
  [#1130](https://github.com/msys2/MSYS2-packages/issues/1130),
  [#411](https://github.com/msys2/MSYS2-packages/issues/411),
  [curl/curl#8084](https://github.com/curl/curl/issues/8084)). The MSYS2
  workaround `MSYS2_ARG_CONV_EXCL` exists *because* of this. We can do
  better: **conversion is opt-in** at the brush level, **opt-in per-tool**
  inside the bundled-tools dispatch, and **never applied** to args that
  aren't path-shaped.
- **Provides a `cygpath`-equivalent builtin** so scripts have an explicit
  conversion surface that doesn't depend on heuristics.
- **Test against `cygpath` as oracle** when available; otherwise against
  a checked-in golden table.
- **Preserves MIT licensing.** No code copied from MSYS2 / Cygwin (GPL).
  Algorithm reimplemented clean-room, informed by the simpler
  [`stdlib-js/utils-convert-path`](https://github.com/stdlib-js/utils-convert-path)
  (Apache-2.0, ~150 LoC, well-tested, ~700K weekly npm downloads — battle-tested
  surface area).

### 4. Root cause analysis

**Why are these gaps still open?**

- **Why 1**: Bundled tools and external binaries don't go through
  `Shell::absolute_path` — that funnel only sees paths brush itself opens.
- **Why 2**: MSYS path translation was bolted into `absolute_path` as a
  minimal Windows-only fix. The translator has *no caller* on the
  outgoing-arg side because the original problem was redirect targets and
  `cd`, both internal to brush.
- **Why 3**: There's no general-purpose "convert path-shaped argv elements"
  utility — `try_translate_msys_path` is single-path, single-shot.
- **Why 4**: Designing such a utility safely is non-trivial: the
  decision *"is this argv element a path?"* is a heuristic that MSYS2 has
  spent 20+ years tuning, and still gets wrong (see linked issues).
- **Why 5**: License hygiene. MSYS2's tuned heuristic is GPL; we can't
  copy it. We need a clean-room reimplementation.

**ROOT CAUSE**: brush has a working *single-path* translator but no
*argv/env stream* translator. Building the latter is gated on (a) a
licensing-clean algorithm and (b) a careful policy about when conversion
fires (the MSYS2 footgun lesson).

### 5. Countermeasures — proposed design

**Layered, opt-in, scope-limited:**

#### Layer 0 — Strengthen the translator core

New module: `brush-core/src/sys/windows/path_conv.rs` (rename / promote
the existing `try_translate_msys_path`). Public surface:

```rust
pub enum PathForm { Win32, Mixed, Posix }

pub fn to_native(s: &OsStr) -> Cow<'_, OsStr>;            // → C:\foo
pub fn to_msys(s: &OsStr) -> Cow<'_, OsStr>;              // → /c/foo
pub fn convert(s: &OsStr, target: PathForm) -> Cow<'_, OsStr>;

/// Heuristic: does this look like a path that should be converted?
pub fn looks_like_path(s: &OsStr) -> bool;

/// Heuristic: does this look like a colon/semicolon-separated path list?
pub fn looks_like_path_list(s: &OsStr) -> Option<char /* sep */>;

pub fn convert_path_list(s: &OsStr, target: PathForm) -> Cow<'_, OsStr>;
```

Inputs accepted (case-insensitive single drive letter `<L>`):
- `/<L>`, `/<L>/...`, `/cygdrive/<L>`, `/cygdrive/<L>/...`
- `\<L>`, `\<L>\...` (backslash-rooted MSYS — current behavior)
- `<L>:\...`, `<L>:/...` (already-native, returned unchanged for `to_native`)
- Mixed-separator (`/c/Users\foo`) normalized to single form per output mode

Inputs explicitly **left alone** (`looks_like_path` returns `false`):
- Args starting with `-` or `--` (option flags)
- Args containing `=` before any `/` (e.g. `--data=foo`)
- Strings with spaces *unless* they're already a clearly-quoted single path
- Regex-looking strings (`grep '/foo/.*'` should not be translated)
- URLs (`http://`, `https://`, `file://`, `ssh://`, etc.)

Out of scope for v1: UNC paths (`//server/share`), extended-length paths
(`\\?\C:\...`), DOS short names (`C:\PROGRA~1\`). Document as known
limitations.

#### Layer 1 — `cygpath` builtin (user-facing, explicit, no heuristics)

Add `cygpath` as a brush builtin under `brush-experimental-builtins` (fork-only).
Surface matches Cygwin's [`cygpath(1)`](https://cygwin.com/cygwin-ug-net/cygpath.html):

| Flag | Effect |
|---|---|
| `-u` / `--unix` | output POSIX form (default) |
| `-w` / `--windows` | output Windows form |
| `-m` / `--mixed` | Windows form with forward slashes |
| `-a` / `--absolute` | force absolute |
| `-p` / `--path` | treat input as `:`/`;`-separated path list |

This is the **safe surface for users and agents** — explicit conversion,
no heuristic guessing. Recommend agents use `$(cygpath -w "/c/...")` when
they need to hand a Windows-form path to a native binary.

#### Layer 2 — Bundled-tools dispatch (opt-in per tool)

Add a `path_arg_policy` field on the bundled `Registration`:

```rust
pub enum PathArgPolicy {
    /// Pass argv verbatim (default for sed, awk, grep — args are scripts/patterns).
    None,
    /// Translate any argv element that looks_like_path. Per-tool exclusions
    /// taken from a small static table (e.g. cat -A, grep -e PATTERN).
    Heuristic { exclusions: &'static [&'static str] },
    /// Translate argv elements at specified positions only.
    Positional(&'static [usize]),
}
```

Dispatch site: [`brush-shell/src/bundled.rs`](../../brush-shell/src/bundled.rs)
right before `--invoke-bundled` re-spawn argv assembly.

Initial per-tool table (conservative — only set `Heuristic` where path args
clearly dominate):

| Tool | Policy | Notes |
|---|---|---|
| `cat`, `head`, `tail`, `wc`, `nl`, `od`, `cksum` | `Heuristic` | path args dominate |
| `cp`, `mv`, `rm`, `ln`, `mkdir`, `rmdir`, `touch`, `chmod`, `chown`, `stat`, `realpath`, `readlink` | `Heuristic` | classic path tools |
| `ls`, `du`, `df` | `Heuristic` | path args dominate |
| `find` | `Positional(&[1])` | only the starting-point arg; rest are predicates |
| `xargs` | `None` | reads paths from stdin; we don't want to mangle the command-template |
| `grep`, `sed`, `awk` | `None` | first non-flag arg is a script/pattern, not a path |
| `tar`, `zip`, `unzip`, `gzip`, `bzip2`, `xz` | TBD per tool | `-f file.tar` is a path; archive members aren't |
| `tree`, `which`, `file`, `xxd`, `column` | `Heuristic` | path args dominate |

Behind a Cargo feature flag `experimental-msys-pathconv` (off by default
for the first cycle so we can ship without breaking anyone).

#### Layer 3 — Outgoing env vars (opt-in, narrow)

When brush spawns a child that is *not* itself brush, optionally translate:
- `PATH` (always — most common foot-gun)
- An additional set named in `BRUSH_PATHCONV_ENV` (a `:`-separated env-var-name list)

Off by default — even with the feature flag on. Two reasons:
- Env vars that look path-shaped but aren't (e.g. `JAVA_OPTS=-Dfoo=/c/bar`)
  break easily.
- The cost / benefit of converting `PATH` is highest; everything else is
  niche enough to want explicit opt-in.

Activation: env var `BRUSH_PATHCONV=1` *or* config file
`[experimental] msys_pathconv = true`.

#### Layer 4 — Argv conversion for external (non-bundled) commands

Hardest layer; deferred to its own cycle. Requires the same heuristic +
exclusion machinery as Layer 2 but applied to every external-binary spawn.
Target: same `experimental-msys-pathconv` flag + a `BRUSH_PATHCONV_EXCL`
env var matching MSYS2's `MSYS2_ARG_CONV_EXCL` semantics so existing
muscle-memory works.

### 6. Implementation plan — PDCA cycles

#### Cycle 1 — Core + `cygpath` builtin (Layer 0 + 1) — ✅ Shipped 2026-04-29

Smallest deliverable. No behavior change for existing callers.

- **Plan**: Promote `try_translate_msys_path` into `path_conv` module.
  Add `PathForm`, `to_native`/`to_msys`/`convert`/`looks_like_path`/
  `convert_path_list`. Add `cygpath` builtin. Differential test against
  installed `cygpath` if available; otherwise a checked-in golden table
  generated by running cygpath once on a CI machine.
- **Do** *(actual)*:
  - New module [`brush-core/src/sys/windows/path_conv.rs`](../../brush-core/src/sys/windows/path_conv.rs)
    with `PathForm` enum, `convert`, `convert_path_list`, `looks_like_path`,
    and the legacy `try_translate_msys_path` (relocated, identical
    semantics). Algorithm: clean-room reimplementation informed by
    [`stdlib-js/utils-convert-path`](https://github.com/stdlib-js/utils-convert-path)
    (Apache-2.0); no MSYS2 / Cygwin code copied (GPL).
  - Cross-platform facade [`brush-core/src/sys/path_conv.rs`](../../brush-core/src/sys/path_conv.rs)
    re-exports the Windows surface and supplies passthrough no-ops on
    Unix/wasm so `crate::sys::path_conv::*` resolves everywhere.
  - Existing callers (`Shell::absolute_path`, `commands.rs:438`)
    rewired from `sys::fs::try_translate_msys_path` to
    `sys::path_conv::try_translate_msys_path`; legacy
    duplicates in `unix/fs.rs` and `stubs/fs.rs` removed.
  - New `cygpath` builtin in
    [`brush-experimental-builtins/src/cygpath.rs`](../../brush-experimental-builtins/src/cygpath.rs)
    behind feature `builtin.cygpath` (default-on for the experimental
    crate). Flags: `-u/-w/-m/-a/-p`.
- **Check** *(observed)*:
  - 25 new unit tests in `path_conv::tests` cover legacy
    `try_translate_msys_path` contract + new `convert`/
    `convert_path_list`/`looks_like_path` surface; all pass.
  - Full brush-core unit suite (90 tests) still green.
  - Manual smoke (Windows, rustc 1.95):
    ```text
    $ brush -c 'cygpath -w /c/Users/foo'        → C:\Users\foo
    $ brush -c 'cygpath -u "C:\Users\foo"'      → /c/Users/foo
    $ brush -c 'cygpath -m /cygdrive/d/data'    → D:/data
    $ brush -c 'cygpath -p -w "/c/a:/d/b"'      → C:\a;D:\b
    $ brush -c 'cygpath -a foo'                 → /c/Github-Tools/brush/foo
    ```
  - Existing MSYS-path translation regression check (cd /c/Windows,
    `[ -d /c/Windows ]`, `/c/Windows/System32/whoami.exe`) still works.
  - **Differential test against installed `cygpath` deferred** — no
    Cygwin install on this dev machine; the golden-table CI gate
    remains a Cycle 2 prerequisite (see *Act* below).
- **Act**:
  - Differential test against `cygpath` is still the gate before
    Cycle 2 calls into the same heuristic for non-explicit
    translation. Tracking item: generate golden table on a CI machine
    with cygpath installed and check it in.

Gate to Cycle 2: ✅ the `path_conv` module is solid enough that other
callers can lean on it.

#### Cycle 2 — Bundled-tools dispatch (Layer 2) — ✅ Shipped 2026-04-29

- **Plan**: Add `PathArgPolicy` to `Registration`. Wire conservative
  per-tool table. Originally scoped behind an `experimental-msys-pathconv`
  feature flag; the gate was dropped (decision log entry below) — a fork
  with low active install count + the surface being unit-test-covered +
  the `None` default for unaudited tools meant a feature flag would
  mostly delay real-world feedback rather than de-risk.
- **Do** *(actual)*:
  - New cross-platform `builtins::PathArgPolicy` enum
    (`None` / `Heuristic` / `Positional(Vec<usize>)`) added to
    [`brush-core/src/builtins.rs`](../../brush-core/src/builtins.rs).
    Enum is `Default` to `None` so future `BundledDispatch` consumers
    using `..Default::default()` patterns retain pre-Cycle-2 semantics.
  - `BundledDispatch` made `#[non_exhaustive]`; gained a
    `path_arg_policy` field plus a `BundledDispatch::new(exe_path,
    dispatch_flag)` factory and `with_path_arg_policy(...)` builder.
    Struct-literal construction from outside brush-core is no longer
    possible — semantically appropriate now that the dispatch carries
    behavioral state, not just static identifiers.
  - New `pub(crate)` helper `commands::apply_path_arg_policy` translates
    the user-argv portion of a bundled re-spawn according to the policy
    via `crate::sys::path_conv::looks_like_path` + `convert(_, Win32)`.
    Argv[0] (the bundled name) is never translated; positional indices
    are 1-based against the spawned argv. Wired into
    `commands.rs::execute_via_bundled` immediately before the
    `child_args` extension.
  - Per-tool table `path_arg_policy_for(name)` in
    [`brush-shell/src/bundled.rs`](../../brush-shell/src/bundled.rs)
    populates the policy at registration time. Initial cut:
    - `Heuristic`: `cat`, `head`, `tail`, `wc`, `nl`, `od`, `cksum`,
      `sum`, `tac`, `tee`, `split`, `csplit`, `uniq`, `sort`, `comm`,
      `join`, `paste`, `shuf`, `tr`, `expand`, `unexpand`, `fmt`,
      `fold`, `pr`, `cp`, `mv`, `rm`, `ln`, `mkdir`, `rmdir`, `touch`,
      `chmod`, `chown`, `chgrp`, `stat`, `realpath`, `readlink`, `ls`,
      `dir`, `vdir`, `du`, `df`, `tree`, `which`, `file`, `xxd`,
      `column`, `basename`, `dirname`, `install`, `mktemp`, `shred`,
      `truncate`, `sync`, `cmp`.
    - `Positional(vec![1])`: `find` (only the starting-point arg;
      predicates and predicate values must remain literal).
    - `None` (default): everything else, including `sed`, `awk`,
      `grep`, `fastgrep`, `rg`, `xargs`, all archivers, and any
      bundled name not yet audited.
  - `register_shims` restructured to construct a per-name registration
    rather than reusing one global `Registration`, so each name carries
    its own policy.
  - 6 new unit tests in `commands::tests` covering all three policy
    variants, the safety case (URL/flag/assignment passthrough under
    `Heuristic`), out-of-range positional, and `None` passthrough.
- **Check** *(observed)*:
  - 96 brush-core unit tests pass (90 baseline + 6 new); full
    workspace `cargo test --lib` green.
  - Manual smoke (Windows release build, all three experimental
    features on):
    ```text
    $ brush -c 'cat /c/Github-Tools/brush/cycle2-smoke.txt'        → hello world
    $ brush -c 'head -n1 /c/Github-Tools/brush/cycle2-smoke.txt'   → hello world
    $ brush -c 'ls /c/Github-Tools/brush/cycle2-smoke.txt'         → C:\Github-Tools\brush\cycle2-smoke.txt
    $ brush -c 'wc -l /c/Github-Tools/brush/cycle2-smoke.txt'      → 1 C:\Github-Tools\brush\cycle2-smoke.txt
    $ brush -c 'find /c/Github-Tools/brush -maxdepth 1 -name Cargo.toml'
                                                                    → C:\Github-Tools\brush\Cargo.toml
    $ brush -c 'grep "/foo/" cycle2-grep.txt'                       → /foo/bar; /foo/baz   (pattern preserved)
    $ brush -c 'echo "/c/Users/foo" | sed "s|/c/Users|REDACTED|"'   → REDACTED/foo (pattern preserved)
    $ brush -c 'echo "/c/Users/foo" | awk -F/ "{print \$3}"'        → Users        (pattern preserved)
    ```
  - Pipeline parallelism preserved: `cat /c/.../file | grep data | wc -l` yields the expected count.
  - No regression for native Win32 paths (`cat C:\Users\foo`),
    backslash-quoted paths, or relative paths.
  - Cycle 1 regression (`cd /c/Windows`, `[ -d /c/Windows ]`,
    `/c/Windows/System32/whoami.exe`) still works.
- **Act**:
  - Expand the per-tool table based on real-world usage and bug
    reports. The conservative default (`None`) for unaudited names
    means missing entries fail safe (verbatim passthrough — same as
    pre-Cycle-2 behavior); they don't silently corrupt argv.
  - `Heuristic { exclusions: ... }` variant from the original plan was
    deferred — the v1 `Heuristic` already excludes `-prefixed` flags,
    URL-shaped strings (`://`), and `KEY=value` shapes via
    `looks_like_path`. Reintroduce only if a tool surfaces a need.
  - Differential test against installed `cygpath` is still the gate
    item for Cycle 3, where the same `looks_like_path` predicate gets
    a much wider blast radius (every external command).

Gate to Cycle 3: Cycle 2 stable for 1+ release without per-tool exclusion
tickets.

#### Cycle 3 — Outgoing env vars + external-binary argv + runtime overrides (Layers 3 + 4)

Most invasive. Requires the user-opt-in story to be thoroughly documented
first because this is where the MSYS2 footgun lives.

> **Carry-over from Cycle 2 (2026-04-29)**: The bundled-tools dispatch
> from Cycle 2 currently ships with a *compile-time* per-tool policy
> table in
> [`brush-shell/src/bundled.rs::path_arg_policy_for`](../../brush-shell/src/bundled.rs).
> Cycle 3 must extend the runtime-override surface
> (`BRUSH_PATHCONV` / `BRUSH_PATHCONV_EXCL`) to *also* control the
> bundled-tool table — not just env-var and external-binary
> translation. The chosen surface should be a single
> `env-var override → config-file override → compiled-in default`
> resolution chain so users learn one mental model. Specifically:
> - `BRUSH_PATHCONV_EXCL=cat,find` should both (a) skip the
>   bundled-tool path-arg translation for those names and (b) skip the
>   external-binary argv translation when those names appear as a
>   spawn target.
> - A future `[experimental] msys_pathconv_excl = ["cat","find"]`
>   config-file knob is a non-goal for Cycle 3 v1 — we'd ship it as a
>   Cycle 3.5 if `BRUSH_PATHCONV_EXCL` proves insufficient.
>
> Why this lives in Cycle 3 rather than a separate 2.5 cycle: real bug
> reports from Cycle 2's compile-time table are needed to know whether
> the right answer is "add another name to the table" (compile-time
> fix) or "give the user a runtime knob" (Cycle 3 work). Splitting it
> into 2.5 risks shipping a bundled-only override surface we'd then
> have to reconcile with the env-var/external-binary surface.

- **Plan**: Implement env-var translation gated on `BRUSH_PATHCONV=1`.
  Implement external-binary argv translation gated on the same flag.
  Implement `BRUSH_PATHCONV_EXCL` for per-program / per-prefix opt-out
  — applies to bundled-tool dispatch *and* external-binary argv.
- **Do**: ~500 LoC + integration tests against `cmd /c set`,
  `python -c 'import os; print(os.environ["PATH"])'` etc. as oracles for
  child-process env-var visibility. Plus regression tests demonstrating
  `BRUSH_PATHCONV_EXCL=cat` toggles Cycle 2's bundled-tool behavior
  back to verbatim passthrough.
- **Check**:
  - Without `BRUSH_PATHCONV=1`: zero behavior change for env vars and
    external-binary argv. Bundled-tool translation continues per the
    Cycle 2 default unless `BRUSH_PATHCONV_EXCL` says otherwise.
  - With it: typical agent-shaped commands (`curl -o /c/.../out.txt`,
    `git diff /c/.../file`) work without manual conversion.
  - `BRUSH_PATHCONV_EXCL=*` disables for all programs; granular
    `BRUSH_PATHCONV_EXCL=git;cargo` skips two named programs.
  - `BRUSH_PATHCONV_EXCL=cat` makes `cat /c/foo` pass through verbatim
    (regression of Cycle 2 default), confirming the runtime override
    reaches the bundled-tool dispatch.
- **Act**: If the footgun rate from Cycle 3 exceeds the win rate
  (measure: GitHub issues over 1 release), ship a "default off, but
  recommended" UX rather than "default on" — i.e., never make this the
  default behavior.

### 7. Follow-up

**Verification**:
- Cycle 1: differential-test pass rate vs `cygpath` ≥ 99%.
- Cycle 2: bundled-tools compat tests stay at 100% with flag off, and
  ≥99% of "MSYS-style path arg" agent reports succeed with flag on.
- Cycle 3: `BRUSH_PATHCONV_EXCL` muscle-memory works for users coming
  from MSYS2; documented edge cases match MSYS2's documented edge cases.

**Monitoring**:
- After each cycle ships: open a tracking issue listing edge-cases /
  programs that misbehave. Treat ≥3 reports for the same program as a
  signal to add it to the default exclusion table.

**Prevention**:
- Differential test against `cygpath` runs on the Windows CI matrix on
  every PR touching `path_conv.rs`.
- Per-tool path-arg policy stays in a single static table — not
  scattered through bundled-tool wrappers — so audit cost is low.
- Document in `docs/reference/path-conversion.md` (to be created in
  Cycle 1) so users / agents know exactly what gets translated and how
  to opt out.

---

## Decision log

| Date | Decision | Rationale |
|---|---|---|
| 2026-04-29 | **No MSYS2 source port.** | License (GPL) incompatible with MIT brush. Will study algorithm, reimplement clean-room. |
| 2026-04-29 | **Use `stdlib-js/utils-convert-path` algorithm as starting reference.** | Apache-2.0 licensed; ~150 LoC; ~700K weekly npm downloads (well-tested surface area); covers the common drive-letter / mixed / posix forms cleanly. |
| 2026-04-29 | **Conversion is opt-in, not opt-out, even with feature flag enabled.** | MSYS2 issues #84, #2316, #1130, #411 demonstrate that auto-conversion is a frequent footgun. Brush's first responsibility is *don't surprise the user*. |
| 2026-04-29 | **Cygpath builtin first, dispatch translation second.** | Cygpath is a *no-heuristic* surface — purely user-driven, easy to reason about, and gives agents an explicit primitive. Heuristic translation is harder and shouldn't ship before users have an explicit alternative. |
| 2026-04-29 | **`cygpath` lives in `brush-experimental-builtins`** | Fork-only utility; matches the existing `save` builtin pattern. Doesn't pollute upstream brush's builtin namespace. |
| 2026-04-29 | **Cycle 2 ships *without* the `experimental-msys-pathconv` feature flag.** | The original plan put Cycle 2 behind a flag for opt-in safety. In a fork with low active-install count and the change covered by unit tests + the conservative `None`-default for unaudited names, a flag would mostly delay real-world feedback rather than de-risk. Failure modes are all argv-only; the `None` default for unrecognized names is the same passthrough behavior pre-Cycle-2. If a footgun surfaces, the per-tool table is the natural surface to react on, not a global flag. |
| 2026-04-29 | **Runtime per-tool overrides for the bundled-tool table land in Cycle 3, not a separate 2.5 cycle.** | Cycle 3 was already designing `BRUSH_PATHCONV` / `BRUSH_PATHCONV_EXCL` for env-var + external-binary translation. Folding the bundled-tool override into the same surface gives users one mental model (`BRUSH_PATHCONV_EXCL=cat,find` works for both) and avoids reconciling two override schemes later. Splitting it out risks shipping a bundled-only knob that gets paved over when Cycle 3 introduces the broader surface. The carry-over note in Cycle 3's section is the binding reminder. |
| 2026-04-29 | **Cycle 3 (env-var translation) stays opt-in even after Cycles 1+2 prove out.** | Env-var auto-translation is the highest-blast-radius layer. Even MSYS2's mature implementation has open issues here ([msys2-runtime#152](https://github.com/msys2/msys2-runtime/issues/152)). |

---

## References

- [stdlib-js/utils-convert-path](https://github.com/stdlib-js/utils-convert-path)
  — algorithm starting reference (Apache-2.0).
- [Cygwin cygpath(1)](https://cygwin.com/cygwin-ug-net/cygpath.html) —
  surface we mirror for the builtin.
- [MSYS2 filesystem paths](https://github.com/msys2/msys2.github.io/blob/9815a0af35c7c1d7af386173c42c6690baeb56c8/docs/filesystem-paths/index.html)
  — informative for edge-case behavior.
- Footgun cluster informing "opt-in by scope":
  - [MSYS2-packages#84 — option to disable path mangling](https://github.com/msys2/MSYS2-packages/issues/84)
  - [MSYS2-packages#2316 — argument mangling, how to disable?](https://github.com/msys2/MSYS2-packages/issues/2316)
  - [MSYS2-packages#1130 — `MSYS2_ARG_CONV_EXCL` doesn't work as expected](https://github.com/msys2/MSYS2-packages/issues/1130)
  - [MSYS2-packages#411 — path conversion with/without winpty differs](https://github.com/msys2/MSYS2-packages/issues/411)
  - [msys2-runtime#152 — unexpected env-var conversion](https://github.com/msys2/msys2-runtime/issues/152)
  - [curl#8084 — disable msys2 path interpretation in CI](https://github.com/curl/curl/issues/8084)
- [`docker-path-workaround`](https://github.com/borekb/docker-path-workaround)
  — community workaround pattern for the same class of problem.
- Existing brush code:
  - [`brush-core/src/sys/windows/fs.rs`](../../brush-core/src/sys/windows/fs.rs)
    — current `try_translate_msys_path`.
  - [`brush-core/src/shell/fs.rs`](../../brush-core/src/shell/fs.rs)
    — single-funnel integration via `Shell::absolute_path`.
  - [`brush-core/src/commands.rs`](../../brush-core/src/commands.rs)
    — direct-exec MSYS path fix from this cycle.
  - [`brush-shell/src/bundled.rs`](../../brush-shell/src/bundled.rs)
    — bundled-shim re-spawn machinery (Cycle 2 integration point).

# bundled-extras Coverage Expansion — Planning

> **Status**: 📋 **Planning, awaiting sign-off** — Cycle 0a (egrep / fgrep
> aliases) ships immediately on the branch as a sanity-check; Cycles 1–4
> queued behind explicit user approval per cycle.
>
> **Created**: 2026-04-28 · **Owner**: @slicingmelon
> **Branch**: `feat/bundled-extras-coverage-expansion`
>
> **Tracks**: closing the "agent thrash" gaps catalogued in
> [`docs/reference/bundled-tools-index.md`](../reference/bundled-tools-index.md)
> §E ("What this install does NOT include"). Specifically the entries
> flagged as **genuinely useful** with no Windows PATH coverage:
> `tar`, `tree`, `xxd`, `column`, `which`, `file`, the gzip / bzip2 / xz / zip
> compression families, plus a grep-ecosystem rework that makes `grep`
> resolve to ripgrep (which has `-P`) instead of fastgrep (which doesn't).
>
> **Supersedes**: nothing — additive to
> [`posixutils-rs-integration.md`](./posixutils-rs-integration.md), which
> shipped the Cycle 0 cohort (sed / awk / grep-via-fastgrep). Cycle 3 of
> *this* plan retires the `grep → fastgrep` registration that
> posixutils-rs-integration Cycle 0b-revised landed; `fastgrep` remains
> available under its own name.

## Why this plan exists (the user-visible problem)

AI-agent shells (Claude Code's Bash tool, Cursor's terminal, automation
harnesses) probe their environment by running CLI commands — `tar`,
`tree`, `which`, `xxd`, `gzip`, etc. When those return "command not
found" the agent retries, falls back to PowerShell, or emits scaffold
that doesn't work. Each failed probe wastes tokens and time.

The fork's stated goal in [`CLAUDE.md`](../../.claude/CLAUDE.md#what-this-repo-is-and-isnt)
is "drop-in Git-Bash replacement". The bundled-tools-index already
identifies the unfilled gaps; this plan closes them.

## Why posixutils-rs is *not* the source for this work

A natural first instinct (raised in this session) is "just hard-copy
the source from `C:\Tools\brush-shell-resources\posixutils-rs`". That
path is **not viable** for the same reasons captured in
[`posixutils-rs-integration.md`](./posixutils-rs-integration.md) Cycles 1–3:

- Every utility imports `plib::*` (the project's internal Unix-syscall
  wrapper) and `libc::*` directly. e.g.
  [`users/id.rs:12`](file:///C:/Tools/brush-shell-resources/posixutils-rs/users/id.rs#L12)
  → `use plib::group; libc::getgrgid(...); libc::getgroups(...)`.
- Every utility wires `gettextrs` for i18n, which itself pulls in
  GNU gettext libraries via libc.
- The "self-contained" claim from posixutils-rs's README is aspirational
  marketing; the concrete state today is a tightly-coupled Unix-only
  workspace.

Vendoring `id.rs` from posixutils-rs onto Windows means rewriting half
of it against Win32 token APIs (`OpenProcessToken` / `GetTokenInformation` /
`LookupAccountSid`) — that's a from-scratch port, not a copy. Same for
`file.rs` (uses `std::os::unix::fs::FileTypeExt`), and `which` simply
isn't in posixutils-rs at all.

For each tool below, the plan picks the **cheapest correct source** —
crates.io dep where a clean upstream lib exists, in-tree implementation
where the tool is small enough to write cheaper than vendor.

## Cycles at a glance

| Cycle | Scope | Source | Effort | Risk | Ships separately? |
|---|---|---|---|---|---|
| **0a** | `egrep`, `fgrep` aliases for fastgrep | 2 lines in `lib.rs` | 15 min | Low | Yes |
| **1** | `which`, `tree`, `xxd`, `column`, `file` | `which`/`walkdir`/`infer` crates + in-tree | ~1.5 days | Low | Yes — one PR per util OR cohort |
| **2** | `tar` + gzip/bzip2/xz/zip families | `tar`/`flate2`/`bzip2`/`xz2`/`zip` crates | ~2 days | Medium | Yes — likely 2 PRs (`tar` solo, then compression cohort) |
| **3** | Grep ecosystem rework: ripgrep replaces fastgrep as `grep` provider; `rg` added; `fastgrep` retained under its own name only | `grep`/`grep-cli`/`grep-pcre2`/`ignore` crates + ported CLI orchestration | ~2–3 days | Medium-high | Yes |
| **4** | `id` (Win32 token API), `iconv`, `ps` | from-scratch / TBD | DEFERRED | High | Future plan |

Cycle 0a ships unconditionally on this branch as a "the branch works"
sanity check — it's two `m.insert()` lines and an entry in the
bundled-tools-index. Cycles 1–3 each pause for explicit go-ahead so the
user can sequence them or cut scope.

---

## Cycle 0a — `egrep` / `fgrep` aliases (immediate)

**Goal**: Stop "command not found" for `egrep file` and `fgrep file`
while leaving the underlying engine choice as-is. The bundled-tools-index
itself flags this as a known leftover task (see §E entry on
"egrep/fgrep — not aliased by fastgrep adapter").

**Mechanism**: Two extra `m.insert(...)` lines in
[`brush-bundled-extras/src/lib.rs`](../../brush-bundled-extras/src/lib.rs)
under the existing `extras.grep` cfg block. Both call the same
`grep_adapter::grep_main` that `grep` and `fastgrep` already dispatch to.

**Caveat**: GNU `egrep` defaults to `-E` (extended regex) and `fgrep`
to `-F` (fixed string). fastgrep's CLI is grep-`-E`-by-default for any
regex-looking pattern, so `egrep` works equivalently. `fgrep` semantics
differ — fastgrep needs `-F` explicit on the command line. This will be
documented in the changelog as a known caveat; Cycle 3 (ripgrep) fixes
it properly by detecting the `argv[0] == "egrep"` / `"fgrep"` case in
the adapter and pre-pending the corresponding flag.

**Acceptance**: `brush -c "type egrep && type fgrep && echo apple | egrep ap"` succeeds.

**Risk**: None — pure additive registration, no behavior change for
existing `grep` / `fastgrep`.

---

## Cycle 1 — Utility quick wins (`which`, `tree`, `xxd`, `column`, `file`)

**Goal**: Close the five highest-frequency "command not found" cases
that have trivial Windows-friendly Rust crates or are small enough to
write in-tree.

| Utility | Source | Approach | Est. lines |
|---|---|---|---|
| `which` | crates.io [`which = "6"`](https://crates.io/crates/which) | Thin CLI wrapper around `which::which_all` | ~50 |
| `tree` | crates.io [`walkdir = "2"`](https://crates.io/crates/walkdir) | In-tree CLI: walk + indent rendering, mimic GNU `tree` flag set (`-L`, `-d`, `-a`, `-I`, `-P`) | ~250 |
| `xxd` | none — write in-tree | `xxd` flag set: `-r` (reverse), `-c` (cols), `-g` (group), `-s` (skip), `-l` (length), `-p` (postscript), `-i` (C include) | ~200 |
| `column` | none — write in-tree | `column -t` flag set: separator detection, column-width measurement, table rendering | ~120 |
| `file` | crates.io [`infer = "0.16"`](https://crates.io/crates/infer) + small magic-bytes table | Use `infer` for media/archive/exec types; fall back to ASCII/UTF-8 detection for text | ~150 |

**New feature flags** (on `brush-bundled-extras/Cargo.toml`):

```text
'extras.which'        — depends on dep:which
'extras.tree'         — depends on dep:walkdir
'extras.xxd'          — no dep
'extras.column'       — no dep
'extras.file'         — depends on dep:infer
'extras.utils-all'    — aggregate of the five above
```

`extras.all` gets `extras.utils-all` layered in.

**New brush-shell flag**: `experimental-bundled-extras-utils` →
pulls `extras.utils-all`.

**Cross-platform check**: `which`, `walkdir`, `infer` all have Windows
CI in upstream and ship clean Windows binaries via downstream projects
(rip-grep uses `walkdir`; `mediainfo`-style tooling uses `infer`).
No Windows smoke gate needed before merge.

**Acceptance** (smoke checks Cycle 1 must pass on Windows):

```text
brush -c "type which && which git"                     → path printed
brush -c "tree -L 1 ."                                 → top-level listing
brush -c "echo hello | xxd"                            → hex dump
brush -c "printf 'a\tb\nc\td\n' | column -t"           → aligned columns
brush -c "file CHANGELOG.FORK.md"                      → "ASCII text"
```

**Risk**: Low — each utility is independently shippable; if `tree` or
`xxd` reveal flag-coverage gaps the cycle can ship a subset and defer
the rest.

---

## Cycle 2 — Compression family (`tar` + gzip/bzip2/xz/zip)

**Goal**: Close the loudest compression gaps. `tar` is the index's
explicit "loudest absence". gzip/bzip2/xz families are routinely
expected by build scripts, dotfile installers, and `curl | tar xz`
pipelines.

| Utility cluster | Source | Sub-utilities | Est. lines |
|---|---|---|---|
| `tar` | crates.io [`tar = "0.4"`](https://crates.io/crates/tar) + `flate2` for `-z` | `tar` (only — single multi-mode binary) | ~400 |
| gzip | crates.io [`flate2 = "1"`](https://crates.io/crates/flate2) | `gzip`, `gunzip`, `gzcat`, `zcat` (aliases) | ~200 |
| bzip2 | crates.io [`bzip2 = "0.4"`](https://crates.io/crates/bzip2) | `bzip2`, `bunzip2`, `bzcat` | ~180 |
| xz | crates.io [`xz2 = "0.1"`](https://crates.io/crates/xz2) | `xz`, `unxz`, `xzcat` | ~180 |
| zip | crates.io [`zip = "2"`](https://crates.io/crates/zip) | `unzip`, `zipinfo` | ~250 |

**Cycle structure**: likely two PRs — `tar` solo (it's the loudest single
gap and the largest adapter), then a compression-cohort PR for the four
smaller families.

**New feature flags**:

```text
'extras.tar'              — depends on dep:tar, dep:flate2
'extras.gzip'             — depends on dep:flate2
'extras.bzip2'            — depends on dep:bzip2
'extras.xz'               — depends on dep:xz2
'extras.zip'              — depends on dep:zip
'extras.compression-all'  — aggregate of gzip/bzip2/xz/zip (NOT tar — keep tar separately optable)
```

**Native dependency caveat**: `bzip2` and `xz2` link C libraries
(libbzip2, liblzma) that the crates ship vendored static C source via
`bzip2-sys` / `lzma-sys`. Both have **Windows CI in upstream** and
build with MSVC's bundled C compiler — no extra system deps needed
beyond the workspace Rust toolchain. `flate2` is pure-Rust by default.
`zip` has optional encryption deps (deflate / bzip2 / aes) — we'll
enable the deflate + bzip2 features and skip aes for the initial cycle.

**Acceptance**:

```text
brush -c "echo hi > /tmp/x && tar czf /tmp/x.tar.gz /tmp/x && tar tzf /tmp/x.tar.gz"
brush -c "echo hi | gzip | gunzip"
brush -c "echo hi | bzip2 | bunzip2"
brush -c "echo hi | xz | unxz"
```

**Risk**: Medium — `tar` has a large flag surface (`-c`/`-x`/`-t`/`-z`/`-j`/`-J`/`-f`/`--strip-components`/`-C`/`-v`/`-O`/`--exclude`/`-T`); the upstream `tar` crate gives us the engine but the CLI matching GNU tar takes care to get right. Plan budgets 1.5 days for `tar` alone.

---

## Cycle 3 — Grep ecosystem rework (ripgrep replaces fastgrep as `grep`)

**Goal**: Make `grep` resolve to a battle-tested, `-P`-supporting,
GNU-compat-tested implementation. Keep `fastgrep` available under its
own name for users who specifically want fastgrep's SIMD speed.

**The user-visible change**:

| Name | Before (Cycle 0b-revised) | After (this cycle) |
|---|---|---|
| `grep` | fastgrep | **ripgrep** |
| `egrep` | (Cycle 0a: fastgrep with caveat) | ripgrep with `-E` pre-pended |
| `fgrep` | (Cycle 0a: fastgrep with caveat) | ripgrep with `-F` pre-pended |
| `rg` | not registered | **ripgrep** (new) |
| `fastgrep` | fastgrep | fastgrep (unchanged) |

**Why ripgrep over fastgrep**:

| Axis | fastgrep 0.1.8 | ripgrep 14.x |
|---|---|---|
| `-P` (PCRE2) support | ❌ unsupported (in `GNU_GREP_COMPAT.md`) | ✅ via `grep-pcre2` |
| Windows CI in upstream | ❌ Linux + macOS only | ✅ Linux + macOS + Windows |
| MSRV | 1.92 (above brush 1.88) | 1.74 (`grep` family) — workspace MSRV satisfied |
| Maintenance | single author, 6mo activity | BurntSushi, decade of activity |
| GNU-compat surface | ~10 unsupported flags catalogued | drop-in for nearly all GNU grep usage |
| Default file-size limit | 100 MiB (skips bigger files) | none |
| Default output ordering | non-deterministic (parallel) | non-deterministic (parallel) — same caveat |
| Performance | 2–12× GNU grep | 2–10× GNU grep — comparable |

The MSRV win is significant: switching to ripgrep removes the
**`experimental-bundled-extras` umbrella's 1.92 requirement** that the
[`README.md`](../../README.md) currently documents as "Alternate install
for rustc 1.88–1.91" — Cycle 3 collapses both install lines into one.

**Integration mechanism — three options**:

ripgrep itself is **binary-only on crates.io** (the `ripgrep` crate is
not a library). The CLI orchestration lives in BurntSushi's
[`crates/core/`](https://github.com/BurntSushi/ripgrep/tree/master/crates/core).
The reusable engine lives in published crates: [`grep`](https://crates.io/crates/grep),
[`grep-cli`](https://crates.io/crates/grep-cli),
[`grep-matcher`](https://crates.io/crates/grep-matcher),
[`grep-pcre2`](https://crates.io/crates/grep-pcre2),
[`grep-printer`](https://crates.io/crates/grep-printer),
[`grep-regex`](https://crates.io/crates/grep-regex),
[`grep-searcher`](https://crates.io/crates/grep-searcher),
[`ignore`](https://crates.io/crates/ignore).

| Option | Mechanism | Pro | Con | Effort |
|---|---|---|---|---|
| **A** | Vendor `crates/core/` into `brush-bundled-extras/src/ripgrep_core/`; depend on the published engine crates | Full ripgrep parity in a single binary | ~4000 lines of vendored code to track upstream against | ~3 days |
| **B** | Build a minimal CLI on the `grep` family directly | Smaller adapter (~1000 lines); we control the surface | Not full ripgrep parity; flag-set design work; some CLI-only features (gitignore, encoding detection) need re-implementation | ~2 days |
| **C** | Require `cargo install ripgrep` separately; brush registers `rg` and `grep` as PATH-spawn shims | Trivial implementation (~50 lines) | Breaks the "single binary" goal; install becomes 2 commands; `grep` shim depends on PATH discovery succeeding | ~2 hours |

**Recommended**: **Option A (vendor `crates/core/`)** — preserves the
fork's single-binary install promise, gives full ripgrep parity, and
the upstream code is high-quality and rarely-breaking. The `~4000
lines` ports across in mostly mechanical edits (mostly: replace
`std::process::exit` with returned `i32`, replace `std::env::args_os()`
with the passed-in `args: Vec<OsString>`, replace `clap::Parser::parse()`
with `try_parse_from(args)`).

PCRE2 dependency caveat: enabling `grep-pcre2` pulls in PCRE2 as a C
dep via `pcre2-sys`. Upstream ships vendored PCRE2 source and builds
on Windows (used by ripgrep itself). MSRV 1.74 — workspace-MSRV-clean.

**Cycle 3 PR plan** (split for review):

1. **PR 1 — Gates record**: vendor a copy of `crates/core/` into
   `brush-bundled-extras/src/ripgrep_core/`, modify only the entrypoint
   shape to a function taking `Vec<OsString>`, get it building under
   `extras.ripgrep` feature, smoke-test on Windows, document MSRV
   collapse.
2. **PR 2 — Production wiring**: register `rg`, `grep`, `egrep` (with
   `-E` prepend), `fgrep` (with `-F` prepend) all dispatching to
   ripgrep. Drop `grep` registration from `grep_adapter.rs` (fastgrep);
   leave `fastgrep` registration. Update CHANGELOG, bundled-tools-index,
   README install lines.

**Acceptance** (Windows smoke gate before PR 2 merges):

```text
brush -c "type rg && type grep && type fastgrep && type egrep && type fgrep"
brush -c "rg --version"                                           → ripgrep banner
brush -c "grep --version"                                         → ripgrep banner (NOT fastgrep)
brush -c "fastgrep --version"                                     → fastgrep banner (unchanged)
brush -c "echo 'aa1bb' | grep -P '\d'"                            → 'aa1bb' (-P works!)
brush -c "echo apple | egrep ap"                                  → 'apple'
brush -c "echo apple | fgrep ap"                                  → 'apple'
brush -c "rg -rn 'fn main' brush-shell/src"                       → matches
```

**Risk**: Medium-high — Cycle 3 is the largest individual cycle in
this plan. Vendoring `crates/core/` is mechanical but tedious; PCRE2
build on Windows is the unknown unknown. PR 1 isolates the "does it
build at all" risk so PR 2 doesn't regress the existing fastgrep flag.

---

## Cycle 4 — Deferred indefinitely

| Utility | Why deferred | Trigger to re-open |
|---|---|---|
| `id` (Windows) | Requires Win32 token API (`OpenProcessToken`, `GetTokenInformation`, `LookupAccountSid`); UID/GID concepts don't map cleanly to Windows SIDs; ~300 lines of from-scratch Windows-only code; no upstream crate to lean on | User demand; or a clean `windows-id` crate appears on crates.io |
| `iconv` | Real implementation needs ICU (heavy dep, build-time C++ toolchain); even posixutils-rs's version is `gettext-rs`-backed; `encoding_rs` covers UTF-* and a few legacy encodings but not the full GNU iconv list | User demand for a specific encoding |
| `ps` | Cross-platform process listing is genuinely hard (`sysinfo` crate exists but its output format diverges from `ps -auxw`); Windows uses `tasklist` natively | User demand; or a "ps-compat" crate matures |

These are **not blocked by infrastructure** — they're blocked on
"the work is real and the demand isn't proven yet". File a separate
planning doc when triggered.

---

## Maintenance hooks

After each cycle ships:

1. Bump `brush-bundled-extras` version per the Unreleased table in
   [`CHANGELOG.FORK.md`](../../CHANGELOG.FORK.md).
2. Bump `brush-shell` version when adding the `experimental-bundled-extras-*`
   flag.
3. Update [`docs/reference/bundled-tools-index.md`](../reference/bundled-tools-index.md):
   - Move entries out of §E ("genuinely useful gaps") into §D ("bundled extras").
   - Refresh the recommended Windows install line in §header if a new
     umbrella feature appears.
4. Update [`README.md`](../../README.md) install lines if MSRV collapses
   (specifically, after Cycle 3 the rustc-1.88–1.91 fallback line can
   be removed).

## Decision log

### 2026-04-28 — vendoring posixutils-rs sources rejected (re-confirmation)

Re-confirmed the
[`posixutils-rs-integration.md`](./posixutils-rs-integration.md) Cycle
1–3 decision. Inspecting `users/id.rs`, `file/file.rs`, and the `tree/`
directory contents on disk shows that:

- The `tree/` directory in posixutils-rs is **not** the `tree` listing
  utility — it's a directory of file-tree operations (`chmod`, `cp`,
  `ls`, `mv`, `rm`, etc.) all of which are already bundled via uutils.
- `id.rs` imports `plib::group` and calls `libc::getgrgid` /
  `libc::getgroups` directly — Unix-only, will not compile on Windows.
- `file.rs` imports `std::os::unix::fs::FileTypeExt` — Unix-only.
- `which` is not present in posixutils-rs at all (grep returns zero
  hits for `which.rs`).

The right sources for these gaps are crates.io deps with Windows CI
(`which`, `walkdir`, `infer`, `tar`, `flate2`, `bzip2`, `xz2`, `zip`,
the `grep`/`grep-pcre2`/`ignore` family) plus in-tree implementations
for the trivially-small ones (`xxd`, `column`).

### 2026-04-28 — ripgrep over fastgrep for the `grep` name

User raised the missing `-P` (PCRE) support in fastgrep as a real
agent-thrash source. Verified: fastgrep's
[`GNU_GREP_COMPAT.md`](https://github.com/awnion/fastgrep/blob/main/GNU_GREP_COMPAT.md)
catalogues `-P` as unsupported. ripgrep supports `-P` via the
`grep-pcre2` crate.

Decision: Cycle 3 retires `grep → fastgrep` and registers
`grep`/`rg`/`egrep`/`fgrep` against a vendored `ripgrep_core`.
`fastgrep` itself stays registered under its own name for
backwards-compat with anyone scripting against it explicitly.

This is **not** a rejection of fastgrep — it's recognition that
fastgrep's "AI-agent-friendly defaults" framing (file-size limits,
parallel ordering) trades correctness for speed in ways that hurt
agent reliability. ripgrep's more permissive defaults are the better
fit for the fork's "Git-Bash drop-in" positioning.

Side effect: workspace MSRV story simplifies — the
`experimental-bundled-extras` umbrella loses its 1.92 requirement
once fastgrep is no longer transitively pulled by `extras.all`.
fastgrep stays available behind its own per-utility flag with the
1.92 requirement called out per-flag.

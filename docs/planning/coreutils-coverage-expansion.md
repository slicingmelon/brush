# Bundled Utilities — Coverage Expansion (coreutils + sibling crates)

> **Status**: planning · **Created**: 2026-04-25 · **Owner**: @slicingmelon
> **Tracks**: gap between current `brush-coreutils-builtins` and a "POSIX-class
> shell environment" expectation; missing utilities reported in bash-testing on
> 2026-04-25 (notably `id`, plus `find`, `xargs`, `diff`, `cmp`, `grep`, `sed`,
> `awk`).

## TL;DR

`brush-coreutils-builtins` ships 80 uutils coreutils. Real-world bash workflows
expect ~25 more utilities that fall into four categories:

1. **Already in uutils/coreutils, just not enabled here.** `id`, `stat`,
   `timeout`, `chmod`, `chown`, `groups`, `nice`, `nohup`, `who`, `users`,
   `tty`, `mkfifo`, `mknod`, `pinky`, `logname`, `pathchk`, `stdbuf`, `stty`,
   `install`, `chgrp`, `kill`, `hashsum`. Adding these is mostly a Cargo
   feature/registry edit per utility, gated for Windows where applicable.
2. **In sibling uutils repos** (separate crates, not currently in our
   workspace): `find`, `xargs`, `locate`, `updatedb` from
   [`uutils/findutils`](https://github.com/uutils/findutils); `diff`, `cmp`,
   `diff3`, `sdiff` from [`uutils/diffutils`](https://github.com/uutils/diffutils);
   `ps`, `top`, `free`, `uptime`, `vmstat`, `watch`, `pgrep`, `pkill` from
   [`uutils/procps`](https://github.com/uutils/procps).
3. **No mature Rust port:** `grep`, `sed`, `awk`. These are not in the uutils
   ecosystem at usable maturity. Options are tracked but deferred — see
   §Cycle 5.
4. **Already a brush native builtin:** `kill` (no action needed); `printf`,
   `echo`, `pwd`, `test`, `true`, `false` are deduplicated by the
   `register_builtin_if_unset` ordering at
   [`bundled.rs:292`](../../brush-shell/src/bundled.rs#L292) — brush's native
   wins on conflict, which is the right precedence (shell-side parsing,
   trap state, etc. live there).

The plan: **four PDCA cycles + one research cycle**, each shippable
independently behind feature flags.

| Cycle | Scope | Source | Priority | Effort | Risk |
|---|---|---|---|---|---|
| 1 | Add missing utilities from existing uutils coreutils dep | `uutils/coreutils` 0.8.0 (already pinned) | High — fixes `id` etc. | 0.5–1 day | Low |
| 2 | Integrate `uutils/findutils` (`find`, `xargs`) | new crate dep | High | 1–2 days | Medium |
| 3 | Integrate `uutils/diffutils` (`diff`, `cmp`) | new crate dep | Medium | 0.5–1 day | Low |
| 4 | Integrate `uutils/procps` (`ps`, `uptime`, `top`, `free`, ...) | new crate dep | Medium-Low | 1–2 days | Medium (Windows surface area) |
| 5 | Research: `grep` / `sed` / `awk` survey | investigation only | Low (unblock, don't ship) | 0.5 day | N/A |

Combined Cycles 1–4: **3–6 days**, mostly mechanical wiring once Cycle 1
establishes the pattern. None of these cycles depend on the
bundled-coreutils-pipelines work — they can land before, after, or in parallel
with Cycle 2 of that plan.

---

## A3 — Problem Frame

### 1. Background

`brush-coreutils-builtins` was scoped at MVP to ship the most commonly used
text-processing utilities and a coverage of POSIX checksum/encoding tools.
80 utilities is a lot, but real-world shell scripting and Claude Code's Bash
tool routinely shell out to commands that aren't in that set.

Concretely, in 2026-04-25 testing on this branch:
- `brush -c 'id'` → `command not found`
- `brush -c 'find . -type f'` → `command not found`
- `brush -c 'xargs -n1 ...'` → `command not found`
- `brush -c 'cmp a b'` → `command not found`
- `brush -c 'grep foo'` → `command not found`

These are **not bugs** — they're absentees. The fork's value proposition as a
Git Bash replacement on Windows is materially weaker if standard utilities
fall through to "command not found". Compare GnuWin32 or Git Bash's MSYS2
runtime: every utility above is present.

### 2. Current Condition

#### 2a. What's bundled today (80 utilities)

Source: [`brush-coreutils-builtins/Cargo.toml`](../../brush-coreutils-builtins/Cargo.toml) feature
list and [`brush-coreutils-builtins/src/lib.rs`](../../brush-coreutils-builtins/src/lib.rs)
registry.

Files-and-text: `arch`, `base32`, `base64`, `basename`, `basenc`, `cat`,
`cksum`, `b2sum`, `md5sum`, `sha1sum`, `sha224sum`, `sha256sum`, `sha384sum`,
`sha512sum`, `comm`, `cp`, `csplit`, `cut`, `date`, `dd`, `df`, `dir`,
`dircolors`, `dirname`, `du`, `echo`, `env`, `expand`, `expr`, `factor`,
`false`, `fmt`, `fold`, `head`, `hostname`, `join`, `link`, `ln`, `ls`,
`mkdir`, `mktemp`, `more`, `mv`, `nl`, `nproc`, `numfmt`, `od`, `paste`,
`pr`, `printenv`, `printf`, `ptx`, `pwd`, `readlink`, `realpath`, `rm`,
`rmdir`, `seq`, `shred`, `shuf`, `sleep`, `sort`, `split`, `sum`, `sync`,
`tac`, `tail`, `tee`, `test`, `touch`, `tr`, `true`, `truncate`, `tsort`,
`uname`, `unexpand`, `uniq`, `unlink`, `vdir`, `wc`, `whoami`, `yes`.

#### 2b. What's missing from uutils/coreutils 0.8.0 (already a dep, just not feature-flagged)

> ⚠️ **Provisional.** This table was assembled from prior knowledge of GNU coreutils
> + uutils, *not* from the actual 0.8.0 manifest. **Phase 0** (below) verifies it
> against upstream before any code change. Effort estimates and feature lists
> downstream of this table may shift after verification — flagged as a known
> pre-plan unknown rather than discovered mid-implementation.

Suspected-missing (each would require a `coreutils.<name>` feature + adapter, *if confirmed by Phase 0*):

| Utility | POSIX class | Windows-portable? | Notes |
|---|---|---|---|
| `id` | user info | yes | high-value; user explicitly cited |
| `groups` | user info | yes | natural pair with `id` |
| `logname` | user info | unix-only | uutils' own platform gate likely |
| `tty` | user info | unix-only | needs `/dev/tty` notion |
| `who` | user info | unix-only | utmp |
| `users` | user info | unix-only | utmp |
| `pinky` | user info | unix-only | utmp |
| `whoami` | user info | yes | **already bundled** — no action |
| `hostname` | system | yes | **already bundled** — no action |
| `stat` | file info | yes | high-value |
| `chmod` | perms | unix-only on Windows it's a no-op-ish; uutils gates |
| `chown` | perms | unix-only |  |
| `chgrp` | perms | unix-only |  |
| `chcon` | selinux | linux-only | low priority for our user base |
| `runcon` | selinux | linux-only | low priority |
| `chroot` | unix | unix-only | rarely useful in shell |
| `install` | files | yes | useful for build scripts |
| `mkfifo` | files | unix-only |  |
| `mknod` | files | unix-only |  |
| `nice` | proc | unix-only |  |
| `nohup` | proc | unix-only |  |
| `timeout` | proc | yes | **high-value**; common in CI scripts |
| `kill` | proc | yes | **already a brush native builtin** — skip (let native win) |
| `pathchk` | misc | yes | rarely used but cheap |
| `stdbuf` | proc | linux-only | uses `LD_PRELOAD` mechanism |
| `stty` | terminal | unix-only |  |
| `cksum` | checksum | yes | **already bundled** |
| `hashsum` | checksum | yes | uutils combined-hash dispatcher; cheap |

**Action set for Cycle 1**: add `id`, `groups`, `stat`, `timeout`, `install`,
`pathchk`, `hashsum` unconditionally; add `chmod`, `chown`, `chgrp`,
`logname`, `nice`, `nohup`, `tty`, `who`, `users`, `pinky`, `mkfifo`,
`mknod`, `stty`, `chroot`, `chcon`, `runcon`, `stdbuf` as `cfg(unix)`-gated
features. Skip `kill` (native).

#### 2c. What needs sibling uutils crates

| Source repo | Utilities | Crate name | Status check needed |
|---|---|---|---|
| [`uutils/findutils`](https://github.com/uutils/findutils) | `find`, `xargs`, `locate`, `updatedb` | `uu_find`, `uu_xargs` (verify) | Confirm crates.io publish + `uumain` API match |
| [`uutils/diffutils`](https://github.com/uutils/diffutils) | `diff`, `cmp`, `diff3`, `sdiff` | `uu_diff`, `uu_cmp` (verify) | Same |
| [`uutils/procps`](https://github.com/uutils/procps) | `ps`, `top`, `free`, `uptime`, `vmstat`, `watch`, `pgrep`, `pkill`, `pmap`, `slabtop`, `tload`, `pwdx`, `snice`, `sysctl`, `w` | `uu_ps`, `uu_uptime`, ... | Many are Linux-only; needs auditing |

Each sibling repo has its own crate naming, MSRV, and `uumain` shape that we
need to verify before committing. The adapter macro at
[`brush-coreutils-builtins/src/lib.rs:91-104`](../../brush-coreutils-builtins/src/lib.rs#L91-L104)
assumes `uucore::bin!`-shape utilities; sibling repos largely follow the same
pattern but it's not a guarantee.

#### 2d. What's not in the uutils ecosystem at usable maturity

`grep`, `sed`, `awk` are absent. Options:

- **`grep`**: There is `uutils/coreutils-grep` (part of uutils umbrella but
  separate); status unverified. Independently, the `ripgrep` crate (`rg`) is
  a mature Rust grep but its argv shape is not POSIX-grep — it's `rg`. We'd
  need either to wait for a uutils grep, or ship a custom `grep` shim that
  translates flags to ripgrep. Not trivial.
- **`sed`**: No mature Rust port. `sd` exists but is a different language
  (uses regex-replace, not full sed-script). Not a drop-in.
- **`awk`**: `frawk` exists as a Rust awk-like tool. Performance-focused but
  not feature-complete with gawk. Not a drop-in for arbitrary awk scripts.

**Recommendation**: defer all three to Cycle 5 (research-only). Document the
gap in CHANGELOG as a known limitation. Users who need them have working
fallbacks (Git Bash's MSYS2 grep/sed/awk on Windows; system grep/sed/awk on
Unix; brush will fall through to PATH lookup since these aren't shadowed by
the bundled set).

#### 2e. Public-API and binary-size considerations

- **Binary size**: TBD pending measurement. Each `uu_*` crate adds *some*
  weight to the brush binary, and 80 utilities is already material. The
  rough order of magnitude for a stripped uutils-coreutils multi-call
  binary is ~6–7 MB; our build is unstripped and embedded, so ours is
  larger but the same order. Adding 25+ utilities and three sibling
  crates will grow this further but the increment is unknown without
  data. **Cycle 1 DoD includes a `cargo build --release` size
  measurement** (with and without `coreutils.all`) so future cycles have
  ground truth. Cargo features keep all additions opt-in. The `bash`
  binary alias inherits whatever `brush` ships.
- **No public-API change**: all additions are inside
  `brush-coreutils-builtins`, behind feature flags. The registry function's
  signature is unchanged. No SemVer impact on `brush-core` or
  `brush-shell`.
- **Windows compatibility**: uutils utilities self-gate Windows behavior
  internally (most return ENOSYS or a meaningful error on platforms they
  don't support). For utilities that genuinely don't compile on Windows
  (`uu_chmod`'s symlink-mode path, `uu_logname` reading utmp, etc.), the
  feature must be `cfg(unix)`-gated in our `Cargo.toml`. Otherwise
  Windows builds break.

### 3. Goal

- A `brush -c '<cmd>'` invocation for any of the 25 missing common-shell
  utilities **resolves** to a bundled implementation (when the feature is
  enabled) rather than `command not found`.
- `coreutils.all` aggregate feature continues to mean "everything supported
  on this platform" — Windows builds stay green even when the feature is
  on.
- No regression in binary size when **none** of the new features is
  enabled (default still empty).
- Sibling-crate utilities (`find`, `xargs`, `diff`, `cmp`, `ps`, ...) are
  available behind their own per-crate feature umbrella feature
  (`findutils.all`, `diffutils.all`, `procps.all`), gated separately from
  `coreutils.all` so consumers can pick subsets.
- `grep`/`sed`/`awk` gap is **explicitly documented** as a known limitation
  with researched workaround paths.

### 4. Root cause — why these are missing today

1. MVP scope of `brush-coreutils-builtins`: shipped the most commonly used
   subset, not the full uutils set. This is fine — but the subset hasn't
   been revisited since.
2. No automated coverage check: nothing today compares our bundled set
   against the upstream uutils list. New uutils releases can add utilities
   we'd want, and we'd never notice. Cycle 1's deliverable should include
   a test or CI step that flags drift.
3. Sibling crates were never integrated because uutils' workspace splits
   them across separate repos with separate release cadences. Pulling them
   in is a workspace-Cargo.toml decision, not a code one.

### 5. Out of scope

- **Not** rewriting any utility from scratch. We use upstream uutils unmodified.
- **Not** writing a brush-native `grep`/`sed`/`awk` — too big, separate epic.
- **Not** changing the bundled-dispatch protocol or the shim mechanism. The
  `register!` macro and `--invoke-bundled` flag stay as-is. New utilities
  plug into the existing pattern.
- **Not** revisiting brush's *native* builtins (the things in
  `brush-builtins/`). Those are shell-internal (`cd`, `eval`, `trap`, ...)
  and outside this plan's scope.
- **Not** doing dependency-graph hygiene of uutils transitive deps in this
  plan. Track separately if it becomes a binary-size concern.

---

## Phase 0 — Verify §2b against upstream (pre-plan, hard gate for Cycle 1)

Before Cycle 1 starts, fetch the actual uutils/coreutils 0.8.0 utility manifest
and reconcile §2b. Without this, every cycle's effort estimate and feature
list is fiction.

**Deliverables**:
1. Fetch `Cargo.toml` from
   <https://github.com/uutils/coreutils/blob/0.8.0/Cargo.toml> (or the
   crates.io page for `coreutils 0.8.0`) and extract the full `feat_*`
   feature list — that's the canonical "all utilities at this version".
2. Diff against §2b. Flag:
   - Utilities present upstream but missing from §2b (we'd be unaware of
     them — likely additions to §2b's "to-add" set).
   - Utilities in §2b's "to-add" set that *don't exist* in uutils 0.8.0
     (we'd ship a dead feature flag — remove from §2b).
   - Aliases like `[` (alias for `test`) — confirm whether enabling
     `coreutils.test` exposes `[` automatically or requires its own
     `register!` line.
   - Anything ambiguous (e.g., `b3sum` may or may not be in 0.8.0).
3. Update §2b in place; record a Decision Log entry summarizing what
   changed.

**Output**: an updated §2b that is the ground truth for the rest of the
plan. **No code change in Phase 0.**

**Effort**: 0.5–1 hour (a single `WebFetch` plus a diff).

## PDCA Cycle 1 — Add missing utilities from existing uutils/coreutils dep

### Plan

**Hypothesis**: Every absentee in §2b can be enabled by (a) adding the
`uu_<name> = { version = "0.8.0", optional = true }` line to
`brush-coreutils-builtins/Cargo.toml`, (b) adding a `coreutils.<name>`
feature mapping it to `dep:uu_<name>`, (c) adding a `register!()` line in
`bundled_commands()`, (d) adding the new feature to `coreutils.all` (or a
`cfg(unix)`-only `coreutils.all-unix` aggregate).

**Baseline**: Inventory the upstream uutils/coreutils 0.8.0 utility list
(scrape from their `Cargo.toml`'s `feat_*` features or crates.io published
list of `uu_*` crates). Diff against §2b above to confirm completeness.
**Don't trust the table — verify against upstream.**

**Change scope**:
1. Verify the upstream list. Update §2b if we missed any utility.
2. For each utility: 4-line edit per row in Cargo.toml + 1 line in lib.rs.
3. Windows-only: split `coreutils.all` into `coreutils.all` (cross-platform)
   and `coreutils.all-unix` (adds the Unix-only set on top). Or use
   `cfg(unix)` in the registration macro itself; need to check the
   registration macro accepts cfg gating cleanly.
4. New CI smoke test: `brush -c 'id'` → exit 0 on Linux/Windows; `brush -c 'chmod --help'` → exit 0 on Linux only.

**Success criteria**:
- Each new feature compiles on Windows in isolation (or is correctly
  `cfg(unix)`-gated so it doesn't try to).
- `coreutils.all` builds on Linux + macOS + Windows.
- `brush -c 'id'` produces output (no `command not found`) on all three platforms.
- `brush -c 'timeout 1 sleep 5'` exits 124 on all three platforms.
- Binary size delta < 1 MB per platform with the new utilities enabled.
- No new clippy lints from the additions (the existing lint config in the
  workspace catches uutils-side issues already).

**Auto-coverage check**: add `xtask coverage-check` (or extend existing
xtask) that downloads the upstream uutils manifest and reports any drift.
Run in CI on a weekly schedule; soft-fail (warn, don't block).

### Do

Implementation sequence (one PR per phase, reviewable):

1. **Phase 1.1 — verify upstream list.** Read uutils/coreutils 0.8.0
   `Cargo.toml` directly from crates.io or GitHub. Update §2b. Capture the
   final list of utilities to add.
2. **Phase 1.2 — extend Cargo.toml.** Add `uu_<name>` deps and
   `coreutils.<name>` features. Group Unix-only ones under a clear header
   comment. Don't yet add to `coreutils.all`.
3. **Phase 1.3 — extend `bundled_commands()`.** Add `register!` lines.
   Gate the Unix-only ones with `#[cfg(unix)]` on the `register!`
   invocations (verify this works with the macro's `#[cfg(feature = ...)]`
   inner gating; may need tweak).
4. **Phase 1.4 — wire `coreutils.all`.** Cross-platform additions go in
   `coreutils.all`. Add a separate `coreutils.all-unix` aggregate that
   includes the Unix-only set; have `coreutils.all-unix` imply
   `coreutils.all`.
5. **Phase 1.5 — smoke tests.** Add YAML cases under
   `brush-shell/tests/cases/brush/` exercising `id`, `stat`, `timeout`,
   `chmod` (Unix-only), `groups`, `install`. Use the harness's
   `incompatible_os` to gate Unix-only tests off Windows runners.
6. **Phase 1.6 — coverage drift check.** xtask subcommand that compares
   our enabled utilities to upstream's full list and prints a diff.
7. **Phase 1.7 — CHANGELOG entry** under Unreleased / Features.

Track during Do:
- Whether any new utility fails to compile on any of the three platforms
  with the existing `uucore = "0.8.0"` pin (forcing a version bump).
- Whether any new utility's argv0 / dispatch shape diverges from the
  existing pattern (forcing macro changes).
- Binary-size delta per added utility (rough; for prioritizing what
  `coreutils.all` must include).

### Check

Measure:
- Test suite delta: number of new YAML cases.
- Per-platform `cargo build --features experimental-bundled-coreutils,coreutils.all` size.
- Smoke test results: `brush -c 'id'` etc. all exit 0.
- xtask drift report: should show 0 missing after this cycle (modulo
  intentional skips like `kill`).

Hypothesis confirmation: every utility from §2b is now resolvable in `brush -c`.

### Act

If successful: standardize and merge. The pattern (Cargo.toml + register
+ aggregate) is now the shape for Cycles 2/3/4.

If unsuccessful: most likely failure mode is a Windows compile error on
a utility we thought was portable. Fall back to `cfg(unix)` for that
utility, document in CHANGELOG, file an upstream issue if it's a bug.

---

## PDCA Cycle 2 — Integrate uutils/findutils

### Plan

**Hypothesis**: Adding `uutils/findutils` as a workspace dependency and
mirroring the registration pattern of `brush-coreutils-builtins` gives us
`find` and `xargs` (and optionally `locate`/`updatedb`) behind a
`findutils.<name>` feature umbrella.

**Pre-flight verification (do FIRST, before committing to the cycle)**:
- Does `uutils/findutils` publish `uu_find`, `uu_xargs` to crates.io? Check
  https://crates.io/crates/uu_find. Versions, MSRV, Windows compatibility.
- Does its `uumain` API match `uucore::bin!` expectations? If not, the
  adapter macro needs adjustment.
- Does it depend on a different `uucore` version than 0.8.0? If yes, we
  need to either align or accept duplicate uucore in the build.

**Architectural decision**: separate crate `brush-findutils-builtins` that
mirrors `brush-coreutils-builtins`, OR fold into `brush-coreutils-builtins`
under a `findutils.<name>` feature namespace?

The case for **separate crate** rests on *one concrete benefit*, not
hand-waved "cleanliness":

- **Tolerates `uucore` version skew.** If `uutils/findutils` pins
  `uucore = "0.9.x"` while our coreutils dep is `uucore = "0.8.0"`, Cargo
  will resolve to two `uucore` instances rather than fail. In a single
  crate, the two `uucore` deps would still coexist (Cargo allows
  semver-incompatible duplicates), but the entanglement makes diagnosing
  a feature-flag conflict harder. Separate crates put the version pin
  next to the utility it serves.
- All other rationales ("cleaner ownership", "easier upstream-tracking")
  are subjective and not load-bearing — Cargo features in one crate are
  already namespaced.

The case for **folding into one crate**:

- One `Cargo.toml` block to maintain, not three.
- One `register!` macro definition (vs duplicating it).
- One feature umbrella to expose to `brush-shell` (`coreutils.all` could
  expand to also enable findutils/diffutils via re-exports).
- The crate name "brush-coreutils-builtins" misrepresents content but
  could be renamed once (to e.g. `brush-bundled-utilities`) — one-time
  cost.

**Decision**: pick separate-crate **only if Phase 2's pre-flight reveals
real `uucore` version skew between sibling repos and 0.8.0**. If skew is
zero (all sibling repos pin `uucore = "0.8.0"`), the fold-into-one-crate
path wins by simplicity. Don't decide by default.

**Success criteria**:
- `brush -c 'find . -name "*.rs" | head'` works on all three platforms.
- `brush -c 'find . -type f | xargs wc -l'` works (multi-stage pipeline,
  exercises Cycle 2 of bundled-coreutils-pipelines plan if landed).
- `findutils.all` aggregate feature works on all three platforms.

### Do

1. Pre-flight: read `uutils/findutils` `Cargo.toml`, check crates.io.
2. New crate `brush-findutils-builtins/` mirroring the coreutils crate
   layout.
3. Wire it into `brush-shell/Cargo.toml` behind a new feature
   `experimental-bundled-findutils` (parallel to
   `experimental-bundled-coreutils`).
4. Update `brush-shell/src/bundled.rs::install_default_providers()` to
   merge the new crate's registry into the unified `BundledFn` map.
5. Smoke tests + CHANGELOG.

### Check / Act

Same shape as Cycle 1: smoke tests pass, no Windows breakage, register
patterns match. If pre-flight reveals API mismatch, escalate to root-cause
analysis before proceeding.

---

## PDCA Cycle 3 — Integrate uutils/diffutils

### Plan

Same shape as Cycle 2, applied to
[`uutils/diffutils`](https://github.com/uutils/diffutils): `diff`, `cmp`,
`diff3`, `sdiff`. Pre-flight: confirm crates.io presence and `uumain`
shape.

**High-value targets**: `cmp` (used in scripts to compare files;
0-or-1 exit semantics, simple); `diff` (universal). `diff3`/`sdiff` are
nice-to-have.

**Effort**: smaller than Cycle 2 because diffutils is more focused.

### Do / Check / Act

Mirror Cycle 2 exactly. Crate: `brush-diffutils-builtins`. Feature flag:
`experimental-bundled-diffutils`. CHANGELOG.

---

## PDCA Cycle 4 — Integrate uutils/procps

### Plan

[`uutils/procps`](https://github.com/uutils/procps) provides ~15 utilities
(`ps`, `top`, `free`, `uptime`, `vmstat`, `watch`, `pgrep`, `pkill`, ...).
Many are Linux-only by nature (`/proc` filesystem readers).

**Pre-flight risk**: unlike coreutils/findutils/diffutils, procps has the
strongest platform binding. Each utility likely needs `cfg(unix)` or
`cfg(target_os = "linux")` gating. Windows coverage may be near-zero for
this set — expect to ship `procps.all` as Linux-only with most utilities
gated off macOS too.

**Decision point during pre-flight**: if Windows support is genuinely
zero, consider deferring this entire cycle. Question: does the Git Bash
audience actually use `ps`/`top` etc.? Probably less so than `find`/`diff`.

### Do / Check / Act

Mirror Cycles 2/3. Crate: `brush-procps-builtins`. Feature flag:
`experimental-bundled-procps`. Heavy use of `cfg`.

---

## PDCA Cycle 5 — Research: grep / sed / awk options

### Plan

**Not implementation — investigation only.**

Deliverables:
1. Status survey of `uutils/grep` (or any uutils-aligned grep effort).
2. Survey of `ripgrep` as a base — feasibility of building a POSIX-grep
   shim that translates flags to `rg`. Compatibility matrix:
   `-E`/`-F`/`-i`/`-v`/`-c`/`-n`/`-l`/`-r` are common; `-A`/`-B`/`-C`
   too. What does `rg` get wrong vs. POSIX grep?
3. Sed: survey of `sd`, `sad`, `xs`. None is sed-compatible; assess what
   "sed-shaped" users actually need (s/// substitution? d? p? scripts?).
4. Awk: survey of `frawk`, `goawk` (Go), what's the user need?

**Output**: a research doc, not code. Decisions deferred to a later epic
unless something jumps out as obviously ready to ship.

### Do

Research session, ~half day. Document findings in
`docs/research/grep-sed-awk-options.md`. No code changes.

### Check / Act

Decide: ship now, defer to later, or document as "use system
grep/sed/awk on PATH" forever.

---

## Effort & Confidence Recap

| Cycle | Effort | Risk | Required for | Reversible? |
|---|---|---|---|---|
| 1 | 0.5–1 day | Low | Closing user gap (`id`, `stat`, `timeout`) | Yes |
| 2 | 1–2 days | Medium (sibling-crate API) | `find`/`xargs` parity | Yes |
| 3 | 0.5–1 day | Low | `diff`/`cmp` parity | Yes |
| 4 | 1–2 days | Medium (Windows surface) | `ps`/`top` etc. | Yes |
| 5 | 0.5 day research | None | Future grep/sed/awk decision | N/A |

Total Cycles 1–4: **3–6 days**, plus 0.5 research day. Independent of the
bundled-coreutils-pipelines plan; can run in parallel.

Recommended order: **1 → 2 → 3 → 4 → 5**. Rationale (priority by user-cited demand and shell-script frequency, *not* by diff size):
- **Cycle 1 first** — closes the user-visible `id`/`stat`/`timeout` gap with cheap edits inside an existing dep.
- **Cycle 2 second** — `find` and `xargs` appear in orders of magnitude more shell scripts than `cmp`/`diff`. The user explicitly cited findutils integration before diffutils. Sibling-crate integration risk is one-time work that benefits Cycles 3+4 too.
- **Cycle 3 third** — `diff`/`cmp` are common but not as universal as `find`/`xargs`. Smaller cycle by scope; safe slot after Cycle 2 establishes the sibling-crate pattern.
- **Cycle 4 fourth** — highest Windows-compatibility risk, narrowest-audience deliverable (`ps`/`top`/`uptime`); do last.
- **Cycle 5 anytime** — research, not blocking; slot whenever there's a gap.

(Earlier draft of this plan recommended 1 → 3 → 2; reflexion review on 2026-04-25 caught that ordering optimized for diff size over user impact and reordered.)

---

## Hard Pre-Flight Gates (before each sibling-crate cycle)

These are **gates, not open questions**. A cycle does not start until each
gate has a documented yes/no answer.

1. **`uucore` version alignment.** Inspect the sibling crate's
   `Cargo.toml` (e.g., `uutils/findutils/Cargo.toml`) and read its
   `uucore = "X.Y.Z"` pin. Compare to our `brush-coreutils-builtins`
   pin (currently `0.8.0`).
   - **Same version** → fold-into-one-crate is viable; revisit
     architectural decision favoring fold.
   - **Different semver-compatible version** (`0.8.0` vs `0.8.1`) →
     Cargo unifies; either approach works.
   - **Different semver-incompatible version** (`0.8.0` vs `0.9.0`) →
     duplicate `uucore` in the build; separate-crate is preferred to
     localize the duplication.
2. **Native-builtin collision check.** For every utility name added by a
   cycle, grep `brush-builtins/src/` for a matching native builtin. If a
   native version exists, confirm `register_builtin_if_unset` at
   [`bundled.rs:292`](../../brush-shell/src/bundled.rs#L292) keeps the
   native version winning (current behavior). Document the collision in
   the cycle's CHANGELOG.

## Open Questions (genuinely unresolved, not gates)

1. **`coreutils.all-unix` vs `cfg(unix)` in `register!`** — which is more
   ergonomic for downstream consumers? Resolve in Cycle 1 Phase 1.4.
2. **Should the `experimental-bundled-coreutils` feature be renamed?** Once
   we have `experimental-bundled-findutils`, etc., the name "coreutils"
   becomes misleading. Maybe a meta-feature
   `experimental-bundled-utilities` that turns on all four crates? Defer
   the naming bikeshed until Cycle 4 lands.
3. **`hash` shell builtin vs uutils `hashsum`** — `hash` already exists
   natively as a brush builtin (PATH cache). `hashsum` is uutils' generic
   checksum dispatcher. Names don't collide but worth a sentence in the
   CHANGELOG to avoid confusion.
4. **Auto-coverage check delivery** — xtask subcommand vs CI workflow vs
   `cargo deny`-style external tool? Resolve in Cycle 1 Phase 1.6.

## Composition with bundled-coreutils-pipelines plan

This plan is **additive** with [`bundled-coreutils-pipelines.md`](./bundled-coreutils-pipelines.md).
No code conflict; both edit different files (`brush-coreutils-builtins/`
here; `brush-shell/src/bundled.rs` and `brush-core/src/commands.rs`
there). Order-of-landing observation:

- If **pipelines Cycle 2 lands first** (parallelism), every utility
  added by *this* plan inherits parallelism on day 1.
- If **this plan's Cycle 1 lands first** (new utilities), those new
  utilities serialize in pipelines until pipelines Cycle 2 lands. They
  still *work* — just one stage at a time.

Either order is fine; reviewers should expect this. No need to gate
either plan on the other.

## Smoke-test conventions (used by all cycles)

New utility tests follow the existing `brush-shell/tests/cases/brush/`
YAML harness:

- One YAML file per utility group (e.g., `cases/brush/coreutils-id.yaml`).
- `incompatible_os: [...]` for distro-specific carve-outs.
- `incompatible_platforms: [...]` for runtime carve-outs (`wasi`, etc.).
- Cross-platform utilities: tests run everywhere; assertions on
  `expected_stdout` or `expected_exit_code`.
- Unix-only utilities: gate the *test* via `incompatible_os: ["windows"]`
  even if the *feature* is `cfg(unix)`-gated, so the test harness on a
  Windows runner skips cleanly rather than fails to find the command.

See existing examples at
[`brush-shell/tests/cases/compat/builtins/trap.yaml:369-393`](../../brush-shell/tests/cases/compat/builtins/trap.yaml#L369-L393).

---

## Definition of Done

For Phase 0 (gate for Cycle 1):
- [ ] Upstream uutils/coreutils 0.8.0 manifest fetched.
- [ ] §2b reconciled in place; Decision Log records what changed.
- [ ] `[` (test alias) registration mechanism confirmed.

For Cycle 1:
- [ ] All cross-platform additions enabled; Unix-only ones `cfg(unix)`-gated.
- [ ] `coreutils.all` and (new) `coreutils.all-unix` aggregates work on all platforms.
- [ ] Native-builtin collision check run for every new utility name.
- [ ] Smoke tests (YAML, in `brush-shell/tests/cases/brush/`) pass: `id`, `stat`, `timeout`, plus one Unix-only utility gated via `incompatible_os: ["windows"]`.
- [ ] **Binary size measured**: `cargo build --release -p brush-shell --features experimental-bundled-coreutils,coreutils.all` size recorded pre/post. Numbers in CHANGELOG.
- [ ] xtask coverage-drift check exists and shows zero drift after this cycle.
- [ ] CHANGELOG.FORK.md updated under Unreleased / Features.

For Cycle 2:
- [ ] Pre-flight verified: `uutils/findutils` is on crates.io with compatible API.
- [ ] `brush-findutils-builtins` crate exists and follows the
      `brush-coreutils-builtins` shape.
- [ ] `experimental-bundled-findutils` feature on `brush-shell` works.
- [ ] Smoke tests: `find . -name '*.rs'`, `find . -type f | xargs wc -l`.
- [ ] CHANGELOG entry.

For Cycle 3:
- [ ] Same shape as Cycle 2, for `brush-diffutils-builtins` / `experimental-bundled-diffutils`.
- [ ] Smoke tests: `diff a b`, `cmp -s a b && echo same`.

For Cycle 4:
- [ ] Same shape, for `brush-procps-builtins` / `experimental-bundled-procps`.
- [ ] Platform support honestly documented (likely Linux-mostly).
- [ ] Smoke tests gated to platforms that support each utility.

For Cycle 5:
- [ ] Research doc in `docs/research/` with concrete recommendation.
- [ ] No code changes (intentionally).

---

## Decision Log

(Append as cycles complete.)

| Date | Cycle | Decision | Evidence |
|---|---|---|---|
| 2026-04-25 | (planning, draft) | Plan drafted using PDCA. Initial order: 1 → 3 → 2 → 4 → 5. Initial separate-crate-per-sibling-repo default. | This doc, pre-amendment. |
| 2026-04-25 | (planning, reflexion) | Reflexion review scored the draft 3.05/5.0, below the 4.0 threshold. Amendments: (1) reorder to 1 → 2 → 3 → 4 → 5 — find/xargs outranks diff/cmp by user demand and shell-script frequency. (2) Add Phase 0 to verify §2b against upstream before any code change; mark §2b as provisional. (3) Replace hand-wavy "separate crate is cleaner" with a single concrete benefit (uucore version-skew tolerance) plus an explicit fold-vs-separate decision rule keyed off Phase 2 pre-flight. (4) Promote uucore-skew check from Open Question to a hard Pre-Flight gate. (5) Replace fabricated "~7 MB" binary-size claim with a Cycle 1 DoD measurement step. (6) Add native-builtin collision check as a hard Pre-Flight gate. (7) Add YAML smoke-test convention reference. (8) Add composition note with bundled-coreutils-pipelines plan. | Reflexion report 2026-04-25; this doc post-amendment. |

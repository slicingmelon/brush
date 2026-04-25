# posixutils-rs — Integration Planning

> **Status**: planning · **Created**: 2026-04-25 · **Owner**: @slicingmelon
> **Tracks**: gap-fillers (`grep`, `sed`, `awk`, `make`, `m4`, `bc`) that the
> uutils ecosystem doesn't ship today. Supersedes the indefinite-defer
> conclusion in [`docs/research/grep-sed-awk-options.md`](../research/grep-sed-awk-options.md)
> *only* if posixutils-rs becomes integration-ready.
>
> ⚠️ **Update 2026-04-25 (afternoon)** — this plan was originally framed as
> "wait for posixutils-rs upstream to expose `[lib]` + publish to crates.io".
> A second sweep of alternative sources (see new section
> [§"Alternative sources reviewed"](#alternative-sources-reviewed--2026-04-25)
> immediately below) found that:
>
> - **`sed`** can ship *now* via a clean crates.io dep on
>   [`uutils/sed`](https://github.com/uutils/sed) v0.1.1 — uucore 0.8.0 pin
>   matches ours, standard `uumain` API, zero vendoring.
> - **`awk`** can ship *now* via a clean crates.io dep on
>   [`pegasusheavy/awk-rs`](https://github.com/pegasusheavy/awk-rs) v0.1.0 —
>   public `[lib]` API, **CI matrix includes Windows**, dual MIT/Apache-2.0,
>   639 tests / 86% library coverage, Criterion benchmarks, ~80ms vs
>   gawk's ~50ms on 100k-line sum (1.6× slower than reference, acceptable).
>   Discovered in the second-pass sweep; supersedes the first-pass plan to
>   vendor awk from posix-tools-for-windows.
> - **`grep`** can ship *now* via a clean crates.io dep on
>   [`awnion/fastgrep`](https://github.com/awnion/fastgrep) v0.1.8 —
>   "drop-in replacement for GNU grep" per upstream README, **2–12× faster
>   than GNU grep** on Criterion-measured workloads, dual MIT/Apache-2.0,
>   public `[lib]` API, bin target literally named `grep`. Discovered in
>   the third-pass sweep; supersedes the second-pass plan to vendor grep
>   from posix-tools-for-windows. **One material gap**: upstream CI tests
>   only Ubuntu + macOS — Windows-buildability and Windows-correctness
>   need a brush-side smoke-test gate before ship.
> - **`m4` / `bc` / `make` / `patch`** remain blocked on posixutils-rs's
>   `plib`-shared internal dependency web; the original Cycle 1 upstream-
>   engagement plan still applies for these.
>
> The original §6a stance ("vendoring must not become the default escape
> hatch") is **softened** — selective vendoring of *self-contained* upstream
> sources is now an accepted Cycle 0 path, conditional on per-utility audit
> + test extension. See the new section for the rationale and the Decision
> Log rows dated 2026-04-25 (afternoon) and 2026-04-25 (evening).

## ▶︎ Start here (for the implementer picking this up)

**You're a Claude Code agent (or human) coming to this fresh.** Read this
section, then jump to the cycle you're executing. The doc below this
section is research-grade context — useful but not required to start
work.

### What's done already

- Cycles 1–5 of [`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md)
  shipped in v0.3.1: 26 missing coreutils + `find`/`xargs` from
  uutils/findutils via the `brush-bundled-extras` adapter pattern.
- Pipeline parallelism + pgid plumbing landed (Cycles 1–2 of
  [`bundled-coreutils-pipelines.md`](./bundled-coreutils-pipelines.md)).
- `uutils/diffutils` Cycle 3 is **deferred** awaiting upstream
  `pub mod diff;` (no action here).
- Source-evaluation sweep complete — see §"Alternative sources reviewed".

### What's next (Cycle 0a/0b/0c — execute in order)

| Order | Cycle | Add | Mechanism | Section anchor |
|---|---|---|---|---|
| 1 | **0a** | `sed` | crates.io dep on `sed = "0.1.1"` (uutils/sed). Standard `uumain` API; uucore 0.8.0 = exact match. **No MSRV friction (sed MSRV 1.88 = brush MSRV 1.88).** | [§"Cycle 0 — Quick wins"](#cycle-0--quick-wins-no-vendoring-required) |
| 2 | **0c-revised** | `awk` | crates.io dep on `awk-rs = "0.1.0"` (pegasusheavy/awk-rs). `[lib]` exposes `Lexer`/`Parser`/`Interpreter`; adapter mirrors upstream `main.rs` (~50 lines glue). **No MSRV friction (awk-rs MSRV 1.85 < brush MSRV 1.88).** CI tests Windows. | [§"Cycle 0c-revised"](#cycle-0c-revised--awk-via-cratesio-dep-on-awk-rs) |
| 3 | **0b-revised** | `grep` + `fastgrep` alias | crates.io dep on `fastgrep = "0.1.8"` (awnion/fastgrep). `[lib]` exposes building blocks; adapter mirrors `src/bin/grep.rs` (~400 lines glue). **MSRV gate first — fastgrep needs rustc ≥ 1.92, brush is 1.88.** Then **Windows smoke gate** (fastgrep CI doesn't cover Windows). If either gate fails, fall back to Cycle 0b-fallback (vendor from posix-tools-for-windows). | [§"Cycle 0b-revised"](#cycle-0b-revised--grep-via-cratesio-dep-on-awnionfastgrep) |

### How each utility plugs in

All three follow the existing `brush-bundled-extras` adapter pattern
established by `find_adapter` / `xargs_adapter` in
[`brush-bundled-extras/src/lib.rs`](../../brush-bundled-extras/src/lib.rs).
The pattern:

1. Add `<crate> = { version = "...", optional = true }` to
   [`brush-bundled-extras/Cargo.toml`](../../brush-bundled-extras/Cargo.toml).
2. Add `extras.<util>` feature mapping to `dep:<crate>`.
3. Write `<util>_adapter(args: Vec<OsString>) -> i32` in
   [`brush-bundled-extras/src/lib.rs`](../../brush-bundled-extras/src/lib.rs).
4. Register in `bundled_commands()` under the feature flag.
5. Add `experimental-bundled-extras-<source>` flag on
   [`brush-shell/Cargo.toml`](../../brush-shell/Cargo.toml) layered into
   `experimental-bundled-extras-all`.
6. CHANGELOG entry under Unreleased / Features in
   [`CHANGELOG.FORK.md`](../../CHANGELOG.FORK.md).

For Cycle 0b-revised's MSRV friction, the recommended choice is
"feature-conditional MSRV" — see Cycle 0b-revised gate 0 for the
explicit option list.

### Reference docs to read alongside this one

- [`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md) —
  Cycles 1+2 establish the `brush-bundled-extras` adapter pattern that
  Cycle 0a/0b/0c reuse. Read its "What's already in place" sections.
- [`docs/research/grep-sed-awk-options.md`](../research/grep-sed-awk-options.md) —
  the **prior** research conclusion (defer all three). **Now superseded
  by Cycle 0a/0b/0c here.** A header note in that doc points back here.
- [`CHANGELOG.FORK.md`](../../CHANGELOG.FORK.md) — see v0.3.1 entries
  for `brush-bundled-extras` precedent (find/xargs adapters) and the
  related coreutils-extras flag plumbing on `brush-shell`.

### Concrete first commit suggestions

- **Cycle 0a** (sed): one PR, ~30 min. Add `sed = "0.1.1"` dep, write
  `sed_adapter`, register, flag, smoke test (`brush -c "echo a | sed s/a/b/"`),
  CHANGELOG. Lowest-risk; ship first.
- **Cycle 0c-revised** (awk): one PR, ~1 hour. Adapter mirrors
  `pegasusheavy/awk-rs/src/main.rs::run` arg parsing; ~50 lines.
  Smoke tests in DoD section.
- **Cycle 0b-revised** (grep + fastgrep): two PRs.
  - PR 1: MSRV decision + Windows smoke gate (no production code yet,
    just a throwaway branch verifying fastgrep builds + runs on Windows).
    Document outcome in this plan's Decision Log.
  - PR 2 (only if PR 1 passes): adapter + dual-name registration +
    CHANGELOG with the three behavioral-deviation notes.

If Cycle 0b-revised PR 1 fails the Windows gate → switch to Cycle
0b-fallback (vendor from posix-tools-for-windows). The user-visible
outcome (working `grep`) is the same.

---

## TL;DR (honest framing)

[`rustcoreutils/posixutils-rs`](https://github.com/rustcoreutils/posixutils-rs)
is the most ambitious POSIX-utility project in Rust today: a 34-crate
workspace with **109 utilities at Stage 3** (test coverage) including
every gap the fork has explicitly deferred (`grep`, `sed`, `awk`) plus
extras (`make`, `m4`, `bc`, `vi`, `patch`, `pax`, `bc`, `cron`).

**It is NOT integration-ready today.** Verified upstream state on
2026-04-25:

- ❌ Not published to crates.io. (`crates.io/api/v1/crates/posixutils-awk` → 404.)
- ❌ Workspace member crates that ship the gap utilities (`text/` for
  grep/sed/diff/patch; `awk/` for awk) are **binary-only** — no `[lib]`
  section, no callable `pub fn`. The `[[bin]]` targets dispatch directly
  from `main()`.
- ✅ Some adjacent crates (`process/`) DO expose a `[lib]` — but the
  utilities they ship (`kill`, `nice`, `nohup`, `timeout`, `xargs`) are
  already covered by uutils/coreutils + findutils.
- ❌ No open issues at posixutils-rs requesting library APIs or
  crates.io publication.
- ✅ License: MIT (compatible).
- ✅ Stated upstream goal (README): "each posixutils utility should look
  like normal Rust code, easily stand alone with little-or-no deps, and
  be used in another project" — i.e., upstream is *aligned* with library
  use, just hasn't done the `[lib]` work yet.

This is the **same shape of problem** as Cycle 3 of
[`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md)
(diffutils' missing `pub mod diff;`) but at workspace scale: instead of
one missing `pub mod`, multiple crates need new library targets. The
fork's "consume as-published" discipline (Decision Log 2026-04-25
Cycle 3) applies the same way.

**Plan**: three PDCA cycles. None of them ship integration in the
current release. They:

| Cycle | Scope | Effort | Confidence | Reversible? |
|---|---|---|---|---|
| 1 | Upstream engagement: file issue at posixutils-rs proposing `[lib]` + crates.io publication for the gap-filler crates | 1 day | High (it's a doc + an upstream PR/issue) | Yes |
| 2 | Local prototype on a fork branch: patch one crate to add `[lib]`, exercise the adapter pattern, prove the integration shape works | 1–2 days | Medium | Yes |
| 3 | Selective integration once Cycle 1 lands upstream: ship 4–6 high-value gap-fillers behind feature flags | 2–4 days | Medium (depends on per-utility quality) | Yes |

## Alternative sources reviewed — 2026-04-25

A parallel sweep of cloned upstream sources at
`C:\Tools\brush-shell-resources\` evaluated six repos as candidate sources
for the gap-filler utilities. Findings, with per-utility verdicts that
supersede the original "wait for posixutils-rs upstream" framing for
sed/awk/grep specifically:

### Selection criteria for any candidate source

Before any source is accepted (whether for crates.io dep or vendoring),
it must meet a baseline on each axis. These are graded — none is binary
pass/fail — but a source that scores low on multiple axes is rejected.

| Axis | What we measure | Why |
|---|---|---|
| **POSIX/Linux compatibility** | Does it pass a recognized reference suite? Have side-by-side comparison tests against gawk/grep/sed? Does the README claim conformance with backed evidence? | Bundled brush utilities must produce the *same* output as Linux/macOS counterparts. A "POSIX-compatible" claim without a test suite isn't enough. |
| **Maintenance signal** | Active CI? Issue tracker traffic? Recent commits? Multi-contributor or single-author? Test-coverage badge? Released on crates.io? | If we vendor, we own the code forever; if we depend, upstream rot blocks us. Both modes need a real upstream. |
| **Performance** | Documented benchmarks (Criterion or similar)? Throughput numbers vs reference implementation? No O(n²) algorithms on hot paths? | Brush is a shell — pipelines flow large amounts of data through these utilities. A correct-but-slow `awk` makes brush feel sluggish. |
| **Cross-platform** | CI matrix includes Windows? Path handling tested with `\` separators? No Unix-only crates (`nix`, `libc::ioctl`, etc.) on the hot path? | Brush's headline use case is Git Bash replacement on Windows. A Linux-only utility is a non-starter. |
| **License compatibility** | MIT, Apache-2.0, or MIT/Apache-2.0 dual. GPL is incompatible with brush's MIT licensing. | Hard requirement. |
| **Library API quality** | `[lib]` exposed (preferred) or self-contained binary source we can rename to a public function (vendorable). Stable signature. | Determines whether crates.io dep or vendor is the cheaper path. |

These criteria apply uniformly. The per-utility verdicts below all
record where each candidate scores on each axis — no source has been
accepted on enthusiasm alone.

### Sources surveyed

| Repo | Origin | License | Last commit | Role |
|---|---|---|---|---|
| `posixutils-rs` | [`rustcoreutils/posixutils-rs`](https://github.com/rustcoreutils/posixutils-rs) | MIT | 2026-04-03 | 109 utilities at Stage 3; **bin-only**, no `[lib]`; deeply tied to internal `plib` libc wrapper. Best provenance, hardest vendor. CI: Ubuntu + macOS only (no Windows). |
| `sed-uutils` | [`uutils/sed`](https://github.com/uutils/sed) | MIT | 2026-04-24 | v0.1.1 published on crates.io; standard `uumain` API; uucore 0.8.0 = zero skew with brush; claims Towers-of-Hanoi + arbitrary-precision math passing. **Direct crates.io dep, no vendoring.** |
| `awk-rs` (`rawk` clone) | [`pegasusheavy/awk-rs`](https://github.com/pegasusheavy/awk-rs) (formerly `quinnjr/rawk`) | MIT OR Apache-2.0 | 2026-04-15 | v0.1.0 published on crates.io with **public `[lib]` API**; **CI matrix tests Windows + macOS + Linux**; 639 tests / 86% library coverage; Criterion benchmarks; minimal deps (`regex` + `thiserror`); claims 100% POSIX + gawk extensions. **Direct crates.io dep, no vendoring.** |
| `fastgrep` | [`awnion/fastgrep`](https://github.com/awnion/fastgrep) | MIT OR Apache-2.0 | 2026-04-14 | v0.1.8 published on crates.io with **public `[lib]` API** (`pub mod {cli, output, pattern, searcher, threadpool, trigram, walker}`); upstream README declares "drop-in replacement for GNU grep"; bin target named `grep`; 10 integration test files; Criterion benchmarks measure **2–12× faster than GNU grep**; SIMD literal search via `memchr`, parallel by default, lazy trigram index. **Single-contributor upstream.** **CI tests Ubuntu + macOS only — no Windows in upstream CI** (mitigation: brush-side Windows smoke gate before ship). MSRV 1.92 (2024 edition). |
| `diffutils` | [`uutils/diffutils`](https://github.com/uutils/diffutils) | MIT/Apache-2.0 | 2026-04-21 | v0.5.0; `pub mod cmp;` exposed but `pub mod diff;` still missing — same blocker as documented in [`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md) Cycle 3. No change. |
| `posix-tools-for-windows` | [`fukuyori/posix-tools-for-windows`](https://github.com/fukuyori/posix-tools-for-windows) | MIT | 2026-03-27 | 37 self-contained Rust crates; **Windows-targeted by design** (internal glob, Shift_JIS/EUC-JP, case-insensitive paths). Single author, 2 commits, no CI, Japanese-only docs. `grep/Cargo.toml` lists `authors = ["Claude"]` → LLM-assisted. **Used only for `grep` in the current plan** (awk-rs supersedes posix-tools-for-windows for awk). |
| `rustix` | [`bytecodealliance/rustix`](https://github.com/bytecodealliance/rustix) | Apache-2.0+LLVM / MIT | 2026-04-17 | POSIX syscall library, not a utility source. Excludes Windows except for Winsock; not a `nix` drop-in for brush; no help with Windows job control. **Out of scope for this plan.** |

### Scorecard against selection criteria

| Source / utility | POSIX compat | Maintenance | Performance | Cross-platform | License | Lib API |
|---|---|---|---|---|---|---|
| `uutils/sed` (sed) | ✅ Towers-of-Hanoi, arbitrary-precision math test scripts | ✅ active, dependabot, recent commits | ⚠️ no published benchmark, but pre-feature-complete (v0.1.1) | ✅ uucore-shape; uutils' standard cross-platform conventions | ✅ MIT | ✅ `uumain(args)` exposed |
| `pegasusheavy/awk-rs` (awk) | ✅ 100% POSIX claim; 34 gawk-compat tests; 412 e2e | ⚠️ 2 contributors (1 human + dependabot); CI green; v0.1.0 fresh | ✅ Criterion benches; ~80ms vs gawk ~50ms (1.6×, acceptable) | ✅ **CI matrix tests Windows-latest** | ✅ MIT OR Apache-2.0 | ✅ `pub mod {ast,error,interpreter,lexer,parser,value}`; re-exports `Lexer`/`Parser`/`Interpreter` |
| `awnion/fastgrep` (grep) | ✅ "drop-in replacement for GNU grep" per upstream; [`GNU_GREP_COMPAT.md`](https://github.com/awnion/fastgrep/blob/main/GNU_GREP_COMPAT.md) catalogs only ~10 unsupported flags (`-G`, `-P`, `-z`, `--line-buffered`, `-R`, `-d`, `-D`, `--binary-files`, `-NUM`); 10 integration-test files | ✅ Active single-author maintenance (regular Jan/Feb/Mar/Apr 2026 commits); 5 CI workflows; CHANGELOG; ARCHITECTURE doc | ✅ Criterion benches show **2–12× faster** than GNU grep on hot paths (sparse-literal `-rn`: 4.4×, dense `-rc`: 12×, regex `-rn`: 9.4×) | ⚠️ **Ubuntu + macOS in CI only — no Windows.** Brush-side Windows smoke gate required before ship. | ✅ MIT OR Apache-2.0 | ✅ `pub mod {cli, output, pattern, searcher, threadpool, trigram, walker}`; bin target named `grep` |
| `posix-tools-for-windows` (grep, fallback only) | ⚠️ ad-hoc; no comparative test suite | ❌ single author, 2 commits, no CI, LLM-assisted | ⚠️ no benchmark | ✅ Windows-by-design | ✅ MIT | ❌ bin-only; vendor required |
| `posixutils-rs` (m4/bc/make/patch) | ✅ POSIX-spec-driven; Stage 3 test coverage | ✅ active, multi-contributor, MIT | ⚠️ no benchmark | ❌ Ubuntu + macOS CI only; tied to `plib` libc wrapper | ✅ MIT | ❌ bin-only; vendor fan-out via `plib` |

### Hard rejections from the criteria

- **`posix-tools-for-windows` for `awk`** — rejected because `pegasusheavy/awk-rs` outscores it on every axis (lib API, CI Windows coverage, test count, license breadth, contributor isolation risk). The earlier first-pass plan (Cycle 0c "vendor awk from posix-tools-for-windows") is **withdrawn**; replaced by Cycle 0c-revised (clean crates.io dep on `awk-rs = "0.1.0"`).
- **`posix-tools-for-windows` for `grep` (primary)** — demoted from primary to **fallback** because `awnion/fastgrep` v0.1.8 outscores it on POSIX/GNU compat (drop-in claim, only ~10 unimplemented GNU flags catalogued in [`GNU_GREP_COMPAT.md`](https://github.com/awnion/fastgrep/blob/main/GNU_GREP_COMPAT.md)), maintenance signal, performance (Criterion benches showing 2–12× over GNU grep), and license breadth. posix-tools-for-windows-grep stays as a **fallback** only if Cycle 0b's brush-side Windows smoke gate fails on fastgrep. The second-pass plan (Cycle 0b "vendor grep from posix-tools-for-windows") is **withdrawn**; replaced by Cycle 0b-revised (clean crates.io dep on `fastgrep = "0.1.8"`).
- **`posixutils-rs` for `sed`/`awk`/`grep`** — rejected because the `plib` internal dependency makes vendoring fan out, and the upstream is bin-only. Stays the path for `m4`/`bc`/`make`/`patch` only because no alternative exists.
- **`rustix` everywhere** — rejected because it's not a utility source; out of scope.

### Per-utility decision matrix (post-sweep)

| Utility | Recommendation | Source | Effort | Risk |
|---|---|---|---|---|
| `sed` | **Cycle 0a**: real crates.io dep on `uutils/sed = "0.1.1"` | uutils/sed | ~30 min adapter | Low — pre-feature-complete; track for 0.2.0+ |
| `awk` | **Cycle 0c-revised**: real crates.io dep on `awk-rs = "0.1.0"`. Clean lib API; **CI tests Windows**; 639 tests; Criterion benches; benchmarked at 1.6× gawk on 100k-line sum. | pegasusheavy/awk-rs | ~1 hour adapter (lib API requires a thin wrapper that mimics the upstream `main.rs` arg-parsing — slightly thicker than `sed`'s `uumain` because awk-rs's API is `Lexer`/`Parser`/`Interpreter` not a single entrypoint) | Low — well-tested upstream with Windows CI |
| `cmp` | **Cycle 0**: real crates.io dep on `diffutils = "0.5.0"` (already known per coverage-expansion) | uutils/diffutils | ~30 min adapter | Low |
| `diff` | Still **deferred** until upstream exposes `pub mod diff;` (1-line PR is the unblock) | uutils/diffutils | (blocked) | — |
| `grep` | **Cycle 0b-revised**: real crates.io dep on `fastgrep = "0.1.8"`. Bin target named `grep`; brush registers both `grep` and `fastgrep` aliases pointing at the same adapter (matching upstream's "installed binary is called `grep`" intent). 2–12× faster than GNU grep per upstream Criterion. **Gated** on a brush-side Windows smoke test before merge — upstream CI doesn't cover Windows. | awnion/fastgrep | ~1 hour adapter + 0.5 day Windows smoke gate | Low-Medium — fastgrep is well-tested upstream, but Windows-buildability is unverified; plan budgets a smoke gate to confirm. |
| `grep` (fallback) | **Cycle 0b-fallback**, used only if Cycle 0b-revised's Windows smoke gate fails: vendor `grep/src/main.rs` (~1,503 lines) from posix-tools-for-windows into `brush-bundled-extras/src/posixtools/grep.rs`. License-compatible (MIT). | posix-tools-for-windows | 0.5–1 day incl. audit | Medium — LLM-authored, sparse tests; only invoked if fastgrep doesn't build/run on Windows. |
| `m4` / `bc` / `make` / `patch` | **Defer.** posixutils-rs versions are tied to internal `plib` (Unix libc wrapper); vendoring fan-out exceeds 2 days each. Original Cycle 1 upstream-engagement plan still the right shape if user demand materializes. | posixutils-rs | 2–4 days each | Medium-High — Unix-leaning; some Windows-incompat hazards (ioctl, signals) |

### Why the §6a "no vendoring" stance is softened (not abandoned)

The original §6a rejected vendoring on the grounds that it "must not
become the default escape hatch". That reasoning still holds **for
posixutils-rs's gap-filler crates** because their vendoring fan-out is
real (each utility imports `plib::*` which wraps libc; vendoring cleanly
means vendoring `plib` too, plus 23 sibling utilities' shared modules in
`text/`).

`posix-tools-for-windows` has the *opposite* shape: each utility is its
own self-contained crate with no `crate::*` cross-dependencies and no
internal shared library. Its `awk` crate has 8 files all in
`awk/src/`; `grep` and `sed` are single files. Vendoring one crate is a
copy-paste, not a maintenance treadmill.

The `posix-tools-for-windows` provenance concerns are real and recorded
explicitly:

1. **Single author, 2 commits.** No upstream community to push fixes
   back to. If we vendor, we own the maintenance forever — but that's
   true of any vendored code.
2. **LLM-assisted authorship.** The `grep/Cargo.toml` literally lists
   `authors = ["Claude"]`. Code looks plausible on inspection but
   has not been audited for correctness against a reference test
   suite. **Pre-vendor audit is mandatory** — at minimum: read the
   regex / address-range / hold-space code paths in sed; the lexer +
   parser + interpreter wiring in awk; the `--include`/`--exclude` +
   recursion logic in grep. Pair this with a brush-side smoke-test
   suite running real-world scripts (a small subset of GNU sed's test
   suite, AWK book exercises, common grep flag combinations).
3. **Japanese-only docs.** Not a blocker, but inline comments are in
   Japanese and would benefit from English translation in the brush-
   side fork. Optional, not on the critical path.
4. **Sparse tests.** sed claims "54 unit + 10 CLI integration"; awk
   has stress tests; grep has glob/path tests. Brush-side test
   coverage extension is part of the audit budget above.

These concerns make Cycle 0b/0c **medium-risk**, not low-risk. The
budget is sized accordingly (1–2 days per vendored utility, dominated
by audit + integration tests, not the copy-paste itself).

### Cycle 0 — Quick wins (no vendoring required)

This sub-cycle ships **before** Cycles 0b/0c (vendoring) and **before**
Cycles 1–3 of the original upstream-engagement plan. Both are clean
crates.io deps that plug directly into the existing
`brush-bundled-extras` adapter pattern.

**Cycle 0a — `sed` via uutils/sed crates.io dep**:

```toml
# brush-bundled-extras/Cargo.toml
sed = { version = "0.1.1", optional = true }

[features]
'extras.sed' = ["dep:sed"]
'extras.uutils-sed-all' = ['extras.sed']
```

```rust
// brush-bundled-extras/src/lib.rs
#[cfg(feature = "extras.sed")]
fn sed_adapter(args: Vec<OsString>) -> i32 {
    sed::sed::uumain(args.into_iter())
}
```

```toml
# brush-shell/Cargo.toml
experimental-bundled-extras-uutils-sed = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.uutils-sed-all",
]
```

DoD: `brush -c 'echo hello | sed s/h/H/'` produces `Hello`.

**Cycle 0b-revised — `grep` via crates.io dep on `awnion/fastgrep`**

`fastgrep` v0.1.8 is published on crates.io with a public `[lib]` API
and a bin target literally named `grep`. The upstream README states
"The installed binary is called `grep`" — meaning the upstream's own
intent matches brush's "register as both `grep` and `fastgrep`" goal.
Performance is 2–12× faster than GNU grep on hot paths per upstream
Criterion benchmarks; AI-agent-friendly defaults (file size limit,
line truncation, JSON output) align with brush's primary use case
(Claude Code's Bash tool).

The library exposes building blocks rather than a single `run(args)`
function, so the brush-side adapter mirrors fastgrep's `src/bin/grep.rs`
orchestration (~400 lines of glue around `Cli::parse`,
`CompiledPattern::compile`, `walk`, `ThreadPool`, `search_file_streaming`,
trigram index load/save, etc.). This is more adapter code than
`uutils/sed`'s `uumain` shape, but still no vendoring.

```toml
# brush-bundled-extras/Cargo.toml
fastgrep = { version = "0.1.8", optional = true }

[features]
'extras.grep' = ["dep:fastgrep"]
'extras.fastgrep-all' = ['extras.grep']
```

```rust
// brush-bundled-extras/src/grep_adapter.rs
#[cfg(feature = "extras.grep")]
pub fn grep_adapter(args: Vec<OsString>) -> i32 {
    use clap::Parser;
    use fastgrep::cli::Cli;
    // Mirror fastgrep/src/bin/grep.rs::main() — config resolve,
    // pattern compile, dispatch to single-file / stdin / files path.
    // Pure glue, no behavioral overrides.
}
```

**Dual-name registration** (matches upstream README intent):

```rust
// brush-bundled-extras/src/lib.rs::bundled_commands()
#[cfg(feature = "extras.grep")]
{
    map.insert("grep".into(), grep_adapter);
    map.insert("fastgrep".into(), grep_adapter);  // alias per user request + upstream README
}
```

Both names dispatch to the same adapter. Native shell `grep` (none
exists in `brush-builtins/`) and PATH `grep` are shadowed by the
bundled version when `extras.grep` is enabled; users on Windows who
install the fork over Git Bash get the fast version by default.

**MANDATORY pre-merge gates** (logged in CHANGELOG before flag is enabled):

0. **MSRV gate** — fastgrep declares `rust-version = "1.92"` ([fastgrep/Cargo.toml:5](https://github.com/awnion/fastgrep/blob/main/Cargo.toml#L5)) while brush declares `rust-version = "1.88.0"` ([brush root Cargo.toml:27](../../Cargo.toml#L27)). Implementer must choose one of:
   - **(a)** Bump brush workspace MSRV from 1.88 → 1.92. Clean but breaks any user on rustc 1.88–1.91.
   - **(b)** Feature-conditional MSRV — workspace stays at 1.88; document that `experimental-bundled-extras-fastgrep` requires rustc ≥ 1.92. Compile error is implicit if a user enables the flag on an older toolchain. Acceptable for an experimental feature. **Recommended default.**
   - **(c)** File an upstream PR at fastgrep asking them to drop MSRV (1.92 is recent; the actual feature usage may not require it). Pursue in parallel with (b); doesn't block ship.
   - **(d)** Fall through to Cycle 0b-fallback (vendor from posix-tools-for-windows). Sidesteps MSRV but loses fastgrep's quality advantages.

   Verified 2026-04-25: brush root MSRV 1.88.0; `awk-rs` MSRV 1.85 (under brush MSRV → no friction); `uutils/sed` 0.1.1 MSRV 1.88 (exact match → no friction). Only `fastgrep` is above MSRV.

1. **Windows-build smoke gate** — fastgrep's CI matrix is
   `[ubuntu-latest, macos-latest]` only; Windows is unverified.
   Verify on a Windows runner:
   - [ ] `cargo build -p brush-shell --features experimental-bundled-extras-fastgrep` exits 0.
   - [ ] `brush -c "grep --version"` runs and prints fastgrep banner.
   - [ ] `brush -c "grep -rn 'fn main' src"` produces output and exit 0
         on a tree with matches; exit 1 on a tree without.
   - [ ] `brush -c "echo hello | grep h"` (stdin path) produces output
         and exit 0.
   - [ ] Trigram cache directory resolves correctly on Windows
         (probably `C:\Users\<user>\AppData\Local\fastgrep\trigram`
         via the `dirs` crate; verify it's writable and not aborting
         the binary if it fails).
   - [ ] Memory-mapped file reads via `memmap2` work on Windows path
         names with backslashes and case-insensitive volumes.
   - [ ] EPIPE behavior under `grep -rn 'pat' . | head -n 5` —
         findutils' EPIPE panic on Windows is a known issue in this
         family; verify fastgrep handles it cleanly. If it doesn't,
         file an upstream issue and decide whether to ship anyway with
         the limitation documented (matches brush's existing findutils
         caveat).

   If any of these fail and can't be patched cheaply (a few-line
   upstream PR, gated `cfg(windows)` adjustment, etc.): **abort
   Cycle 0b-revised, fall through to Cycle 0b-fallback** (vendor
   from posix-tools-for-windows). The fallback is **explicitly
   designed to be invoked only on smoke-gate failure** — the user-
   visible outcome (working `grep`) is the same; the implementation
   path is just less preferred.

2. **GNU compat audit** — read [`GNU_GREP_COMPAT.md`](https://github.com/awnion/fastgrep/blob/main/GNU_GREP_COMPAT.md)
   and confirm the unsupported flags don't matter for brush's
   target audience. Currently catalogued unsupported: `-G`/`-P`/`-z`,
   `--line-buffered`, `-R`/`-d`/`-D`, `--binary-files`, `-NUM`. Most
   common interactive flags (`-rn`, `-i`, `-c`, `-l`, `-F`, `-E`,
   `-A`/`-B`/`-C`, `-o`, `--include`/`--exclude`, color) are
   supported.

3. **Behavioral-deviation note in CHANGELOG** — fastgrep has three
   intentional deviations from GNU grep that users should know about
   *before* they're surprised:
   - **Default 100 MiB file size limit** (override:
     `FASTGREP_NO_LIMIT=1` or `--max-file-size=<BYTES>`).
   - **Default 15000-byte line truncation** (override:
     `--max-line-len=0`).
   - **Output order non-deterministic** (parallel by default; force
     single-threaded with `-j1`).
   These are AI-agent-friendly defaults but may surprise users
   coming from GNU grep. Brush takes them as-is — overriding upstream
   defaults is out of scope.

**Cycle 0c-revised — `awk` via crates.io dep on `awk-rs`**

`pegasusheavy/awk-rs` v0.1.0 is published on crates.io and exposes a
clean `[lib]` API with `pub mod {ast, error, interpreter, lexer,
parser, value}` and re-exports `Lexer`, `Parser`, `Interpreter`,
`Value`. Its CI matrix tests **Windows + macOS + Linux**, it ships
639 tests at 86% library coverage, and Criterion benchmarks document
performance within 1.6× of gawk on a 100k-line sum.

The upstream `main.rs` shows the canonical CLI driver — about 150
lines that parse `-f`/`-F`/`-v`/`--posix`/`--traditional` flags,
construct an `Interpreter`, and call `interpreter.run(inputs,
&mut output)`. The brush-side adapter mirrors that shape:

```toml
# brush-bundled-extras/Cargo.toml
awk-rs = { version = "0.1.0", optional = true }

[features]
'extras.awk' = ["dep:awk-rs"]
'extras.awk-rs-all' = ['extras.awk']
```

```rust
// brush-bundled-extras/src/awk_adapter.rs
#[cfg(feature = "extras.awk")]
pub fn awk_adapter(args: Vec<OsString>) -> i32 {
    use awk_rs::{Interpreter, Lexer, Parser};
    use std::io::{self, BufReader};

    // CLI parsing modeled on awk-rs/src/main.rs::run().
    // ~50 lines — no logic change vs upstream, just adapted to
    // brush's BundledFn signature.
    // ...
}
```

DoD:
- `brush -c "echo 'a b c' | awk '{print $2}'"` → `b`
- `brush -c "awk 'BEGIN{for(i=1;i<=10;i++) sum+=i; print sum}'"` → `55`
- Mixed pipeline: `brush -c "find . -name '*.rs' | xargs awk '{print FILENAME, NR}'"` works.
- Verify upstream MSRV `1.85` (2024 edition) is compatible with
  brush's MSRV. If not, hold for a later awk-rs version that
  loosens MSRV, or fall back to vendoring the relevant interpreter
  modules with a brush-side back-port.

Risk profile: **Low.** Unlike the original Cycle 0c (vendor from
posix-tools-for-windows), this path has:
- Real upstream maintenance (CI, dependabot, security policy).
- Test coverage we can rely on rather than recreate.
- Cross-platform CI we don't have to verify ourselves.
- Clean upgrade path (`cargo update`) when awk-rs ships fixes.

The upstream is still single-author + dependabot, so it's not
risk-free — but the audit + test-extension budget that the original
Cycle 0c demanded is largely obviated by upstream's existing
infrastructure.

### Updated bundle layout

The proposed `extras.posixutils-all` aggregate is renamed to
`extras.posixtools-all` to reflect that the source is now multi-upstream
(posix-tools-for-windows for awk/grep; uutils for sed/cmp; posixutils-rs
deferred for m4/bc/make/patch):

```toml
# brush-bundled-extras/Cargo.toml — proposed additions

[dependencies]
sed      = { version = "0.1.1", optional = true }  # Cycle 0a         — clean dep (uutils/sed)
fastgrep = { version = "0.1.8", optional = true }  # Cycle 0b-revised — clean dep (awnion/fastgrep)
awk-rs   = { version = "0.1.0", optional = true }  # Cycle 0c-revised — clean dep (pegasusheavy/awk-rs)

# (no Cargo deps for vendored utilities — `extras.posixtools-all`
# would only contain code if Cycle 0b-fallback were invoked.)

[features]
# Per-utility features (opt-in individually).
'extras.sed'  = ["dep:sed"]
'extras.grep' = ["dep:fastgrep"]
'extras.awk'  = ["dep:awk-rs"]

# Source-grouped aggregates.
'extras.uutils-sed-all' = ['extras.sed']
'extras.fastgrep-all'   = ['extras.grep']  # registers `grep` + `fastgrep` aliases
'extras.awk-rs-all'     = ['extras.awk']
'extras.posixtools-all' = []               # reserved; only populated if 0b-fallback fires

# Top-level aggregate now layers in everything available today.
'extras.all' = [
    'extras.findutils-all',
    'extras.uutils-sed-all',
    'extras.fastgrep-all',
    'extras.awk-rs-all',
]
```

```toml
# brush-shell/Cargo.toml — proposed additions

experimental-bundled-extras-uutils-sed = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.uutils-sed-all",
]
experimental-bundled-extras-fastgrep = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.fastgrep-all",
]
experimental-bundled-extras-awk-rs = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.awk-rs-all",
]
experimental-bundled-extras-posixtools = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.posixtools-all",
]
```

The original `extras.posixutils-all` slot stays reserved for any future
posixutils-rs integration (m4/bc/make/patch) once Cycle 1 upstream
engagement lands. Multiple parallel namespaces (`uutils-sed`,
`fastgrep`, `awk-rs`, `posixtools`, `posixutils-all`) because the
upstreams have different ship cadences, different audit profiles, and
different risk levels — collapsing them into one would lose the
per-utility provenance signal.

### What this changes vs. the original cycle plan

| Original cycle | Status after sweep |
|---|---|
| Cycle 1 (file upstream issue at posixutils-rs) | **Still applicable for `m4` / `bc` / `make` / `patch`.** No longer the path for `sed` / `awk` / `grep`. |
| Cycle 2 (prototype with m4 or bc) | **Deferred** until the gap-filler set narrows enough to justify. With Cycle 0 closing the user-cited gap (sed/awk/grep), m4/bc/make/patch are now niche-audience asks. Re-open if user demand materializes. |
| Cycle 3 (selective integration of all 6 from posixutils-rs) | **Replaced** by Cycles 0a/0b/0c for sed/awk/grep. m4/bc/make/patch slots remain reserved here. |

90-day abort condition on the original Cycle 1 still applies — but the
reduced scope (only m4/bc/make/patch, all niche) makes it easier to let
that abort land cleanly without user-facing impact.

---

## Bundle answer (the user's direct question)

**Where do these utilities go?** Under the existing
`brush-bundled-extras` mega-crate, with a new feature group:

```toml
# brush-bundled-extras/Cargo.toml — proposed additions

# Per-utility features (opt-in individually).
'extras.awk'  = ["dep:posixutils-awk"]
'extras.grep' = ["dep:posixutils-text"]
'extras.sed'  = ["dep:posixutils-text"]
'extras.make' = ["dep:posixutils-make"]
'extras.m4'   = ["dep:posixutils-m4"]
'extras.bc'   = ["dep:posixutils-calc"]

# Source-grouped aggregate.
'extras.posixutils-all' = [
    'extras.awk', 'extras.grep', 'extras.sed',
    'extras.make', 'extras.m4', 'extras.bc',
]

# Top-level aggregate now layers in posixutils.
'extras.all' = [
    'extras.findutils-all',
    'extras.posixutils-all',  # ← new
]
```

```toml
# brush-shell/Cargo.toml — proposed additions

experimental-bundled-extras-posixutils = [
    "dep:brush-bundled-extras",
    "brush-bundled-extras/extras.posixutils-all",
]
```

**Why this layout**:

- Consistent with the
  [`coverage-expansion.md` Cycle 2 decision](./coreutils-coverage-expansion.md):
  one mega-crate `brush-bundled-extras` for every non-uutils-coreutils
  utility, regardless of upstream provider. The `uucore` version-skew
  argument that *was* used to justify per-upstream crates was already
  shown not to drive crate layout (Cargo handles version coexistence at
  the dependency-graph level). Same reasoning applies to posixutils-rs.
- Per-utility flags use the existing `extras.<name>` namespace.
  Source-grouped aggregate is `extras.posixutils-all` (matches the
  `extras.findutils-all` precedent).
- Top-level brush-shell flag is
  `experimental-bundled-extras-posixutils` (matches the
  `experimental-bundled-extras-findutils` precedent).
- **Name collisions are real**: posixutils-rs ships `find`, `xargs`,
  `kill`, `cat`, `ls`, etc. that we already get from
  uutils/findutils/coreutils. Selective integration is the rule:
  posixutils-rs is for the **gaps**, not duplicates. We do NOT enable
  `posixutils-find` if findutils already provides `find`. See §"Scope
  filter" below.

---

## A3 — Problem Frame

### 1. Background

Three utilities have been documented as `command not found` gaps for the
fork's bundled-utility set: `grep`, `sed`, `awk`. The fork's prior
research (`docs/research/grep-sed-awk-options.md`) concluded:

- `grep` → defer indefinitely (no embeddable Rust grep).
- `sed` → track upstream `uutils/sed`.
- `awk` → defer indefinitely (no usable Rust port).

posixutils-rs changes the picture for grep and awk. It DOES ship strict
POSIX implementations of all three plus a long tail of other tools.
Whether to revise the prior conclusion depends on whether posixutils-rs
is integration-ready.

### 2. Current condition (verified 2026-04-25 via `gh api` and raw URL fetches)

#### 2a. What posixutils-rs ships

109 utilities at Stage 3 (test coverage), including:

- `awk`, `grep`, `sed` — the explicit gaps.
- `make`, `m4`, `bc` — extras with no Rust-ecosystem alternative we've
  surveyed.
- `vi` — interactive editor; out of scope for "bundled utility" framing
  (separate epic if ever).
- `patch`, `pax`, `cron`, `mailx`, `uucp`, `cc` (a C99 compiler), `man`,
  `more`/`less`-class display tools — niche, deferred.

Workspace layout (relevant subset):

```text
text/        — 23 binaries: asa, comm, csplit, cut, diff, expand, fold,
               grep, head, join, nl, paste, patch, pr, sed, sort, tail,
               tr, tsort, unexpand, uniq, wc, cmp
awk/         — awk
m4/          — m4
calc/        — bc, calc
make/        — make
process/     — env, fuser, kill, nice, nohup, renice, timeout, xargs
file/        — file ops (cp, mv, ls, ...)
fs/          — fs ops (chmod, du, df, ...)
sh/          — POSIX sh
editors/     — vi, ex, ed
...
```

Stage breakdown (per upstream README):
- Stage 6 (Audited): 0
- Stage 5 (Translated to 2+ languages): 0
- Stage 4 (Code coverage): 0
- Stage 3 (Test coverage): 109 utilities — including all the gap-fillers
- Stage 2 (Feature-complete + POSIX compliant): **21 utilities only**
- Stage 1 (Rough draft): 0
- Stage 0 (Not started): 0

**Important**: most of the gap-filler utilities sit at Stage 3 (tests
exist) but **NOT** Stage 2 (feature-complete). Tests passing != all
POSIX behavior implemented. Integration testing against real-world
shell scripts is required before declaring any utility production-ready.

#### 2b. What blocks integration today

| Crate (member) | Package name | Has `[lib]`? | Published to crates.io? | Verified by |
|---|---|---|---|---|
| `text/` (grep, sed, diff, ...) | `posixutils-text` | ❌ no (23 `[[bin]]`, no `[lib]`) | ❌ no | raw `Cargo.toml` fetch + crates.io 404 |
| `awk/` (awk) | `posixutils-awk` | ❌ no | ❌ no | raw `Cargo.toml` fetch + crates.io 404 |
| `m4/` (m4) | `posixutils-m4` | ❌ no (1 `[[bin]]`, no `[lib]`) | ❌ no | raw `Cargo.toml` fetch |
| `make/` (make) | `posixutils-make` | ❌ no (1 `[[bin]]`, no `[lib]`) | ❌ no | raw `Cargo.toml` fetch |
| `calc/` (bc, expr) | `posixutils-calc` | ❌ no (2 `[[bin]]`, no `[lib]`) | ❌ no | raw `Cargo.toml` fetch |
| `process/` (kill, xargs, timeout, ...) | `posixutils-process` | ✅ yes | ❌ no | raw `Cargo.toml` fetch |

**All five gap-filler host crates** (`text`, `awk`, `m4`, `make`, `calc`)
have no library targets. The one crate that DOES have a library target
(`process/`) ships utilities we already cover via uutils
(`kill`, `xargs`, `timeout`, ...). The blockage is uniform across the
target set, not heterogeneous.

The closest parallel in the fork's history:

- **uutils/diffutils** (Cycle 3 of coverage-expansion) — `cmp::main`
  exposed via `pub mod cmp;` in lib.rs but `diff::main` NOT exposed
  (`pub mod diff;` missing). We deferred Cycle 3 with the rationale that
  vendoring "must not become the default escape hatch."
- **`frawk`** (research doc, awk option 2) — binary-only, declared
  "Awk-like" not POSIX. Same shape but worse semantics.

posixutils-rs's gap-filler crates have the diffutils problem at a
**larger scale** (per-crate, not per-module).

#### 2c. What's already in place to integrate from

[`brush-bundled-extras/src/lib.rs`](../../brush-bundled-extras/src/lib.rs)
has the adapter-function pattern from Cycle 2 of coverage-expansion:

```rust
pub type BundledFn = fn(args: Vec<OsString>) -> i32;

#[cfg(feature = "extras.find")]
fn find_adapter(args: Vec<OsString>) -> i32 {
    // OsString → &str (lossy), call upstream, return exit code.
}

pub fn bundled_commands() -> HashMap<String, BundledFn> { ... }
```

Same pattern works for posixutils-rs once a `[lib]` exists upstream.
Each gap-filler gets a `<util>_adapter` function; `bundled_commands()`
adds them to the registry under their feature flag.

### 3. Goal

- A `brush -c '<cmd>'` invocation for `awk`, `grep`, `sed`, `make`, `m4`,
  or `bc` resolves to a bundled implementation when the corresponding
  feature is enabled, without falling through to PATH.
- Selective scope: posixutils-rs is enabled ONLY for utilities the fork
  doesn't already cover via uutils. Native shell builtins and uutils
  utilities take precedence (existing `register_builtin_if_unset`
  semantics).
- No regression in binary size when no posixutils features are enabled
  (default still empty).
- Integration is upstream-clean: no vendored source in
  `brush-bundled-extras/`. Cycle 3 ships only after Cycle 1 lands a
  posixutils-rs release with the required `[lib]` exposure +
  crates.io publication.

### 4. Root cause — why integration is blocked

1. posixutils-rs's gap-filler crates were authored with Stage-3
   maturity (test coverage) as the priority, not library reusability.
2. The workspace structure groups utilities by domain (`text/`, `awk/`,
   ...), each as a multi-binary crate. Adding `[lib]` per crate is a
   non-trivial structural change — it means deciding the public API for
   each utility (single `pub fn run(args)` per utility? a dispatcher
   function?).
3. crates.io publication has not been a project goal — the README
   explicitly says installation is "via `cargo install` for testing
   purposes."
4. No external project has yet driven the demand for library APIs
   (verified: no relevant open issues at posixutils-rs as of 2026-04-25).
5. Brush is the first known consumer that would benefit. **Filing the
   upstream issue + PR is the unblock action.**

### 5. Out of scope

- Integrating `vi`, `ex`, `ed`, `cc`, `cron`, `mailx`, `uucp`, `pax`,
  `man` — these are not "shell utility" shaped; separate epic if ever.
- Replacing already-bundled utilities (`find`, `xargs`, `cat`, `ls`,
  ...) with posixutils-rs versions. Selective integration only.

### 6. Alternatives considered (rejected)

For each rejected alternative, the rejection rationale is recorded so
future re-evaluation has explicit grounds.

#### 6a. Vendor source into `brush-bundled-extras/src/posixutils/`

**Mechanic**: copy `awk/src/main.rs` (etc.) verbatim into our tree;
rename `fn main()` → `pub fn run(args: ...) -> i32`; commit upstream
license headers.

**Rejected because**:
- Cycle 3 of `coreutils-coverage-expansion.md` (Decision Log 2026-04-25)
  explicitly committed to "consume upstream as-published", citing
  "vendor it must not become the default escape hatch" — vendoring
  decisions compound across cycles.
- posixutils-rs's `text/` crate has 23 binaries; vendoring to ship
  grep + sed means tracking dual upstream sources for `text/`'s shared
  modules (`plib` path-dep, internal helpers) — not a one-time copy.
- License-compatible (MIT/MIT) but maintenance burden is real.

**Re-evaluation trigger**: if Cycle 1 stalls indefinitely AND a hard
user requirement materializes for one of the gap-filler utilities, a
**bounded** vendor of one utility (not the whole crate) could be
considered as a Cycle 4 fallback. Not the default path.

#### 6b. Embed pre-built binaries via `include_bytes!`

**Mechanic**: build posixutils utilities at compile time as standalone
exes; embed each via `include_bytes!`; extract to a temp dir on first
call; spawn via `Command`.

**Rejected because**:
- Binary bloat: each utility 1–5 MB. Six utilities = 6–30 MB added to
  brush.exe.
- Cross-platform build complexity (needs target-specific binaries
  embedded; doubles or triples the CI build matrix).
- Reproducibility: pre-built binary version drift hidden inside the
  brush binary.
- Already rejected for `ripgrep` and `frawk` in the prior research
  (`docs/research/grep-sed-awk-options.md`); same reasoning here.

**Re-evaluation trigger**: none anticipated. If embedding becomes the
only path, the right answer is to defer the utility entirely.

#### 6c. Maintain a brush-side fork of posixutils-rs

**Mechanic**: clone `rustcoreutils/posixutils-rs` to
`slicingmelon/posixutils-rs`; apply the `[lib]` patches there; depend
on the fork via `[patch.crates-io]` or git URL.

**Rejected because**:
- A shipped binary on crates.io can't depend on a git URL — the fork
  branch only works for prototyping, not for ship.
- Maintaining a fork against a 34-crate workspace with active upstream
  development is multi-month effort that would cannibalize brush's own
  development bandwidth.
- The "consume as-published" discipline (see 6a) excludes
  long-running forks too.

**Re-evaluation trigger**: same as 6a — bounded fork of one or two
crates as a Cycle 4 fallback if Cycle 1 stalls AND user demand
materializes.

---

## Scope filter (which utilities to actually ship)

posixutils-rs ships ~109 utilities. We will ship a small subset
behind feature flags. The selection criteria:

1. **Must fill an existing gap.** Already-covered utilities (find,
   xargs, kill, cat, ls, ...) are excluded.
2. **Must be Stage 2 or close.** Stage 3 (tests exist) without Stage 2
   (POSIX-compliant) is shipped only with explicit per-utility quality
   verification.
3. **Must have a stable upstream library API.** Cycle 1 unblock
   condition: `[lib]` + crates.io publish for the host crate.
4. **Must be cross-platform-buildable** (or honestly `cfg`-gated).

Result — the proposed initial set (Cycle 3 target):

| Utility | Posixutils-rs crate | Why ship | Risk |
|---|---|---|---|
| `awk` | `posixutils-awk` (currently bin-only) | High demand; no Rust alternative | Stage 2 status unverified |
| `grep` | `posixutils-text` (currently bin-only) | High demand; ripgrep ≠ POSIX grep | Stage 2 status unverified |
| `sed` | `posixutils-text` (currently bin-only) | uutils/sed v0.1.1 alternative is earlier | Two implementations exist; pick at integration time |
| `make` | `posixutils-make` (status unverified) | No Rust alternative | Niche audience |
| `m4` | `posixutils-m4` (status unverified) | No Rust alternative | Niche audience |
| `bc` | `posixutils-calc` (status unverified) | No Rust alternative | Niche audience |

`vi`, `patch`, `pax`, `cron`, etc. — explicitly out of scope for the
initial integration.

`sed` overlap with uutils/sed: both are tracked. Cycle 3 picks one at
integration time based on completeness, build cost, and upstream
activity. Default tiebreaker: prefer uutils/sed (already pins
`uucore = "0.8.0"` matching ours; zero version skew) unless
posixutils-sed is meaningfully more POSIX-compliant.

---

## PDCA Cycle 1 — Upstream engagement

### Plan

**Hypothesis**: A clear upstream issue + PR proposing `[lib]` sections
on the gap-filler crates is the lowest-risk path to integration. The
upstream's stated goal ("easily ... used in another project") aligns;
this likely just needs a maintainer to greenlight the structure.

**Deliverables**:
1. A drafted issue at
   [`rustcoreutils/posixutils-rs`](https://github.com/rustcoreutils/posixutils-rs/issues)
   titled *"Library API for embedding utilities in third-party shells"*
   (or similar). Cite brush as a concrete consumer. Link the user
   request and the relevant brush plan files. Propose a minimal API
   shape:

   ```rust
   // proposed addition to text/lib.rs (new file)
   pub mod grep { pub fn run(args: impl Iterator<Item = OsString>) -> i32; }
   pub mod sed  { pub fn run(args: impl Iterator<Item = OsString>) -> i32; }
   ```

   This shape is **identical** to uutils' `uumain` convention, which
   makes the brush-side adapter trivial.

2. (Optional, conditional on maintainer signal) A follow-up PR
   implementing `[lib]` for one crate as a proof of concept. Picking
   `text/` or `awk/` is the highest-leverage choice.

3. (Separate ask in same issue) crates.io publication for the affected
   crates. This is required for brush to depend on them as
   `[dependencies]` entries; git-deps work for ad-hoc but not for a
   shipped binary on crates.io.

**Baseline**: `gh issue list --repo rustcoreutils/posixutils-rs` —
verify no duplicate ask exists. (Verified empty 2026-04-25.)

**Success criteria**:
- Issue filed; maintainer acknowledgment within 2 weeks.
- Stated path forward (yes/no/needs-design).

**Failure modes**:
- Issue rejected → revisit Cycle 2 strategy (alternatives in §6 still
  excluded by default; bounded vendor of one utility per §6a is the
  only re-evaluable Cycle 4 fallback).
- Issue ignored → file a draft PR to demonstrate the change is small.
- Maintainer wants a different API shape (e.g., dispatcher function
  instead of per-utility module) → adapt; ship whatever upstream
  prefers since the brush-side adapter layer can absorb shape
  differences.

**Abort condition (time-bound)**:

This plan does not stay open indefinitely. The Cycle 1 outcome must
land within **90 days of issue filing** (one of: PR merged with `[lib]`
+ release tag, OR maintainer rejection, OR an explicit "needs-design"
follow-up issue tracked here). If 90 days pass with no upstream
movement:

- Update Decision Log with the stalled state.
- Mark this entire plan as "stalled — no upstream traction".
- coverage-expansion.md Cycle 5's defer conclusion remains the active
  state for grep/sed/awk.
- A new PDCA only opens if user demand re-prioritizes a §6a bounded
  vendor — not before.

90-day budget chosen deliberately. It's long enough to accommodate
maintainer review cycles on a small open-source project but short
enough that the plan doesn't become a perpetual "tracking" entry in
the planning directory.

### Do

1. Read posixutils-rs CONTRIBUTING.md to understand issue/PR conventions.
2. Draft the issue body. Include:
   - Concrete brush plan link (this doc).
   - Use case: bundled-utility shells.
   - Proposed minimal API.
   - Offer to file the implementation PR.
3. File the issue. Cross-link from this doc's Decision Log.
4. (If time permits) draft the proof-of-concept PR for `text/lib.rs`
   and `awk/lib.rs`.

### Check

- Was the issue acknowledged?
- Is the proposed API shape acceptable to upstream?
- Is crates.io publication on the table?

### Act

- **Accepted**: proceed to Cycle 2 prototype (against the upstream
  branch with the new `[lib]`).
- **Rejected**: document the rejection. Cycle 3 stays deferred; this
  plan converges with the indefinite-defer state.
- **Conditional**: address upstream's conditions; iterate.

---

## PDCA Cycle 2 — Local prototype (gated on Cycle 1 progress)

### Plan

**Hypothesis**: Once one posixutils-rs crate exposes a library API
(either via Cycle 1 landing upstream, or via a brush-side fork
branch for the prototype), the existing
[`brush-bundled-extras/src/lib.rs`](../../brush-bundled-extras/src/lib.rs)
adapter pattern works for posixutils-rs without architectural changes.

**Pre-requisite**: at least one of:
- (a) posixutils-rs upstream merges a `[lib]` PR for one gap-filler
  crate AND publishes it to crates.io, OR
- (b) we accept a temporary git-dep arrangement on a `proto/` branch
  for prototype-only validation (described below).

(b) is acceptable for prototype only — never for ship — because brush
on crates.io can't depend on git URLs.

**Prototype dependency strategy (option b mechanics)**:

```toml
# brush-bundled-extras/Cargo.toml on a proto/ branch only
[dependencies]
posixutils-calc = { git = "https://github.com/<fork>/posixutils-rs",
                    branch = "proto/lib-api-bc",
                    optional = true }
```

- Branch lives at a brush-side temporary fork, NOT promoted to
  `slicingmelon/posixutils-rs` (per §6c rejection — no long-running
  forks).
- The `proto/bundled-posixutils-calc` brush branch is the *only* place
  this dep appears. `main` never carries a git URL.
- If Cycle 2 confirms the adapter pattern, the prototype branch is
  discarded; Cycle 3 starts from clean `main` with crates.io deps once
  upstream lands.

**Why prototype with `m4` or `bc` (not `awk`/`grep`/`sed`)**:

The Cycle 2 prototype validates the **adapter pattern**, not utility
semantics. Picking the smallest surface area minimizes confounding:

- `m4` (1 binary) and `bc` (1 of 2 binaries in `calc/`) are smaller
  utilities with constrained input/output behavior.
- `awk`/`grep`/`sed` semantics are large; bugs in the prototype could
  be misattributed to the adapter pattern when they're actually
  utility-internal.
- The adapter pattern is invariant under utility complexity — the
  shape proven for `bc` works identically for `awk`. Cycle 3 inherits
  the validation without re-running it.

If Cycle 1 lands `[lib]` for `text/` or `awk/` first (because that's
where upstream attention goes), use whichever lands; the prototype
goal is "exercise the adapter on real upstream lib", not "validate a
specific utility."

**Success criteria**:
- `brush -c 'echo "1+2" | bc'` produces `3` (or whatever the chosen
  utility's smoke test is).
- Adapter follows the existing `find_adapter` shape from Cycle 2 of
  coverage-expansion.
- No regression on existing tests.
- Cross-platform: builds on Windows + Linux.

### Do

1. Add the prototype dep (git or local-path) to
   `brush-bundled-extras/Cargo.toml`.
2. Write `<util>_adapter(args: Vec<OsString>) -> i32` translating to the
   upstream API.
3. Add `extras.<util>` feature flag.
4. Add `extras.posixutils-all` source-grouped aggregate (initially
   contains only the prototype utility).
5. Add `experimental-bundled-extras-posixutils` flag on `brush-shell`.
6. Plumb through `install_default_providers()`.
7. Write a smoke test under `brush-shell/tests/cases/` (or, if test
   harness feature-flag plumbing is still deferred per coverage-expansion
   Cycle 1 Phase 1.5, do a manual smoke and document it).

### Check

- Smoke test passes.
- `wc -l` of diff: should be ~30-50 lines following the existing
  pattern.
- Binary size delta with feature on: measure (per coverage-expansion
  Cycle 1 DoD precedent).

### Act

- **Pattern confirmed**: ship Cycle 3 (full selective integration).
- **Pattern broke**: investigate. Most likely cause is upstream API
  shape divergence (e.g., dependency-injection objects like findutils'
  `StandardDependencies`) — solvable in the adapter layer.

---

## PDCA Cycle 3 — Selective integration (gated on Cycles 1 & 2)

### Plan

**Hypothesis**: With the upstream `[lib]` PRs merged + crates.io
publication, and the prototype confirming the adapter pattern, we ship
the initial 4–6 utility set behind feature flags.

**Target utilities**: the §"Scope filter" set —
`awk`, `grep`, `sed`, `make`, `m4`, `bc`.
Each per-utility flag opts in independently; aggregate
`extras.posixutils-all` enables all.

**Success criteria**:
- `brush -c '<util> --version'` works for every utility in the set.
- Each utility has a smoke test under
  `brush-shell/tests/cases/brush/posixutils-<util>.yaml`.
- Mixed pipelines work: `find . | xargs awk '{...}'` (combines
  findutils + posixutils-rs).
- CHANGELOG.FORK.md entry under Features.
- Binary size measured with all posixutils features on.
- Native-builtin collision audit: no posixutils-rs utility shadows a
  brush native or already-bundled uutils utility name. (Selective scope
  prevents collisions by construction; verify in code.)
- Per-utility quality gate: each shipped utility passes a small set of
  real-world script smoke tests. Stage 3 in posixutils-rs ≠ ready for
  brush ship; we set our own bar.

### Do

For each utility in the target set:
1. Add `posixutils-<crate> = "<version>"` to
   `brush-bundled-extras/Cargo.toml`.
2. Add `extras.<util> = ["dep:posixutils-<crate>"]` feature.
3. Write `<util>_adapter` in `brush-bundled-extras/src/lib.rs` per the
   established pattern.
4. Register in `bundled_commands()` under the feature flag.
5. Add `extras.<util>` to the `extras.posixutils-all` aggregate.
6. Smoke test under `brush-shell/tests/cases/brush/`.
7. CHANGELOG entry.

### Check

- All smoke tests pass on Linux + Windows.
- Binary size with all posixutils features: target < 80 MB
  (debug, unstripped) — comparison with Cycle 1 of coverage-expansion's
  42.8 MB measurement.
- No regression on existing tests.

### Act

- **Pass**: ship behind `experimental-bundled-extras-posixutils`.
  Document known limitations per utility.
- **Fail per-utility**: drop that utility from the initial set; file
  upstream issue; document in CHANGELOG as "tracking".
- **Fail across the board** (unlikely if Cycle 2 worked): re-enter
  root-cause analysis.

---

## Composition with other plans

This plan is **additive** with:

- [`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md)
  — Cycle 5 (research, complete) concluded grep/sed/awk should defer
  indefinitely. **This plan supersedes that conclusion only if Cycle 1
  here lands successfully upstream.** Until then, the research doc's
  conclusion stands.
- [`bundled-coreutils-pipelines.md`](./bundled-coreutils-pipelines.md)
  — Path A's external-spawn dispatch (commit `86a8c1c`) means any new
  bundled utility automatically inherits the same parallelism + pgid
  handling as findutils + uutils-coreutils. No additional work needed.

Order-of-landing observation:

- If posixutils Cycle 1 fails or stalls → no impact on existing plans;
  this plan converges with research's defer state.
- If posixutils Cycle 1 succeeds → coverage-expansion Cycle 5 reopens
  with `posixutils-rs` as the integration target, supplanting the
  `track uutils/sed only` recommendation for sed.

---

## Hard pre-flight gates (per cycle)

These gates must be answered before the cycle starts:

### Cycle 1 gates

1. **No duplicate issue exists.** Verified 2026-04-25 (`gh issue list
   --repo rustcoreutils/posixutils-rs`).
2. **CONTRIBUTING.md / governance docs read.** Pending.
3. **Issue body drafted and reviewed.** Pending.

### Cycle 2 gates

1. **One posixutils-rs crate has `[lib]`** — either upstream or in a
   brush-side fork branch.
2. **Adapter target chosen** (recommendation: m4 or bc).
3. **Prototype dependency strategy chosen** — git URL for prototype OK;
   crates.io for ship.

### Cycle 3 gates

1. **Cycle 1 landed upstream**: at least the host crates for the target
   utilities (`text/`, `awk/`, `make/`, `m4/`, `calc/`) have `[lib]` in
   a published release.
2. **crates.io publication confirmed** for those crates.
3. **Per-utility quality smoke** — for each target utility, run a small
   real-world script suite. Stage 2 status from the upstream README
   helps but isn't sufficient on its own.
4. **Native-builtin collision audit** — every utility name in the target
   set verified against `brush-builtins/`, `brush-coreutils-builtins/`,
   `brush-bundled-extras/` registries.

---

## Open questions

1. **Which `sed` implementation to ship?** Two candidates: `uutils/sed`
   (cleaner integration, smaller, earlier) vs `posixutils-rs/sed`
   (larger surface, possibly more complete). Decide at Cycle 3
   integration time based on completeness measurement against a real
   sed script suite.
2. **Should brush-side maintain a posixutils-rs fork?** Default no
   (consume-as-published discipline). But if Cycle 1 stalls
   indefinitely, a temporary fork is the only realistic alternative.
   Decision deferred to Cycle 1 outcome.
3. **`make` is a build tool, not a typical shell utility.** Does it
   belong in `brush-bundled-extras`, or in a new `brush-bundled-tools`
   crate? Defer until Cycle 3; if `make` is the only tool-class entry,
   keep it in `brush-bundled-extras` for simplicity.
4. **vi/ex/ed.** Out of this plan. If a future need arises, that's a
   separate epic with its own PDCA — bundling an interactive editor
   into a shell binary is not a "feature flag" decision.

---

## Effort & confidence recap

| Cycle | Effort | Risk | Required for | Reversible? |
|---|---|---|---|---|
| 1 | 1 day | Low (issue + draft PR; outcome is upstream-controlled) | Cycle 2 to be useful | Yes |
| 2 | 1–2 days | Medium (prototype on a moving target) | Cycle 3 confidence | Yes |
| 3 | 2–4 days | Medium (per-utility quality variance) | Closing user gap | Yes |

Combined Cycles 1+2+3: **4–7 days of brush-side work**, but the wall
clock is **gated on upstream response** to Cycle 1. Calendar duration
could be weeks-to-months.

---

## Definition of Done

For Cycle 1:
- [ ] Upstream issue filed at posixutils-rs.
- [ ] Decision Log entry in this doc with the issue URL.
- [ ] Optional draft PR linked.
- [ ] Acknowledgment received (any direction: yes/no/needs-design).

For Cycle 2:
- [ ] One utility's library API exposed (upstream or fork branch).
- [ ] Adapter function follows the existing `find_adapter` shape.
- [ ] Smoke test in CHANGELOG (or manual smoke documented).
- [ ] Binary size delta measured.
- [ ] CHANGELOG entry under Unreleased / Experimental.

For Cycle 3:
- [ ] All target utilities (`awk`, `grep`, `sed`, `make`, `m4`, `bc`)
  enabled behind feature flags.
- [ ] `experimental-bundled-extras-posixutils` flag on `brush-shell`.
- [ ] Smoke tests under `brush-shell/tests/cases/brush/`.
- [ ] Native-builtin collision audit passed.
- [ ] Per-utility quality smoke logged in CHANGELOG.
- [ ] Cross-platform build verified (Linux + Windows).
- [ ] CHANGELOG entry under Features.
- [ ] coverage-expansion.md Cycle 5 status updated to reflect
  supersession (where applicable).

---

## Decision Log

| Date | Cycle | Decision | Evidence |
|---|---|---|---|
| 2026-04-25 | (planning, draft) | Plan drafted via PDCA. posixutils-rs verified rich (109 utilities at Stage 3) but **integration-blocked**: gap-filler crates `text/` (grep, sed) and `awk/` are binary-only with no `[lib]`; not on crates.io. Bundle slot reserved at `experimental-bundled-extras-posixutils` under existing `brush-bundled-extras` mega-crate (consistent with coverage-expansion Cycle 2's "one mega-crate for non-coreutils" decision). Cycles 1–3 structured as upstream-engagement → prototype → selective integration. Selective scope: `awk`, `grep`, `sed`, `make`, `m4`, `bc`; vi/cron/cc/etc. excluded. | Raw `Cargo.toml` fetches for `text/`, `awk/`, `process/` (process/ has `[lib]`; others don't); crates.io 404 for `posixutils-awk`; posixutils-rs README maturity table; no relevant open upstream issues as of 2026-04-25. |
| 2026-04-25 | (planning, reflexion) | Reflexion review scored draft 3.50/5.0, below 4.0 threshold. Amendments: (1) Verified `m4/`, `make/`, `calc/` Cargo.toml — all binary-only, no `[lib]`. Updated §2b table to remove ❓ entries; blockage now confirmed *uniform* across all five gap-filler crates, not heterogeneous. (2) Added §6 "Alternatives considered" enumerating vendoring (rejected on consume-as-published discipline), binary-embedding (rejected on bloat), brush-side fork maintenance (rejected on cargo-on-crates.io constraint + ongoing-maintenance cost). (3) Added Cycle 1 "Abort condition (time-bound)" — 90-day budget from issue filing; if no upstream traction in that window, plan stalls and coverage-expansion Cycle 5's defer conclusion remains active. (4) Cycle 2 prototype rationale made explicit: prototype with `m4`/`bc` (small surface area) validates the *adapter pattern* without confounding from `awk`/`grep`/`sed` utility-semantics complexity; pattern is invariant under utility complexity. (5) Cycle 2 dependency strategy spelled out: `git`-URL on a brush-side `proto/` branch (never `main`), explicitly NOT a long-running fork, discarded after prototype validation. | Reflexion report 2026-04-25; this doc post-amendment. |
| 2026-04-25 (late evening, post-fastgrep) | (planning, MSRV verification + implementer-entry-point) | User asked "what is MSRV check?", then asked to verify brush's MSRV against the candidates. Findings: brush workspace MSRV is **1.88.0** ([root Cargo.toml:27](../../Cargo.toml#L27)) on edition 2024; local toolchain 1.95.0. **Per-candidate**: `awk-rs` MSRV 1.85 ✅ (under brush; no friction); `uutils/sed` 0.1.1 MSRV 1.88 ✅ (exact match; no friction); `fastgrep` 0.1.8 MSRV 1.92 ❌ (4 versions above brush). Cycle 0b-revised gains a new mandatory **gate 0 (MSRV)** with four explicit options: (a) bump brush MSRV 1.88→1.92, (b) feature-conditional MSRV documented as "experimental flag requires 1.92" — **recommended default**, (c) file upstream PR at fastgrep to drop MSRV (parallel ask), (d) fall back to Cycle 0b-fallback. Also added a "▶︎ Start here (for the implementer picking this up)" section at the top of the plan with concrete first-commit suggestions for Cycles 0a/0b/0c, the integration-pattern reference, and pointers to companion docs ([`coreutils-coverage-expansion.md`](./coreutils-coverage-expansion.md) for the existing adapter pattern; [`docs/research/grep-sed-awk-options.md`](../research/grep-sed-awk-options.md) for prior research). The research doc also gets a supersedence header pointing back here. Together these changes make the plan executable by a fresh Claude Code agent — read "Start here", pick a cycle, ship. | This Decision Log row; new "▶︎ Start here" section in this doc; new gate 0 in Cycle 0b-revised; supersedence header in `docs/research/grep-sed-awk-options.md`. |
| 2026-04-25 (late evening) | (planning, third-pass sweep — fastgrep) | User cloned [`awnion/fastgrep`](https://github.com/awnion/fastgrep) at `C:\Tools\brush-shell-resources\fastgrep` and proposed using it as the brush-side `grep` (registered as both `grep` and `fastgrep`, matching upstream README's "installed binary is called `grep`" intent). Audit against the selection criteria: (1) **POSIX/GNU compat** ✅ — upstream README declares "drop-in replacement for GNU grep"; [`GNU_GREP_COMPAT.md`](https://github.com/awnion/fastgrep/blob/main/GNU_GREP_COMPAT.md) catalogues only ~10 unsupported flags (`-G`/`-P`/`-z`/`--line-buffered`/`-R`/`-d`/`-D`/`--binary-files`/`-NUM`); 10 integration-test files comparing against GNU grep behavior. (2) **Maintenance** ✅ — active single-author commit cadence (Jan/Feb/Mar/Apr 2026); 5 CI workflows (build, lint, test, ci, release); CHANGELOG; ARCHITECTURE.md; AI_AGENT_GREP_USECASES.md; ENVIRONMENT.md. (3) **Performance** ✅ — Criterion benches: **2–12× faster than GNU grep** on hot paths (sparse-literal `-rn` 4.4×, dense `-rc` 12×, regex `-rn` 9.4×); SIMD via `memchr`, parallel by default, lazy trigram index. (4) **Cross-platform** ⚠️ — **CI matrix is `[ubuntu-latest, macos-latest]` only — no Windows.** Mitigation: brush-side Windows smoke gate before merge (build + run grep with `-rn`/stdin/EPIPE checks; verify trigram cache dir resolution via `dirs` crate; verify `memmap2` reads on Windows path semantics). If smoke gate fails, fall back to vendoring grep from posix-tools-for-windows (Cycle 0b-fallback). (5) **License** ✅ — MIT OR Apache-2.0. (6) **Lib API** ✅ — `pub mod {cli, output, pattern, searcher, threadpool, trigram, walker}`; bin target literally named `grep`. **Decision: Cycle 0b is reframed as Cycle 0b-revised (clean crates.io dep on `fastgrep = "0.1.8"`); the prior Cycle 0b (vendor from posix-tools-for-windows) is withdrawn but kept as Cycle 0b-fallback explicitly conditioned on the Windows smoke gate failing.** Both `grep` and `fastgrep` aliases registered in `bundled_commands()` per user request + upstream intent. Three intentional behavioral deviations from GNU grep recorded for CHANGELOG: 100 MiB default file-size limit (override `FASTGREP_NO_LIMIT=1` or `--max-file-size`), 15000-byte default line truncation (override `--max-line-len=0`), non-deterministic output order (parallel; force serial with `-j1`). MSRV 1.92 (2024 edition) — verify against brush MSRV alongside the awk-rs MSRV check. | New `fastgrep` row in §"Sources surveyed", §"Scorecard", §"Hard rejections" (with posix-tools-for-windows-grep demoted to fallback). Cycle 0b reframed; Cycle 0b-fallback added. Bundle layout updated with `fastgrep` dep and `extras.fastgrep-all` aggregate. Code reviewed: `C:\Tools\brush-shell-resources\fastgrep\{Cargo.toml,README.md,GNU_GREP_COMPAT.md,src/lib.rs,src/bin/grep.rs,.github/workflows/_test.yml}`. |
| 2026-04-25 (evening) | (planning, second-pass sweep + criteria formalization) | User raised the question: does the plan check that candidates *match Linux built-ins properly* and are *professional/fast/efficient/cross-platform*? Two follow-ups: (1) New section "Selection criteria for any candidate source" added before the per-utility table, formalizing six axes — POSIX/Linux compat, maintenance signal, performance, cross-platform, license, lib API. Each candidate now scored against these axes in a "Scorecard" table; "Hard rejections" subsection records what fails which axis (posix-tools-for-windows for awk; posixutils-rs for sed/grep/awk; rustix for everything). (2) Second-pass sweep discovered [`pegasusheavy/awk-rs`](https://github.com/pegasusheavy/awk-rs) (cloned as `rawk`, formerly `quinnjr/rawk`) — v0.1.0 published on crates.io with public `[lib]` API (`pub mod {ast,error,interpreter,lexer,parser,value}`; re-exports `Lexer`/`Parser`/`Interpreter`/`Value`), dual MIT OR Apache-2.0, **CI matrix tests Windows + macOS + Linux**, 639 tests / 86% library coverage, Criterion benchmarks documenting 1.6× gawk on a 100k-line sum, minimal deps (`regex` + `thiserror`). This source outscores posix-tools-for-windows on every axis. **First-pass Cycle 0c (vendor awk from posix-tools-for-windows) is withdrawn.** Replaced by Cycle 0c-revised: clean crates.io dep on `awk-rs = "0.1.0"`. Adapter is ~50 lines mirroring upstream's `main.rs` arg parsing — slightly thicker than `sed`'s `uumain` because awk-rs's API is `Lexer`/`Parser`/`Interpreter` rather than a single dispatcher, but still no vendoring. posix-tools-for-windows is downgraded to **grep-only**, and even that is now flagged for "consider deferring until a stronger candidate surfaces" — its low scores on maintenance/POSIX-compat axes make it the highest-risk source in the plan. MSRV check is the one remaining gate (`awk-rs` requires Rust 1.85 / 2024 edition; brush MSRV needs to be verified). | New "Selection criteria" + "Scorecard" + "Hard rejections" subsections in §"Alternative sources reviewed". Revised Cycle 0c. Bundle layout updated with `awk-rs` dep and `extras.awk-rs-all` aggregate. Code reviewed: `C:\Tools\brush-shell-resources\rawk\{Cargo.toml,src/lib.rs,src/main.rs,README.md,BENCHMARK.md,TODO.md,.github/workflows/*.yml}`. |
| 2026-04-25 (afternoon) | (planning, alternative-source sweep) | Sweep of cloned upstream sources at `C:\Tools\brush-shell-resources\` evaluated five candidate repos: `posixutils-rs` (rustcoreutils), `sed-uutils` (uutils/sed v0.1.1), `diffutils` (uutils/diffutils v0.5.0), `posix-tools-for-windows` (fukuyori), and `rustix` (Bytecode Alliance). Findings drove a pivot for sed/awk/grep specifically, while leaving the original posixutils-rs plan intact for m4/bc/make/patch. Per-utility verdicts: (1) **`sed`** — drop deferral. `uutils/sed = "0.1.1"` is published with standard `uumain` API and uucore 0.8.0 pin matching ours; ship as a clean crates.io dep in Cycle 0a. (2) **`awk`** + **`grep`** — `posix-tools-for-windows` ships functional self-contained Rust ports (awk: 6,112 lines / 8 files; grep: 1,503 lines / 1 file), MIT, Windows-targeted by design (internal glob, encoding, case-insensitive paths). Vendor in Cycles 0b/0c. Provenance concerns documented: single author, 2 commits, no CI, Japanese-only docs, `grep/Cargo.toml` lists `authors = ["Claude"]` → LLM-assisted; **per-utility audit + brush-side test-extension is mandatory** (1–2 days each, not a copy-paste). (3) **`m4` / `bc` / `make` / `patch`** — stay on the original posixutils-rs upstream-engagement track; their internal `plib` dependency makes vendoring fan out. (4) **`diff`** — `uutils/diffutils` v0.5.0 still missing `pub mod diff;` (no change). (5) **`rustix`** — out of scope (POSIX syscall lib, not a utility source; no Windows job-control coverage). The §6a stance ("vendoring must not become the default escape hatch") is **softened, not abandoned**: vendoring is now acceptable for sources whose code is *self-contained* (no internal-library fan-out); posixutils-rs's plib-bound crates remain rejected. Two parallel feature namespaces — `extras.uutils-sed-all` (clean dep) and `extras.posixtools-all` (vendored) — preserve per-utility provenance instead of collapsing into one. | New §"Alternative sources reviewed — 2026-04-25" in this doc. Code reviewed: `C:\Tools\brush-shell-resources\posix-tools-for-windows\{sed,grep,awk}\src\*.rs`, `\sed-uutils\Cargo.toml` + `src/lib.rs`, `\diffutils\src\lib.rs`. |

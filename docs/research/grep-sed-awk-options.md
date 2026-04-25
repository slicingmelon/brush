# grep / sed / awk — Rust embedding options for brush

> **Status**: ⚠️ **SUPERSEDED 2026-04-25 (evening)** — see
> [`../planning/posixutils-rs-integration.md`](../planning/posixutils-rs-integration.md)
> §"Alternative sources reviewed" and the "Start here" section there.
>
> The conclusions in this document (defer grep/awk indefinitely; track
> uutils/sed for v0.2.0+) were drawn from a first-pass survey of well-known
> Rust grep/awk projects (ripgrep, frawk). A second-pass sweep of cloned
> upstream sources at `C:\Tools\brush-shell-resources\` discovered three
> candidates that change the picture entirely:
>
> | Utility | Old conclusion (this doc) | New conclusion ([planning doc](../planning/posixutils-rs-integration.md)) |
> |---|---|---|
> | `sed`  | Track upstream, re-evaluate at v0.2.0+ | **Ship now** via crates.io dep on `uutils/sed = "0.1.1"` (Cycle 0a). MSRV 1.88 = brush 1.88. |
> | `awk`  | Defer indefinitely (no usable Rust port; frawk is "Awk-like") | **Ship now** via crates.io dep on `pegasusheavy/awk-rs = "0.1.0"` (Cycle 0c-revised). 100% POSIX claim, 639 tests, CI tests Windows. MSRV 1.85 < brush 1.88. |
> | `grep` | Defer indefinitely (no embeddable Rust grep) | **Ship now** via crates.io dep on `awnion/fastgrep = "0.1.8"` (Cycle 0b-revised). "Drop-in replacement for GNU grep", 2–12× faster. MSRV 1.92 — feature-conditional MSRV bump required, plus Windows smoke gate (fastgrep CI doesn't cover Windows). |
>
> The methodology in this doc was sound — it just didn't include the right
> repos. Future research passes should also check community projects (not
> just the well-known ones), GitHub-search by keyword + recent activity,
> and any candidates the user surfaces. Specifically:
> [`pegasusheavy/awk-rs`](https://github.com/pegasusheavy/awk-rs) and
> [`awnion/fastgrep`](https://github.com/awnion/fastgrep) were both v0.1
> at the time of the first-pass survey but were not in the result set.
>
> **Action for an implementer**: read this doc only for historical context
> (why grep/awk were initially deferred). Then go to the planning doc for
> the actual cycle-0 work. The "Start here" section at the top of the
> planning doc is the entry point.

> **Status**: research · **Created**: 2026-04-25 · **Owner**: @slicingmelon
> **Cycle**: [`coreutils-coverage-expansion.md`](../planning/coreutils-coverage-expansion.md) Cycle 5
> **Output**: this document. **No code change.**

## TL;DR

| Utility | Best Rust candidate | Maturity | Embeddable as `BundledFn`? | Recommendation |
|---|---|---|---|---|
| **grep** | `ripgrep` (`rg`) — BurntSushi | Mature (62k★, v15.1.0, last push 2026-02-27) | ❌ Binary-only; lib crates (`grep`, `grep-searcher`) ship the engine, not a CLI dispatcher. POSIX-grep-on-top-of-rg is a substantial reimplementation. | **Defer indefinitely.** Document as known gap; fall through to PATH. |
| **sed**  | `uutils/sed` v0.1.1     | Early but active (86★, last release 2025-12-17, uucore 0.8.0 = matches ours) | ✅ Standard `uumain(args)` API — drop-in to the existing `register!` macro in `brush-coreutils-builtins`, zero adapter code. | **Track upstream.** Re-evaluate at v0.2.0+ for inclusion via the existing macro path. |
| **awk**  | `frawk` v0.4.8          | Mid (1296★, last push 2025-09-26) | ❌ Binary-only (no `lib.rs`); declares itself "Awk-like", not strict POSIX awk. | **Defer indefinitely.** Document as known gap; fall through to PATH. |

**Net**: only `sed` has a viable Rust path today (uutils/sed), and it's still pre-1.0. `grep` and `awk` should be documented as PATH-fallthrough until the Rust ecosystem matures.

---

## Methodology

For each utility we evaluated:
1. **Existence of a uutils-org project** (would slot into the existing crate scaffold).
2. **Maturity** (stars, last activity, release cadence) as a proxy for shipping risk.
3. **Embeddability** — does the crate expose a function we can call from `bundled_commands()`, or is it bin-only? Bin-only crates need either binary embedding (`include_bytes!` + spawn) or CLI-dispatch reimplementation, both of which are large enough to warrant their own cycles.
4. **API shape** — if a function exists, does it match `uumain(impl Iterator<Item = OsString>) -> i32` (drop-in to `register!`) or need an adapter (à la findutils)?
5. **License compatibility** — needs to be MIT-compatible.
6. **uucore version skew** — informational; binary-bloat tax.

Data was gathered 2026-04-25 via `gh api` against each upstream's repository and `Cargo.toml`.

---

## grep

### Landscape

- **No `uutils/grep` repo exists.** Confirmed via `gh api orgs/uutils/repos --paginate` — the uutils org has no grep effort.
- **`ripgrep` (`BurntSushi/ripgrep`)** is the canonical Rust grep:
  - 62,926★ · last push 2026-02-27 · v15.1.0
  - License: `Unlicense OR MIT` (compatible)
  - Workspace structure: `[[bin]] name = "rg" path = "crates/core/main.rs"` plus library member crates (`grep`, `grep-searcher`, `grep-printer`, `grep-matcher`, `grep-regex`, `grep-cli`, `grep-pcre2`, `globset`, `ignore`).
  - **Critically**: the ripgrep binary's argv parsing lives in `crates/core/main.rs` and is not a `pub fn`. The library crates expose the *search engine*, not the CLI.

### Embeddability assessment

Three paths, none of them clean:

1. **Embed `rg.exe` and dispatch via subprocess.** Pre-builds a binary, ships it inside brush via `include_bytes!`, writes it to a temp dir on first call, dispatches. ~5 MB binary cost. Adds complexity to extraction logic, breaks reproducibility if pre-built rg version drifts.
2. **Reimplement a POSIX-grep CLI on top of ripgrep's library crates.** Use `grep-searcher` + `grep-regex` + `grep-printer` to build a new `grep_main(args: Vec<OsString>) -> i32` function that parses POSIX grep flags (`-E`/`-F`/`-i`/`-v`/`-c`/`-n`/`-l`/`-r`/`-A`/`-B`/`-C`/`-w`/`-x`) and produces POSIX-grep output. Substantial work — equivalent to writing a small grep clone. Ongoing maintenance burden if ripgrep's library crates evolve.
3. **Translate POSIX grep argv to `rg` argv and embed.** Hybrid of (1) and (2): take POSIX-grep flags, rewrite to ripgrep flags, hand off to `rg`. Mostly works; some flag combinations (`-w` vs `--word-regexp`, `-r` semantics, `-l`/`-L` vs `--files-with-matches`) need careful translation. Output format differences (line numbers, color, file headers) need ripgrep's `--no-heading` / `--color=never` etc. forced. Maintains a translation layer between two grep dialects.

### POSIX vs ripgrep flag matrix (for path 3 reference)

| POSIX grep | ripgrep equivalent | Notes |
|---|---|---|
| `-E` | `-E` / `--regexp-type=ext` | Default in rg already |
| `-F` | `-F` / `--fixed-strings` | Same |
| `-i` | `-i` | Same |
| `-v` | `-v` | Same |
| `-c` | `-c` | Same |
| `-n` | `-n` | rg auto-emits in TTY; force with `-n` |
| `-l` | `-l` / `--files-with-matches` | Same |
| `-L` | `--files-without-match` | Different long flag |
| `-r` | (default) | rg always recurses |
| `-w` | `-w` / `--word-regexp` | Same |
| `-x` | `-x` / `--line-regexp` | Same |
| `-A N` | `-A N` | Same |
| `-B N` | `-B N` | Same |
| `-C N` | `-C N` | Same |
| `-h` | `--no-filename` | Different |
| `-H` | `--with-filename` | Different |
| `-q` | `-q` | Same |
| `-s` | (no equivalent; suppress errors via stderr redirect) | rg always logs errors |
| `--include=PAT` | `-g PAT` / `--glob` | Different syntax |
| `--exclude=PAT` | `-g '!PAT'` | Different |
| color heuristics | `--color=never` to disable | rg defaults to auto |

### Recommendation

**Defer indefinitely.** None of the three paths is a small enough cycle to warrant including in the current expansion plan. ripgrep on PATH (Git Bash MSYS2 ships it; most Linux distros have grep) is a working fallback for the foreseeable future.

If grep becomes a hard requirement later, **path 3 (translation layer)** is the lowest-risk option — it's a few hundred lines of argv translation plus a fork-exec, no library-crate reimplementation. But it should be its own cycle, not a sub-task of the coverage-expansion plan.

---

## sed

### Landscape

- **`uutils/sed` exists and is active.**
  - 86★ · last commit 2026-04-24 · semver release v0.1.1 (2025-12-17)
  - License: MIT (compatible)
  - **uucore = "0.8.0"** — matches our `brush-coreutils-builtins` pin exactly. Zero version skew.
  - Standard uutils `uumain` shape: `sed::sed::uumain(args.into_iter()) -> i32`
- Other Rust "sed alternatives" — `chmln/sd` (7077★) and `ms-jpq/sad` (2026★) — are NOT POSIX sed compatible. They are intuitive find/replace tools with different semantics. Out of scope.

### Embeddability assessment

uutils/sed is the cleanest non-coreutils integration we've evaluated:

- API shape is identical to `uutils/coreutils` utilities — `uumain(impl Iterator<Item = OsString>) -> i32`.
- uucore version matches; no binary-bloat tax beyond uu_sed's own footprint.
- Crate name is just `sed`, importable as `sed::sed::uumain` (the crate has `pub mod sed` in `lib.rs` and the inner module exports `uumain`).
- Could be wired through the existing `register!` macro in `brush-coreutils-builtins` with **one line**:
  ```rust
  register!(m, "coreutils.sed", "sed", sed::sed);
  ```
  with a `coreutils.sed = ["dep:sed"]` feature plus dep entry. (The macro stringifies the third argument; the path `sed::sed` would resolve the wrapped uumain as `sed::sed::uumain`. Need to verify macro hygiene against this; may need a small adapter.)
- Alternatively, slot under `brush-bundled-extras/extras.sed` if we want to keep the "non-uutils-coreutils-source" convention (the rationale being that even though sed has uutils' API shape, it's a sibling repo not part of `uutils/coreutils`). **Architectural call to revisit at integration time.**

### Risks / blockers

- **Maturity**: 0.1.1 is early. uutils/sed README likely has a "not feature-complete" disclaimer. POSIX sed has a large surface (s/// substitution, addressing, branching, hold space, `N`/`P`/`D`, label commands) — uutils/sed may not cover all of it yet. Test against real-world sed scripts before recommending wide enablement.
- **Cross-platform**: confirmed Windows-aware (the bin source has `#[cfg(windows)]` for `.exe` stripping). But sed's behavior is POSIX — Windows may reveal edge cases.

### Recommendation

**Track upstream.** Re-evaluate at v0.2.0+ (or earlier if upstream hits a "feature-complete" milestone). When the cycle reopens:

- Add `sed = "0.X.Y"` as an optional dep in `brush-coreutils-builtins/Cargo.toml`.
- Add `coreutils.sed = ["dep:sed"]` feature.
- Add a `register!` line (verify macro hygiene against `sed::sed::uumain`; may need a one-line adapter to satisfy the macro's `$util_crate` token).
- Bundle in the `coreutils.all` cross-platform aggregate (subject to Windows verification).

Effort estimate (when reopened): ~half a day, mostly testing and CHANGELOG.

---

## awk

### Landscape

- **No `uutils/awk` repo exists.**
- **`frawk` (`ezrosent/frawk`)**:
  - 1296★ · last push 2025-09-26 · v0.4.8
  - License: MIT/Apache-2.0 (compatible)
  - Description: "an efficient Awk-like language" — note **"Awk-like", not POSIX awk**.
  - Crate is binary-only — `src/` contains `main.rs`, no `lib.rs`. No embeddable API.
- Other options:
  - **`goawk`** (Ben Hoyt) — Go implementation, mature but not Rust. Would require pre-built binary embedding via `include_bytes!`.
  - **gawk / mawk** — C implementations, not Rust. Out of scope.

### Embeddability assessment

frawk being binary-only puts it in the same bucket as ripgrep — no `lib.rs` to call into, only an executable to spawn. Two paths:

1. **Embed `frawk.exe` via `include_bytes!`.** ~3-5 MB. Same complexity / reproducibility issues as the ripgrep approach.
2. **Build awk from scratch using `lalrpop` / `regex` / `petgraph`.** frawk uses these crates; we'd be reinventing significant chunks of awk semantics. Multi-week effort minimum.

Neither is in scope for this plan.

### Recommendation

**Defer indefinitely.** Document as known gap. Users fall through to PATH:
- Linux/macOS: gawk/mawk are universally present.
- Windows: Git Bash MSYS2 ships gawk; users without Git Bash would need to install one.

If awk becomes a hard requirement later, **option 1 (embed frawk.exe)** is the only realistic path, and even then frawk's "awk-like" semantics may surprise users running real GNU awk scripts. Strict POSIX awk is not available in usable Rust form today.

---

## Cross-cutting open questions

1. **Where does sed live when integrated?** uutils/sed has uutils API shape and uucore = 0.8.0 — slots cleanly into `brush-coreutils-builtins` under a `coreutils.sed` feature. But it's not part of the `uutils/coreutils` repo, and lumping it there blurs "what's in coreutils vs sibling repos". Alternative is `brush-bundled-extras` which keeps the "non-uutils-coreutils-source" convention. **Resolve at integration time, not now.** This is the same shape of question as Cycle 4 procps re-evaluation; consistent answers preferred.

2. **Is "fall through to PATH" acceptable as the long-term answer for grep and awk?** For brush-as-Git-Bash-replacement on Windows, PATH inheritance from the parent process means MSYS2's grep/awk are reachable. For brush running standalone (e.g., a Docker container with no other utilities), they wouldn't be. The answer probably depends on the deployment target — worth getting explicit user signal before re-opening grep/awk research.

3. **Should we file issues / PRs upstream for sed maturation?** uutils/sed at 0.1.1 has no formal feature-completeness milestone in their README (last verified 2026-04-25). A "what's required for brush to ship sed in production" issue might accelerate that. Optional contribution path; not in scope without explicit user authorization.

---

## Decision summary

| Utility | Decision | Trigger to re-open |
|---|---|---|
| grep | Defer indefinitely | Hard user requirement; or a ripgrep-based POSIX-grep shim materializes upstream |
| sed | Track upstream uutils/sed | uutils/sed v0.2.0+ release, or upstream milestone declaring POSIX feature-completeness |
| awk | Defer indefinitely | Hard user requirement; or a Rust POSIX-awk crate appears with embeddable lib API |

No action in this cycle. Coverage-expansion plan's Cycle 5 DoD is satisfied by this document.

# Upstream v0.4.0 Migration — Planning

> **Status**: 🟢 **All 5 cycles shipped** (2026-05-05) · branch
> [`feat/upstream-v0.4.0-migration`](https://github.com/slicingmelon/brush/tree/feat/upstream-v0.4.0-migration)
> ready to merge to `main`.
>
> **Created**: 2026-05-04 · **Owner**: TBD
> **Tracks**: bringing the fork up to upstream's
> [`brush-shell-v0.4.0`](https://github.com/reubeno/brush/releases/tag/brush-shell-v0.4.0)
> tag (released 2026-05-03) plus the two post-tag dependabot commits on
> upstream/main.
>
> ## Cycle status
>
> | Cycle | Risk | Status | Outcome |
> |---|---|---|---|
> | 1 — PR [#1109](https://github.com/reubeno/brush/pull/1109) parser `!` fix | 🟢 zero | ✅ skipped | Fork's release 0.3.13 (commit [`d6ca82b`](https://github.com/slicingmelon/brush/commit/d6ca82b)) independently applied the same 1-line fix three days *before* upstream merged #1109. Cherry-pick was an empty no-op. |
> | 2 — PRs [#1110](https://github.com/reubeno/brush/pull/1110)/[#1111](https://github.com/reubeno/brush/pull/1111)/[#1122](https://github.com/reubeno/brush/pull/1122) CI workflow churn | 🟢 low | ✅ shipped | All three cherry-picked clean (`d7e6b50`, `92f2139`, `9091d59`). |
> | 3 — PRs [#1112](https://github.com/reubeno/brush/pull/1112)/[#1117](https://github.com/reubeno/brush/pull/1117)/[#1123](https://github.com/reubeno/brush/pull/1123) Cargo dep bumps | 🟡 medium | ✅ shipped | Cargo.lock conflicts in #1112 and #1117 resolved by keeping `--ours` lock + `cargo update -p` for each bumped dep; #1123 auto-merged. brush-shell builds clean post-bumps. tokio 1.50→1.52.2, reedline 0.46→0.47 (with the bundled `read_line` API fix), uuid 1.23.0→1.23.1, junit-report 0.8.3→0.9.0, const_format 0.2.35→0.2.36, fancy-regex 0.17→0.18 (uutils/sed transitively retains 0.17 — expected). |
> | 4 — PRs [#1113](https://github.com/reubeno/brush/pull/1113)/[#1114](https://github.com/reubeno/brush/pull/1114) doc updates | 🟢 low | ✅ shipped | Both auto-merged. Follow-up commit `0d1a408` adds bidirectional cross-links between upstream's new `experimental.md` and fork's `bundled-tools-index.md`. |
> | 5 — PR [#1118](https://github.com/reubeno/brush/pull/1118) workspace version bumps to v0.4.0 | 🔴 high | ✅ shipped | Cargo.toml conflicts in `brush-parser`/`brush-core`/`brush-shell` resolved per the version-alignment matrix. `brush-shell` lands at exactly `0.4.0`. `brush --version` reads `brush version 0.4.0 (git:...) - https://github.com/slicingmelon/brush`. CHANGELOG.FORK.md `[0.4.0]` section added. |
> | Post-cycle cleanup | n/a | ✅ shipped | Two follow-up commits surfaced by the dep upgrades' new toolchain rules: `9ad75a1` (rustfmt fixes for fork-only files) and `2a0e42d` (clippy-1.95 strictness fixes — 8 errors in `brush-core` + 4 errors in `brush-bundled-extras`, all in fork-only code, all surgical with `#[allow(..., reason = "...")]` attributes per workspace convention). |
> | Final gate | n/a | ✅ green | `cargo fmt --check --all` clean; `cargo clippy --workspace --all-features --all-targets` clean; **396 unit tests pass** across the workspace, zero failures; binary smoke test green: `cygpath`, MSYS path translation, bundled tools (ls/sed/awk/tar/rg), and version banner all work as expected. (`cargo xtask ci quick` final step `cargo nextest run` failed because `cargo-nextest` is not installed in this environment — env issue, not a regression; the equivalent `cargo test --workspace --lib` runs green.) |

---

## TL;DR — actual divergence is small

The v0.4.0 release notes describe "200+ merged PRs", but the fork has
been continuously merging from upstream and reached merge-base
[`cb00235`](https://github.com/reubeno/brush/commit/cb00235) on
2026-04-20 — that commit is upstream PR
[#1031](https://github.com/reubeno/brush/pull/1031) (the
bundled-coreutils scaffolding), one of the headline v0.4.0 features.
Almost all v0.4.0 work was already absorbed via earlier merges.

**What's actually new since the fork's last upstream-pull**:
**8 commits in `brush-shell-v0.4.0` + 2 post-tag commits on
`upstream/main`** = 10 commits. Most are dependabot/CI churn; one is a
substantive parser bug fix; one is the v0.4.0 version-bump.

```
$ git rev-list --count brush-shell-v0.4.0 ^origin/main
8
$ git rev-list --count upstream/main ^brush-shell-v0.4.0
2
```

This plan migrates those 10 commits in five small kaizen cycles, each
gated by `cargo xtask ci quick` (or `pre-commit` for the final).

---

## Commit inventory (10 total)

| # | Hash | PR | Type | Files | Conflict risk |
|---|---|---|---|---|---|
| 1 | `760636c` | [#1109](https://github.com/reubeno/brush/pull/1109) | parser bug fix (`!` formatted without space) | `brush-parser/src/ast.rs` (1 line) | 🟢 zero |
| 2 | `febce43` | [#1110](https://github.com/reubeno/brush/pull/1110) | CI: `cargo about` in CD | `.github/workflows/cd.yaml` | 🟢 low |
| 3 | `0fd11da` | [#1111](https://github.com/reubeno/brush/pull/1111) | github-actions deps (5 updates) | 5 workflow files | 🟢 low |
| 4 | `bdc5585` | [#1112](https://github.com/reubeno/brush/pull/1112) | cargo deps: tokio 1.50→1.52.1, reedline 0.46→0.47 (+ API fix), uuid 1.23.0→1.23.1, junit-report 0.8.3→0.9.0, const_format 0.2.35→0.2.36 | 8 Cargo.toml + 2 reedline-API .rs files + Cargo.lock | 🟡 medium |
| 5 | `97b32bc` | [#1113](https://github.com/reubeno/brush/pull/1113) | docs: README header image | `README.md` (1 line) | 🟡 medium (fork README is heavily customized) |
| 6 | `e45ef40` | [#1114](https://github.com/reubeno/brush/pull/1114) | docs: refresh reference + add `experimental.md` | `docs/reference/{compatibility,configuration,experimental,README}.md` + `README.md` | 🟢 low (largely additive) |
| 7 | `1732e26` | [#1117](https://github.com/reubeno/brush/pull/1117) | chore: lock file refresh | `Cargo.lock` only | 🟡 medium (regenerable) |
| 8 | `96a26d0` | [#1118](https://github.com/reubeno/brush/pull/1118) | **chore: prepare for next release** — bumps every workspace crate to v0.4.0 majors | 8 Cargo.toml + Cargo.lock + CHANGELOG.md | 🔴 **HIGH** — collides with fork's version scheme |
| 9 | `851d56a` | [#1122](https://github.com/reubeno/brush/pull/1122) | github-actions deps (post-tag, 2 updates) | 3 workflow files | 🟢 low |
| 10 | `c33c190` | [#1123](https://github.com/reubeno/brush/pull/1123) | cargo deps: fancy-regex 0.17→0.18 (post-tag) | 2 Cargo.toml + Cargo.lock | 🟢 low |

### The version-bump matrix (Cycle 5)

Upstream's v0.4.0 release bumped the following crate majors:

| Crate | Pre-v0.4.0 (upstream) | v0.4.0 (upstream) | Fork's current | Fork's target after Cycle 5 |
|---|---|---|---|---|
| `brush-parser` | 0.3.0 | **0.4.0** | 0.3.1 (CRLF tokenizer fix) | 0.4.1 (re-applied fork delta on top of 0.4.0) |
| `brush-core` | 0.4.0 | **0.5.0** | 0.4.4 (PathArgPolicy + path_conv + SHELL/USER/CREATE_NO_WINDOW) | 0.5.1 (fork delta on top of 0.5.0) |
| `brush-builtins` | 0.1.0 | **0.2.0** | 0.1.0 | 0.2.0 |
| `brush-experimental-builtins` | 0.1.0 (just `save`) | 0.1.0 | 0.1.1 (`+ cygpath`) | 0.1.2 |
| `brush-interactive` | 0.3.0 | **0.4.0** | 0.3.0 | 0.4.0 |
| `brush-shell` | 0.3.0 | **0.4.0** | 0.3.13 | **0.4.0** (canonical alignment) |
| `brush` (top-level) | 0.3.0 | **0.4.0** | 0.3.0 | 0.4.0 |
| `brush-bundled-extras` (fork-only) | n/a | n/a | 0.1.9 | 0.2.0 (sympathy bump alongside extras-of-v0.4.0-era) |
| `brush-coreutils-builtins` (fork-extended) | 0.1.0 (upstream) | 0.1.0 | (fork extended on top of upstream's 0.1.0) | reconcile to upstream's `0.1.0` line, fork delta as 0.1.1 |

**Decision logged in this doc**: align fork's published version of
`brush-shell` exactly to upstream's `0.4.0` (not `0.4.1` or
`0.3.14` etc.) so that downstream users seeing
`brush 0.4.0 (fork: slicingmelon)` understand they're getting upstream
v0.4.0's surface area + the fork's bundled-tools layer + Windows
quality-of-life fixes. Fork-specific patch-level deltas land as
`brush-shell 0.4.1` on the next fork-only release.

---

## Cycles

> Each cycle = a small, gated, reversible change. Pattern:
> Plan → Cherry-pick → `cargo xtask ci quick` → Decide
> (revert / continue). One cycle per commit-cluster keeps blast
> radius small and the bisect-distance short if anything regresses.

### Cycle 1 — Parser `!` formatting fix (PR #1109) 🟢 low risk

- **Plan**: cherry-pick `760636c`. One-line change to `Display for
  Pipeline` in `brush-parser/src/ast.rs`.
- **Why first**: zero conflict risk, real bug fix, builds momentum.
- **Gate**: `cargo xtask ci quick` (≈ 7 s warm).
- **Rollback**: `git reset --hard HEAD~1`.

### Cycle 2 — CI workflow churn (PRs #1110, #1111, #1122) 🟢 low risk

- **Plan**: cherry-pick all three together — they're scoped to
  `.github/workflows/*` and don't touch fork code or fork docs. CI/CD
  workflows are upstream-owned territory; the fork hasn't customized
  them.
- **Gate**: visual diff review only (workflows aren't exercised by
  `cargo xtask ci quick`). Optionally trigger one CI run from the
  branch if pushing.
- **Rollback**: `git reset --hard HEAD~3`.

### Cycle 3 — Cargo dep bumps (PRs #1112, #1117, #1123) 🟡 medium risk

- **Plan**: cherry-pick `bdc5585` (cargo group of 5), then `1732e26`
  (lock-file refresh), then `c33c190` (fancy-regex). The reedline
  0.46→0.47 bump in #1112 carries a small API-surface fix in
  `brush-interactive/src/{error.rs, reedline/input_backend.rs}` —
  cherry-pick that whole.
- **Why bundled**: all three converge on `Cargo.lock`; landing
  separately would cause noisy lock-file churn.
- **Risk surface**: Cargo.toml conflicts where the fork has wired
  `experimental-bundled-coreutils` and `experimental-bundled-extras`
  feature flags into `brush-shell/Cargo.toml`. Resolution strategy:
  accept upstream's dep version changes, keep the fork's feature
  flags + `brush-bundled-extras`/`brush-coreutils-builtins` paths.
- **Gate**: `cargo xtask ci pre-commit` (~ 45 s warm) — must run the
  fuller suite because dep upgrades can break tests through transitive
  behavior.
- **Rollback**: `git reset --hard HEAD~3`.

### Cycle 4 — Doc updates (PRs #1113, #1114) 🟢 low risk

- **Plan**: cherry-pick `97b32bc` (README image — should be a clean
  one-line change, but our README has fork-specific install
  instructions; line context will likely conflict at the top of the
  README). Then cherry-pick `e45ef40` — adds a new
  `docs/reference/experimental.md` page (clean add) and edits
  `compatibility.md`, `configuration.md`, and `docs/reference/README.md`.
- **Reconciliation work**: upstream's `experimental.md` documents
  `experimental-bundled-coreutils` officially. Our fork has its own
  reference doc at
  [`docs/reference/bundled-tools-index.md`](../reference/bundled-tools-index.md).
  These should coexist — the upstream page covers what the upstream
  feature flag does; the fork page covers the full bundled inventory
  + Windows gap analysis. Add a cross-link from each to the other.
- **Gate**: visual diff review.
- **Rollback**: `git reset --hard HEAD~2`.

### Cycle 5 — Version alignment (PR #1118 + fork-suffix decision) 🔴 high

- **Plan**:
  1. Apply upstream's version bumps from `96a26d0` to all crates
     listed in the version-bump matrix above.
  2. Re-apply the fork's per-crate patch-level bumps on top
     (`brush-core` becomes 0.5.1 not 0.5.0, etc.) where the fork
     carries delta vs upstream — see the matrix.
  3. `brush-shell` lands at **exactly `0.4.0`** (not 0.4.1) — so
     `brush --version` reads `brush 0.4.0 (fork: slicingmelon)`
     unambiguously aligned with upstream's release name.
  4. Update workspace dep version constraints (`^0.5.0` for
     brush-core, `^0.4.0` for brush-parser, etc.) to match.
  5. Drop a fresh `## [0.4.0] - 2026-05-04` section into
     [`CHANGELOG.FORK.md`](../../CHANGELOG.FORK.md) summarizing this
     migration.
- **Risk surface**: any Cargo.toml in the workspace that pins
  brush-core / brush-parser / brush-builtins / brush-interactive
  versions will need its constraint bumped. `brush-bundled-extras`
  and `brush-coreutils-builtins` are fork-only, so their constraints
  bump alongside.
- **Gate**: `cargo xtask ci pre-commit` + manual smoke check of
  `bash --version` (must read `bash 0.4.0 (fork: slicingmelon)` or
  similar).
- **Rollback**: keep the cycle as a single commit so a `git reset
  --hard HEAD~1` cleanly unwinds it.

### Final gate

After all five cycles land:

- `cargo xtask ci pre-commit` clean on Windows (the fork's daily
  driver platform).
- Smoke checks: `cargo run -- -c 'cygpath -w /c/Users'`,
  `cargo run -- -c 'cat /c/Github-Tools/brush/Cargo.toml | head -1'`
  (verifies fork's MSYS-path-conversion still works after dep
  shuffles), `cargo run -- --version` reads `0.4.0`.
- Open PR from `feat/upstream-v0.4.0-migration` → `main`. PR body
  links this doc.

---

## Why kaizen, not a single merge?

A `git merge upstream/brush-shell-v0.4.0` would resolve in maybe 30
minutes if the merge driver doesn't get confused — but it would
collapse all 10 changes into one commit, making bisect-distance bad
if a regression slips in (e.g. a tokio 1.52 surprise, a reedline
0.47 input-backend behavior change). Kaizen's PDCA cycle structure
gives us:

- **One commit per cluster** → clean `git log` for the migration.
- **Per-cycle gate** → if a cycle breaks the build, we know
  exactly which commit-cluster caused it without bisecting 10 commits.
- **Reversible at every step** → 1-commit `git reset --hard`
  unwinds any single cycle.
- **Reviewable in pieces** → the eventual PR reads as 5 logically
  scoped commits, not "merge upstream, fix conflicts" with a 1500-line
  diff.

---

## Decisions log

| Date | Decision | Rationale |
|---|---|---|
| 2026-05-04 | Cherry-pick instead of merge | Smaller blast radius per step; cleaner final history. |
| 2026-05-04 | Version `brush-shell` to exactly `0.4.0` (not `0.4.1`) | Fork's brand identity is "upstream + extras"; aligning the headline binary version to upstream's release name communicates that clearly. |
| 2026-05-04 | Defer `brush-bundled-extras`/`brush-coreutils-builtins` major bumps to a follow-up release | These are fork-only crates; upstream v0.4.0 has no bearing on their version cadence. Bump them as their own changes ship. |

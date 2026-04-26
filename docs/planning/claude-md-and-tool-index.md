# Project memory (CLAUDE.md) + Bundled-Tool Index

> **Status**: ✅ shipped · **Created**: 2026-04-26 · **Owner**: @slicingmelon
> **Branch**: `docs/claude-md-and-tool-index`
> **Tracks**: a project-local `.claude/CLAUDE.md` (so future Claude Code
> sessions get the right context the moment they enter the repo) plus an
> auditable index of every command brush ships when built with the
> recommended Windows install line, cross-checked against Git-for-Windows.

## Why this exists

This fork is built primarily to be the user's daily-driver bash on
Windows — installed at `~/.cargo/bin/bash.exe` and pointed to by Claude
Code via `CLAUDE_CODE_GIT_BASH_PATH`. Two recurring questions need a
single source of truth:

1. **What does Claude Code see when it lands in this repo?** A
   project-scoped `CLAUDE.md` answers: what brush is, where the
   feature flags live, the MSRV split, the fork-vs-upstream story,
   the planning-doc layout, and which subagents actually help here.
2. **Which utilities does the brush install actually carry, and which
   does it fall through to PATH for?** When the user types `sed` /
   `awk` / `grep` / `find` they hit the bundled Rust implementation;
   `tar`, `ssh`, `git`, `perl`, `column`, `expr` (Windows build), …
   fall through to Git-for-Windows on `PATH`. Confusing the two
   wastes time.

The operative fact for the gap analysis: brush is invoked from a
Windows shell environment (typically PowerShell or cmd), so it
inherits Windows `PATH` — which does **not** include
`C:\Software\GitForWindows\usr\bin`. Git-Bash itself prepends
`/usr/bin` and `/mingw64/bin` via MSYSTEM init, but brush bypasses
that. So a binary present in `usr\bin` is **not on brush's PATH**
unless explicitly added. "GfW has it, brush doesn't bundle it" is a
real gap from brush's point of view, even though the file exists on
disk.

## Deliverables

1. **`.claude/CLAUDE.md`** — project memory loaded automatically by
   Claude Code in every session that opens this repo. Generated via the
   `/init` skill, then audited and tightened with `claude-md-improver`.
   Lives in `.claude/` rather than the repo root because (a) the user
   asked for it explicitly, and (b) keeping the root clean matters for a
   public fork — root-level `CLAUDE.md` would show up in every
   GitHub directory listing.
2. **`docs/reference/bundled-tools-index.md`** — single combined doc
   covering both the auditable matrix of every command brush registers
   under the recommended Windows install line
   (`--features experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras`)
   *and* the gap analysis against Git-for-Windows. Driven by parsing
   `brush-coreutils-builtins/Cargo.toml`, `brush-bundled-extras/Cargo.toml`,
   and `brush-builtins/src/*.rs`, then diffed against
   `dir C:\Software\GitForWindows\usr\bin` and
   `dir C:\Software\GitForWindows\mingw64\bin`. Gap section splits by
   category:
    - *On Windows PATH already* — non-issue (e.g., `git`, `ssh`,
      `curl`, `gpg`).
    - *In `usr\bin` but not on Windows PATH* — real gaps from brush's
      point of view (e.g., `tar`, `column`, `getopt`, `nano`, `vim`).
    - *MSYS-internal helpers* — irrelevant by design (cygwin compat
      shims, `cygpath`, `mount`, `mkpasswd`).

   Originally planned as two separate docs (index + gap) but combined
   into one for a single navigation point. If the gap section grows
   into a backlog with its own cadence, split it out then.

## Plan / sequence

1. ✅ Branch `docs/claude-md-and-tool-index` created off
   `claude/hardcore-visvesvaraya-6489be` (the parent worktree branch).
2. ✅ This planning doc written.
3. ✅ Tool index built (`docs/reference/bundled-tools-index.md`) by
   parsing the three Cargo.toml feature lists + the
   `brush-builtins/src/*.rs` file inventory; gap analysis against
   `mingw64\bin` and `usr\bin` listings included.
4. ✅ `.claude/CLAUDE.md` generated via the `/init` skill (lives on
   disk only — gitignored by the user's global rule, intentionally
   kept out of the public fork).
5. ✅ Audited and revised via `claude-md-improver` skill.
6. ✅ Reflexion pass caught four issues (cross-platform uutils count
   was 82 → corrected to 83; `egrep`/`fgrep` framing softened from
   "probable bug" to "deliberate omission"; worktree section in
   CLAUDE.md removed for hardcoded user path; Windows-vs-Unix
   masked-builtins count clarified). All fixed before commit.
7. ✅ Two commits on this branch:
    - `7da6539` — this planning doc
    - `078bd5b` — tool index + Git-for-Windows gap analysis +
      `docs/reference/README.md` link entry

## Constraints worth recording

- **`.claude/CLAUDE.md` over root `CLAUDE.md`.** User-explicit
  preference; keep the root tidy for the public fork README story.
- **MSRV split is load-bearing.** Workspace MSRV is 1.88.0; only the
  `experimental-bundled-extras-fastgrep` flag carries the rustc ≥ 1.92
  bump (via `awnion/fastgrep`'s requirement). Both reference docs and
  CLAUDE.md must surface this — it's the single most-likely "why
  doesn't this build?" hit on a stock toolchain.
- **The bundled-extras umbrella inherits fastgrep's MSRV transitively.**
  So the recommended one-line install requires rustc ≥ 1.92. Already
  documented in `README.md` and the CHANGELOG; CLAUDE.md should point
  there rather than duplicate.
- **Two binaries per install.** `brush.exe` and `bash.exe` ship side by
  side from the same crate — the `bash` alias exists so this fork can
  be a drop-in `CLAUDE_CODE_GIT_BASH_PATH` target. CLAUDE.md must
  call this out so a future agent doesn't try to "fix" the duplication.

## Out of scope

- Changes to the install line, the feature flag set, or the upstream
  pin versions. This is a docs-only branch.
- Adding utilities from the gap list. That's separate work — gap doc
  exists to feed it; new cycles in `posixutils-rs-integration.md` or a
  fresh planning doc would actually execute it.
- Changing the upstream `reubeno/brush` README story. The fork install
  block in the README is owned by `CHANGELOG.FORK.md` + the
  posixutils-rs-integration plan; CLAUDE.md just *references* those.

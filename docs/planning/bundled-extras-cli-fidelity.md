# bundled-extras CLI fidelity — Planning

> **Status**: 🚧 **Layer 1 in flight** — Layer 1 (this branch) ships the
> `clap`-derive parser + mode-aware behavior for `rg` / `ripgrep` /
> `grep` / `egrep` / `fgrep`. Layer 2 (vendor `BurntSushi/ripgrep`
> `crates/core/` as a workspace member) is queued behind explicit
> approval.
>
> **Created**: 2026-05-05 · **Owner**: @slicingmelon
> **Branch**: `feat/bundled-extras-cli-fidelity`
>
> **Tracks**: closing the agent-friction "unknown option" gaps that
> appeared after [`bundled-extras-coverage-expansion.md`](./bundled-extras-coverage-expansion.md)
> Cycle 3 shipped the hand-rolled `ripgrep_adapter`. Specifically:
> agents probe `rg -t rust 'pat'`, `grep --exclude-dir=.git`,
> `rg -S pat`, `rg --column`, `rg -j 4`, etc. — all of which the
> hand-rolled adapter rejected.

## Why this plan exists (the user-visible problem)

The Cycle 3 ripgrep adapter (`brush-bundled-extras/src/ripgrep_adapter.rs`,
705 lines pre-rewrite) hand-rolled an argv parser supporting ~30 flags.
Real ripgrep has 100+; real GNU grep has more. When agents (Claude
Code's Bash tool, Cursor's terminal, automation harnesses) invoked
`rg -t rust pat path` or `grep --exclude-dir=.git pat path`, the
adapter responded with `grep: unknown option: -t` and the agent
either fell back to PowerShell or emitted a broken plan. Both wasted
tokens.

Two additional gaps:

1. **`ripgrep` (the canonical name) was not registered** — only `rg`.
   Agents that prefer the full name (`command -v ripgrep`) hit
   "command not found".
2. **`grep` was wired to the same engine as `rg`** — meaning
   `grep PATH/TO/DIR` quietly auto-recursed and honored `.gitignore`
   instead of erroring out. GNU grep doesn't recurse without `-r`.
   Subtly wrong-feeling for shell scripts.

## Five-Whys root cause

| Why | Answer |
|---|---|
| 1. Why are flags missing? | Adapter hand-rolls argv parsing, supports only a curated subset. |
| 2. Why not delegate to upstream's parser? | ripgrep has no published `pub fn main(args)` (bin-only on crates.io); awk-rs exposes lib components but no CLI driver. |
| 3. Why not vendor upstream source? | Cycle 3 deliberately picked "minimal-code adapter" over vendoring (~500 vs ~5000 lines). |
| 4. Why was that the wrong call? | The hidden cost (agent friction) wasn't budgeted into the trade-off. |
| 5 (root). | Cycle 3 assumed agent workloads only need a small flag subset. They don't — agents fluently use the full ripgrep flag matrix. |

## Cycles at a glance

| Layer | Scope | Source | Effort | Risk | Ships separately? |
|---|---|---|---|---|---|
| **1** (this branch) | `clap`-derive parser; full flag matrix recognition; mode-aware behavior (`grep` GNU defaults vs `rg` ripgrep defaults); `ripgrep` alias registration; smoke tests | Reuses existing `regex` + `pcre2` + `ignore` + adds `globset` + `clap` | ~1 day | Low | Yes |
| **2** (queued) | Vendor `BurntSushi/ripgrep` `crates/core/` as `brush-vendored-ripgrep` workspace member with `pub fn rg_main(args)` — full ripgrep CLI fidelity | UNLICENSE+MIT, vendored from upstream tag | ~3–5 days | Medium-high | Yes |
| **3** (deferred) | Same shape for `awk`: vendor `frawk` or contribute a `pub fn run(args)` upstream to `awk-rs` for full gawk extensions | TBD | TBD | Medium | Future plan |

## Layer 1 — what shipped on `feat/bundled-extras-cli-fidelity`

### Adapter rewrite

[`brush-bundled-extras/src/ripgrep_adapter.rs`](../../brush-bundled-extras/src/ripgrep_adapter.rs)
swaps its hand-rolled argv loop for a `clap`-derive `Cli` struct. The
struct enumerates the agent-relevant flag matrix; clap handles
`--flag VALUE` / `--flag=VALUE` / bundled shorts / value-terminator
positionals automatically.

Mode-aware behavior is bound at the entry-point layer:

| Mode | gitignore? | smart-case default? | dir without `-r` | `-s` means |
|---|---|---|---|---|
| `Mode::Rg` | yes | off (opt-in via `-S`) | auto-recurse | `--case-sensitive` |
| `Mode::Grep` | no | off | error "Is a directory" | `--no-messages` |
| `Mode::Egrep` | no | off | error | `--no-messages` |
| `Mode::Fgrep` | no | off | error | `--no-messages` |

GNU-only shortcuts (`-NUM` for `-C NUM`, `-y` synonym for `-i`) are
handled by `preprocess_argv` running before clap. `-s` is rewritten
based on mode so a single clap schema can serve both worlds.

### Engine improvements

- `-o` / `--only-matching` now emits every non-overlapping match in a
  line (previously only the first). Implemented via the new
  `engine_find_all` helper.
- Smart-case (`-S`) only forces case-insensitive when the pattern
  contains no uppercase characters.
- `-t TYPE` / `--type-list` / `--type-add` / `--type-clear` use
  `ignore::types::TypesBuilder::add_defaults` — the same definitions
  ripgrep itself ships.
- Glob filters (`-g`, `--iglob`, `--include`, `--exclude`,
  `--exclude-dir`) wire through `ignore::overrides::OverrideBuilder`.

### Bug fix

The previous binary-detection peek used `f.try_clone()` and read 8 KiB
from the clone before passing the original handle to a `BufReader`.
On Windows, the cloned handle shared the file position with the
original — so `BufReader` was reading from byte 8192 of a 12-byte
file, producing zero matches. Fixed by using a separate `File::open`
for the peek (two opens per file, but cheap relative to search work).

### Smoke tests

[`brush-shell/tests/bundled_extras_smoke.rs`](../../brush-shell/tests/bundled_extras_smoke.rs)
spawns the built `brush.exe` via `assert_cmd::cargo_bin!` and exercises
38 scenarios across the four bundled tool families. Gated by
`required-features = ["experimental-bundled-extras"]` so it only
builds when the feature is on.

Coverage:

- Name registration: `rg`, `ripgrep`, `grep`, `egrep`, `fgrep`,
  `awk`, `sed`.
- ripgrep flag matrix: `--version`, `--type-list`, `-S`, `-P` (PCRE2
  lookahead), `-j N`, `--passthru`, `-A`/`-B`/`-C`, `-o` (multi-match
  per line), `-c`, `-v`, `-w`, `-F`, `-m`.
- GNU grep semantics: dir without `-r` → "Is a directory"; `-r`
  recurses; `-NUM` → `-C NUM`; `-y` → `-i`; `-P` lookbehind; `-s`
  → `--no-messages`; `-E` alternation; `egrep` / `fgrep` aliases.
- sed: `s/`, `-E`, `-e` chains, `-n …p`, `Nd`.
- awk: `$N`, `-F`, `-v`, `BEGIN`, `END`.

### Acceptance gate

```
cargo test -p brush-shell --test brush-bundled-extras-smoke \
  --features 'experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras'
```

…must report **38 passed**.

---

## Layer 2 — vendor `BurntSushi/ripgrep` (queued)

### What it'd ship

A new workspace member `brush-vendored-ripgrep/` containing
`crates/core/` from a pinned ripgrep release tag, lightly patched to
expose `pub fn rg_main(args: impl IntoIterator<Item=OsString>) -> i32`
(the same shape `uutils/sed` exposes). The
`brush-bundled-extras::ripgrep_adapter::rg_main` collapses to a
one-line forward.

### Why it'd be worth it

Layer 1 recognizes 100+ flags; some it implements, others it
accepts-and-noops (`--mmap`, `--encoding`, `--pre`, `--sort`, etc.).
Agents won't see "unknown option" but they won't always get the
behavior they expect. Layer 2 gives full-fidelity behavior for those
flags too — at the cost of carrying ~5000 lines of vendored code.

### Why it isn't in Layer 1

- **Maintenance cost**: every ripgrep release would need a manual
  resync. Doable (slicingmelon/brush already rebases on reubeno/brush
  via the upstream-sync workflow in
  [`CLAUDE.md`](../../.claude/CLAUDE.md#upstream-sync-notes)) but
  non-trivial.
- **Binary size**: the vendored crate pulls in `bstr`, `serde`,
  `globset` (already a dep), `walkdir`, `grep` (the umbrella),
  `grep-cli` / `grep-printer` / `grep-searcher` / `grep-regex` /
  `grep-pcre2`. Some of these we already use; the rest add
  measurable bytes. Worth quantifying before committing.
- **Layer 1 covers 90% of the agent-friction**. The remaining 10%
  doesn't justify the maintenance burden until / unless we hear
  specific complaints.

### Decision criteria

Promote Layer 2 to "in flight" when ANY of:

- A user reports that a Layer-1-accepted-but-noop flag is required
  for their workflow (especially `--encoding`, `--pre`, `--sort`).
- ripgrep ships a structural CLI change that breaks our schema (very
  unlikely; ripgrep's flag matrix is stable).
- We reach for ripgrep features Layer 1 architecturally can't do —
  e.g., parallel multi-file search (Layer 1 is single-threaded line
  scan) or memory-mapped large-file search.

Until then: stay on Layer 1.

### Sketch of the work

1. Add a Cargo workspace member `brush-vendored-ripgrep/`. Set
   `package.publish = false` and license headers per UNLICENSE+MIT.
2. Vendor `BurntSushi/ripgrep` at a pinned tag (e.g. `15.1.0`) into
   `crates/core/` and adjust `Cargo.toml` to consume the published
   `grep`, `globset`, `ignore`, etc. crates instead of workspace-internal
   paths.
3. Lift `main.rs::main()` into `lib.rs::run(args)` returning `i32`.
4. Replace `ripgrep_adapter::rg_main` body with
   `brush_vendored_ripgrep::run(args)`.
5. Keep the Layer-1 `grep_main` / `egrep_main` / `fgrep_main` (GNU
   semantics) as separate paths — Layer 2 is rg-only.
6. Add a "vendored ripgrep" smoke test that exercises one of the
   currently-noop flags (e.g. `--sort path`, `--mmap`).

This sketch lives here so future-us can pick up without re-deriving
the strategy.

---

## Things to avoid

- **Don't replace fastgrep with the new ripgrep_adapter** as the
  `grep` provider unconditionally. Some users on rustc 1.88–1.91
  build with `experimental-bundled-extras-fastgrep` only (no
  ripgrep). Keep the [`bundled-extras-coverage-expansion.md`](./bundled-extras-coverage-expansion.md)
  Cycle 3 winner-rule (HashMap insertion order: ripgrep wins for
  `grep`/`egrep`/`fgrep` when both features are enabled; otherwise
  fastgrep wins).
- **Don't bump workspace MSRV** to add a `#[cfg]` shortcut. Layer 1
  uses only crates already in the workspace's dep graph (`clap`
  was already in for fastgrep; `globset` is the only new addition,
  via `ignore`'s transitive).
- **Don't write yet another adapter for grep that re-implements the
  same logic with a different schema.** If Layer 2 lands, the
  Layer-1 adapter should still hold the GNU-grep path — keep one
  module, one set of tests.

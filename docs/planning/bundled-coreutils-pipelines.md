# Bundled Coreutils — Pipeline Parallelism & Process Groups

> **Status**: planning · **Created**: 2026-04-25 · **Owner**: @slicingmelon
> **Tracks**: TODOs at [`brush-shell/src/bundled.rs:193-214`](../../brush-shell/src/bundled.rs#L193-L214)

## TL;DR

Two TODOs in `bundled.rs` prevent bundled coreutils from achieving parity with external binaries in pipelines. They are **not architecturally hard**, but they are **not a single 1-day fix** either. A Windows-platform gap effectively scopes one of them to a separate epic.

The plan: **three PDCA cycles**, each narrow enough to ship independently.

| Cycle | Scope | Platforms | Effort range | Confidence |
|---|---|---|---|---|
| 1 | pgid plumbing through `ExecutionContext` | Unix only (Windows is a no-op) | 0.5–1 day | High |
| 2 | Pipeline parallelism (Path A vs B by prototype) | Linux + Windows | 2–4 days | Medium |
| 3 | Windows job-control primitives | Windows only | 5–10 days (separate epic) | Low — design first |

---

## A3 — Problem Frame (verified)

### 1. Background

The fork's `experimental-bundled-coreutils` feature ships ~78 uutils utilities inside `brush.exe`, dispatched busybox-style via `brush --invoke-bundled <name>`. The author shipped a working MVP behind an experimental flag with two gaps documented inline as TODOs. Both prevent bundled coreutils from matching external-binary behavior in pipelines and under job control.

User-visible effects today:
- `cat big.txt | grep foo | wc -l` with bundled stages **serializes** — stage N+1 waits for stage N to fully complete before starting (no parallelism, no overlapped I/O).
- Ctrl-C in a pipeline containing bundled stages does not cleanly kill the whole pipeline on Linux.

### 2. Current Condition — code-grounded

#### Where TODO #1 (pgid) breaks down

- [`commands.rs:282-345`](../../brush-core/src/commands.rs#L282-L345) — `SimpleCommand` has `process_group_id: Option<i32>` (already plumbed by the pipeline at [`interp.rs:1386`](../../brush-core/src/interp.rs#L1386)).
- [`commands.rs:474-499`](../../brush-core/src/commands.rs#L474-L499) — `execute_via_builtin_in_parent_shell` constructs `ExecutionContext { shell, command_name, params }` — **drops `process_group_id`**. The shim has no way to read it back.
- [`bundled.rs:215-262`](../../brush-shell/src/bundled.rs#L215-L262) — shim builds the child `SimpleCommand` with `process_group_id` left as `None`.

#### Where TODO #2 (serialization) breaks down

- [`commands.rs:474-499`](../../brush-core/src/commands.rs#L474-L499) — builtin path returns `Ok(result.into())` → `ExecutionSpawnResult::Completed`.
- [`bundled.rs:258-260`](../../brush-shell/src/bundled.rs#L258-L260) — shim does `cmd.execute().await? → spawn_result.wait().await?`, **awaiting child to completion** before returning.
- [`interp.rs:445-523`](../../brush-core/src/interp.rs#L445-L523) — pipeline spawn loop awaits each stage's spawn before kicking the next. With a `Completed` result, "spawn" means "ran to completion." Hence serialization.
- External commands avoid this: [`commands.rs:647`](../../brush-core/src/commands.rs#L647) returns `StartedProcess` immediately after `sys::process::spawn`, before waiting.

#### Platform reality (this is the critical correction)

- **Linux/macOS**: `process_group`, `lead_session`, `take_foreground` all functional via `nix` ([`sys/unix/commands.rs`](../../brush-core/src/sys/unix/commands.rs)). TODO #1 is meaningful.
- **Windows**: [`sys/windows.rs:2`](../../brush-core/src/sys/windows.rs#L2) re-exports `sys/stubs/commands` for the entire `CommandExt` impl. In [`sys/stubs/commands.rs:35,89,93`](../../brush-core/src/sys/stubs/commands.rs#L35-L93), `process_group`, `lead_session`, and `take_foreground` are **silent no-ops**. So is `arg0` ([line 27](../../brush-core/src/sys/stubs/commands.rs#L27-L33)) — meaning the `cmd.argv0 = Some(name)` trick that [`bundled.rs:252-256`](../../brush-shell/src/bundled.rs#L252-L256) documents as fixing argv[0] in error messages **is silently dropped on Windows today.**
- TODO #2 (serialization) IS Windows-relevant: `CreateProcess` + `wait` works there; only the parallelism is missing.

#### Public-API constraint

- [`builtins.rs:148-165`](../../brush-core/src/builtins.rs#L148-L165) — `Registration` is **not** `#[non_exhaustive]`. brush-core is published at v0.4.0 and consumed externally as a library. Adding a public field IS a SemVer break for downstream consumers. Mitigations: gate the new field behind `#[non_exhaustive]` first (its own minor bump), or add it as a private field with a `with_*` builder method.

#### What's already in place

- [`results.rs:254-258`](../../brush-core/src/results.rs#L254-L258) — `From<ExecutionResult> for ExecutionSpawnResult` already exists. Path B's per-builtin migration is mechanical (`Ok(result)` → `Ok(result.into())`).
- [`interp.rs:519-523`](../../brush-core/src/interp.rs#L519-L523) — pipeline pgid is harvested from the first stage to return `StartedProcess`. Once a bundled stage returns `StartedProcess`, harvesting Just Works (on Unix).

### 3. Goal

- Bundled stages in `a | b | c` run **concurrently** with neighbours on Linux and Windows (no serialization).
- A pipeline of bundled stages obeys a single shared pgid on Linux/macOS — `kill -INT $pgid` reaches all stages.
- No regression: standalone bundled invocation, `type cat`, `help ls`, redirections, `command cat` all keep working.
- No undisclosed SemVer break to brush-core's public API.
- Windows pgid is **explicitly out of scope** for Cycles 1–2; it depends on Cycle 3 which is design-first.

(No throughput ratio target stated. Establish baseline first; target stays "no regression vs current external-binary path" until measured.)

### 3a. Explicitly out of scope (don't expect these to change)

- **EPIPE noise on Windows.** When a downstream stage closes early (`… | head -n 5`), upstream uutils stages on Windows print `brush.exe: write failed: 'standard output': Broken pipe` to stderr. Cause: Windows has no SIGPIPE; uutils logs the EPIPE instead of being silently killed by the kernel. Independent of pgid plumbing or pipeline parallelism — neither cycle will fix it. Tracked separately if it becomes user-facing enough to fix (likely needs a uutils-side patch or a brush-side stderr filter on the bundled shim).
- **uutils argv[0] error attribution on Windows.** [`bundled.rs:252-256`](../../brush-shell/src/bundled.rs#L252-L256) sets `cmd.argv0 = Some(name)` so errors render as `ls:` rather than `brush.exe:`. The Windows stub at [`sys/stubs/commands.rs:27-33`](../../brush-core/src/sys/stubs/commands.rs#L27-L33) silently drops it. So bundled errors say `brush.exe: cannot access '/nope'` on Windows today, and will keep saying that until Cycle 3 (or a narrower fix) implements real `arg0` on Windows. Documented here so reviewers don't expect Cycle 1/2 benchmarks to change error prefixes.
- **Process substitution `<(...)` on Windows.** `error: operation not supported on this platform: fd redirections` — pre-existing limitation, unrelated.
- **Symbolic signal names in `trap`** (e.g. `trap … INT`) on Windows. Rejected as `invalid signal specification`. Pre-existing, unrelated.

### 4. Root Cause — split by branch

**Branch A — Pipeline serialization**
1. Bundled stage returns `Completed`, blocks the spawn loop.
2. Builtin contract returns `ExecutionResult`, not `ExecutionSpawnResult`.
3. Builtins (`cd`, `set`, `declare`) genuinely complete synchronously; the contract was right for them.
4. Bundled-shim is the first builtin that legitimately wants to return a spawn handle.
5. Author chose lowest-risk MVP shape (re-use existing builtin dispatch) to ship behind `experimental-`.

**Branch B — pgid propagation (Unix only)**
1. Shim has no pgid to set on the child `SimpleCommand`.
2. `ExecutionContext` doesn't carry pgid.
3. Builtins that complete inline don't need pgid; only the bundled shim does.
4. Adding a field for one consumer felt like over-fitting.
5. MVP scope; deferred until someone hit the limitation.

**Branch C — Windows job control (newly identified, was missed in prior analysis)**
1. `process_group` on Windows is a stub that silently ignores its argument.
2. brush-core has never built a Windows job-control story.
3. Job Objects + `CREATE_NEW_PROCESS_GROUP` require non-trivial `windows-sys` integration.
4. Brush's compatibility focus has been Linux/macOS where `nix` covers the primitives.
5. Windows users tolerate degraded Ctrl-C behavior because the wider WSL/Git-Bash ecosystem also has it.

**Conclusion**: Cycles 1–2 are unscoped, not hard. Cycle 3 is genuinely hard and should be designed before estimated.

---

## PDCA Cycle 1 — pgid plumbing

### Plan

**Hypothesis**: If `ExecutionContext` carries `process_group_id` from `SimpleCommand`, and the bundled shim reads it onto the child `SimpleCommand`, then bundled stages will join the pipeline pgid on Unix.

**Baseline**: Before changes, run on Linux:
```sh
brush -c 'sleep 100 | sleep 100' &
ps -o pid,pgid,comm -g $! ; kill $!
```
Capture: do all stages share a pgid today? Compare bundled-led vs external-led.

**Change**:
- Add `pub process_group_id: Option<i32>` to `ExecutionContext` ([`commands.rs:36-43`](../../brush-core/src/commands.rs#L36-L43)).
- Set it at all 4 construction sites: `execute_via_builtin_in_owned_shell`, `execute_via_builtin_in_parent_shell`, `execute_via_function`, `execute_via_external` (it's effectively redundant for the last two but consistent).
- In [`bundled.rs:244-249`](../../brush-shell/src/bundled.rs#L244-L249), set `cmd.process_group_id = context.process_group_id;`

**Success criteria**:
- All `brush-compat-tests` and `brush-integration-tests` pass.
- New Linux test: bundled-led pipeline `bundled_cat | bundled_wc` shares a single pgid.
- No public API field added to `Registration` (this cycle changes only `ExecutionContext`, which is already `pub` and not `#[non_exhaustive]` either — so this *is* a SemVer change for that struct; document in CHANGELOG).
- Windows test suite still green (the change is a no-op on Windows since pgid is a stub).

### Do

Implementation order (one PR per phase, reviewable):
1. Add field to `ExecutionContext`, default `None`. Compile workspace. Fix compile errors at construction sites.
2. Plumb through `execute_via_builtin_in_*`. Run brush-compat-tests.
3. Read in shim. Run brush-integration-tests.
4. Write Linux pgid integration test.
5. Update `CHANGELOG.FORK.md` with the public-struct field addition (it's brush-core's `ExecutionContext`).

Track during Do:
- Number of construction-site updates required (predicted 4; verify).
- Whether any test required updating beyond the new one (regression sniff).
- Any unexpected `ExecutionContext { ... }` consumers in dependent crates.

### Check

Measure:
- Test suite delta: number of new tests, number of modified tests, total runtime change.
- Linux pgid test: pgid shared = ✓.
- Windows test runtime: unchanged.

Hypothesis confirmation:
- Did the shim child join the pipeline pgid on Linux? `ps -o pgid` should match the leader.
- Did `kill -INT <pgid>` reach all stages? (Manual verification on Linux.)

### Act

If successful: standardize. Move to Cycle 2.

If unsuccessful: most likely cause is that `interp.rs:519-523` harvests pgid only from the first `StartedProcess`, and the bundled shim still returns `Completed` (we haven't fixed that until Cycle 2). In that case Cycle 1's fix is *necessary but not sufficient* — the pipeline never learns the bundled leader's pgid because there's no leader spawn to harvest from. **This is expected** and surfaces as a Cycle 2 dependency. Cycle 1 can still merge as it's correct in isolation; the user-visible effect just won't show until Cycle 2 lands.

**Cycle 1 ships even if Cycle 2 is not started.** It's an internal correctness fix that costs nothing.

---

## PDCA Cycle 2 — Pipeline parallelism (Path A vs B by experiment)

> **Status**: closed 2026-04-25. Path A landed as an architectural cleanup,
> *not* as a parallelism unlock. The original hypothesis below was based on
> a misread of the orchestrator code; see Decision Log entry and the
> "What actually happened" section after this Plan block.

### Plan

**Hypothesis** *(invalidated 2026-04-25 — kept for historical context)*:
Routing the bundled shim through `execute_via_external`-shaped machinery
(so it returns `StartedProcess` instead of `Completed`) restores pipeline
parallelism without regressing standalone bundled invocation.

**Two candidate paths — decide by prototype, not prescription:**

| | Path A: `bundled_dispatch` hook on `Registration` | Path B: generalize `CommandExecuteFunc` to return `ExecutionSpawnResult` |
|---|---|---|
| Surface area | 1 new field on `Registration` + 1 new dispatch branch in `SimpleCommand::execute` | Type signature change to `execute_func`; `.into()` wrap in ~50 builtins |
| Layering | brush-core learns about `--invoke-bundled` protocol (or accepts an opaque dispatch callback) | brush-core stays clean; brush-shell uses the new return type |
| SemVer | Adds public field to `Registration` (not `#[non_exhaustive]`) — **breaks downstream** | Changes `CommandExecuteFunc` type alias — **breaks downstream** |
| Async lifetime hygiene | Hook returns `BoxFuture` borrowing `SimpleCommand` — non-trivial | Existing pattern, no new lifetimes |
| Risk to existing builtins | None | Mass mechanical edit; risk of one missed `Ok(result)` site |

**Both are SemVer breaks.** Path A pretends not to be one in informal discussion; honest accounting puts them on equal footing for that criterion. The prior recommendation that "Path A wins" was wrong.

**Experiment design**: build both prototypes on a throwaway branch, measure the same pipeline on each, then choose. Prototyping cost is bounded:
- Path A prototype: ~half day (one new field, one branch in `execute`, reuse external-spawn path).
- Path B prototype: ~half day (change one type alias, run cargo check, add `.into()` to compile errors).

**Baseline benchmark**:
```sh
hyperfine --warmup 3 \
  'brush -c "seq 1 1000000 | wc -l"' \
  'bash -c "seq 1 1000000 | wc -l"'   # external git-bash baseline
```
Plus a CPU-bound variant: `'brush -c "seq 1 100000 | sort | uniq -c | wc -l"'`.

**Success criteria**:
- All existing tests pass on both prototypes.
- Pipeline benchmark: bundled-only pipeline runtime is **lower than serial** (proves parallelism, regardless of absolute number).
- No regression on standalone bundled invocation (`brush -c 'cat big.txt > /dev/null'`).
- Mixed pipelines (`bundled | external | bundled`) work correctly.
- Final choice between A and B documented with measurements, not opinion.

### Do

**Sequencing**: prototype Path A *first*. Path B is conditional on Path A's results — see Phase 2.2 gate. Rationale: Path A has a smaller blast radius; if it passes all tests and the parallelism benchmark hits the success threshold, the comparative case for spending Path B's prototyping budget weakens. This is cheaper-experiment-first, not gut-based shortcutting — Path B remains the fallback if Path A under-delivers.

Phase 2.1 — Prototype Path A on `proto/bundled-path-a` branch:
- Add `bundled_dispatch: Option<BundledDispatch>` field to `Registration`. (Make `Registration` `#[non_exhaustive]` first as a separate prep PR — that's its own SemVer break that should land deliberately.)
- In `SimpleCommand::execute` ([`commands.rs:355`](../../brush-core/src/commands.rs#L355)), branch on `builtin.bundled_dispatch.is_some()` and route to a new `execute_via_bundled` that builds a `SimpleCommand` against `current_exe()` and calls `execute_via_external`.
- Move `DISPATCH_FLAG` const to brush-core (or pass it via the `BundledDispatch` struct as an opaque string — preferred for layering).
- Run benchmarks. Capture numbers.

Phase 2.2 — **Conditional** Path B prototype on `proto/bundled-path-b` branch:

Skip this phase if **all** of the following hold after Phase 2.1:
1. All existing `brush-compat-tests` and `brush-integration-tests` pass on Path A.
2. Bundled-only pipeline benchmark on Path A is **< 70% of the serial-baseline runtime** (i.e. parallelism is real, not noise — at least 30% improvement over today's serialized behavior).
3. No regression on standalone bundled invocation (`brush -c 'cat big.txt > /dev/null'`).
4. Mixed pipelines (`bundled | external | bundled`) work correctly on Path A.

If any of (1)–(4) fail, run Path B:
- Change `CommandExecuteFunc` return type to `Result<ExecutionSpawnResult, error::Error>`.
- Run `cargo check --workspace`. Add `Ok(result.into())` at every compile-error site.
- Update `execute_via_builtin_in_*` to no longer wrap in `.into()`.
- Update bundled shim to return `StartedProcess` directly (it already has the `cmd.execute().await` call — just remove the `.wait()`).
- Run benchmarks. Capture numbers.

Phase 2.3 — Comparative report (only if Phase 2.2 ran; otherwise just report Path A's numbers):
- Lines changed per path.
- Files touched per path.
- Runtime delta on the benchmark.
- Subjective: which is more confusing for a future contributor reading `SimpleCommand::execute`?

### Check

Measure (concrete, not adjectival):
- `wc -l` of diff per path.
- Files touched per path.
- `hyperfine` median runtime, both pipelines.
- Test suite pass/fail per path.
- For each path: is the SemVer break documentable in CHANGELOG.FORK.md as a single line, or does it need multi-paragraph explanation?

Decision rule (set up front, before measuring):
1. If only one path passes all tests → choose it.
2. If both pass and runtime delta is < 5% → choose smaller diff.
3. If runtime differs > 5% → choose faster.
4. If subjective layering judgment is split, ask in the brush upstream Discord/issues — this is also useful for upstream submission later.

### Act

If chosen path's prototype works:
- Discard the loser branch.
- Promote the winner to a clean PR with tests and CHANGELOG.
- Add Windows verification (does parallelism actually happen there? Smoke-test on a Windows runner).
- Update `bundled.rs:193-214` TODO comments — replace TODO #2 with a 2-line note pointing to the new dispatch path.

If both prototypes fail (unlikely): re-enter root-cause analysis. Likely culprit would be a deeper architectural issue with `spawn_pipeline_processes`'s assumption that "spawn" is non-blocking — possibly an issue with the `tokio::task::spawn_blocking` path for owned-shell builtins.

### What actually happened (added 2026-04-25)

Path A's prototype landed and works, but the empirical measurement
*invalidated the cycle's framing*. Pre-Path-A and post-Path-A pipelines
were equally parallel — both achieve concurrent stage execution, just
through different mechanisms.

**Empirical evidence on Windows**:

| Workload                            | Pre-Path-A (release) | Post-Path-A (debug) |
|-------------------------------------|----------------------|---------------------|
| `sleep 2 \| sleep 2 \| sleep 2`     | 2.4s                 | 2.8s                |
| `seq 1 5_000_000 \| wc -l`          | 0.39s                | 0.87s (debug noise) |

Both binaries achieve parallelism. If pre-Path-A had been serializing,
the 3-stage sleep pipeline would have run ~6s; it ran in 2.4s.

**Why the original hypothesis was wrong**:
[`commands.rs:466-490`](../../brush-core/src/commands.rs#L466-L490) —
`execute_via_builtin_in_owned_shell` wraps the builtin call in
`tokio::task::spawn_blocking` and returns
`ExecutionSpawnResult::StartedTask(join_handle)`. The pipeline
orchestrator at [`interp.rs:514-516`](../../brush-core/src/interp.rs#L514-L516)
awaits this future, but `spawn_blocking` returns immediately — the
actual builtin execution (including the shim's `wait_result =
spawn_result.wait().await?` for the brush child) runs concurrently on
the tokio blocking thread pool.

So for a 3-stage `bundled | bundled | bundled` pipeline pre-Path-A:
- Stages 1 and 2 ran in owned-shell subshells via spawn_blocking (parallel).
- Stage 3 ran in the parent shell via the inline shim (sequentially with the others' completion, but only this stage took 2s).
- Total: max(stage_durations) ≈ 2-3s, **not** sum(stage_durations) ≈ 6s.

The original Cycle 2 framing missed the `StartedTask` variant when first analyzing the dispatch table. The plan stated *"the bundled shim awaits child to completion before returning"* causes serialization — true ONLY for the last-stage-in-current-shell (lastpipe) path, which is one stage out of N.

**What Path A actually delivers**:

| Property                       | Pre-Path-A                                    | Post-Path-A                                  |
|--------------------------------|-----------------------------------------------|----------------------------------------------|
| Pipeline parallelism           | ✅ (via `spawn_blocking` + `StartedTask`)     | ✅ (via `execute_via_external` + `StartedProcess`) |
| Thread overhead per pipe stage | one `spawn_blocking` thread                   | none (direct OS spawn)                       |
| Pgid propagation               | ✅ (Cycle 1 plumbing + manual on shim)        | ✅ (via standard `execute_via_external` path) |
| Architecture                   | Bundled commands go through builtin path with shim | Bundled commands routed through same machinery as ordinary `PATH` commands — uniform |
| LoC in shim                    | ~50 (`shim_execute` does spawn+wait)          | ~10 (`shim_execute` is a defensive stub)     |

Real but smaller wins than the plan claimed. Path A is a refactor / cleanup, not a parallelism unlock.

**Phase 2.2 (Path B prototype) skipped**: Path A meets criteria (1), (3), (4) of the gating; criterion (2) ("< 70% of serial-baseline runtime, ≥30% improvement") was based on the now-invalidated hypothesis. There is no parallelism gap to close, so Path B's value would have been an even larger SemVer break (changing `CommandExecuteFunc`'s return type and `.into()`-wrapping ~50 builtins) for no functional benefit. Correctly skipped.

If chosen path passes Linux but fails Windows parallelism: Windows likely needs different handling (re-exec spawn cost dominates). Document, ship Linux-only behavior, open follow-up issue.

---

## PDCA Cycle 3 — Windows job control (deferred epic)

> ⚠️ This cycle is **design-first**. Do not start implementation until the design doc lands.

### Plan

**Hypothesis (unverified)**: Replacing the Windows stubs in [`sys/stubs/commands.rs`](../../brush-core/src/sys/stubs/commands.rs) with `windows-sys`-based Job Object integration will give brush genuine pgid semantics on Windows, including Ctrl-C propagation across pipeline stages.

**Pre-work** (separate PR, design only):
- Survey: how do `nushell`, `mvdan/sh`, and `Oils` handle Windows job control?
- Decide: Job Objects vs. process group IDs (`CREATE_NEW_PROCESS_GROUP`) vs. both.
- Decide: where does the Job Object handle live? On `Shell`? On `ChildProcess`?
- Decide: does brush want to depend on `windows-sys` directly, or proxy through an existing crate?
- Identify: which other stub no-ops in `sys/stubs/commands.rs` need to become real for Cycle 1 to actually do something on Windows? (Likely `process_group` and `lead_session` minimum.)

**Why not just plow ahead**: building Windows job control without a design doc tends to produce subtle bugs around Ctrl-C handling, process tree teardown on shell exit, and detached daemons. These are hard to test and hard to fix retroactively.

**Effort range**: 5–10 days, with high variance because the design phase will surface the actual scope.

### Do, Check, Act

Deferred until design lands. Re-open PDCA on the design doc itself.

### Out-of-scope marker

Cycles 1 and 2 explicitly **do not** wait on Cycle 3. They land for Linux/macOS users immediately. Windows users get pipeline parallelism (Cycle 2) but not improved Ctrl-C — they have the same job-control gap they have today, just not made worse.

---

## Effort & Confidence Recap

| Cycle | Effort | Risk | Required for | Reversible? |
|---|---|---|---|---|
| 1 | 0.5–1 day | Low | Cycle 2 to be useful on Linux | Yes (revert one PR) |
| 2 | 2–4 days | Medium (mostly benchmarking + Windows verify) | Pipeline UX win | Yes |
| 3 | 5–10 days + design | High (Windows surface area) | Real Ctrl-C on Windows | Partial |

Cycles 1+2 combined: **2.5–5 days**, materially more than the "1 day" claim in the prior analysis. Cycle 3 is its own thing.

---

## Open Questions (genuinely unresolved)

1. **Should `Registration` become `#[non_exhaustive]` first, as a separate prep PR?** Probably yes, regardless of which path Cycle 2 takes. It buys headroom for any future Registration field.
2. **Is upstream interested?** Cycles 1+2 fix MVP gaps that the upstream `experimental-bundled-coreutils` author also has. Worth opening a draft issue at `reubeno/brush` before doing the work to avoid duplicate effort, even if we end up shipping in the fork only.
3. **Does `current_exe()` work reliably when brush is invoked via a symlink, or via the `bash.exe` alias the fork ships?** [`bundled.rs:148-152`](../../brush-shell/src/bundled.rs#L148-L152) caches the result with `OnceLock<Option<PathBuf>>`. Worth a focused integration test for the `bash.exe → brush.exe` re-exec path specifically.
4. **Does `spawn_blocking` interaction at [`commands.rs:455-471`](../../brush-core/src/commands.rs#L455-L471) (owned-shell builtin path) interact badly with the new dispatch route?** The bundled shim only ever runs in the parent-shell path today. Worth verifying we don't break subshell `(...)` execution.

---

## Definition of Done

For Cycle 1:
- [x] `ExecutionContext.process_group_id` field exists and is plumbed at all 4 sites. *(commit `151f026`)*
- [x] Bundled shim sets `cmd.process_group_id` from context. *(commit `151f026`)*
- [x] Existing test suite green on Windows. *(`cargo test -p brush-core --lib` 2026-04-25: 70 passed, 0 failed.)* Linux green deferred to CI.
- [x] CHANGELOG.FORK.md updated under Unreleased. *(2026-04-25)*

> **DoD revision 2026-04-25**: Originally this checklist included "New integration
> test passes on Linux: bundled-led pipeline shares a single pgid." Removed —
> the commit's own description correctly notes that pgid sharing is *not
> observable* until Cycle 2 (the bundled shim still returns `Completed`, so
> [`interp.rs:519-523`](../../brush-core/src/interp.rs#L519-L523) never harvests
> the bundled leader's pgid). Writing a black-box pgid-sharing test against
> Cycle 1 alone would have to be `#[ignore]`d until Cycle 2 lands — adding
> dead test code is worse than deferring. The integration test is now
> Cycle 2's responsibility (see Cycle 2 DoD below).

For Cycle 2 — **landed as architectural cleanup, not parallelism unlock**:
- [x] Path A prototype built and measured. *(commit `8936677` on `proto/bundled-path-a`, cherry-picked as `86a8c1c`.)* Phase 2.2 (Path B prototype) explicitly skipped — see "What actually happened" above. The gating criterion ("≥30% parallelism improvement") was based on the now-invalidated hypothesis; pre-Path-A was already parallel via `spawn_blocking` + `StartedTask`. There was no parallelism gap for Path B to close, so its larger SemVer break would have been pure cost.
- [x] Single-path report written up in this doc's "What actually happened" section above and in the Decision Log.
- [x] Chosen path landed (Path A). `Registration` also marked `#[non_exhaustive]` and the helper factory `raw_builtin()` added so brush-shell can construct shim registrations without a struct literal — addresses the open question on `#[non_exhaustive]` deferred from earlier.
- [x] `bundled.rs:193-214` TODOs removed. The shim's `shim_execute` is now an unreachable defensive fallback; new doc comment supersedes the prior TODO block. Path A's routing handles both old TODOs (pgid + serialization, the second of which turned out to be a non-issue).
- [ ] Pipeline benchmark added to `brush-shell/benches/` — **skipped with rationale**: the empirical timings in "What actually happened" demonstrate parallelism was already achieved; a microbenchmark wouldn't surface a meaningful delta between pre- and post-Path-A. If a benchmark becomes useful later (e.g., for measuring `spawn_blocking` thread overhead in long pipelines), file as a follow-up.
- [x] **Linux pgid integration test** added at `brush-shell/tests/bundled_pgid.rs`, gated with `#![cfg(target_os = "linux")]`. Test runs `cat /proc/self/stat | sh -c 'ps -o pgid= -p $$'` and asserts the bundled cat's pgid equals the sh stage's pgid. CI validates on Linux runners; local Windows builds compile-skip the file.

For Cycle 3:
- [ ] Design doc written.
- [ ] Reviewed by at least one other contributor (or upstream).
- [ ] Re-PDCA'd before implementation.

---

## Decision Log

(Append to this section as cycles complete.)

| Date | Cycle | Decision | Evidence |
|---|---|---|---|
| 2026-04-25 | 1 | Land pgid plumbing as planned. `ExecutionContext.process_group_id: Option<i32>` added; threaded through 4 construction sites in `commands.rs` + 1 in `shell/funcs.rs`; bundled shim reads it onto child `SimpleCommand`. No observable behavior change (bundled shim still returns `Completed` — unblocked by Cycle 2). Windows is a no-op as predicted. | Commit `151f026` — `brush-core/src/commands.rs`, `brush-core/src/shell/funcs.rs`, `brush-shell/src/bundled.rs` (+18 lines). |
| 2026-04-25 | 2 (planning) | Reorder Cycle 2 Do phase: prototype Path A first; gate Path B prototype on Path A failing concrete thresholds (test failures, < 30% parallelism gain, regressions). Cheaper-experiment-first, not gut-based — Path B remains the fallback. | Reflexion review 2026-04-25; updated Phase 2.2 in this doc. |
| 2026-04-25 | (carve-out) | Document EPIPE noise on Windows, uutils argv[0] stub-drop on Windows, and process-substitution / `trap INT` Windows limits as **explicitly out of scope** for Cycles 1–2. None of these are caused by serialization or pgid; reviewers shouldn't expect Cycle 2 benchmarks to change them. | Bash testing 2026-04-25 surfaced all four; added §3a to this doc. |
| 2026-04-25 | 2 (closed, hypothesis invalidated) | Path A landed as architectural cleanup, not parallelism unlock. Empirical measurement found pre-Path-A pipelines were *already parallel* — `commands.rs:466-490` wraps owned-shell builtin calls in `tokio::task::spawn_blocking` returning `ExecutionSpawnResult::StartedTask`, which the orchestrator awaits non-blockingly. The original Cycle 2 framing ("the bundled shim awaits child to completion → pipeline serializes") missed `StartedTask` in the dispatch table. Sleep-pipeline timings: pre-Path-A 2.4s, post-Path-A 2.8s for `sleep 2 \| sleep 2 \| sleep 2`; both demonstrate parallelism (a serialized version would be ~6s). Phase 2.2 (Path B prototype) skipped — its gating criterion was based on the invalidated hypothesis. Path A still landed because it's a real if smaller win: removes the `spawn_blocking` thread-pool hop, makes pgid handling uniform with external commands, replaces the inline shim path with direct external-spawn dispatch. `Registration` also marked `#[non_exhaustive]` (resolves earlier open question); new `raw_builtin()` helper on brush-core lets brush-shell construct registrations without struct-literal access. Linux pgid integration test added at `brush-shell/tests/bundled_pgid.rs`, gated to `cfg(target_os = "linux")`. | Commits `8936677` (prototype) and `86a8c1c` (cherry-pick to working branch); empirical timings on Windows; full analysis in this doc's "What actually happened" section. |

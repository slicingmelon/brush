# Path Conversion (MSYS-style ↔ native Windows)

> What this fork does when you hand it an MSYS-style POSIX path
> (`/c/Users/foo`, `/cygdrive/c/...`) on Windows. Last revision:
> 2026-04-29.
>
> Designed and shipped per
> [`docs/planning/path-conversion-msys.md`](../planning/path-conversion-msys.md)
> (Cycles 1 + 2). Cycle 3 (env-var + external-binary argv translation,
> plus runtime overrides) is not yet shipped — see "Roadmap" at the bottom.

## TL;DR

On Windows, this fork accepts MSYS-style paths almost everywhere a
native Windows path would work:

```bash
cd /c/Users/foo                 # works
[ -d /c/Windows ]               # works
/c/Windows/System32/whoami.exe  # works (direct exec)
cat /c/Github-Tools/brush/Cargo.toml          # works (bundled cat)
ls /c/Users/foo                               # works (bundled ls)
find /c/Github-Tools/brush -name Cargo.toml   # works (bundled find)
cygpath -w /c/Users/foo                       # → C:\Users\foo (explicit)
```

It does **not** translate paths handed to `sed` / `awk` / `grep` / `xargs`
or to external binaries (`git`, `curl`, `python`, …). That's a Cycle 3
item, opt-in only. If you need to translate a single path explicitly
today, use `cygpath -w "$x"` (Cycle 1 builtin).

On non-Windows targets the entire feature is a no-op passthrough — the
`cygpath` builtin still exists so portable scripts using
`$(cygpath -w "$x")` work unchanged on Linux/macOS.

## Where translation fires

Four layers exist in the design. This fork ships layers 0–2. Layer 3+4
are roadmap.

| Layer | Surface | Status | How to invoke |
|---|---|---|---|
| 0 | `path_conv` translation core | ✅ Cycle 1 | n/a (library) |
| 1 | `cygpath` builtin (explicit) | ✅ Cycle 1 | `cygpath -u/-w/-m/-a/-p PATH` |
| 2 | Bundled-tool argv (per-tool policy) | ✅ Cycle 2 | implicit on `cat /c/...` etc. |
| 3 | Outgoing env vars (e.g. `PATH`) | ⏳ Cycle 3 | future: `BRUSH_PATHCONV=1` |
| 4 | External-binary argv (`git`, `curl`, …) | ⏳ Cycle 3 | future: `BRUSH_PATHCONV=1` |

## Layer 1 — `cygpath` builtin

A clean-room `cygpath`-equivalent. No heuristics — purely user-driven.

| Flag | Effect | Example |
|---|---|---|
| `-u` / `--unix` | output POSIX form (default) | `cygpath -u "C:\Users\foo"` → `/c/Users/foo` |
| `-w` / `--windows` | output Windows form | `cygpath -w /c/Users/foo` → `C:\Users\foo` |
| `-m` / `--mixed` | Windows form with forward slashes | `cygpath -m /c/Users/foo` → `C:/Users/foo` |
| `-a` / `--absolute` | force absolute (joined with `cwd`) | `cygpath -a foo` → `/c/Github-Tools/brush/foo` |
| `-p` / `--path` | treat input as `:`/`;`-separated path list | `cygpath -p -w "/c/a:/d/b"` → `C:\a;D:\b` |

Recognised input forms (case-insensitive single drive letter):
`/c`, `/c/...`, `/cygdrive/c`, `/cygdrive/c/...`, `\c`, `\c\...`,
`C:`, `C:\...`, `C:/...`. Already-correct inputs are returned
borrowed (no allocation).

**Out of scope**: UNC paths (`//server/share`), extended-length paths
(`\\?\C:\...`), DOS short names (`C:\PROGRA~1\`).

Lives in
[`brush-experimental-builtins/src/cygpath.rs`](../../brush-experimental-builtins/src/cygpath.rs)
behind the `builtin.cygpath` feature (default-on for the
experimental-builtins crate).

## Layer 2 — Bundled-tool argv translation (per-tool policy)

When you invoke a bundled tool (anything from `brush-coreutils-builtins`
or `brush-bundled-extras`), brush re-spawns itself with
`--invoke-bundled <name> <args...>`. Before that re-spawn, the user
argv is translated according to a **compile-time per-tool policy**
defined in
[`brush-shell/src/bundled.rs::path_arg_policy_for`](../../brush-shell/src/bundled.rs).

Three policy variants:

| Policy | Behaviour | Tools assigned |
|---|---|---|
| `None` | Pass argv verbatim (default) | `sed`, `awk`, `grep`, `fastgrep`, `rg`, `xargs`, all archivers, anything not listed below |
| `Heuristic` | Translate any argv element where `looks_like_path` returns true, using `convert(_, PathForm::Win32)` | `cat`, `head`, `tail`, `wc`, `nl`, `od`, `cksum`, `sum`, `tac`, `tee`, `split`, `csplit`, `uniq`, `sort`, `comm`, `join`, `paste`, `shuf`, `tr`, `expand`, `unexpand`, `fmt`, `fold`, `pr`, `cp`, `mv`, `rm`, `ln`, `mkdir`, `rmdir`, `touch`, `chmod`, `chown`, `chgrp`, `stat`, `realpath`, `readlink`, `ls`, `dir`, `vdir`, `du`, `df`, `tree`, `which`, `file`, `xxd`, `column`, `basename`, `dirname`, `install`, `mktemp`, `shred`, `truncate`, `sync`, `cmp` |
| `Positional(positions)` | Translate only argv elements at the listed 1-based positions | `find` → `Positional(vec![1])` (starting-point only; predicates and predicate values pass through) |

`looks_like_path` excludes:

- args starting with `-` (option flags)
- URL-shaped strings containing `://`
- `KEY=value` shapes where `=` precedes any separator
- empty strings
- relative paths (`foo/bar`) — they don't need translation between MSYS and native

It accepts `/<L>`, `/<L>/...`, `/cygdrive/<L>`, `/cygdrive/<L>/...`,
and `<L>:`, `<L>:\...`, `<L>:/...` forms.

### Why `sed`/`awk`/`grep` are `None`

These tools take **patterns or scripts** as their first non-flag
argument. A pattern like `/c/foo` is a valid regex AND a valid
drive-rooted path — the meaning is determined by which tool is
consuming it. Translating would silently corrupt:

```bash
grep '/c/foo' file.txt           # /c/foo is a regex, must remain literal
sed -e '/c/d' file.txt           # /c/d is a sed address+command, must remain literal
awk '/^\/c/ { print }' file.txt  # the regex is path-shaped at boundaries
```

`None` is a fail-safe default. If you need a translated path with one
of these tools, use `cygpath` explicitly:

```bash
sed 's/foo/bar/' "$(cygpath -w /c/data/in.txt)" > out.txt
```

### Why `find` is `Positional([1])`

`find /c/path -predicate value` — only argv[1] is a path. Predicate
values like `-name "/c/foo"` or `-path "*/foo/*"` must remain literal.

## Layer 0 — `path_conv` core (library API)

The `convert`, `convert_path_list`, `looks_like_path`, and
`try_translate_msys_path` functions in
[`brush-core/src/sys/windows/path_conv.rs`](../../brush-core/src/sys/windows/path_conv.rs).
Used internally by:

- `Shell::absolute_path` (which is what makes `cd /c/...` work)
- `commands.rs` direct-exec resolution (`/c/Windows/System32/whoami.exe`)
- The `cygpath` builtin
- `apply_path_arg_policy` (Layer 2)

A cross-platform facade in
[`brush-core/src/sys/path_conv.rs`](../../brush-core/src/sys/path_conv.rs)
re-exports the Windows surface and provides no-op passthroughs on
other platforms — same API everywhere, behaviour only on Windows.

Algorithm reference: clean-room reimplementation informed by
[`stdlib-js/utils-convert-path`](https://github.com/stdlib-js/utils-convert-path)
(Apache-2.0). No code copied from MSYS2 / Cygwin (those are GPL).

## What this fork **doesn't** translate (yet)

Things that currently still need explicit `cygpath -w`:

- **External binary argv**: `git diff /c/...`, `curl -o /c/.../out`,
  `python /c/scripts/x.py`, `cargo run /c/...` — all pass argv verbatim.
- **Outgoing env vars**: setting `export PATH="/c/MyTools:$PATH"` in
  `.bashrc` doesn't translate the entry; native Windows children won't
  find executables under `/c/MyTools`.
- **Bundled tools assigned `None`**: `sed`, `awk`, `grep`, `xargs`, etc.

Workaround for all three:

```bash
git diff "$(cygpath -w /c/some/file)"
export PATH="$(cygpath -w /c/MyTools);$PATH"
sed 's/x/y/' "$(cygpath -w /c/data.txt)"
```

## Roadmap

- **Cycle 3** (planned, not started): env-var translation + external-binary
  argv translation, gated on `BRUSH_PATHCONV=1`. Plus runtime overrides
  via `BRUSH_PATHCONV_EXCL=cat,find,git` that toggle Cycle 2's
  bundled-tool table *and* Cycle 3's external-binary translation with
  one knob. See the carry-over note in
  [`docs/planning/path-conversion-msys.md`](../planning/path-conversion-msys.md#cycle-3--outgoing-env-vars--external-binary-argv--runtime-overrides-layers-3--4).

## See also

- [`CHANGELOG.FORK.md`](../../CHANGELOG.FORK.md) — release notes
- [`docs/planning/path-conversion-msys.md`](../planning/path-conversion-msys.md) — full A3 problem analysis + decision log
- [`docs/reference/bundled-tools-index.md`](bundled-tools-index.md) — what gets bundled vs. what falls through to PATH
- [Cygwin `cygpath(1)`](https://cygwin.com/cygwin-ug-net/cygpath.html) — the surface this fork's `cygpath` mirrors

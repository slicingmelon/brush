<div align="center">
  <img src="https://github.com/user-attachments/assets/19351a8e-7b03-4338-81be-dd5b6d7e5abc"/>
</div>

<br/>

<!-- Primary badges -->
<p align="center">
  <!-- crates.io version badge -->
  <a href="https://crates.io/crates/brush-shell"><img src="https://img.shields.io/crates/v/brush-shell?style=flat-square"/></a>
  <!-- msrv badge -->
  <img src="https://img.shields.io/crates/msrv/brush-shell"/>
  <!-- license badge -->
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square"/>
  <br/>
  <!-- crates.io download badge -->
  <a href="https://crates.io/crates/brush-shell"><img src="https://img.shields.io/crates/d/brush-shell?style=flat-square"/></a>
  <!-- compat tests badge -->
  <img src="https://img.shields.io/badge/compat_tests-1389-brightgreen?style=flat-square" alt="1389 compatibility tests"/>
  <!-- Packaging badges -->
  <a href="https://repology.org/project/brush/versions">
    <img src="https://repology.org/badge/tiny-repos/brush.svg" alt="Packaging status"/>
  </a>
  <!-- Social badges -->
  <a href="https://discord.gg/kPRgC9j3Tj">
    <img src="https://dcbadge.limes.pink/api/server/https://discord.gg/kPRgC9j3Tj?compact=true&style=flat" alt="Discord invite"/>
  </a>
</p>

<a href="https://repology.org/project/brush/versions">
</a>

</p>

<hr/>

`brush` (**B**o(u)rn(e) **RU**sty **SH**ell) is a modern [bash-](https://www.gnu.org/software/bash/) and [POSIX-](https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html) compatible shell written in Rust. Run your existing scripts and `.bashrc` unchanged -- with syntax highlighting and auto-suggestions built in.

> **🍴 This is a fork** of [reubeno/brush](https://github.com/reubeno/brush) maintained at [slicingmelon/brush](https://github.com/slicingmelon/brush) with Windows / Git-Bash / Claude Code compatibility fixes. See [`CHANGELOG.FORK.md`](./CHANGELOG.FORK.md) for fork-specific changes. Jump to [Installing this fork](#-installing-this-fork) for the install commands.

## At a glance

✅ Your existing `.bashrc` just works—aliases, functions, completions, all of it.<br/>
✨ Syntax highlighting and auto-suggestions built in and enabled by default.<br/>
🧪 Validated against bash with [~1500 compatibility tests](brush-shell/tests/cases).<br/>
🧩 Easily embeddable in your Rust apps using `brush_core::Shell`.<br/>

<p align="center">
  <img src="https://github.com/user-attachments/assets/0e64d1b9-7e4e-43be-8593-6c1b9607ac52" width="80%"/>
</p>

> ⚠️ **Not everything works yet:** `coproc`, `select`, and some edge cases aren't supported. See the [Compatibility Reference](docs/reference/compatibility.md) for details.

### Quick start:

```console
$ cargo binstall brush-shell         # using cargo-binstall
$ brew install brush                 # using Homebrew
$ pacman -S brush                    # Arch Linux
$ cargo install --locked brush-shell # Build from sources
```

`brush` is ready for use as a daily driver. We test every change against `bash` to keep it that way.

More detailed installation instructions are available below.

## ✨ Features

### 🐚 `bash` Compatibility

| | Feature | Description |
|--|---------|-------------|
| ✅ | **50+ builtins** | `echo`, `declare`, `read`, `complete`, `trap`, `ulimit`, ... |
| ✅ | **Full expansions** | brace, parameter, arithmetic, command/process substitution, globs, `extglob`, `globstar` |
| ✅ | **Control flow** | `if`/`for`/`while`/`until`/`case`, `&&`/`\|\|`, subshells, pipelines, etc. |
| ✅ | **Redirection** | here docs, here strings, fd duplication, process substitution redirects |
| ✅ | **Arrays & variables** | indexed/associative arrays, dynamic variables, standard well-known variables, etc. |
| ✅ | **Programmable completion** | Works with [bash-completion](https://github.com/scop/bash-completion) out of the box |
| ✅ | **Job control** | background jobs, suspend/resume, `fg`/`bg`/`jobs` |
| 🔷 | **Traps & options** | `DEBUG`/`ERR`/`EXIT` traps work; signal traps and options in progress |

### ⌨️ User Experience

| | Feature | Description |
|--|---------|-------------|
| ✅ | **Syntax highlighting** | Real-time as you type ([reedline](https://github.com/nushell/reedline)) |
| ✅ | **Auto-suggestions** | History-based hints as you type ([reedline](https://github.com/nushell/reedline)) |
| ✅ | **Rich prompts** | `PS1`/`PROMPT_COMMAND`, right prompts, [starship](https://starship.rs) compatible |
| ✅ | **TOML config** | `~/.config/brush/config.toml` for persistent settings |
| 🧪 | **Extras** | `fzf`/`atuin` support, zsh-style `precmd`/`preexec` hooks (experimental), VS Code terminal integration |

## Installation

_When you run `brush`, it should look exactly as `bash` does on your system: it processes your `.bashrc` and
other standard configuration. If you'd like to distinguish the look of `brush` from the other shells
on your system, you may author a `~/.brushrc` file._

### 🍴 Installing this fork

This fork lives at [`slicingmelon/brush`](https://github.com/slicingmelon/brush). The recommended install path is to build from the fork with the experimental feature flags below. Pick the right line for your platform — `--force` lets you reinstall over an existing build (useful when iterating, or when using brush as a Git-Bash replacement via Claude Code's `CLAUDE_CODE_GIT_BASH_PATH`).

> ℹ️ **Two binaries are produced.** Every install of this fork deposits both `brush` *and* `bash` (`brush.exe` / `bash.exe` on Windows) into `~/.cargo/bin/`. The `bash` binary is the same shell — it simply identifies itself as `bash (brush)` in version banners. This lets brush act as a drop-in Git-Bash replacement without any manual rename step.

**Recommended per-platform install (everything bundled — coreutils + Unix/Linux extras + findutils + `sed`/`awk` + `grep` family + utility quick-wins + compression family):**

> ⚠️ **Toolchain requirement**: the recommended commands below pull `fastgrep` (one of the bundled `grep` providers), which has `rust-version = "1.92"`. You need **rustc ≥ 1.92** for these commands to compile. The brush workspace itself stays at MSRV 1.88.0; only the optional fastgrep flag carries the bumped requirement. If you're on rustc 1.88–1.91, see the "without fastgrep" alternate further down — you still get a full GNU-grep-compatible `grep` (with `-P`/PCRE2!) via the bundled ripgrep adapter.

The single `experimental-bundled-extras` umbrella flag now pulls in **everything bundled-extras** transitively: `find`/`xargs`, `sed`, `awk`, fastgrep + ripgrep (`grep`/`rg`/`egrep`/`fgrep`/`fastgrep`), the utility quick-wins (`which`/`tree`/`xxd`/`column`/`file`/`id`/`clear`), and the compression family (`tar`/`gzip`/`gunzip`/`zcat`/`gzcat`/`bzip2`/`bunzip2`/`bzcat`/`xz`/`unxz`/`xzcat`/`unzip`/`zipinfo`).

```bash
# Linux (full Unix+Linux extras + everything bundled-extras)
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force --features experimental-builtins,experimental-bundled-coreutils-linux-extras,experimental-bundled-extras

# macOS (Unix extras + everything bundled-extras — same as Linux minus stdbuf/chcon/runcon)
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force --features experimental-builtins,experimental-bundled-coreutils-unix-extras,experimental-bundled-extras

# Windows (cross-platform coreutils + everything bundled-extras — uutils gates chmod/chown/etc to cfg(unix); brush bundles its own `id` via Win32 token API)
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force --features experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras
```

**Install from a local clone** (use the same `--features` line as above, just replace the `--git ...` part with `--path brush/brush-shell`):

```bash
git clone https://github.com/slicingmelon/brush
cargo install --locked --path brush/brush-shell --force --features experimental-builtins,experimental-bundled-coreutils,experimental-bundled-extras
```

**Alternate install for rustc 1.88–1.91 (no `fastgrep`, but still full `grep` via ripgrep)** — replaces the umbrella `experimental-bundled-extras` with the per-utility flags so fastgrep's MSRV bump doesn't apply. You **still get** `grep`/`egrep`/`fgrep`/`rg` (the ripgrep adapter has no MSRV bump) plus `find`/`xargs`/`sed`/`awk`/all utility quick-wins / compression family:

```bash
# Pick the matching coreutils flag for your platform; this example is Linux.
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell --force --features experimental-builtins,experimental-bundled-coreutils-linux-extras,experimental-bundled-extras-findutils,experimental-bundled-extras-uutils-sed,experimental-bundled-extras-awk-rs,experimental-bundled-extras-ripgrep,experimental-bundled-extras-utils,experimental-bundled-extras-compression
```

The only difference vs. the umbrella install is that `fastgrep` (under its own command name) won't be available — `grep -P` still works because the ripgrep-backed `grep` supports PCRE2.

**Plain install (no experimental features — minimal shell, no bundled utilities):**

```bash
cargo install --locked --git https://github.com/slicingmelon/brush brush-shell
```

**What each feature flag enables:**

| Flag                                              | What it bundles                                                                                                                                           | Platforms | MSRV  |
|---------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------|-----------|-------|
| `experimental-builtins`                           | Extra native shell builtins (e.g. `save`)                                                                                                                 | all       | 1.88  |
| `experimental-bundled-coreutils`                  | uutils/coreutils — every utility that builds on Tier-1 targets (cross-platform set, ~82 utilities including `cat`, `ls`, `head`, `tail`, `wc`, `sort`, ...)| all       | 1.88  |
| `experimental-bundled-coreutils-unix-extras`      | Adds Unix-only utilities on top: `id`, `groups`, `stat`, `timeout`, `install`, `chmod`, `chown`, `chgrp`, `chroot`, `logname`, `tty`, `mkfifo`, `mknod`, `nice`, `nohup`, `stty`, `kill`, `pinky`, `uptime`, `users`, `who`, `hostid` | Unix only | 1.88  |
| `experimental-bundled-coreutils-linux-extras`     | Adds Linux-only utilities on top of `-unix-extras`: `stdbuf`, `chcon`, `runcon`                                                                           | Linux only| 1.88  |
| `experimental-bundled-extras` *(umbrella)*        | All bundled-extras transitively: `find`/`xargs`, `sed`, `awk`, fastgrep + ripgrep (both register `grep`/`egrep`/`fgrep`; `rg` and `fastgrep` keep their own names), the utility quick-wins, and the compression family. Inherits fastgrep's MSRV bump (skip the `-fastgrep` flag below to drop the requirement and stay at 1.88). | all       | **1.92** |
| `experimental-bundled-extras-findutils`           | `find` and `xargs` from `uutils/findutils@0.8.0`                                                                                                          | all       | 1.88  |
| `experimental-bundled-extras-uutils-sed`          | `sed` from `uutils/sed@0.1.1`                                                                                                                             | all       | 1.88  |
| `experimental-bundled-extras-awk-rs`              | `awk` from `pegasusheavy/awk-rs@0.1.0` — 100 % POSIX claim, gawk extensions, ~1.6× gawk on 100k-line sums                                                 | all       | 1.85  |
| `experimental-bundled-extras-fastgrep`            | `grep` + `fastgrep` aliases from `awnion/fastgrep@0.1.8` — SIMD-fast, 2–12× faster than GNU grep on Criterion benchmarks; **does NOT support `-P`/PCRE2**. When combined with `-ripgrep` below, ripgrep wins for `grep`/`egrep`/`fgrep` (HashMap order) and `fastgrep` keeps its own name. | all       | **1.92** |
| `experimental-bundled-extras-ripgrep`             | `rg` (canonical) + `grep`/`egrep`/`fgrep` from a line-based ripgrep-style adapter using `regex` + `pcre2` + `ignore` (the same crate ripgrep uses for gitignore-aware walks). **Supports `-P` (PCRE2)** — the headline reason this exists alongside fastgrep. | all       | 1.88  |
| `experimental-bundled-extras-utils`               | Utility quick-wins: `which` (via `which` crate), `tree` (in-tree using `walkdir`), `xxd` / `column` / `clear` (pure in-tree), `file` (via `infer`), `id` (libc on Unix, Win32 token API on Windows — fills the gap left by uutils' `cfg(unix)`-gated `uu_id`). | all       | 1.88  |
| `experimental-bundled-extras-compression`         | Compression family: `tar` (via `tar` + `flate2`), gzip family (`gzip`/`gunzip`/`zcat`/`gzcat` via `flate2`), bzip2 family (`bzip2`/`bunzip2`/`bzcat` via `bzip2` with pure-Rust `libbz2-rs-sys` backend), xz family (`xz`/`unxz`/`xzcat` via `xz2`), zip family (`unzip`/`zipinfo` via `zip`). Archive-creation `zip` not included. | all  | 1.88  |
| `experimental` *(umbrella)*                       | Convenience meta-feature: `experimental-builtins` + `experimental-bundled-coreutils` + `experimental-load` + `experimental-parser`                        | all       | 1.88  |

**Verify both binaries:**

```bash
brush --version    # → brush version 0.3.9 (...)
bash  --version    # → bash (brush) version 0.3.9 (...)
```

**Spot-check the bundled commands:**

```bash
brush -c 'type rg && type grep && type tar && type tree && type id && type clear'
# → all "is a shell builtin"

brush -c "echo 'aa1bb' | grep -P '\d'"
# → aa1bb   (PCRE2 works via the ripgrep-backed grep)
```

**Uninstall:**

```bash
cargo uninstall brush-shell
```

See [`CHANGELOG.FORK.md`](./CHANGELOG.FORK.md) for the full release notes and per-component version bumps. The latest release is **0.3.9** which folded the ripgrep adapter into the umbrella so the recommended install line above bundles everything in one shot.


<details>
<summary>🍺 <b>Installing using Homebrew</b> (macOS/Linux)</summary>

Homebrew users can install using [the `brush` formula](https://formulae.brew.sh/formula/brush):

```bash
brew install brush
```

</details>

<details>
<summary>🐧 <b>Installing on Arch Linux</b></summary>

Arch Linux users can install `brush` from the official [extra repository](https://archlinux.org/packages/extra/x86_64/brush/):

```bash
pacman -S brush
```

</details>

<details>
<summary>🚀 <b>Installing prebuilt binaries via `cargo binstall`</b></summary>

You may use [cargo binstall](https://github.com/cargo-bins/cargo-binstall) to install pre-built `brush` binaries. Once you've installed `cargo-binstall` you can run:

```bash
cargo binstall brush-shell
```

</details>

<details>
<summary>🚀 <b>Installing prebuilt binaries from GitHub</b></summary>

We publish prebuilt binaries of `brush` for Linux (x86_64, aarch64) and macOS (aarch64) to GitHub for official [releases](https://github.com/reubeno/brush/releases). You can manually download and extract the `brush` binary from one of the archives published there, or otherwise use the GitHub CLI to download it, e.g.:

```bash
gh release download --repo reubeno/brush --pattern "brush-x86_64-unknown-linux-gnu.*"
```

After downloading the archive for your platform, you may verify its authenticity using the [GitHub CLI](https://cli.github.com/), e.g.:

```bash
gh attestation verify brush-x86_64-unknown-linux-gnu.tar.gz --repo reubeno/brush
```

</details>

<details>
<summary>🐧 <b>Installing using Nix</b></summary>

If you are a Nix user, you can use the registered version:

```bash
nix run 'github:NixOS/nixpkgs/nixpkgs-unstable#brush' -- --version
```

</details>

<details>
<summary> 🔨 <b>Building from sources</b></summary>

To build from sources, first install a working (and recent) `rust` toolchain; we recommend installing it via [`rustup`](https://rustup.rs/). Then run:

```bash
cargo install --locked brush-shell
```

</details>

## Community & Contributing

This project started out of curiosity and a desire to learn—we're keeping that attitude. If something doesn't work the way you'd expect, [let us know](https://github.com/reubeno/brush/issues)!

* [Discord server](https://discord.gg/kPRgC9j3Tj) — chat with the community
* [Building from source](docs/how-to/build.md) — development workflow
* [Contribution guidelines](CONTRIBUTING.md) — how to submit changes
* [Technical docs](docs/README.md) — architecture and reference

## Related Projects

Other POSIX-ish shells implemented in non-C/C++ languages:

* [`nushell`](https://www.nushell.sh/) — modern Rust shell (provides `reedline`)
* [`fish`](https://fishshell.com) — user-friendly shell ([Rust port in 4.0](https://fishshell.com/blog/rustport/))
* [`Oils`](https://github.com/oils-for-unix/oils) — bash-compatible with new Oil language
* [`mvdan/sh`](https://github.com/mvdan/sh) — Go implementation
* [`rusty_bash`](https://github.com/shellgei/rusty_bash) — another Rust bash-like shell

<details>
<summary><b>🙏 Credits</b></summary>

This project relies on many excellent OSS crates:

* [`reedline`](https://github.com/nushell/reedline) — readline-like input and interactive features
* [`clap`](https://github.com/clap-rs/clap) — command-line parsing
* [`fancy-regex`](https://github.com/fancy-regex/fancy-regex) — regex support
* [`tokio`](https://github.com/tokio-rs/tokio) — async runtime
* [`nix`](https://github.com/nix-rust/nix) — Unix/POSIX APIs
* [`criterion.rs`](https://github.com/bheisler/criterion.rs) — benchmarking
* [`bash-completion`](https://github.com/scop/bash-completion) — completion test suite

</details>

---

Licensed under the [MIT license](LICENSE).

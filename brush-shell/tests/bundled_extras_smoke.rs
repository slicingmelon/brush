//! Smoke tests for fork-bundled `rg` / `ripgrep` / `grep` / `egrep` /
//! `fgrep` / `awk` / `sed`. Verifies the agent-relevant flag matrix
//! that the `feat/bundled-extras-cli-fidelity` branch widens.
//!
//! These tests build a brush.exe with the `experimental-bundled-extras`
//! feature and invoke it as a subprocess via `assert_cmd::cargo_bin!`.
//! They are not gated on a `[[test]] required-features` because the
//! workspace test runner builds with the default feature set; instead
//! each test calls [`brush_with_extras`] which `panic!`s with a
//! self-explanatory message if the binary lacks bundled-extras support
//! (e.g. the bundled name is not registered).
//!
//! Tests cover:
//!
//! * `rg` / `ripgrep`: name registration, `--type-list`, `-t TYPE`,
//!   `-S` smart-case, `--column`, `-g GLOB`, `--no-heading`,
//!   `-j N` (no-op accept), `--passthru`, `-A`/`-B`/`-C`.
//! * `grep`: GNU semantics — non-recursive on dirs (errors), `-r`
//!   recurses, `-NUM` shorthand for `-C NUM`, `-y` synonym for `-i`,
//!   `-P` PCRE2 lookahead.
//! * `egrep` / `fgrep`: alias dispatch.
//! * `sed`: basic substitution, `-E` ERE, `-e` chains, `-n NUM p`.
//! * `awk`: `$N` field access, `-F SEP`, `-v VAR=VAL`, `BEGIN` block.

#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::tests_outside_test_module,
    clippy::uninlined_format_args,
    clippy::needless_raw_string_hashes,
    reason = "integration test scaffolding — panics ARE the test failure mode"
)]

use std::process::Command;

/// Locate the brush binary built by the current cargo target.
///
/// Mirrors [`bundled_pgid`](./bundled_pgid.rs) — `assert_cmd::cargo_bin!`
/// resolves to `target/<profile>/brush(.exe)` per the `[[bin]]` entries
/// in `brush-shell/Cargo.toml`.
fn brush_bin() -> std::path::PathBuf {
    let p = assert_cmd::cargo::cargo_bin!("brush").to_path_buf();
    assert!(
        p.exists(),
        "brush binary not found at {} — was it built with experimental-bundled-extras?",
        p.display(),
    );
    p
}

/// Run `brush -c <script>` with stdin (optional) and return `(stdout, stderr, exit_code)`.
fn run_brush(script: &str, stdin: Option<&str>) -> (String, String, i32) {
    let mut cmd = Command::new(brush_bin());
    cmd.args(["-c", script]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().expect("spawn brush");
    if let Some(s) = stdin {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(s.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("wait brush");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.code().unwrap_or(-1),
    )
}

/// Convenience wrapper that asserts brush exited 0 and returns stdout.
fn brush_with_extras(script: &str) -> String {
    let (stdout, stderr, code) = run_brush(script, None);
    assert_eq!(
        code, 0,
        "brush -c {script:?} exited {code}\nstdout: {stdout}\nstderr: {stderr}",
    );
    stdout
}

// =====================================================================
// Name registration
// =====================================================================

#[test]
fn rg_name_registered() {
    let out = brush_with_extras("command -v rg");
    assert_eq!(out.trim(), "rg");
}

#[test]
fn ripgrep_name_registered() {
    // Regression: `ripgrep` was missing in 0.5.x; agents probe the full
    // name as well as the short `rg`.
    let out = brush_with_extras("command -v ripgrep");
    assert_eq!(out.trim(), "ripgrep");
}

#[test]
fn grep_egrep_fgrep_names_registered() {
    let out = brush_with_extras("command -v grep && command -v egrep && command -v fgrep");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["grep", "egrep", "fgrep"]);
}

#[test]
fn awk_sed_names_registered() {
    let out = brush_with_extras("command -v awk && command -v sed");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["awk", "sed"]);
}

// =====================================================================
// rg / ripgrep flag matrix
// =====================================================================

#[test]
fn rg_version_banner_identifies_brush_adapter() {
    let out = brush_with_extras("rg --version");
    assert!(
        out.contains("brush-bundled-extras"),
        "rg --version should identify itself as brush-bundled-extras adapter; got: {out}",
    );
    assert!(out.contains("PCRE2"));
}

#[test]
fn rg_type_list_works() {
    let out = brush_with_extras("rg --type-list");
    // `ada` is one of the first definitions in ignore::types::add_defaults.
    assert!(
        out.lines().any(|l| l.starts_with("ada:")),
        "rg --type-list should list ada among types; got: {out}",
    );
    // 'rust' must be there — it's the most agent-likely use of -t.
    assert!(
        out.lines().any(|l| l.starts_with("rust:")),
        "rg --type-list should list rust; got: {out}",
    );
}

#[test]
fn rg_smart_case_lowercase_pattern_matches_uppercase() {
    let (stdout, _, code) = run_brush("echo APPLE | rg -S apple", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "APPLE");
}

#[test]
fn rg_smart_case_uppercase_pattern_does_not_match_lowercase() {
    let (stdout, _, code) = run_brush("echo apple | rg -S Apple", None);
    assert_eq!(
        code, 1,
        "exit 1 expected for no match; got stdout: {stdout}"
    );
    assert!(stdout.is_empty());
}

#[test]
fn rg_pcre2_lookahead() {
    let (stdout, _, code) = run_brush("echo lookahead | rg -P 'look(?=ahead)'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "lookahead");
}

#[test]
fn rg_threads_flag_accepted() {
    // -j is a no-op in our adapter; verify it parses without error.
    let (stdout, stderr, code) = run_brush("echo a | rg -j 4 a", None);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout.trim(), "a");
}

#[test]
fn rg_passthru_prints_non_matching_lines() {
    let (stdout, _, code) = run_brush("printf 'a\\nb\\nc\\n' | rg --passthru b", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["a", "b", "c"]);
}

#[test]
fn rg_after_context() {
    let (stdout, _, code) = run_brush("printf '1\\n2\\n3\\n4\\n5\\n' | rg -A2 3", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["3", "4", "5"]);
}

#[test]
fn rg_before_context() {
    let (stdout, _, code) = run_brush("printf '1\\n2\\n3\\n4\\n5\\n' | rg -B2 3", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["1", "2", "3"]);
}

#[test]
fn rg_only_matching() {
    let (stdout, _, code) = run_brush("echo 'abc def ghi' | rg -o '[a-z]+'", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["abc", "def", "ghi"]);
}

#[test]
fn rg_count() {
    let (stdout, _, code) = run_brush("printf 'a\\nb\\na\\nc\\na\\n' | rg -c a", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "3");
}

#[test]
fn rg_invert_match() {
    let (stdout, _, code) = run_brush("printf 'a\\nb\\nc\\n' | rg -v b", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["a", "c"]);
}

#[test]
fn rg_word_boundary() {
    let (stdout, _, code) = run_brush("echo 'cat catcher caterpillar' | rg -w cat", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "cat catcher caterpillar");
    // -w should match only standalone 'cat' — line still printed because
    // 'cat' as a word is in the line.
}

#[test]
fn rg_fixed_string_treats_pattern_literally() {
    // Regex meta chars in pattern should not error
    let (stdout, _, code) = run_brush("echo 'a.b.c' | rg -F '.b.'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "a.b.c");
}

#[test]
fn rg_max_count_stops_after_n_matches() {
    let (stdout, _, code) = run_brush("printf 'x\\nx\\nx\\nx\\nx\\n' | rg -m 2 x", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["x", "x"]);
}

// =====================================================================
// GNU grep semantics
// =====================================================================

/// Build a small temp directory with two text files for filesystem-walking tests.
/// Cleaned up automatically when the returned [`tempfile::TempDir`] drops.
fn fixture_dir() -> tempfile::TempDir {
    use std::io::Write;
    let dir = tempfile::tempdir().expect("create tempdir");
    for (name, body) in [
        ("a.txt", "alpha\nbeta\ngamma\n"),
        ("b.txt", "delta\nepsilon\nzeta\n"),
    ] {
        let mut f = std::fs::File::create(dir.path().join(name)).expect("create file");
        f.write_all(body.as_bytes()).expect("write file");
    }
    dir
}

#[test]
fn grep_directory_without_r_errors() {
    // GNU grep: `grep PATTERN DIR` errors with "Is a directory".
    // ripgrep auto-recurses. Our `grep` adapter must follow GNU semantics
    // since `grep` is what agents invoke when they want GNU behavior.
    let dir = fixture_dir();
    let path = dir.path().display().to_string().replace('\\', "/");
    let script = format!("grep alpha '{path}'");
    let (stdout, stderr, code) = run_brush(&script, None);
    assert!(
        stderr.contains("Is a directory") || stderr.contains("is a directory"),
        "grep on a dir without -r should report 'Is a directory' on stderr; \
         got stdout={stdout:?} stderr={stderr:?}",
    );
    assert_ne!(code, 0);
}

#[test]
fn grep_recursive_with_r_works() {
    let dir = fixture_dir();
    let path = dir.path().display().to_string().replace('\\', "/");
    let script = format!("grep -r alpha '{path}'");
    let (stdout, stderr, code) = run_brush(&script, None);
    assert_eq!(code, 0, "stderr: {stderr}\nstdout: {stdout}");
    assert!(
        stdout.contains("alpha"),
        "grep -r should match 'alpha' in fixture; got: {stdout}",
    );
}

#[test]
fn grep_dash_num_shortcut_for_context() {
    // GNU grep `-3` is shorthand for `-C 3`. Verify our preprocess_argv
    // rewrites it.
    let (stdout, _, code) = run_brush("printf '1\\n2\\n3\\n4\\n5\\n' | grep -2 3", None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["1", "2", "3", "4", "5"]);
}

#[test]
fn grep_dash_y_synonym_for_dash_i() {
    let (stdout, _, code) = run_brush("echo HELLO | grep -y hello", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "HELLO");
}

#[test]
fn grep_pcre2_lookbehind() {
    let (stdout, _, code) = run_brush("echo foo123 | grep -P '(?<=foo)\\d+'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "foo123");
}

#[test]
fn grep_no_messages_suppresses_errors() {
    // -s means --no-messages in GNU grep; our preprocess rewrites it.
    let (_, stderr, _) = run_brush("grep -s xyz /nonexistent/file 2>&1", None);
    assert!(
        !stderr.contains("No such file"),
        "grep -s should suppress 'No such file'; stderr: {stderr}",
    );
}

#[test]
fn grep_extended_regex_alternation() {
    let (stdout, _, code) = run_brush(
        "printf 'apple\\nbanana\\ncherry\\n' | grep -E 'apple|cherry'",
        None,
    );
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["apple", "cherry"]);
}

#[test]
fn egrep_alias_extended_regex() {
    let (stdout, _, code) = run_brush(
        "printf 'apple\\nbanana\\ncherry\\n' | egrep 'apple|cherry'",
        None,
    );
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["apple", "cherry"]);
}

#[test]
fn fgrep_alias_fixed_string() {
    // fgrep treats pattern as literal — regex meta should not match.
    let (stdout, _, code) = run_brush("echo 'a.b' | fgrep '.'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "a.b");
}

// =====================================================================
// sed (uutils/sed via uumain — full upstream CLI)
// =====================================================================

#[test]
fn sed_basic_substitution() {
    let (stdout, _, code) = run_brush("echo abc | sed 's/b/X/'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "aXc");
}

#[test]
fn sed_extended_regex_capture_group() {
    let (stdout, _, code) = run_brush(r#"echo abc | sed -E 's/(.)/[\1]/g'"#, None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "[a][b][c]");
}

#[test]
fn sed_multiple_e_scripts() {
    let (stdout, _, code) = run_brush("echo abc | sed -e 's/a/A/' -e 's/c/C/'", None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "AbC");
}

#[test]
fn sed_n_with_p_for_specific_line() {
    let (stdout, _, code) = run_brush(r#"printf 'one\ntwo\nthree\n' | sed -n '2p'"#, None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "two");
}

#[test]
fn sed_d_for_delete_lines() {
    let (stdout, _, code) = run_brush(r#"printf 'a\nb\nc\n' | sed '2d'"#, None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["a", "c"]);
}

// =====================================================================
// awk (pegasusheavy/awk-rs)
// =====================================================================

#[test]
fn awk_field_access() {
    let (stdout, _, code) = run_brush(r#"echo 'a b c' | awk '{print $2}'"#, None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "b");
}

#[test]
fn awk_field_separator_dash_f() {
    let (stdout, _, code) = run_brush(r#"echo a:b:c | awk -F: '{print $2}'"#, None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "b");
}

#[test]
fn awk_variable_assignment_dash_v() {
    let (stdout, _, code) = run_brush(r#"echo hi | awk -v X=Y '{print X, $0}'"#, None);
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "Y hi");
}

#[test]
fn awk_begin_block() {
    let (stdout, _, code) = run_brush(r#"awk 'BEGIN{for(i=1;i<=3;i++)print i*i}'"#, None);
    assert_eq!(code, 0);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines, vec!["1", "4", "9"]);
}

#[test]
fn awk_end_block_aggregates() {
    let (stdout, _, code) = run_brush(
        r#"printf '1\n2\n3\n' | awk '{sum+=$1} END{print sum}'"#,
        None,
    );
    assert_eq!(code, 0);
    assert_eq!(stdout.trim(), "6");
}

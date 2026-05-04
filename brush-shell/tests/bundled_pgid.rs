//! Linux-only integration test for bundled-pipeline pgid sharing.
//!
//! Verifies that two stages of a brush pipeline — one bundled, one not —
//! share a single process group ID. This is the end-to-end check that
//! Cycle 1 (pgid plumbing through `ExecutionContext`) plus Cycle 2 of
//! `docs/planning/bundled-coreutils-pipelines.md` (Path A — routing
//! bundled dispatch through the same external-spawn machinery as
//! ordinary `PATH` commands) compose correctly on Unix.
//!
//! Skipped on macOS and Windows: macOS lacks `/proc/<pid>/stat` (Linux
//! procfs only) and Windows' `process_group` is a stub. CI runners
//! cover Linux; local dev builds on other platforms compile but skip.
//!
//! ## Test mechanics
//!
//! Pipeline: `cat /proc/self/stat | sh -c 'ps -o pgid= -p $$'`
//!
//! * Stage 1: bundled `cat` reads `/proc/self/stat` of *its own* brush
//!   child process. Field 5 in `/proc/<pid>/stat` is the process-group
//!   ID. So stdout starts with cat's pgid (after the pid + comm fields).
//! * Stage 2: system `sh` runs `ps -o pgid= -p $$`, printing its own
//!   pgid as the final line of stdout.
//!
//! Both stages should be in the pipeline pgid (which is the pgid of the
//! pipeline leader — the first stage to call `process_group(0)`). If
//! pgid plumbing or external-spawn routing is broken, the values
//! differ.

#![cfg(target_os = "linux")]

use std::process::Command;

/// Parses field 5 (pgid) from a `/proc/<pid>/stat` line.
///
/// Format: `pid (comm) state ppid pgid ...`
///
/// The `comm` field is parenthesized but can contain spaces and other
/// parens, so rfind the closing paren first instead of split_whitespace
/// from the start.
fn pgid_from_proc_stat(stat: &str) -> Option<i32> {
    let close = stat.rfind(')')?;
    let after = &stat[close + 1..];
    let mut fields = after.split_whitespace();
    let _state = fields.next()?;
    let _ppid = fields.next()?;
    fields.next()?.parse().ok()
}

#[test]
fn bundled_pipeline_shares_pgid() {
    let brush = assert_cmd::cargo::cargo_bin!("brush");
    assert!(
        brush.exists(),
        "brush binary not found at {brush:?} — was it built?",
    );

    let output = Command::new(&brush)
        .args(["-c", "cat /proc/self/stat | sh -c 'ps -o pgid= -p $$'"])
        .output()
        .expect("brush exec failed");

    assert!(
        output.status.success(),
        "brush returned non-zero: status={:?} stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    assert!(
        !trimmed.is_empty(),
        "brush produced empty stdout (stderr: {})",
        String::from_utf8_lossy(&output.stderr),
    );

    // The /proc/self/stat blob is the entire stdout up through the
    // closing paren of `comm` and on through the rest of stat's fields,
    // followed (with no separator from `cat`) by the ps output line.
    // ps emits a number with leading whitespace, terminated by newline.
    //
    // The full stdout looks like:
    //
    //     <pid> (<comm>) <state> <ppid> <pgid> ... <last-stat-field>
    //         <pgid>
    //
    // Splitting by lines: line[0] is the stat blob, line[1] is the ps
    // pgid (possibly preceded by leading whitespace). cat's stat content
    // doesn't contain a newline — it's a single line — so this works
    // unless the kernel changes /proc/<pid>/stat formatting.
    let mut lines = trimmed.lines();
    let stat_line = lines
        .next()
        .expect("expected /proc/self/stat content as first line");
    let ps_line = lines.next().expect("expected ps pgid as second line");

    let cat_pgid = pgid_from_proc_stat(stat_line)
        .unwrap_or_else(|| panic!("could not parse pgid from /proc/self/stat line: {stat_line:?}"));
    let sh_pgid: i32 = ps_line
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("could not parse ps pgid from line {ps_line:?}: {e}"));

    assert_eq!(
        cat_pgid, sh_pgid,
        "bundled cat (pgid={cat_pgid}) and pipeline sh (pgid={sh_pgid}) \
         should share a single process group; this means either Cycle 1's \
         ExecutionContext.process_group_id plumbing or Cycle 2's \
         execute_via_bundled routing is not propagating pgid correctly. \
         Full stdout: {stdout:?}"
    );
}

#[test]
fn pgid_parser_handles_complex_comm() {
    // Sanity check the parser against a known-tricky comm field —
    // names with spaces, parens, etc. don't break field 5 extraction.
    assert_eq!(
        pgid_from_proc_stat("1234 (some weird (comm) name) S 1 5678 ..."),
        Some(5678),
    );
    assert_eq!(pgid_from_proc_stat("1 (init) S 0 1 1 0 ..."), Some(1),);
    assert_eq!(pgid_from_proc_stat("not stat"), None);
    assert_eq!(pgid_from_proc_stat(""), None);
}

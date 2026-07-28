//! CLI smoke tests.
//!
//! These tests spawn the binary against the bundled fixture and
//! verify exit codes + non-empty output. They run as part of
//! `cargo test --all`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn fixture_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/
    p.pop(); // LinkMarks/
    p.push("crates/bridges/linkmarks-bridge-chromium/tests/fixtures/chrome-bookmarks.example.json");
    p
}

fn linkmarks_bin() -> Command {
    // We use `cargo run` so the workspace rebuilds the CLI on demand.
    // For a tighter loop in CI, swap to the release binary.
    let mut cmd = Command::new(env!("CARGO"));
    cmd.arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg("linkmarks-cli")
        .arg("--")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

#[test]
fn list_smoke_exits_zero_and_emits_table() {
    let path = fixture_path();
    assert!(path.exists(), "fixture missing: {}", path.display());

    let out = linkmarks_bin()
        .arg("list")
        .arg("--source=chrome")
        .arg("--path")
        .arg(&path)
        .arg("--format=table")
        .output()
        .expect("spawn linkmarks list");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.starts_with("id\tcanonical_url\t"), "got: {stdout}");
}

#[test]
fn list_json_smoke_emits_ndjson() {
    let path = fixture_path();
    let out = linkmarks_bin()
        .arg("list")
        .arg("--source=chrome")
        .arg("--path")
        .arg(&path)
        .arg("--format=json")
        .output()
        .expect("spawn linkmarks list");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(!lines.is_empty(), "no NDJSON lines emitted");
    // Each line must parse as JSON.
    for line in &lines {
        let v: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|e| panic!("bad JSON line {line:?}: {e}"));
        assert!(v.get("canonical_url").is_some());
    }
}

#[test]
fn dedupe_smoke_exits_zero_or_three() {
    let path = fixture_path();
    let out = linkmarks_bin()
        .arg("dedupe")
        .arg("--source=chrome")
        .arg("--path")
        .arg(&path)
        .output()
        .expect("spawn linkmarks dedupe");
    // Exit code is 0 (no conflicts) or 3 (conflicts).
    let code = out.status.code().unwrap_or(-1);
    assert!(
        code == 0 || code == 3,
        "unexpected exit code {code}; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// silence unused-import lint when no test references it.
#[allow(dead_code)]
fn _force_use(mut w: impl Write) {
    let _ = w.write_all(b"");
}

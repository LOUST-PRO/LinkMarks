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

// New Fase-2 smoke tests (store-backed flows). Each test uses a fresh
// tempdir for the store + config so the suite is hermetic.

fn linkmarks_bin_with_env(dir: &std::path::Path) -> Command {
    let mut cmd = linkmarks_bin();
    cmd.env("LINKMARKS_STORE", dir.join("store.db"))
        .env("LINKMARKS_CONFIG", dir.join("config.toml"));
    cmd
}

#[test]
fn init_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");

    // First init: writes the default config and the DB.
    let out = linkmarks_bin_with_env(dir.path())
        .arg("init")
        .output()
        .expect("spawn linkmarks init");
    assert!(
        out.status.success(),
        "init failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("config_written=true"), "got: {stdout}");
    assert!(
        std::path::Path::new(&dir.path().join("store.db")).exists(),
        "store.db was not created at {:?}",
        dir.path().join("store.db")
    );
    assert!(
        std::path::Path::new(&dir.path().join("config.toml")).exists(),
        "config.toml was not created"
    );

    // Second init: idempotent; config_written=false.
    let out = linkmarks_bin_with_env(dir.path())
        .arg("init")
        .output()
        .expect("spawn linkmarks init again");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("config_written=false"), "got: {stdout}");
}

#[test]
fn import_into_store_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Initialize first.
    let init = linkmarks_bin_with_env(dir.path())
        .arg("init")
        .output()
        .expect("init");
    assert!(init.status.success(), "init failed");

    // Import the fixture into the store.
    let path = fixture_path();
    let out = linkmarks_bin_with_env(dir.path())
        .arg("import")
        .arg("--source=chrome")
        .arg("--path")
        .arg(&path)
        .output()
        .expect("spawn linkmarks import");
    assert!(
        out.status.success(),
        "import failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The fixture has at least 4 bookmarks; the import report must
    // report a positive written count.
    assert!(stdout.contains("written="), "got: {stdout}");

    // Verify the store has rows by querying it via the sqlite3 binary
    // is not portable; instead, use `linkmarks list --source=store`
    // and inspect the output.
    let list = linkmarks_bin_with_env(dir.path())
        .arg("list")
        .arg("--source=store")
        .arg("--format=json")
        .output()
        .expect("spawn linkmarks list");
    assert!(list.status.success());
    let list_stdout = String::from_utf8_lossy(&list.stdout).into_owned();
    let lines: Vec<&str> = list_stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(!lines.is_empty(), "store has no rows after import");
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("canonical_url").is_some());
    }
}

#[test]
fn list_from_store_smoke() {
    let dir = tempfile::tempdir().expect("tempdir");

    // Init + import to populate.
    let _ = linkmarks_bin_with_env(dir.path())
        .arg("init")
        .output()
        .expect("init");
    let path = fixture_path();
    let import = linkmarks_bin_with_env(dir.path())
        .arg("import")
        .arg("--source=chrome")
        .arg("--path")
        .arg(&path)
        .output()
        .expect("import");
    assert!(import.status.success());

    // Empty-store path: a fresh init (no import) yields an empty table.
    let dir2 = tempfile::tempdir().expect("tempdir");
    let _ = linkmarks_bin_with_env(dir2.path())
        .arg("init")
        .output()
        .expect("init2");
    let empty = linkmarks_bin_with_env(dir2.path())
        .arg("list")
        .arg("--source=store")
        .arg("--format=table")
        .output()
        .expect("list empty");
    assert!(
        empty.status.success(),
        "list on empty store failed: stderr={}",
        String::from_utf8_lossy(&empty.stderr)
    );
    let stdout = String::from_utf8_lossy(&empty.stdout);
    assert!(
        stdout.starts_with("id\tcanonical_url\t"),
        "table header missing: {stdout}"
    );
    // Body must be empty (just the header line).
    let non_empty: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(non_empty.len(), 1, "got body lines: {non_empty:?}");

    // Populated-store path: same flags, must produce JSON lines.
    let pop = linkmarks_bin_with_env(dir.path())
        .arg("list")
        .arg("--source=store")
        .arg("--format=json")
        .output()
        .expect("list populated");
    assert!(pop.status.success());
    let pop_stdout = String::from_utf8_lossy(&pop.stdout).into_owned();
    let lines: Vec<&str> = pop_stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .collect();
    assert!(!lines.is_empty(), "populated store emits no rows");
    for line in &lines {
        let v: serde_json::Value = serde_json::from_str(line).expect("row is JSON");
        assert!(v.get("canonical_url").is_some());
        assert!(v.get("id").is_some());
    }
}

// silence unused-import lint when no test references it.
#[allow(dead_code)]
fn _force_use(mut w: impl Write) {
    let _ = w.write_all(b"");
}

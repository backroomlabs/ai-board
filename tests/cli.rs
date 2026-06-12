use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

fn board(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("abd").unwrap();
    cmd.env("BOARD_DB", dir.path().join("board.db"));
    cmd
}

fn init(dir: &TempDir) {
    board(dir).arg("init").assert().success();
}

fn make_design(dir: &TempDir) -> i64 {
    let spec = dir.path().join("spec.md");
    std::fs::write(&spec, "spec").unwrap();
    let out = board(dir)
        .args(["create-design", "--title", "D", "--file"])
        .arg(&spec)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    v["id"].as_i64().unwrap()
}

fn add_ticket(dir: &TempDir, design: i64, title: &str) -> i64 {
    let out = board(dir)
        .args([
            "add-ticket",
            "--design",
            &design.to_string(),
            "--title",
            title,
            "--spec",
            "do x",
            "--criteria",
            r#"["true => PASS"]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    v["id"].as_i64().unwrap()
}

#[test]
fn init_creates_db() {
    let dir = TempDir::new().unwrap();
    board(&dir).arg("init").assert().success();
    assert!(dir.path().join("board.db").exists());
}

#[test]
fn create_design_from_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec = dir.path().join("spec.md");
    std::fs::write(&spec, "# My Spec\nbody").unwrap();
    let out = board(&dir)
        .args(["create-design", "--title", "Auth", "--file"])
        .arg(&spec)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], 1);
    assert_eq!(v["title"], "Auth");
}

#[test]
fn create_design_from_stdin() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .args(["create-design", "--title", "Auth", "--stdin"])
        .write_stdin("# Via stdin")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], 1);
}

#[test]
fn add_ticket_ok() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let design = make_design(&dir);
    let out = board(&dir)
        .args([
            "add-ticket",
            "--design",
            &design.to_string(),
            "--title",
            "T",
            "--spec",
            "do x",
            "--criteria",
            r#"["cargo test => PASS"]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], 1);
}

#[test]
fn add_ticket_rejects_non_array_criteria() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let design = make_design(&dir);
    board(&dir)
        .args([
            "add-ticket",
            "--design",
            &design.to_string(),
            "--title",
            "T",
            "--spec",
            "do x",
            "--criteria",
            r#"{"not":"array"}"#,
        ])
        .assert()
        .failure();
}

#[test]
fn add_ticket_rejects_unknown_design() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    board(&dir)
        .args([
            "add-ticket",
            "--design",
            "999",
            "--title",
            "T",
            "--spec",
            "do x",
            "--criteria",
            r#"["true => PASS"]"#,
        ])
        .assert()
        .failure();
}

#[test]
fn next_claims_oldest_and_sets_implementing() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let d = make_design(&dir);
    let first = add_ticket(&dir, d, "first");
    let _second = add_ticket(&dir, d, "second");

    let out = board(&dir)
        .args(["next", "--design", &d.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], first);
    assert_eq!(v["status"], "implementing");
}

#[test]
fn next_returns_null_when_empty() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .arg("next")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert!(v["ticket"].is_null());
}

#[test]
fn show_includes_parent_design_md() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec = dir.path().join("spec.md");
    std::fs::write(&spec, "DESIGN BODY").unwrap();
    let out = board(&dir)
        .args(["create-design", "--title", "D", "--file"])
        .arg(&spec)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let d: i64 = serde_json::from_slice::<Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap();
    let t = add_ticket(&dir, d, "t1");

    let out = board(&dir)
        .args(["show", &t.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], t);
    assert_eq!(v["design_md"], "DESIGN BODY");
    assert!(v["acceptance_criteria"].is_array());
}

#[test]
fn list_returns_array() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let d = make_design(&dir);
    add_ticket(&dir, d, "a");
    add_ticket(&dir, d, "b");
    let out = board(&dir)
        .args(["list", "--design", &d.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn update_status_and_bump_attempts() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let d = make_design(&dir);
    let t = add_ticket(&dir, d, "a");
    let out = board(&dir)
        .args([
            "update",
            &t.to_string(),
            "--status",
            "needs_human",
            "--context",
            "stuck on X",
            "--bump-attempts",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["status"], "needs_human");
    assert_eq!(v["attempts"], 1);
    assert_eq!(v["human_context"], "stuck on X");
}

#[test]
fn update_rejects_bad_status() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let d = make_design(&dir);
    let t = add_ticket(&dir, d, "a");
    board(&dir)
        .args(["update", &t.to_string(), "--status", "bogus"])
        .assert()
        .failure();
}

#[test]
fn needs_human_returns_stranded_ticket() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let d = make_design(&dir);
    let t = add_ticket(&dir, d, "a");
    board(&dir)
        .args([
            "update",
            &t.to_string(),
            "--status",
            "needs_human",
            "--context",
            "q",
        ])
        .assert()
        .success();
    let out = board(&dir)
        .args(["needs-human", "--design", &d.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], t);
    assert_eq!(v["human_context"], "q");
}

#[test]
fn needs_human_null_when_none() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .arg("needs-human")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert!(v["ticket"].is_null());
}

#[test]
fn design_prints_raw_markdown() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec = dir.path().join("spec.md");
    std::fs::write(&spec, "# Raw\nmd").unwrap();
    let out = board(&dir)
        .args(["create-design", "--title", "D", "--file"])
        .arg(&spec)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let d: i64 = serde_json::from_slice::<Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap();
    let out = board(&dir)
        .args(["design", &d.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap().trim_end(), "# Raw\nmd");
}

#[test]
fn errors_emit_json_envelope_to_stderr() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .args(["show", "999"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].is_string());
}

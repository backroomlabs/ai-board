use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{json, Value};
use std::net::TcpListener;
use tempfile::TempDir;

fn board(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("abd").unwrap();
    cmd.env("BOARD_DB", dir.path().join("board.db"));
    cmd
}

fn init(dir: &TempDir) {
    board(dir).arg("init").assert().success();
}

fn make_legacy_db(dir: &TempDir) {
    let conn = rusqlite::Connection::open(dir.path().join("board.db")).unwrap();
    conn.execute("CREATE TABLE design (id INTEGER PRIMARY KEY)", [])
        .unwrap();
}

fn assert_json_error(dir: &TempDir, args: &[&str], expected: &str) {
    let stderr = board(dir)
        .args(args)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let value: Value = serde_json::from_slice(&stderr).unwrap();
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"], expected);
}

fn make_spec(dir: &TempDir) -> i64 {
    let content = dir.path().join("spec.md");
    std::fs::write(&content, "spec").unwrap();
    let out = board(dir)
        .args(["spec", "add", "--title", "S", "--file"])
        .arg(&content)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&out).unwrap();
    value["id"].as_i64().unwrap()
}

fn add_ticket(dir: &TempDir, spec_id: i64, title: &str) -> i64 {
    let out = board(dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            &spec_id.to_string(),
            "--title",
            title,
            "--description",
            "do x",
            "--dod",
            r#"["greeter exists"]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&out).unwrap();
    value["id"].as_i64().unwrap()
}

fn add_task(dir: &TempDir, ticket_id: i64, title: &str) -> i64 {
    let out = board(dir)
        .args([
            "task",
            "add",
            "--ticket-id",
            &ticket_id.to_string(),
            "--title",
            title,
            "--work-type",
            "code_implementation",
            "--objective",
            "detect range",
            "--criteria",
            r#"["cargo test range => PASS"]"#,
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice::<Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap()
}

#[test]
fn task_add_list_show_and_ticket_show_nests_tasks() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "targeting");
    let t1 = add_task(&dir, ticket_id, "range");
    let t2 = add_task(&dir, ticket_id, "nearest");

    let listed = board(&dir)
        .args(["task", "list", "--ticket-id", &ticket_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed: Value = serde_json::from_slice(&listed).unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0]["id"], t1);
    assert_eq!(listed[1]["id"], t2);
    assert_eq!(listed[0]["work_type"], "code_implementation");
    assert_eq!(listed[0]["context"], "");

    let shown = board(&dir)
        .args(["task", "show", &t1.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let shown: Value = serde_json::from_slice(&shown).unwrap();
    assert_eq!(shown["objective"], "detect range");
    assert_eq!(shown["acceptance_criteria"], json!(["cargo test range => PASS"]));

    let ticket = board(&dir)
        .args(["ticket", "show", &ticket_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let ticket: Value = serde_json::from_slice(&ticket).unwrap();
    assert_eq!(ticket["tasks"].as_array().unwrap().len(), 2);
    assert_eq!(ticket["tasks"][0]["title"], "range");

    let tickets = board(&dir)
        .args(["ticket", "list", "--spec-id", &spec_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let tickets: Value = serde_json::from_slice(&tickets).unwrap();
    assert!(tickets[0].get("tasks").is_none());
}

#[test]
fn task_add_rejects_unknown_ticket_and_work_type() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(
        &dir,
        &[
            "task",
            "add",
            "--ticket-id",
            "999",
            "--title",
            "T",
            "--work-type",
            "code_implementation",
            "--objective",
            "x",
            "--criteria",
            "[]",
        ],
        "ticket 999 not found",
    );
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "t");
    board(&dir)
        .args([
            "task",
            "add",
            "--ticket-id",
            &ticket_id.to_string(),
            "--title",
            "T",
            "--work-type",
            "refactor",
            "--objective",
            "x",
            "--criteria",
            "[]",
        ])
        .assert()
        .failure();
}

#[test]
fn task_add_omitted_context_is_empty_string() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "t");
    let id = add_task(&dir, ticket_id, "range");
    let shown = board(&dir)
        .args(["task", "show", &id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let shown: Value = serde_json::from_slice(&shown).unwrap();
    assert_eq!(shown["context"], "");
}

#[test]
fn task_add_context_round_trips() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "t");
    let out = board(&dir)
        .args([
            "task",
            "add",
            "--ticket-id",
            &ticket_id.to_string(),
            "--title",
            "range",
            "--work-type",
            "code_implementation",
            "--objective",
            "detect range",
            "--criteria",
            r#"["cargo test range => PASS"]"#,
            "--context",
            "notes",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let id = serde_json::from_slice::<Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap();
    let shown = board(&dir)
        .args(["task", "show", &id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let shown: Value = serde_json::from_slice(&shown).unwrap();
    assert_eq!(shown["context"], "notes");
}

#[test]
fn task_show_unknown_id_errors() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(&dir, &["task", "show", "999"], "task 999 not found");
}

#[test]
fn task_list_rejects_unknown_ticket() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(
        &dir,
        &["task", "list", "--ticket-id", "999"],
        "ticket 999 not found",
    );
}

#[test]
fn init_creates_db() {
    let dir = TempDir::new().unwrap();
    board(&dir).arg("init").assert().success();
    assert!(dir.path().join("board.db").exists());
}

#[test]
fn spec_add_from_file() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let content = dir.path().join("spec.md");
    std::fs::write(&content, "# Authentication\nbody").unwrap();
    let out = board(&dir)
        .args(["spec", "add", "--title", "Auth", "--file"])
        .arg(&content)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["id"], 1);
    assert_eq!(value["title"], "Auth");
}

#[test]
fn spec_add_from_stdin() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .args(["spec", "add", "--title", "Auth", "--stdin"])
        .write_stdin("# Via stdin")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(value["id"], 1);
}

#[test]
fn add_ticket_ok() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let out = board(&dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            &spec_id.to_string(),
            "--title",
            "T",
            "--description",
            "do x",
            "--dod",
            r#"["greeter exists"]"#,
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
fn add_ticket_rejects_non_array_dod() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    board(&dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            &spec_id.to_string(),
            "--title",
            "T",
            "--description",
            "do x",
            "--dod",
            r#"{"not":"array"}"#,
        ])
        .assert()
        .failure();
}

#[test]
fn add_ticket_rejects_unknown_spec() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    board(&dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            "999",
            "--title",
            "T",
            "--description",
            "do x",
            "--dod",
            r#"["greeter exists"]"#,
        ])
        .assert()
        .failure();
}

#[test]
fn next_claims_oldest_and_sets_implementing() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let first = add_ticket(&dir, spec_id, "first");
    let _second = add_ticket(&dir, spec_id, "second");

    let out = board(&dir)
        .args(["next", "--spec-id", &spec_id.to_string()])
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
fn next_rejects_unknown_spec() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(&dir, &["next", "--spec-id", "999"], "spec 999 not found");
}

#[test]
fn show_returns_ticket_without_parent_content() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "t1");

    let out = board(&dir)
        .args(["ticket", "show", &ticket_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&out).unwrap();

    assert_eq!(value["spec_id"], spec_id);
    assert_eq!(value["description"], "do x");
    assert_eq!(value["definitions_of_done"], json!(["greeter exists"]));
    assert!(value.get("acceptance_criteria").is_none());
    assert!(value.get("content").is_none());
    assert!(value.get("design_md").is_none());
}

#[test]
fn list_returns_array() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    add_ticket(&dir, spec_id, "a");
    add_ticket(&dir, spec_id, "b");
    let out = board(&dir)
        .args(["ticket", "list", "--spec-id", &spec_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[test]
fn list_rejects_unknown_spec() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(
        &dir,
        &["ticket", "list", "--spec-id", "999"],
        "spec 999 not found",
    );
}

#[test]
fn update_status_and_bump_attempts() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "a");
    let out = board(&dir)
        .args([
            "update",
            &ticket_id.to_string(),
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
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "a");
    board(&dir)
        .args(["update", &ticket_id.to_string(), "--status", "bogus"])
        .assert()
        .failure();
}

#[test]
fn needs_human_returns_stranded_ticket() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let spec_id = make_spec(&dir);
    let ticket_id = add_ticket(&dir, spec_id, "a");
    board(&dir)
        .args([
            "update",
            &ticket_id.to_string(),
            "--status",
            "needs_human",
            "--context",
            "q",
        ])
        .assert()
        .success();
    let out = board(&dir)
        .args(["needs-human", "--spec-id", &spec_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["id"], ticket_id);
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
fn needs_human_rejects_unknown_spec() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    assert_json_error(
        &dir,
        &["needs-human", "--spec-id", "999"],
        "spec 999 not found",
    );
}

#[test]
fn spec_get_prints_raw_content() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let content = dir.path().join("spec.md");
    std::fs::write(&content, "# Raw\ncontent").unwrap();
    let out = board(&dir)
        .args(["spec", "add", "--title", "S", "--file"])
        .arg(&content)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let spec_id = serde_json::from_slice::<Value>(&out).unwrap()["id"]
        .as_i64()
        .unwrap();
    let out = board(&dir)
        .args(["spec", "get", &spec_id.to_string()])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap().trim_end(), "# Raw\ncontent");
}

#[test]
fn spec_list_returns_newest_first() {
    let dir = TempDir::new().unwrap();
    init(&dir);

    let content = dir.path().join("spec.md");
    std::fs::write(&content, "first").unwrap();
    board(&dir)
        .args(["spec", "add", "--title", "First", "--file"])
        .arg(&content)
        .assert()
        .success();
    std::fs::write(&content, "second").unwrap();
    board(&dir)
        .args(["spec", "add", "--title", "Second", "--file"])
        .arg(&content)
        .assert()
        .success();

    let out = board(&dir)
        .args(["spec", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let values: Value = serde_json::from_slice(&out).unwrap();
    let values = values.as_array().unwrap();
    assert_eq!(values[0]["title"], "Second");
    assert_eq!(values[1]["title"], "First");
}

#[test]
fn spec_list_empty_returns_empty_array() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .args(["spec", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 0);
}

#[test]
fn removed_design_commands_and_flags_are_rejected() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let content = dir.path().join("legacy-design.md");
    std::fs::write(&content, "legacy").unwrap();

    board(&dir)
        .args(["create-design", "--title", "Legacy", "--file"])
        .arg(&content)
        .assert()
        .failure();
    board(&dir).args(["design", "list"]).assert().failure();
    board(&dir)
        .args(["next", "--design", "1"])
        .assert()
        .failure();
    board(&dir).args(["add-ticket"]).assert().failure();
    board(&dir).args(["show", "1"]).assert().failure();
    board(&dir)
        .args(["list", "--spec-id", "1"])
        .assert()
        .failure();
    let spec_id = make_spec(&dir).to_string();
    board(&dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            &spec_id,
            "--title",
            "T",
            "--description",
            "valid description",
            "--criteria",
            "[]",
        ])
        .assert()
        .failure();
    board(&dir)
        .args([
            "ticket",
            "add",
            "--spec-id",
            &spec_id,
            "--title",
            "T",
            "--description",
            "valid description",
            "--spec",
            "old field",
            "--dod",
            "[]",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unexpected argument '--spec' found",
        ));
}

#[test]
fn legacy_schema_is_rejected_by_init_and_non_init_commands() {
    let dir = TempDir::new().unwrap();
    make_legacy_db(&dir);

    for args in [vec!["init"], vec!["spec", "list"]] {
        let stderr = board(&dir)
            .args(args)
            .assert()
            .failure()
            .get_output()
            .stderr
            .clone();
        let value: Value = serde_json::from_slice(&stderr).unwrap();
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("legacy design schema"));
    }
}

#[test]
fn legacy_schema_is_rejected_before_missing_spec_file_validation() {
    let dir = TempDir::new().unwrap();
    make_legacy_db(&dir);
    let missing = dir.path().join("missing.md");
    let stderr = board(&dir)
        .args(["spec", "add", "--title", "S", "--file"])
        .arg(&missing)
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let value: Value = serde_json::from_slice(&stderr).unwrap();
    assert!(value["error"]
        .as_str()
        .unwrap()
        .contains("legacy design schema"));
    assert!(!value["error"]
        .as_str()
        .unwrap()
        .contains("No such file or directory"));
}

#[test]
fn legacy_schema_is_rejected_before_serve_port_check() {
    let dir = TempDir::new().unwrap();
    make_legacy_db(&dir);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let stderr = board(&dir)
        .args(["serve", "--port", &port])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let value: Value = serde_json::from_slice(&stderr).unwrap();
    assert!(value["error"]
        .as_str()
        .unwrap()
        .contains("legacy design schema"));
}

#[test]
fn serve_help_describes_editable_board() {
    board(&TempDir::new().unwrap())
        .args(["serve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("editable"))
        .stdout(predicate::str::contains("read-only").not());
}

#[test]
fn init_creates_spec_and_ticket_columns() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let conn = rusqlite::Connection::open(dir.path().join("board.db")).unwrap();

    let spec_columns: Vec<String> = conn
        .prepare("PRAGMA table_info(spec)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(spec_columns.contains(&"content".to_string()));

    let ticket_columns: Vec<String> = conn
        .prepare("PRAGMA table_info(ticket)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(ticket_columns.contains(&"spec_id".to_string()));
    assert!(ticket_columns.contains(&"description".to_string()));
    assert!(ticket_columns.contains(&"definitions_of_done".to_string()));
    assert!(!ticket_columns.contains(&"acceptance_criteria".to_string()));
    assert!(!ticket_columns.contains(&"design_id".to_string()));
    assert!(!ticket_columns.contains(&"spec".to_string()));

    let task_columns: Vec<String> = conn
        .prepare("PRAGMA table_info(task)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(task_columns.contains(&"ticket_id".to_string()));
    assert!(task_columns.contains(&"work_type".to_string()));
    assert!(task_columns.contains(&"objective".to_string()));
    assert!(task_columns.contains(&"acceptance_criteria".to_string()));
    assert!(task_columns.contains(&"context".to_string()));
}

#[test]
fn ticket_acceptance_criteria_schema_is_rejected() {
    let dir = TempDir::new().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("board.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE spec (id INTEGER PRIMARY KEY);
         CREATE TABLE ticket (
            id INTEGER PRIMARY KEY,
            spec_id INTEGER,
            title TEXT,
            description TEXT,
            acceptance_criteria TEXT,
            status TEXT
         );",
    )
    .unwrap();
    let stderr = board(&dir)
        .arg("init")
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let value: Value = serde_json::from_slice(&stderr).unwrap();
    let error = value["error"].as_str().unwrap();
    assert!(error.contains("acceptance_criteria"));
    assert!(error.contains("recreate the board database"));
}

#[test]
fn errors_emit_json_envelope_to_stderr() {
    let dir = TempDir::new().unwrap();
    init(&dir);
    let out = board(&dir)
        .args(["ticket", "show", "999"])
        .assert()
        .failure()
        .get_output()
        .stderr
        .clone();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].is_string());
}

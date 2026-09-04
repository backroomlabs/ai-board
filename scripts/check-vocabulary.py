#!/usr/bin/env python3

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SCAN_ROOTS = ("README.md", "src", "tests", "skills", "demo", "scripts")

# scanner-self-definition:start
FORBIDDEN = (
    (
        "create-design",
        re.compile(
            r"(?:\bcreate-design\b|\bCreateDesign[A-Za-z0-9_]*\b|"
            r"\bcreate_design[A-Za-z0-9_]*\b)"
        ),
    ),
    (
        "design-command",
        re.compile(
            r'(?:\babd\s+design\s+(?:list|show)\b|'
            r'\[\s*"design"\s*,\s*"(?:list|show)"\s*\])'
        ),
    ),
    ("--design", re.compile(r"--design\b")),
    ("design_id", re.compile(r"\bdesign_id\b")),
    ("design_md", re.compile(r"\bdesign_md\b")),
    ("/api/design", re.compile(r"/api/design")),
    ("design-query", re.compile(r"[?&]design=")),
    ("ticket.spec", re.compile(r"\bticket\.spec\b")),
    ("--spec", re.compile(r"--spec(?!-id\b)(?=[^A-Za-z0-9]|$)")),
    ("ticket.acceptance_criteria", re.compile(r"\bticket\.acceptance_criteria\b")),
    (
        "ticket-add-criteria",
        re.compile(r"\babd\s+ticket\s+add\b.*--criteria\b"),
    ),
    (
        "ticket-patch-acceptance_criteria",
        re.compile(
            r"JSON\.stringify\(\{\s*title,\s*description,\s*acceptance_criteria\s*\}"
        ),
    ),
)

SELF_TEST_CASES = (
    ("abd create-design --title X", "create-design"),
    ("Cmd::CreateDesign", "create-design"),
    ("CreateDesignArgs", "create-design"),
    ("fn create_design()", "create-design"),
    ('args(["next", "--design", "1"])', "--design"),
    ('args(["next", "--design-id", "1"])', "--design"),
    ("ticket.design_id", "design_id"),
    ("ticket.design_md", "design_md"),
    ('fetch("/api/designs")', "/api/design"),
    ('fetch("/api/board?design=1")', "design-query"),
    ("ticket.spec", "ticket.spec"),
    ('args(["--spec", "old"])', "--spec"),
    ("ticket.acceptance_criteria", "ticket.acceptance_criteria"),
    (
        "abd ticket add --spec-id 1 --title T --description d --criteria '[]'",
        "ticket-add-criteria",
    ),
    (
        "JSON.stringify({ title, description, acceptance_criteria })",
        "ticket-patch-acceptance_criteria",
    ),
)

COMPATIBILITY_ALLOWLIST = {
    ("tests/cli.rs", '.args(["create-design", "--title", "Legacy", "--file"])'),
    ("tests/cli.rs", 'board(&dir).args(["design", "list"]).assert().failure();'),
    ("tests/cli.rs", '.args(["next", "--design", "1"])'),
    ("tests/cli.rs", '"--spec",'),
    ("tests/cli.rs", '"unexpected argument \'--spec\' found",'),
    ("tests/cli.rs", 'assert!(value.get("design_md").is_none());'),
    (
        "tests/cli.rs",
        'assert!(!ticket_columns.contains(&"design_id".to_string()));',
    ),
    # Negative: ticket add --criteria must fail (clap / renamed flag)
    ("tests/cli.rs", '"--criteria",'),
    ("src/serve.rs", 'assert!(!INDEX_HTML.contains("/api/design"));'),
    ("src/serve.rs", 'assert!(!INDEX_HTML.contains("ticket.spec"));'),
    ("src/serve.rs", 'assert!(route_json("/api/designs").is_err());'),
    ("src/serve.rs", 'assert!(route_json("/api/design/1").is_err());'),
    ("src/serve.rs", 'assert!(route_json("/api/board?design=1").is_err());'),
    # Preflight + negative UI/API guards for legacy ticket.acceptance_criteria
    (
        "src/db.rs",
        '"legacy ticket.acceptance_criteria schema detected at {}; recreate the board database",',
    ),
    (
        "src/serve.rs",
        'assert!(!INDEX_HTML.contains("ticket.acceptance_criteria"));',
    ),
    (
        "src/serve.rs",
        'assert!(!INDEX_HTML.contains("JSON.stringify({ title, description, acceptance_criteria })"));',
    ),
    # Contract script mentions forbidden tokens while grepping for them
    (
        "scripts/check-skills.sh",
        "grep -q 'ticket.acceptance_criteria' skills/board-planning/SKILL.md \\",
    ),
    (
        "scripts/check-skills.sh",
        '&& fail "board-planning still mentions ticket.acceptance_criteria"',
    ),
    (
        "scripts/check-skills.sh",
        "grep -q 'abd ticket add --criteria' skills/board-planning/SKILL.md \\",
    ),
}
# scanner-self-definition:end


def token_kinds(line):
    return {name for name, pattern in FORBIDDEN if pattern.search(line)}


def self_test():
    failures = []
    for sample, expected in SELF_TEST_CASES:
        if expected not in token_kinds(sample):
            failures.append(f"{expected}: {sample}")
    if token_kinds("abd next --spec-id 7"):
        failures.append("--spec-id must remain allowed")
    if token_kinds(
        'abd task add --ticket-id 1 --title T --work-type code_implementation '
        '--objective o --criteria \'["x"]\''
    ):
        failures.append("task add --criteria must remain allowed")
    if failures:
        print("scanner self-test missed:\n" + "\n".join(failures), file=sys.stderr)
        return 1
    return 0


def files_to_scan():
    for root_name in SCAN_ROOTS:
        root = ROOT / root_name
        if root.is_file():
            yield root
        else:
            yield from (path for path in root.rglob("*") if path.is_file())


def in_scanner_definition(relative_path, line_number, lines):
    if relative_path != "scripts/check-vocabulary.py":
        return False
    start = next(i for i, line in enumerate(lines, 1) if "scanner-self-definition:start" in line)
    end = next(i for i, line in enumerate(lines, 1) if "scanner-self-definition:end" in line)
    return start <= line_number <= end


def main():
    if self_test():
        return 1

    findings = []
    for path in files_to_scan():
        try:
            lines = path.read_text().splitlines()
        except UnicodeDecodeError:
            continue
        relative_path = path.relative_to(ROOT).as_posix()
        for line_number, line in enumerate(lines, 1):
            kinds = token_kinds(line)
            if not kinds:
                continue
            if in_scanner_definition(relative_path, line_number, lines):
                continue
            if (relative_path, line.strip()) in COMPATIBILITY_ALLOWLIST:
                continue
            findings.append(
                f"{relative_path}:{line_number}: {','.join(sorted(kinds))}: {line.strip()}"
            )

    if findings:
        print("\n".join(findings), file=sys.stderr)
        print("FAIL: stale design-domain vocabulary remains", file=sys.stderr)
        return 1
    print("OK: stale vocabulary scan passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

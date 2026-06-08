#!/usr/bin/env bash
set -euo pipefail
fail() { echo "FAIL: $1"; exit 1; }

# brainstorming: handoff now calls the board CLI, not a bare writing-plans handoff
grep -q 'board create-design' skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming missing 'board create-design' handoff"
grep -q 'design_id' skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming missing design_id capture"

# board-planning: emits tickets, no plan.md, enforces teeth rule
grep -q 'board add-ticket' skills/board-planning/SKILL.md \
  || fail "board-planning missing 'board add-ticket'"
grep -qi 'plan\.md' skills/board-planning/SKILL.md \
  && fail "board-planning still references plan.md"
grep -q 'Plan Document Header' skills/board-planning/SKILL.md \
  && fail "board-planning still has Plan Document Header section"
grep -qi 'JSON array' skills/board-planning/SKILL.md \
  || fail "board-planning missing JSON-array criteria rule"
grep -qi 'reject' skills/board-planning/SKILL.md \
  || fail "board-planning missing prose-criteria rejection rule"

# board-execute: needs-human first, then next; cap 3; never weaken
grep -q 'board needs-human' skills/board-execute/SKILL.md \
  || fail "board-execute missing needs-human startup check"
grep -q 'board next' skills/board-execute/SKILL.md \
  || fail "board-execute missing board next"
grep -qi 'cap' skills/board-execute/SKILL.md \
  || fail "board-execute missing attempts cap"
grep -qi 'never weaken' skills/board-execute/SKILL.md \
  || fail "board-execute missing 'never weaken a criterion' rule"

# every skill has parseable frontmatter with name + description
python3 - <<'PY'
import pathlib
expected = {
    "board-brainstorming": "board-brainstorming",
    "board-planning":      "board-planning",
    "board-execute":       "board-execute",
    "using-ai-board":      "using-ai-board",
}
for dirname, name in expected.items():
    text = pathlib.Path(f"skills/{dirname}/SKILL.md").read_text()
    assert text.startswith("---\n"), f"{dirname}: no frontmatter"
    fm = text.split("---\n", 2)[1]
    assert f"name: {name}" in fm, f"{dirname}: expected name '{name}', got:\n{fm[:120]}"
    assert "description:" in fm, f"{dirname}: missing description"
PY

echo "OK: all skill checks passed"

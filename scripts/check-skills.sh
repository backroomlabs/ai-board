#!/usr/bin/env bash
set -euo pipefail
fail() { echo "FAIL: $1"; exit 1; }

# brainstorming persists the approved spec and hands its id to board-planning
grep -q 'abd spec add' skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming missing 'abd spec add' handoff"
grep -q 'spec_id' skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming missing spec_id capture"
grep -q 'board-planning' skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming missing board-planning handoff"
grep -q 'skills/board-brainstorming/visual-companion.md' \
  skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming has wrong visual companion path"
grep -Fq '"User reviews spec?" -> "Persist spec with abd spec add\ncapture spec_id" [label="approved"];' \
  skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming flow skips spec persistence"
grep -Fq '"Persist spec with abd spec add\ncapture spec_id" -> "Invoke board-planning skill";' \
  skills/board-brainstorming/SKILL.md \
  || fail "board-brainstorming flow skips board-planning handoff"

# planning emits tickets (--dod) then tasks; no ticket --criteria
grep -q 'abd ticket add' skills/board-planning/SKILL.md \
  || fail "board-planning missing 'abd ticket add'"
grep -q -- '--dod' skills/board-planning/SKILL.md \
  || fail "board-planning missing --dod"
grep -q 'abd ticket add --criteria' skills/board-planning/SKILL.md \
  && fail "board-planning still uses ticket --criteria"
grep -q 'abd task add' skills/board-planning/SKILL.md \
  || fail "board-planning missing 'abd task add'"
grep -q -- '--work-type' skills/board-planning/SKILL.md \
  || fail "board-planning missing --work-type"
grep -q 'ticket.acceptance_criteria' skills/board-planning/SKILL.md \
  && fail "board-planning still mentions ticket.acceptance_criteria"

for wt in code_implementation investigation documentation design; do
  grep -q "$wt" skills/board-planning/SKILL.md \
    || fail "board-planning missing legal work-type '$wt'"
done
grep -q 'human_input' skills/board-planning/SKILL.md \
  && fail "board-planning still mentions illegal work-type human_input"
grep -q '|decision>' skills/board-planning/SKILL.md \
  && fail "board-planning still mentions illegal work-type decision"

grep -q 'abd next' skills/board-execute/SKILL.md \
  && fail "board-execute still uses abd next as the loop"
grep -q 'Runs' skills/board-execute/SKILL.md \
  || fail "board-execute missing Runs handoff"

if [ -f scripts/check-vocabulary.py ]; then
  python3 scripts/check-vocabulary.py
fi
if [ -f scripts/check-editor-requests.js ]; then
  node scripts/check-editor-requests.js
fi

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

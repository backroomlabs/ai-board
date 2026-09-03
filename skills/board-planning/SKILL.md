---
name: board-planning
description: Use instead of superpowers:writing-plans when working in an ai-board project — decomposes a spec into board tickets via 'abd ticket add' with machine-checkable acceptance_criteria, then hands off to board-execute.
---

# Writing Plans

## Overview

Write comprehensive implementation plans assuming the engineer has zero context for our codebase and questionable taste. Document everything they need to know: which files to touch for each task, code, testing, docs they might need to check, how to test it. Give them the whole plan as bite-sized tasks. DRY. YAGNI. TDD. Frequent commits.

Assume they are a skilled developer, but know almost nothing about our toolset or problem domain. Assume they don't know good test design very well.

**Announce at start:** "I'm using the board-planning skill to create the implementation plan."

**Context:** If working in an isolated worktree, it should have been created via the `superpowers:using-git-worktrees` skill at execution time.

**Output:** This skill does NOT write a plan file. Each task is emitted as a board ticket via `abd ticket add` (see Output Target below). The board is the single source of truth; `spec_id` is passed in by the brainstorming skill.

## Scope Check

If the spec covers multiple independent subsystems, it should have been broken into sub-project specs during brainstorming. If it wasn't, suggest breaking this into separate plans — one per subsystem. Each plan should produce working, testable software on its own.

## File Structure

Before defining tasks, map out which files will be created or modified and what each one is responsible for. This is where decomposition decisions get locked in.

- Design units with clear boundaries and well-defined interfaces. Each file should have one clear responsibility.
- You reason best about code you can hold in context at once, and your edits are more reliable when files are focused. Prefer smaller, focused files over large ones that do too much.
- Files that change together should live together. Split by responsibility, not by technical layer.
- In existing codebases, follow established patterns. If the codebase uses large files, don't unilaterally restructure - but if a file you're modifying has grown unwieldy, including a split in the plan is reasonable.

This structure informs the task decomposition. Each task should produce self-contained changes that make sense independently.

## Output Target: Board Tickets

This skill does NOT write a `plan` file. For each task you would have written, call the `abd` CLI once:

```bash
abd ticket add --spec-id <spec_id> \
  --title "<task component name>" \
  --description "<complete self-contained implementation instructions>" \
  --criteria '<JSON array of exact commands + expected result>'
```

Mapping:
- `--title` ← the task/component name.
- `--description` ← the `Files:` block plus the full implementation steps and code (the same bite-sized, no-placeholder content you'd otherwise have written into a plan file).
- `--criteria` ← the verification steps as a **JSON array of checkable commands**, each ending in its expected result, e.g. `["pytest tests/auth.py::test_x -v => PASS", "tsc --noEmit => clean"]`.

Every ticket description is sufficient for a fresh execution agent:

- Include exact files, complete implementation steps, relevant interfaces, and all constraints needed for that ticket.
- A ticket may build on code committed by earlier tickets, but must not require reading another ticket's text or the parent spec.
- Each ticket should produce one reviewable commit.

Run the **Self-Review** checklist BEFORE emitting any tickets.

## CRITICAL: Acceptance Criteria Must Be Machine-Checkable

`--criteria` is a JSON array of **exact commands with expected results** — never prose. This is the same discipline as the No Placeholders rule, applied to the criteria field. The whole system's teeth depend on this line.

**REJECT** as criteria any of: "implement correctly", "handle edge cases", "works as expected", "passes review", or any sentence a machine cannot run and check. If a task has no runnable verification, that is a planning failure — make the task test-shaped (TDD red/green) so it does.

Good:   `["cargo test auth::login -v => PASS", "curl -s localhost:4141/health => 200"]`
Bad:    `["login works", "no regressions"]`

The `abd` CLI also rejects non-array criteria, but you must never lean on that — emit a real, runnable array every time.

## Bite-Sized Task Granularity

**Each step is one action (2-5 minutes):**
- "Write the failing test" - step
- "Run it to make sure it fails" - step
- "Implement the minimal code to make the test pass" - step
- "Run the tests and make sure they pass" - step
- "Commit" - step

## Task Structure

````markdown
### Task N: [Component Name]

**Files:**
- Create: `exact/path/to/file.py`
- Modify: `exact/path/to/existing.py:123-145`
- Test: `tests/exact/path/to/test.py`

- [ ] **Step 1: Write the failing test**

```python
def test_specific_behavior():
    result = function(input)
    assert result == expected
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pytest tests/path/test.py::test_name -v`
Expected: FAIL with "function not defined"

- [ ] **Step 3: Write minimal implementation**

```python
def function(input):
    return expected
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pytest tests/path/test.py::test_name -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add tests/path/test.py src/path/file.py
git commit -m "feat: add specific feature"
```
````

## No Placeholders

Every step must contain the actual content an engineer needs. These are **plan failures** — never write them:
- "TBD", "TODO", "implement later", "fill in details"
- "Add appropriate error handling" / "add validation" / "handle edge cases"
- "Write tests for the above" (without actual test code)
- "Similar to Task N" (repeat the code — the engineer may be reading tasks out of order)
- Steps that describe what to do without showing how (code blocks required for code steps)
- References to types, functions, or methods not defined in any task

## Remember
- Exact file paths always
- Complete code in every step — if a step changes code, show the code
- Exact commands with expected output
- DRY, YAGNI, TDD, frequent commits

## Self-Review

After writing the complete plan, look at the spec with fresh eyes and check the plan against it. This is a checklist you run yourself — not a subagent dispatch.

**1. Spec coverage:** Skim each section/requirement in the spec. Can you point to a task that implements it? List any gaps.

**2. Placeholder scan:** Search your plan for red flags — any of the patterns from the "No Placeholders" section above. Fix them.

**3. Type consistency:** Do the types, method signatures, and property names you used in later tasks match what you defined in earlier tasks? A function called `clearLayers()` in Task 3 but `clearFullLayers()` in Task 7 is a bug.

If you find issues, fix them inline. No need to re-review — just fix and move on. If you find a spec requirement with no task, add the task.

## Handoff

After all tickets are emitted and Self-Review has passed:

**1. Start the board UI and direct the user to review.** Run this first (idempotent — safe if already running):

```bash
abd serve --port 4141 &
```

Then tell the user:

> "N tickets are queued on the board. Open http://localhost:4141 to review the plan — click each card to see its description and acceptance criteria. Let me know when you're happy with it, or tell me what to change."

**2. Wait for explicit approval.** Do not invoke `board-execute` until the user says yes. If they request content changes, direct them to edit the affected tickets in the board UI, then ask them to re-check the board. Do not claim that ticket deletion is supported.

**3. On approval, invoke `board-execute`** with `spec_id`. There is no other execution path.

---
name: board-execute
description: Use after board-planning has emitted tickets — runs the sequential execute loop (claim ticket, dispatch sub-agent, verify acceptance_criteria via real exit codes, mark done or needs_human) until the board is drained.
---

# Execute Ticket Loop

Drive a design's tickets to `done` using the board as the single source of
truth and the harness's native sub-agent dispatch. One worker, sequential.

You are given a `design_id`. **The user has already reviewed and approved the
ticket plan** (that gate lives in `board-planning`). Do not re-ask for approval
— start the loop immediately.

## Hard Rules

- **Never guess on ambiguity — stop and ask.** Write a self-contained question to
  `human_context` and set the ticket `needs_human`.
- **Never weaken a criterion to make it pass.** If `acceptance_criteria` fails,
  fix the code, not the criterion.
- **Cap attempts at 3.** On the 3rd failed attempt, escalate to `needs_human`.
- **Always set `verifying` before running criteria — no exceptions.** Call
  `abd update <id> --status verifying` BEFORE running any acceptance_criteria
  command. Never jump from `implementing` directly to `done`.

## The Loop

### 0. Startup: resume any stranded question FIRST

Before claiming new work, check for a question left by a previous (possibly
crashed) session:

```bash
abd needs-human --design <design_id>
```

If it returns a ticket, surface its `human_context` conversationally — assume the
human has zero memory of the session ("Last session I was blocked on ticket N:
…"). Wait for their answer. The answer lives in chat context only; it is never
persisted. Once answered, requeue the ticket and continue:

```bash
abd update <ticket_id> --status queued
```

### 1. Claim the next ticket

```bash
abd next --design <design_id>
```

If the result is `{"ticket": null}`, the board is drained — all tickets are
`done`. Stop; report completion. Otherwise you now hold a ticket in
`implementing` (the claim is atomic).

### 2. Read the full ticket

```bash
abd show <ticket_id>
```

This includes the parent `design_md` for context, the `spec`, and the
`acceptance_criteria` array.

### 3. Dispatch a FRESH sub-agent to implement

Dispatch a new sub-agent (native harness dispatch) to implement the ticket per
its `spec`. A fresh context per ticket is the point — do not implement inline in
the orchestrator's context.

### 4. Verify — run the acceptance_criteria

**First, immediately call this — before running any criteria:**

```bash
abd update <ticket_id> --status verifying
```

Then run EVERY command in `acceptance_criteria` and read exit codes. This
deterministic check IS the review; there is no LLM reviewer in v1.

- **All criteria pass** →

  ```bash
  abd update <ticket_id> --status done
  ```

  Go to step 1.

- **Any criterion fails** and `attempts < 3` →

  ```bash
  abd update <ticket_id> --status implementing --bump-attempts
  ```

  Retry from step 3 (dispatch a fresh sub-agent with the failure context).

- **3rd failure, OR genuine ambiguity** → write a self-contained question and
  stop:

  ```bash
  abd update <ticket_id> --status needs_human \
    --context "Ticket N (<title>): <what failed / what is ambiguous>. Tried: <attempts>. Need: <exact decision required>."
  ```

  Then STOP and surface the question to the human.

### 5. Loop

Return to step 1 until `abd next` yields `{"ticket": null}`.

## needs_human is normal

Hitting `needs_human` is an expected path, not an edge case — LLM
non-determinism and unforeseen real dependencies make "stop and ask" routine.
The persisted question survives a crash; the loop resumes from step 0.

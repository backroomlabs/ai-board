---
name: board-execute
description: Use after board-planning has emitted tickets — runs the sequential execute loop (claim ticket, dispatch sub-agent, verify acceptance_criteria via real exit codes, mark done or needs_human) until the board is drained.
---

# Execute Ticket Loop

Drive a spec's tickets to `done` using the board as the single source of
truth and the harness's native sub-agent dispatch. One worker, sequential.

You are given a `spec_id`. **The user has already reviewed and approved the
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

### 0. Startup: recover stranded tickets

Before claiming new work, check for a question left by a previous (possibly
crashed) session:

```bash
abd needs-human --spec-id <spec_id>
```

If it returns a ticket, surface its `human_context` conversationally — assume the
human has zero memory of the session ("Last session I was blocked on ticket N:
…"). Wait for their answer. The answer lives in chat context only; it is never
persisted. Once answered, requeue the ticket and continue:

```bash
abd update <ticket_id> --status queued
```

Also recover work stranded in `implementing` by a crashed session:

```bash
abd list --spec-id <spec_id>
```

Find every ticket with `"status": "implementing"` and reset it before claiming
new work:

```bash
abd update <ticket_id> --status queued
```

The abandoned worker did not finish, so the ticket must be reclaimed and
implemented from scratch. Both recovery checks belong to this startup step and
run on every invocation.

### 1. Claim the next ticket

```bash
abd next --spec-id <spec_id>
```

If the result is `{"ticket": null}`, the board is drained — all tickets are
`done`. Stop; report completion. Otherwise you now hold a ticket in
`implementing` (the claim is atomic).

### 2. Dispatch a FRESH sub-agent to implement

The successful `abd next` response is the complete self-contained ticket. Treat
that response as the complete ticket and dispatch a new sub-agent (native
harness dispatch) with it. Do not fetch the parent spec. A fresh context per
ticket is the point — do not implement inline in the orchestrator's context.

### 3. Verify — run the acceptance_criteria

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

  Retry from step 2. Dispatch a fresh sub-agent with the same ticket
  description plus the failed command and its observed output.

- **3rd failure, OR genuine ambiguity** → write a self-contained question and
  stop:

  ```bash
  abd update <ticket_id> --status needs_human \
    --context "Ticket N (<title>): <what failed / what is ambiguous>. Tried: <attempts>. Need: <exact decision required>."
  ```

  Then STOP and surface the question to the human.

### 4. Loop

Return to step 1 until `abd next` yields `{"ticket": null}`.

## needs_human is normal

Hitting `needs_human` is an expected path, not an edge case — LLM
non-determinism and unforeseen real dependencies make "stop and ask" routine.
The persisted question survives a crash; the loop resumes from step 0.

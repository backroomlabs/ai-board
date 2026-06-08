# AI Board — Design

A board-backed orchestration layer for agentic software development. It replaces
Superpowers' markdown `plan.md` with a **SQLite board as the single source of
truth** for one workflow: **brainstorm → plan → execute**, with a human gate.

Geared toward harnesses that can spawn sub-agents natively — **Claude Code** and
**Codex** — so per-ticket work runs in isolated sub-agent contexts.

---

## 1. Motivation

Superpowers already nails decomposition: it turns a rough idea into a spec, then
a plan of bite-sized, TDD-shaped tasks with exact file paths and verification
commands. But it stores the plan as a single markdown file that a sub-agent walks
top-to-bottom in one session.

A markdown plan can't do three things a board can, and those three are the entire
justification for this project:

- **Atomic task claiming** — `queued → implementing` in one statement, so a task
  is never picked up twice (and the model extends to a worker pool later).
- **Persisted state across crashes** — if the process dies, the board still knows
  exactly which tickets are done, in flight, or blocked. No re-deriving from a
  file.
- **Queryable status** — "what's blocked?", "what's in flight?", "show me the
  failing one" are SQL, not text parsing. This also feeds a live UI for free.

What this is **not**: a moat. A readable, editable, live board is good UX and a
great demo — but it's table stakes, not defensibility. The honest value is
developer ergonomics + a recordable demo, not a competitive advantage.

---

## 2. Scope

### In scope (v1, "mini")

- **One design → many tickets, sequential, single worker.** No parallelism.
- **Board CLI** as the seam that lets a skill (a prompt) touch the database.
- **Atomic claim** on `next` even with one worker (it's what makes the board not
  just a markdown file).
- **Deterministic verification as the "review"** — run the ticket's
  `acceptance_criteria` commands, trust exit codes. No LLM reviewer.
- **`needs_human` as a persisted question** — agent stops, writes a
  self-contained question into `human_context`, survives a crash.
- **Human reads a design** via a dedicated raw-markdown CLI command.
- **Two forked skills + one new skill** (see §6).

### Out of scope (v2+)

- Worker pool / parallel agents
- Git worktree isolation, branches, PR creation
- LLM code-review agent (two-stage spec/quality review)
- Dependency graph (`blocked_by`) — sequential order *is* the dependency in v1
- General crash recovery (resetting stranded `implementing` tickets)
- Persisting the human's *answer* — the answer lives only in chat context
- Auth, multi-board, real-time websockets in the UI
- The `plan-document-reviewer-prompt` (the human reviews tickets via the UI)

---

## 3. Architecture

Four components, one source of truth:

- **SQLite board** — the source of truth. Two tables: `design`, `ticket`.
- **`board` CLI** — the only thing that touches the DB. Emits JSON to stdout so a
  skill can shell out and parse results. Design viewing is the one exception
  (raw markdown).
- **Skills (prompts)** — `brainstorming`, `writing-plans` (both forked from
  Superpowers), `execute-ticket` (new). Skills can't touch a DB; they call the
  CLI.
- **UI (separate phase)** — a read-mostly projection of the board. Reflects
  state, doesn't drive it.

Flow of control: a skill running inside Claude Code / Codex calls `board <cmd>`;
the CLI reads/writes SQLite; the UI polls the same SQLite file to visualize.

---

## 4. Data model

### `design`

| column      | type    | notes                                  |
| ----------- | ------- | -------------------------------------- |
| id          | INTEGER | PK                                     |
| title       | TEXT    |                                        |
| design_md   | TEXT    | the approved brainstorming spec, blob  |
| status      | TEXT    | planning \| active \| done             |
| created_at  | TEXT    | default now                            |

The design is stored as a **blob** — it's narrative, read whole by humans, never
machine-acted-on. One design = one planning run (re-running brainstorm creates a
new design, never appends).

### `ticket`

| column               | type    | notes                                          |
| -------------------- | ------- | ---------------------------------------------- |
| id                   | INTEGER | PK                                             |
| design_id            | INTEGER | FK → design.id                                 |
| title                | TEXT    |                                                |
| spec                 | TEXT    | blob: files + code steps (human/agent readable)|
| acceptance_criteria  | TEXT    | **structured** JSON array of checkable commands|
| status               | TEXT    | see §5                                         |
| attempts             | INTEGER | retry counter, cap 3                           |
| human_context        | TEXT    | nullable; the persisted question on needs_human|

**Why `spec` is a blob but `acceptance_criteria` is structured:** the executor
mechanically *iterates* the criteria to verify, so it must be a real list. The
spec is just read whole. `status` and `attempts` are also load-bearing for the
loop. Blobbing the criteria would rebuild `plan.md` with extra steps — the
structured criteria list is the entire value-add over markdown.

---

## 5. State machine

```
queued ──► implementing ──► verifying ──► done
              │                  │
              └──────────────────┴──► needs_human
```

- **queued** — waiting. `next` claims the oldest.
- **implementing** — sub-agent is writing code per `spec`.
- **verifying** — running `acceptance_criteria`. This *is* the review (deterministic).
- **done** — criteria passed.
- **needs_human** — blocked: 3 failed attempts, or genuine ambiguity. Carries the
  question in `human_context`.

There is **no separate LLM review state** in v1. When the v2 LLM reviewer arrives,
it slots *inside* `verifying` (criteria pass → spec-compliance check → quality
check → done) without renaming anything.

---

## 6. Skills

All three are forked/created in a repo we own. **Check the Superpowers license
before redistributing** the forked skills; attribute at minimum.

### 6.1 `brainstorming` (forked — minimal change)

Keep intact: one-question-at-a-time refinement, propose 2–3 alternatives, YAGNI,
write spec to `docs/superpowers/specs/`, and the mandatory user-approval loop
before any implementation.

Change exactly one step — the implementation handoff. After the spec is written,
committed, **and user-approved**:

- Call `board create-design --title <title> --file <spec path>`, capture
  `design_id`.
- Pass `design_id` to `writing-plans`.

### 6.2 `writing-plans` (forked — real change)

Keep intact (this is the quality engine): bite-sized tasks, exact file paths,
complete code per step, the **No Placeholders** rules, TDD red/green structure,
and the **Self-Review** checklist (spec coverage / placeholder scan / type
consistency).

Change the **output target**. Today it writes one `plan.md`. Instead, for each
task, call `board add-ticket --design <id>` once, mapping:

- `--title` ← task component name
- `--spec` ← the `Files:` block + implementation steps/code
- `--criteria` ← verification steps as a **JSON array of exact commands +
  expected result**, e.g. `["pytest tests/auth.py::test_x -v => PASS", "tsc --noEmit => clean"]`

Delete the "Plan Document Header" and "Execution Handoff" sections (no `plan.md`,
no execution-mode choice — one path). Run Self-Review *before* emitting tickets.

**The single most important rule in the project:** `acceptance_criteria` must be
machine-checkable commands, never prose. The skill must explicitly reject
"implement correctly" / "handle edge cases" as criteria — the same discipline as
their No Placeholders rule, applied to the criteria field. The whole system's
teeth depend on this line.

### 6.3 `execute-ticket` (new)

A prose skill instructing the orchestrating agent (Claude Code / Codex) to run
the loop. It has no runtime of its own — it relies on the harness's native
sub-agent dispatch. Loop:

1. **Startup check first:** `board needs-human --design <id>`. If a ticket is
   stranded, surface its `human_context` conversationally ("I was blocked on
   ticket N: …"), wait for the human's answer, then set the ticket back to
   `queued`. Do this **before** `next`.
2. `board next --design <id>` → claims a ticket (or null → all done).
3. **Dispatch a fresh sub-agent** to implement the ticket per `spec`.
4. Run the ticket's `acceptance_criteria`; read exit codes.
5. Pass → `board update <id> --status done`. Fail → `board update <id> --status
   implementing --bump-attempts`, retry. On the **3rd** failure **or** genuine
   ambiguity → write a self-contained question to `human_context`, set
   `--status needs_human`, **stop**.
6. Loop to 2.

Hard rules in the prompt: **never guess on ambiguity — stop and ask. Never weaken
a criterion to make it pass. Cap attempts at 3.**

---

## 7. `board` CLI surface

All commands emit JSON to stdout. Errors print `{"ok":false,"error":...}` to
stderr and exit non-zero. DB path from `$BOARD_DB`, default `./board.db`.

```
board init
board create-design --title T (--file path | --stdin)   -> {id, title}
board add-ticket --design <id> --title T --spec "..." \
      --criteria '["...", "..."]'                        -> {id}
board next [--design <id>]      -> claims oldest queued (->implementing), returns ticket | {ticket:null}
board show <ticket_id>          -> full ticket incl. parent design_md (JSON, for agents)
board update <ticket_id> --status S [--context "..."] [--bump-attempts]
board list --design <id>        -> [tickets...] (JSON)
board needs-human [--design <id>] -> stranded needs_human ticket | {ticket:null}
board design <design_id>        -> raw markdown of design_md (NOT JSON; for humans, pipe to glow/less)
```

Two surfaces for the design blob: `show <ticket>` wraps it in JSON for the agent's
execution context; `board design <id>` prints raw markdown for a human to read.

---

## 8. needs_human & crash resume

`needs_human` exists for one narrow case: the agent hit ambiguity, asked, the
human didn't answer, and then the process/PC died. Without persistence the
question is lost.

- The agent writes a **self-contained** question into `human_context` (assume the
  reader has zero memory of the session: name the ticket, the ambiguity/failure,
  what's needed, what was tried).
- On the next run, `execute-ticket` checks `needs-human` **first** and the agent
  **drives the resume** — it opens the conversation ("I crashed last session; I
  was blocked on ticket N: …") and waits for the answer.
- The human's answer goes into **chat context only** — never persisted. Once
  answered, the ticket flips back to `queued` and the loop continues.

No general crash recovery beyond this. If the process dies mid-`implementing`
without a pending question, re-running simply continues from board state.

---

## 9. UI (separate phase — build after the headless loop works)

A read-mostly projection of the board that updates as states change. The board is
already the source of truth; the UI reflects it.

Must-haves (these are what make the demo land):

- Columns by `status`; tickets as cards moving across as states change.
- Card → click → `spec`, `acceptance_criteria`, `human_context`.
- A visible "currently implementing" highlight (the live progress).
- `needs_human` cards visually flagged — where the human intervenes.
- Editable: change a ticket's `spec`/`criteria`, requeue, mark done — all just
  `board update` behind a button.

Architecture, deliberately minimal:

- Thin read/write HTTP layer over the same SQLite file (or read the DB directly).
- "Live" = poll the DB every 1–2s. **No websockets in v1** — polling looks
  identical on screen and is trivial.
- Stack option: small TS read API + React board, or a single static page that
  polls. For a demo, static-page-polling is the least code.

Sequencing: the headless loop must move tickets through states on its own first.
The UI animates that; it doesn't drive it. Building the UI first means animating a
board nothing updates.

---

## 10. Build / distribution

Target: a single static binary CLI; SQLite compiled in (no system `libsqlite3`).

The wrinkle — "fully static" only cleanly means **Linux + musl**:

- **Linux:** `x86_64-unknown-linux-musl` + `musl-tools` (bundled SQLite compiles
  C, so it needs `musl-gcc`). Truly static.
- **macOS:** not possible — Apple ships no static system libraries. The default
  build (SQLite still bundled) is the ceiling.
- **Windows:** `+crt-static` on the MSVC target — a different meaning of "static".

**Open decision:** confirm the target platform so the build config is set up
correctly (musl `.cargo/config.toml` vs default).

---

## 11. Risks & honest notes

- **The whole project lives or dies on `writing-plans` emitting checkable
  criteria.** Superpowers already does this well for TDD-shaped tasks. The risk is
  non-test-shaped tasks degrading to "it works". Keeping their TDD insistence is
  what inherits their teeth. This is prompt engineering, not schema work — budget
  time accordingly.
- **The board UI is not a moat.** It's good UX and a great demo. Treat it as
  marketing/ergonomics, not defensibility.
- **Scope creep is the main threat.** The mini core (skills + CLI + execute-ticket)
  must ship and run headless before the UI is built. Resist parallelism, PRs, and
  LLM review until v1 demonstrably works end-to-end.
- **Planning everything upfront does not prevent execution failure.** LLM
  non-determinism and unforeseen real dependencies mean "stop and ask" is a normal
  path, not an edge case. The verification + `needs_human` machinery exists because
  failure is expected.

---

## 12. Roadmap

- **v1 (mini):** forked `brainstorming` + `writing-plans`, new `execute-ticket`,
  `board` CLI, SQLite board, sequential single-worker loop, deterministic
  verification, `needs_human` resume. Headless.
- **v1.5:** the live board UI (polling) — the recordable demo.
- **v2:** sub-agent worker pool + parallelism, git worktree isolation + PR
  creation, LLM two-stage code review inside `verifying`, dependency graph
  (`blocked_by`).

---

## 13. Open decisions

1. **Target platform for the static binary** (Linux/musl vs macOS-local) — sets
   build config.
2. **Project name** — "AI Board" is the working name; confirm or change before
   the repo goes public.
3. **UI stack** — TS API + React vs single static polling page. Defer until the
   headless loop works.
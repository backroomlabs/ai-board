---
name: using-ai-board
description: Use at the start of any session involving the ai-board workflow — establishes the three skills, the abd CLI, and the Spec → Ticket → Task hierarchy.
---

# Using AI Board

AI Board replaces a markdown `plan.md` with a **SQLite board** as the single
source of truth for `brainstorm → plan → (future) execute`. Skills drive the
workflow; the `abd` CLI is the only thing that touches the database.

## Hierarchy

- **Spec** — overarching PRD for one project.
- **Ticket** — smaller PRD and Kanban card: `description` + prose
  `definitions_of_done` (`--dod`).
- **Task** — atomic deliverable under a ticket: `work_type`, `objective`,
  machine-checkable `acceptance_criteria`, optional `context`.

The planner stops at Task. How a Task is carried out is owned by a future Run
layer (not this release). `abd next` remains in the CLI but is unused by skills.

One spec owns one logical board of tickets (each with nested tasks). The SQLite
database may contain multiple specs.

**If `abd` is not on `$PATH`** (`command -v abd` fails), stop and tell the user
to install it first:

```bash
curl -fsSL https://raw.githubusercontent.com/backroomlabs/ai-board/main/install.sh | sh
```

## The Three Skills

This bootstrap skill establishes the workflow; three additional skills perform
the work:

| Skill | When to use |
|---|---|
| `board-brainstorming` | Starting a new feature — explores intent, writes and persists the spec, starts `abd serve`, hands off to `board-planning` |
| `board-planning` | After brainstorming — decomposes spec into tickets (`--dod`) and tasks (`abd task add`), shows board for approval, hands off to `board-execute` |
| `board-execute` | After plan approved — does **not** claim work; tells the user Runs are the next layer |

Always invoke in order. Never skip `board-brainstorming` to go straight to
`board-planning`. Never invoke `board-execute` before the user approves the plan.

## The `abd` CLI

Commands emit JSON to stdout except `abd spec get`, which emits raw spec
content. Errors → `{"ok":false,"error":...}` on stderr, exit non-zero. DB path
from `$BOARD_DB`, default `./board.db`.

```
abd init
abd spec add --title T (--file PATH | --stdin)  → {id, title}
abd spec list                                → all specs, newest first
abd spec get SPEC_ID                         → raw spec content
abd ticket add --spec-id ID --title T --description "..." --dod '[...]'  → {id}
abd ticket show TICKET_ID                    → ticket JSON (nested tasks)
abd ticket list --spec-id ID                 → all tickets for a spec
abd task add --ticket-id ID --title T --work-type TYPE --objective "..." --criteria '[...]' [--context "..."]  → {id}
abd task list --ticket-id ID                 → tasks for a ticket
abd task show TASK_ID                        → task JSON
abd next [--spec-id ID]                      → leftover status command (unused by skills)
abd update TICKET_ID --status S [--context "..."] [--bump-attempts]  → leftover status command
abd needs-human [--spec-id ID]               → leftover status command
abd serve [--port 4141]                      → live editable UI at http://localhost:4141
```

`abd serve` is **idempotent** — safe to call even if already running.

## Planner handoff

After `board-planning`, the durable handoff is the set of **Tasks** on the board
(`abd task list --ticket-id` / `abd ticket show`). Review them in the UI before
approving. Do not drive execution with `abd next` or `needs-human` — those are
leftover CLI, not this workflow. Runs will own execution later.

## Board State Machine

Ticket statuses still exist for the leftover CLI / UI columns:

```
queued → implementing → verifying → done
              │               │
              └───────────────┴→ needs_human
```

Skills in this release do not advance tickets through that machine. Task
execution belongs to the future Run layer.

## Live UI

```bash
abd serve --port 4141   # http://localhost:4141
```

Columns by ticket status. Click a card for description, definitions of done,
nested tasks, and human context. Specs, tickets, and tasks can be edited in the
UI.

---
name: using-ai-board
description: Use at the start of any session involving the ai-board workflow — establishes the three skills, the board CLI, and how to recover from crashes or stranded tickets.
---

# Using AI Board

AI Board replaces a markdown `plan.md` with a **SQLite board** as the single
source of truth for `brainstorm → plan → execute`. Skills drive the workflow;
the `board` CLI is the only thing that touches the database.

## The Three Skills

| Skill | When to use |
|---|---|
| `board-brainstorming` | Starting a new feature — explores intent, writes spec, persists design to board, starts `board serve`, hands off to `board-planning` |
| `board-planning` | After brainstorming — decomposes spec into board tickets with machine-checkable criteria, shows board for approval, hands off to `board-execute` |
| `board-execute` | After plan approved — runs the sequential loop: claim → implement → verify → done |

Always invoke in order. Never skip `board-brainstorming` to go straight to
`board-planning`. Never invoke `board-execute` before the user approves the plan.

## The `board` CLI

All commands emit JSON to stdout. Errors → `{"ok":false,"error":...}` on stderr,
exit non-zero. DB path from `$BOARD_DB`, default `./board.db`.

```
board init
board create-design --title T (--file PATH | --stdin)  → {id, title}
board add-ticket --design ID --title T --spec "..." --criteria '[...]'  → {id}
board next [--design ID]      → claims oldest queued ticket (→ implementing)
board show TICKET_ID          → full ticket + parent design_md
board list --design ID        → all tickets for a design (full JSON)
board update TICKET_ID --status S [--context "..."] [--bump-attempts]
board needs-human [--design ID] → stranded needs_human ticket or {ticket:null}
board design DESIGN_ID        → raw markdown of the design spec
board serve [--port 4141]     → live read-only UI at http://localhost:4141
```

`board serve` is **idempotent** — safe to call even if already running.

## Crash Recovery

### Stranded `needs_human` ticket

Handled automatically by `board-execute` step 0 on every startup:

```bash
board needs-human --design <id>
```

If a ticket is returned, surface its `human_context` to the user, wait for the
answer, then requeue:

```bash
board update <ticket_id> --status queued
```

### Stranded `implementing` ticket

If a session crashed mid-implementation, the ticket stays in `implementing`.
`board next` skips it (only claims `queued`). Detect and reset on startup:

```bash
board list --design <id>
```

Find any tickets with `"status": "implementing"`. These are stranded — the
sub-agent did not finish. Reset each to `queued` before calling `board next`:

```bash
board update <ticket_id> --status queued
```

Then proceed with the normal loop. The ticket will be re-claimed and
re-implemented from scratch.

**Both recovery checks belong in step 0 of `board-execute`** — run them every
time before claiming new work.

## Board State Machine

```
queued → implementing → verifying → done
              │               │
              └───────────────┴→ needs_human
```

- `board next` claims `queued` → `implementing` (atomic, safe for concurrent workers)
- `board-execute` must set `verifying` before running any criteria — never skip it
- `needs_human` survives crashes; `implementing` without a question does not

## Live UI

```bash
board serve --port 4141   # http://localhost:4141
```

Five columns by status. Cards move as tickets progress. `implementing` is
accent-highlighted; `needs_human` is flagged red. Click a card for full spec,
criteria, and human context. Polls every 500ms — `verifying` state is visible.

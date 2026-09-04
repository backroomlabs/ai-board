# AI Board

A live planning board for agentic development. Your idea becomes a spec, a
capable planning agent turns that spec into tickets and tasks, and you review
the board before a future Run layer executes the work.

![AI Board live board](resources/board_readme.jpg)

---

## Vision — AI spec-driven development

AI Board is built around a simple idea: use a capable agent for the hard
planning work — clarifying intent, writing the design, and breaking it into
bounded work — then (later) let deterministic Runs carry out each atomic task.

Hierarchy:

- **Spec** — overarching PRD.
- **Ticket** — smaller PRD and Kanban card (`description` + prose
  `definitions_of_done`).
- **Task** — atomic deliverable (`work_type`, `objective`, machine-checkable
  `acceptance_criteria`, optional `context`).

The planner stops at Task. Execution policy (Runs) is not this release; `abd
next` remains in the CLI unused by skills.

One spec owns one logical board of tickets (with nested tasks). A single SQLite
database may contain multiple specs.

---

## How it works

Three AI skills drive the workflow:

**1. `board-brainstorming`** — Explores your idea, asks clarifying questions,
writes a spec, and saves it to the board.

**2. `board-planning`** — Reads the spec and breaks it into tickets and tasks.
Each ticket has a `description` and prose `definitions_of_done`. Each task has a
`work_type`, `objective`, and machine-checkable `acceptance_criteria` (real
shell commands — never prose). Opens the live board so you can review every
ticket and task before execution starts.

**3. `board-execute`** — Does not claim tickets or run criteria. After you
approve the plan, it tells you that deterministic Runs (selected by work type)
are the next layer. `abd next` is leftover CLI, not this workflow.

`using-ai-board` is the bootstrap skill that establishes this three-skill
workflow and the Spec → Ticket → Task hierarchy.

You can watch the whole thing on the live board at `http://localhost:4141`.

---

## Install

### Claude Code

```text
/plugin marketplace add backroomlabs/ai-board
/plugin install ai-board@ai-board
```

Then install the `abd` binary:

```bash
curl -fsSL https://raw.githubusercontent.com/backroomlabs/ai-board/main/install.sh | sh
```

### Codex, Cursor, or anything reading `~/.agents/skills`

```bash
curl -fsSL https://raw.githubusercontent.com/backroomlabs/ai-board/main/install.sh | sh
```

Installs `abd` to `~/.local/bin` and the four skills to `~/.agents/skills`.

### From source

```bash
cargo install ai-board   # binary is named `abd`
# or
cargo build --release    # binary at target/release/abd
```

---

## Usage

Start a new project:

```
/board-brainstorming
```

The agent guides you through brainstorming and planning. Open
`http://localhost:4141` when prompted to review tickets and tasks.

---

## Live board

```bash
abd serve   # http://localhost:4141
```

Ticket columns: `queued → implementing → verifying → done` + `needs_human`.
Click any card to see its description, definitions of done, nested tasks, and
(if blocked) human context. Specs, tickets, and tasks are editable directly from
the board.

---

## `abd` CLI reference

The `abd` binary is the only thing that touches the database. Commands emit JSON
to stdout except `abd spec get`, which emits raw spec content. Errors go to
stderr as `{"ok":false,"error":"..."}` with a non-zero exit code.

| Command | What it does |
|---|---|
| `abd init` | Create the schema (idempotent) |
| `abd spec add --title T (--file PATH \| --stdin)` | Save a spec |
| `abd spec list` | List all specs (JSON array, newest first) |
| `abd spec get SPEC_ID` | Print the raw spec content |
| `abd ticket add --spec-id ID --title T --description "..." --dod '[...]'` | Add a ticket (prose definitions of done) |
| `abd ticket show TICKET_ID` | Show ticket JSON (nested tasks) |
| `abd ticket list --spec-id ID` | List all tickets for a spec |
| `abd task add --ticket-id ID --title T --work-type TYPE --objective "..." --criteria '[...]' [--context "..."]` | Add a task under a ticket |
| `abd task list --ticket-id ID` | List tasks for a ticket |
| `abd task show TASK_ID` | Show task JSON |
| `abd next [--spec-id ID]` | Leftover: claim oldest queued ticket |
| `abd update TICKET_ID --status S [--context "..."] [--bump-attempts]` | Leftover: update ticket status |
| `abd needs-human [--spec-id ID]` | Leftover: get blocked ticket, if any |
| `abd serve [--port 4141]` | Start the live board UI (idempotent) |

`$BOARD_DB` sets the database path (default `./board.db`).

---

## Demo

```bash
demo/demo.sh                 # build, serve UI, emit tickets + tasks
DEMO_SLEEP=0.4 demo/demo.sh  # faster
NO_SERVE=1 demo/demo.sh      # headless
```

Persists a canned spec, tickets (`--dod`), and tasks (`--criteria`). Does not
claim tickets or run criteria.

---

## License

MIT — see [LICENSE](LICENSE). `board-brainstorming` and `board-planning` derive
from [Superpowers](https://github.com/obra/superpowers) (MIT, Jesse Vincent);
see [LICENSE-NOTICE.md](LICENSE-NOTICE.md) and [LICENSE.superpowers](LICENSE.superpowers).

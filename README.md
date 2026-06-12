# AI Board

A live planning board for agentic development. Your idea becomes a spec, the spec becomes
tickets, an AI agent works through them — you watch and approve.

![AI Board live board](resources/board_readme.jpg)

---

## How it works

Three AI skills drive the workflow:

**1. `board-brainstorming`** — Explores your idea, asks clarifying questions,
writes a spec, saves it to the board as a design.

**2. `board-planning`** — Reads the design, breaks it into tickets. Each ticket
has a `spec` (what to build) and `acceptance_criteria` (real shell commands that
must pass — never prose). Opens the live board so you can review every ticket
before execution starts.

**3. `board-execute`** — Works through tickets in order. For each ticket it
dispatches a fresh sub-agent (clean context, no baggage from prior tickets),
runs the acceptance criteria for real, and marks the ticket done. If something
fails three times or hits genuine ambiguity, it stops and asks you.

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

The agent guides you through the whole flow. Open `http://localhost:4141` when
prompted to review the plan, then say yes to start execution.

---

## Live board

```bash
abd serve   # http://localhost:4141
```

Five columns: `queued → implementing → verifying → done` + `needs_human`.
Cards move across as the agent works. Click any card to see its spec, acceptance
criteria, and (if blocked) the question the agent is asking. The `needs_human`
column is where you intervene.

The board is **read-only** in v1 — editing tickets via UI is coming in v2.

---

## `abd` CLI reference

The `abd` binary is the only thing that touches the database. All commands emit
JSON to stdout; errors go to stderr as `{"ok":false,"error":"..."}` with a
non-zero exit code.

| Command | What it does |
|---|---|
| `abd init` | Create the schema (idempotent) |
| `abd create-design --title T (--file PATH \| --stdin)` | Save a design spec |
| `abd add-ticket --design ID --title T --spec "..." --criteria '[...]'` | Add a ticket |
| `abd next [--design ID]` | Atomically claim the oldest queued ticket |
| `abd show TICKET_ID` | Full ticket including the parent design spec |
| `abd list --design ID` | All tickets for a design |
| `abd update TICKET_ID --status S [--context "..."] [--bump-attempts]` | Update a ticket |
| `abd needs-human [--design ID]` | Get the blocked ticket, if any |
| `abd design DESIGN_ID` | Print the raw design spec (pipe to `glow`/`less`) |
| `abd serve [--port 4141]` | Start the live board UI (idempotent) |

`$BOARD_DB` sets the database path (default `./board.db`).

---

## Demo

```bash
demo/demo.sh                 # build, serve UI, run the full loop
DEMO_SLEEP=0.4 demo/demo.sh  # faster
NO_SERVE=1 demo/demo.sh      # headless
```

Runs the full state machine — including a ticket that fails three times, escalates
to `needs_human`, and resumes after a simulated crash — against a live board you
can watch.

---

## License

MIT — see [LICENSE](LICENSE). `board-brainstorming` and `board-planning` derive
from [Superpowers](https://github.com/obra/superpowers) (MIT, Jesse Vincent);
see [LICENSE-NOTICE.md](LICENSE-NOTICE.md) and [LICENSE.superpowers](LICENSE.superpowers).

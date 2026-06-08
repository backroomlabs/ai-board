# `board` — AI Board CLI

SQLite-backed orchestration board for agentic development: one design → many
tickets, walked through `queued → implementing → verifying → done` with a
`needs_human` escape hatch. See [`ai-board.md`](ai-board.md) for the full design.

This repo is the **backend + CLI** (Rust, single static binary, SQLite bundled).
Skills and UI are out of scope here.

## Build

```bash
cargo build --release   # binary at target/release/board
cargo test              # unit + end-to-end tests
```

SQLite is compiled in (`rusqlite` `bundled` feature) — no system `libsqlite3`.
For a truly static Linux binary: `cargo build --release --target x86_64-unknown-linux-musl`
(needs `musl-tools` for `musl-gcc`, since bundled SQLite compiles C).

## Database

DB path comes from `$BOARD_DB`, default `./board.db`.

```bash
export BOARD_DB=/path/to/board.db
board init      # create schema (idempotent)
```

## Commands

All commands emit **JSON to stdout**. On error they print
`{"ok":false,"error":...}` to **stderr** and exit non-zero. The one exception is
`board design`, which prints **raw markdown** for humans.

| Command | Output |
| --- | --- |
| `board init` | `{ok, db}` — creates the schema |
| `board create-design --title T (--file PATH \| --stdin)` | `{id, title}` |
| `board add-ticket --design ID --title T --spec "..." --criteria '[...]'` | `{id}` |
| `board next [--design ID]` | claimed ticket (→`implementing`) or `{ticket:null}` |
| `board show TICKET_ID` | full ticket incl. parent `design_md` |
| `board list --design ID` | `[tickets...]` |
| `board update TICKET_ID --status S [--context "..."] [--bump-attempts]` | updated ticket |
| `board needs-human [--design ID]` | stranded `needs_human` ticket or `{ticket:null}` |
| `board design DESIGN_ID` | raw markdown (pipe to `glow`/`less`) |
| `board serve [--port 4141]` | serve the read-only live UI (see below) |

`--criteria` **must** be a JSON array of checkable command strings, e.g.
`'["cargo test => PASS", "tsc --noEmit => clean"]'`. Non-array input is rejected —
this is the system's teeth: acceptance criteria are machine-checkable, never prose.

Valid `--status` values: `queued`, `implementing`, `verifying`, `done`,
`needs_human`. `--bump-attempts` increments the retry counter (cap-at-3 is
enforced by the executor, not the CLI).

## Example

```bash
export BOARD_DB=/tmp/board.db
board init
board create-design --title "Auth" --file specs/auth.md      # -> {"id":1,...}
board add-ticket --design 1 --title "Login endpoint" \
  --spec "Add POST /login ..." \
  --criteria '["cargo test login => PASS"]'                   # -> {"id":1}
board next --design 1                                         # claims ticket 1
board update 1 --status done                                  # mark verified
board design 1 | less                                         # read the design
```

## Live UI

```bash
board serve --port 4141   # http://localhost:4141
```

Read-only board over the same `$BOARD_DB`. Five columns by status
(`queued → implementing → verifying → done`, plus `needs_human`); cards move as
the headless loop updates the board. The page polls every ~1.5s (no websockets).
Click a card for its `spec`, `acceptance_criteria`, and `human_context`. The
`implementing` card is accent-highlighted; `needs_human` cards are flagged red.

Endpoints (GET only): `/api/designs`, `/api/board?design=N`, `/` (the page).
Editing from the UI (requeue, mark done, edit spec/criteria) is **v2** — v1.5 is
read-only.

## Demo

```bash
demo/demo.sh                 # build, serve UI, run the loop (open http://localhost:4141)
DEMO_SLEEP=0.4 demo/demo.sh  # faster pacing
NO_SERVE=1 demo/demo.sh      # headless, no UI
```

Pure simulation: the board state machine, atomic claim, criteria-as-shell verify
(real exit codes), attempts-cap → `needs_human`, and crash-style resume are all
genuine; only the spec ([demo/spec.md](demo/spec.md)) and the per-ticket "code"
are canned (those are the LLM steps). Three tickets: two clean passes, one rigged
to fail 3× → `needs_human` → resolved on resume. Run with the UI open to watch
cards move through the columns live.

## Skills

`skills/` holds the three orchestration skills (Superpowers SKILL.md format):
`brainstorming` and `writing-plans` (forked) and `execute-ticket` (new). They
drive the `brainstorm → plan → execute` workflow through this CLI. See
[`LICENSE-NOTICE.md`](LICENSE-NOTICE.md) for attribution. Run
`scripts/check-skills.sh` to verify their structure.

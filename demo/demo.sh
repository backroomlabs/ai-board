#!/usr/bin/env bash
#
# Demo of the AI Board planner handoff.
#
# What is REAL here: persisting a spec, tickets with prose --dod, and tasks with
# machine-checkable --criteria; nesting tasks under tickets; the live UI.
#
# What is STUBBED: the LLM steps — writing the spec (brainstorming) and
# decomposing into tickets/tasks (planning). The spec is canned (demo/spec.md)
# and the tickets/tasks are pre-baked. This demo does NOT claim tickets or run
# criteria (Runs are a later layer; do not call abd next).
#
# Usage:
#   demo/demo.sh                 # build if needed, serve UI, emit tickets+tasks
#   DEMO_SLEEP=0.4 demo/demo.sh  # faster
#   PORT=9000 demo/demo.sh       # different UI port
#   NO_SERVE=1 demo/demo.sh      # headless (no UI server)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-8080}"
SLEEP="${DEMO_SLEEP:-1.2}"
STATE="/tmp/ai-board-demo"
WORK="$STATE/work"
export BOARD_DB="$STATE/board.db"
BIN="$ROOT/target/release/abd"

# ---- helpers --------------------------------------------------------------

pause() { sleep "$SLEEP"; }

# Read a top-level scalar field from JSON on stdin.
jget() { python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get(sys.argv[1],""))' "$1"; }

banner() { printf "\n\033[1;36m== %s ==\033[0m\n" "$*"; }

# ---- main -----------------------------------------------------------------

[ -x "$BIN" ] || { echo "building release binary..."; (cd "$ROOT" && cargo build --release); }

rm -rf "$STATE"; mkdir -p "$WORK"
"$BIN" init >/dev/null

SRV_PID=""
if [ "${NO_SERVE:-0}" != "1" ]; then
  "$BIN" serve --port "$PORT" >/dev/null 2>&1 &
  SRV_PID=$!
  trap '[ -n "$SRV_PID" ] && kill "$SRV_PID" 2>/dev/null' EXIT
  banner "UI live at http://localhost:$PORT  — open it now"
  sleep 2
fi

banner "create spec from demo/spec.md"
SID="$("$BIN" spec add --title "Greeter" --file "$ROOT/demo/spec.md" | jget id)"
printf "  spec_id=%s\n" "$SID"
pause

banner "board-planning stub — emit tickets (--dod) and tasks (--criteria)"

TID="$("$BIN" ticket add --spec-id "$SID" --title "Create greeter" \
  --description "Player-facing greeter script exists and prints hello." \
  --dod "[\"greet.sh exists\", \"running it without args prints hello\"]" | jget id)"
"$BIN" task add --ticket-id "$TID" --title "Add greet.sh" \
  --work-type code_implementation \
  --objective "Create greet.sh that prints exactly hello with no arguments." \
  --criteria "[\"test -f $WORK/greet.sh => PASS\", \"bash $WORK/greet.sh | grep -q '^hello$' => PASS\"]" >/dev/null

TID="$("$BIN" ticket add --spec-id "$SID" --title "Greet by name" \
  --description "Greeter accepts a name argument and prints hello <name>." \
  --dod "[\"greet.sh accepts a name\", \"default name is world\"]" | jget id)"
"$BIN" task add --ticket-id "$TID" --title "Accept name argument" \
  --work-type code_implementation \
  --objective "Update greet.sh so its first argument is a name, defaulting to world, printing exactly hello followed by that name." \
  --criteria "[\"bash $WORK/greet.sh World | grep -q 'hello World' => PASS\"]" >/dev/null

TID="$("$BIN" ticket add --spec-id "$SID" --title "Uppercase greeting" \
  --description "Greeter prints an uppercase greeting for the supplied name." \
  --dod "[\"greeting and name are uppercase\"]" | jget id)"
"$BIN" task add --ticket-id "$TID" --title "Uppercase output" \
  --work-type code_implementation \
  --objective "Update greet.sh so the greeting and supplied name are uppercase, producing exactly HELLO WORLD for argument world." \
  --criteria "[\"bash $WORK/greet.sh world | grep -q 'HELLO WORLD' => PASS\"]" >/dev/null

printf "  3 tickets with tasks queued\n"
pause

banner "final board (tickets + nested tasks)"
"$BIN" ticket list --spec-id "$SID" | python3 -c '
import sys, json
for t in json.load(sys.stdin):
    print("  #%s %-20s %-12s dod=%s" % (
        t["id"], t["title"], t["status"], t.get("definitions_of_done")))
'
echo
while IFS= read -r tid; do
  [ -z "$tid" ] && continue
  "$BIN" ticket show "$tid" | python3 -c '
import sys, json
t = json.load(sys.stdin)
print("ticket #%s %s" % (t["id"], t["title"]))
for task in t.get("tasks") or []:
    print("  task #%s [%s] %s" % (task["id"], task["work_type"], task["title"]))
    print("    objective: %s" % task.get("objective"))
    print("    criteria: %s" % task.get("acceptance_criteria"))
'
done < <("$BIN" ticket list --spec-id "$SID" | python3 -c '
import sys, json
for t in json.load(sys.stdin):
    print(t["id"])
')

if [ -n "$SRV_PID" ]; then
  banner "done — UI still live at http://localhost:$PORT (Ctrl-C to stop)"
  wait "$SRV_PID"
fi

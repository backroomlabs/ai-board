#!/usr/bin/env bash
#
# Pure-simulation demo of the AI Board.
#
# What is REAL here: the board state machine, atomic `next` claim, running each
# ticket's acceptance_criteria as shell commands and trusting exit codes, the
# attempts-cap -> needs_human escalation, and crash-style resume. The live UI
# animates all of it.
#
# What is STUBBED: the two steps that need an LLM in a real run — writing the
# spec (brainstorming) and writing novel code (the sub-agent implementation).
# The spec is canned (demo/spec.md) and each ticket's "implementation" is a
# pre-baked artifact this script writes. This is a faithful simulation of the
# agent, not the agent.
#
# Usage:
#   demo/demo.sh                 # build if needed, serve UI, run the loop
#   DEMO_SLEEP=0.4 demo/demo.sh  # faster
#   PORT=9000 demo/demo.sh       # different UI port
#   NO_SERVE=1 demo/demo.sh      # headless (no UI server)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-8080}"
SLEEP="${DEMO_SLEEP:-1.2}"
STATE="/tmp/ai-board-demo"
WORK="$STATE/work"
RESOLVED="$STATE/.resolved"          # set when the human "answers" ticket 3
export BOARD_DB="$STATE/board.db"
BIN="$ROOT/target/release/abd"

# ---- helpers --------------------------------------------------------------

pause() { sleep "$SLEEP"; }

# Read a top-level scalar field from JSON on stdin.
jget() { python3 -c 'import sys,json;d=json.load(sys.stdin);print(d.get(sys.argv[1],""))' "$1"; }

# Print each acceptance-criteria command (the part before " => ") on its own line.
criteria_cmds() {
  printf '%s' "$1" | python3 -c '
import sys, json
d = json.load(sys.stdin)
for c in d.get("acceptance_criteria", []):
    print(str(c).split(" => ", 1)[0])
'
}

banner() { printf "\n\033[1;36m== %s ==\033[0m\n" "$*"; }

# Write the pre-baked artifact for a ticket title. This stands in for the
# sub-agent that would write code in a real run.
implement() {
  local title="$1"
  mkdir -p "$WORK"
  case "$title" in
    *Create*)
      cat > "$WORK/greet.sh" <<'EOF'
#!/usr/bin/env bash
echo "hello"
EOF
      ;;
    *name*)
      cat > "$WORK/greet.sh" <<'EOF'
#!/usr/bin/env bash
name="${1:-world}"
echo "hello $name"
EOF
      ;;
    *Uppercase*)
      if [ -f "$RESOLVED" ]; then
        cat > "$WORK/greet.sh" <<'EOF'
#!/usr/bin/env bash
name="${1:-world}"
echo "HELLO ${name^^}"
EOF
      else
        # Rigged: keep the lowercase version, so the uppercase criteria fail.
        cat > "$WORK/greet.sh" <<'EOF'
#!/usr/bin/env bash
name="${1:-world}"
echo "hello $name"
EOF
      fi
      ;;
  esac
}

# Run every criterion for a ticket. Returns 0 only if all pass.
verify() {
  local ticket="$1" ok=0 cmd
  while IFS= read -r cmd; do
    [ -z "$cmd" ] && continue
    if bash -c "$cmd"; then
      printf "    \033[32mPASS\033[0m %s\n" "$cmd"
    else
      printf "    \033[31mFAIL\033[0m %s\n" "$cmd"
      ok=1
    fi
  done < <(criteria_cmds "$ticket")
  return $ok
}

# ---- the execute-ticket loop (mirrors skills/execute-ticket) --------------

SID=""

# Step 0: resume any stranded needs_human ticket first.
startup_resume() {
  local nh id ctx
  nh="$("$BIN" needs-human --spec-id "$SID")"
  id="$(printf '%s' "$nh" | jget id)"
  [ -z "$id" ] && return 0
  ctx="$(printf '%s' "$nh" | jget human_context)"
  banner "RESUME — stranded question on ticket $id"
  printf "  human_context: %s\n" "$ctx"
  printf "  (human answers: 'yes, use bash \${name^^} for uppercase')\n"
  pause
  touch "$RESOLVED"                       # the answer, applied
  "$BIN" update "$id" --status queued >/dev/null
  printf "  -> ticket %s requeued\n" "$id"
  pause
}

# Drain queued tickets. Returns 0 when board empty, 2 when a ticket escalated.
drain() {
  local t id title attempts
  while true; do
    t="$("$BIN" next --spec-id "$SID")"
    id="$(printf '%s' "$t" | jget id)"
    if [ -z "$id" ]; then
      banner "board drained — all tickets done"
      return 0
    fi
    title="$(printf '%s' "$t" | jget title)"
    attempts="$(printf '%s' "$t" | jget attempts)"
    banner "claimed ticket $id: $title  (-> implementing)"
    pause

    while true; do
      implement "$title"
      "$BIN" update "$id" --status verifying >/dev/null
      printf "  verifying ticket %s...\n" "$id"
      pause
      if verify "$t"; then
        "$BIN" update "$id" --status done >/dev/null
        printf "  \033[32mticket %s done\033[0m\n" "$id"
        pause
        break
      fi
      "$BIN" update "$id" --status implementing --bump-attempts >/dev/null
      attempts=$((attempts + 1))
      printf "  attempt %s failed\n" "$attempts"
      pause
      if [ "$attempts" -ge 3 ]; then
        "$BIN" update "$id" --status needs_human \
          --context "Ticket $id ($title): uppercase output never matched after 3 attempts. Tried: lowercase greet.sh. Need: confirmation to use bash \${name^^} uppercasing." >/dev/null
        banner "ticket $id -> needs_human (attempts capped at 3)"
        pause
        return 2
      fi
    done
  done
}

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

banner "board-planning stub — emit 3 tickets with real acceptance_criteria"
"$BIN" ticket add --spec-id "$SID" --title "Create greeter" \
  --description "Create $WORK/greet.sh as a Bash script that prints exactly hello when run without arguments." \
  --criteria "[\"test -f $WORK/greet.sh => PASS\", \"bash $WORK/greet.sh | grep -q '^hello$' => PASS\"]" >/dev/null
"$BIN" ticket add --spec-id "$SID" --title "Greet by name" \
  --description "Update the existing $WORK/greet.sh so its first argument is a name, defaulting to world, and it prints exactly hello followed by that name." \
  --criteria "[\"bash $WORK/greet.sh World | grep -q 'hello World' => PASS\"]" >/dev/null
"$BIN" ticket add --spec-id "$SID" --title "Uppercase greeting" \
  --description "Update the existing $WORK/greet.sh so the greeting and supplied name are uppercase, producing exactly HELLO WORLD for argument world." \
  --criteria "[\"bash $WORK/greet.sh world | grep -q 'HELLO WORLD' => PASS\"]" >/dev/null
printf "  3 tickets queued\n"
pause

# Session 1: T1 + T2 pass, T3 escalates to needs_human.
startup_resume
set +e
drain
rc=$?
set -e

# Session 2: simulate next run — startup_resume answers T3, then drain finishes.
if [ "$rc" -eq 2 ]; then
  banner "--- simulating crash + next session ---"
  pause
  startup_resume
  drain
fi

banner "final board"
"$BIN" list --spec-id "$SID" | python3 -c '
import sys, json
for t in json.load(sys.stdin):
    print("  #%s %-20s %-12s attempts=%s" % (t["id"], t["title"], t["status"], t["attempts"]))
'

if [ -n "$SRV_PID" ]; then
  banner "done — UI still live at http://localhost:$PORT (Ctrl-C to stop)"
  wait "$SRV_PID"
fi

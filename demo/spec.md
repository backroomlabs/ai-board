# Greeter — Demo Spec

A tiny shell `greet.sh` built incrementally to exercise the board end-to-end.

This spec is canned for the demo: in a real run the `brainstorming` skill would
produce it and `board-planning` would emit the tickets. Here the demo script
stands in for those LLM steps so the board, the verify loop, the `needs_human`
escalation, and the live UI can be shown deterministically.

## Tickets

1. **Create greeter** — `greet.sh` prints `hello`.
2. **Greet by name** — `greet.sh <name>` prints `hello <name>`.
3. **Uppercase greeting** — `greet.sh <name>` prints `HELLO <NAME>`.
   (Rigged to fail its acceptance criteria three times → `needs_human`, then
   resolved on resume — demonstrates the escape hatch and crash-style resume.)

Every ticket's `acceptance_criteria` are real shell commands. The loop runs them
and trusts exit codes — that half is genuine, not simulated.

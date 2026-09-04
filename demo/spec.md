# Greeter — Demo Spec

A tiny shell `greet.sh` built incrementally to exercise the board end-to-end.

This spec is canned for the demo: in a real run the `brainstorming` skill would
produce it and `board-planning` would emit tickets (with prose definitions of
done) and tasks (with machine-checkable acceptance criteria). Here the demo
script stands in for those LLM steps so the board, nested tasks under tickets,
and the live UI can be shown deterministically.

## Tickets

1. **Create greeter** — `greet.sh` prints `hello`.
2. **Greet by name** — `greet.sh <name>` prints `hello <name>`.
3. **Uppercase greeting** — `greet.sh <name>` prints `HELLO <NAME>`.

Each ticket carries prose `definitions_of_done`. Tasks under each ticket hold
the shell-command `acceptance_criteria` the future Run layer would execute;
this demo persists the planner handoff only and does not claim tickets or run
criteria.

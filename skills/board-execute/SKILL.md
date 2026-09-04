---
name: board-execute
description: Use after board-planning has emitted tickets and tasks — does not claim work. Planner stops at Task; Runs (deterministic execution) are the next layer.
---

# Execute

Do not claim tickets. Do not run the leftover claim/`next` CLI. Do not execute ticket definitions of done (they are prose). Do not run task acceptance criteria yourself in this skill.

The planner's last output is Task (`abd task list --ticket-id` / `abd ticket show`). How a Task is carried out is owned by AI Board Run workflows, which are not implemented yet.

Tell the user:

- Tickets and tasks are on the board
- Execution will be a later deterministic Run layer selected by work type
- The claim/`next` command is leftover CLI, not this workflow

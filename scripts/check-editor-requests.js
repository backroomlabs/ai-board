#!/usr/bin/env node

const assert = require("node:assert/strict");
const fs = require("node:fs");

const html = fs.readFileSync("src/ui/index.html", "utf8");
const match = html.match(
  /\/\* editor-request-helpers:start \*\/([\s\S]*?)\/\* editor-request-helpers:end \*\//,
);
assert.ok(match, "editor request helper block not found");

const helpers = Function(
  `"use strict";
${match[1]}
return {
  specEditorLoadRequest,
  specEditorUpdateRequest,
  ticketEditorLoadRequest,
  ticketEditorUpdateRequest,
  taskCreateRequest,
  taskUpdateRequest,
};`,
)();

const specLoad = helpers.specEditorLoadRequest(12);
const specUpdate = helpers.specEditorUpdateRequest(12, "Spec title", "# Spec body");
assert.equal(specLoad.url, "/api/spec/12");
assert.equal(specUpdate.url, "/api/spec/12");
assert.equal(specUpdate.options.method, "PATCH");
assert.deepEqual(JSON.parse(specUpdate.options.body), {
  title: "Spec title",
  content: "# Spec body",
});

const ticketLoad = helpers.ticketEditorLoadRequest(34);
const ticketUpdate = helpers.ticketEditorUpdateRequest(
  34,
  "Ticket title",
  "Ticket description",
  ["greeter exists"],
);
assert.equal(ticketLoad.url, "/api/ticket/34");
assert.equal(ticketUpdate.url, "/api/ticket/34");
assert.equal(ticketUpdate.options.method, "PATCH");
assert.deepEqual(JSON.parse(ticketUpdate.options.body), {
  title: "Ticket title",
  description: "Ticket description",
  definitions_of_done: ["greeter exists"],
});

const ticketRequestUrls = [ticketLoad.url, ticketUpdate.url];
assert.equal(ticketRequestUrls.length, 2);
assert.equal(
  ticketRequestUrls.some((url) => url.startsWith("/api/spec/")),
  false,
  "ticket editor must not request its parent spec",
);

const taskCreate = helpers.taskCreateRequest(
  34,
  "New task",
  "code_implementation",
  "Objective",
  [],
  "",
);
assert.equal(taskCreate.url, "/api/tasks");
assert.equal(taskCreate.options.method, "POST");
assert.deepEqual(JSON.parse(taskCreate.options.body), {
  ticket_id: 34,
  title: "New task",
  work_type: "code_implementation",
  objective: "Objective",
  acceptance_criteria: [],
  context: "",
});

const taskUpdate = helpers.taskUpdateRequest(
  7,
  "Task title",
  "investigation",
  "Survey",
  ["cargo test => PASS"],
  "notes",
);
assert.equal(taskUpdate.url, "/api/task/7");
assert.equal(taskUpdate.options.method, "PATCH");
assert.deepEqual(JSON.parse(taskUpdate.options.body), {
  title: "Task title",
  work_type: "investigation",
  objective: "Survey",
  acceptance_criteria: ["cargo test => PASS"],
  context: "notes",
});

console.log("OK: editor request construction passed");

# A prompt fragment for driving Dewey applications

Paste this into an agent's system prompt. It is the same text the MCP server
returns from `initialize`, for clients that surface no MCP instructions.

Why it is needed: an ontology costs the same whether or not anyone queries it.
A model that has not been told the application describes itself reads the
source instead — slower, far larger, and an answer about what the code *could*
do rather than what is on screen. Every performance figure this project
publishes assumes the first thing and measures none of the second.

---

```text
This application is built with Dewey and describes itself. You do not need to
read its source code to drive it, and reading the source answers a different
question — what the program could do, rather than what is on screen now.

Send JSON Lines on its stdin and read replies from its stdout, one document
per line:

  {"id": "1", "request": {"type": "get_tree"}}

Start with `get_tree`. It returns every widget currently displayed, with its
id, its state, its bounds, and the actions it accepts. Act with
`execute_action` using an id from that tree:

  {"id": "2", "request": {"type": "execute_action", "agent_id": "inc", "action": "click"}}

An action a widget does not advertise is refused rather than silently ignored,
so a success means the change happened.

Three things worth knowing. `get_tree` takes `since`, the `version` from your
last reply, and answers `unchanged` without rendering anything — polling that
way costs about a hundredth of a full read, and polling is most of what you
will do. It also takes a `viewport`, and a long list is a large reply without
one. And `validate` reports faults you cannot see in a screenshot: widgets
that rendered with no id and so cannot be clicked at all, duplicate ids,
zero-size bounds, and text painted at a contrast nobody can read.

`query_ontology` describes the kinds of widget available rather than what is on
screen. It is the most expensive call in the protocol and it is what you want
before writing an interface, not while driving one.
```

---

## What this is not

Evidence that it helps. The sibling project measured the equivalent
intervention across 184 recorded runs: MCP tools raised ontology consultation
from 4% to 42%, trigger prompts took it to 83%, and score, cost and turns did
not move — one task got monotonically worse. Consultation is not the metric.

`benches/agentic/` is the harness for finding out whether it changes anything
here. Until it has been run, this fragment is a cheap and plausible measure,
not a demonstrated one.

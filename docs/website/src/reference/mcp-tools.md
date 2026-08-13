# MCP tools

`omni init` registers OMNI as an MCP server, which gives the agent **26 tools** it can
call itself without going through you.

Confirm the list against your own binary rather than this page:

```sh
{ echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"p","version":"1"}}}'
  echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'; } \
| omni --mcp | tail -1 | jq -r '.result.tools[].name'
```

## Getting content back

| tool | what it does |
|---|---|
| `omni_retrieve` | Retrieve full content a marker omitted, by its handle |
| `omni_run` | Run a shell command and return distilled output |
| `omni_signal_extract` | Extract signal from raw text, without the hook pipeline |

`omni_run` matters most on hosts that cannot rewrite their built-in shell tool. There,
it is the only path to distilled output, which is why `omni init --cursor` installs a
rule telling the agent to prefer it.

## Understanding what OMNI did

| tool | what it does |
|---|---|
| `omni_explain_savings` | Route, filter, input and output bytes, savings % per recent command |
| `omni_history` | Recent distillations with per-call savings and ratios |
| `omni_context_breakdown` | Token breakdown by source for the current turn |
| `omni_density` | How much signal against noise in a piece of text |
| `omni_budget` | Token budget usage and compression efficiency for this session |

These four are the right answer to "is OMNI helping here". Pull the numbers rather
than forming an impression.

## Memory

| tool | what it does |
|---|---|
| `omni_remember` | Store a decision, gotcha or constraint |
| `omni_recall` | Semantic search across engrams, knowledge and distillation history |
| `omni_knowledge` | Query or store cross-session project knowledge |
| `omni_insight` | Top recurring issues and error patterns across the project |
| `omni_adaptive_insights` | Retrieval patterns, as a judgement on distillation effectiveness |
| `omni_handoff` | Export session state as portable markdown, no network needed |

`omni_handoff` is MCP only. The CLI subcommand of that name was removed.

## Session and search

| tool | what it does |
|---|---|
| `omni_session` | Session state: status, context, clear |
| `omni_search` | Search this session's history |
| `omni_query` | Query distillation history with the fixed query forms |
| `omni_context` | Lightweight dependency context for a file |
| `omni_agents` | Other agents currently active on this project |

## Tuning

| tool | what it does |
|---|---|
| `omni_find_noise` | Analyse recent raw traces for repetitive noise |

> Advisory only, and the learner treats "repeated" as "noise". It has suggested
> stripping `^metadata:`, `^spec:`, code fences and `^\[stderr\]`, which are structure
> and the error channel. Never paste its output anywhere without reading it line by
> line.

## Loops

| tool | what it does |
|---|---|
| `omni_loop_status` | One-call status check for an orchestrator before each iteration |
| `omni_loop_memory` | Read and write loop memory that survives session restarts |
| `omni_set_loop_context` | Update loop context dynamically |
| `omni_budget_status` | Budget status for this iteration. Call before expensive work. |
| `omni_verify` | As a checker sub-agent, evaluate the maker agent's recent work |

See [Loop engineering](../integrations/loops.md).

## A tool that is not one

`omni_auto_noise` appears as a string in the server source and is **not** a tool. It is
a filter name passed to the TOML generator. Calling it returns `-32602 tool not found`.

It has been miscounted before: a source grep for `"omni_*"` returns 27, and 27 is
therefore wrong wherever it appears. The server's own `tools/list` says 26.

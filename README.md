<p align="center">
  <img src="assets/social-preview.png" width="720"
       alt="FerroStep — the referee for multi-agent loops: a stepped path climbing from a hollow initial-state node to a glowing terminal-state node, beside the project wordmark">
</p>

# FerroStep

**A data-driven state-machine referee for multi-agent loops, with the ledger in
your database — not in a framework's graph object.**

FerroStep is for teams running LLM agents against real work — developer/reviewer
loops, generate/QC pipelines, escalation paths — who want the loop's *rules*
(who may move what where, and how many passes before a human is called) defined
once, validated, and enforced consistently, while keeping prompts, network
calls, and agent runtimes in their own language and stack.

Instead of hiding orchestration state inside an in-memory graph, FerroStep
treats an external database (PocketBase, SQLite, Postgres, …) as the single
source of truth. Every record carries a `state` string and counters; the engine
is a pure function that answers one question:

> *May this role move this record to that state — and what does that cost?*

Your application reads a record, asks the engine, and persists what the
decision instructs. The runtime stays stateless, crash-recovery is "read the
ledger", and the whole system is inspectable with a database browser.

## What the engine guarantees

The core encodes lessons from running real agent loops in production:

- **Workflows are data, not code.** A definition is JSON (states, roles,
  transitions, counters), validated structurally at load time — unknown states,
  dead ends, exits from terminal states all fail before the first record moves.
- **Loop ceilings that survive crashes.** Counters spend on *entry* to work
  (claiming a pass costs it), so an agent that crashes mid-pass has already
  paid. A crash loop cannot become an infinite loop.
- **Exhaustion is routing, not an error.** When a ceiling is spent the engine
  answers with the state to route to instead — typically escalate-to-human.
- **Role-gated transitions.** "The worker never closes an issue; the reviewer
  resolves what it verifies" is expressible — and checked — per transition.
- **Counters belong to the operator.** The engine spends them; it never resets
  or "corrects" them. A hand-zeroed counter is a deliberate re-arm.
- **Purpose travels with the definition.** An optional, engine-opaque
  `purpose` field names why the loop exists (or points at the document that
  does — a north-star file, a mission statement), so review-role actors can be
  briefed from a stated source instead of tribal knowledge. The engine carries
  it and never interprets it.

## What FerroStep is not

It is honest about its position in your stack: the engine is *consulted*, not
in the write path. A buggy caller could skip it and write state directly —
hard enforcement belongs in your database's own access rules (row-level
security, PocketBase API rules), which express the same constraints. Defining
both from one `WorkflowDef` is on the roadmap. FerroStep gives you a single,
tested, shared implementation of the loop logic across every language in your
stack; your database gives you the lock.

## The gap this fills

Neighboring tools each want to **be the loop**: agent runtimes (PydanticAI,
smolagents) are the *actors*; graph frameworks (LangGraph) host every actor
inside their runtime; durable execution (Temporal, DBOS) resumes *a crashed
program* — but a loop whose actors are independent processes and a human
editing the database has no single program to resume; in-process state
machines (Apache Burr) referee an application that owns them. FerroStep is the
piece none of them ship: a **referee, not a runtime** — the ledger owns the
loop, and every actor, human included, is just a client of the truth.
[`docs/prior-art.md`](docs/prior-art.md) works this through tool by tool
against eight concrete requirements, and says honestly when to use the others
instead.

## Example: a worker/reviewer rework loop

```json
{
  "name": "review-loop",
  "roles": ["worker", "reviewer", "operator"],
  "states": ["awaiting_worker", "working", "awaiting_review", "approved", "escalated"],
  "initial": "awaiting_worker",
  "terminal": ["approved", "escalated"],
  "counters": [{ "name": "agent_passes", "max": 3, "on_exhausted": "escalated" }],
  "transitions": [
    { "from": "awaiting_worker", "to": "working", "role": "worker", "spends": ["agent_passes"] },
    { "from": "working", "to": "awaiting_review", "role": "worker" },
    { "from": "awaiting_review", "to": "awaiting_worker", "role": "reviewer" },
    { "from": "awaiting_review", "to": "approved", "role": "reviewer" },
    { "from": "awaiting_review", "to": "escalated", "role": "reviewer" },
    { "from": "working", "to": "awaiting_worker", "role": "operator" }
  ]
}
```

From Python:

```python
from ferrostep import Engine

engine = Engine(workflow)          # ValueError here if the definition is broken

decision = engine.authorize(
    state="awaiting_worker", counters={"agent_passes": 2},
    role="worker", to="working",
)
# {'kind': 'allow', 'to': 'working', 'counter_updates': {'agent_passes': 3}}
# Persist the flip AND the counter update in ONE atomic write.

decision = engine.authorize(
    state="awaiting_worker", counters={"agent_passes": 3},
    role="worker", to="working",
)
# {'kind': 'exhausted', 'to': 'escalated', 'counter': 'agent_passes'}
# The loop is spent: route the record to a human instead.
```

The same loop runs from Rust via `ferrostep-core` directly.

More shapes — including a product-alignment review whose ledger record is the
product itself at a point in time — live in [`examples/`](examples/). They are
illustrations, never standards: FerroStep ships **no canonical workflow**, and
the engine knows nothing about any particular one. Fluid configuration is the
product; the configurations are yours.

## Layout

| crate | what |
|---|---|
| `ferrostep-core` | Pure Rust engine: definitions, validation, decisions. No IO, no async, no database. |
| `ferrostep-py` | PyO3/maturin bindings; installs as the `ferrostep` Python package. |
| `ferrostep-github` | Represents an agent roster to GitHub as a GitHub App; today it emits the registration manifest ([roadmap](docs/github-agents-roadmap.md)). |

TypeScript bindings (NAPI-RS) are planned and the workspace leaves room for
them; they land when there is a TypeScript consumer to drive their API.

## Building

```sh
cargo test -p ferrostep-core          # core + acceptance fixture

# Python bindings (uses uv; any PEP-517 installer works)
uv venv && uv pip install ./ferrostep-py pytest
.venv/bin/pytest ferrostep-py/tests
```

## Roadmap

[`docs/ROADMAP.md`](docs/ROADMAP.md) is the roadmap of record. In one breath:
prove the referee on one real loop (two baseline ledger adapters, a surface a
human can decide from, and notifications that reach them), then compile
enforcement into the database itself, then expand by demand — the
GitHub surface ([`docs/github-agents-roadmap.md`](docs/github-agents-roadmap.md)),
more backends, more languages — until the engine referees its own
development.

## License

Apache-2.0. Copyright 2026 Artificial Humanity LLC.

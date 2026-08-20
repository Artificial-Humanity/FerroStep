# FerroStep — agent orientation

This file is authoritative inside this repo. The workspace map one level up
routes here; nothing there overrides anything here.

**This repo is public.** Nothing lab-internal belongs in it: no hostnames, no
service names, no credential inventory, no references to specific internal
deployments. Describe workloads generically ("a worker/reviewer loop"), never
by their home.

## What this is

A Rust-core, polyglot-bindings engine that referees database-ledger multi-agent
loops. `README.md` is the product description; `north-star.md` is the *why*
(its Vision section is an unratified draft until the owner signs it).

## Layout

- `ferrostep-core/` — the engine. **Pure by rule**: no IO, no async, no clock,
  no database, no network. It takes a definition and a snapshot and returns a
  decision. If a change needs a side effect, it belongs in an adapter crate or
  the binding layer, not here.
- `ferrostep-py/` — PyO3/maturin bindings, mixed layout: Rust bridge in `src/`,
  pure-Python surface in `python/ferrostep/`. The bridge speaks JSON strings;
  the Python wrapper turns them into dicts. Keep new API in the wrapper thin.
- `ferrostep-ts/` — does not exist yet, deliberately. It lands when a
  TypeScript consumer exists to drive its API. Don't scaffold it speculatively.
- `examples/` — illustrations of configuration, never standards (see
  conventions below); kept honest by the core's `shipped_examples_stay_valid`
  test.
- `workflow/` — the working conventions: the persona files `config.yaml`
  routes to (an agent adopts the default entry's persona via `CLAUDE.md`,
  which imports it). There is deliberately no second, reviewing persona and
  no review lane — see the persona's §3.
- `config.yaml` — the single place this repo's configurable working values
  live: today, the agent roster (titles, identities, persona paths). **Prose
  points at a value here and never writes it out** (owner, 2026-08-20) — a
  restated value, a *title* included, is a second copy waiting to drift.
  `cargo xtask agent-env` is the reader that turns an entry into shell
  variables.
- `xtask/` — repo tooling, invoked as `cargo xtask` (alias in
  `.cargo/config.toml`): the config reader today. Not a product crate, never
  published; its test guards `config.yaml` (parses, default agent complete,
  persona file exists).
- `docs/` — true and proper documentation. **Write a document here when it is
  a deliverable**: finished, public-facing, something an outside reader is
  meant to find and the README can link into (prior-art lives here).
- `notes/` — the long-term scratchpad (owner, 2026-08-20). **Write a document
  here when it serves the work rather than the reader**: working thoughts,
  investigations, drafts. A placeholder README keeps the location present
  even when it is otherwise empty; a document that graduates moves to
  `docs/`.

## Conventions

- **Workflow definitions are data.** Never encode a specific workflow's states
  as Rust enums in the core. The reference review-loop lives only in tests and
  docs, as a fixture.
- **No blessed workflows** (owner, 2026-08-20: fluid configuration, not set
  standards). `examples/` are illustrations; never present them as normative,
  and never make the engine aware of any specific workflow. The `purpose`
  field is engine-opaque and must stay so — the engine has no concept of what
  a review, an alignment check, or any other workflow *means*.
- **Decision JSON is a public contract.** `kind: allow | exhausted | deny` and
  their fields are what every binding and app layer switches on; changing the
  shape is a breaking change and needs a version bump and a changelog entry.
- Python tooling is **uv only** (`uv venv`, `uv pip install ./ferrostep-py`);
  never introduce bare pip/venv or poetry.
- **Favor Rust when the choice of tool is ambiguous; otherwise pick the best
  tool for the job** (owner, 2026-08-20). This is not a purity rule — the
  Python bindings are a first-class citizen — it is a tiebreaker.
- License is Apache-2.0; new files need no per-file headers. **Every
  dependency and vendored tool must be license-compatible with Apache-2.0**
  (owner, 2026-08-20) — check the license before adding it, and record the
  check in the commit message that introduces it.

## Build & test

```sh
cargo test -p ferrostep-core -p xtask           # fast, no Python needed
cargo build -p ferrostep-py                     # checks the bridge compiles
uv venv && uv pip install ./ferrostep-py pytest # build + install bindings
.venv/bin/pytest ferrostep-py/tests
```

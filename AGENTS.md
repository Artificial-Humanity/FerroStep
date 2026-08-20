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
- `workflow/` — the working conventions. `DEVELOPER.md` defines **Cyndi**, the
  developer persona every agent here adopts by default (routed via `CLAUDE.md`,
  which imports it). There is no reviewer persona and no review lane,
  deliberately — see that file's §3.

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
- License is Apache-2.0; new files need no per-file headers.

## Build & test

```sh
cargo test -p ferrostep-core                    # fast, no Python needed
cargo build -p ferrostep-py                     # checks the bridge compiles
uv venv && uv pip install ./ferrostep-py pytest # build + install bindings
.venv/bin/pytest ferrostep-py/tests
```

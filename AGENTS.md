# FerroStep — agent orientation

This file is authoritative inside this repo. The workspace map one level up
routes here; nothing there overrides anything here.

**This repo is public.** Nothing lab-internal belongs in it: no hostnames, no
service names, no credential inventory, no references to specific internal
deployments. Describe workloads generically ("a worker/reviewer loop"), never
by their home.

## What this is

A Rust-core, polyglot-bindings engine that referees database-ledger multi-agent
loops. `README.md` is the product description; `docs/north-star.md` is the *why*
(its Vision is owner-ratified; its "ours vs rented" section is a stated default
to argue against, not a constraint — the section says so itself).

## Layout

- `ferrostep-core/` — the engine. **Pure by rule**: no IO, no async, no clock,
  no database, no network. It takes a definition and a snapshot and returns a
  decision. If a change needs a side effect, it belongs in an adapter crate or
  the binding layer, not here.
- `ferrostep-ledger/` — the interface a ledger adapter implements: records as
  objects, capabilities stated honestly, one write path. The contract adapters
  are judged against; it holds no IO of its own.
- `ferrostep-sqlite/` — the SQLite ledger adapter, and the zero-install path:
  one host, one WAL-mode file, no server. Compare-and-swap and the atomic
  apply are by construction; append-only history is trigger-enforced.
- `ferrostep-py/` — PyO3/maturin bindings, mixed layout: Rust bridge in `src/`,
  pure-Python surface in `python/ferrostep/`. The bridge speaks JSON strings;
  the Python wrapper turns them into dicts. Keep new API in the wrapper thin.
- `ferrostep-ts/` — does not exist yet, deliberately. It lands when a
  TypeScript consumer exists to drive its API. Don't scaffold it speculatively.
- `ferrostep-github/` — the GitHub App sub-project: represents a repo's agent
  roster to GitHub. v0 emits the App manifest for one-step registration and
  nothing more; the phased plan (through GitHub-side agents, e.g. a reviewing
  persona in the PR process) is `docs/github-agents-roadmap.md`, and later
  phases are deliberately absent until they arrive.
- `examples/` — illustrations of configuration, never standards (see
  conventions below); kept honest by the core's `shipped_examples_stay_valid`
  test.
- `skills/` — skills that **deploy with the product**, for actors *inside*
  loops the engine referees (open `SKILL.md` format). Product artifacts with
  `examples/`'s admission bar: a skill lands when a real loop consumes it.
  **Write a skill here when it serves the loops FerroStep referees; a skill
  that serves work *on* FerroStep itself is a working convention and belongs
  in `workflow/skills/` instead** — which does not exist yet, deliberately,
  and is created with its first real skill (owner, 2026-08-20). Do not
  conflate the two: one ships, the other stays a repo convention.
- `workflow/` — the working conventions: the persona files `config.yaml`
  routes to (an agent adopts the default entry's persona via `CLAUDE.md`,
  which imports it), and eventually `skills/` for repo-working skills (see
  above). There is deliberately no second, reviewing persona and no review
  lane — see the persona's §3.
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
- `assets/` — project identity. `icon.png` (1024px, alpha) is the icon, and
  the GitHub App avatar (a manual upload — `ferrostep-github/README.md` says
  why registration cannot do it); `icon.svg` holds the canonical
  geometry (a state path climbing steps: hollow initial state, ferrous
  treads, filled terminal state) and was the generation reference. The PNG
  is a Lucida (google-lane) refinement composited onto clean geometry, and
  carries an invisible SynthID watermark. `social-preview.png` (1280x640,
  GitHub's recommended size) is the banner built from that mark and the
  wordmark: the README's header, and the file uploaded as the repo's social
  preview card.
- `docs/` — true and proper documentation. **Write a document here when it is
  a deliverable**: finished, public-facing, something an outside reader is
  meant to find and the README can link into (prior-art lives here). The
  **deployment map** (`docs/deployment-map.md`) is the single place
  deployment disposition is recorded — what ships, through which channel,
  what never leaves — and the xtask test `deployment_map_covers_the_tree`
  fails when a tracked top-level path is missing from it.
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
- **Everything outside the engine is an adapter** (owner, 2026-08-21). Define
  the thing internally first — the ledger record, the issue log, the
  notification, the agent interface — then reach the world through an adapter
  for it. No vendor, product or service name belongs at framework level: a
  store, a notifier or a coding agent is something an adapter speaks to,
  never something the core knows about. This is what keeps the north star's
  organizing principle affordable — the engine stays a pure function because
  every side effect it implies lives on the far side of an adapter boundary.
  ⚠ **Shape each interface around what the thing IS, not around how one target
  happens to deliver it** (owner, 2026-08-21). Message transports are not
  alike: one takes a URL, the next needs service credentials and a payload
  envelope, the next a device token and a signed key, the next is a local
  program. An interface modelled on any single one of them cannot reach the
  others, and the corner is only visible once you are in it. The standing test
  for any external surface is whether somebody could write a simple adapter for
  a target nobody here has thought of.
  ⚠ **Shipping a default adapter is not the same as naming a vendor at
  framework level** (owner, 2026-08-21), and over-applying the rule above is a
  real risk — a stack has to function, so defaults ship and are maintained.
  They hold the bar `examples/` holds: a worked example somebody copies to
  write the next one, never a blessed one, and never granted standing in the
  interface they implement.
- **Decision JSON is a public contract.** `kind: allow | exhausted | deny` and
  their fields are what every binding and app layer switches on; changing the
  shape is a breaking change and needs a version bump and a changelog entry.
- Python tooling is **uv only** (`uv venv`, `uv pip install ./ferrostep-py`);
  never introduce bare pip/venv or poetry.
- **Favor Rust when the choice of tool is ambiguous; otherwise pick the best
  tool for the job** (owner, 2026-08-20). This is not a purity rule — the
  Python bindings are a first-class citizen — it is a tiebreaker.
- ⚠ **Derive counts; never state them in prose** (2026-08-21). "Every call site
  of X" ages well. "Five places" is stale the moment someone adds a sixth, and
  unlike a wrong number in code, **nothing will ever go red**. The distinction
  that makes this usable rather than pious: a count in a *test* is fine,
  because it fails when reality moves — `the_emitted_keys_are_fixed_not_derived`
  asserts a number precisely so a fifth key cannot appear quietly. Prose has no
  such property, so prose does not get to hold one. ⚠ This applies to issue and
  finding text as much as to documents, and more urgently: tracker text is
  frozen at filing while the tree keeps moving underneath it, which makes it the
  worst possible home for a hardcoded count and the last place anyone thinks to
  look.
- ⚠ **A guard must ask the question it actually means, and must fail when it
  checked nothing** (2026-08-21, both measured here). Two ways a green guard
  lies, and neither is visible from its passing output:
  - **Existence is a working-tree question; shipping is an index question.**
    `Path::is_file` and `git ls-files` disagree in precisely the case worth
    catching — a file present locally and never added, which works perfectly
    for whoever wrote it and is absent from every fresh clone. Measured in a
    sibling repo: a full suite passed green while the module a release step
    imported was untracked. Ask the index when the requirement is "a stranger
    gets this too". (Resist `--others --exclude-standard` as the discriminator
    unless you need it: it also reads per-host, unshared exclude files, so it
    can call a legitimate build output "forgotten".)
  - **An enumerating guard needs a floor, on the population that MATTERS.** A
    check over an empty collection reports success, having verified nothing —
    and the outer list is the wrong thing to floor, because a filter can match
    none of it while the list itself is healthy. Floor whatever the assertion
    actually iterates. In the same family, and worse in Python: `parametrize`
    over an empty list reports `1 skipped` and exits 0.
- ⚠ **A concurrency or timing property needs repeated rounds and a failure
  count. One green run is not evidence, it is a coin landing your way.** A
  measurement of this shape was taken against a candidate backend and its first
  round passed cleanly; running it again showed the property failing in most
  rounds, with two writers enough to break it. Had it stopped at one round it
  would have reported the opposite conclusion with a real measurement behind
  it. This is the same family as *check the instrument ran before believing a
  negative*, in the direction that flatters you: a passing probabilistic test
  is the easier mistake to make and the harder one to notice.
- License is Apache-2.0; new files need no per-file headers. **Every
  dependency and vendored tool must be license-compatible with Apache-2.0**
  (owner, 2026-08-20) — check the license before adding it, and record the
  check in the commit message that introduces it. ⚠ **That strictness is
  about what we BUNDLE** (owner, 2026-08-21). What an adapter merely talks to
  — a service reached over HTTP, a binary it shells out to — is not vendored
  and is not held to that bar, unless the licence is unusually onerous.

## Build & test

```sh
cargo test -p ferrostep-core -p xtask           # fast, no Python needed
cargo build -p ferrostep-py                     # checks the bridge compiles
uv venv && uv pip install ./ferrostep-py pytest # build + install bindings
.venv/bin/pytest ferrostep-py/tests
```

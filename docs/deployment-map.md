# Deployment map

What ships from this repo, through which channel, and what never leaves it.
This file is the single place deployment disposition is recorded; the layout
section of [AGENTS.md](../AGENTS.md) says what each path is *for*, and this
map says only where it *goes*.

**Maintained by mechanism, not memory:** the xtask test
`deployment_map_covers_the_tree` fails when a tracked top-level path has no
mention here, so adding a directory forces classifying it. That guard checks
*mention*, not truth — a green run means "nothing is unclassified", never
"every classification is right" — and it reads the git index, so a path is
invisible to it until `git add`.

"Ships" means a consumer of the product uses it. "Never ships" means it
exists to build or govern the product — the repo is public, so everything is
*readable*; this map is about deployment, not visibility.

## Ships

| artifact | ships as | channel | status (2026-08-20) |
|---|---|---|---|
| `ferrostep-core/` | Rust crate `ferrostep-core` | crates.io | not yet published |
| `ferrostep-ledger/` | Rust crate `ferrostep-ledger` — the interface an adapter implements | crates.io | not yet published |
| `ferrostep-sqlite/` | Rust crate `ferrostep-sqlite` — the SQLite ledger adapter, the zero-install path | crates.io | not yet published |
| `ferrostep-pocketbase/` | Rust crate `ferrostep-pocketbase` — the PocketBase ledger adapter, plus the generated migration and hook files it installs | crates.io; the generated files land in a deployment's PocketBase directory | not yet published |
| `ferrostep-notify/` | Rust crate `ferrostep-notify` — the notification message and its delivery adapter boundary, with the ntfy default | crates.io | not yet published |
| `ferrostep-roster/` | Rust crate `ferrostep-roster` — the actor roster a deployment configures: titles, the identity work is signed under, and the persona document a launcher hands an agent | crates.io | not yet published |
| `ferrostep-cli/` | Rust crate `ferrostep-cli`, installing the `ferrostep` binary — decision surface, move, audit, notify wiring, and `agent-env` (the roster reader, which is how a repo with no Rust toolchain resolves an actor) | crates.io | not yet published |
| `ferrostep-py/` | Python package `ferrostep` | PyPI | not yet published |
| the `ferrostep` binary | **one static executable per target**, fetched and run — the light-touch install | a release channel (B10, rung 0.4.0) | planned 2026-09-14; nothing built yet |
| `ferrostep-github/` | Rust crate/binary `ferrostep-github`, and a registered GitHub App instance per org | crates.io; the App via GitHub registration | scaffold; no App registered yet |
| `skills/` | actor skills (`SKILL.md` format) | with the product; channel decided with the first skill | empty by design |
| `examples/` | workflow definitions, copy-and-edit | the repo itself | live |
| `docs/` | project documentation | the public repo | live |
| `assets/` | the project identity: icon (SVG geometry + rendered PNG) and social-preview banner | the public repo; copied into the org profile repo (`.github/assets/ferrostep-icon.png`); the banner uploaded once as the repo's social preview (Settings — no API exists); the icon uploaded by hand as the GitHub App avatar *after* registration (the manifest carries no logo field) | live; social preview awaiting the owner's one-click upload, App avatar awaiting an App |
| `README.md`, `LICENSE`, `CHANGELOG.md` | the repo's public surface | the public repo | live |

Planned but absent: `ferrostep-ts` (npm, when a TypeScript consumer exists) —
it gets a row when the crate does.

⚠ **A crate and a binary are two channels for two readers, and this file said
only one of them existed** (corrected 2026-09-14). Every row above was a crate,
which is right for somebody writing an adapter and useless to somebody who just
wants the tool: it left "install it" meaning "clone it and build eight crates".
The owner's goal is a curl or an unzip, so the artifact row now exists ahead of
the artifact — **the status column is what says it is not built yet**, and that
is the honest way round. An absent row would have read as a decision nobody
made.

## Never ships

| path | what keeps it home |
|---|---|
| `workflow/` | working conventions: personas, and repo-working skills when they exist |
| `config.yaml` | the repo's *own* agent roster — the format ships (`ferrostep-roster`), this instance of it does not |
| `xtask/` | repo tooling; `publish = false` in its manifest is the mechanism |
| `.cargo/` | the cargo alias that invokes xtask |
| `notes/` | long-term scratchpad — **moved out of this repo 2026-09-08**; it is a private repo reached through a gitignored symlink, so a clone has neither the files nor the path |
| `AGENTS.md`, `CLAUDE.md` | rules of record and persona routing |
| `Cargo.toml`, `Cargo.lock`, `.gitignore` | repo plumbing |

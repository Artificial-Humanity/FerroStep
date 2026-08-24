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
| `ferrostep-py/` | Python package `ferrostep` | PyPI | not yet published |
| `ferrostep-github/` | Rust crate/binary `ferrostep-github`, and a registered GitHub App instance per org | crates.io; the App via GitHub registration | scaffold; no App registered yet |
| `skills/` | actor skills (`SKILL.md` format) | with the product; channel decided with the first skill | empty by design |
| `examples/` | workflow definitions, copy-and-edit | the repo itself | live |
| `docs/` | project documentation | the public repo | live |
| `assets/` | the project identity: icon (SVG geometry + rendered PNG) and social-preview banner | the public repo; copied into the org profile repo (`.github/assets/ferrostep-icon.png`); the banner uploaded once as the repo's social preview (Settings — no API exists); the icon uploaded by hand as the GitHub App avatar *after* registration (the manifest carries no logo field) | live; social preview awaiting the owner's one-click upload, App avatar awaiting an App |
| `README.md`, `LICENSE`, `CHANGELOG.md` | the repo's public surface | the public repo | live |

Planned but absent: `ferrostep-ts` (npm, when a TypeScript consumer exists) —
it gets a row when the crate does.

## Never ships

| path | what keeps it home |
|---|---|
| `workflow/` | working conventions: personas, and repo-working skills when they exist |
| `config.yaml` | the repo's agent roster |
| `xtask/` | repo tooling; `publish = false` in its manifest is the mechanism |
| `.cargo/` | the cargo alias that invokes xtask |
| `notes/` | long-term scratchpad |
| `AGENTS.md`, `CLAUDE.md` | rules of record and persona routing |
| `Cargo.toml`, `Cargo.lock`, `.gitignore` | repo plumbing |

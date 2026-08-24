# Changelog

Notable changes, per release. What a version *means* is defined in
[docs/ROADMAP.md](docs/ROADMAP.md) §Releases — outcomes, not dates. The
Decision JSON rule in [AGENTS.md](AGENTS.md) §Conventions is one reason an
entry here is mandatory rather than courtesy.

## Unreleased

Everything to date — the first release has not been cut.

- `ferrostep-ledger`: `Scope::matches` and `decided_snapshot` — the one shared
  meaning of "apply this decision to this snapshot".
- `ferrostep-sqlite`: the first ledger adapter. WAL-mode SQLite on one host;
  atomic apply and compare-and-swap by construction, append-only history
  enforced by triggers, all three capability flags earned by tests including
  a repeated-rounds concurrency battery.

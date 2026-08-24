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
- `ferrostep-pocketbase`: the second ledger adapter — a stock instance plus a
  generated migration and transactional apply/create routes (the compare
  inside the store's transaction, the only placement that measured sound).
  Detects at connect time whether the routes are installed and says which
  mode it is in; without them it is read-only and refuses writes by name.
  Live end-to-end loop and concurrency battery ship as ignored-by-default
  tests, run against a real instance.
- `ferrostep-pocketbase`, again: **mapped deployments** — a `CollectionMap`
  referees an existing collection's own columns (state, counters, version
  token, scope labels), so a loop already living in a collection keeps one
  truth and its console view; filing stays with the collection's own
  procedure and is refused by name. Generated routes became
  collection-scoped so refereed collections cannot collide. An optional
  generated release hook makes writing a decision field perform the
  definition's release transition with the referee's bookkeeping (version
  bump, event append) — the store-side transition B5 warns about, as
  generated output instead of a hand-written peer.
- `ferrostep-notify`: the notification message — which record, why, how
  urgently, how to get back — and the `Notifier` adapter boundary, with ntfy
  as the maintained default. Nothing polls or schedules; callers decide when.
- `ferrostep-cli`: the `ferrostep` binary. `awaiting` renders which records
  await a person and what their moves would actually do; `move` resolves one
  without a database console; `audit` reports what happened (moves,
  escalations, releases, last note) from the same enumeration `awaiting`
  reads; `notify` sends one notification per awaiting record.

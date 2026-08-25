# Changelog

Notable changes, per release. What a version *means* is defined in
[docs/ROADMAP.md](docs/ROADMAP.md) §Releases — outcomes, not dates. The
Decision JSON rule in [AGENTS.md](AGENTS.md) §Conventions is one reason an
entry here is mandatory rather than courtesy.

## Unreleased

- `ferrostep-cli`: `explain` no longer panics on a maximal ceiling. It prints
  each asserted number *and its off-by-one neighbour*, and computed that
  neighbour with `max + 1` — which overflows at the top of the range: a crash
  in a debug build, and in release a wrap to `0` that would have sent the
  reader hunting their tree for the wrong number entirely. A ceiling is a
  value out of a file somebody else wrote, and this is the subcommand whose
  audience is a person who has not got the system working yet.
- `ferrostep-cli`: the audit no longer reads a state change out of a rescope.
  A rescope moves a record between units of work and leaves it in the state it
  was already in, so its event carries `to` equal to `from_state` — which
  satisfied both the escalation test (arrived somewhere halted) and the release
  test (departed somewhere halted) at once. Rescoping a paused record therefore
  reported an escalation *and* a release for a record that had not moved. Not a
  crash and not a visibly wrong number: a **plausible story**, on the one
  surface offered to a person who is deliberately not opening a database
  console to check. Both tallies now ask whether the record moved at all.
- `ferrostep-roster`: the actor roster as a product surface. A deployment's
  `config.yaml` names its agents by title; each entry carries the identity work
  is signed under and the persona document that tells that agent how to behave.
  Titles are configured values and the crate knows nothing by any of them. The
  persona resolves against the roster's own directory and is checked to exist
  before it is emitted, because that path is what a launcher hands to
  `--system-prompt-file`.
- `ferrostep-cli`: `agent-env` — the roster as shell assignments, taking no
  workflow and no store. A repo adopts a roster before it adopts a referee, and
  a repo with no Rust toolchain could not reach the reader at all while it was
  an `xtask` subcommand. Every failure is a refusal with a message rather than
  an empty assignment at status zero: a caller `eval`s this and then commits
  with it, so falling back is how work gets signed under the wrong name.
  `--format json` answers the same resolution for a caller that is not a
  shell, so recovering a name does not require decoding shell quoting in
  another language.
- `ferrostep-cli`: `explain` — what a definition permits, readable without a
  store. Its numbers section exists because of a migrating loop, not taste: when
  a ceiling moves into a definition, FerroStep owns the *value* and knows
  nothing about the *arithmetic derived from it* elsewhere in the adopter's tree
  — `max + 1` in a guard, a range in help text, a sentence in a brief handed to
  an actor. That arithmetic does not contain the value it came from, so a search
  for the ceiling finds none of it. Three times in one migration the search term
  that worked was a number the definition never states, so `explain` prints the
  asserted values *and* their off-by-one neighbours.
- `xtask agent-env` now delegates to `ferrostep-roster` rather than carrying a
  second reader of the same format.
- **Rescope: moving a record between units of work is now a refereed
  operation.** A record's scope decides which queries find it, so a record
  whose scope names a finished unit is invisible to all of them — and until
  now nothing could move one, so consuming loops did it as un-versioned,
  un-evented writes to the field every query depends on. A definition grants
  it per label and per role (`rescopes`), or nobody has it; `ferrostep
  rescope` performs it; it lands versioned and evented like any other move and
  shows up in `audit`. ⚠ Refused on terminal records, and that is not
  configurable: a finished record's scope is the provenance of what it was
  resolved against.
- `Decision::Allow` grows `scope_updates`, omitted from the JSON when empty —
  so a consumer written before rescope existed reads byte-identical JSON for
  everything it already handled, and no fourth `kind` was added for every
  binding to learn.
- `CounterDef` grows `exhausted_requires_note`: the attempt that finds a ceiling
  spent can be required to say what decision is being asked for. Exhaustion
  routes a record to a person, and an automatic route arrives in front of them
  with **no question attached** — which is the whole content of the handover.
  The actor that just ran out of attempts is the one who knows what cannot be
  settled, and that is the moment it knows it. Deliberately not `requires_note`
  on the spending transition, which would tax every attempt when only the last
  one is addressed to anybody; the decision surface still shows where a spent
  ceiling would route, because it offers moves with a note already attached.
- `ferrostep-pocketbase`: the generated ping now states what the installed
  routes can write, and the adapter reads it. Hooks are deployed separately
  from the binary, so a current adapter meets older routes routinely — and
  those answer an apply carrying scope updates with a cheerful 200 while
  writing no label. That is now refused by name, with the remedy in the
  message, instead of being reported as a move that happened. In mapped
  deployments the writable labels are the map's `scope_fields` and nothing
  else, as one generated line per declared label rather than a loop over
  whatever a request names.

## 0.1.0 — 2026-08-24

The internal MVP ([ROADMAP §Releases](docs/ROADMAP.md)): cut on the owner's
judgment after the lane's store was provisioned live and a real record ran
the full refereed cycle — a pass claimed and spent, a genuine design
escalation, the owner's release through the generated hook, and a close —
all of it in the ledger's own history.

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

# Cutover runbook — a hand-driven loop moves onto the engine

Working notes, not a deliverable: B5's procedure, written down *before* the
first cutover so its one structural decision is made before the move rather
than during it. Generic on purpose — which loop, which store, and when are
the owner's; this is the order of operations any cutover follows.

## 0. The two-referees decision, first

A loop being migrated already has enforcement somewhere. Inventory every
store-side hook or rule that *performs* a transition today — changes a
state, clears a counter — as opposed to merely refusing one (refusals are
defence in depth and stay). Each performer either **retires into the
definition** (its behaviour becomes a transition the engine referees) or
becomes **generated output** of the definition (B6). It must not survive as
a hand-written peer: two referees free to disagree, with nothing comparing
them. For the first cutover the standing proposal
([road-to-0.1.0](road-to-0.1.0.md) §6) is *retire into it*.

## 1. Author the definition

The lane's practiced reality, as data: states, roles (people marked
`human`), counters with ceilings and their `on_exhausted` routes, the
release transitions with their `resets`, `requires_note` where a move is
useless without a reason. `purpose` points at the north-star. Validation is
the first test: `Engine::new` refuses dead ends, unreachable states, halts
without a human way back, unpaid agent resets — fix the definition, not the
validator.

## 2. Provision the ledger

- **PocketBase**: `cargo run -p ferrostep-pocketbase --example install --
  <dir>`, then confirm the ping route answers (⚠ a watching server restarts
  itself when the hook file lands; a health check racing that restart is not
  evidence). Role-scoped accounts per actor — never administrators (B1's
  standing ruling); each actor's token is what the adapter connects with.
- **SQLite**: nothing to provision — the adapter creates its schema on open.
  The right store for the rehearsal below even when production is elsewhere.

## 3. Rehearse on a throwaway

Run the definition end-to-end against a throwaway store before any real
record exists: file, claim, submit, send back, exhaust the ceiling, watch it
route, release with the reset, finish. The adapters' own live tests are the
template. The rehearsal is also where `ferrostep awaiting`, `move`, `audit`
and `notify` are pointed at the real definition for the first time.

## 4. Backfill or fresh start

Decide what happens to the loop's open matters: they enter the new ledger as
*filings at cutover* (their history starts here), or the old lane drains
while new work starts refereed. ⚠ Never synthesize past history into
invented events — a chronology that pretends to predate its ledger is a
false record wearing an audit trail.

## 5. The first refereed change

B5's done-when, exercised for real and not in a fixture: a real change ships
through the loop with a ceiling spent and an escalation exercised — found on
the decision surface, resolved with `move`, visible in `audit`, heard
through `notify`.

## 6. Rollback stays open

The hand lane stays runnable until the done-when is met. The ledger is
additive — nothing in the cutover deletes or rewrites what the old lane
kept — so falling back is stopping, not restoring.

## 7. Then the cut

ROADMAP §Releases: when the owner judges the replacement deployable, the
pre-release marker comes off the workspace version (the internal dependency
requirement moves with it — a caret requirement does not match a
pre-release), the changelog gains the release entry, and the tag is cut.

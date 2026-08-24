# Road to 0.1.0 — working plan

Working notes, not a deliverable. 0.1.0 is outcome-defined in
[ROADMAP §Releases](../docs/ROADMAP.md); this note orders the work between
here and that cut and names each step's first move. The ordering and shapes
are the resident developer's proposals (2026-08-24), owner-directed like
everything else; roadmap item definitions are pointed at, never restated.

## Where the tree stands

The referee core and Python bindings are built and tested. `ferrostep-ledger`
is a complete, documented contract with nothing implementing it. The write
path for the first backend is measured and recorded — compare inside the
store's transaction, with the failing variant measured beside it
([the requirements note](ledger-requirements-and-pocketbase.md)) — so B1
begins as implementation, not investigation.

## 1. B1, first half — the PocketBase adapter

First because the lane 0.1.0 replaces already runs on this store, and its
write path is the one already measured. A new adapter crate (proposal:
`ferrostep-pocketbase`) implementing `Ledger`, plus its deployment-map row
and the license audit any new dependency needs (an HTTP client at minimum).

The shape is settled by the requirements note §3 — stock instance plus a
generated, self-contained hook route applying a decision transactionally
server-side. The adapter's obligations, each from the measured record:

- Emit self-contained handlers; the hook runtime isolates each callback, so
  factored-out helpers fail at call time.
- The version token is an adapter-owned integer compared for equality —
  never the store's timestamps.
- Match refusal messages case-insensitively, and never by their tail; the
  store normalizes them.
- Enumeration pages to completion and asserts against the store's reported
  total, with the skip-total flag explicitly off.
- Capability flags come from measurement; startup detects what is installed
  and says which mode it is running in.
- Error mapping is measured for conflicts and not-founds and inferred for
  validation failures — and says so where the mapping is written.

⚠ **The concurrency battery ships as a runnable check, not a memory of one.**
An opt-in integration test — live store address provided ⇒ runs; absent ⇒
reported as skipped, never silently green — with repeated rounds and a
failure count, per AGENTS.md's repeated-rounds rule. It is what earns
`compare_and_swap` on every future change, not once.

*Done when:* the reference review-loop fixture runs end-to-end against a
live instance and the battery holds.

## 2. B1, second half — the SQLite adapter

Immediately after, before any consumer hardens against one store's shape —
the second implementation is what proves the interface, on B1's own terms.
Also the zero-install path. `rusqlite` (license checked before it is added),
WAL mode, one host by SQLite's own rule. Compare-and-swap is native here;
append-only history is enforced by the storage rather than a convention.
B1's done-when then closes on both backends: the version-guarded write
proving crash-accounting against a real database.

## 3. B2 — the decision surface

One enumeration — records awaiting a person, and the moves their role has —
with one renderer over it, and any narrating agent a consumer of the same
query. MVP form is judged against the lane's real needs; plausibly a CLI
report before anything richer. B2's done-when is the bar: a real escalation
resolved without a database console.

## 4. B3 — notifications

The message type defined internally; ntfy as the maintained default
adapter. MVP scope: the lane's escalations reach a person who was not
watching.

## 5. B4 — the audit report

A second reader of B2's enumeration, so the views cannot disagree.
Informational; depth judged at the cut.

## 6. B5 — the cutover, then the cut

The hand-driven lane moves onto the engine. Ahead of it sits the
two-referees decision B5 warns about: existing store-side enforcement
becomes generated output of the definition or retires into it — for the
MVP, the proposal is *retire into it*, with generation arriving as B6 when
it earns its place. B7's first shipped skill likely lands here (the
worker's), deciding the skills channel with a consumer in hand.

When the owner judges the replacement deployable: drop the pre-release
marker (the internal dependency requirement moves with it), write the
changelog entry, tag 0.1.0.

## Deliberately not on this road

- **Presence** — sequenced independently of everything above: it has an
  in-house consumer and slots in whenever the owner rules, gating nothing
  here.
- **The expansion tier**, E5 included — 1.0.0-line polish, exercised
  in-house when each item's turn comes.
- **B6** — after the MVP unless the cutover proves it needed earlier; the
  hook layer's measured behaviour already provides depth.

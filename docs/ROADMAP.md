# FerroStep — roadmap

The ordering of intent. [`north-star.md`](north-star.md) is *why*, the
[README](../README.md) is *what*, this is *in what order and why that order*.
It sequences; it does not re-legislate — the standing rules (fluid
configuration, no speculative scenarios, the admission bar of a real consuming
loop) live in [AGENTS.md](../AGENTS.md) and the north star, and every
milestone below inherits them.

**Where it stands (2026-08-21):** the referee core and Python bindings are
built and tested; workflows are data with a validated reference loop; the
repo's own conventions (identity roster, deployment map, guards) are
mechanized; the GitHub App sub-project emits its registration manifest.
Nothing is published to a registry yet, and no production loop runs on the
engine. Everything below is ordered around changing that last fact first.

---

## Baseline — the product proving itself

The baseline is done when two things are true: **the author's own loop runs
on the engine daily**, and **a stranger can read this repo and run their own
loop on their own database by imitation**. Everything in this tier serves one
of those two sentences.

**B1 — Baseline ledger adapters (PocketBase and SQLite).**
Derive the minimal ledger interface from what the engine actually needs —
load a snapshot, apply a decision atomically (state flip + counter spends in
one guarded write), append an event, and enumerate the records awaiting
someone — then implement it twice, honestly. Two from the start rather than
one, because a second implementation is the only proof the interface is
generic, and because it exposes what the first one hides: a backend with a
console of its own answers "what needs me?" without the interface ever being
asked, and SQLite has no console to hide behind (owner, 2026-08-21). The
adapter is where each backend's real atomicity story is made explicit, never
papered over.

SQLite is also the **zero-install path**, which is a first-class concern and not
a courtesy: a first loop on one developer's machine needs no server, no account
and no configuration, because every actor is a separate process on the same host
and that is exactly the case SQLite's WAL mode supports — readers and writers
concurrent, one writer at a time. ⚠ It stops there, by SQLite's own rule: WAL
needs shared memory between processes, so **all of them must be on one host, and
a database file on a network share is corruption waiting rather than a
small-team deployment.** The moment actors span machines the ledger has to be
something reachable over a network, and that is the line the other adapters
exist on the far side of.

An event carries the actor, the move, the counter changes, and an opaque note
(owner, 2026-08-21). The note matters because a record can be released from a
pause more than once, and a human's reasoning for each release has to survive
the next one — a single field on the record is overwritten by the second
decision. It belongs in the event log rather than in a decisions table beside
it, which would be a second chronology of the same record, free to disagree
with the first. A *comment* is discussion that moves nothing and stays its own
thing; a *decision* is a move with a reason attached, and the log is where
moves already live.
*Done when:* the reference review loop runs end-to-end on both, with a
version-guarded write proving the crash-accounting promise survives contact
with a real database.

**B2 — The decision surface.**
Escalation routes a record to a human, and nothing today lets that human find
it or see what they may do about it. This milestone answers the *blocking*
question — which records await someone, and which moves does their role have
— and renders it for a person. A ledger browser shows a row; this shows a
decision.
Both the rendered view and any agent that narrates an escalation are
consumers of that one query rather than independent readers of state, so the
presentation cannot drift from the ledger.
*Done when:* a human resolves a real escalation from the rendered view
without opening a database console.

**B3 — Notifications, as an adapter.**
A decision surface nobody looks at is a record that waits forever. FerroStep
emits a notification when something needs a person; it never polls, never
schedules, and never decides when work runs — which is what keeps this on the
right side of the non-goals below.

**The message is defined here; delivering it is somebody else's problem.** A
message says which record needs a person, why, how urgently, and how to get
back to it. That much is ours and does not vary. Everything past it is an
adapter.

⚠ **This milestone is not a list of services to support, and must not become
one.** Delivery mechanisms are genuinely unalike — one is a URL you post to,
the next wants service credentials and a payload envelope, the next a device
token and a key-signed request, the next is a program run on the local machine.
An interface shaped around whichever gets written first quietly excludes the
rest. So the target to design against is the one nobody has thought of yet, and
any service named anywhere in this repo is an illustration rather than a
commitment.
*In use here:* ntfy — Apache-2.0, self-hostable, needs no account. Being the
one we run buys it no standing in the interface.
*Done when:* an escalation reaches a human who was not watching, through an
adapter the engine knows nothing about.

**B4 — The audit report.**
B2 answers what is blocked on a person; this answers what *happened* (owner,
2026-08-21). A loop may let its agents finish at a resolved state and leave
the final close to a human, in which case the merge is the audit point:
whoever reviews it needs to see which records were resolved and by which path
— including the ones that escalated and were released — without opening a
database console. Informational rather than blocking, and a reader of the same
enumeration B2 uses, so the two views cannot disagree about the ledger.
*Done when:* a person reviews a real merge from the report alone, and closes
records from it.

**B5 — First production loop.**
The author's existing hand-driven worker/reviewer lane moves onto the engine:
the same actors (agent sessions and a human at the console), the same
ceilings, the same escalation — refereed instead of remembered. Timing is the
owner's call; the engine earns the migration rather than demanding it.
*Done when:* a real change ships through a FerroStep-refereed loop with a
ceiling spent and an escalation exercised for real, not in a fixture.

**B6 — Defense in depth: compile the rules into the database.**
The engine is consulted, not in the write path — by design. This milestone
emits database-side enforcement (API rules, constraints) from the same
`WorkflowDef` the engine validates, so definition and enforcement cannot
drift apart.
*Done when:* an illegal transition is blocked by the database itself with the
engine bypassed entirely.

**B7 — First shipped skill.**
The first entry in `skills/` lands with its first real consumer — the actor
skill B5's worker loads, or the one that narrates B2's decision surface to a
human, whichever arrives first. The skills distribution channel is decided
then, with that consumer in hand and not before.

---

## Expansion — demand-gated, in whatever order demand arrives

**E1 — The GitHub surface.** `ferrostep-github` grows along its own phased
plan ([`github-agents-roadmap.md`](github-agents-roadmap.md)): push-as-App,
verified attribution, then GitHub-side agents — expected first case, a
reviewing persona in the PR process.

**E2 — Further ledger backends** (Postgres first) when a real loop needs one.
The baseline pair already proves the interface is not shaped around a single
store, so a third is demand-gated like everything else in this tier.

A ledger built for this shape of work — agent actors, a human peer, an
append-only history — is a plausible sibling project rather than part of this
one (owner, 2026-08-21). If one arrives it is **an adapter like any other and
never the assumed deployment**: requirement 8 in [prior-art](prior-art.md) is a
library plus the database you already run, and that stops being true of us the
moment a server of ours is the default path. Optional and self-hosted keeps it
clear of the non-goals below, which rule out running anything *for* a user, not
shipping something they can run.

⚠ **It carries a risk the external backends do not.** PocketBase and SQLite
keep the interface honest precisely because we cannot change them — every
awkwardness has to be absorbed on our side. A backend the same hands control
can have the interface bent toward it instead, one convenience at a time, and
nothing fails while that happens. A first-party ledger earns its adapter
against the same interface as the others, with no privileges they lack; and
because it would be built to serve this engine, the adapter interface is the
requirements document it should be tracking.

**E3 — TypeScript bindings** when a TypeScript consumer exists to drive the
API. The workspace has left room since day one.

**E4 — Full dog-food.** This repo regains a reviewing persona — refereed by
the engine it builds. The current "no review lane" state is ended by the
product becoming able to end it, not by process arriving early.

---

## Horizon — the ambitions that order the road

- **One audit surface for an organization's loops.** Humans and agents as
  peers on the same ledger, every delegation bounded by roles, ceilings, and
  an escalation path — the reason a human can hand agents real work and walk
  away.
- **Purpose-driven review lanes as ordinary configuration.** Session reviews,
  full reviews, product-alignment reviews: the same primitives at different
  cadences, briefed from the `purpose` their definitions carry.
- **The referee as commons.** Apache-2.0, no platform, no capture: useful to
  others precisely because it was built for one operator and shared legibly.

## Non-goals — permanent

No runtime, scheduler, queue, or hosted anything. No blessed workflows. No
feature without a consuming loop. No competing with actor-layer frameworks —
agents built on them are actors *in* FerroStep loops, not rivals to it. And
no vendor's agent tooling gets framework-level support (owner, 2026-08-21):
it is reached through an agent adapter or it is not reached at all.

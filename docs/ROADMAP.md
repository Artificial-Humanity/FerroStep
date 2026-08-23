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
papered over. Both ship as maintained defaults, and doubling as the worked
example for a third is part of the job — an adapter nobody could imitate has
only half solved the problem.

**An adapter states what it cannot guarantee, not only how it achieves what it
can** (owner, 2026-08-21). Immutable history is the case in point: it is a
per-store property rather than an interface-wide promise, and an adapter able
to offer it only by convention says so, rather than letting the audit report
imply more than the store delivers.

⚠ **Agents authenticate as role-scoped accounts, never as store administrators**
(owner, 2026-08-21). An administrator credential bypasses a store's own access
rules, which makes every enforcement story collapse into a promise: the actor
can edit the history it just wrote, and the records besides — so guarding the
log against it was never meaningful in the first place. With an account per
role, the rules apply, history is append-only without special machinery, and
the role the engine gates on is the role the store authenticates. That last
equivalence is what makes B6 possible at all.

⚠ **Measured since (2026-08-21): a store's rule layer and its extension layer
answer this differently, and the ruling above is now depth rather than the only
defence.** On the first backend, an administrator bypasses access rules and does
*not* bypass server-side extension code — the same wrongful claim was refused
from both an ordinary and an administrator credential. So enforcement that
survives an administrator is reachable before the identity model changes. This
moves *when* the identity work is needed, never *whether*: an adapter that
depends on one layer alone should say which, and B6 is where the second lands.

⚠ **The interface is defined over records as objects, never rows** (owner,
2026-08-21). A snapshot is a state and a set of counters; an event is a value.
Serialization, and whatever shape the store wants it in, belongs to the
adapter, and nothing above the adapter may assume tables, columns, a query
language or a schema at all. Relational, document and embedded key-value stores
are all in range — an interface that quietly assumes the first cannot reach the
third, and the two written first are both relational, which is exactly the way
to acquire that assumption without noticing. The operation this bites hardest
is enumeration: a store with no query language can only answer "which records
await someone" from an index the adapter maintains itself, and that cost is the
adapter's to carry rather than the caller's to know about.

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
decision. It belongs in the event log rather than in a decisions store beside
it, which would be a second chronology of the same record, free to disagree
with the first. A *comment* is discussion that moves nothing and stays its own
thing; a *decision* is a move with a reason attached, and the log is where
moves already live.
*Done when:* the reference review loop runs end-to-end on both, with a
version-guarded write proving the crash-accounting promise survives contact
with a real database. ✅ **The guarantee that done-when depends on is measured
and present on the first backend** (2026-08-21) — a compare performed inside
the store's own transaction held over 43 rounds at up to sixteen concurrent
writers, including for administrator credentials. The milestone is now an
implementation rather than an open question, and the adapter's write path is a
generated server-side handler rather than a sequence of REST calls.

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
rest, so the target to design against is the one nobody has thought of yet.

*The default:* an **ntfy** adapter ships and is maintained — Apache-2.0,
self-hostable, no account needed. A stack has to actually function, and a
default is how it does. What a default earns is the job of being the worked
example somebody copies when they write the fourth one; what it does not earn
is any standing in the interface.
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
⚠ **A loop being migrated already has enforcement somewhere, and moving it
naively leaves two referees.** Where a store-side hook performs a transition
today — releasing a paused record and clearing its counter in one save, say —
and the loop then becomes a definition, that transition is enforced in two
places free to disagree, with nothing comparing them. Same disease as a second
chronology beside the log. Such a hook has to become the compiled output of the
definition (see B6) or be retired into it; it must not survive as a
hand-written peer. Worth deciding before a cutover rather than during one.
*Done when:* a real change ships through a FerroStep-refereed loop with a
ceiling spent and an escalation exercised for real, not in a fixture.

**B6 — Defense in depth: compile the rules into the database.**
The engine is consulted, not in the write path — by design. This milestone
emits store-side enforcement from the same `WorkflowDef` the engine validates,
so definition and enforcement cannot drift apart. What that enforcement *is*
varies by store and is not always its access rules — a hook, a constraint, a
trigger, a rule expression. ⚠ Some stores can enforce nothing at all, and for
those the engine is the only gate; that is a fact to state plainly in the
adapter rather than a milestone to fake.
*Done when:* an illegal transition is blocked by the store itself with the
engine bypassed entirely, on a store capable of it.

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

**E5 — Inter-agent messaging.** A public-release item (owner, 2026-08-23):
not necessary for in-house dogfooding, so it never gates the baseline and is
demand-gated like the rest of this tier. Deliberately discrete from B3 — the
drafted shape, and why the two items must not blur, is
[`notes/agent-messaging-and-presence.md`](../notes/agent-messaging-and-presence.md),
which graduates to `docs/` with the item. Presence — the identity-to-address
claim the messaging routes over — has an in-house consumer of its own and is
sequenced independently of it.

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

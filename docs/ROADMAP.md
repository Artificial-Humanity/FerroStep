# FerroStep — roadmap

The ordering of intent. [`north-star.md`](north-star.md) is *why*, the
[README](../README.md) is *what*, this is *in what order and why that order*.
It sequences; it does not re-legislate — the standing rules live in
[AGENTS.md](../AGENTS.md) and the north star, and every milestone below
inherits them.

**Where it stands (2026-08-25):** the baseline through B5 is built, tested
and running — the referee core, both ledger adapters with their measured
batteries, the decision surface and its resolving move, the notification
boundary with its default, and the audit report. The author's lane has
finished its migration: the workflow machinery it ran on before is retired
rather than running alongside, its store carries the referee (a mapped
collection with generated transactional routes), and real records have run
the full refereed cycle — claim, escalation, owner release, close —
including a refereed move between units of work. 0.1.0 is cut on that basis
and nothing is published to a registry yet.

⚠ **What that migration cost the adopter is the tier's most valuable
output, and it is written down:**
[`notes/adoption-friction.md`](../notes/adoption-friction.md) is the
evidence file the expansion tier is meant to be argued from, so "a real loop
needs this" stops being a claim made from memory. Where an entry there
produced a change, the entry names it — which is how to tell what the
migration actually bought without counting anything here.

## Releases — the named points on the road

The versions below are defined by outcome, never by date (owner,
2026-08-23). [`CHANGELOG.md`](../CHANGELOG.md) records what each release
carried once it is cut.

**0.1.0 — the internal MVP.** Cut when the author judges the engine
deployable as the replacement for the hand-driven worker/reviewer lane it
generalizes — the cutover B5 describes. Which baseline items that takes,
and how much of each, is judged then against the lane's real needs rather
than enumerated now. Deployability to that lane is the bar; registry
publication is its own decision and not implied by it.

**1.0.0 — the planned road, complete.** The goal release: the baseline and
expansion tiers done. An item admitted to the roadmap after this ruling
states at admission whether it sits inside 1.0.0 or beyond it.

### The ladder between them — 0.1.0 increments (owner, 2026-08-25)

Milestones climb in tenths, and each names an **outcome** rather than a
date. Two of the rungs below are the owner's own placement; the rest are
this file's proposal and are moved freely.

| release | outcome | items |
|---|---|---|
| **0.1.0** ✅ | the internal MVP — the author's lane runs on the referee | B1–B5 |
| **0.2.0** | **the store enforces, not just the referee** — actors stop authenticating as an administrator, and the rules the engine validates are also applied by the store | B6 |
| **0.3.0** | **a stranger can adopt it from the documentation alone** *(owner)* | B8, B9 |
| **0.4.0** | *unassigned* — see the note below | — |
| **0.5.0** | **the zero-install path gets a console** *(owner)* | E6 |
| **0.6.0 →** | the remaining expansion tier, in whatever order demand arrives | E1–E5, B7 |
| **1.0.0** | the planned road complete | — |

⚠ **B9 on 0.3.0 is this file's proposal on an owner-placed rung**, and is
the one addition above that is not free to move without the owner. The
argument for the pairing is that the rung's outcome is unreachable without
it: a stranger who adopts from the documentation alone is following an
install procedure, and documenting a hand procedure leaves them holding it.
Move it if the outcome reads differently from here.

⚠ **This ladder places items; it does not gate them.** An expansion item
whose demand arrives early lands early and moves the ladder rather than
waiting for its rung. What a rung fixes is the *outcome that names the
release*, so a version number means something specific rather than "the
work that happened to finish."

**On 0.4.0, deliberately empty.** The leading candidate is **registry
publication** — after 0.3.0 a stranger can read how to adopt it, and being
unable to install it without cloning is the next thing in their way. That
is the owner's decision and is not implied by any release here (nor was it
by 0.1.0), so the rung stays open until they make it. B7 is the other
candidate and cannot be scheduled: it lands with its first real consumer.

"Public-release item" on an entry names a polish bar, never a testing
venue — everything here, the expansion tier included, is exercised in-house
before it is anyone else's.

Between cuts, the workspace version in `Cargo.toml` carries the next number
with a pre-release marker, so the tree never claims a release that has not
happened — only a tagged commit carries a released version.

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
✅ **Met 2026-08-24.** A real record ran claim, spend, escalation, owner
release and close through the referee, in the ledger's own history. The
two-referees warning above was answered the way it asks for rather than the
way that is easier: the store-side hook that performed a transition became
generated output of the definition, and the loop's prior workflow machinery
was retired in the same migration instead of being left to agree with the
definition by luck.

**B6 — Defense in depth: compile the rules into the database.** *(0.2.0)*
The engine is consulted, not in the write path — by design. This milestone
emits store-side enforcement from the same `WorkflowDef` the engine validates,
so definition and enforcement cannot drift apart. What that enforcement *is*
varies by store and is not always its access rules — a hook, a constraint, a
trigger, a rule expression. ⚠ Some stores can enforce nothing at all, and for
those the engine is the only gate; that is a fact to state plainly in the
adapter rather than a milestone to fake.
Two things already known to be wrong belong to this milestone rather than
waiting for it:

⚠⚠ **A generated history collection must never be more readable than the
records it describes.** The generated migration hardcodes an
authenticated-user read rule on the events collection it creates. For a
deployment using the adapter's own collections that matches; for a **mapped**
one it does not, because there the refereed records are a collection the
adopter already had, under whatever rules it already has — commonly stricter.
The result is an inversion delivered by default and silently: every state
change, actor, role and human note about records nobody may read, in a
collection anybody authenticated may. **The generated rules should be derived
from the records collection rather than assumed**, since the whole point is
that the history is no more visible than its subject.

✅ **Role-scoped actors — the mechanism landed 2026-08-25.** The write routes
took `role` from the request body, so any authenticated caller could act as
any role. It now comes from the authenticated principal and a contradicting
claim is refused. ⚠ **Bind, don't mint** (prior-art §requirement 9): the
binding names an auth collection the deployment already has and reads one
field on it; it creates no identities, because owning accounts would mean
enumerating actors at design time and that is the assumption which fails
first. What remains for this milestone is the *deployment* half — creating
actors, granting them read on the records collection, and flipping
`allow_unbound` to `false`, after which a principal with no role cannot move a
record even holding administrator credentials.

⚠ **A mapped deployment must widen its own records collection**, and this
project must not do it for them: actors need read on a collection the adopter
owns, under rules only they can weigh. The generic shape creates that
collection and so can grant it; the mapped shape says so and stops. Same line
as the read-rule fix above, in the other direction.

*Done when:* an illegal transition is blocked by the store itself with the
engine bypassed entirely, on a store capable of it — and no generated
artifact grants a read the collection it describes would refuse.

**B7 — First shipped skill.**
The first entry in `skills/` lands with its first real consumer — the actor
skill B5's worker loads, or the one that narrates B2's decision surface to a
human, whichever arrives first. The skills distribution channel is decided
then, with that consumer in hand and not before.

**B8 — Adopter documentation.** *(0.3.0)*
The baseline is two sentences and the second one — *a stranger can read this
repo and run their own loop on their own database by imitation* — has never
had an owner. The README now describes the whole product, which was the
blocker; what it cannot be is the walkthrough.

⚠ **Write it from the friction file, not from the code.**
[`notes/adoption-friction.md`](../notes/adoption-friction.md) is a record of
what a real migration actually cost, and two of its entries are this document
in negative: **authoring a definition** (entry 6) is the first thing an
adopter does and the step with the least support and the quietest failure —
a definition that permits *less* than the loop it models reads like tidying —
and **the roster bootstrap** (entry 9) lands on every adopter exactly once,
at their first landing, when the tool that assigns identity is not yet
installed. A guide written from the code would cover neither, because from
the code neither looks like a step.

⚠ **What is knowably missing must be in it**: there is no way to file a first
record other than the CLI or your own program, no lint that compares a
definition against the loop it claims to describe, and actors authenticate as
one credential until B6 lands. An adopter finding those out by hitting them
is the same friction over again.
*Done when:* somebody outside this project stands a loop up from the
documentation, and the places they get stuck become entries rather than
surprises.

**B9 — Install and update as an operation, not a procedure.** *(proposed:
0.3.0, with B8)*
The generator stops at the file. Everything after it belongs to a human with
a shell: where each artifact goes, in what order relative to the other one,
whether the thing now running is the thing that was generated, and how to get
back. That is a *procedure*, it is carried in somebody's head or in a note,
and B8 cannot fix it — documenting an unsafe sequence documents an unsafe
sequence. Both items serve the same sentence, which is why the pairing is
proposed rather than a later rung.

⚠ **This is the generator/installer split, not a backend complaint.** Two of
the constraints below are the store's and are not going anywhere: on this
backend a hooks write *is* the restart, which couples artifact placement to
service lifecycle, and migrations are files applied in filename order that
the store also writes itself, so a generated migration competes for position
with machine-authored ones. Everything else is ours, and would reappear on
Postgres (E2) or on any backend where this project emits artifacts into a
runtime it does not own. The zero-install path and a native store escape it
by having no extension layer to install into — which is an absence, not a
solution to copy.

What the two migrations done to date actually cost:

- ⚠⚠ **The migration's filename decides whether it runs, and a migration
  that did not run looks exactly like one that changed nothing.** Measured
  2026-08-29: an idempotency re-run sorted below the instance's highwater
  migration and never executed, and it became evidence only once a marker
  file in the same restart proved new files execute at all. **A no-op and a
  no-run are the same diff.**
- ⚠⚠ **`install_files` hardcodes `1756000000_ferrostep.js`, and that prefix
  resolves to 2025-08-24 — a year before this repository's first commit.** On
  a fresh store it sorts first, which is presumably the intent. On any store
  already carrying migrations of its own it sorts below all of them, and no
  adopter's store can have been migrated by this project before this project
  existed. Whether the store still applies a file below its highwater mark is
  **unverified for this path** — the measurement above covered a mapped
  hand-install — and establishing it either way is the first thing this
  milestone measures, because the two outcomes are a working installer and a
  silent one.
- **The mapped shape has no installer at all.** Both adopters use it;
  `emit-mapped` writes to two paths the caller names, and placement is a
  written instruction. The one installer that exists serves the shape neither
  of them chose.
- **Nothing compares what is about to be installed with what is running.**
  Both adopters staged copies under `.ferrostep/` by hand and diffed by eye,
  and one of those staged copies then had to be cleaned up by the owner
  because no step owned removing it.
- **The way back is destructive.** Measured: `migrate down 1` deletes the
  events collection and every row in it, deletes the actor collection and
  removes the version field. That is a removal, not a rollback, and an
  adopter reaching for it mid-update loses the history the referee exists to
  keep.

What it needs, stated as outcomes because the shape is not decided:

- One command that installs or updates, and **refuses rather than
  half-applies** — the ordering constraint between the two artifacts is the
  store's, so the command owns it instead of the reader.
- A migration name derived from the target instance rather than a constant.
- A pre-flight that answers *what is running, and what would change* before
  anything is written.
- A verification the command runs itself. A checklist a human works through
  is the artifact this milestone is replacing.
- ⚠ A non-destructive way back, **or a plain statement that there is not
  one** — the same rule the adapters follow about what they cannot guarantee.

✅ **First piece landed 2026-08-29:** the emitter now prints what the file it
wrote refuses and what it leaves open, and `guard_refereed_fields` gained a
third value so *absent* stops reading as *stated false*. That is the
pre-flight's first half — what the artifact *would* do — with the other half,
what the running store *does*, still unanswered by anything but `doctor`.

*Done when:* an adopter installs a deployment and later updates it with one
command each, and the command tells them what will change before it changes
it.

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

**E5 — Inter-agent messaging.** A public-release item (owner, 2026-08-23) —
a polish level, not a venue: it is tested in-house like everything else, but
its finish belongs to the 1.0.0 line, so it never gates the internal MVP.
Deliberately discrete from B3 — the
drafted shape, and why the two items must not blur, is
[`notes/agent-messaging-and-presence.md`](../notes/agent-messaging-and-presence.md),
which graduates to `docs/` with the item. Presence — the identity-to-address
claim the messaging routes over — has an in-house consumer of its own and is
sequenced independently of it.

**E6 — The SQLite console.** *(0.5.0)*
A browsable, editable view of the records in a SQLite ledger — what a store
with an admin UI gives you for free, for the store that has none.

**Provenance, recorded because it was lost.** ⚠ **The owner proposed this at
the project's outset** — SQLite holding the agents' issues, decisions
surfaced in chat, and a simple console to follow along — and **it was never
written down**, in this file or anywhere else. It surfaced again on
2026-08-25 only because the owner said so from memory. Two of the three parts
had been built anyway (the adapter; B2's rendered decision surface, which is
also what an agent narrates from). The console was not, and the reason is
worth keeping: the idea was absorbed into a requirement *on the store*
([`notes/ledger-requirements-and-pocketbase.md`](../notes/ledger-requirements-and-pocketbase.md),
requirement 8 — *"the requirement a purpose-built replacement is most likely
to drop and most likely to regret dropping"*), which PocketBase satisfies and
SQLite cannot. From there SQLite's lack of a console was reframed as a
**virtue** — it is what stops the interface hiding behind a store's own UI,
and that argument is good and stays. But a suggestion had been answered with
a reason it was fine not to do, and nothing recorded that it had been.

**Scope — small on purpose** (owner, 2026-08-25): **it need not be as robust
or as configurable as a mature store's admin UI.** Requirement 8's bar
describes what such a console gives you for free; this is not held to it, and
recording that now is what keeps it from growing toward one. No theming, no
custom views, no configuration surface of its own.

Concretely: list the records in a scope with their state and counters, open
one to see its fields and its history, and take the moves the definition
allows. Not a live tail — `awaiting` and `audit` already answer *what is
happening*.

⚠ **It goes through the ledger interface, never the database file.** A console
writing rows directly is a second writer with none of the referee's
bookkeeping, which is the thing rescope exists to have ended. That has a
consequence better stated than discovered: **arbitrary field editing is
therefore not on offer**, because the interface has no such operation.
Requirement 8's *"editable view of every row"* is answered by a store's own
console and deliberately not by this one — the escape hatch for a SQLite loop
stays a SQL client, used knowingly.

**One question, cheap to defer to the item:** built against `Ledger` this
works for every adapter at no extra cost, and only the *need* is
SQLite-specific. The trait is the leaning.

*Done when:* a person inspects a SQLite-backed loop and resolves what it is
waiting on, without reaching for a SQL client.

**E7 — Worktrees.** *(owner, 2026-08-29 — raised, not yet placed)*
The owner asked for this to be on the road: **worktrees, addressed at some
point in the product's evolution.** That is the whole of what was said, and
this entry states no more than it.

⚠ **The scope is not stated, and this file must not supply one.** A reading
written here by anybody other than the owner becomes the shortlist the item is
later built against, and it becomes it silently — the entry reads as a record
of their intent whoever actually wrote the words. What the term covers, and
which part of the product it lands in, is theirs to say.

⚠ **Recorded now for E6's reason, one item up.** The owner proposed that
console at the project's outset, nothing wrote it down, and it surfaced again
on 2026-08-25 only because they said so from memory. **An owner proposal
answered with silence is one that comes back from memory or does not come
back.** The bar for an entry here is not a designed item. It is that somebody
said it, and that the entry is dated and attributed.

**One fact worth having before the scoping conversation, and it narrows
nothing.** ⚠ **Scope already models a branch** — the README defines it as
*which unit of work a record belongs to: a branch, a cycle, a tenant*, with
`rescope` as its refereed move and per-label grants in the definition. So one
reading of this item lands on machinery that exists, and a reading about
*parallel checkouts of the same branch* — two actors at work in one unit,
isolated from each other — does not obviously land there at all. Which of
those is meant changes whether the item is configuration or a new category,
and that is the first question to answer rather than the first thing to
assume.

*Admitted when:* the owner states the scope, and states whether it sits inside
1.0.0 or beyond — the placement every item admitted after 2026-08-25 carries
(§Releases). Until then this is a recorded intent and deliberately not a
milestone, which is why it takes no rung on the ladder.

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
competing with actor-layer frameworks — agents built on them are actors *in*
FerroStep loops, not rivals to it. And no vendor's agent tooling gets
framework-level support (owner, 2026-08-21): it is reached through an agent
adapter or it is not reached at all.

⚠ **"No feature without a consuming loop" was here and was removed by the
owner on 2026-08-25. Do not restore it.** It had stood as a permanent
non-goal, entered unattributed, and no ruling behind it was ever recorded;
asked directly, the owner did not hold the position. **Features are judged
case by case, with no standing bar.** Per-item deferrals below still stand on
their own reasons — they are judgments about those items, not instances of a
rule.

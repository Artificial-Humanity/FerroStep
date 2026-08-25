# Prior art, and the gap FerroStep fills

Plenty of good tools live near this space. None of them referee the loop below —
which is not a hypothetical, but the workload FerroStep was extracted from.
Use it as the test: when evaluating any orchestration tool, place this loop in
it and see what breaks.

## The test loop

A **worker** agent and a **reviewer** agent iterate on a piece of work, with a
human **operator** above them. The rules:

1. The actors are **independent processes** — separate agent sessions, cron
   jobs, a human at a keyboard. There is no single program that *is* the loop,
   and no shared framework runtime you could impose on all of them.
2. The **human is a first-class actor**, and acts by editing the database
   directly — approving, re-arming a counter, pulling a record out of
   escalation. No orchestrator API in between.
3. Transitions are **role-gated**: the worker may never close work; only the
   reviewer resolves what it verified.
4. The loop has a **ceiling priced against crashes**: claiming a pass spends
   it, so a worker that dies mid-pass has still consumed one. A crash loop must
   never become an infinite loop.
5. Hitting the ceiling is **routing, not an error**: the record goes to the
   human. Automation stopping is a defined state of the system.
6. The whole state of the system is **legible in a database browser**, and an
   operator hand-editing it is legitimate, not corruption.
7. The same rulebook must be callable from **more than one language**, because
   the actors are written in whatever they're written in.
8. Deployment is **a library plus the database you already run**. No new
   server.
9. **The set of actors is not knowable when the loop is designed.** New ones
   arrive that nobody foresaw — a different vendor's runtime, a colleague's
   script, an agent an end user stood up this morning — and each needs an
   identity the loop can gate on *without the loop being rewritten*. This is
   the same shape as the adapter rule the project already holds for message
   transports: the target to design against is the one nobody has thought of
   yet. It is listed last because it was learned last, and it is the
   requirement the managed platforms have moved on hardest.

## Placing the existing tools against it

**Agent runtimes — PydanticAI, smolagents, Claude Code, …**
These are the *actors*, not the referee. They define how one agent calls a
model, validates tool output, executes code. They have no concept of the
cross-actor loop, and they shouldn't — FerroStep is deliberately
runtime-agnostic so that a PydanticAI app, a CLI agent session, and a human
can all be actors in the same loop. Complementary, not competing.

**Graph agent frameworks — LangGraph and kin.**
The state machine exists, but it lives inside a runtime that must host every
actor; requirement 1 fails immediately. State persists as framework
checkpoints rather than plain relational rows (6 suffers), human-in-the-loop
is an API interaction with the framework rather than an edit to shared truth
(2), and roles are not a first-class concept (3).

**Durable execution — Temporal, DBOS Transact, Restate.**
The strongest neighbors, and the most instructive contrast. Durable execution
answers: *"my **program** crashed — resume it from the last completed step."*
It checkpoints one program's control flow. The test loop has no such program:
the worker and reviewer start independently and the human never "executes" at
all, so there is nothing to checkpoint (1, 2). Execution state is
correct-but-opaque history, not a hand-editable ledger (6). And the loop's
hard problem isn't resuming a crashed step — it's *pricing* the crash
(4) and deciding who may do what (3), which durable execution doesn't model.
Temporal additionally runs as a self-hosted service cluster (8). DBOS gets
deployment right (a library over Postgres) and is well worth studying for
adapter ergonomics — but per language it is a separate implementation of
"resume my program," not a shared rulebook over a shared record (7).

**In-process state machines — Apache Burr (incubating).**
Closest in shape: explicit states, transitions, pluggable persistence, a
telemetry UI. But it describes itself precisely: an *in-process* Python
framework. The application that instantiates the state machine owns the loop;
persistence exists so *that application* can resume and be observed.
Multi-process actors without a shared runtime (1), the human as a peer actor
(2), and role-gated authorization (3) are outside its model, and it is
Python-only (7). Worth reading for API taste in expressing transitions.

**Dataflow orchestrators — Prefect, Hamilton, Airflow.**
DAGs of data transformations. A judgment loop that cycles an unbounded number
of times and then parks indefinitely on a human is against the grain of the
model — cycles, long waits, and per-record authorization all fight the
abstraction.

## The managed agent platforms, and where identity landed (2026)

The three large platforms moved fastest on requirement 9, and an honest
reading grants them ground. Surveyed 2026-08-25; this section is the one most
likely to age, so check it before quoting it.

**LangGraph Platform.** Custom authentication through an `@auth.authenticate`
handler, with `@auth.on` handlers giving resource-level authorization, RBAC,
and metadata filtering on list and read. Two things to be precise about: it
authorizes **its own resources** — threads, assistants, crons — so "may this
role move this work item to that state" remains the graph's business and not
the platform's; and it is a **platform** capability, reached by deploying on
LangGraph Platform rather than by using the library.

**Amazon Bedrock AgentCore** (GA October 2025). AgentCore Identity handles
non-human identities across SigV4, OAuth 2.0 and API keys. ⚠ **Its temporal
policies are the closest thing to this project's core that exists in
production**: stateful rules evaluating authorization from an agent's session
history, expressly to enforce workflow sequencing, cap financial exposure, and
require human approval for high-value actions. Transitions, ceilings and
escalation, in someone else's product. Their human-in-the-loop pauses agent
execution pending an asynchronous approval.

**Microsoft Foundry, with Entra Agent ID.** The strongest identity position of
the three: every agent gets a first-class directory identity under the same
management as human users. At Build 2026 this extended to "autopilot" agents
carrying an email address, a presence in Teams, and a place in the org chart,
governed and audited through Agent 365.

### What they settled, and what it costs

They settled requirement 9 the same way, and they are right: **an agent is a
principal in a directory you already run.** Nobody should adopt a referee that
cannot sit behind Entra, Auth0, or an OIDC provider they already operate.

What it costs is the rest of the list. All three scope authorization to a
**session or an agent invocation**; the test loop's unit is a **record with a
durable lifetime** that outlives every session and is edited by a human at a
console. Their human approval is a pause in a running program — something has
to be alive to be paused. Escalation here is a *state a record sits in*, which
needs nothing running to hold, survives every process dying, and is visible to
anyone who opens the database. That is requirements 1, 2 and 6 again, arriving
from a new direction.

### Bind, don't mint

The identity question splits into two answers, and only one of them is ours.

**Minting** is owning an account store: the tool issues actor credentials, and
an actor exists because the tool says so. Entra and AgentCore mint, correctly
— they *are* directories, or are hosted runtimes fronting one.

**Binding** is owning no accounts at all. The store authenticates whoever it
authenticates; the referee looks up exactly one fact about that principal —
**which role it may act in** — and refuses any request claiming a different
one. Authentication is somebody else's job, permanently.

⚠ **Requirement 9 is why this is not a preference.** Minting requires
enumerating your actors at design time, which is the assumption the
requirement says will be false. Under binding, an actor nobody foresaw is a
new principal in a directory that already exists, plus one row saying which
role it plays — no code, no release, no schema change here.

Two consequences worth stating, because they are what makes this different
from the platforms rather than a smaller version of them:

- **The role is data in the definition; the binding is data in the store.** So
  "who may move this record" has one answer whether the engine is asked, the
  store is asked, or a person reads the configuration. LangGraph's `@auth.on`
  handlers are *code you maintain per resource* — expressive, and a second
  place for the answer to live. Nobody else can collapse the two, because
  nobody else has a definition to be the single source.
- **It composes with all three rather than competing.** An agent with an Entra
  Agent ID, or an AgentCore workload identity, authenticates to your store as
  itself; the referee binds that principal to a role and gates the transition.
  Being uninterested in how the principal was authenticated is exactly what
  lets it sit downstream of any of them.

⚠ **Where this project is behind, plainly.** There is no OIDC or OAuth story
here yet; binding assumes the store authenticated somebody, and for a store
that cannot authenticate at all the engine is the only gate — a fact the
adapter has to state rather than a milestone to fake. Until B6 lands, actors
authenticate as a store administrator and the roster is *attribution, not
authentication*. On requirement 9 the platforms are ahead today.

## The gap, stated

Every tool above wants to **be the loop** — to host the code, own the
execution, or schedule the steps. The missing piece is small and nobody ships
it: given a shared, human-legible database record and a set of independent
actors with roles, decide **which transitions are legal and what they cost**,
with crash-priced ceilings and escalation as routing — as a pure library,
embeddable in any language, over any database, owning nothing.

A referee, not a runtime. That inversion — the ledger owns the loop, and
every actor including the human is just a client of the truth — is the whole
project. It is also precisely the property none of the above can retrofit,
because each is architecturally committed to owning the execution.

| Requirement | Agent runtimes | Graph frameworks | Durable execution | Burr | Dataflow | Managed platforms | FerroStep |
|---|---|---|---|---|---|---|---|
| 1. Independent-process actors, no shared runtime | — | ✗ | ✗ | ✗ | ✗ | ✗ | ✓ |
| 2. Human acts by editing the DB directly | — | ◐ api | ◐ signals | ✗ | ✗ | ✗ | ✓ |
| 3. Role-gated transitions | ✗ | ✗ | ✗ | ✗ | ✗ | ◐ policies | ✓ |
| 4. Crash-priced loop ceilings | ✗ | ✗ | ◐ retries | ✗ | ◐ retries | ◐ per-session | ✓ |
| 5. Escalation as routing | ✗ | ◐ | ✗ | ◐ | ✗ | ◐ pause | ✓ |
| 6. State legible/editable in a DB browser | — | ✗ | ✗ | ◐ | ✗ | ✗ | ✓ |
| 7. One rulebook, many languages | — | ✗ | ◐ per-lang | ✗ | ✗ | ✗ | ✓ |
| 8. Library + your existing DB, no new server | ✓ | ◐ | ◐ DBOS ✓ / Temporal ✗ | ✓ | ◐ | ✗ | ✓ |
| 9. Unforeseen actors, identity from your directory | — | ✗ | ✗ | ✗ | ✗ | **✓** | ◐ B6 |

◐ = partially, or through the tool's own runtime/API rather than the ledger.
"—" = out of scope for that layer. The columns are categories; individual
tools vary — see prose above for the honest per-tool nuance.

⚠ **Row 9 is the one this project does not win, and the table is more useful
for having a row like that in it.** A comparison where the author sweeps every
line is a comparison whose requirements were chosen after the answers. Row 9
was added because the platforms made it obvious, not because it was foreseen —
which is itself the requirement demonstrating itself.

## When to buy instead

FerroStep is the right tool only inside its gap. Reach for something else when:

- **One program needs to survive crashes** (a pipeline, a saga, a backend
  job): use **DBOS Transact** (library, Postgres) or **Temporal** (if you
  already run its cluster). That is durable execution's home turf.
- **One Python application is a conversational state machine** (a chatbot, a
  simulation): **Apache Burr** models exactly that, with a UI.
- **You want a batteries-included TS agent stack**: Mastra.
- **You need an agent runtime**: PydanticAI / smolagents — and use it *as an
  actor* in a FerroStep loop rather than instead of one.
- **You are already inside one cloud and want agents governed like employees**:
  Microsoft Foundry with Entra Agent ID, or Bedrock AgentCore. They give
  directory identity, token brokering and audit that this project does not and
  should not try to reproduce. ⚠ **This is a "use both", not a "use instead"**
  — an agent holding one of their identities is a perfectly good actor in a
  refereed loop, and binding it to a role is the seam where the two meet.

## What FerroStep borrows from each

- From **DBOS**: the deployment bar — `pip install` plus the database you
  already have, nothing else. Also the value of an append-only event log for
  auditability; the ledger-adapter interface includes one.
- From **Burr**: the discipline that transitions are explicit, enumerable,
  and observable; and that a self-hostable view of state is a feature, not an
  afterthought (a database browser gives us this for free).
- From **Temporal**: determinism as a contract. Decisions are pure functions
  of definition + snapshot, replayable and testable by construction.
- From **LangGraph**, negatively: what happens when the framework owns the
  runtime — every actor must live inside it. FerroStep's core rule (pure
  engine, no IO) exists to make that capture structurally impossible.
- From **Entra Agent ID and AgentCore Identity**: that an agent is a principal
  in a directory, not an account a workflow tool invents. It is the settled
  answer to requirement 9 and this project adopts it rather than arguing with
  it — which is what *bind, don't mint* means in one line.
- From **AgentCore's temporal policies**, as confirmation rather than
  borrowing: an independent team reached sequencing, spend caps and mandatory
  human approval from a different starting point. Convergent design is the
  best available evidence that a problem is real. ⚠ It is also the closest
  competitor to the core, and pretending otherwise in this document would make
  the rest of it untrustworthy.

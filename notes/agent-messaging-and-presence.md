# Inter-agent messaging: presence, logical addresses, store-and-nudge

Working notes, not a deliverable. The direction is the owner's (2026-08-23):
an adapter-based routing system; our own take on inter-agent messaging; the
agent harness never assumed, because it has already varied and will again —
possibly to a local model. Every shape below marked *proposal* is the
resident developer's and unratified.

---

## 1. The gap, measured

2026-08-23, in the workspace this repo is developed from: five concurrent
agent sessions, every one addressed as the workspace directory's name plus a
random suffix. Which project each session was resident in — which roster
identity it had adopted — was invisible from the address, so finding "the
session at project X" meant messaging each one and asking. The hand-driven
loop this engine generalizes had already paid for the underlying lesson: the
session is the address, and a greeting line inside a message routes nothing.

Two facts make this FerroStep's problem rather than any harness's:

- A deployment's roster already defines *who* an actor is. What is missing
  is *where*: which live address currently embodies a roster identity.
- The harness is not a constant. An end user's could be anything, including
  a local model — which is the adapter rule doing its normal job, not a new
  principle.

## 2. What already legislates this

Nothing here starts from zero:

- AGENTS.md's adapter convention lists **the agent interface** among the
  things defined internally first, and its transport warning — delivery
  mechanisms genuinely unalike, design for the target nobody has thought of
  — was written about message transports specifically.
- ROADMAP **B3** defines the engine→human message and the boundary sentence
  this note extends to actor→actor: the message is defined here; delivering
  it is somebody else's problem.
- The north star's organizing principle — the ledger is the truth, the
  engine a pure function over it — and its non-goal that no vendor's agent
  tooling gets framework-level support. The proposal below leans on the
  ledger harder than on any transport.

## 3. The things that are ours (proposal)

**Logical address.** A message addresses a roster identity at a deployment —
actor-at-project — never a transport address. The roster names identities;
presence binds them to transports, late, at routing time.

**Presence record.** Written when a session adopts residence: identity,
project, transport tag, transport address, claimed-at. The address is opaque
text — whether it is shaped like a session name, a URL, or a pipe path is
the adapter's business alone. ⚠ A presence record is a *claim*, not a
truth: nothing guarantees a departure record, so staleness is normal, and a
reader that treats presence as authoritative will route work to ghosts. The
design below makes that failure cost a delay, never a loss.

**The message.** From-identity, to-identity, body, sent-at, optional
in-reply-to. As engine-opaque as `purpose`: the engine has no concept of
what a message means.

**Store-and-nudge — the load-bearing proposal.** The canonical act of
sending is *appending the message to the ledger*; a transport delivers only
a nudge — "you have mail" — to the address presence claims. Each
consequence earns its keep:

- A message survives a dead, absent, or mis-claimed recipient: the nudge
  fails, the record waits. Stale presence degrades to latency.
- Messaging inherits the ledger's audit story. In a product whose horizon
  is one audit surface for an organization's loops, agent-to-agent traffic
  outside the ledger would be a second, invisible chronology.
- The zero-install deployment gets messaging for free: on one host over
  SQLite, the store *is* the transport and the nudge adapter can be nothing
  at all — the recipient reads its inbox when it next looks. No vendor, no
  account, no daemon.
- The engine stays out of it entirely: actors write and read records;
  nothing polls, schedules, or listens on the engine's behalf.

**Routing resolution, pure.** Presence snapshot + logical destination +
caller-supplied now → the candidate addresses with their claim ages, or
"unresolvable". The engine's usual shape: data in, explainable decision
out. How stale is too stale, and what to do with two live claims, is
per-deployment configuration, never a constant in code.

## 4. What is rented (adapters)

Transport adapters deliver nudges: a session harness's message bus, a CLI
runtime, a local model server's endpoint, a file drop, a plain URL. Vendor
names live here and nowhere else. B3's warning applies verbatim, and the
standing test is unchanged: could somebody write a simple adapter for a
target nobody here has thought of?

A default ships, holding the worked-example bar and earning no standing in
the interface. The order the consuming loop suggests: the **store-only
null transport** first (it is the zero-install path, and it is also what
every other transport degrades to when its nudge fails), then an adapter
for the harness the author's own sessions actually run in — the one whose
absence §1 measured.

## 5. Relationship to B3

The same delivery problem with different endpoints: B3 is engine→human on
escalation, this is actor→actor. One delivery boundary should serve both —
but nothing pre-builds the unification. Whichever lands first defines the
boundary and the second is the honesty test, exactly B1's two-stores logic.

## 6. The deployment aspect

Presence is written at residence-adoption. For this repo, that would be a
verb beside `agent-env` in the session-start procedure; for a deployment,
it is bootstrap. Disposition lands in the deployment map when something
ships. Sequencing is implied rather than chosen: store-and-nudge consumes
B1's record-as-object store interface and adds no store requirement beyond
another record kind, so this work sits behind B1 wherever it is placed.

## 7. Open for the owner

1. Roadmap placement: a peer of B3 in the baseline, or expansion? The
   consuming loop already exists (§1), which argues baseline.
2. Does presence live in the same store as the workflow ledger (proposal:
   yes — one truth, one reach) or may a deployment split them?
3. Is a message ever something the *engine* reasons about, or purely
   actor-side data in the shared store? (Proposal: actor-side only — the
   engine referees workflows, not conversations.)

# Where we sit beside the agent platforms — a discussion note

**Working notes for discussion. Nothing here is a ruling, a roadmap or an
admission bar.** No item below has been decided, and the file's own opinions
are marked as mine (the resident persona's) so they cannot be mistaken later
for the owner's. ⚠ This matters more than usual here: a comparison against
named products arrives already sounding like a requirements list, and an
unattributed idea shelved beside attributed ones is promoted rather than
merely restated (AGENTS.md §Conventions, and it is in that file because this
repo has already paid for it once).

**Source, dated so it can be argued with:** a third-party model (Gemini) was
asked to compare FerroStep against full-stack agent platforms and execution
harnesses; the owner brought its answer here on **2026-08-27**. It is an
outside reading of a public repo by something that did not build it, which is
exactly what makes it worth reading and exactly why every claim below was
checked against HEAD before being written down.

The platforms it compared us against, by category: type-safe tool runtimes
(PydanticAI, smolagents), full-stack orchestration (Mastra), role/graph
orchestrators (LangGraph, CrewAI), autonomous coding CLIs (Claude Code, agy),
and sovereign agent daemons (Hermes).

---

## 1. What the comparison gets wrong about the current repo

Filing a false premise into the notes is the expensive failure, so these come
first. Each was checked against the tree, not recalled.

### ⚠⚠ "Schema rigidity — adding a step requires updating Rust types, state machines and database migrations"

**Wrong for states, transitions, roles, counters-as-concepts and rescope
grants.** A definition is JSON. Adding a state to `examples/product-review.json`
is adding a string to `"states"` and a row to `"transitions"`; the core never
learns what it means. This is not an accident of the current implementation —
it is the first line of AGENTS.md §Conventions (*workflow definitions are
data; never encode a specific workflow's states as Rust enums in the core*),
and the reference loop lives in `examples/` and tests precisely so the engine
stays ignorant of it.

**Half-right for counters, and only on one adapter — which is the finding
worth keeping.** A counter has to live somewhere in the record:

- `ferrostep-sqlite` stores all counters in **one JSON column**
  (`counters TEXT NOT NULL DEFAULT '{}'`). A new counter needs **no
  migration**.
- `ferrostep-pocketbase` maps **each counter to its own column**
  (`CollectionMap.counter_fields`). A new counter there **is** a schema change
  on the adopter's store.

So the friction the comparison points at is real, is confined to one adapter's
mapping strategy, and is therefore fixable without touching the core or the
ledger contract. That is a much better problem than the one it was reported as.

### "Observability is limited to raw SQL database queries"

**Outdated.** The CLI answers the operational questions directly:
`ferrostep awaiting` renders what is waiting, why it is waiting, the counter
state, and what each role may do next; `audit` walks the history; `explain`
prints what a definition asserts, and with `--map` the refereed columns, the
guard's state and the sweep an adopter has to run before turning it on.

**What is true:** all of it is text. There is no visual surface, and nothing
renders a definition as a graph. Whether that matters is §3.D.

### "Custom polling loops so each agent knows when it has permission to claim a record"

**The reasoning is not yours to build** — `awaiting --role <role>` is exactly
the "may I, and on what" question, answered by the engine against the
definition rather than by an adopter's hand-written predicate.

**What is genuinely missing is push.** Nothing wakes an agent when work
appears. An adopter writes a poll loop — but a `sleep`-and-ask loop over a
good answer, not a claim protocol of their own. The distinction changes the
size of the gap by an order of magnitude and it should not be blurred.

### "Zero model integration", "no subprocess/OS hooks", "no scheduling or ingestion"

**All three accurate, and all three are the design working**, not gaps in it:
the core is pure by rule — no IO, no async, no clock, no database, no network
(AGENTS.md §Layout). An LLM client, a process spawner and a scheduler are
side effects, so under the standing ruling that *everything outside the engine
is an adapter* (owner, 2026-08-21) they are adapter questions by construction.

⚠ **That is the reframe I would put at the centre of any discussion of this
table.** Read column-by-column, "where FerroStep lacks" is mostly *a list of
adapters nobody has written yet*, not a list of engine defects. The useful
question per row is therefore not "should the engine do this" — the answer is
no, every time, by rule — but **"is this an adapter we should ship as a
default, an adapter an adopter writes, or a thing we deliberately do not do?"**
The repo already has the vocabulary for that distinction: shipping a default
adapter is not naming a vendor at framework level, and defaults hold the
`examples/` bar — copied to write the next one, never blessed.

---

## 2. The comparison's real contribution

Stripped of the errors, the outside reading lands one thing hard, and it is
not on the roadmap:

> **Every adopter writes the same runner.**

The engine referees a transition that somebody proposes. Something has to
spawn the headless process, capture its output, decide what the result means
and propose the transition. The first migrating loop wrote that by hand. Any
second adopter writes it again, differently, and the two will disagree about
the parts that are actually general — retries, timeouts, what a crash means,
whether a non-zero exit is a refusal or a failure.

⚠ It is the same shape as the counters/columns finding: a real gap that is
**not** a core gap. And it is the one item here I would argue is
underweighted rather than overweighted by the comparison, because it is
invisible from inside a single adopter — the first loop's runner looks like
that loop's business until there is a second one.

---

## 3. Candidates, with what each one costs

My read, offered as argument rather than proposal. None is decided.

**A. A runner interface (not a runner).** Define what a "work attempt" is —
launch, capture, outcome, timeout, crash — as an interface in the ledger's
spirit: shaped around what the thing *is*, not around how one coding CLI
happens to deliver it. ⚠ The standing test applies and it is a hard one here:
could somebody write a simple adapter for a target nobody here has thought of?
A runner interface modelled on one headless CLI's flags fails that test in a
way that is only visible once you are in the corner.
*Cost:* design-heavy, low code. *Risk:* getting the shape wrong is expensive
and hard to reverse — this is the interface most likely to be modelled on
whatever we happen to use.

**B. Counters as a mapped blob on the PocketBase adapter**, matching SQLite.
Removes the migration-per-counter friction at its actual source.
*Cost:* small and local. *Risk — and I got this wrong on the first pass, so it
is worth stating precisely:* the intuitive objection is that a JSON blob
cannot be filtered or indexed by the store while individual columns can. **The
adapter does not currently exercise that.** Its store-side filter is built on
the state field alone; scope is narrowed **in the adapter, in Rust, after
reading the state-wide set**, and the code says that cost is deliberate. So no
query path in use today would be lost.

⚠ **The real coupling is elsewhere, and it is the one to weigh:**
`refereed_fields()` chains `counter_fields`, so the generated guard closes
**one column per counter** and `explain --map` prints them individually as the
set an adopter must sweep for. Collapsing counters into a blob changes the
shape of both — the guard would close a single column, and the hunting list
would stop naming the thing an adopter actually reasons about. Whether that is
a loss or a simplification is the genuine question in this item, and it is not
a question about query performance at all.

**C. A wake path.** There is already a `notify` surface; the missing direction
is inbound — something that tells a waiting agent that `awaiting` would now
return differently.
*Cost:* moderate. *Risk:* it is the first thing that puts a clock or a socket
near the engine's edge, so the boundary has to be drawn carefully or the
purity rule erodes by convenience.

**D. A visual state surface.** Rendering a definition as a graph, or a live
board of records.
*Cost:* real, and ongoing. *Recommendation: not now, and I would argue against
it if pressed.* The north star's target client is the author, who reads
`awaiting`. A visual surface is a thing you build when someone who is not the
author needs to see the state — that person does not exist yet, and building
for them first is how the ergonomics tail wags the engine.

**E. The proposal-translation layer** — tool output → validated payload →
transition proposal. This is the thing PydanticAI and friends hand you.
*Cost:* unclear, and I would want a second adopter's evidence before shaping
it, because the first loop's version of this is entangled with its own
tracker. Named here so it is not lost, not because it is ready.

---

## 4. What I would ask the owner

1. **Is the runner interface (A) the next design conversation?** It is the
   only item here I think is underweighted, and the argument for doing it
   *before* a second adopter arrives is that the interface gets shaped by
   evidence rather than by whichever runner exists when we get around to it.
   The argument against is that one adopter is thin evidence for an interface
   this consequential — and that argument is good.
2. **(B) is a measurement, not a decision** — worth taking if the
   queryability trade measures small. Should it be measured?
3. **(D) I recommend declining** and would like that recorded as declined
   rather than deferred, so it stops resurfacing every time a comparison table
   mentions devtools.

⚠ Note what is deliberately absent from all of the above: nothing here says a
feature needs a consuming loop behind it. There is **no standing admission
bar** (owner, 2026-08-25); each item stands on its own reasons.

# FerroStep — north star

## 1. Vision

> **Ratified by the owner, 2026-08-21. Extended and re-ratified by them,
> 2026-09-14.** The owner's own position, given in conversation and read back —
> not an agent's inference, which both previous drafts were and neither of which
> was ever signed.

**What it is** (owner, 2026-09-14): **a multi-agent orchestrator — agnostic to
model vendor, subscription, API key and local models alike — carrying a
code-review loop and agent identities, and installed light.** Every word of that
is a requirement rather than a description: *agnostic* is why no vendor's tooling
gets framework-level support, *the loop and the identities* are the two things it
already does, and *light* is the bar an install is measured against.

**The target client is the author, who uses this product.** FerroStep exists so
its own operator can run serious multi-agent loops — worker/reviewer cycles, QC
gates, human escalation — with the rules of the loop written down once, in data,
and enforced the same way everywhere, without adopting a framework that owns the
runtime, the state, or the hosting. It is the deployable form of a harness that
was first proven by hand.

**Other users are always kept in mind, and that is why the adapter pattern
exists.** Their needs shape the architecture rather than trailing it: the
ledger, the notifications, the issue log and the agent runtimes are reached
through adapters precisely so that somebody else's stack can sit behind them. Shareability is a
design constraint, and it means **much more lives in configuration than a single
operator would ever need stated up front** — what a state means, which roles are
people, what a loop costs, which store holds the truth. Where the engine has
already decided something on a user's behalf, that is an assumption to remove
rather than a feature to defend.

**There are two deployment contexts, and they pull opposite ways** (owner,
2026-08-21). One is a loop already running against a store that is installed and
staying, which wants an adapter for what is already there. The other is a team
that will not point an agent harness at their existing data systems at all, and
that evaluates at the scale of a single developer's workstation before anything
else — which wants nothing to install. Serving both is another reason the ledger
sits behind an adapter instead of being assumed, and it is why the zero-install
path is a first-class concern rather than a courtesy to newcomers.

What does not change: the ledger is the memory, and the human stays the
authority the loop escalates to.

⚠ **A third clause stood in that sentence until 2026-09-14, and the owner
removed it**: *"the engine is a referee rather than a runtime"*. Their words for
why — *it was never truly mine*. It had entered as an agent's inference and was
then ratified in company with text that genuinely was the owner's, which is
exactly how an inference acquires an authority nobody granted it. The owner's
stated goal (2026-09-11) is a light-touch installation — *"I don't care if it's
a runtime or not"* — so whether this engine is one is not a constraint on the
product. The rest of this section stands as ratified 2026-08-21.

⚠ **How the sentence above got here, because that is the part worth keeping.**
It was said in conversation on 2026-09-14, recorded here the same day **marked
unratified**, and ratified by the owner on 2026-09-14 in the same sitting that
struck the clause. It was deliberately not written straight into the Vision: an
agent promoting its own record of a remark into ratified text is exactly the
mechanism that put *"a referee rather than a runtime"* in this section, and the
gap between recording and ratifying is the only thing that distinguishes them.

⚠ **What the strike does not license is the opposite wall.** "Not a runtime"
being gone is not "it is a runtime". How much of an orchestrator this becomes is
answered by the roadmap item by item, with sources — not by reading a removed
prohibition backwards.

**Tiebreaker.** The author's own loop decides *what* gets built. Other users
decide *how it is shaped*: given something worth building, prefer the form a
stranger could reconfigure over the one that hardcodes our arrangement.

> ⚠ This paragraph used to continue "a feature still needs a real consuming
> loop, and that loop is ours" — **removed by the owner on 2026-08-25, and not
> to be restored.** It sat inside a ratified section without ever having been
> ratified itself, which is how it came to be quoted back at the owner as
> their own ruling. Whose needs set the priority is the part that was ratified;
> a bar on what may be built at all was not.

## 2. Ours vs rented

> **A philosophical outset, not a rulebook** (owner, 2026-08-21). This section
> says where effort belongs *today*, in a young product. It is a default to
> start from and argue against, not a wall to build around — where a real need
> says otherwise, the need decides.

**Ours:** the workflow definition format, the validation and decision
semantics, the crash-accounting model (spend-on-entry), the bindings.
**Rented:** the database, the agent runtimes, the LLM providers, the transport
— reached through adapters rather than named at framework level.

That preference is practical rather than principled, which is what makes it
worth stating and also what makes it movable. An interface shaped around the
first target somebody writes tends not to reach the second, and the corner is
usually invisible until you are standing in it; adapters are how a small
project stays cheap to point somewhere new. When that stops being the cheaper
answer for something, it stops being the answer.

A ledger built by the same hands is no longer hypothetical and no longer merely
one more choice: **FerroTrack is FerroStep's out-of-the-box store as well as a
product in its own right** (owner, 2026-08-28). ⚠ This paragraph said the
opposite — that such a ledger *"sits on the rented side… one more adapter and
one more choice"* — until 2026-09-14, because the ruling that replaced it landed
in FerroTrack's files and not in these.

The sequencing is still PocketBase and SQLite first — PocketBase is in live use
and expected to stay that way (owner, 2026-08-21) — and that is not deference to
history. **A store nobody here controls is what keeps the interface honest, and
that matters more now that one of the stores is ours**, not less. The terms are
in [ROADMAP.md](ROADMAP.md) §E2 and are deliberately not restated here.

## 3. The one organizing principle

**The ledger is the truth, and `ferrostep-core` is a pure function over it.**
Every side effect the product needs lives on the far side of an adapter — which
is a rule about **where code goes**, not a limit on what the product may grow
into.

⚠ **A second sentence stood here until 2026-09-14, and the owner struck it**:
*"Anything that would make the engine stateful, asynchronous, or a network peer
is scope creep, however convenient."* Unattributed, and written as a wall around
the whole product rather than as the crate rule it actually is.

**The invariant survives; the wall does not.** `ferrostep-core` stays pure —
that is real, it is what makes every adapter possible, and
[AGENTS.md](../AGENTS.md) §Layout is where it binds. What was never the owner's
is the leap from *the core is a pure function* to *the product may not be
stateful, asynchronous, or reachable over a network* — a leap that would have
ruled out the light-touch install they actually want, headless process and all.

## 4. Load-bearing constraints

- Decisions must be deterministic and explainable — a denied move names why.
- A crashed pass has already been paid for; no design change may reopen that.
- Enforcement is layered: the engine defines, the store enforces — by whatever
  mechanism that store actually has, which is not always its access rules. The
  engine alone is advisory and the docs say so plainly.
- The Decision JSON shape is a public contract across every binding. Two exist
  (Rust, Python); a third lands when a consumer drives it.

## 5. The real bottleneck

Not engine features — **adapter honesty**. The value lands only when the ledger
write is atomic per backend, and no two backends make the same atomicity
promise. **Two adapters is the floor, not one**: a single implementation cannot
show whether the interface is general or merely shaped around the store it was
written against, and the second one is what exposes the parts the first hid.

## 6. One breath

A pure referee over a database ledger: your agents do the work, your database
holds the truth, FerroStep says what's legal and when a human takes over.

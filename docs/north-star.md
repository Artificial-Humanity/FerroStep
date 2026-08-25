# FerroStep — north star

## 1. Vision

> **Ratified by the owner, 2026-08-21.** The owner's own position, given in
> conversation and read back — not an agent's inference, which both previous
> drafts were and neither of which was ever signed.

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

What does not change: the ledger is the memory, the engine is a referee rather
than a runtime, and the human stays the authority the loop escalates to.

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

A ledger built by the same hands sits on the rented side for the same reason:
it would be one more adapter and one more choice. The sequencing is PocketBase
and SQLite first — PocketBase is in live use and expected to stay that way
(owner, 2026-08-21) — because a store nobody here controls is what keeps the
interface honest. The terms are in [ROADMAP.md](ROADMAP.md) §E2, along with the
non-goals, and are deliberately not restated here.

## 3. The one organizing principle

**The ledger is the truth and the engine is a pure function over it.** Anything
that would make the engine stateful, asynchronous, or a network peer is scope
creep, however convenient.

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

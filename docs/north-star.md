# FerroStep — north star

## 1. Vision

> ⚠ **UNRATIFIED DRAFT** (agent, 2026-08-20) — assembled from the owner's own
> project blueprint and stated intent, but the Vision section is the owner's
> to sign. Treat as a proposal until this warning is removed.

**The target client is the author.** FerroStep exists so its own operator can
run serious multi-agent loops — worker/reviewer cycles, QC gates, human
escalation — with the rules of the loop written down once, in data, and
enforced the same way everywhere, without adopting a framework that owns the
runtime, the state, or the hosting. It is the shareable, deployable form of a
harness that was first proven by hand: the database ledger is the memory, the
engine is the referee, and the human stays the authority the loop escalates to.

Being useful to others is a hoped-for side effect, pursued through legibility —
honest docs, stated prior art, tested invariants — never through features the
author does not need. This is not a competitive product and has no roadmap
obligation to anyone's use case but its operator's; when a design question has
no clear answer, "what does the author's own loop need?" is the tiebreaker.

## 2. Ours vs rented

**Ours:** the workflow definition format, the validation and decision
semantics, the crash-accounting model (spend-on-entry), the bindings.
**Rented:** the database (PocketBase/SQLite/Postgres — the user's choice), the
agent runtimes, the LLM providers, the transport. FerroStep must never grow a
scheduler, a queue, or a hosted anything.

## 3. The one organizing principle

**The ledger is the truth and the engine is a pure function over it.** Anything
that would make the engine stateful, asynchronous, or a network peer is scope
creep, however convenient.

## 4. Load-bearing constraints

- Decisions must be deterministic and explainable — a denied move names why.
- A crashed pass has already been paid for; no design change may reopen that.
- Enforcement is layered: engine defines, database rules enforce. The engine
  alone is advisory and the docs say so plainly.
- The Decision JSON shape is a public contract across three languages.

## 5. The real bottleneck

Not engine features — **adapter honesty**. The value lands only when the
ledger write is atomic per backend, and the three candidate backends make
three different atomicity promises. Getting one adapter (PocketBase) exactly
right beats sketching three.

## 6. One breath

A pure referee over a database ledger: your agents do the work, your database
holds the truth, FerroStep says what's legal and when a human takes over.

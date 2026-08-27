# An expression language in a definition — a consideration

⚠⚠ **THIS IS A CONSIDERATION, NOT A DECISION, AND NOTHING IN IT IS A RULING.**
No part of this file has been decided, by anyone. The recommendation in §6 is
**the resident persona's argument**, marked as such so it cannot later be
quoted as the owner's position — which is a failure this repo has already paid
for once, in the file that *is* an agent's system prompt (AGENTS.md
§Conventions). ⚠ **If you find this cited as a ruling, the citation is wrong.**

**The question, and who asked it.** The owner, **2026-08-27**: *could we
benefit from using Rhai — or Koto — as a script interface to our core engine?*
Everything below is an answer to that question, not a plan.

---

## 1. The gap is real, and it is not hypothetical

Two things a definition **cannot currently say**, both of which came up on the
day the question was asked:

* **A rule with a direction over an ordered value** — *raising a grade is
  anyone's, lowering it is the reviewer's*. The asymmetry is the whole point:
  raising cannot clear a gate and lowering can.
* **A relation between fields** — *these scope labels are one address*, so
  moving a subset leaves the record in no consistent unit of work.

A definition speaks in states, transitions, roles, counters and rescope grants.
Neither of the above is expressible in that vocabulary. So the pressure toward
an expression language is genuine rather than imagined, and saying so is not
the same as saying yes.

## 2. The constraint that decides it

**A definition has to be analyzable without being executed.** This is not a
preference; it is the product. `explain` prints every number a definition
asserts. `explain --map` hands an adopter the sweep to run before closing
columns to direct writes. The satisfiability check asks whether a definition
can even hold against a given store. **All three work by reading a definition,
never by running it.**

⚠ A predicate that can only be understood by evaluating it degrades all three
to *"there is a script here"*, and does so silently — the tools keep working
and stop being informative.

## 3. The constraint is satisfiable, which is why this is not a dead end

Verified rather than assumed, 2026-08-27:

* Rhai compiles to an `AST` and exposes **`AST::walk`**, a depth-first
  traversal taking a callback — so what a predicate *references* can be
  enumerated without evaluating it. That is the mechanism by which `explain`
  and the satisfiability check could survive.
* It ships real sandbox limits — `set_max_operations`, `set_max_string_size`,
  `set_max_array_size`, `set_max_map_size` — returning a termination error when
  the budget is hit.

⚠ **The operation budget is not a nicety for a referee.** Definitions are data
an adopter supplies. An unbounded predicate is a hang in the component that
decides whether work may proceed, and a referee that cannot answer is worse
than one that answers no.

Sources: <https://rhai.rs/book/engine/ast.html>,
<https://docs.rs/rhai/latest/rhai/struct.Engine.html>.

## 4. ⚠⚠ A standing gap this would amplify

**An event records actor, role, from-state, decision and note — and nothing
identifying which definition decided it.** No hash, no definition version.
Checked in the ledger contract, not recalled.

That is **already** a gap: change a counter's ceiling and past events do not
record which ceiling applied. Scripting does not create it. But it amplifies it
badly — a ledger's history should be re-derivable from *(definition,
snapshot)*, and a predicate that changed quietly makes past decisions
unreproducible in a way that a diffable declarative change does not.

⚠ **Worth fixing on its own merits regardless of this question**, and it would
be a prerequisite rather than a follow-up if the answer here were ever yes.

## 5. Rhai or Koto — the axis, not the syntax

**Decide on static analyzability**, not on ergonomics. Rhai's is confirmed
above. Koto is lighter and has a cleaner functional syntax; **its public
AST-walking story has not been checked here**, and that is the thing to check
before choosing, because it is what preserves the tools in §2.

⚠ Stated as unchecked rather than guessed at: this file should not become the
place someone later reads a comparison that was never measured.

For a component whose job is to be trusted, maturity and a small surprise
surface are worth more than syntax.

## 6. The resident's recommendation — an argument, not an outcome

**Not now — and the strongest reason argues against the case for it.**

The same day this was asked, a **first-class declarative concept** for graded
attributes was agreed: an ordered ladder with directional grants, living in the
definition where rules live. **An expression language and that path are
competing answers to the same gap.** Nobody builds the declarative concept if a
predicate will do — and then `explain` quietly stops being informative while
every tool still reports green.

Where an expression language genuinely wins is **the long tail**: the tenth
concept added to serve one adopter's one rule. That is a real problem and a
distant one, and the honest way to reach it is demand evidence in
[`adoption-friction.md`](adoption-friction.md) rather than an argument made
from a clean sheet.

⚠ **A nearer and safer use, worth separating from the question as asked:**
scripting is safe exactly where inspectability is not the product. Not the
decision path — but the satisfiability check's own checks, or adopter-defined
notification and report templates. There, opacity to `explain` costs nothing,
because there is no decision to explain.

**If it ever happens:** a separate, feature-gated crate, and **never a
dependency of `ferrostep-core`**. A pure core is the selling point, and a
script engine inside it is a large dependency in the one place meant to be
minimal.

## 7. What would change this answer

Stated so the question can be reopened on evidence rather than on mood:

* An adopter needing a rule that **no declarative concept fits**, rather than
  one that no *current* concept fits.
* The declarative path proving to cost a new concept **per adopter** — which is
  the long tail arriving, and is measurable.
* Events gaining a definition identity (§4), which removes the auditability
  objection rather than arguing with it.

⚠ **None of these is a bar.** There is no standing admission requirement in
this repo (owner, 2026-08-25); features are judged case by case. These are the
things that would make the argument in §6 wrong, which is a different thing
from a gate.

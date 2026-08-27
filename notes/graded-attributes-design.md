# Graded attributes — the design, before the code

**Register entry 3, item B.** The owner ruled the *direction* on 2026-08-27 —
a labelled stopgap now, the real thing behind `doctor` — and the register was
explicit that **the shape is the hard part and still open**, with the standing
interface test named as the reason not to rush: *shape it around what the thing
is, not around how this one adopter spells it.*

This states the shape and the two decisions that produce it, so they can be
argued with rather than discovered in a diff.

---

## The hole, restated

A lane's merge gate reads a **severity** grade. Below a floor a finding rides;
at or above it the branch is blocked. That column is outside the referee, so
the only thing between a developer and clearing their own gate is a
**self-declared author flag** in a script — whose own comment calls it *a
convention, not a mechanism*.

`attribute_fields` closed half of it: the column is refereed, so writes carry a
token and land as events. ⚠ **What it does not say is who may set which value,
or in which direction** — and for a graded column that is the half that matters.

---

## Decision 1 — a grade change is its own operation, not a field on a move

Grading does not move the record. The adopter's grade command sets a value with
no state change at all, and a design that made grading ride a transition would
force a fabricated move to record one.

⚠ **The precedent is `rescope`, and it is exact.** That is also "not where the
record goes next, but something else about it that is refereed" — it returns
the state the record is already in, so a caller persists it through the one
atomic path it already has. Grades take the same shape and inherit its
properties for free: versioned, evented, refused on a terminal record.

## Decision 2 — ⚠⚠ THE ENGINE HAS NO OPINION ABOUT WHICH DIRECTION IS DANGEROUS

The owner's framing was *raising is anyone's, lowering is the reviewer's*, and
for this adopter that is right: their gate blocks at or above a floor, so
raising cannot clear it and lowering can.

**It is not right in general, and hard-coding it would fail the interface
test.** A gate that requires a *minimum* — "confidence must be `high` to
merge" — inverts the whole thing: there, raising clears the gate and lowering
is the safe direction. An engine that assumed the first shape would be
silently wrong for the second, in the direction that grants permission.

So a definition names the roles permitted in **each** direction, and the engine
never infers which one is privileged:

```json
"grades": [
  { "attribute": "severity",
    "ladder": ["low", "medium", "high", "critical"],
    "raise": ["worker", "reviewer"],
    "lower": ["reviewer"],
    "requires_note": true }
]
```

⚠ **Order comes from the ladder's position, never from the value's name.** No
lexical comparison, no implied numbers — `"low" < "medium"` is true only
because the ladder says so, and an adopter whose grades are `p3 p2 p1` gets the
same engine.

⚠ **Empty means nobody**, exactly as `rescopes` already does. A definition that
does not name a direction has not left it open; it has closed it.

## What this deliberately does NOT model, and why

**No threshold, and no notion of which side passes.** The register's phrasing
reached for "an ordered ladder *and a threshold*", and the threshold is the
part to leave out. The moment the engine knows which side of a line is
"blocked", it has the opinion Decision 2 exists to refuse — and the gate is the
adopter's policy, living in their merge check, where it already is.

What the engine guards is **who may move the value, and in which direction.**
That is precisely the hole the self-declared author flag was standing in.

⚠ **`purpose` is the standing precedent for carrying without interpreting**, and
a grade's *meaning* belongs in the same category. The engine knows `critical`
is further along a ladder than `low`. It does not know that this is bad.

## The join to the deployment half

A graded attribute still needs its column named in the collection map's
`attribute_fields` — the same split that already puts `counter_fields` in the
map and each counter's `max` in the definition. **Nothing built on the stopgap
has to be unbuilt**, which is what the stopgap's own docstring promised.

⚠ **Sequencing, and it has three steps rather than two** (measured with the
adopter, 2026-08-27): declare the column in the map → **regenerate and install**,
because a route generated before the column has no branch for it and would
refuse it → *then* move the client. The middle step is not optional and was not
in the original two-step plan.

## What would change this design

* An adopter needing a rule about a **relation between two grades** ("severity
  may not exceed impact"). That is the expression-language question, and this
  note does not answer it — see `expression-language-consideration.md`.
* A gate that needs the engine to *evaluate* it rather than guard the value
  feeding it. That is a real request and a different feature.
* Evidence that naming both directions is too verbose in practice. It is more
  typing than "raising is anyone's"; the trade is that it cannot be wrong.

⚠ **None of these is an admission bar.** This repo has no standing requirement
that a design answer them before it ships.

# What a ledger owes this engine, and where PocketBase falls short

Working notes, not a deliverable. Written as input to a purpose-built ledger
being designed elsewhere, and as the reasoning behind whatever the PocketBase
adapter ends up doing. Everything measured here is measured against stock
PocketBase behaviour and its documented defaults; none of it is a complaint
about the project, which is doing a general-purpose job well and was never
aiming at this one.

**Read the requirements section first even if you only came for the
shortcomings.** Most of what looks like a PocketBase deficiency is really this
engine wanting something a general backend has no reason to offer.

---

## 1. What the engine actually needs from a ledger

Derived from the engine, not from any backend. An adapter is judged against
this list; so is a ledger built on purpose.

1. **Apply one decision atomically.** A decision can change the state, spend
   counters, and append an event. Those must land together or not at all. A
   partial apply is worse than a refusal, because the record then disagrees
   with its own history and nothing says so.
2. **Compare-and-swap on the record.** Two actors reading the same snapshot
   must not both succeed. Without this, two workers each claiming a pass from
   `2` both write `3`: one pass is unaccounted for and two agents are working
   the same record believing they are alone. This is the difference between a
   ceiling and a suggestion.
3. **An append-only event log**, enforced structurally rather than by
   convention. Its rows carry the actor, the role, the move, the counter
   changes, and an opaque note. It is the only place a human's reasoning for
   releasing a paused record survives more than one release.
4. **Deterministic ordering within a record.** Timestamps tie — a batch of
   writes can share a millisecond — so ordering needs a per-record sequence
   with uniqueness enforced by the store, not assigned hopefully by the
   caller.
5. **Enumeration by scope.** "Which records await a person" and "which records
   belong to this unit of work" are the queries the decision surface, the
   audit report and any merge gate are all built on. They must page correctly
   over unbounded results.
6. **Role-scoped identity.** The engine gates transitions on a workflow role.
   A ledger that only knows users and admins can carry the role in a column,
   but then the store cannot enforce what the engine defines, and defence in
   depth is not available.
7. **Independent processes over a network.** Actors are separate programs in
   different languages, plus a human. This rules out an embedded library with
   no service in front of it, whatever its other merits.
8. **Human-legible and hand-editable.** An operator opening the store and
   changing a row is legitimate, not corruption. This one is easy to lose
   when building something "for agents", and losing it costs more than any
   feature on this list is worth.

---

## 2. Where PocketBase meets them, and where it does not

### It meets, comfortably

- **One-record atomicity (part of 1).** State and counters live on the same
  record, so a single `PATCH` writes both atomically. The crash-pricing
  promise — a claimed pass is spent even if the claimer dies — is safe on
  stock PocketBase with no special measures. This is worth stating plainly
  because it is easy to assume the whole of requirement 1 is at risk when
  only part of it is.
- **Requirement 7.** An HTTP API and auth in a single binary, which is most of
  why it keeps coming up.
- **Requirement 8.** The admin console is a genuine browsable, editable view of
  every row. **This is the requirement a purpose-built replacement is most
  likely to drop and most likely to regret dropping.**
- **Requirement 4**, if you build it: a per-record sequence column plus a
  unique index on `(record, seq)`. The index is the referee — concurrent
  appends collide with a `400` and retry rather than silently interleaving.

### It does not meet

- ~~**Cross-record atomicity (rest of 1).**~~ ⚠ **CORRECTED 2026-08-21 — this
  was recorded as a gap and is not one.** The transactional batch endpoint is
  disabled by default, which is what the original note keyed on. But a hook
  runs *inside* the record's own save and the hook runtime exposes an explicit
  run-in-transaction primitive, so record-plus-history in one atomic operation
  is reachable **without any settings change**. The mistake mattered: an
  adapter following the original note would have reported itself non-atomic,
  and that field is exactly what a caller is meant to trust.
- **Requirement 2, through the REST API.** There is no conditional update — no
  `If-Match`, no `WHERE version = n`. ⚠ **This was the sharpest gap and it is
  now closed, but only on one specific write path** (measured 2026-08-21; see
  the resolution at the end of this bullet).

  ⚠ **The obvious workaround was measured and it does not work.** An access
  rule can compare a submitted value against the stored record, which looks
  like a conditional update and is the cheapest one available. Under concurrent
  writers it is not a compare-and-swap: **two writers are enough**, and in a
  repeated run most rounds produced more than one winner. The predicate is
  evaluated before the write commits, so anyone arriving inside that window
  passes a check that is already stale.

  A control settles what kind of failure it is. With the predicate replaced by
  an always-true rule, every writer won and the last one's value survived — a
  textbook lost update. So the predicate's *semantics* are correct and its
  *atomicity* is absent; only the second licenses the capability flag. Treat a
  rule-level predicate as a cheap pre-filter and never as a ceiling.

  ⚠ **An administrator bypasses it entirely** — not a window, an absence. A
  write submitting a wildly wrong expected version succeeded and set the
  version to whatever it claimed. So while agents authenticate as
  administrators, a rule-level ceiling is not weak here, it is *absent*, and
  requirements 2 and 6 are one finding rather than two.

  ✅ **RESOLVED 2026-08-21: a hook-level compare inside an explicit transaction
  IS a compare-and-swap here.** 43 rounds across four independent runs, zero
  failures — 2, 4 and 16 concurrent writers, one winner every round, the losers
  refused cleanly rather than erroring. It holds for **administrator
  credentials too**, which was tested deliberately rather than assumed. So an
  adapter on this backend *can* advertise the flag, and the write path that
  earns it is a generated handler rather than a REST call.

  ⚠ **The distinction the measurement isolates is the whole finding: the
  compare must be inside the TRANSACTION, not merely inside the REQUEST.** The
  same check, run in a hook that then lets the ordinary write proceed, fails —
  and fails in the worst available way. At 16 writers it produced 1, 2, 7, 6,
  1, 7 winners across six rounds: **two of those six rounds look perfectly
  correct.** In every failing round the version advanced by exactly one while
  several writers were told they had succeeded, which is a silent lost update
  wearing a `200`. At two writers it failed every round, making it *worse* than
  the rule-level predicate rather than an improvement on it.

  That is the practical argument for [`AGENTS.md`'s repeated-rounds
  rule](../AGENTS.md) stated as a number: a single-round test of this variant
  had two chances in six of certifying it as working. Testing the failing
  variant alongside the passing one is what produced that number, and it cost
  minutes — **run the design you rejected, not just the one you hope for.**
- **Requirement 3, through API rules.** Rules cannot make a collection
  append-only for the actors that matter, because **superusers bypass rules
  entirely** and agents typically authenticate as superusers. Rules are also
  per-operation, not per-field, so "only a human may write this column" has no
  expression either.

  ⚠ **Hooks are a different layer and superusers do NOT bypass them** (measured
  2026-08-21: an administrator's deliberately wrong claim was refused, same as
  an ordinary account's, with the stored value unchanged after both). This is
  the opposite of the rule layer's behaviour, and it is worth stating next to
  it because the two are easy to reason about as one thing. So requirement 3
  is reachable at the hook layer even before the identity model changes — which
  **relaxes** the sequencing rather than hardening it: role-scoped accounts stop
  being the only thing standing between an administrator and a ceiling, and
  become defence in depth.
- **Requirement 6.** Identity is user/superuser, not role. Roles can be carried
  in a column, but the store cannot enforce them, so role-gating stays advisory
  at the database layer.

### Sharp edges worth knowing regardless

- `perPage` defaults to **30** and caps at **500**. A query written against a
  small table silently truncates when the table grows, and the caller sees a
  successful response. Any enumeration must page and never trust one page.
  ⚠ **The response carries a total count, so truncation is DETECTABLE** — an
  adapter can assert it read as many as the store said existed, which is a
  check rather than a discipline. There is also a flag that skips the count
  query for speed, and skipping it is the one way to make truncation silent.
  **At least one client in use defaults that flag on.** Set it explicitly;
  never inherit it on an enumeration whose completeness matters.
- **There is no native revision or etag**, and the store-maintained timestamp
  fields are the obvious wrong substitute: they are set by the store rather
  than the caller, and equality across two writes in the same instant is not a
  safe comparison. A version token should be an integer the adapter owns and
  increments — equality is the entire requirement, and an integer has no
  precision question.
- ⚠ **Each hook callback runs in its own isolated runtime, and file-scope
  functions and constants are not visible inside it.** Shared logic factored
  into a top-level helper produces a reference error on every call, surfacing
  as a generic `400` with the real message only in the store's own log
  endpoint — which returns oldest-first unless sorted. This is a landmine for
  any milestone that *generates* store-side enforcement, because factoring
  shared logic is what a generator naturally does. **Emit self-contained
  handlers**, and expect the deliberate duplication to look like something a
  later reader should tidy.
- ⚠ **A refusal message is normalized before it reaches the caller.** A thrown
  `"cas_conflict: … again"` came back as `"Cas_conflict: … again."` — first
  letter capitalized, trailing period appended. **An adapter matching a literal
  prefix will classify every conflict as unretryable**, silently and in the
  direction that looks like a broken store. Match case-insensitively, and do
  not depend on the tail of the string at all.
- **A refusal can carry the actual stored value**, so a caller can re-read and
  decide from the error body without a second round trip. Worth designing the
  handler's message around deliberately, since it is free at the point the
  handler already knows both numbers.
- **A server-computed field a hook owns cannot be set by the caller, only
  advanced by it** (measured: a caller submitting an absurd version alongside a
  correct expected-version still advanced the record by exactly one — the hook
  computes the next value and discards what it was sent). ⚠ **The measurement is
  about the version token, not about workflow counters**, and the two must not
  be conflated in an adapter's claims. What it establishes is the *pattern*: a
  value the store computes is enforced where a value the client submits is only
  trusted, and that is the shape a spend-on-entry counter wants. It also means
  a **reset** cannot travel this path — a monotonic handler has no way to
  express one, so re-arming a ceiling needs a distinct authorized operation
  rather than a smaller number in the same call.
- ⚠ **Writing a hook file auto-restarts the service** (the watch flag defaults
  on), so an install needs no explicit restart — and a health check fired
  immediately after the write can answer *before the restart begins*, reporting
  healthy and then dropping the next connection. **A check that runs before the
  event it is meant to observe is not evidence**, which is the same family as
  checking the instrument ran before believing a negative.
- A `text` field caps at **5000 characters** by default, and `"max": 0` does
  **not** lift it — `0` means unset, so the default applies. It fails at insert
  time, not at schema time, so it surfaces as a partial migration.
- `maxSelect` on a `select` must be less than or equal to the number of values.
- Schema payloads use `fields`, not `schema` (renamed in 0.23; most material
  online predates it).
- An empty-string rule means **public** and a null rule means **superuser
  only**. They are opposites, and the failure is silent in the dangerous
  direction.
- Still pre-1.0 (0.39 at time of writing) with no backward-compatibility
  guarantee.

---

## 3. What the adapter can do about it

**Start by not being a superuser** (owner, 2026-08-21). Requirements 3 and 6 both
looked unreachable while agents held administrator credentials, and both were
unreachable for the same reason rather than two — an administrator bypasses the
store's access rules, so no rule constrains the actor that matters. Give each
agent an account scoped to its workflow role and the rules apply again: the
event collection permits create and refuses update and delete, which is
append-only with no special machinery, and the role the engine gates on becomes
the role the store authenticates.

⚠ That reframing is worth keeping even where it feels like belt-and-braces. An
actor holding an administrator credential can rewrite the records themselves,
not merely their history, so guarding the log against it never protected
anything. **The append-only property is a defence against mistakes, and the
identity model is the defence against everything else.**

⚠ **Update, 2026-08-21: the ruling stands and its urgency does not.** Hooks
turned out to constrain administrators where rules do not (§2), so the identity
model is no longer the *only* thing between an administrator and a ceiling. It
is now the second layer rather than the first, which changes when it has to be
built, not whether. Keep the distinction visible: enforcement that survives an
administrator lives at the hook layer, and everything the identity model adds
on top of that is depth.

What still needs `pb_hooks`, the embedded JavaScript runtime, is the part the
REST API genuinely cannot express:

- `$app.runInTransaction((txApp) => { … })` gives a real transaction, so the
  record update and the event append land together — closing the gap that
  otherwise leaves the history able to disagree with the record.
- `routerAdd("POST", "/api/…", handler)` gives a custom endpoint. One call
  carrying a decision, applied transactionally server-side, beats any sequence
  of REST calls — and it is where compare-and-swap lives, since the handler can
  re-read the record inside the transaction and refuse a stale snapshot. A
  custom route is invoked deliberately by the caller, so unlike a request hook
  it raises no question about which credentials it fires for. ✅ **The
  compare-inside-the-transaction half is now measured and holds** (§2); what
  remains for the adapter is the shape of the payload, not whether the
  guarantee exists.

⚠ **One failure class is untested and must not be quietly assumed covered:**
field-validation failure through either variant. Both compute the guarded value
server-side and discard the caller's, which is exactly why it could not be
provoked — so an adapter's error mapping is measured for conflicts and
not-founds, and inferred for validation. Say so where the mapping is written.

So the PocketBase story is *stock instance, plus a role-scoped account per
agent, plus a generated route for applying decisions*. The adapter detects at
startup what it has and **says which mode it is running in** rather than
degrading quietly.

Remaining honestly: immutable history here is enforced by rules that a store
administrator can still step around, and the adapter should say so rather than
let the audit report imply more than the store delivers.

---

## 4. Notes for a purpose-built ledger

What the list above implies, if someone is starting from a blank page:

- **One write path.** `apply(decision, expected_version)` as the primary
  operation, transactional and compare-and-swap by construction. Everything
  above needs it; nothing above is served by a general-purpose record API that
  happens to allow it.
- **The event log is write-once because the storage says so**, not because a
  hook objects. Append-only enforced by a rule someone can turn off is a
  convention wearing a costume.
- **Role is the authentication unit.** A credential is scoped to a workflow
  role, so what the engine gates on and what the store enforces are the same
  concept rather than two that have to be kept agreeing.
- **Enumeration is first-class and paged correctly by default**, because the
  decision surface, the audit report and any merge gate are all the same query
  with different filters, and a truncated page is indistinguishable from a
  short answer.
- **Schema derived from the workflow definition**, not hand-declared beside it.
  Two descriptions of the same state machine drift; one of them generated does
  not.
- ⚠ **Keep the browsable, editable view.** It is the least exciting item and
  the easiest to defer, and it is load-bearing: an operator hand-fixing a row
  is a designed-for path, and a ledger that only speaks to programs turns every
  incident into a database console session anyway — just a worse one.

And the constraint that outlives all of the above: whatever gets built, this
engine reaches it through the same adapter interface as everything else, with
no privileges the others lack. An interface bent toward a backend the same
hands control is the failure that has nothing fail while it happens.

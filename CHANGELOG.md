# Changelog

Notable changes, per release. What a version *means* is defined in
[docs/ROADMAP.md](docs/ROADMAP.md) §Releases — outcomes, not dates. The
Decision JSON rule in [AGENTS.md](AGENTS.md) §Conventions is one reason an
entry here is mandatory rather than courtesy.

## Unreleased

- `ferrostep-cli`: **`--map` accepts the generation config as well as the bare
  map**, so a mapped deployment needs one file rather than two.

  ⚠⚠ **TWO FILES THAT MUST AGREE WILL EVENTUALLY DISAGREE, AND THIS PAIR DID IT
  QUIETLY.** The generator reads a wrapper (`{"map": …, "actors": …}`) and the
  CLI read only the bare map, so a deployment kept both — a source and a
  derived copy, held in agreement by somebody remembering a command. Edit the
  wrapper, forget to re-derive, and **both files still parse and disagree**;
  `doctor` then checks a map **nothing was generated from** and reports
  agreement. An instrument confirming the wrong artifact is worse than no
  instrument, and it was reachable by a documented workflow rather than a
  mistake.

  ⚠ **Ambiguity is refused, not resolved by precedence.** A file carrying both
  a `map` key and a `records` key is one somebody is mid-conversion on, and
  picking a winner would silently use the half they are not editing — the same
  drift, reintroduced by its own fix.

  Found by the second adopter's first deployment. They resolved it with a
  derivation convention, which was right and which cannot detect its own
  staleness — the fix belongs on this side.

- `ferrostep-pocketbase`: **the mapped migration's idempotency is measured**,
  and the test that covered it now covers all three of its guarded changes
  rather than two.

  ⚠⚠ **THE TEST WAS NAMED FOR A POPULATION THAT HAD GROWN.**
  `..._guards_both_of_its_changes` asserted the version-field guard and the
  events guard while the migration had since gained a third — the actor
  collection — so it would have passed with that guard deleted outright.
  Renamed, widened, and red under exactly that mutation.

  ✅ **Measured against a disposable instance**: applied once to a collection
  having none of the three, all three appeared; applied again with a live
  record and a live event present, the schema came back identical and both
  rows survived.

  ⚠⚠ **The first attempt at that measurement was vacuous and looked exactly
  the same.** PocketBase keys applied migrations on filename and applies them
  in filename order, so a re-run copy numbered *below* a migration the store
  had auto-written gives a flawless "nothing changed" from a file that may
  never have executed. It became evidence only once the copy was numbered
  above the highwater mark and a marker migration in the same restart proved
  new files execute at all. **A no-op and a no-run are the same diff.**

  ⚠⚠ **And the down path is destructive, measured rather than inferred.**
  `pocketbase migrate down 1` deleted the events collection and every row in
  it, deleted the actor collection, and removed the version field — while
  leaving the refereed records in place. A revert separates records from their
  history, which is the one thing the mapped shape exists to keep together. It
  prompts first and will not run unattended; that is the only mitigation.
  ⚠ Every copy of the file carries the same down path, so a duplicate
  installed to force a re-run is a file whose revert destroys the deployment.

- `examples/product-review.json`: **declare a ladder**, and assert that every
  optional kind appears in at least one shipped example.

  ⚠⚠ **A DEFINITION THAT SHIPS IN `examples/` IS EXERCISED, NOT ILLUSTRATED.**
  These files are `include_str!`'d into the test binary, so a kind one of them
  declares gets a real round trip through the shipped bytes. `grades` was the
  only configurable kind with none — which reads as a documentation gap and is
  a **coverage gap wearing documentation's clothes**. Named by the first
  adopter, who checked the load sites before answering.

  A test now grades a record through the referee using the shipped file, and a
  floor in `shipped_examples_stay_valid` fails if any kind stops appearing —
  so the gap cannot return silently the next time a kind is added. Both are
  red when the block is removed.

  ⚠ Added to the file with **one** load site rather than the acceptance
  fixture's twelve: a block added where a dozen tests already assert would
  risk perturbing something else, and a green suite afterwards would not
  distinguish a block that is *exercised* from one that is merely *tolerated*.

  ⚠⚠ The example is deliberately **the ladder whose permissive direction is
  the raise** — confidence must reach `high` before delivery, so raising
  clears the gate and belongs to the human while the reviewer may only lower.
  The familiar shape is the opposite. An example is read as a recommendation,
  so this one illustrates the case an adopter's intuition gets backwards
  rather than the case it gets right.

- `ferrostep-pocketbase`, `ferrostep-ledger`, `ferrostep-sqlite`,
  `ferrostep-cli`: **`doctor` checks a ladder against the values its column
  accepts**, the same check it already made for the state column. Raised by the
  first adopter, who was about to declare such a column.

  ⚠⚠ **THIS IS THE FAULT `doctor` WAS BUILT FOR, ONE COLUMN OVER.** The finding
  that created the tool was a definition naming a state value the select column
  would refuse: it passed every check and failed at the first transition. A
  ladder value the column would refuse did exactly that and failed at the first
  **grade**, while the instrument built to catch it reported clean — a checker
  whose population is narrower than its subject.

  ⚠ **It was invisible in the state that makes it invisible.** The adopter's
  column is a select whose accepted values match its ladder exactly, so the
  check would have passed the day it was added and stayed passing until someone
  edited either list. "It does not affect us" was not available; only *before or
  after somebody touches one side* was.

  The schema route now reports **one entry per field** — an array where the
  column enumerates its values, `null` where it does not, and no entry where the
  column does not exist. Those are three different facts and the nested
  `Answer` keeps them one match arm apart. `states` is now **derived from that
  map** rather than looked up separately, so the two cannot come to disagree,
  and it stays in the response so an adapter predating `values` reads exactly
  what it read before.

  ⚠ **A behaviour change requiring a deliberate install**: the answer comes from
  the generated route, so an existing install keeps reporting the ladder
  question `?` (unchecked, non-zero) until the hooks are regenerated. That is
  the intended reading — unchecked is not clean.

  Verified live against a disposable instance, both directions: a ladder value
  outside the column's select is reported as a fault and exits non-zero; a
  ladder the column accepts is counted and shown as an agreement. The live test
  asserts the derivation end to end and is red under two mutations of the
  generated JavaScript — which is the only way to know a generated file
  *computes* anything, as the text suite has now failed to notice three times.

- `ferrostep-pocketbase`: **refuse a grade sent to hooks that predate graded
  attributes**, instead of dropping it and answering success. Found by
  self-review of the commit that introduced grading.

  ⚠⚠ **THE GENERATOR EMITTED THE SIGNAL AND THE ADAPTER NEVER READ IT.** The
  generated ping already advertises `"attributes"` in `writes`; `open()`
  extracted `writes_scope` from that same array and nothing else. So the
  capability check existed on the wire, on one side only.

  ⚠ **The column allowlist could not cover it, and that is not a bug in the
  allowlist.** A file old enough to predate attributes also predates the
  `columns` key, so `writable` is `None` and `refuse_unwritable` returns `Ok` —
  the honest answer to *are these names writable*, and the wrong answer to *can
  this file write attributes at all*. Only the kind check reaches that file.

  Measured against the pre-attribute ping before the fix: the grade was
  accepted, dropped, and answered `version 2` — the version advanced and the
  appended event recorded a grade the row never took. The same defect
  `writes_scope` exists for, one column-kind over, shipped by the commit whose
  own message is about that defect. The reader is now one closure over the
  kind, so a kind added later cannot be advertised and left unread again.

  A behaviour change requiring a deliberate install: grading against hooks
  installed before `e20ffd4` moves from a silent 200 to a refusal naming the
  remedy.

- `ferrostep-core`: **a refusal to open a grade names every role that could
  open it.** Opening is permitted to `raise` ∪ `lower`, and the message
  reported one list — so with an empty `raise` a refused operator was told
  *"grants that to nobody"* while a `lower` holder could do it right then. A
  refusal that hides an existing remedy is worse than a vague one. Roles
  holding both directions are named once.

- `ferrostep-ledger`: restore `decided_scope_updates`'s documentation. Adding
  `decided_grade_updates` *inside* its doc block left the scope function
  publishing bare and gave the grade function fourteen lines about scope. The
  two now sit in order and use the same idiom for their empty case.

- `ferrostep-core`, `ferrostep-ledger`, `ferrostep-sqlite`,
  `ferrostep-pocketbase`, `ferrostep-cli`: **graded attributes** — an ordered
  ladder in the definition, with **each direction granted separately**, and
  `ferrostep grade` to move one. Register entry 3 item B, the successor the
  owner sequenced behind `doctor`. Design written down first in
  `notes/graded-attributes-design.md`.
  Closes the hole the stopgap could only half-close: a lane's merge gate reads
  a severity grade, and the only thing between a developer and clearing their
  own gate was a **self-declared author flag** in a script whose own comment
  called it *a convention, not a mechanism*.

  ⚠⚠ **The engine has no opinion about which direction is dangerous.** The
  shape this arrived as was *raising is anyone's, lowering is the reviewer's* —
  correct for a gate that blocks at or above a floor. A gate requiring a
  **minimum** ("confidence must be `high` to merge") inverts it completely, and
  an engine that assumed the first would be silently wrong for the second **in
  the direction that grants permission**. A definition names roles per
  direction; `explain` prints that the referee guards who moves the value and
  which way, and has no opinion about which end clears your gate. **No
  threshold is modelled**, deliberately: the moment the engine knows which side
  is blocked it has that opinion back.

  ⚠ Order is the ladder's position, never the value's name, so `p3 p2 p1`
  works. The first grade is **neither a raise nor a lower** — treating an
  ungraded record as sitting on the floor would classify every opening grade as
  a raise and hand it to whoever holds that direction.

  ⚠⚠ **An unread grade is a silently widened permission, and this was measured
  rather than anticipated.** The PocketBase adapter did not read the graded
  column into the snapshot, so every change read as *opening* a grade — which
  any role with either direction may do. **On a live store a worker holding
  only `raise` lowered a finding from `high` to `low` and was answered
  success.** Found by running it; no text assertion here would have. Fixed,
  with a regression test, and the generic collection now refuses grading
  outright rather than writing it nowhere.
  ⚠ SQLite gained a `grades` column **including for files that predate it** —
  `CREATE TABLE IF NOT EXISTS` does nothing to an existing table, and without
  the upgrade path an old ledger opens cleanly and reads every record as
  ungraded.

  ⚠ `doctor` now checks a ladder against the column it needs — the gap it
  previously reported as unavailable "because the engine has no vocabulary for
  attributes". The reverse direction is a **note, not a fault**: a refereed
  attribute with no ladder is the stopgap shape, and reporting it as broken
  would go red on a deployment that is exactly as its owner intended.

- `ferrostep-pocketbase` (tests): ⚠⚠ **the two authorization controls this
  crate advertises were proven by grep; now they are executed.** An audit of
  every text-level assertion over a generated file — 23 of them — asked which
  claim a *string* can actually support. **Measured: disabling the role binding,
  and separately the direct-write guard, by replacing each condition with
  `if (false)`, left the entire text suite green in both cases.** A security
  control verified by asserting that `const boundRole =` appears in a file is
  not verified.
  New live test covering, on a real store: an account bound to one role
  claiming another is refused by name; a direct write to a refereed column is
  refused; an undeclared column is refused; **none of the three spends a
  version or moves the record**; and — the positive control, without which a
  route that refused everything would pass — the bound account's own role still
  works and the appended event records the **account's** role rather than the
  request's. Verified to go red under each mutation above and green when
  restored.
  ⚠ Four behaviours are knowingly left text-only (`unbound_principal`, the
  release hook's `WRITERS` allowlist, migration idempotency, generic scope
  merge) and **each now says so in its doc, with the reason**. The dangerous
  state is not an uncovered property; it is an uncovered property that reads as
  covered. The tests module says which kind is which and why.

- `ferrostep-pocketbase`: ⚠⚠ **the mapped apply route refuses a column it has
  no branch for, instead of ignoring it** — new wire prefix
  `unwritable_column:`. A mapped file emits one write branch per column name
  the map **declares**, so a name it does not declare has no branch at all —
  not a rejecting one, *none*. **Measured against a live instance,
  2026-08-27:** an undeclared attribute and an undeclared counter both vanished
  from a call that answered **200**, the version advanced, and the appended
  event recorded a `counter_updates` for a column the row does not have. That
  is the history disagreeing with the record, which this crate's contract says
  cannot happen *because the two are written from one value* — true of the
  value, not of what reached the row.
  The refusal runs **before the transaction**, so a refused request spends no
  version and appends no event, and the allowlist it enforces is the **same
  string the ping advertises** (asserted by slicing both and comparing).
  ⚠ Generic deployments are unaffected and that is asserted: counters and scope
  are JSON there, so every name is writable and there is nothing to refuse.
  ⚠⚠ **A behaviour change requiring a deliberate install**: a client sending a
  column the map does not declare moves from 200 to 400. That is the point, and
  it is not free.

  ⚠⚠ **SWEEP FOR PROSE DESCRIBING THE OLD BEHAVIOUR — INCLUDING IN YOUR OWN
  TREE.** A behaviour change to a generated surface makes every comment, note
  and persona that described the old behaviour false *while it still reads as a
  current statement about the system*. The tell is a sentence saying a write is
  "accepted, dropped and answered 200" with no version beside it. Four such
  sentences were in this repo and are now scoped to the file age they are true
  of. ⚠ Reported by the first adopter, who found the same thing in their own
  tree in the sharpest possible form: **a comment written to warn about this
  exact drift was made false by the fix** — the seam reproduced inside the
  warning.

  ⚠⚠ **This was never a version-skew problem, and thinking it was is why it
  survived.** The known shape was *stale install meets newer binary*; this needs
  no skew at all — a current file from a current binary, generated from a map
  that simply does not declare the column. Named by the first adopter reading
  their own earlier finding back at themselves.

  ⚠⚠ **And it is where testing generated JavaScript by its text ran out.**
  Measured by mutation: replacing the check's condition with `if (false)` leaves
  the allowlist and the refusal name in the file, **all forty-five text tests
  pass**, and the route refuses nothing. Whether a check *runs* is a property of
  a runtime. The live test added with it fails under exactly that mutation, with
  the original symptom, and passes when restored.

- `ferrostep-cli`, `ferrostep-ledger`, `ferrostep-pocketbase`,
  `ferrostep-sqlite`: **`ferrostep doctor` — is this definition satisfiable
  against this store?** (owner, 2026-08-27). A definition asserts things about
  a store that nothing checked: its states must be values the state column
  accepts, its counters and scope labels must be columns that exist, and the
  *installed* write path must be able to reach them. The drift ran in the
  direction nothing goes red in — the JSON looks right, the tests pass against
  the JSON, and the disagreement arrives as a refused write on the first live
  transition. Reported by the first adopter at the moment of impact: they added
  a state to a definition whose store keeps its state column as a fixed value
  list, and **every transition into it would have been refused.** It was
  avoided only because they happened to patch the store by hand first, in the
  right order.
  Read-only, and explicit rather than automatic — run it before landing a
  definition change. Backed by `Ledger::store_shape`, so bindings and other
  consumers get the same check rather than reimplementing it.

  ⚠⚠ **An unchecked question exits non-zero, exactly like a fault.** A gate
  that passes because it could not look is the defect this command exists to
  remove, and every cheap design produces one: a check that cannot run has
  nothing to print, so it prints nothing, so the report reads clean. The
  levels are `fault`, `unchecked`, `note` and `agreed` — and the agreements are
  **counted and shown**, because a run that checked nothing also has no
  complaints.

  ⚠⚠ **`Answer` has three variants, not two.** "This column takes any string,
  so no state can be wrong" and "nobody could tell me what this column takes"
  both reduce to *nothing to report* under an `Option` — and one is a verified
  all-clear while the other is an unasked question wearing its clothes. The
  first draft of the type collapsed them, which was the same defect it exists
  to prevent, one level up.

  ⚠ **The checks that need no store run first and run always.** A store that
  cannot answer still leaves the definition-versus-mapping half fully
  answerable, and a counter declared in a definition with no column to spend it
  on is the cheapest, most certain fault here — two files and no connection.

  ⚠ `Ledger::store_shape` is **provided, and refuses by default**: an adapter
  that has not implemented it must not be indistinguishable from one that
  looked and found nothing wrong. Every other spelling of that default — an
  empty shape, an `Ok` with nothing in it — is a value a report can render as
  "no problems found".

- `ferrostep-pocketbase`: **a generated `schema` route**, and it is the one
  route whose answer is not fixed at generation time. The ping states the
  column names the installed file was *written* with; this reads the
  collection at request time, because the collection is the half that moves
  without anybody regenerating anything. ⚠ **Authenticated, where the ping is
  deliberately anonymous** — a collection's field names and the accepted values
  of its select columns are a different disclosure, and widening the anonymous
  route to carry them would have been the easy way to build this.
  ⚠ An installed file that predates the route produces a **refusal naming the
  fix**, not an empty schema. Verified against a live instance before
  shipping, in both directions: present it answers, absent the check goes red.

- `ferrostep-pocketbase` (tests): **two guards were counting authenticated
  routes as a proxy for write routes**, which was exact only while every
  authenticated route was a write route. The first authenticated *read* route
  made both fail on correct output. Rewritten to check the property directly
  and in both directions — every write route binds a role, **and** a route that
  binds no role cannot write — with a floor asserting such a route exists, so
  the second half cannot pass vacuously. ⚠ Both the old check and its first
  replacement matched `const boundRole` as a substring, so `const boundRole2`
  satisfied them; **found by mutation, not by reading.**

- `ferrostep-cli`: **a rescope that moves part of a record's address says what
  it left behind** (owner, 2026-08-27). A record's unit of work is the whole
  tuple of scope labels and every query that finds work filters on it, so
  setting one label and leaving the rest does not relocate the record — it
  leaves it in no consistent unit at all, with each untouched label still naming
  the old one. Measured on the first adopter: one label moved, a tool still
  selecting on the other counted four records its own queue could not act on,
  and would have spent every remaining review before reporting it had not
  converged. Two other tools there filtered on the full tuple and were right;
  **the disagreement between instruments is what surfaced it.**
  ⚠ **A warning, not a refusal, and deliberately.** That partial move was
  legitimate at the time — there was no value for the other label yet — so
  refusing would have gone red on correct behaviour, which is how a guard gets
  switched off. Whether a definition should declare labels as one address, and
  refuse then, is open and belongs with the satisfiability check.

- `ferrostep-cli`: **`--note-file` on every move that takes a reason**, and a
  guard that no accepted flag is missing from the help text. A reason
  containing backticks or quotes could not survive a command line without a
  heredoc — the first adopter hit that posting the comment that reported the
  same defect in their own tooling. ⚠ This repo had already ruled that a commit
  message goes in a file and never in a quoted `-m`, for exactly this reason:
  **the rule existed and the surface did not.**
  ⚠⚠ **Both flags set is a refusal, and so is an unreadable or empty file.**
  Silently preferring one records a reason the caller did not write. And a
  missing path quietly resolving to "no note" would make the engine refuse a
  required-note move for the *wrong cause* — a true message pointing at the
  definition when the fault is the path.
  ⚠ The documentation guard derives its population from `accepted_flags`, so a
  new flag arms it by existing. **A flag the help text never mentions is a flag
  nobody runs** — the same shape as a driver named in three files, in the third
  person, and driven by hand because no line ever showed a reader a command
  they could copy.

- `ferrostep-pocketbase`: **the wire's refusal prefixes are a contract, with one
  derivation.** `CAS_CONFLICT`, `NO_RECORD` and `ROLE_NOT_YOURS` are public
  constants; the generated JavaScript interpolates them and the adapter matches
  on them. They were **two independent literals** — emitted as text, grepped for
  as separate strings, with no test asserting the spellings agreed. Drift would
  have gone unnoticed in the usual direction: the adapter would have stopped
  recognising a conflict, reported a plain transport error, and the caller's
  *re-read and retry* remedy would have vanished with it.
  ⚠ **Why it is a contract and not an internal detail:** a caller has to tell a
  **retryable** refusal from a **denial**, and both arrive as a 400. Adapters in
  other languages key on the same prefixes. The cross-check test asserts them
  against the **generated text**, never a second copy of the spellings.

- `ferrostep-pocketbase`: **the ping states which COLUMNS an installed file can
  write, and the adapter refuses by name.** `writes` answers in kinds — *state,
  counters, scope* — and that granularity was measured wrong. A mapped file
  carries one branch per column name known when it was generated, so a counter
  added to a map afterwards is **accepted, silently dropped, and answered
  200**: the ceiling never fires and the column is never guarded, while `writes`
  still says "counters" and the adapter is told yes. Found by the first adopter
  adding a counter to a live lane; the only thing that caught it was a person
  diffing the generated file before installing.
  ⚠ **A new `columns` key rather than a changed `writes`.** An older adapter
  reading `writes` sees exactly what it saw before, and a newer one finding no
  `columns` knows it **cannot verify** rather than concluding there is nothing
  to write. Fixing a compatibility defect incompatibly would be the same
  mistake twice. Absence is never read as refusal.

- `ferrostep-ledger`: **`LedgerError::Unsupported` now carries `String`**
  (was `&'static str`). ⚠ **Source-breaking for anything constructing it** —
  a literal needs `.to_string()`. The reason is the entry above: every refusal
  was forced to be a fixed sentence, so the one that matters most could not say
  *which* column an installed file was unable to write, only that some column
  was. An adapter's job here is to state capabilities honestly, and a refusal
  that cannot name its subject sends the reader looking for what it already
  knew. Matches `Transport` and `Malformed`, which are owned already.

- `ferrostep-pocketbase`: **`CollectionMap.attribute_fields` — a refereed column
  the engine has no opinion about. A deliberate, labelled stopgap** (owner,
  2026-08-27). A lane can gate on a column that is none of state, version,
  counter or scope: the first adopter's merge gate reads a severity grade, and
  that column sat outside the referee entirely, with only a self-declared author
  flag between a developer and clearing their own gate — *a convention, not a
  mechanism*, as their own docstring said. Listing a column here puts it in the
  guard's refereed set and gives the apply route a branch for it.
  ⚠⚠ **It buys authentication and audit, not authorisation.** The writer becomes
  a token holder rather than whoever typed a name, and the write lands as an
  event — but nothing says who may set which value, or in which direction, and
  for a graded column that is the half that matters, since raising a grade
  cannot clear a gate and lowering it can. The successor is a definition-level
  ordered ladder with directional grants, and it **subsumes this rather than
  replacing it**: rules live in the definition, column names live in the map,
  exactly as a counter's `max` and its column already divide.
  ⚠ **The guard and the write path ship together, and the tests assert them
  together.** Because the refereed list is one derivation, adding the category
  closed the column instantly while the route still could not write it —
  measured in that broken state — which would have made the adopter's grade
  command a documented, unreachable operation.
  ⚠ **No Rust write API yet, and that is the deliberate boundary.** An older
  installed hook meeting a newer map would answer an attribute write with a
  cheerful 200 and write nothing. The adapter's capability check and the write
  method therefore land together in a later change, so that gap never exists.
  The ping advertises `attributes` **only when the map declares some**, so
  deployments without them answer exactly what they answered before.
  `#[serde(default)]`, so maps written before this field keep loading.

- `ferrostep-cli` / `ferrostep-pocketbase`: **`explain --map` hands over the
  sweep to run before closing a column to direct writes.** `guard_refereed_
  fields` closes the refereed columns to every writer at once, and only the
  adopter can enumerate the writers — so the engine now prints the terms to
  enumerate *with*: the columns it owns, whether the guard is on or off, the
  apply route to point a refused writer at, and the sweep.
  ⚠⚠ **The sweep says code AND PROSE, because that is what was measured.** An
  adopter enumerated their lane's four scripted call sites and a second party
  checked the enumeration; the guard's first refusal came from neither list. It
  came from a persona telling an agent to move the state column with a generic
  record-mutation tool — **no call site, no import, and no authentication step
  to grep for**, because the tool server had authenticated already. The third
  instance was a machine-wide skill file, reaching sessions with no lane
  persona at all. Two correct passes over one population that never held the
  writer; the output names the three kinds it still cannot find, rather than
  implying a clean sweep.
  ⚠ Same argument as `explain`'s numbers section one layer out — the engine
  cannot see an adopter's writers and does not pretend to. The guard and the
  printed list share **one** derivation (`CollectionMap::refereed_fields`), and
  a test asserts the list against the generated hook text rather than a second
  copy of the names: two derivations would drift toward *reporting a clean
  sweep*, which is the direction nothing goes red in.

- `ferrostep-cli`: **an unknown flag is refused, not ignored.** `Flags::parse`
  accepted any `--name value` and the code read only what it asked for, so
  anything else was silently dropped. Two ways that bit, both measured
  2026-08-26: a **typo** — `--scpoe branch=main` — quietly widened a scoped
  audit to every record and exited 0; and a **binary older than a flag**
  accepted `--role`, ignored it, and reported *"0 of 12 await a person"*, which
  is correct for the question it actually asked and completely wrong for the
  one asked of it. The second was found by an adopter whose installed binary
  predated the flag by two days.
  ⚠ This is AGENTS.md's generated-files convention arriving at a surface that
  had never been held to it: **an older thing meeting a newer request refuses
  the part it does not understand, rather than accepting and ignoring it.** The
  PocketBase ping's `writes` exists for exactly this reason; a CLI's flags
  outlive the binary that parses them the same way a hook outlives its adapter.
  The refusal names the flag, lists what the subcommand accepts, and says the
  build may predate it — so it doubles as the version diagnostic and no
  separate capability probe is needed.

- `ferrostep-core` / `ferrostep-cli`: **an agent's queue is visible and
  notifiable — it was neither.** `awaiting` and `notify` both selected on
  "does this need a person", so a record handed from a reviewer back to a
  developer appeared in no listing and rang no doorbell. It is `Status::Live`,
  which is true and useless: `Live` says *some* automated role can act and
  never which, so a loop with two agent roles has two queues and the
  enumeration could see neither. ⚠ **In a worker/reviewer loop that handover
  is the ordinary case, not an edge** — the actor whose turn it now was had no
  way to find out except by querying the database directly, which is the thing
  these surfaces exist to replace.
  `Engine::awaits(snapshot, role)` answers *whose turn is it*, asked of one
  role, and `--role` on both subcommands scopes them to it. Without the flag
  they behave exactly as before, so the person-scoped question B2 was built
  for is unchanged. ⚠ An **exhausted** move is not a turn: a role whose every
  option would route the record away is not waiting, and reporting otherwise
  sends an actor to do work the referee is about to refuse.

- `ferrostep-pocketbase`: **`CollectionMap::guard_refereed_fields` — the
  refereed columns can be closed to direct writes.** The engine is consulted,
  not in the write path, so a client holding credentials could edit `state` or
  a counter straight on the row: no version bump, no event, and every later
  compare-and-swap arguing about a number that moved behind it. With the guard
  on, those columns change through the apply route or they do not change.
  ⚠ **It is a hook rather than an access rule because an administrator
  bypasses rules and does not bypass hooks** — measured on this backend, and
  the whole reason the placement matters. The route's own writes are internal
  saves that never reach a request hook, so the referee is unaffected; only a
  direct edit is refused. Registered *ahead of* the release hook, since
  handlers chain and a guard running second would refuse the release it exists
  to permit.
  ⚠ **Off by default**, like `ActorBinding::allow_unbound`, because on is a
  behaviour change for a running deployment — and it is not free: a console
  hand-edit of a counter stops working too, leaving the release hook and the
  routes as the operator's supported path. The guarded columns are derived
  from the map, so a counter or scope label added later is covered because it
  is declared, not because somebody remembered.
  ⚠⚠ **Before turning it on, audit the personas as well as the code.** The
  first adopter enumerated their lane's four scripted writers, checked from
  both sides, and the guard's first refusal came from none of them — it came
  from prose telling a reviewing agent to move `state` with a generic
  record-mutation tool. That write path has no call site, no import and **no
  authentication step to grep for**, so every search that finds a scripted
  writer misses it. ⚠ Worse than the refused write: an agent reports what it
  *concluded*, and a persona that also says "an unreachable tracker means the
  findings are lost" can turn one refused field into an abandoned review.
- `ferrostep-roster`: **layered rosters and a credential *source*.** Discovery
  collected the first `config.yaml` above the working directory and stopped;
  it now collects every one and layers them, nearest last. That is what lets a
  workspace of several repos share values from a file above them while each
  repo overrides what it needs. ⚠ **`agents` merges per title and takes an
  entry whole** — never field-merged, because half an identity assembled from
  two files is worse than either of them complete — and **`auth` is replaced
  as a block**, so a `type` from one file can never meet a `path` meant for
  another.
  ⚠⚠ **Every value resolves its relative paths against the file that WROTE
  it**, not against the nearest file and not against the working directory. An
  entry inherited from a workspace roster names a persona beside *that* file.
  Getting this wrong is close to invisible: in a layered tree the wrong join
  frequently lands on a real file, so the actor loads somebody else's persona
  and nothing fails.
  `auth` names a **type** with a path — `simple` to begin with, a file of
  credentials keyed by the identity in the roster's `email`. An unrecognised
  type is a refusal naming the file rather than an ignored block, because a
  deployment that believes it configured something and got nothing is the
  failure worth preventing.
  ⚠⚠ **This crate never reads the secret.** `agent-env` emits where credentials
  live and which identity to look up; the lookup is the caller's. A password
  put in the environment is inherited by every subprocess — including one
  launched to act as a *different* actor, which authenticates as whoever
  spawned it while everything appears to work. `--format json` is the
  inheritance-proof path: a caller reads it from a pipe and exports nothing.
  Absent rather than empty when unconfigured, so a consumer under `set -u`
  fails loudly instead of authenticating as nobody.
- `ferrostep-pocketbase`: **role-scoped actors — the write routes stop
  believing the request about who is asking.** Every route authenticated and
  then wrote `role` straight from the request body, so any authenticated
  caller could act as any role. That is invisible while every actor shares one
  credential and is the entire point once they do not. The acting role is now
  read from the **authenticated principal**, and a request claiming a
  different one is refused by name.
  ⚠ **Bind, don't mint** (`docs/prior-art.md` §requirement 9). `ActorBinding`
  names an auth collection the deployment already has and one field on it; it
  creates no identities and is not an account store. The store authenticates
  whoever it authenticates — a password, an OAuth provider, a directory
  federated behind it — and the only thing read here is which role that
  principal may act in. Owning accounts would mean enumerating the actors when
  the loop is designed, and the actors are exactly what a deployment cannot
  enumerate up front: an agent nobody foresaw should be a new principal in a
  directory that already exists plus one row naming its role.
  **Defaults work on a stock instance** — a `ferrostep_actors` auth collection
  with a `role` field, created by the migration only when absent, superuser-only
  to read. `allow_unbound` is `true` by default and that is a *transition, not
  a position*: a deployment with no actors yet authenticates as an
  administrator, so refusing unbound principals on install would be an outage.
  Set it `false` once your actors exist — from then on a principal with no role
  cannot move a record even holding administrator credentials, which is why the
  check lives in a hook rather than an access rule.
- `ferrostep-pocketbase`: **a generated history no longer outranks the records
  it describes.** The mapped migration created its events collection with an
  authenticated-user read rule — matching in the generic shape, which creates
  *both* collections and where the two therefore agree by construction, and
  wrong in the mapped one, where the refereed records are a collection the
  adopter already had under rules commonly stricter. That shipped an inversion
  by default and in silence: every state change, actor, role and human note
  about records the reader may not open, in a collection any authenticated
  account may list. The mapped shape now creates it **superuser-only**, which
  is the only end of the range that is right whatever the adopter's rules say;
  widening is their deliberate act in the admin UI, and the create-if-absent
  guard means a later regeneration will not undo it. `events_collection_body`
  is strict for the same reason and a stronger one — it is handed a name and
  nothing else, so it can know even less about what it sits beside.
  ⚠ The general shape, and it is not about access rules: **generated output
  that attaches to something the adopter owns cannot carry a constant.** The
  invariant here is relational — *no more visible than its subject* — and one
  value was shipped for two situations, then tested in the one where it holds.
- `ferrostep-cli`: `file` (also spelled `create`) — the way into a ledger.
  `authorize_create` had been in the engine, the Python binding and both
  adapters since 0.1.0, and was reachable from the person-facing surface
  nowhere: a store with a console of its own can be handed a record without
  the referee ever being asked, and **SQLite has no console to hide behind**,
  so the deployment shape the roadmap calls first-class could not get a first
  record in short of writing a program. ⚠ A filing ceiling is **measured
  against a count this binary cannot take** — it bounds a branch or a cycle
  rather than the record being filed — so it is passed with `--counter` and a
  missing one is a refusal naming the remedy. Defaulting it to zero would mean
  every filing ceiling silently never fires, which is a guard reporting
  success having checked nothing.
- `ferrostep-cli`: `explain` now says who may file and what filing costs, and
  says **"nobody"** out loud where a definition grants it to no one. Filing is
  default-deny like a rescope, and a heading that is not there leaves a reader
  to work that out for themselves.
- `examples/product-review.json` grows a `creation` block, so the permission
  has an illustration and the guard that keeps `examples/` honest validates
  one. Its filing ceiling is the shape the field exists for: every record
  individually bounded, the population bounded separately.
- `ferrostep-py`: `authorize_rescope`. The core, both ledger adapters and the
  CLI grew rescope; the binding did not, and nothing said so — which left the
  Decision JSON contract a strict superset of what a first-class binding could
  produce, and left a Python consumer wanting to move a record between units
  of work with the raw store write as its only option. That is precisely the
  hole rescope exists to close, still open in one language. No counters are
  asked for, because a rescope spends nothing.
- `examples/product-review.json` grows a `rescopes` rule: a review belongs to
  a release line, and only the owner may move it to another, with a reason.
  The newest concept in the engine had no illustration, and the guard that
  keeps `examples/` honest had never validated a definition carrying one.
  `review-loop.json` deliberately keeps none, so the pair also shows what
  absent means — nobody may, rather than anybody.
- `ferrostep-cli`: `explain` no longer panics on a maximal ceiling. It prints
  each asserted number *and its off-by-one neighbour*, and computed that
  neighbour with `max + 1` — which overflows at the top of the range: a crash
  in a debug build, and in release a wrap to `0` that would have sent the
  reader hunting their tree for the wrong number entirely. A ceiling is a
  value out of a file somebody else wrote, and this is the subcommand whose
  audience is a person who has not got the system working yet.
- `ferrostep-cli`: the audit no longer reads a state change out of a rescope.
  A rescope moves a record between units of work and leaves it in the state it
  was already in, so its event carries `to` equal to `from_state` — which
  satisfied both the escalation test (arrived somewhere halted) and the release
  test (departed somewhere halted) at once. Rescoping a paused record therefore
  reported an escalation *and* a release for a record that had not moved. Not a
  crash and not a visibly wrong number: a **plausible story**, on the one
  surface offered to a person who is deliberately not opening a database
  console to check. Both tallies now ask whether the record moved at all.
- `ferrostep-roster`: the actor roster as a product surface. A deployment's
  `config.yaml` names its agents by title; each entry carries the identity work
  is signed under and the persona document that tells that agent how to behave.
  Titles are configured values and the crate knows nothing by any of them. The
  persona resolves against the roster's own directory and is checked to exist
  before it is emitted, because that path is what a launcher hands to
  `--system-prompt-file`.
- `ferrostep-cli`: `agent-env` — the roster as shell assignments, taking no
  workflow and no store. A repo adopts a roster before it adopts a referee, and
  a repo with no Rust toolchain could not reach the reader at all while it was
  an `xtask` subcommand. Every failure is a refusal with a message rather than
  an empty assignment at status zero: a caller `eval`s this and then commits
  with it, so falling back is how work gets signed under the wrong name.
  `--format json` answers the same resolution for a caller that is not a
  shell, so recovering a name does not require decoding shell quoting in
  another language.
- `ferrostep-cli`: `explain` — what a definition permits, readable without a
  store. Its numbers section exists because of a migrating loop, not taste: when
  a ceiling moves into a definition, FerroStep owns the *value* and knows
  nothing about the *arithmetic derived from it* elsewhere in the adopter's tree
  — `max + 1` in a guard, a range in help text, a sentence in a brief handed to
  an actor. That arithmetic does not contain the value it came from, so a search
  for the ceiling finds none of it. Three times in one migration the search term
  that worked was a number the definition never states, so `explain` prints the
  asserted values *and* their off-by-one neighbours.
- `xtask agent-env` now delegates to `ferrostep-roster` rather than carrying a
  second reader of the same format.
- **Rescope: moving a record between units of work is now a refereed
  operation.** A record's scope decides which queries find it, so a record
  whose scope names a finished unit is invisible to all of them — and until
  now nothing could move one, so consuming loops did it as un-versioned,
  un-evented writes to the field every query depends on. A definition grants
  it per label and per role (`rescopes`), or nobody has it; `ferrostep
  rescope` performs it; it lands versioned and evented like any other move and
  shows up in `audit`. ⚠ Refused on terminal records, and that is not
  configurable: a finished record's scope is the provenance of what it was
  resolved against.
- `Decision::Allow` grows `scope_updates`, omitted from the JSON when empty —
  so a consumer written before rescope existed reads byte-identical JSON for
  everything it already handled, and no fourth `kind` was added for every
  binding to learn.
- `CounterDef` grows `exhausted_requires_note`: the attempt that finds a ceiling
  spent can be required to say what decision is being asked for. Exhaustion
  routes a record to a person, and an automatic route arrives in front of them
  with **no question attached** — which is the whole content of the handover.
  The actor that just ran out of attempts is the one who knows what cannot be
  settled, and that is the moment it knows it. Deliberately not `requires_note`
  on the spending transition, which would tax every attempt when only the last
  one is addressed to anybody; the decision surface still shows where a spent
  ceiling would route, because it offers moves with a note already attached.
- `ferrostep-pocketbase`: the generated ping now states what the installed
  routes can write, and the adapter reads it. Hooks are deployed separately
  from the binary, so a current adapter meets older routes routinely — and
  those answer an apply carrying scope updates with a cheerful 200 while
  writing no label. That is now refused by name, with the remedy in the
  message, instead of being reported as a move that happened. In mapped
  deployments the writable labels are the map's `scope_fields` and nothing
  else, as one generated line per declared label rather than a loop over
  whatever a request names.

## 0.1.0 — 2026-08-24

The internal MVP ([ROADMAP §Releases](docs/ROADMAP.md)): cut on the owner's
judgment after the lane's store was provisioned live and a real record ran
the full refereed cycle — a pass claimed and spent, a genuine design
escalation, the owner's release through the generated hook, and a close —
all of it in the ledger's own history.

- `ferrostep-ledger`: `Scope::matches` and `decided_snapshot` — the one shared
  meaning of "apply this decision to this snapshot".
- `ferrostep-sqlite`: the first ledger adapter. WAL-mode SQLite on one host;
  atomic apply and compare-and-swap by construction, append-only history
  enforced by triggers, all three capability flags earned by tests including
  a repeated-rounds concurrency battery.
- `ferrostep-pocketbase`: the second ledger adapter — a stock instance plus a
  generated migration and transactional apply/create routes (the compare
  inside the store's transaction, the only placement that measured sound).
  Detects at connect time whether the routes are installed and says which
  mode it is in; without them it is read-only and refuses writes by name.
  Live end-to-end loop and concurrency battery ship as ignored-by-default
  tests, run against a real instance.
- `ferrostep-pocketbase`, again: **mapped deployments** — a `CollectionMap`
  referees an existing collection's own columns (state, counters, version
  token, scope labels), so a loop already living in a collection keeps one
  truth and its console view; filing stays with the collection's own
  procedure and is refused by name. Generated routes became
  collection-scoped so refereed collections cannot collide. An optional
  generated release hook makes writing a decision field perform the
  definition's release transition with the referee's bookkeeping (version
  bump, event append) — the store-side transition B5 warns about, as
  generated output instead of a hand-written peer.
- `ferrostep-notify`: the notification message — which record, why, how
  urgently, how to get back — and the `Notifier` adapter boundary, with ntfy
  as the maintained default. Nothing polls or schedules; callers decide when.
- `ferrostep-cli`: the `ferrostep` binary. `awaiting` renders which records
  await a person and what their moves would actually do; `move` resolves one
  without a database console; `audit` reports what happened (moves,
  escalations, releases, last note) from the same enumeration `awaiting`
  reads; `notify` sends one notification per awaiting record.

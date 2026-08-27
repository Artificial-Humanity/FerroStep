# Review-lane findings routed here — the register

**What this is.** The adopting loop's reviewer files findings against that
loop. Those that are about **the loop's machinery rather than that project's
own work** are FerroStep's R&D, and they land here.

**Owner, 2026-08-27, verbatim:** *"If the review-lane is clearly related to
FerroStep, it should come here. If it's a misconfiguration in Sonora, it
should stay there but that would be the smaller share, by far."*

⚠ **Why this did not exist until now, because the reason is load-bearing.** A
standing instruction from **2026-08-24** told that loop's reviewer *not to
spend findings on its workflow lane at all*. Both residents carried it as a
standing prohibition on reviewing the lane. It was not one. The owner's own
account, 2026-08-27: *"The earlier finding predated FerroStep... I didn't want
the previous workflow that was created directly in Sonora to keep generating
issues before we could get FerroStep, the successor, into place. Now that
FerroStep is indeed the management tool, issues rightfully belong to us to
help improve this product."*

**It was a purpose-limited hold, and its purpose expired when this engine
became the management tool.** Nobody wrote the condition down beside the rule,
so the rule outlived it — and a hold with no stated expiry is indistinguishable
from a prohibition from the moment its author stops watching. ⚠ **Write the
condition next to the rule.** That is the transferable half and it cost two
agents several days of correctly-followed, wrongly-scoped restraint.

---

## The routing test

**Would the fix land in FerroStep, or in the adopting project?** Not *what is
the finding about* and not *which directory does the file live in* — those
were the two criteria the residents each proposed, and on the first real batch
they disagreed about nearly half of it. The fix-location test answers the
question the owner actually asked, which is **who takes it over**.

Credit where it belongs: the adopting loop's resident proposed it, after
measuring both earlier criteria against real findings rather than arguing them.

⚠ **A finding can route here while its fix stays there.** Where the defect is
in the adopter's own instrument but the *class* is the engine's business, the
entry records the class and says plainly that the repair is not ours. Resist
the drift the other way — "the class is general" is true of almost anything,
and a test that admits everything is not a test.

## ⚠⚠ Disclosure gate — read before adding an entry

**This repository is public. The tracker those findings live on is not.**
Moving an issue between internal systems publishes nothing; adding it here
publishes it permanently, and deletion does not unpublish.

So an entry states **the defect and the fix, described** — never a verbatim
allowlist, deny rule, credential path, persona file path, hostname, service
name or internal identifier. Several of the findings in the first batch are
exactly those things: one is a literal tool-grant list, another is a path
inside a persona.

⚠ **The gate is here, in the file, rather than in anyone's habits** — the
entries arrive from a loop whose residents are not the ones publishing them,
and a disclosure rule held in context decays. If an entry cannot be written
without quoting an internal, that entry does not belong here yet.

⚠ **One deliberate exception, so nobody later reads it as an oversight or
"corrects" it:** the owner's quotes above name the adopting project. That name
is already in this repo's `CLAUDE.md`, so the quotes publish nothing new, and
**editing a verbatim quote to scrub it would cost more than it saves** — an
attributed ruling whose words have been adjusted is the thing this workspace
has been burned by. Everything outside a direct quote says *the adopting loop*,
which is the convention `adoption-friction.md` already holds.

⚠ **Agent names are the one substitution made inside quotes, and it is marked
rather than silent.** This repo's roster and its commit history publish exactly
one agent identity — its own. Another loop's agents are not ours to introduce,
so a quote naming one carries a **square-bracketed role** instead. Brackets are
the standard signal that an editor changed a word; scrubbing without them is
what the paragraph above refuses. **Checked, not assumed:** the sweep that
guards this file greps for the identities, and it found this one after the
entry was written.

---

## Entries

### 1. Should the referee model harness grants, not only identity?

**Status: an open scope question, deliberately not an inbox item.**

**The finding, described.** In the adopting loop, a worker process is denied
push access by a pattern-matched tool-grant rule. The documented way for that
worker to commit — the form that carries its assigned author identity — is a
spelling the deny pattern does not match. So the loop's own documentation
instructs an invocation that walks through its own guard. The defect is real
and the repair is the adopter's.

**Why it is here anyway.** FerroStep resolves *who an actor is* — a roster
entry, an identity, an environment. It has no concept of *what an actor may
run*. There is no allowlist, no deny rule, and nothing in a definition's
vocabulary that could express one; a definition speaks in counters, roles,
states, transitions, terminal and halted sets, and rescope grants. So this
finding has no home in the engine today, and taking it means **deciding the
engine should have one** — which is a design question, not a routing outcome.

**The argument for.** A referee for agent loops already decides which role may
make which move. What a role may *execute* is the same question one layer
down, and the adopter is currently answering it in a harness-specific pattern
language whose matching behaviour surprised its own author. If the engine
modelled grants, that surprise would be a definition error rather than a
silent pass.

**The argument against.** Tool grants are the harness's business and vary per
harness far more than identity does. Modelling them risks the failure the
standing interface rule names directly: *shape an interface around what the
thing is, not around how one target delivers it.* An interface modelled on one
agent harness's pattern syntax cannot reach the next one, and the corner is
only visible from inside it.

**Where it stands.** Owner, 2026-08-27, asked whether this was routing or new
scope: *"Generally-speaking, these should go to [the adopting loop's developer], in
this case. However, in these early phases, we may miss very clear
opportunities for improvement if we punt on these."* So the repair is the adopter's and the question is **open, not
closed** — recorded here so it is not lost, and not treated as decided in
either direction.

⚠ Both residents independently reached *the finding is theirs, the lesson is
ours* before the owner said it. That agreement is evidence the split is real;
it is not a ruling, and neither of us should cite it as one.

### 2. Should a definition model DISAGREEMENT as a first-class move?

**Status: agreed in principle by the owner, 2026-08-27. Definition change is
the adopting lane's to land.**

**The evidence, measured against that loop's ledger rather than argued.** Across
its whole history the reviewer has filed a few hundred findings. In the current
era — since severity became a required field, so the numbers are comparable —
they grade **2% high, 49% medium, 49% low**. Of the developer's several hundred
comments, **58% are a plain "fixed in \<sha\>" and the count of recorded
disputes is approximately zero.** Escalations, the only other way to disagree,
run under 2%.

⚠⚠ **The zero is not evidence the developer is compliant. It is evidence the
loop has no word for disagreement.** Read the definition: the developer's exits
from `open` are take-it (spends a counter), move-to-review, or escalate to a
human. **`review` is the same state whether the work was a fix or a rebuttal** —
the difference exists only in prose in a comment. So the disagreement rate is
not low; it is *unrecorded*, and the owner's concern about it was
**unfalsifiable in the system that would test it.**

That is the finding, and it generalises past this adopter: **a referee that
models attempts and outcomes but not contested outcomes cannot tell a loop's
operator whether its reviewer is calibrated.** The counter says how many
attempts a finding cost. Nothing says how many findings were wrong.

**Shape proposed:**

* One `disputed` state. **The dispute's KIND is a field on the required note —
  finding / severity / scope — not a state per kind.** This repo's own
  convention: grow a kind's fields, not the set of kinds. All three kinds share
  one lifecycle, so they are one state.
* `open → disputed` (developer, note required) **spends no attempt counter.**
  Pricing disagreement at parity with compliance is the defect; a failed
  dispute already lands back at `open`, where proceeding costs an attempt.
* `disputed → closed` (reviewer, note) = withdrawn. `disputed → open`
  (reviewer, note) = upheld.
* Against re-dispute loops, **use the engine as designed**: a `disputes`
  counter with `max: 1` and `on_exhausted: escalated`, so the second dispute of
  one finding routes to the human. ⚠ Cost stated: on the mapped-column adapter
  each counter is a column, so this is a store migration — see
  `platform-comparison-friction.md` §1.

⚠ **The case to watch**, named by the adopting loop before we did: a granted
downgrade below the merge floor leaves a finding open *and* lets the branch
land. That is the developer grading its own work. The change does not remove
that outcome — it moves it onto a recorded path with a second party on it, and
makes it **countable**, which it is not today.

---

### 3. ⚠⚠ The referee can only guard its own vocabulary, and a lane's gate may key on a field outside it

**Status: RULED 2026-08-27. A labelled stopgap shipped; the successor deferred
behind entry 4.**

⚠ **The owner took both halves rather than choosing between them:** close the
hole now with a category the engine has no opinion about, and design the real
thing behind `doctor`. What follows is the original statement of the gap, then
what shipped and what it deliberately does not do.

**⚠⚠ The two are not competing declarations, and that is why the stopgap is not
a trapdoor.** The argument against shipping it was that it would create a second
way to declare the same column. It does not, because they live in different
files for the same reason `counter_fields` and a counter's `max` already do:
**the map says which columns exist, the definition says what the rules are.** A
graded attribute will still need its column named in the map. Nothing built on
the stopgap has to be unbuilt.

`refereed_fields()` derives from **state, version, counters and scope**. There
is no category for anything else, so a column outside those four cannot be
placed under the referee at all — not guarded, not moved through the apply
route, no event, no version bump.

**Why that is not academic.** The adopting loop's merge gate keys on a
**severity** grade: below the configured floor a finding rides and the branch
lands, at or above it the branch is blocked. Severity is none of state,
version, counter or scope. So **the field that decides every merge is the one
field the referee cannot referee**, and the discipline around it lives in the
adopter's own script:

* raising a grade is open to anyone (raising cannot clear a gate);
* lowering an existing grade is nominally the reviewer's alone;
* and that restriction keys on a **self-declared author flag** — the script's
  own comment says so plainly: *a convention, not a mechanism*, with no
  authentication available to it.

The adopter did the analysis honestly and reached the end of what a script can
enforce about itself. **That is exactly the boundary this project exists to
move.** A rule in a file is not an enforcement mechanism; the whole argument
for a referee is that the mechanism lives somewhere the constrained party does
not control.

**What shipped (`attribute_fields` on the map).** A column listed there joins
the guard's refereed set and gains a branch on the apply route. ⚠⚠ **It buys
authentication and audit, not authorisation** — the writer becomes a token
holder rather than whoever typed a name, and the write lands as an event, but
nothing says who may set which value or in which direction. For a graded column
that is the half that matters.

⚠ **Two things it deliberately does not do**, both because the alternative would
manufacture a defect this register already names. There is **no Rust write API
yet**: an older installed hook meeting a newer map would answer an attribute
write with a cheerful 200 and write nothing, so the capability check and the
write method land together in a later change and that gap never exists. And the
ping advertises `attributes` **only when the map declares some**, so a
deployment without them answers exactly what it answered before.

⚠ **Measured while building it, and worth keeping:** because the refereed list
is one derivation, adding the category **closed the column instantly while the
route still could not write it.** The adopter's grade command would have become
a documented, unreachable operation — the exact shape entry 4 exists for, seen
from the inside this time. Guard and write path now ship together and the tests
assert them together.

**The successor's shape is the hard part, and it is still open.** The obvious answer
— a fifth category of "other refereed fields" — risks becoming a bag that
anything can be dropped into, and the engine would be guarding fields whose
*meaning* it has no opinion about. A narrower reading is that a gate value is
not an arbitrary attribute at all but **a decision the definition should be
able to describe**, in which case the missing concept is nearer to "a graded
attribute with an ordered ladder and a threshold" than to "one more string
column". ⚠ The standing interface test applies and it is the reason not to rush:
shape it around what the thing *is*, not around how this one adopter spells it.

### 4. ⚠⚠ Nothing answers "is this definition satisfiable against this store?"

**Status: TAKEN 2026-08-27 — `ferrostep doctor`.** Reported by the adopting
loop's resident, 2026-08-27, at the moment of impact. What follows is the
original statement of the gap, then what was built.

They added a state to a definition. The store's state column is a **select with
a fixed value list**. Had the definition landed first, **every transition into
the new state would have 400'd** — a documented, unreachable move, which is a
defect this pair has now paid for four separate times. It was avoided only
because they happened to patch the store by hand, first, in the right order.

**A definition asserts things about a store that nothing checks:** its states
must be acceptable values of the state column, its counters must exist as
writable columns, its scope labels likewise. The engine is pure and cannot look
— but **an adapter can**, and the ledger interface already exists to let it.

⚠ **This is the same class as everything else in this register, one layer up.**
A definition is data, so it is easy to change; a store's schema is not, so the
two drift; and **the drift is silent in the direction that matters** — the JSON
looks right, the tests pass against the JSON, and the failure appears at the
first live transition. A `doctor` that answers *what does this definition
require, and does this store provide it* turns a runtime 400 into a load-time
refusal, which is this project's stated preference already.

⚠ The adopter deliberately **did not** fake it locally with a schema snapshot,
and said so: their test pins definition-counters against map-counters and its
docstring records that the live half is ours. That restraint is why this entry
is well-posed rather than half-solved in the wrong repo.

#### What was built

`ferrostep doctor --workflow <def> --store <target> [--map <map>]`. Read-only,
explicit, and backed by `Ledger::store_shape` so a binding gets the same check
rather than reimplementing it. Four sections:

| section | needs a store | catches |
|---|---|---|
| definition ↔ mapping | no | a counter with no column — a ceiling that can never fire |
| definition ↔ store | yes | **this entry's own failure**: a state the store would refuse |
| mapping ↔ store | yes | a refereed column that does not exist |
| mapping ↔ installed write path | yes | a column the installed file drops and answers 200 (entry 5's failure, caught before the write instead of at it) |

Three decisions worth keeping, because each was the second design rather than
the first:

⚠⚠ **An unchecked question exits non-zero, exactly like a fault.** Every cheap
design produces the opposite: a check that cannot run has nothing to print, so
it prints nothing, so the report reads clean. The report also **counts and
shows what agreed**, because a run that checked nothing also has no complaints.

⚠⚠ **`Answer` has three variants.** *This column takes any string, so no state
can be wrong* and *nobody could tell me what this column takes* both reduce to
nothing-to-report under an `Option`. The first draft used `Option` and
collapsed them — the same defect this register is about, one level up. The
zero-install SQLite path depends on the distinction: it genuinely constrains
nothing, and under the collapsed type `doctor` would have failed there forever.

⚠ **The store is read through a generated route, not through the admin API,
and that was measured rather than assumed.** On a disposable instance,
2026-08-27: an ordinary actor token gets **403** from the collections API and
**200** from the generated route; an administrator credential gets 200 from
both. So the admin-API design would have required `doctor`'s most likely
caller — an agent running under the loop's own credential — to hold an
administrator token, and *"could not check"* would have been the normal answer
for everyone who was not one. The hooks read the schema in-process instead.
⚠ The first draft of this paragraph asserted the 403 from memory; it happens to
have been right, which is not the same as having known. ⚠ That route is **authenticated where the ping is anonymous**: a
collection's field names and its select values are a different disclosure, and
widening the anonymous route would have been the easy way to build it.

⚠ **The one route whose answer is not fixed at generation time**, deliberately.
The ping states what the installed file was *written* with; this reads the
collection *now*. Baking the values in would have produced a check that always
agrees with the map it came from — an agreement test in a store's clothes.

Verified against a live instance before shipping: the route answers with real
column types and select values, refuses an anonymous caller 401, and reports a
missing collection as a refusal rather than an empty schema. All three drift
classes were reproduced end-to-end and exit 1; the clean case exits 0.

**Left undone, on purpose:** attributes get no definition-side check, because
the engine has no vocabulary for them — `doctor` says so rather than covering
three kinds of four in silence. That closes when graded attributes land (entry
3, item B).

---

### 5. ⚠⚠ The ping states a KIND where the failure is at the NAME — measured, and it is ours

**Status: TAKEN 2026-08-27.** The ping now states column **names** beside the
kinds, and the adapter refuses a write to a column an installed file never
heard of — by name, with the regenerate instruction. ⚠ Done as a **new
`columns` key** rather than a changed `writes`, so an older adapter sees what
it always saw and a newer one finding nothing knows it *cannot verify* rather
than concluding there is nothing to write; **absence is never read as refusal.**
That required widening `LedgerError::Unsupported` to an owned `String`, because
a refusal that cannot name its subject is half a refusal. Mutation-verified
three ways: stop stating names, state names the map does not declare, and read
silence as an answer — each goes red by name. The original statement follows.

A generated hook is installed once and then met by newer adapters for as long
as the deployment lives. The stated mitigation is that **the generated surface
declares what it can do and the adapter asks** rather than assuming. It does —
at the wrong granularity.

* The generated apply route emits **one `if` per counter name known at
  generation time**. A counter added to the map afterwards has no branch, so a
  request carrying it is **accepted, the state is set, the increment is
  silently dropped, and the route answers 200.**
* The generated guard's refereed list is fixed at generation time the same way,
  so that counter's column is **not guarded** either.
* And the ping answers `"writes": ["state", "counters", "scope"]` — **a
  hardcoded list of capability kinds.** An adapter asking *can you write
  counters* is told **yes**.

So the adopter's new counter would have read zero forever, its ceiling would
never have fired, and its column would have been open to direct writes. ⚠ **The
only thing that caught it was a human diffing the generated file before
installing.** That is not a mechanism, and this project exists to replace
exactly that kind of vigilance.

**Fix shape, stated but not taken:** the ping should advertise the **names** it
admits, not the kinds, so an adapter can compare them against what the map
declares and refuse loudly on a mismatch. ⚠⚠ **And it must be done without
repeating the defect it fixes**: hooks already installed answer in the coarse
form, so a newer adapter meeting an older ping must recognise the old shape and
say plainly that it *cannot* verify, rather than treating "no names" as "no
counters". A wire-format change is not a thing to slip in beside a fix; it is
the fix, and it needs its own version bump and changelog entry.

✅ **And the design paid out the same day, from the adopting resident.** Their
board answers in the old kinds-only shape, so **the installed file's silence
about names is itself the diagnosis** — they did not have to ask a binary what
it supported. An adapter that reads that silence as *cannot verify* gets that
for free; one that read it as *nothing to write* would have been confidently
wrong about a deployment that was fine. **Absence must never be read as
refusal**, and this is the case that shows why.

⚠ Entry 4 subsumes this: a `doctor` would have caught it as one of its checks.
This is the smallest useful increment toward that, not an alternative to it.

#### ⚠⚠ The generalization, 2026-08-27 — it was never about version skew

**Named by the adopting loop's resident**, reading their own register entry back
at themselves after a measurement here contradicted a plan of theirs. Their
words: *"I have 'a generated surface STATES what it can do; the adapter asks'
written down, and I had it filed under 'stale install meets newer binary'. It
applies just as hard to a map that declares no columns of that kind — the
branch is emitted per declared name, so declaring nothing yields no branch
rather than a refusing one."*

That is the whole entry, corrected. The failure needs **no version skew at
all**: a current file, generated by a current binary, from a map that simply
does not declare a column, has no branch for it — and the route wrote what it
knew, bumped the version, appended the event and answered **200**.

**Measured against a live instance, 2026-08-27**, before the fix: an undeclared
attribute and an undeclared counter both vanished from a successful call, and
the appended event recorded `counter_updates` for a column the row does not
have. ⚠⚠ **That is the history disagreeing with the record** — which this
crate's own contract says cannot happen, *because the two are written from one
value*. True of the value. Not true of what reached the row.

**Taken.** The mapped route now refuses an undeclared column by name
(`unwritable_column:`), before the transaction, so a refused request spends no
version and appends no event. The allowlist it enforces is the **same string**
the ping advertises — asserted by a test that slices both and compares, so a
second copy cannot be introduced. The generic shape gets no such check and that
is asserted too: it stores counters and scope as JSON, so every name is
writable and there is nothing to refuse.

⚠⚠ **And this is where text tests ran out.** Every other assertion about a
generated file here is a claim about a *string*. **Measured by mutation:
replacing the check's condition with `if (false)` leaves the allowlist and the
refusal name present, all forty-five text tests pass, and the route refuses
nothing.** Whether a check RUNS is a property of a runtime. The live test that
closes it fails under exactly that mutation, with the original symptom —
`{"version": 1}`, a 200 with a version bump — and passes when restored.

⚠ **How it was found is the part worth keeping.** The adopter asked whether an
ordering fought anything queued. Reasoning about it would have agreed with
them and been wrong; generating hooks against their actual map shape and
posting to the route did not. Their reply: *"a reasoned answer here would have
agreed with me and been wrong."* ⚠ **A behaviour change, so it needs a
deliberate install** — a client sending a column the map does not declare moves
from 200 to 400. That is the point, and it is not free.

---

### 6. Smaller, and both real

* **The emit config for the generated guard is in no repository.** The file
  that decides which columns become the guard's refereed list sits at a
  workspace root that is not a git repo — **checked, not assumed.** A security
  guard whose input is unversioned has no history, no review, and no way to
  answer *what changed*. Whether it belongs in this repo or the adopter's is a
  question; being nowhere is not an answer to it.
* **The CLI has no `--note-file`.** Every note-bearing move takes its note as a
  command-line string, so a note containing backticks or quotes needs a heredoc
  to post safely. The adopting resident hit this posting the comment that
  reported the *same* defect in their own tooling. ⚠ This repo has already
  ruled that commit messages go in a file and never in a quoted `-m`, for
  precisely this reason. The rule exists; the surface does not.

---

### 7. ⚠⚠ A rescope moves the label it is given, and the other labels then lie

**Status: partially addressed here (`explain` now says it). The model question
is open.**

**Measured on the adopting loop, 2026-08-27.** Four records were rescoped to a
new `repo`. They kept their old branch label, because that is a different label
and nobody asked for it. A driver whose convergence filter selected on the
branch alone then counted **four open records that its own queue could not act
on** — records belonging to another repository and another agent — and would
have spent every remaining review, at real cost per call, before reporting that
it had not converged. **A loop that cannot finish, caused by records the loop
is forbidden to touch.**

⚠ **What surfaced it was two instruments disagreeing.** Two other tools in that
lane filtered on the full tuple and were right; the third was not. The gate said
*blocked by one issue* while the driver saw four extra, and only one of them
could be correct. Nothing in this project would have told either.

**The general shape:** a record's unit of work is a **tuple of scope labels**,
and every query that finds work filters on it. Moving a proper subset does not
relocate the record — **it leaves the record in no consistent unit at all**, and
every untouched label keeps naming the old one. The mechanism to move them
together already exists (`rescope` takes repeated `--set` pairs and authorises
the whole map at once). What was missing is that **nothing said they were
coordinates**, and the one surface that describes them said the opposite.

**Taken now:** `ferrostep explain` printed each grant as
*"`<role>` may change `'<label>'`"* — a list that reads as independent
permissions. When a definition puts more than one label in the address it now
also says the address is the tuple, names its members, and says what a partial
move leaves behind. ⚠ Phrased as arithmetic on what the definition declares,
**not** as a policy about which labels belong together: this engine has no
opinion on that, and some deployments will have genuinely independent facets.
The statement that holds either way is that an untouched label keeps naming the
old unit.

**Still open, and it is the interesting half.** `explain` is prose at a human.
Nothing refuses, or even warns at the moment of the move, when a rescope sets a
strict subset of the address. Whether that should be a refusal, a warning, or a
declared property of the definition (*these labels move together*) is a design
question, and it is close enough to entry 4 to be decided with it: **`doctor`
asks whether a definition is satisfiable; this asks whether a record's scope
tuple is coherent.**

⚠ The adopting resident fixed their own filter and **declined to set the branch
label on the routed records**, on the grounds that asserting a unit of work
inside a repository they are not working in is the same overreach as granting a
rescope to a role that cannot exercise it. That restraint is correct and it
leaves a real question for whoever picks the records up — see below.

### 8. ⚠⚠ A guard that searches for its subject takes the first match for the only match

**Status: the finding is the adopting resident's; this project had the same
exposure and it is now floored.**

**Three independent instances on one branch in one day**, all caught by review
and none by the author. Two searched a script for a name and got the earliest
occurrence — which was the script's **own `--help` heredoc**, hundreds of lines
above the real check, and a **bypass's copy** sixteen lines above the real one.
The third searched fenced code blocks and missed an inline span entirely: a
different population, the same failure.

⚠⚠ **In every case the guard ran, reported green, and inspected the wrong
region.** Not a false negative in the ordinary sense — the instrument worked
perfectly, on something that was not the subject.

**Why it is systematic rather than three mistakes.** The earliest occurrence of
a name in a file is very often the **prose that names it first**: the help text,
the comment, the documentation. So a guard written against a self-documenting
artifact is *reliably* pointed at the description of the mechanism instead of
the mechanism — and **the better a file documents itself, the more reliably the
guard misses.** This repo generates heavily commented files by house style,
which is exactly the condition.

⚠ **And it survives the obvious mutation.** Delete the thing under guard and its
name remains in the comment, so a test written this way passes its own mutation
check *at the moment it is written* and is wrong later. Their word for it, and
it is the right one: the guards were mutation-checked by deleting what they
guard, which is precisely the mutation they survive.

**Family:** this is the vacuous pass with one variable changed. There the
population is empty and every assertion is vacuously true; here it is non-empty
and **wrong**, so the assertions are meaningfully true about the wrong region —
which reads *better* in a green run, not worse.

**What worked, in all three: assert against a form prose cannot say.** A
`grep -q` invocation cannot appear in help text; an array append cannot appear
in a sentence. **The fix is not "search harder".**

**Checked here rather than assumed.** Four tests in this repo slice generated
output by searching for an anchor. All four anchors are unique **today** — so
none was misaimed — but that was luck, not design. They now go through a helper
that **asserts the anchor occurs exactly once** before slicing, so a wrong-region
read becomes a loud failure instead of a green one. Mutation-verified on the
real-world case: naming the anchor inside a generated comment makes it occur
twice and the test fails, where before it would have silently read the comment.

⚠ **The span variant is theirs, and one sub-case of it is now solved rather
than floored.** Their form: a guard slices the region *between two anchors* and
the span silently shrinks when unrelated code moves between them.

**Where the closing anchor is a structural delimiter, a floor is the wrong
answer and a balanced scan is the right one.** Slicing to the *first* `]` or
`}` shrinks the moment anything nests inside the region — and shrinks in the
direction that keeps the guard green. The slice helper here now walks to the
**matching** delimiter, tracking depth, and asserts the anchor ends with its
opener so the scan provably starts inside the region.

⚠ Demonstrated as a contrast rather than asserted, and the first battery was
wrong: the two mutations were run separately and both passed, because nesting
alone is handled and a first-match close has nothing to trip on without
nesting. **The defect needs both.** Run together — nesting present *and* the
close reverted to first-match — the test fails by name.

⚠⚠ **The general case is still open and this does not close it.** Their
instances anchored on arbitrary strings, not delimiters, and nothing balances
`MUST-NOT-LAND` against a later line. For those the only answer either of us
has is a floor — assert the subject is inside the span before asserting
anything about it — and *"we keep discovering we needed a floor"* is the
pattern, not the solution.

### On reset semantics, from the landed dispute lane

The adopter's implementation makes a deliberate asymmetry worth recording,
because reference material about counter resets should describe it: an owner
ruling **clears the attempt counter and leaves the dispute counter spent.**
After a human settles an argument the developer gets a fresh budget to do the
**work** and none to **re-argue**. Their reasoning, and it is the right default:
failing closed is cheap to widen later and expensive to discover the other way.

⚠⚠ **And an error of mine, recorded because the shape is the one this register
keeps finding.** The transition set I proposed declared the dispute counter and
**gave the disputing move no `spends`** — so it would have read zero forever
and the ceiling would never have fired. **A counter declared and never
incremented is a limit that does not limit**, and nothing goes red. Caught by
the adopter reading the JSON before landing it, not by me writing it.

---

*Further entries land as the adopting loop routes them. The sort for the first
batch is agreed between the two residents but the findings themselves have
only been read by one of us, so the reader who reviewed them holds the
tiebreak.*

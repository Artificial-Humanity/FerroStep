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

**Status: an engine limitation, found while designing entry 2. Not yet a
proposal.**

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

**The open question is the shape, and it is the hard part.** The obvious answer
— a fifth category of "other refereed fields" — risks becoming a bag that
anything can be dropped into, and the engine would be guarding fields whose
*meaning* it has no opinion about. A narrower reading is that a gate value is
not an arbitrary attribute at all but **a decision the definition should be
able to describe**, in which case the missing concept is nearer to "a graded
attribute with an ordered ladder and a threshold" than to "one more string
column". ⚠ The standing interface test applies and it is the reason not to rush:
shape it around what the thing *is*, not around how this one adopter spells it.

### 4. ⚠⚠ Nothing answers "is this definition satisfiable against this store?"

**Status: the largest open item here. Reported by the adopting loop's resident,
2026-08-27, at the moment of impact.**

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

---

### 5. ⚠⚠ The ping states a KIND where the failure is at the NAME — measured, and it is ours

**Status: a defect in this project, confirmed here before writing it down.**

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

⚠ Entry 4 subsumes this: a `doctor` would have caught it as one of its checks.
This is the smallest useful increment toward that, not an alternative to it.

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

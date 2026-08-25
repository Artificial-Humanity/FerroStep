# Adoption friction — what the first migrating loop actually hit

Working notes, not a deliverable. **This is the evidence file for interface
growth.** The north star says the author's loop decides *what* gets built
while other users decide *how it is shaped* — and shaping needs evidence
about what an adopter actually met, or it is done from memory. That is what
lands here.

⚠ **This is not a gate, and it never was one.** An earlier version of this
header said the repo's standing bar was that a feature needs a real consuming
loop behind it. The owner removed that bar on 2026-08-25 (AGENTS.md
§Conventions) — it had never been theirs. **Nothing has to appear in this file
to be worth building.** What a full entry buys is a better argument, not
permission.

The first loop to migrate onto the referee is a hand-driven worker/reviewer
lane on a shared record store, being converted from a script that enforced its
own state machine. Its resident agent is the reporter for everything below;
entries are what was *measured or hit*, not what was anticipated.

⚠⚠ **THIS FILE IS ABOUT FRICTION *FERROSTEP* CAUSES — not defects of the loop
it replaces.** The question every entry answers is: **what did FerroStep do, or
fail to do, that cost an adopter something?** A migrating loop turns up plenty
of its own problems, and those belong to that loop and are none of this
project's business. What is this project's business is the surface an adopter
meets: something FerroStep could not express, made harder than it needed to be,
documented in a way that misled, or silently changed the meaning of.

The distinction is not pedantic. A file that collects both drifts into a
catalogue of somebody else's technical debt and stops being usable as the
evidence bar for growing *this* interface.

⚠ **An entry earns its place by having cost something.** A wish is not
friction. Where an entry led to a change, the change is named so a later
reader can tell which way the influence ran.

---

## 1. The roster was unreachable from the repo that needed it

**Hit:** the reader was a `cargo xtask` subcommand. The migrating repo has no
Rust toolchain and never will. So identity — the thing the migration was
*about* — was a privilege of repos that build this workspace.

**Shape of the lesson:** a surface that only works inside this repo is not a
product surface, however good it is. The test is whether the repo that needs
it can reach it, not whether it works.

**Changed:** `ferrostep-roster` ships; `ferrostep agent-env` is in the binary;
`xtask` delegates rather than keeping a second reader.

## 2. A correct refusal, discarded at the call site

**Hit:** the reader refuses on a missing roster, an unknown title, or an
incomplete entry — exit 1, message on stderr, nothing on stdout. The documented
call-site idiom was `eval "$(ferrostep agent-env)"`, which **returns 0
regardless**: `eval`'s status is the status of the text it ran, and a refusal
emits no text. Measured: `|| die` does not fire, and `set -e` does not stop it.

**Shape of the lesson, and it is the one to generalize:** *the contract was
never wrong.* The defect lived entirely in the example beside it. A tool whose
refusal is correct and whose documented usage throws it away has shipped the
failure it designed against, and no test of the tool can see it.

**Changed:** the persona teaches capture-then-eval and says what it measured.
Worth applying to every `eval`-shaped example this project ever ships.

## 3. Shell-shaped output forced a decoding step on a non-shell caller

**Hit:** the first consumer after the launcher was a stdlib-only Python script
that wanted a name and an address. Handed shell assignments, it would have had
to reimplement shell quoting to recover values the emitter had in hand.

**Shape of the lesson:** an output format is an interface, and the second
consumer is the one that reveals whether the first format was general or just
convenient.

**Changed:** `--format json`, going through the same resolution and the same
persona check so the two cannot disagree.

## 4. The operation nobody could perform

**Hit:** moving a record between units of work had **no mechanism at all**, and
the resident reported doing it as a raw store write **four times in one
session**. Scope decides which queries find a record, so this was un-versioned,
un-evented editing of the field every query depends on.

⚠ **Attribution, corrected by the reporter, and the correction matters because
these entries drive design:** the hole did not start here. The loop's own
script had no such operation either, so the raw writes predate any contact with
this project. **FerroStep inherited the gap rather than creating it.**

**What is squarely ours, though, and is the entry's real content:**
1. **We modelled the loop and reproduced the hole.** Handed a loop with an
   unexpressible operation, the definition simply did not mention it — and
   nothing in the engine noticed that the model was smaller than the thing
   modelled. See entry 6; this is the same failure with a different face.
2. **We then ruled, on our own reasoning, not to add it** — and the ruling was
   reversed hours later by this adopter's evidence.
3. **The referee concentrated the risk.** Every *modelled* operation became
   safe, which made the unmodelled one the only place risk could accumulate.
   Adopting a referee therefore makes an unmodelled operation *more* dangerous
   than it was before, not less, and nothing warns you.

⚠⚠ **And here is what that feels like from inside the loop, which is the half
that makes it actionable** — the reporter's words, and not something this side
of the boundary could have observed: **the raw writes felt SAFER than they
were, precisely because everything around them was refereed.** The mechanism
explains where the risk goes; this explains why nobody looks. An operator
surrounded by careful machinery stops asking whether the one uncareful thing is
dangerous, because the *setting* reads as careful. A referee therefore buys
safety and spends some of it on complacency, and an adopter should be told so
in as many words.

⚠ **Ask a migrating loop what it does that the definition cannot express.**
That question found this; a review of the definition never would have, because
the definition is exactly where the answer is missing.

**Changed:** rescope, refereed. Also reversed a same-day ruling — see
`rescope-design.md`, where the revisit criterion was met by exactly this
evidence.

## 5. Automating a step deleted the reason the step existed

**Hit:** the lane's ceiling worked by *refusing* the fourth attempt, after
which a person escalated by hand **with a mandatory comment stating the
decision being asked for**. Modelled naively, the ceiling auto-routes to the
escalation state — correct, and it produces a record in front of a person with
**no question attached**.

**Shape of the lesson, and it generalizes past this feature:** converting a
manual step into an automatic one can silently drop an obligation that lived in
the manual step. The obligation was never in the state machine; it was in the
procedure the state machine is replacing. ⚠ **When modelling an existing loop,
ask what the humans do around each step, not just what each step does.**

**Changed:** `CounterDef::exhausted_requires_note` — the attempt that finds a
ceiling spent can be required to carry the question. Mechanism rather than
vigilance, which is the whole argument for a referee.

## 6. Authoring a definition is a step FerroStep offers no help with

**Hit:** the lane definition was written by hand from the loop's workflow map —
its states, its diagram, its prose — and came out **one transition short** of
what the loop actually permits. The definition would have silently removed a
real path. It was caught by a reader who went to the enforcing code instead.

**What this is FerroStep's problem:** adoption *begins* with authoring a
definition, and that is the one step where the project currently offers
nothing — no scaffold, no import, no diff against an existing loop, and no way
to be told "your definition permits less than your loop does". The engine
validates a definition for internal coherence and cannot say a word about
whether it matches the loop it claims to describe. So the first thing an
adopter does is the thing with the least support and the quietest failure.

⚠ **The failure mode is a NARROWING, and a narrowing has no advocate.** A
definition that permits too much shows up as a wrong move being allowed. One
that permits too little shows up as a path nobody happens to need this week —
and during a migration, a removed path looks like tidying.

**Not changed yet, and worth deciding on:** a `ferrostep lint`-shaped answer
(replay a loop's existing history against a candidate definition and report
every past event the definition would now refuse) would have caught this
mechanically. That is real work and there is exactly one data point, so it is
recorded here rather than built.

**Changed for now:** nothing in the engine. The narrowing was put to the
operator as a decision and reinstated — which is the practice this suggests to
adopters, not a mechanism.

## 7. Generated files outlive the binary that meets them

**Hit:** hooks generated at one version, met by a newer adapter sending a field
they predate. The old route accepted the request, ignored what it did not
understand, and answered **200 with a fresh version** — reporting a move that
never happened.

**Shape of the lesson:** anything this project *generates and installs* is a
compatibility boundary, and it is the boundary nobody thinks of as one, because
it is our own code on both sides.

**Changed:** generated pings state what they can write; the adapter reads it at
connect time; an apply it cannot honour is refused by name with the remedy in
the message. Now a convention in AGENTS.md.

## 8. Adopting the roster creates a second home for a value the loop already had

**Hit:** the roster arrives as a *new* place identity lives, while the loop's
existing settings file already held the same four keys and its script read them
from there. FerroStep offers no story for that overlap: it says where identity
should live and is silent about the copy already in the adopter's tree.

**What this is FerroStep's problem, narrowly:** the project's own standing rule
is that a configurable value lives in one place and prose points at it. The
roster is how FerroStep applies that rule to identity — and the act of adopting
it *creates* a second copy, for however long the migration takes. The rule and
the adoption path point in opposite directions during the window.

**Not changed:** the honest answer may be that this is inherently the adopter's
to sequence (the keys leave in the same commit the roster arrives, which is
what happened here and cost nothing). Recorded because the *next* adopter will
meet it too, and because "FerroStep tells you where a value goes but not how to
move it" is a documentation gap even if it is not a code one.

## 9. The roster cannot identify the commit that installs the roster

**Hit:** landing the change that *introduces* `config.yaml`. On the base branch
there is no roster yet, so `agent-env` correctly refuses — but the identity it
refuses to supply is the one needed to author the very merge commit that
delivers the roster. **Every first adopter meets this on their first landing.**
Cost: one failed merge and the diagnosis behind it.

**Checked before reporting**, which is the right instinct: is the refusal
wrong? No. Refusing when there is no roster is exactly correct, and a fallback
would be the fail-open behaviour this whole surface exists to prevent.

**So this is a guidance gap, not a code defect** — which does not make it less
of an adoption cost, and it is the kind that lands on **every** adopter exactly
once, at the least forgiving moment. The working pattern, from the adopter:

```sh
git merge --no-ff --no-commit <branch>   # roster is now in the working tree
env="$(ferrostep agent-env)" || exit 1; eval "$env"
git -c user.name="$AGENT_NAME" -c user.email="$AGENT_EMAIL" commit
```

⚠ **Generalizes past merges:** the roster is resolved from the working tree, so
*any* first landing has a window where the tool that assigns identity is not
yet installed. Bootstrap belongs in adopter-facing documentation, which this
project does not yet have.

**Not changed in code, deliberately.** Do not "solve" this by teaching
`agent-env` to read a roster out of git — that is cleverness bought at the cost
of the one property this reader has, which is that it tells you exactly which
file it read.

## 10. Both spellings of "help me" failed

**Hit:** `ferrostep awaiting --help` answered **"--help needs a value"**, and
`ferrostep help awaiting` answered **"unexpected argument"**. Usage printed only
on some *other* misuse. Cost: a round trip — small, and reported only because
the standing ask says to report what gets absorbed silently.

**Why it is worth an entry despite being trivial:** every flag in this CLI takes
a value, so the parser treated `--help` as a flag missing its argument. That is
internally consistent and completely wrong from outside. **The failure lands on
someone who has just admitted they do not know how the tool works** — the worst
possible moment for the tool to be clever instead of plain. It is also
invisible from inside: nobody who knows the commands ever types `--help`.

**Changed:** `--help`, `-h`, `help`, and `help <subcommand>` all print usage and
exit 0, from any position, without needing a workflow or a store. A test asks
all six spellings, because one that only fixes the reported spelling would leave
the other one for the next person.

## 11. Our own lead paragraph said "enforced"

**Hit:** a public write-up drawn from this project's material claimed a ceiling
was "now **enforced**" in one paragraph and that the engine is "consulted, not
enforcing" in another. Caught in review — but the interesting part is where it
came from. **It was inherited, not invented.** The README's opening pitch said
the loop's rules are "defined once, validated, and **enforced** consistently",
56 lines above the section that says a caller could skip the engine entirely.

**Why this is ours and not a writing problem:** the word doing the honest work
there was *consistently* — one implementation, applied the same way everywhere.
But *enforced* is the word that lands, and a reader who skims the pitch stops
with it. We wrote the caveat carefully and then undercut it in the first
paragraph anybody reads.

⚠ **The test that caught it, from the adopter, and it is worth keeping:** an
internally incoherent claim survives every reader who likes the story, because
each half reads fine on its own. It is caught by refusing to read the halves
and instead demanding **the end state, singular** — one machine, one answer:
*does the ceiling stop a writer that does not ask?* No. So "enforced" is the
wrong word wherever it appears about the engine, however well the surrounding
paragraph is qualified.

**Changed:** "enforced" → "applied" in the README's lead. The layered claim was
already correct everywhere it was stated deliberately — the "What FerroStep is
not" section and the north star's "the engine alone is advisory and the docs
say so plainly". The defect was only ever in the pitch, which is exactly where
it does the most damage.

**Open:** the heading "What the engine guarantees" is defensible — the bullets
under it are things the engine does guarantee about its own answers — but it is
the same word class and worth a second opinion.

## 12. The referee was a REGRESSION at the moment the number mattered

**Hit:** `ferrostep move` reported the record's new state and version but **not
the counter it had just spent**. The hand-rolled tooling being replaced printed
the arithmetic — *"agent_passes 0 -> 1"* — so confirming a spend became a second
read against the store. The adopter worked around it silently with their old
tool before deciding it was worth reporting.

⚠⚠ **This is its own friction shape and the file should name it: not "FerroStep
lacks X" but "FerroStep is WORSE THAN WHAT THEY ALREADY HAD."** A migration is
judged against the thing it replaces, not against nothing, and a capability the
old tool had is one the adopter has *paid* for losing. Those are the entries
most likely to go unreported, because losing something feels like the price of
moving rather than a defect.

**And the specific loss was badly chosen.** Spend-on-entry is this project's
signature guarantee; the moment an operator most wants to see the number is the
moment the referee had stopped printing it.

**Changed:** the move reports `agent_passes 0 → 1`. Old *and* new, so a spend
and an operator's re-arm are told apart by the numbers rather than by a label
the engine would have to invent — the engine deliberately does not know which
of the two a counter update is, and this keeps it that way.

**Counter-entry, recorded for balance because a friction file that only
collects complaints stops being evidence:** the same adopter reported that the
refusal messages needed nothing. *"role 'developer' may not move 'review' ->
'open'"* names the role and both states and was usable exactly as printed. The
split matters — refusal text was designed carefully and holds up; success text
was not designed at all, and that is where the regression was.

## 13. A ceiling moved into the definition; its ARITHMETIC stayed behind

**Hit:** the loop's fix-pass ceiling became `agent_passes.max` in the workflow
definition. Elsewhere in the loop, a launcher carried a hardcoded review
ceiling — **`max + 1`**, wearing a refusal — because three fix passes means up
to four reviews (the one that finds a finding, then one after each pass).
Migrating the value did not migrate the arithmetic derived from it. A targeted
sweep missed it; a broader battery found it, along with three other live
leftovers.

⚠⚠ **The general shape, and it will hit every adopter who moves a ceiling:
FerroStep takes ownership of the NUMBER and knows nothing about its
DERIVATIVES.** Those are not copies of `3` — they are `max + 1` in a guard, a
`1-4` range in a help string, a diagram with four arrows, a sentence in a brief.
**Searching for the literal value finds none of them.** They live in registers
that do not look like configuration, which is exactly why they survive a
migration that was careful about configuration.

⚠⚠ **And the first fix is likely incomplete, which is the part worth
internalizing.** After the derived ceiling was fixed, **two more instances
survived in the same file** — one in `--help` text and one *inside the brief
handed to the reviewing actor*, eight lines from the corrected code. The one
that was found was the one **wearing a refusal**; the ones wearing prose stayed.
That is not carelessness. A refusal announces itself when it fires; a brief
never does, and an actor trusts it precisely because it arrives on the channel
its rules arrive on.

⚠⚠ **THIRD DATA POINT, AND IT SETTLED THE DESIGN.** The report of those two
survivors triggered a wider sweep that found **six more** — including a second
live refusal in another launcher, another passage in the brief, and two stale
claims in a persona sending a reviewer to verify a mechanism that had moved out
from under it that morning. ⚠ **Neither of us enumerated the set by insight.
Only a grep, run after the shape was known, found them** — and the adopter's
own words: *"`3` was never the search term that worked."*

**Changed: `ferrostep explain`.** It prints what a definition permits and, the
point of it, **the numbers the definition asserts together with their
off-by-one neighbours** — *"search your tree for 3 AND for 4"*. The engine
cannot see an adopter's derivatives and should not pretend to; what it can do
is hand over the list to go hunting with, which is the artefact that was
missing all three times. The expensive cousin from entry 6 (replay a loop's
history against a candidate definition) stays unbuilt on one data point.

## 14. The referee's history was more readable than the records it describes

**Hit:** provisioning a mapped deployment creates an events collection from
our generated migration, which hardcodes an authenticated-user read rule on
it. The collection being refereed was the adopter's own, already locked to
administrators only. So the migration delivered, by default and without
saying so, a collection carrying **every state change, actor, role and human
note about records nobody was allowed to read** — readable by any account the
instance would authenticate. Found by inspection days later, not by anything
failing.

**Why this is squarely ours.** A generic deployment supplies both collections,
so the generated rule matches what it created and looks right in testing. The
mapped path is the one where the records' rules are *the adopter's*, and it is
exactly the path where our default cannot know what it is matching. We shipped
one number for two situations and only tested the one where it holds.

⚠ **The shape, and it generalizes past access rules:** *when generated output
sits beside something the adopter already owns, a constant is a guess.* The
generated artifact should be **derived from what it is attaching to** — here,
from the records collection's own rules — because the invariant is relational
("no more visible than its subject"), not absolute. Same family as entry 7:
the generated file is the compatibility boundary nobody inspects, because it
is our code on both sides.

⚠⚠ **And the severity is set by something the referee cannot see.** How bad
this is depends entirely on who can reach the instance and whether it accepts
new accounts — facts that live in the deployment, not in any definition. A
referee that provisions storage inherits a security posture it has no way to
read, so the safe default is the strict one and widening is the adopter's
deliberate act.

**Changed, the generator half, same day:** the mapped shape creates its
events collection **superuser-only** — the only end of the range that is
right whatever the adopter's rules turn out to say. ⚠ **Deliberately not
"copy the records collection's rule":** a rule may reference that
collection's own fields (`author = @request.auth.id`), and a copy of it onto
a collection without those fields is refused on save — so the naive
derivation would trade a silent leak for a broken migration. Widening is the
adopter's act in the admin UI, and the create-if-absent guard means a later
regeneration will not undo it.

**Still open:** a deployment already provisioned under the old default keeps
the lax rule, precisely *because* of that guard — so this fix reaches new
deployments and no existing one. Fixing those is a hand act per deployment.
Role-scoped actor accounts remain ROADMAP B6, at 0.2.0: what an actor may
read and what a history may expose are one question.

---

## Open questions the migration has not answered yet

- **What does a loop do with findings the referee has no opinion about?** The
  severity floor that gates this lane's merges is not a referee concept and
  stays with the loop. Is that the right line, or does a merge gate belong in
  the definition? No evidence yet — nothing has hurt.
- **Filing.** Deliberately refused by the adapter in mapped deployments, so the
  loop keeps its own procedure. That has cost nothing so far, which is itself
  worth recording.
- ~~**Two actors, one credential.**~~ ⚠ **No longer an open question — entry
  14 measured it.** Every actor authenticates as the same privileged identity,
  so "who may do what" is enforced by the definition and not by the store.
  That was recorded as a known gap and *not yet a measured pain*; the read-rule
  inversion is what turned it into one, because "which accounts exist and what
  may they see" turned out to be the question that decided how bad a generated
  default was. Now ROADMAP B6, at 0.2.0.

# Adoption friction — what the first migrating loop actually hit

Working notes, not a deliverable. **This is the evidence file for interface
growth.** The repo's standing bar is that a feature needs a real consuming
loop behind it, and the north star says the author's loop decides *what* gets
built while other users decide *how it is shaped*. Both of those need somewhere
for the evidence to land before it is argued from — otherwise "a real loop
needs this" is a claim made from memory.

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

---

## Open questions the migration has not answered yet

- **What does a loop do with findings the referee has no opinion about?** The
  severity floor that gates this lane's merges is not a referee concept and
  stays with the loop. Is that the right line, or does a merge gate belong in
  the definition? No evidence yet — nothing has hurt.
- **Filing.** Deliberately refused by the adapter in mapped deployments, so the
  loop keeps its own procedure. That has cost nothing so far, which is itself
  worth recording.
- **Two actors, one credential.** Every actor authenticates as the same
  privileged identity, so "who may do what" is enforced by the definition and
  not by the store. Role-scoped accounts are a known gap, not yet a measured
  pain.

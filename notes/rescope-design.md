# Moving a record to a different unit of work — the rescope question

Working notes, not a deliverable: the analysis behind a live tracker
escalation. A policy inherited from the loop this engine generalizes says a
below-severity-floor finding *rides* to a follow-up unit of work when its
own unit merges — it stays open, it does not block, it is picked up later.
"Later" is carried entirely by the record's scope: every query that finds
work filters on it, so a record whose scope names a finished unit is
invisible to all of them. Something must be able to perform that move, and
today nothing can: **scope is written at filing and no operation changes
it.** That is a design decision to make, not a bug to patch.

## The fork

**A — the interface gains the operation.** `Ledger::rescope(record,
changes, …) → Version`: version-guarded, evented, refused on terminal
records (a closed finding's scope is provenance; rewriting it falsifies
which range the finding was found against — the filed issue's own guard).

What it costs: the history's event shape. An event today carries a
[`Decision`], and a rescope is not one — the engine never decides it,
because the engine has no notion of scope at all. Either the Decision JSON
contract gains a kind the engine never emits (muddying what every binding
switches on), or the event shape grows an alternative body (a breaking
change to the history contract), or rescope events fabricate an `allow` to
an unchanged state (a false record of engine involvement). Every variant
spends contract for an operation only one policy needs.

**B — the policy stops needing the move.** At the moment a unit of work
finishes, a riding finding is **closed on its own unit and refiled as a
successor on the follow-up unit**: the close event's note names the
successor, the successor's filing note names the predecessor. Both are
existing operations; nothing changes in any contract.

What it buys, by construction rather than by guard:
- Provenance is untouchable: the original record keeps its scope *because
  it is never rewritten*, closed against exactly the range it was found in.
- The ride is real: the successor lives on the live unit and every query
  finds it there.
- The referee's history covers both ends — a close with a reason, a filing
  with a reason — instead of one mutation event on a single record.

What it costs, said plainly:
- Record identity does not survive the move; the thread is two records
  joined by notes. A reader following the story crosses one link.
- The successor's ceilings start fresh. Defensible — a new unit of work is
  a new context for effort — but it is a semantic change from "the counter
  rides along", and worth ruling on knowingly.

**C — scope moves stay operator hand-edits.** Legitimate for an operator
(a hand-edited row is a designed-for path), but as *the* mechanism it is
the same shape this escalation exists to end: an agent's un-versioned,
un-evented write to a field every query depends on.

## Recommendation

**B**, for the internal MVP: zero interface change, provenance by
construction, expressible today. Revisit A only if successor-filing proves
annoying in real use — that is a consuming-loop's evidence, which is what
this repo requires of interface growth anyway. C stays what it already is:
an operator's prerogative, never an agent's procedure.

## Ruled

**B — close-and-refile** (owner, 2026-08-24, given against this note and
recorded on the tracker record whose escalation asked it). A stays a
demand-gated possibility on real successor-filing pain; nothing pre-builds
it.

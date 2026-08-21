"""Binding-level tests: the same reference loop the Rust core uses as its
acceptance fixture, exercised through the Python surface."""

import json
from pathlib import Path

import pytest

from ferrostep import Engine

# Loaded, not copied. This was a hand-written duplicate of the shipped example
# and it silently fell behind by a whole design change — it still described
# escalation as an ending with no way back, long after the core stopped
# allowing that. Reading the real file means the drift cannot recur, and
# `shipped_examples_stay_valid` in the core keeps the file itself honest.
REVIEW_LOOP = json.loads(
    (Path(__file__).resolve().parents[2] / "examples" / "review-loop.json").read_text()
)


@pytest.fixture
def engine():
    return Engine(REVIEW_LOOP)


def test_claiming_a_pass_spends_the_counter(engine):
    decision = engine.authorize("awaiting_worker", {"agent_passes": 0}, "worker", "working")
    assert decision == {
        "kind": "allow",
        "to": "working",
        "counter_updates": {"agent_passes": 1},
    }


def test_ceiling_routes_to_escalation(engine):
    decision = engine.authorize("awaiting_worker", {"agent_passes": 3}, "worker", "working")
    assert decision == {"kind": "exhausted", "to": "escalated", "counter": "agent_passes"}


def test_worker_cannot_approve(engine):
    decision = engine.authorize("awaiting_review", {"agent_passes": 1}, "worker", "approved")
    assert decision["kind"] == "deny"
    assert "worker" in decision["reason"]


def test_next_moves(engine):
    moves = engine.next_moves("awaiting_review", {"agent_passes": 1}, "reviewer")
    assert [m["to"] for m in moves] == ["awaiting_worker", "approved", "escalated"]
    # None of these spends anything, so all three would fire as offered.
    assert all(m["decision"]["kind"] == "allow" for m in moves)


def test_next_moves_carries_what_a_move_would_actually_do(engine):
    # Same state, same role, budget spent: the move list is identical and only
    # the decision differs. A caller reading the list alone cannot tell.
    spent = engine.next_moves("awaiting_worker", {"agent_passes": 3}, "worker")
    fresh = engine.next_moves("awaiting_worker", {"agent_passes": 0}, "worker")
    assert [m["to"] for m in spent] == [m["to"] for m in fresh]
    assert [m["decision"]["kind"] for m in spent] == ["exhausted"]
    assert [m["decision"]["kind"] for m in fresh] == ["allow"]


def test_status_sees_what_the_state_cannot(engine):
    assert engine.status("awaiting_worker", {"agent_passes": 0}) == "live"
    assert engine.status("awaiting_worker", {"agent_passes": 3}) == "will_escalate"
    assert engine.status("escalated", {"agent_passes": 3}) == "needs_person"
    assert engine.status("approved", {"agent_passes": 1}) == "ended"


def test_structural_defects_raise_at_load_time():
    broken = dict(REVIEW_LOOP, initial="nowhere")
    with pytest.raises(ValueError, match="initial state"):
        Engine(broken)


def test_purpose_is_carried_and_opaque(engine):
    assert engine.purpose == REVIEW_LOOP["purpose"]

    # Opaque means opaque: any string survives, and none is dereferenced.
    rewritten = Engine({**REVIEW_LOOP, "purpose": "a sentence, not a path"})
    assert rewritten.purpose == "a sentence, not a path"

    without = Engine({k: v for k, v in REVIEW_LOOP.items() if k != "purpose"})
    assert without.purpose is None

    # Decisions are identical whichever it holds: it never reaches the referee.
    args = ("awaiting_worker", {"agent_passes": 0}, "worker", "working")
    assert engine.authorize(*args) == rewritten.authorize(*args) == without.authorize(*args)


def test_missing_counters_default_to_zero(engine):
    decision = engine.authorize("awaiting_worker", {}, "worker", "working")
    assert decision["kind"] == "allow"

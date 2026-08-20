"""Binding-level tests: the same reference loop the Rust core uses as its
acceptance fixture, exercised through the Python surface."""

import pytest

from ferrostep import Engine

REVIEW_LOOP = {
    "name": "review-loop",
    "roles": ["worker", "reviewer", "operator"],
    "states": ["awaiting_worker", "working", "awaiting_review", "approved", "escalated"],
    "initial": "awaiting_worker",
    "terminal": ["approved", "escalated"],
    "counters": [{"name": "agent_passes", "max": 3, "on_exhausted": "escalated"}],
    "transitions": [
        {"from": "awaiting_worker", "to": "working", "role": "worker", "spends": ["agent_passes"]},
        {"from": "working", "to": "awaiting_review", "role": "worker"},
        {"from": "awaiting_review", "to": "awaiting_worker", "role": "reviewer"},
        {"from": "awaiting_review", "to": "approved", "role": "reviewer"},
        {"from": "awaiting_review", "to": "escalated", "role": "reviewer"},
        {"from": "working", "to": "awaiting_worker", "role": "operator"},
    ],
}


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


def test_structural_defects_raise_at_load_time():
    broken = dict(REVIEW_LOOP, initial="nowhere")
    with pytest.raises(ValueError, match="initial state"):
        Engine(broken)


def test_purpose_is_carried_and_opaque(engine):
    with_purpose = Engine({**REVIEW_LOOP, "purpose": "notes/north-star.md@main"})
    assert with_purpose.purpose == "notes/north-star.md@main"
    assert engine.purpose is None
    # Decisions are identical either way: the field never reaches the referee.
    args = ("awaiting_worker", {"agent_passes": 0}, "worker", "working")
    assert with_purpose.authorize(*args) == engine.authorize(*args)


def test_missing_counters_default_to_zero(engine):
    decision = engine.authorize("awaiting_worker", {}, "worker", "working")
    assert decision["kind"] == "allow"

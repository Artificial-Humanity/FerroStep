"""FerroStep: data-driven state-machine referee for multi-agent loops.

The Rust core decides; your application persists. Read a ledger record, ask
:meth:`Engine.authorize` whether a transition is legal, then write what the
decision instructs — the state flip and any counter updates in ONE atomic
write. Splitting that write breaks the crashed-pass-still-costs-one guarantee.
"""

from __future__ import annotations

import json

from ferrostep._core import Engine as _CoreEngine

__all__ = ["Engine"]


class Engine:
    """A validated workflow, ready to referee transitions.

    Raises ``ValueError`` on construction if the definition has a structural
    defect (unknown states, dead ends, exits from terminal states, …), so
    every error surfaces once at load time rather than mid-loop.
    """

    def __init__(self, workflow: dict) -> None:
        self._core = _CoreEngine(json.dumps(workflow))

    @property
    def workflow(self) -> dict:
        """The validated, normalized workflow definition."""
        return json.loads(self._core.workflow_json())

    @property
    def purpose(self) -> str | None:
        """The workflow's stated *why* (or a pointer to the document holding
        it). Opaque to the engine — surfaced here so briefing code can hand it
        to review-role actors. ``None`` when the definition names none."""
        return self.workflow.get("purpose")

    def authorize(
        self, state: str, counters: dict[str, int], role: str, to: str
    ) -> dict:
        """May ``role`` move a record in ``state`` to ``to``?

        Returns one of::

            {"kind": "allow", "to": ..., "counter_updates": {...}}
            {"kind": "exhausted", "to": ..., "counter": ...}
            {"kind": "deny", "reason": ...}

        On ``allow``, persist the state flip and ``counter_updates`` in one
        atomic write. On ``exhausted``, route the record to ``to`` (the
        counter's escalation state) instead; change no counters.
        """
        snapshot = json.dumps({"state": state, "counters": counters})
        return json.loads(self._core.authorize_json(snapshot, role, to))

    def next_moves(self, state: str, counters: dict[str, int], role: str) -> list[dict]:
        """Every move ``role`` could attempt from ``state``, each carrying what
        it would actually do.

        Each entry is the transition's own fields plus ``decision``, holding
        exactly what :meth:`authorize` would answer for that move right now::

            {"from": ..., "to": ..., "role": ..., "spends": [...],
             "resets": [...], "decision": {"kind": "exhausted", ...}}

        Read ``decision`` before offering the move to anyone. A transition that
        exists and a transition that would fire are different facts, and the
        difference is invisible in the rest of the entry.
        """
        snapshot = json.dumps({"state": state, "counters": counters})
        return json.loads(self._core.next_moves_json(snapshot, role))

    def status(self, state: str, counters: dict[str, int]) -> str:
        """What can happen to this record next, as a whole.

        One of ``"ended"``, ``"needs_person"``, ``"will_escalate"`` or
        ``"live"``. Derived from what the moves would do rather than from which
        state the record sits in — which is the only way ``"will_escalate"``
        can be seen at all, since nothing about the state itself says a record
        has run out of budget.
        """
        snapshot = json.dumps({"state": state, "counters": counters})
        return json.loads(self._core.status_json(snapshot))

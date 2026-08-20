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
        """The transitions ``role`` could attempt from ``state``.

        Advisory: each still goes through :meth:`authorize` to commit, which is
        where ceilings are checked.
        """
        snapshot = json.dumps({"state": state, "counters": counters})
        return json.loads(self._core.next_moves_json(snapshot, role))

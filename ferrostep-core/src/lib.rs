//! FerroStep core: a pure, IO-free state-machine referee for multi-agent loops.
//!
//! The engine owns no state and opens no connections. The application layer holds
//! a ledger (PocketBase, SQLite, Postgres, …); it reads one record into a
//! [`Snapshot`], asks [`Engine::authorize`] whether a transition is legal, and
//! persists what the returned [`Decision`] says — the state flip and any counter
//! spends in one atomic write. Because the engine is consulted rather than in the
//! write path, real enforcement of "only the reviewer may close" belongs in the
//! database's own access rules; the engine is the single place that logic is
//! *defined*, so the two can be generated from the same [`WorkflowDef`].
//!
//! Invariants the design encodes, each learned from running real agent loops:
//!
//! * Workflow definitions are **data**, loaded and validated at runtime — never
//!   compiled-in enums. A new workflow must not require a recompile, a wheel
//!   release, or a redeploy of every consumer.
//! * Counters spend on **entry** to work, not on completion: a pass that crashes
//!   halfway has already been paid for, so a crash loop cannot become an infinite
//!   loop. See [`TransitionDef::spends`].
//! * The engine never resets or "corrects" a counter. Counter values live in the
//!   ledger and belong to the operator; a hand-zeroed counter is a deliberate
//!   re-arm, not corruption.
//! * Exhaustion is a **routing decision**, not an error: when a ceiling is hit
//!   the engine says which state (typically an escalate-to-human state) the
//!   record must move to instead.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

/// A complete workflow definition: the states a ledger record may occupy, who
/// may move it between them, and the ceilings that bound every loop.
///
/// This is the unit a team writes (as JSON/TOML), validates once with
/// [`Engine::new`], and stores alongside the ledger it governs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDef {
    pub name: String,
    /// Optional statement of *why* this loop exists, or a pointer to the
    /// document that states it (a north-star file at a ref, a mission
    /// statement). Opaque to the engine: carried and serialized, never
    /// interpreted. Briefing code typically hands it to review-role actors so
    /// alignment is measured against a stated source, not tribal knowledge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// Every state a record may occupy, including terminal ones.
    pub states: Vec<String>,
    /// The state a freshly created record starts in.
    pub initial: String,
    /// States where automation stops. No transition may leave them; getting a
    /// record out of one (e.g. re-arming an escalated item) is an operator
    /// action on the ledger, outside the engine's authority.
    pub terminal: Vec<String>,
    /// The actors that may perform transitions ("worker", "reviewer", …).
    pub roles: Vec<String>,
    pub transitions: Vec<TransitionDef>,
    #[serde(default)]
    pub counters: Vec<CounterDef>,
}

/// One legal move: `role` may flip a record from `from` to `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionDef {
    pub from: String,
    pub to: String,
    pub role: String,
    /// Counters spent when this transition fires. Put the spend on the
    /// transition that *claims* work (e.g. `queued -> working`), not the one
    /// that completes it — that is what makes a crashed pass still cost one.
    #[serde(default)]
    pub spends: Vec<String>,
}

/// A loop ceiling. When a spending transition finds the counter at `max`, the
/// engine routes the record to `on_exhausted` instead of allowing the move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterDef {
    pub name: String,
    pub max: u32,
    pub on_exhausted: String,
}

/// A record's current position, as read from the ledger. Counters absent from
/// the map are treated as 0; extra keys the workflow doesn't define are ignored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub state: String,
    #[serde(default)]
    pub counters: BTreeMap<String, u32>,
}

/// The engine's answer to "may `role` move this record to `to`?".
///
/// Serialized with a `kind` tag (`allow` / `exhausted` / `deny`) so bindings and
/// app layers can switch on it without knowing the Rust enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Decision {
    /// Legal. Persist the state flip AND `counter_updates` in one atomic write —
    /// splitting them re-opens the crashed-pass-costs-nothing hole.
    Allow {
        to: String,
        counter_updates: BTreeMap<String, u32>,
    },
    /// The move was legal but a ceiling is spent: route the record to `to`
    /// (the counter's `on_exhausted` state) instead. No counter changes.
    Exhausted { to: String, counter: String },
    /// Not a legal move for this record/role. Nothing to persist.
    Deny { reason: String },
}

/// A defect found while validating a [`WorkflowDef`].
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    DuplicateState(String),
    DuplicateRole(String),
    DuplicateCounter(String),
    DuplicateTransition { from: String, to: String, role: String },
    UnknownInitial(String),
    UnknownTerminal(String),
    UnknownTransitionState { endpoint: String, state: String },
    UnknownTransitionRole(String),
    TransitionOutOfTerminal { from: String, to: String },
    UnknownSpend { from: String, to: String, counter: String },
    UnknownExhaustTarget { counter: String, state: String },
    /// A non-terminal state with no way out: records entering it strand forever.
    DeadEnd(String),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ValidationError::*;
        match self {
            DuplicateState(s) => write!(f, "state '{s}' is defined more than once"),
            DuplicateRole(r) => write!(f, "role '{r}' is defined more than once"),
            DuplicateCounter(c) => write!(f, "counter '{c}' is defined more than once"),
            DuplicateTransition { from, to, role } => {
                write!(f, "transition '{from}' -> '{to}' for role '{role}' is defined more than once")
            }
            UnknownInitial(s) => write!(f, "initial state '{s}' is not in `states`"),
            UnknownTerminal(s) => write!(f, "terminal state '{s}' is not in `states`"),
            UnknownTransitionState { endpoint, state } => {
                write!(f, "transition {endpoint} state '{state}' is not in `states`")
            }
            UnknownTransitionRole(r) => write!(f, "transition role '{r}' is not in `roles`"),
            TransitionOutOfTerminal { from, to } => {
                write!(f, "transition '{from}' -> '{to}' leaves a terminal state")
            }
            UnknownSpend { from, to, counter } => {
                write!(f, "transition '{from}' -> '{to}' spends undefined counter '{counter}'")
            }
            UnknownExhaustTarget { counter, state } => {
                write!(f, "counter '{counter}' routes exhaustion to unknown state '{state}'")
            }
            DeadEnd(s) => write!(f, "non-terminal state '{s}' has no outgoing transition"),
        }
    }
}

impl std::error::Error for ValidationError {}

impl WorkflowDef {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Check every cross-reference in the definition. Returns the first defect
    /// found; a definition that passes cannot make the engine panic or strand a
    /// record in an undefined state.
    pub fn validate(&self) -> Result<(), ValidationError> {
        use ValidationError::*;

        let mut states = BTreeSet::new();
        for s in &self.states {
            if !states.insert(s.as_str()) {
                return Err(DuplicateState(s.clone()));
            }
        }
        let mut roles = BTreeSet::new();
        for r in &self.roles {
            if !roles.insert(r.as_str()) {
                return Err(DuplicateRole(r.clone()));
            }
        }
        let mut counters = BTreeSet::new();
        for c in &self.counters {
            if !counters.insert(c.name.as_str()) {
                return Err(DuplicateCounter(c.name.clone()));
            }
            if !states.contains(c.on_exhausted.as_str()) {
                return Err(UnknownExhaustTarget {
                    counter: c.name.clone(),
                    state: c.on_exhausted.clone(),
                });
            }
        }

        if !states.contains(self.initial.as_str()) {
            return Err(UnknownInitial(self.initial.clone()));
        }
        let terminal: BTreeSet<&str> = self.terminal.iter().map(String::as_str).collect();
        for t in &terminal {
            if !states.contains(t) {
                return Err(UnknownTerminal(t.to_string()));
            }
        }

        let mut seen = BTreeSet::new();
        for t in &self.transitions {
            for (endpoint, state) in [("from", &t.from), ("to", &t.to)] {
                if !states.contains(state.as_str()) {
                    return Err(UnknownTransitionState {
                        endpoint: endpoint.to_string(),
                        state: state.clone(),
                    });
                }
            }
            if !roles.contains(t.role.as_str()) {
                return Err(UnknownTransitionRole(t.role.clone()));
            }
            if terminal.contains(t.from.as_str()) {
                return Err(TransitionOutOfTerminal {
                    from: t.from.clone(),
                    to: t.to.clone(),
                });
            }
            if !seen.insert((t.from.as_str(), t.to.as_str(), t.role.as_str())) {
                return Err(DuplicateTransition {
                    from: t.from.clone(),
                    to: t.to.clone(),
                    role: t.role.clone(),
                });
            }
            for c in &t.spends {
                if !counters.contains(c.as_str()) {
                    return Err(UnknownSpend {
                        from: t.from.clone(),
                        to: t.to.clone(),
                        counter: c.clone(),
                    });
                }
            }
        }

        for s in &self.states {
            let is_terminal = terminal.contains(s.as_str());
            let has_exit = self.transitions.iter().any(|t| &t.from == s);
            if !is_terminal && !has_exit {
                return Err(DeadEnd(s.clone()));
            }
        }

        Ok(())
    }
}

/// A validated workflow, ready to referee transitions. Stateless and cheap to
/// share: every query gets the record's current position via a [`Snapshot`].
#[derive(Debug, Clone)]
pub struct Engine {
    def: WorkflowDef,
}

impl Engine {
    /// Validate `def` and wrap it. All structural errors surface here, once,
    /// rather than mid-loop at three in the morning.
    pub fn new(def: WorkflowDef) -> Result<Self, ValidationError> {
        def.validate()?;
        Ok(Engine { def })
    }

    pub fn def(&self) -> &WorkflowDef {
        &self.def
    }

    /// May `role` move this record to `to`? Pure function of the definition and
    /// the snapshot; the caller persists whatever the decision instructs.
    pub fn authorize(&self, snap: &Snapshot, role: &str, to: &str) -> Decision {
        if !self.def.states.contains(&snap.state) {
            return Decision::Deny {
                reason: format!(
                    "record state '{}' is not a state of workflow '{}'",
                    snap.state, self.def.name
                ),
            };
        }

        let found = self
            .def
            .transitions
            .iter()
            .find(|t| t.from == snap.state && t.to == to && t.role == role);
        let Some(transition) = found else {
            // Distinguish "wrong role" from "no such move" — the caller's log
            // line is the first thing a human reads when a loop stalls.
            let exists_for_other_role = self
                .def
                .transitions
                .iter()
                .any(|t| t.from == snap.state && t.to == to);
            let reason = if exists_for_other_role {
                format!("role '{role}' may not move '{}' -> '{to}'", snap.state)
            } else {
                format!("no transition '{}' -> '{to}' in workflow '{}'", snap.state, self.def.name)
            };
            return Decision::Deny { reason };
        };

        let mut counter_updates = BTreeMap::new();
        for name in &transition.spends {
            // Validation guarantees the counter exists.
            let counter = self.def.counters.iter().find(|c| &c.name == name).unwrap();
            let current = snap.counters.get(name).copied().unwrap_or(0);
            if current >= counter.max {
                return Decision::Exhausted {
                    to: counter.on_exhausted.clone(),
                    counter: name.clone(),
                };
            }
            counter_updates.insert(name.clone(), current + 1);
        }

        Decision::Allow {
            to: to.to_string(),
            counter_updates,
        }
    }

    /// The transitions `role` could attempt from the record's current state.
    /// Advisory: each still goes through [`Engine::authorize`] to commit, which
    /// is where ceilings are checked.
    pub fn next_moves(&self, snap: &Snapshot, role: &str) -> Vec<&TransitionDef> {
        self.def
            .transitions
            .iter()
            .filter(|t| t.from == snap.state && t.role == role)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The reference workflow: a worker/reviewer rework loop with a pass
    /// ceiling that escalates to a human. This is the shape FerroStep exists
    /// to referee, so it doubles as the acceptance fixture.
    fn review_loop() -> WorkflowDef {
        serde_json::from_value(json!({
            "name": "review-loop",
            "roles": ["worker", "reviewer", "operator"],
            "states": ["awaiting_worker", "working", "awaiting_review", "approved", "escalated"],
            "initial": "awaiting_worker",
            "terminal": ["approved", "escalated"],
            "counters": [
                { "name": "agent_passes", "max": 3, "on_exhausted": "escalated" }
            ],
            "transitions": [
                // Claiming the pass is what spends the counter: a worker that
                // crashes mid-pass has already paid.
                { "from": "awaiting_worker", "to": "working", "role": "worker", "spends": ["agent_passes"] },
                { "from": "working", "to": "awaiting_review", "role": "worker" },
                { "from": "awaiting_review", "to": "awaiting_worker", "role": "reviewer" },
                { "from": "awaiting_review", "to": "approved", "role": "reviewer" },
                { "from": "awaiting_review", "to": "escalated", "role": "reviewer" },
                // Operator re-queues a wedged pass; note: no refund.
                { "from": "working", "to": "awaiting_worker", "role": "operator" }
            ]
        }))
        .unwrap()
    }

    fn snap(state: &str, passes: u32) -> Snapshot {
        Snapshot {
            state: state.to_string(),
            counters: BTreeMap::from([("agent_passes".to_string(), passes)]),
        }
    }

    #[test]
    fn reference_loop_validates() {
        Engine::new(review_loop()).unwrap();
    }

    #[test]
    fn claiming_a_pass_spends_the_counter() {
        let engine = Engine::new(review_loop()).unwrap();
        let decision = engine.authorize(&snap("awaiting_worker", 0), "worker", "working");
        assert_eq!(
            decision,
            Decision::Allow {
                to: "working".to_string(),
                counter_updates: BTreeMap::from([("agent_passes".to_string(), 1)]),
            }
        );
    }

    #[test]
    fn ceiling_routes_to_escalation() {
        let engine = Engine::new(review_loop()).unwrap();
        let decision = engine.authorize(&snap("awaiting_worker", 3), "worker", "working");
        assert_eq!(
            decision,
            Decision::Exhausted {
                to: "escalated".to_string(),
                counter: "agent_passes".to_string(),
            }
        );
    }

    #[test]
    fn worker_cannot_approve() {
        let engine = Engine::new(review_loop()).unwrap();
        let decision = engine.authorize(&snap("awaiting_review", 1), "worker", "approved");
        assert!(matches!(decision, Decision::Deny { .. }));
    }

    #[test]
    fn crashed_pass_still_costs_one() {
        let engine = Engine::new(review_loop()).unwrap();
        // Pass 1 claimed (counter -> 1), worker crashes in `working`.
        // Operator re-queues; no refund is offered anywhere.
        let d = engine.authorize(&snap("working", 1), "operator", "awaiting_worker");
        assert_eq!(
            d,
            Decision::Allow {
                to: "awaiting_worker".to_string(),
                counter_updates: BTreeMap::new(),
            }
        );
        // Two more claims spend 2 and 3, then the ceiling routes to escalation
        // even though only two passes ever produced work.
        for expected in [2u32, 3] {
            let d = engine.authorize(&snap("awaiting_worker", expected - 1), "worker", "working");
            assert_eq!(
                d,
                Decision::Allow {
                    to: "working".to_string(),
                    counter_updates: BTreeMap::from([("agent_passes".to_string(), expected)]),
                }
            );
        }
        let d = engine.authorize(&snap("awaiting_worker", 3), "worker", "working");
        assert!(matches!(d, Decision::Exhausted { .. }));
    }

    #[test]
    fn hand_zeroed_counter_re_arms() {
        let engine = Engine::new(review_loop()).unwrap();
        // An operator zeroing the counter in the ledger is a deliberate re-arm;
        // the engine takes the snapshot at face value.
        let d = engine.authorize(&snap("awaiting_worker", 0), "worker", "working");
        assert!(matches!(d, Decision::Allow { .. }));
    }

    #[test]
    fn no_transitions_out_of_terminal_states() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("approved", 3), "reviewer", "awaiting_worker");
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn unknown_record_state_is_denied_not_a_panic() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("limbo", 0), "worker", "working");
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn next_moves_lists_role_options() {
        let engine = Engine::new(review_loop()).unwrap();
        let moves = engine.next_moves(&snap("awaiting_review", 1), "reviewer");
        let targets: Vec<&str> = moves.iter().map(|t| t.to.as_str()).collect();
        assert_eq!(targets, ["awaiting_worker", "approved", "escalated"]);
    }

    #[test]
    fn next_moves_is_empty_exactly_for_terminal_states() {
        let engine = Engine::new(review_loop()).unwrap();
        let def = engine.def();
        for state in &def.states {
            let is_terminal = def.terminal.contains(state);
            let has_moves = def
                .roles
                .iter()
                .any(|role| !engine.next_moves(&snap(state, 0), role).is_empty());
            // An empty move set across every role is the only signal that a
            // record needs an out-of-band write to go anywhere. That signal is
            // worth nothing unless no live state can also produce it.
            assert_eq!(
                has_moves, !is_terminal,
                "state '{state}': terminal={is_terminal}, has_moves={has_moves}"
            );
        }
    }

    #[test]
    fn validation_rejects_dead_ends() {
        let mut def = review_loop();
        def.states.push("oubliette".to_string());
        let err = def.validate().unwrap_err();
        assert_eq!(err, ValidationError::DeadEnd("oubliette".to_string()));
    }

    #[test]
    fn validation_rejects_exit_from_terminal() {
        let mut def = review_loop();
        def.transitions.push(TransitionDef {
            from: "approved".to_string(),
            to: "awaiting_worker".to_string(),
            role: "reviewer".to_string(),
            spends: vec![],
        });
        assert!(matches!(
            def.validate().unwrap_err(),
            ValidationError::TransitionOutOfTerminal { .. }
        ));
    }

    #[test]
    fn validation_rejects_unknown_spend() {
        let mut def = review_loop();
        def.transitions[0].spends.push("mana".to_string());
        assert!(matches!(
            def.validate().unwrap_err(),
            ValidationError::UnknownSpend { .. }
        ));
    }

    #[test]
    fn shipped_examples_stay_valid() {
        // The files in examples/ are illustrations, not standards — but they
        // must never illustrate something the engine would reject.
        for (path, src) in [
            ("review-loop", include_str!("../../examples/review-loop.json")),
            ("product-review", include_str!("../../examples/product-review.json")),
        ] {
            let def = WorkflowDef::from_json(src)
                .unwrap_or_else(|e| panic!("examples/{path}.json does not parse: {e}"));
            Engine::new(def)
                .unwrap_or_else(|e| panic!("examples/{path}.json does not validate: {e}"));
        }
    }

    #[test]
    fn purpose_is_carried_but_never_interpreted() {
        let mut def = review_loop();
        def.purpose = Some("notes/north-star.md@main".to_string());
        let with_purpose = Engine::new(def).unwrap();
        let without = Engine::new(review_loop()).unwrap();
        // Identical decisions either way: the field is opaque to the engine.
        assert_eq!(
            with_purpose.authorize(&snap("awaiting_worker", 0), "worker", "working"),
            without.authorize(&snap("awaiting_worker", 0), "worker", "working"),
        );
        // It round-trips when present and stays absent (not null) when not.
        let json = serde_json::to_value(with_purpose.def()).unwrap();
        assert_eq!(json["purpose"], "notes/north-star.md@main");
        let bare = serde_json::to_value(without.def()).unwrap();
        assert!(bare.get("purpose").is_none());
    }

    #[test]
    fn decision_json_shape_is_stable() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("awaiting_worker", 0), "worker", "working");
        let value = serde_json::to_value(&d).unwrap();
        assert_eq!(
            value,
            json!({
                "kind": "allow",
                "to": "working",
                "counter_updates": { "agent_passes": 1 }
            })
        );
    }
}

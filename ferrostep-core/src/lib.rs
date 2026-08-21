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
//! * The engine never resets a counter on its own initiative. It clears one
//!   only where the definition told it to, via a transition's `resets` — the
//!   operator's instruction, validated up front, not the engine correcting
//!   what it thinks is wrong. Values otherwise live in the ledger and belong
//!   to the operator; a hand-zeroed counter is a deliberate re-arm.
//! * An ending and a pause are different kinds of state. Nothing ever leaves
//!   a `terminal` one. A `halted` one stops automation but must declare the
//!   way back, and only a human role may take it — so "does this need a
//!   person?" is answered from the definition rather than guessed at.
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
    /// Endings. The loop is over here and no transition may ever leave one.
    pub terminal: Vec<String>,
    /// Pauses. Automation stops, but a person may send the record back into
    /// the loop — the state a ceiling escalates to is usually one of these.
    /// Every halted state must declare that way back, and only a human role
    /// may take it; a halt an agent could undo is not a halt. Distinguishing
    /// these from [`Self::terminal`] is what lets a caller ask "does this
    /// need a person?" instead of inferring it from the transition table.
    #[serde(default)]
    pub halted: Vec<String>,
    /// The actors that may perform transitions ("worker", "reviewer", …).
    pub roles: Vec<RoleDef>,
    pub transitions: Vec<TransitionDef>,
    #[serde(default)]
    pub counters: Vec<CounterDef>,
}

/// An actor that may perform transitions. Written as a bare string for the
/// common case (`"worker"`), or as an object when it carries attributes
/// (`{ "name": "operator", "human": true }`); both forms are accepted and a
/// non-human role round-trips back to the bare string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RoleRepr", into = "RoleRepr")]
pub struct RoleDef {
    pub name: String,
    /// Whether a person performs this role. Configuration, never inference —
    /// the engine cannot tell an operator from a worker by name, and this is
    /// the difference between "a person must act" and "an agent that isn't
    /// running", which look identical from the outside.
    pub human: bool,
}

/// Wire shape for [`RoleDef`]: the shorthand and the long form.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum RoleRepr {
    Name(String),
    Full {
        name: String,
        #[serde(default)]
        human: bool,
    },
}

impl From<RoleRepr> for RoleDef {
    fn from(repr: RoleRepr) -> Self {
        match repr {
            RoleRepr::Name(name) => RoleDef { name, human: false },
            RoleRepr::Full { name, human } => RoleDef { name, human },
        }
    }
}

impl From<RoleDef> for RoleRepr {
    fn from(role: RoleDef) -> Self {
        // Keep the shorthand shorthand: only a role with an attribute set
        // needs the long form, so existing definitions survive a round trip.
        if role.human {
            RoleRepr::Full { name: role.name, human: true }
        } else {
            RoleRepr::Name(role.name)
        }
    }
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
    /// Counters returned to zero when this transition fires — the mirror of
    /// [`Self::spends`], and how a person re-arms a loop they are releasing
    /// from a pause. The state move and the clear come back in one decision
    /// so the ledger takes both in a single write, or neither.
    #[serde(default)]
    pub resets: Vec<String>,
}

/// A loop ceiling. When a spending transition finds the counter at `max`, the
/// engine routes the record to `on_exhausted` instead of allowing the move.
///
/// Routing to a halted state is the usual choice: a person decides and releases
/// it. Routing to a terminal one is legal and means the opposite — spending the
/// ceiling *ends* the work, with no way back for anybody. Both are real
/// designs, so nothing here forbids either; pick on purpose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterDef {
    pub name: String,
    pub max: u32,
    pub on_exhausted: String,
    /// States whose *entry* spends this counter, by whichever door the record
    /// came through. Declare the cost against the thing being metered — being
    /// worked on — rather than against each transition that leads to it, and a
    /// path added later cannot silently arrive free.
    ///
    /// A counter is metered one way or the other: naming states here and also
    /// listing it in a transition's `spends` is a load-time error, because the
    /// two would both fire on the same move.
    #[serde(default)]
    pub spend_on_entry_to: Vec<String>,
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
    UnknownReset { from: String, to: String, counter: String },
    UnknownExhaustTarget { counter: String, state: String },
    UnknownHalted(String),
    /// A state listed as both an ending and a pause. It is one or the other.
    HaltedAndTerminal(String),
    /// A halted state with no way back: a terminal state wearing the wrong
    /// label, and the trap this split exists to make unrepresentable.
    HaltedWithNoExit(String),
    /// A pause is left by a person. An automated role leaving one would make
    /// the halt automation-recoverable, which is the opposite of a halt.
    HaltedExitNotHuman { from: String, to: String, role: String },
    /// One transition both spending and clearing the same counter.
    SpendsAndResets { from: String, to: String, counter: String },
    /// A non-terminal state with no way out: records entering it strand forever.
    DeadEnd(String),
    /// A state nothing can reach. Usually the acceptance path someone forgot
    /// to write, and it reads as a working design right up until no record
    /// ever arrives.
    Unreachable(String),
    /// A ceiling nothing spends. It looks like a guard in the definition and
    /// can never fire.
    UnspendableCounter(String),
    UnknownEntrySpendState { counter: String, state: String },
    /// A counter metered both by state entry and by a transition: the same
    /// move would spend it twice.
    CounterSpentTwoWays(String),
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
            UnknownReset { from, to, counter } => {
                write!(f, "transition '{from}' -> '{to}' resets undefined counter '{counter}'")
            }
            UnknownExhaustTarget { counter, state } => {
                write!(f, "counter '{counter}' routes exhaustion to unknown state '{state}'")
            }
            UnknownHalted(s) => write!(f, "halted state '{s}' is not in `states`"),
            HaltedAndTerminal(s) => {
                write!(f, "state '{s}' is both terminal and halted; it is one or the other")
            }
            HaltedWithNoExit(s) => {
                write!(f, "halted state '{s}' has no way back; make it terminal or give it one")
            }
            HaltedExitNotHuman { from, to, role } => {
                write!(f, "transition '{from}' -> '{to}' leaves a halted state as non-human role '{role}'")
            }
            SpendsAndResets { from, to, counter } => {
                write!(f, "transition '{from}' -> '{to}' both spends and resets '{counter}'")
            }
            DeadEnd(s) => write!(f, "non-terminal state '{s}' has no outgoing transition"),
            Unreachable(s) => {
                write!(f, "state '{s}' cannot be reached from '{{initial}}' by any path")
            }
            UnspendableCounter(c) => {
                write!(f, "counter '{c}' is never spent, so its ceiling cannot fire")
            }
            UnknownEntrySpendState { counter, state } => {
                write!(f, "counter '{counter}' spends on entry to unknown state '{state}'")
            }
            CounterSpentTwoWays(c) => {
                write!(f, "counter '{c}' is spent both on state entry and by a transition")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

impl WorkflowDef {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// The roles a person performs. The caller that asks "does this record
    /// need someone, or just an agent that isn't running?" answers it by
    /// splitting [`Engine::next_moves`] along this line.
    pub fn human_roles(&self) -> impl Iterator<Item = &str> {
        self.roles.iter().filter(|r| r.human).map(|r| r.name.as_str())
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
            if !roles.insert(r.name.as_str()) {
                return Err(DuplicateRole(r.name.clone()));
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
        let halted: BTreeSet<&str> = self.halted.iter().map(String::as_str).collect();
        for h in &halted {
            if !states.contains(h) {
                return Err(UnknownHalted(h.to_string()));
            }
            if terminal.contains(h) {
                return Err(HaltedAndTerminal(h.to_string()));
            }
        }
        let human: BTreeSet<&str> = self.human_roles().collect();

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
            if halted.contains(t.from.as_str()) && !human.contains(t.role.as_str()) {
                return Err(HaltedExitNotHuman {
                    from: t.from.clone(),
                    to: t.to.clone(),
                    role: t.role.clone(),
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
            for c in &t.resets {
                if !counters.contains(c.as_str()) {
                    return Err(UnknownReset {
                        from: t.from.clone(),
                        to: t.to.clone(),
                        counter: c.clone(),
                    });
                }
                if t.spends.contains(c) {
                    return Err(SpendsAndResets {
                        from: t.from.clone(),
                        to: t.to.clone(),
                        counter: c.clone(),
                    });
                }
            }
        }

        for c in &self.counters {
            for s in &c.spend_on_entry_to {
                if !states.contains(s.as_str()) {
                    return Err(UnknownEntrySpendState {
                        counter: c.name.clone(),
                        state: s.clone(),
                    });
                }
            }
            let by_transition = self.transitions.iter().any(|t| t.spends.contains(&c.name));
            if by_transition && !c.spend_on_entry_to.is_empty() {
                return Err(CounterSpentTwoWays(c.name.clone()));
            }
            if !by_transition && c.spend_on_entry_to.is_empty() {
                return Err(UnspendableCounter(c.name.clone()));
            }
        }

        for s in &self.states {
            let has_exit = self.transitions.iter().any(|t| &t.from == s);
            if halted.contains(s.as_str()) {
                // A pause nobody can release is an ending that lied about it.
                if !has_exit {
                    return Err(HaltedWithNoExit(s.clone()));
                }
            } else if !terminal.contains(s.as_str()) && !has_exit {
                return Err(DeadEnd(s.clone()));
            }
        }

        // Reachability, last: a state with no way *out* is the worse defect and
        // is reported first, since a state with neither is both. Exhaustion
        // counts as an edge here — a ceiling is a way into its escalation
        // state, and is often the only way in.
        let mut reached: BTreeSet<&str> = BTreeSet::from([self.initial.as_str()]);
        let mut grew = true;
        while grew {
            grew = false;
            for t in &self.transitions {
                if !reached.contains(t.from.as_str()) {
                    continue;
                }
                grew |= reached.insert(t.to.as_str());
                for c in &self.counters {
                    // The lookup is guaranteed by the spend check above.
                    let spent_here =
                        t.spends.contains(&c.name) || c.spend_on_entry_to.contains(&t.to);
                    if spent_here {
                        grew |= reached.insert(c.on_exhausted.as_str());
                    }
                }
            }
        }
        for s in &self.states {
            if !reached.contains(s.as_str()) {
                return Err(Unreachable(s.clone()));
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
        // What this move costs: what the transition names, plus whatever the
        // destination charges for being entered at all. Validation guarantees
        // no counter appears in both, so neither can double-charge the other.
        let spends = self.def.counters.iter().filter(|c| {
            transition.spends.contains(&c.name) || c.spend_on_entry_to.contains(&transition.to)
        });
        for counter in spends {
            let current = snap.counters.get(&counter.name).copied().unwrap_or(0);
            if current >= counter.max {
                return Decision::Exhausted {
                    to: counter.on_exhausted.clone(),
                    counter: counter.name.clone(),
                };
            }
            counter_updates.insert(counter.name.clone(), current + 1);
        }
        for name in &transition.resets {
            // Validation guarantees no counter is both spent and reset here,
            // so these cannot contradict the spends above.
            counter_updates.insert(name.clone(), 0);
        }

        Decision::Allow {
            to: to.to_string(),
            counter_updates,
        }
    }

    /// Does this record need a person? True in a halted state — the
    /// definition's own statement that automation has stopped here and only a
    /// human may release it. An adapter enumerates the waiting records by
    /// selecting on [`WorkflowDef::halted`]; the engine has no ledger to scan.
    pub fn awaits_human(&self, snap: &Snapshot) -> bool {
        self.def.halted.iter().any(|h| h == &snap.state)
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
            "roles": ["worker", "reviewer", { "name": "operator", "human": true }],
            "states": ["awaiting_worker", "working", "awaiting_review", "approved", "escalated"],
            "initial": "awaiting_worker",
            "terminal": ["approved"],
            "halted": ["escalated"],
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
                { "from": "working", "to": "awaiting_worker", "role": "operator" },
                // Releasing the halt: the only move out of it, and the clear
                // rides along so the ledger takes both or neither.
                { "from": "escalated", "to": "awaiting_worker", "role": "operator", "resets": ["agent_passes"] }
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
    fn every_state_offers_exactly_the_moves_its_kind_allows() {
        let engine = Engine::new(review_loop()).unwrap();
        let def = engine.def();
        let human: Vec<&str> = def.human_roles().collect();
        for state in &def.states {
            let s = snap(state, 0);
            let movers: Vec<&str> = def
                .roles
                .iter()
                .filter(|r| !engine.next_moves(&s, &r.name).is_empty())
                .map(|r| r.name.as_str())
                .collect();
            // These three shapes are what every caller reads to tell an ending
            // from a pause from live work. If any two of them can look alike,
            // the decision surface cannot be built on top of them.
            if def.terminal.contains(state) {
                assert!(movers.is_empty(), "ending '{state}' offered {movers:?}");
                assert!(!engine.awaits_human(&s));
            } else if def.halted.contains(state) {
                assert!(!movers.is_empty(), "pause '{state}' offered nobody a way back");
                assert!(
                    movers.iter().all(|m| human.contains(m)),
                    "pause '{state}' offered {movers:?}, not all of them people"
                );
                assert!(engine.awaits_human(&s));
            } else {
                assert!(!movers.is_empty(), "live '{state}' offered nobody");
                assert!(!engine.awaits_human(&s));
            }
        }
    }

    #[test]
    fn releasing_a_halt_moves_the_state_and_clears_the_counter_together() {
        let engine = Engine::new(review_loop()).unwrap();
        let spent = snap("escalated", 3);
        assert!(engine.awaits_human(&spent));

        // The trap this replaces: zeroing the counter alone changed nothing,
        // because no transition was ever found to go and consult it.
        assert!(matches!(
            engine.authorize(&spent, "worker", "working"),
            Decision::Deny { .. }
        ));

        match engine.authorize(&spent, "operator", "awaiting_worker") {
            Decision::Allow { to, counter_updates } => {
                assert_eq!(to, "awaiting_worker");
                assert_eq!(counter_updates.get("agent_passes"), Some(&0));
            }
            other => panic!("expected the halt to release, got {other:?}"),
        }
    }

    #[test]
    fn a_halt_an_agent_could_release_is_rejected() {
        let mut def = review_loop();
        def.transitions.push(TransitionDef {
            from: "escalated".to_string(),
            to: "awaiting_worker".to_string(),
            role: "worker".to_string(),
            spends: Vec::new(),
            resets: Vec::new(),
        });
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::HaltedExitNotHuman {
                from: "escalated".to_string(),
                to: "awaiting_worker".to_string(),
                role: "worker".to_string(),
            }
        );
    }

    #[test]
    fn a_halt_with_no_way_back_is_rejected() {
        // The original defect, now unrepresentable: a state a ceiling routes
        // to, that nothing and nobody can leave, no longer validates.
        let mut def = review_loop();
        def.transitions.retain(|t| t.from != "escalated");
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::HaltedWithNoExit("escalated".to_string())
        );
    }

    #[test]
    fn a_role_is_a_bare_string_until_it_needs_an_attribute() {
        let def: WorkflowDef = serde_json::from_value(json!({
            "name": "mixed",
            "roles": ["worker", { "name": "operator", "human": true }],
            "states": ["queued", "done"],
            "initial": "queued",
            "terminal": ["done"],
            "transitions": [{ "from": "queued", "to": "done", "role": "worker" }]
        }))
        .unwrap();
        def.validate().unwrap();
        assert_eq!(def.human_roles().collect::<Vec<_>>(), ["operator"]);

        // Only an attributed role expands, so adding this field cannot churn
        // the definitions that never use it.
        let back = serde_json::to_value(&def).unwrap();
        assert_eq!(back["roles"][0], json!("worker"));
        assert_eq!(back["roles"][1], json!({ "name": "operator", "human": true }));
    }

    #[test]
    fn duplicate_roles_are_caught_across_both_forms() {
        let mut def = review_loop();
        def.roles.push(RoleDef { name: "worker".into(), human: true });
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::DuplicateRole("worker".to_string())
        );
    }

    #[test]
    fn a_state_nothing_enters_is_rejected() {
        // The acceptance path someone forgot to write: the state has exits, so
        // every other check passes, and no record ever arrives to use them.
        let mut def = review_loop();
        def.states.push("resolved".to_string());
        def.transitions.push(TransitionDef {
            from: "resolved".to_string(),
            to: "approved".to_string(),
            role: "reviewer".to_string(),
            spends: Vec::new(),
            resets: Vec::new(),
        });
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::Unreachable("resolved".to_string())
        );
    }

    #[test]
    fn reaching_a_state_only_by_exhaustion_still_counts() {
        // `escalated` is entered by a reviewer transition AND by the ceiling.
        // Remove the transition and the ceiling alone must keep it reachable,
        // or the check would reject every workflow that escalates by budget.
        let mut def = review_loop();
        def.transitions
            .retain(|t| !(t.from == "awaiting_review" && t.to == "escalated"));
        def.validate().expect("exhaustion routing is a way in");
    }

    #[test]
    fn entering_a_state_costs_the_same_by_every_door() {
        let def: WorkflowDef = serde_json::from_value(json!({
            "name": "two-doors",
            "roles": ["worker", { "name": "operator", "human": true }],
            "states": ["open", "working", "reopened", "done", "escalated"],
            "initial": "open",
            "terminal": ["done"],
            "halted": ["escalated"],
            "counters": [
                { "name": "passes", "max": 2, "on_exhausted": "escalated",
                  "spend_on_entry_to": ["working"] }
            ],
            "transitions": [
                { "from": "open", "to": "working", "role": "worker" },
                { "from": "working", "to": "done", "role": "worker" },
                { "from": "working", "to": "reopened", "role": "worker" },
                { "from": "reopened", "to": "working", "role": "worker" },
                { "from": "escalated", "to": "open", "role": "operator", "resets": ["passes"] }
            ]
        }))
        .unwrap();
        let engine = Engine::new(def).unwrap();

        // No transition names the counter. The destination charges regardless
        // of which door was used, which is the point: a door added later
        // cannot arrive free by being forgotten.
        for door in ["open", "reopened"] {
            let s = Snapshot {
                state: door.to_string(),
                counters: BTreeMap::from([("passes".to_string(), 1)]),
            };
            match engine.authorize(&s, "worker", "working") {
                Decision::Allow { counter_updates, .. } => {
                    assert_eq!(counter_updates.get("passes"), Some(&2), "door '{door}'");
                }
                other => panic!("door '{door}' should charge: {other:?}"),
            }
        }

        let spent = Snapshot {
            state: "reopened".to_string(),
            counters: BTreeMap::from([("passes".to_string(), 2)]),
        };
        assert!(matches!(
            engine.authorize(&spent, "worker", "working"),
            Decision::Exhausted { .. }
        ));
    }

    #[test]
    fn a_counter_metered_two_ways_is_rejected() {
        let mut def = review_loop();
        def.counters[0].spend_on_entry_to.push("working".to_string());
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::CounterSpentTwoWays("agent_passes".to_string())
        );
    }

    #[test]
    fn a_counter_nothing_spends_is_rejected() {
        let mut def = review_loop();
        def.counters.push(CounterDef {
            name: "review_rounds".to_string(),
            max: 2,
            on_exhausted: "escalated".to_string(),
            spend_on_entry_to: Vec::new(),
        });
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::UnspendableCounter("review_rounds".to_string())
        );
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
            resets: vec![],
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

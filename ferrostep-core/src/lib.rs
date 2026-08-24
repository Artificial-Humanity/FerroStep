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
    /// Who may file a new record, and what it costs. Absent means nobody does,
    /// through this engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creation: Option<CreationDef>,
    /// Who may move a record between units of work, per scope label. **Empty
    /// means nobody may** — scope is written at filing and stays there unless a
    /// definition says otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rescopes: Vec<RescopeDef>,
}

/// Permission to change one scope label on a record.
///
/// A record's scope says which unit of work it belongs to, and every query
/// that finds work filters on it — so a record whose scope names a finished
/// unit is invisible to all of them. Moving it is a real operation, and one
/// that was being performed as an un-versioned, un-evented write to the field
/// every query depends on until it became this.
///
/// ⚠ **The label is opaque to the engine.** It knows a rule is *about* a
/// label; it never knows what any particular label means, exactly as it never
/// knows what a review means.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RescopeDef {
    pub label: String,
    pub role: String,
    /// Whether the move must carry a reason. A scope change with no stated
    /// reason is indistinguishable from a record quietly disappearing out of
    /// one queue and into another.
    #[serde(default)]
    pub requires_note: bool,
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
    /// Whether this move must carry a reason.
    ///
    /// Some moves are unusable without one. A record sent back for more work
    /// with no explanation spends an attempt on a guess; an escalation with no
    /// stated question cannot be answered by the person it is addressed to.
    /// Both are rules a persona can hold and forget, so they are refusals here
    /// instead.
    #[serde(default)]
    pub requires_note: bool,
}

/// What an actor is trying to do, and what they are saying about it.
///
/// Bundled rather than passed loose so the engine sees everything it is
/// judging in one value — a note that is not on the attempt cannot be checked,
/// and a check the engine cannot make becomes a rule somebody else has to
/// remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt<'a> {
    pub role: &'a str,
    pub to: &'a str,
    pub note: Option<&'a str>,
}

impl<'a> Attempt<'a> {
    /// A move carrying no reason.
    pub fn new(role: &'a str, to: &'a str) -> Self {
        Attempt { role, to, note: None }
    }

    /// The same move, with the reason attached.
    pub fn saying(mut self, note: &'a str) -> Self {
        self.note = Some(note);
        self
    }

    fn has_note(&self) -> bool {
        self.note.is_some_and(|n| !n.trim().is_empty())
    }
}

/// Who may file a new record, and what filing one costs.
///
/// Absent from a definition means nobody files through the engine — which is
/// the safe default, because a workflow that has not said who may create work
/// should refuse rather than assume anyone may.
///
/// The cost is the interesting part. A loop whose reviewing role files findings
/// can be individually bounded and collectively unbounded: every record honours
/// its ceiling while the population grows without limit, because each fix pass
/// produces new work to find. A counter spent here is what bounds that, and
/// because counter values come from the snapshot the adapter builds, it can be
/// scoped to a branch or a cycle rather than to any one record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreationDef {
    /// Roles permitted to file.
    pub roles: Vec<String>,
    #[serde(default)]
    pub spends: Vec<String>,
    #[serde(default)]
    pub requires_note: bool,
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
    /// Whether the attempt that finds this ceiling spent must carry a reason.
    ///
    /// Exhaustion is routing, and where it routes is usually a person. But an
    /// automatic route produces a record sitting in front of that person **with
    /// no question attached** — and the question is the whole content of the
    /// handover. The actor that ran out of attempts is the one who knows what
    /// cannot be settled, and this is the moment it knows it.
    ///
    /// ⚠ Deliberately not `requires_note` on the spending transition: that
    /// would demand a reason for *every* attempt, when only the last one is
    /// addressed to anybody.
    #[serde(default)]
    pub exhausted_requires_note: bool,
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
    ///
    /// `scope_updates` moves the record between units of work (see
    /// [`Engine::authorize_rescope`]) and is empty for every ordinary move, so
    /// it is omitted from the JSON when empty and a consumer written before it
    /// existed sees exactly what it saw before. It persists in the same atomic
    /// write for the same reason the counters do.
    Allow {
        to: String,
        counter_updates: BTreeMap<String, u32>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        scope_updates: BTreeMap<String, String>,
    },
    /// The move was legal but a ceiling is spent: route the record to `to`
    /// (the counter's `on_exhausted` state) instead. No counter changes.
    Exhausted { to: String, counter: String },
    /// Not a legal move for this record/role. Nothing to persist.
    Deny { reason: String },
}

impl Decision {
    /// An allow that moves state and counters and leaves scope alone — which
    /// is every move except a rescope. Exists so the ordinary case does not
    /// have to name the field it is not using.
    pub fn allow(to: impl Into<String>, counter_updates: BTreeMap<String, u32>) -> Decision {
        Decision::Allow { to: to.into(), counter_updates, scope_updates: BTreeMap::new() }
    }
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
    /// An automated role clearing a ceiling without paying for the privilege.
    ///
    /// A reset hands back a fresh budget. Where a person does that, the person
    /// is the bound — they decided this work was worth continuing. Where an
    /// agent does it for free, the ceiling has deleted itself: every recurrence
    /// is granted a new allowance, and a defect that will not die never reaches
    /// anybody. So an agent may re-arm a loop, but only by spending something
    /// that itself runs out.
    UnpaidAgentReset { from: String, to: String, role: String },
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
    UnknownCreationRole(String),
    UnknownCreationSpend(String),
    /// A rescope rule handing a scope label to a role the workflow has no
    /// actor for. It reads as a permission and grants nothing.
    UnknownRescopeRole(String),
    /// The same label granted to the same role twice: one of them is dead
    /// text, and which one is dead depends on a lookup order nobody should
    /// have to know.
    DuplicateRescope { label: String, role: String },
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
            UnpaidAgentReset { from, to, role } => write!(
                f,
                "transition '{from}' -> '{to}' clears a counter as non-human role '{role}' \
                 without spending one; an agent may re-arm a loop only by paying for it"
            ),
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
            UnknownCreationRole(r) => {
                write!(f, "creation names role '{r}', which is not in `roles`")
            }
            UnknownCreationSpend(c) => {
                write!(f, "creation spends undefined counter '{c}'")
            }
            CounterSpentTwoWays(c) => {
                write!(f, "counter '{c}' is spent both on state entry and by a transition")
            }
            UnknownRescopeRole(r) => {
                write!(f, "a rescope names role '{r}', which is not in `roles`")
            }
            DuplicateRescope { label, role } => {
                write!(f, "scope label '{label}' is granted to role '{role}' twice")
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
            // A reset an agent gets for free is a ceiling deleting itself.
            if !t.resets.is_empty() && !human.contains(t.role.as_str()) {
                let pays = !t.spends.is_empty()
                    || self.counters.iter().any(|c| c.spend_on_entry_to.contains(&t.to));
                if !pays {
                    return Err(UnpaidAgentReset {
                        from: t.from.clone(),
                        to: t.to.clone(),
                        role: t.role.clone(),
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
            let by_creation = self
                .creation
                .as_ref()
                .is_some_and(|c2| c2.spends.contains(&c.name));
            if by_transition && !c.spend_on_entry_to.is_empty() {
                return Err(CounterSpentTwoWays(c.name.clone()));
            }
            // Filing counts as spending. A budget on how much work a round may
            // create is spent nowhere else, and rejecting it as inert would
            // make the one ceiling that bounds a population unexpressible.
            if !by_transition && !by_creation && c.spend_on_entry_to.is_empty() {
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
        // A rescope rule naming a role that does not exist grants nothing and
        // reads as a granted permission — the same shape as a transition whose
        // role is a typo, refused for the same reason.
        let mut rescope_pairs: BTreeSet<(&str, &str)> = BTreeSet::new();
        for rescope in &self.rescopes {
            if !roles.contains(rescope.role.as_str()) {
                return Err(UnknownRescopeRole(rescope.role.clone()));
            }
            if !rescope_pairs.insert((rescope.label.as_str(), rescope.role.as_str())) {
                return Err(DuplicateRescope {
                    label: rescope.label.clone(),
                    role: rescope.role.clone(),
                });
            }
        }

        let mut reached: BTreeSet<&str> = BTreeSet::from([self.initial.as_str()]);
        if let Some(creation) = &self.creation {
            for name in &creation.roles {
                if !roles.contains(name.as_str()) {
                    return Err(UnknownCreationRole(name.clone()));
                }
            }
            for name in &creation.spends {
                let Some(counter) = self.counters.iter().find(|c| &c.name == name) else {
                    return Err(UnknownCreationSpend(name.clone()));
                };
                // Where a filing budget sends the matter when it is spent is a
                // destination this workflow names, and is reachable by saying so
                // — no record travels there, because none was created.
                reached.insert(counter.on_exhausted.as_str());
            }
        }
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
    pub fn authorize(&self, snap: &Snapshot, attempt: &Attempt<'_>) -> Decision {
        let Attempt { role, to, .. } = *attempt;
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

        // Before anything is spent: a move that must carry a reason and does
        // not is refused, not charged.
        if transition.requires_note && !attempt.has_note() {
            return Decision::Deny {
                reason: format!(
                    "'{}' -> '{to}' requires a note saying why",
                    snap.state
                ),
            };
        }

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
                // The routing is about to hand this record to somebody. Refuse
                // to do it silently where the definition says the handover
                // carries a question — a person cannot answer one that was
                // never asked.
                if counter.exhausted_requires_note && !attempt.has_note() {
                    return Decision::Deny {
                        reason: format!(
                            "'{}' is spent: the attempt that routes this record to '{}' \
                             must say what decision is being asked for",
                            counter.name, counter.on_exhausted
                        ),
                    };
                }
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
            scope_updates: BTreeMap::new(),
        }
    }

    /// May `role` move this record to a different unit of work?
    ///
    /// A rescope changes scope labels and nothing else: the state does not
    /// move and no counter is spent, so the returned `to` is the state the
    /// record is already in. That is not a fabricated state change — it is the
    /// engine saying "stay here, with this scope", in the one shape a caller
    /// already knows how to persist atomically.
    ///
    /// ⚠ **Refused on a terminal record, and that is not configurable.** A
    /// finished record's scope is provenance: it says which unit of work the
    /// record was resolved against, and rewriting it falsifies the answer to a
    /// question nobody can re-ask later.
    pub fn authorize_rescope(
        &self,
        snap: &Snapshot,
        role: &str,
        updates: &BTreeMap<String, String>,
        note: Option<&str>,
    ) -> Decision {
        if !self.def.states.contains(&snap.state) {
            return Decision::Deny {
                reason: format!(
                    "record state '{}' is not a state of workflow '{}'",
                    snap.state, self.def.name
                ),
            };
        }
        if self.def.terminal.contains(&snap.state) {
            return Decision::Deny {
                reason: format!(
                    "record is in terminal state '{}': a finished record's scope is \
                     the unit of work it was resolved against, and is not rewritten",
                    snap.state
                ),
            };
        }
        if updates.is_empty() {
            return Decision::Deny { reason: "no scope labels given to change".to_string() };
        }

        let mut needs_note = false;
        for label in updates.keys() {
            let permitted = self.def.rescopes.iter().find(|r| &r.label == label && r.role == role);
            let Some(rule) = permitted else {
                // Same split as `authorize`: "you may not" and "nobody may"
                // send a reader to different places.
                let others = self.def.rescopes.iter().any(|r| &r.label == label);
                let reason = if others {
                    format!("role '{role}' may not change scope label '{label}'")
                } else {
                    format!(
                        "workflow '{}' does not say who may change scope label '{label}'",
                        self.def.name
                    )
                };
                return Decision::Deny { reason };
            };
            needs_note |= rule.requires_note;
        }
        if needs_note && note.is_none_or(|n| n.trim().is_empty()) {
            return Decision::Deny {
                reason: "changing this record's unit of work requires a note saying why"
                    .to_string(),
            };
        }

        Decision::Allow {
            to: snap.state.clone(),
            counter_updates: BTreeMap::new(),
            scope_updates: updates.clone(),
        }
    }

    /// May `attempt`'s role file a new record, and what does filing cost?
    ///
    /// `counters` are the values the ceilings named by [`CreationDef::spends`]
    /// are measured against. They need not belong to any record — an adapter
    /// can compute them per branch, per cycle, per anything it can count — and
    /// that is what lets a creation budget bound a loop whose every individual
    /// record is already bounded.
    ///
    /// The attempt's `to` must name the workflow's initial state. Requiring it
    /// rather than filling it in catches a caller that believes it is filing
    /// somewhere else.
    pub fn authorize_create(
        &self,
        attempt: &Attempt<'_>,
        counters: &BTreeMap<String, u32>,
    ) -> Decision {
        let Some(creation) = &self.def.creation else {
            return Decision::Deny {
                reason: format!(
                    "workflow '{}' does not say who may file a record",
                    self.def.name
                ),
            };
        };
        if attempt.to != self.def.initial {
            return Decision::Deny {
                reason: format!(
                    "a new record starts in '{}', not '{}'",
                    self.def.initial, attempt.to
                ),
            };
        }
        if !creation.roles.iter().any(|r| r == attempt.role) {
            return Decision::Deny {
                reason: format!("role '{}' may not file a record", attempt.role),
            };
        }
        if creation.requires_note && !attempt.has_note() {
            return Decision::Deny {
                reason: "filing a record requires a note saying why".to_string(),
            };
        }

        let mut counter_updates = BTreeMap::new();
        for name in &creation.spends {
            // Validation guarantees the counter exists.
            let counter = self.def.counters.iter().find(|c| &c.name == name).unwrap();
            let current = counters.get(name).copied().unwrap_or(0);
            if current >= counter.max {
                return Decision::Exhausted {
                    to: counter.on_exhausted.clone(),
                    counter: counter.name.clone(),
                };
            }
            counter_updates.insert(counter.name.clone(), current + 1);
        }
        Decision::Allow {
            to: self.def.initial.clone(),
            counter_updates,
            // A new record's scope is the scope it is filed into; there is no
            // previous unit of work for it to move between.
            scope_updates: BTreeMap::new(),
        }
    }

    /// What can happen to this record next, taken as a whole.
    ///
    /// Derived from what the moves would actually do rather than from which
    /// state the record sits in, which is the only way [`Status::WillEscalate`]
    /// can be seen at all.
    pub fn status(&self, snap: &Snapshot) -> Status {
        if !self.def.states.contains(&snap.state) {
            // Outside the workflow's own vocabulary. Nothing automated can be
            // said about it and somebody should look.
            return Status::NeedsPerson;
        }
        if self.def.terminal.iter().any(|t| t == &snap.state) {
            return Status::Ended;
        }
        let mut automated_can_act = false;
        let mut person_can_act = false;
        for role in &self.def.roles {
            for (_, decision) in self.next_moves(snap, &role.name) {
                if matches!(decision, Decision::Allow { .. }) {
                    if role.human {
                        person_can_act = true;
                    } else {
                        automated_can_act = true;
                    }
                }
            }
        }
        if automated_can_act {
            Status::Live
        } else if person_can_act {
            Status::NeedsPerson
        } else {
            // Validation guarantees a non-terminal state has a way out, so
            // moves exist here; none of them would be allowed.
            Status::WillEscalate
        }
    }

    /// Every move `role` could attempt from the record's current state, each
    /// paired with what [`Engine::authorize`] would answer for it right now.
    ///
    /// The pairing is the whole point. "This transition exists" and "this
    /// transition would fire" are different facts, and a surface built on the
    /// first offers a person a move that quietly routes the record somewhere
    /// else. [`Decision::Deny`] never appears here: only moves this role may
    /// make from this state are enumerated in the first place.
    pub fn next_moves(&self, snap: &Snapshot, role: &str) -> Vec<(&TransitionDef, Decision)> {
        self.def
            .transitions
            .iter()
            .filter(|t| t.from == snap.state && t.role == role)
            // Offered WITH a note, so a move that merely lacks one is not
            // reported as impossible. What this answers is "could this happen",
            // not "would this exact call succeed" — a surface showing a person
            // their options must still show one that needs a reason attached.
            .map(|t| {
                let attempt = Attempt { role, to: &t.to, note: Some("…") };
                (t, self.authorize(snap, &attempt))
            })
            .collect()
    }
}

/// What can happen to a record next. See [`Engine::status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// A terminal state. Nothing moves it, ever.
    Ended,
    /// No automated role can make progress, but a person can — either because
    /// the record is paused, or because every ceiling automation would spend
    /// is gone while a human's route out is not.
    NeedsPerson,
    /// Moves remain and every one of them would spend a spent ceiling, so the
    /// next actor to try will route the record away instead of doing what it
    /// asked. Nothing about the record's *state* says this, which is why the
    /// condition has a name: it reads as healthy right up until someone acts.
    WillEscalate,
    /// At least one automated role can still make progress.
    Live,
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
        let decision = engine.authorize(&snap("awaiting_worker", 0), &Attempt::new("worker", "working"));
        assert_eq!(
            decision,
            Decision::allow("working", BTreeMap::from([("agent_passes".to_string(), 1)]))
        );
    }

    #[test]
    fn ceiling_routes_to_escalation() {
        let engine = Engine::new(review_loop()).unwrap();
        let decision = engine.authorize(&snap("awaiting_worker", 3), &Attempt::new("worker", "working"));
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
        let decision = engine.authorize(&snap("awaiting_review", 1), &Attempt::new("worker", "approved"));
        assert!(matches!(decision, Decision::Deny { .. }));
    }

    #[test]
    fn crashed_pass_still_costs_one() {
        let engine = Engine::new(review_loop()).unwrap();
        // Pass 1 claimed (counter -> 1), worker crashes in `working`.
        // Operator re-queues; no refund is offered anywhere.
        let d = engine.authorize(&snap("working", 1), &Attempt::new("operator", "awaiting_worker"));
        assert_eq!(
            d,
            Decision::allow("awaiting_worker", BTreeMap::new())
        );
        // Two more claims spend 2 and 3, then the ceiling routes to escalation
        // even though only two passes ever produced work.
        for expected in [2u32, 3] {
            let d = engine.authorize(
                &snap("awaiting_worker", expected - 1),
                &Attempt::new("worker", "working"),
            );
            assert_eq!(
                d,
                Decision::allow("working", BTreeMap::from([("agent_passes".to_string(), expected)]))
            );
        }
        let d = engine.authorize(&snap("awaiting_worker", 3), &Attempt::new("worker", "working"));
        assert!(matches!(d, Decision::Exhausted { .. }));
    }

    #[test]
    fn hand_zeroed_counter_re_arms() {
        let engine = Engine::new(review_loop()).unwrap();
        // An operator zeroing the counter in the ledger is a deliberate re-arm;
        // the engine takes the snapshot at face value.
        let d = engine.authorize(&snap("awaiting_worker", 0), &Attempt::new("worker", "working"));
        assert!(matches!(d, Decision::Allow { .. }));
    }

    #[test]
    fn no_transitions_out_of_terminal_states() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("approved", 3), &Attempt::new("reviewer", "awaiting_worker"));
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn unknown_record_state_is_denied_not_a_panic() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("limbo", 0), &Attempt::new("worker", "working"));
        assert!(matches!(d, Decision::Deny { .. }));
    }

    #[test]
    fn next_moves_lists_role_options() {
        let engine = Engine::new(review_loop()).unwrap();
        let moves = engine.next_moves(&snap("awaiting_review", 1), "reviewer");
        let targets: Vec<&str> = moves.iter().map(|(t, _)| t.to.as_str()).collect();
        assert_eq!(targets, ["awaiting_worker", "approved", "escalated"]);
        // None of these spends anything, so all three would fire as offered.
        assert!(moves.iter().all(|(_, d)| matches!(d, Decision::Allow { .. })));
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
                assert_eq!(engine.status(&s), Status::Ended);
            } else if def.halted.contains(state) {
                assert!(!movers.is_empty(), "pause '{state}' offered nobody a way back");
                assert!(
                    movers.iter().all(|m| human.contains(m)),
                    "pause '{state}' offered {movers:?}, not all of them people"
                );
                assert_eq!(engine.status(&s), Status::NeedsPerson);
            } else {
                assert!(!movers.is_empty(), "live '{state}' offered nobody");
                assert_eq!(engine.status(&s), Status::Live);
            }
        }
    }

    #[test]
    fn a_record_whose_only_moves_would_exhaust_says_so() {
        // Measured in the hand-driven loop this engine succeeds: a cycle
        // ceiling was spent while every per-item counter still had headroom,
        // so the work read as healthy and the surface offered moves that
        // could not fire. The state alone cannot tell you this.
        let engine = Engine::new(review_loop()).unwrap();
        let spent = snap("awaiting_worker", 3);

        let moves = engine.next_moves(&spent, "worker");
        assert!(!moves.is_empty(), "the transition still exists");
        assert!(
            moves.iter().all(|(_, d)| matches!(d, Decision::Exhausted { .. })),
            "every move offered would route away instead of firing"
        );
        assert_eq!(engine.status(&spent), Status::WillEscalate);

        // Same state, same role, budget intact: indistinguishable by state,
        // opposite by disposition. That difference is the whole feature.
        let fresh = snap("awaiting_worker", 0);
        assert_eq!(
            engine.next_moves(&fresh, "worker").len(),
            moves.len(),
            "the move list is identical; only what it would do differs"
        );
        assert_eq!(engine.status(&fresh), Status::Live);
    }

    #[test]
    fn a_record_outside_the_workflow_needs_a_person() {
        let engine = Engine::new(review_loop()).unwrap();
        assert_eq!(engine.status(&snap("limbo", 0)), Status::NeedsPerson);
    }

    #[test]
    fn releasing_a_halt_moves_the_state_and_clears_the_counter_together() {
        let engine = Engine::new(review_loop()).unwrap();
        let spent = snap("escalated", 3);
        assert_eq!(engine.status(&spent), Status::NeedsPerson);

        // The trap this replaces: zeroing the counter alone changed nothing,
        // because no transition was ever found to go and consult it.
        assert!(matches!(
            engine.authorize(&spent, &Attempt::new("worker", "working")),
            Decision::Deny { .. }
        ));

        match engine.authorize(&spent, &Attempt::new("operator", "awaiting_worker")) {
            Decision::Allow { to, counter_updates, .. } => {
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
            requires_note: false,
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
            requires_note: false,
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
            match engine.authorize(&s, &Attempt::new("worker", "working")) {
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
            engine.authorize(&spent, &Attempt::new("worker", "working")),
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

    /// A loop whose reviewer files findings, with both new rules in use.
    fn filing_loop() -> WorkflowDef {
        serde_json::from_value(json!({
            "name": "filing-loop",
            "roles": ["worker", "reviewer", { "name": "operator", "human": true }],
            "states": ["open", "working", "review", "done", "escalated"],
            "initial": "open",
            "terminal": ["done"],
            "halted": ["escalated"],
            "counters": [
                { "name": "passes",  "max": 3, "on_exhausted": "escalated",
                  "spend_on_entry_to": ["working"] },
                { "name": "filings", "max": 2, "on_exhausted": "escalated" }
            ],
            "creation": { "roles": ["reviewer"], "spends": ["filings"], "requires_note": true },
            "transitions": [
                { "from": "open", "to": "working", "role": "worker" },
                { "from": "working", "to": "review", "role": "worker" },
                { "from": "review", "to": "open", "role": "reviewer", "requires_note": true },
                { "from": "review", "to": "done", "role": "reviewer" },
                { "from": "working", "to": "escalated", "role": "worker", "requires_note": true },
                { "from": "escalated", "to": "open", "role": "operator", "resets": ["passes"] }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_move_that_must_say_why_is_refused_when_it_does_not() {
        let engine = Engine::new(filing_loop()).unwrap();
        let at_review = Snapshot {
            state: "review".to_string(),
            counters: BTreeMap::from([("passes".to_string(), 1)]),
        };

        // Sending work back with no explanation spends an attempt on a guess.
        match engine.authorize(&at_review, &Attempt::new("reviewer", "open")) {
            Decision::Deny { reason } => assert!(reason.contains("requires a note"), "{reason}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(matches!(
            engine.authorize(&at_review, &Attempt::new("reviewer", "open").saying("the guard still scans docstrings")),
            Decision::Allow { .. }
        ));

        // Whitespace is not a reason.
        assert!(matches!(
            engine.authorize(&at_review, &Attempt::new("reviewer", "open").saying("   ")),
            Decision::Deny { .. }
        ));

        // And a move that needs no reason is unaffected either way.
        assert!(matches!(
            engine.authorize(&at_review, &Attempt::new("reviewer", "done")),
            Decision::Allow { .. }
        ));
    }

    #[test]
    fn the_note_is_checked_before_anything_is_spent() {
        // A refusal must not cost what a success would have cost.
        let engine = Engine::new(filing_loop()).unwrap();
        let working = Snapshot {
            state: "working".to_string(),
            counters: BTreeMap::from([("passes".to_string(), 1)]),
        };
        match engine.authorize(&working, &Attempt::new("worker", "escalated")) {
            Decision::Deny { .. } => {}
            other => panic!("a note-less escalation must be refused, got {other:?}"),
        }
    }

    #[test]
    fn filing_is_role_gated_metered_and_bounded() {
        let engine = Engine::new(filing_loop()).unwrap();
        let none = BTreeMap::new();
        let spent = BTreeMap::from([("filings".to_string(), 2)]);

        // The worker does not file, whatever it says.
        assert!(matches!(
            engine.authorize_create(&Attempt::new("worker", "open").saying("found one"), &none),
            Decision::Deny { .. }
        ));
        // Nor does the reviewer, silently.
        assert!(matches!(
            engine.authorize_create(&Attempt::new("reviewer", "open"), &none),
            Decision::Deny { .. }
        ));
        // Filing into anywhere but the initial state is a caller that thinks
        // it is doing something else.
        assert!(matches!(
            engine.authorize_create(&Attempt::new("reviewer", "review").saying("x"), &none),
            Decision::Deny { .. }
        ));

        match engine.authorize_create(&Attempt::new("reviewer", "open").saying("the gate is unrun"), &none) {
            Decision::Allow { to, counter_updates, .. } => {
                assert_eq!(to, "open");
                assert_eq!(counter_updates.get("filings"), Some(&1));
            }
            other => panic!("expected the filing to be allowed, got {other:?}"),
        }

        // The budget runs out, and running out routes to a person. Every
        // record this loop holds may still be well inside its own ceiling —
        // that is the whole point of a bound at this scope.
        assert_eq!(
            engine.authorize_create(&Attempt::new("reviewer", "open").saying("and another"), &spent),
            Decision::Exhausted { to: "escalated".to_string(), counter: "filings".to_string() }
        );
    }

    #[test]
    fn a_workflow_that_does_not_say_who_files_refuses_filing() {
        // review_loop() names no creation, so nothing files through the engine.
        let engine = Engine::new(review_loop()).unwrap();
        assert!(matches!(
            engine.authorize_create(&Attempt::new("reviewer", "awaiting_worker").saying("x"), &BTreeMap::new()),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn a_move_needing_a_note_is_still_offered_as_an_option() {
        // next_moves answers "could this happen", not "would this exact call
        // succeed" — a surface must still show a person the move that needs a
        // reason, or they can never take it.
        let engine = Engine::new(filing_loop()).unwrap();
        let at_review = Snapshot {
            state: "review".to_string(),
            counters: BTreeMap::from([("passes".to_string(), 1)]),
        };
        let offered: Vec<&str> = engine
            .next_moves(&at_review, "reviewer")
            .iter()
            .map(|(t, _)| t.to.as_str())
            .collect();
        assert!(offered.contains(&"open"), "got {offered:?}");
    }

    #[test]
    fn an_agent_cannot_re_arm_a_loop_for_free() {
        // The degradation this prevents is silent: keep the reset, drop what it
        // costs, and every recurrence is granted a fresh allowance while the
        // ceiling still looks present. A defect that will not die then never
        // reaches anybody, because it never runs out of attempts.
        let mut def = review_loop();
        def.transitions.push(TransitionDef {
            from: "working".to_string(),
            to: "awaiting_worker".to_string(),
            role: "reviewer".to_string(),
            spends: Vec::new(),
            resets: vec!["agent_passes".to_string()],
            requires_note: false,
        });
        assert_eq!(
            def.validate().unwrap_err(),
            ValidationError::UnpaidAgentReset {
                from: "working".to_string(),
                to: "awaiting_worker".to_string(),
                role: "reviewer".to_string(),
            }
        );
    }

    #[test]
    fn an_agent_may_re_arm_a_loop_by_paying_for_it() {
        // Same move, now metered by a ceiling that itself runs out. This is the
        // shape a two-level design needs: fresh attempts are purchased, and the
        // thing they are purchased with is finite.
        let mut def = review_loop();
        def.counters.push(CounterDef {
            name: "reopens".to_string(),
            max: 2,
            on_exhausted: "escalated".to_string(),
            spend_on_entry_to: Vec::new(),
            exhausted_requires_note: false,
        });
        def.transitions.push(TransitionDef {
            from: "working".to_string(),
            to: "awaiting_worker".to_string(),
            role: "reviewer".to_string(),
            spends: vec!["reopens".to_string()],
            resets: vec!["agent_passes".to_string()],
            requires_note: false,
        });
        def.validate().expect("a paid re-arm is legitimate");
    }

    #[test]
    fn a_person_re_arms_without_paying_because_they_are_the_bound() {
        // Every reset this repo ships is on a human role, and none of them
        // spends anything. That is not an oversight being grandfathered: a
        // person deciding the work is worth continuing IS the ceiling.
        let engine = Engine::new(review_loop()).unwrap();
        let released = engine.def().transitions.iter().find(|t| t.from == "escalated").unwrap();
        assert!(!released.resets.is_empty() && released.spends.is_empty());
        assert!(engine.def().human_roles().any(|r| r == released.role));
    }

    #[test]
    fn a_counter_nothing_spends_is_rejected() {
        let mut def = review_loop();
        def.counters.push(CounterDef {
            name: "review_rounds".to_string(),
            max: 2,
            on_exhausted: "escalated".to_string(),
            spend_on_entry_to: Vec::new(),
            exhausted_requires_note: false,
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
            requires_note: false,
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
        def.purpose = Some("docs/north-star.md@main".to_string());
        let with_purpose = Engine::new(def).unwrap();
        let without = Engine::new(review_loop()).unwrap();
        // Identical decisions either way: the field is opaque to the engine.
        assert_eq!(
            with_purpose.authorize(&snap("awaiting_worker", 0), &Attempt::new("worker", "working")),
            without.authorize(&snap("awaiting_worker", 0), &Attempt::new("worker", "working")),
        );
        // It round-trips when present and stays absent (not null) when not.
        let json = serde_json::to_value(with_purpose.def()).unwrap();
        assert_eq!(json["purpose"], "docs/north-star.md@main");
        let bare = serde_json::to_value(without.def()).unwrap();
        assert!(bare.get("purpose").is_none());
    }

    #[test]
    fn decision_json_shape_is_stable() {
        let engine = Engine::new(review_loop()).unwrap();
        let d = engine.authorize(&snap("awaiting_worker", 0), &Attempt::new("worker", "working"));
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

    /// The reference loop plus permission to move a record between units of
    /// work: the worker may re-label the unit, the reviewer may not.
    fn with_rescopes() -> Engine {
        let mut def = review_loop();
        def.rescopes = vec![RescopeDef {
            label: "branch".to_string(),
            role: "worker".to_string(),
            requires_note: true,
        }];
        Engine::new(def).unwrap()
    }

    fn moving_to(branch: &str) -> BTreeMap<String, String> {
        BTreeMap::from([("branch".to_string(), branch.to_string())])
    }

    #[test]
    fn a_permitted_rescope_keeps_the_record_where_it_stands() {
        let engine = with_rescopes();
        let decision = engine.authorize_rescope(
            &snap("awaiting_worker", 2),
            "worker",
            &moving_to("follow-up"),
            Some("rides to the follow-up unit"),
        );
        let Decision::Allow { to, counter_updates, scope_updates } = decision else {
            panic!("a permitted rescope was refused: {decision:?}");
        };
        // The state does not move and nothing is spent: a rescope is not a
        // free pass, and it is not a state change wearing a different name.
        assert_eq!(to, "awaiting_worker");
        assert!(counter_updates.is_empty(), "a rescope spent a counter");
        assert_eq!(scope_updates, moving_to("follow-up"));
    }

    /// ⚠ Not configurable, and the reason is provenance: a finished record's
    /// scope says which unit of work it was resolved against, and no later
    /// reader can re-derive that once it is overwritten.
    #[test]
    fn a_finished_record_cannot_be_moved_to_another_unit_of_work() {
        let engine = with_rescopes();
        let decision = engine.authorize_rescope(
            &snap("approved", 1),
            "worker",
            &moving_to("follow-up"),
            Some("tidying the board"),
        );
        let Decision::Deny { reason } = decision else {
            panic!("a terminal record was rescoped");
        };
        assert!(reason.contains("terminal"), "{reason}");
    }

    #[test]
    fn rescope_refusals_tell_you_which_kind_of_no_it_is() {
        let engine = with_rescopes();
        let note = Some("because");
        // A role the label was not granted to.
        let Decision::Deny { reason } =
            engine.authorize_rescope(&snap("working", 0), "reviewer", &moving_to("x"), note)
        else {
            panic!("the reviewer was allowed a label it does not hold");
        };
        assert!(reason.contains("may not change scope label 'branch'"), "{reason}");

        // A label nobody was granted: a different problem with a different fix.
        let unknown = BTreeMap::from([("repo".to_string(), "other".to_string())]);
        let Decision::Deny { reason } =
            engine.authorize_rescope(&snap("working", 0), "worker", &unknown, note)
        else {
            panic!("an ungranted label was written");
        };
        assert!(reason.contains("does not say who may change"), "{reason}");

        // Nothing named at all.
        let Decision::Deny { reason } =
            engine.authorize_rescope(&snap("working", 0), "worker", &BTreeMap::new(), note)
        else {
            panic!("an empty rescope was allowed");
        };
        assert!(reason.contains("no scope labels"), "{reason}");
    }

    /// A scope change with no stated reason is indistinguishable from a record
    /// quietly disappearing out of one queue and into another.
    #[test]
    fn a_rescope_that_must_carry_a_reason_is_refused_without_one() {
        let engine = with_rescopes();
        for missing in [None, Some(""), Some("   ")] {
            let decision =
                engine.authorize_rescope(&snap("working", 0), "worker", &moving_to("x"), missing);
            let Decision::Deny { reason } = decision else {
                panic!("a rescope was allowed with note {missing:?}");
            };
            assert!(reason.contains("requires a note"), "{reason}");
        }
    }

    /// A workflow with no `rescopes` grants none. Scope is written at filing
    /// and stays there unless a definition says otherwise, so the default has
    /// to be the closed one.
    #[test]
    fn a_workflow_that_says_nothing_about_scope_permits_no_rescope() {
        let engine = Engine::new(review_loop()).unwrap();
        let decision = engine.authorize_rescope(
            &snap("working", 0),
            "worker",
            &moving_to("anywhere"),
            Some("why not"),
        );
        assert!(matches!(decision, Decision::Deny { .. }), "{decision:?}");
    }

    #[test]
    fn a_rescope_naming_an_actor_the_workflow_does_not_have_is_a_defect() {
        let mut def = review_loop();
        def.rescopes = vec![RescopeDef {
            label: "branch".to_string(),
            role: "archivist".to_string(),
            requires_note: false,
        }];
        assert_eq!(
            def.validate(),
            Err(ValidationError::UnknownRescopeRole("archivist".to_string()))
        );
    }

    #[test]
    fn the_same_label_granted_twice_to_one_role_is_a_defect() {
        let mut def = review_loop();
        let rule = RescopeDef {
            label: "branch".to_string(),
            role: "worker".to_string(),
            requires_note: false,
        };
        def.rescopes = vec![rule.clone(), RescopeDef { requires_note: true, ..rule }];
        assert_eq!(
            def.validate(),
            Err(ValidationError::DuplicateRescope {
                label: "branch".to_string(),
                role: "worker".to_string(),
            })
        );
    }

    /// ⚠ Exhaustion routes a record to a person, and an automatic route
    /// arrives with no question attached — which is the whole content of the
    /// handover. The actor that just ran out of attempts is the one who knows
    /// what cannot be settled, so that is where the reason is demanded.
    #[test]
    fn the_attempt_that_spends_a_ceiling_can_be_made_to_carry_the_question() {
        let mut def = review_loop();
        def.counters[0].exhausted_requires_note = true;
        let engine = Engine::new(def).unwrap();
        let spent = snap("awaiting_worker", 3);

        let silent = engine.authorize(&spent, &Attempt::new("worker", "working"));
        let Decision::Deny { reason } = silent else {
            panic!("a ceiling routed to a person with nothing said: {silent:?}");
        };
        assert!(reason.contains("escalated"), "{reason}");
        assert!(reason.contains("what decision is being asked for"), "{reason}");

        // With the question attached it routes, and the note rides into the
        // event the caller builds — which is where the person reads it.
        let asked = engine.authorize(
            &spent,
            &Attempt::new("worker", "working").saying("the corpus licence is unclear; ruling?"),
        );
        assert_eq!(
            asked,
            Decision::Exhausted {
                to: "escalated".to_string(),
                counter: "agent_passes".to_string()
            }
        );
    }

    /// ⚠ Only the LAST attempt is addressed to anybody. Demanding a reason for
    /// every claim would be `requires_note` on the transition, which is a
    /// different and much more expensive rule.
    #[test]
    fn requiring_the_question_does_not_tax_the_attempts_before_it() {
        let mut def = review_loop();
        def.counters[0].exhausted_requires_note = true;
        let engine = Engine::new(def).unwrap();
        for spent in [0u32, 1, 2] {
            let decision =
                engine.authorize(&snap("awaiting_worker", spent), &Attempt::new("worker", "working"));
            assert!(
                matches!(decision, Decision::Allow { .. }),
                "claim {spent} was taxed for a note: {decision:?}"
            );
        }
    }

    /// The decision surface offers moves WITH a placeholder note, so a move
    /// that merely lacks one is not reported impossible. That has to keep
    /// holding, or turning this rule on hides the escalation from the very
    /// view a person reads to find it.
    #[test]
    fn the_decision_surface_still_shows_where_a_spent_ceiling_would_route() {
        let mut def = review_loop();
        def.counters[0].exhausted_requires_note = true;
        let engine = Engine::new(def).unwrap();
        let moves = engine.next_moves(&snap("awaiting_worker", 3), "worker");
        assert!(
            moves.iter().any(|(t, d)| t.to == "working"
                && matches!(d, Decision::Exhausted { to, .. } if to == "escalated")),
            "the spent ceiling stopped being visible: {moves:?}"
        );
    }

    /// ⚠ The Decision contract is what every binding switches on. A rescope
    /// reuses `allow` rather than adding a fourth kind, and the new field is
    /// absent from an ordinary move — so a consumer written before rescope
    /// existed sees byte-identical JSON for everything it already handled.
    #[test]
    fn scope_updates_are_absent_from_an_ordinary_decision() {
        let engine = with_rescopes();
        let ordinary = engine.authorize(&snap("working", 1), &Attempt::new("worker", "awaiting_review"));
        let value = serde_json::to_value(&ordinary).unwrap();
        assert_eq!(
            value,
            json!({ "kind": "allow", "to": "awaiting_review", "counter_updates": {} })
        );

        let rescope = engine.authorize_rescope(
            &snap("working", 1),
            "worker",
            &moving_to("follow-up"),
            Some("rides"),
        );
        assert_eq!(
            serde_json::to_value(&rescope).unwrap(),
            json!({
                "kind": "allow",
                "to": "working",
                "counter_updates": {},
                "scope_updates": { "branch": "follow-up" }
            })
        );
    }
}

//! The interface a ledger adapter implements.
//!
//! [`ferrostep_core`] decides; something has to read the record it decided
//! about and persist what the decision said. That is an adapter, and this crate
//! is the contract it implements. The core depends on none of this — it opens
//! no connections and holds no state — so every side effect the engine implies
//! lives on this side of the boundary.
//!
//! Two things shape the whole design.
//!
//! **A record is an object, never a row.** A snapshot is a state and a set of
//! counters; an event is a value. Serialization, and whatever shape the store
//! wants it in, belongs to the adapter. Nothing here assumes tables, columns, a
//! query language, or a schema at all, because relational, document and
//! embedded key-value stores are all in range and an interface that quietly
//! assumes the first cannot reach the third.
//!
//! **An adapter states what it cannot guarantee.** Stores differ in what they
//! can promise, and the difference is invisible to a caller until it matters.
//! [`Capabilities`] makes it answerable up front rather than discovered during
//! an incident.

use std::collections::BTreeMap;
use std::fmt;

use ferrostep_core::{Decision, Snapshot};
use serde::{Deserialize, Serialize};

/// A record's identity in whatever store holds it.
///
/// Opaque on purpose: a row id, a key, a document id, a path. The engine never
/// reads it and no adapter should assume another adapter's shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RecordId(pub String);

/// A compare-and-swap token, read with a record and handed back to move it.
///
/// Opaque, and compared only for equality — a revision counter, an ETag, a row
/// version, a content hash all work. Equality is the entire requirement, which
/// is what keeps this reachable for stores that have no notion of a version and
/// must synthesize one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version(pub String);

/// A record as read: what it is, where it stands, and the token proving nobody
/// has moved it since.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub id: RecordId,
    pub snapshot: Snapshot,
    pub version: Version,
}

/// One line of history: who moved a record, what it cost, and why.
///
/// Nearly a serialized [`Decision`] plus the parts the engine cannot know —
/// which actor asked, and the reasoning a person attached. `from_state` is
/// here because a decision names only where a record is going.
///
/// The note is where a human's reasoning for releasing a paused record lives. A
/// record can be released more than once, so that reasoning has to survive the
/// next release; a single field on the record is overwritten by the second
/// decision, and a separate table of decisions would be a second chronology of
/// the same record, free to disagree with this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub actor: String,
    pub role: String,
    /// Where the record was. `None` when this event opened its history — a
    /// record being filed came from nowhere, and an empty string would be a
    /// state name that is merely blank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_state: Option<String>,
    pub decision: Decision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// An event as stored, carrying the ordering the store assigned it.
///
/// `seq` orders events within one record and `at` is when the store recorded
/// it. Ordering is `seq`, not `at`: a batch of writes can share a timestamp,
/// and a history that ties is a history you cannot replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: u64,
    pub at: String,
    #[serde(flatten)]
    pub event: Event,
}

/// Which records to look at.
///
/// Interpreted entirely by the adapter. The engine has no notion of a branch, a
/// repository, a project or a tenant, so a scope is whatever key/value pairs
/// the store can filter on — and an adapter that cannot filter on a given key
/// should say [`LedgerError::Unsupported`] rather than return everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope(BTreeMap<String, String>);

impl Scope {
    /// Every record the adapter can see.
    pub fn all() -> Self {
        Scope(BTreeMap::new())
    }

    /// Narrow to records whose `key` equals `value`.
    pub fn with(mut self, key: &str, value: &str) -> Self {
        self.0.insert(key.to_string(), value.to_string());
        self
    }

    pub fn filters(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether a record labelled with `labels` falls inside this scope: every
    /// filter key present, with an equal value. An empty scope matches
    /// everything, which is what [`Scope::all`] means. Adapters whose store
    /// cannot filter server-side apply this after reading — correctness first,
    /// and the cost is the adapter's to carry rather than the caller's to know
    /// about.
    pub fn matches(&self, labels: &BTreeMap<String, String>) -> bool {
        self.0.iter().all(|(key, value)| labels.get(key) == Some(value))
    }
}

/// The snapshot a decision leaves behind, or `None` for a denial.
///
/// One implementation, shared by every adapter, because two copies of "what
/// does applying mean" would be free to disagree. An [`Decision::Allow`] moves
/// the state and lays its counter updates over the existing counters; an
/// [`Decision::Exhausted`] moves the state and touches no counter — the spend
/// never happened, the record is being routed instead; a [`Decision::Deny`]
/// changes nothing and has nothing to persist.
pub fn decided_snapshot(current: &Snapshot, decision: &Decision) -> Option<Snapshot> {
    match decision {
        Decision::Allow { to, counter_updates, .. } => {
            let mut counters = current.counters.clone();
            for (name, value) in counter_updates {
                counters.insert(name.clone(), *value);
            }
            Some(Snapshot { state: to.clone(), counters })
        }
        Decision::Exhausted { to, .. } => Some(Snapshot {
            state: to.clone(),
            counters: current.counters.clone(),
        }),
        Decision::Deny { .. } => None,
    }
}

/// The scope labels a decision moves, empty for every ordinary move.
///
/// Companion to [`decided_snapshot`], and deliberately not a merge: the
/// updates are **absolute writes to named labels**. A rescope names the labels
/// it moves and says nothing about the rest, so an adapter sets exactly these
/// and leaves every other part of the record's identity alone. There is no
/// "resulting scope" to compute, which is why this returns the instruction
/// rather than an outcome — and why a record's other labels cannot be lost by
/// an adapter that reads a stale copy of them.
///
/// Empty is the common answer, which lets an adapter skip touching scope at
/// all on an ordinary move: a write that always fires is a write that can
/// always go wrong.
pub fn decided_scope_updates(decision: &Decision) -> &BTreeMap<String, String> {
    match decision {
        Decision::Allow { scope_updates, .. } => scope_updates,
        _ => {
            // A denial persists nothing, and an exhausted decision routes a
            // record without moving it between units of work.
            static NONE: std::sync::LazyLock<BTreeMap<String, String>> =
                std::sync::LazyLock::new(BTreeMap::new);
            &NONE
        }
    }
}

/// What an adapter can actually promise.
///
/// Every field here is false for some real store. Reporting them honestly is
/// the point: a caller that knows history is not enforced can say so in its
/// audit output, and one that does not will imply more than the store
/// delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// The state change, the counter changes and the event append land
    /// together or not at all. Where this is false, a crash between them
    /// leaves history disagreeing with the record, and the adapter should
    /// make that detectable rather than silent.
    pub atomic_apply: bool,
    /// A stale [`Version`] is refused rather than silently overwritten. Where
    /// this is false, two actors reading the same record can both succeed and
    /// one ceiling spend is lost — which turns a ceiling into a suggestion.
    ///
    /// ⚠ **Set this from a measurement under concurrent writers, never from
    /// reading an API.** A store can evaluate a version predicate with
    /// perfectly correct semantics and still have no serialization behind it:
    /// if the check happens before the write commits, any writer arriving in
    /// that window passes a predicate that is already stale. Measured on one
    /// candidate backend: **two** concurrent writers were enough to produce two
    /// winners, and a control with the predicate replaced by an always-true
    /// rule showed a textbook lost update — so the mechanism was working and
    /// its atomicity was not. Those are different properties and only the
    /// second one licenses this flag.
    ///
    /// ⚠ **And run the measurement repeatedly.** In that same experiment the
    /// first round passed cleanly — twelve writers, exactly one winner — which
    /// was luck. A single green round of a concurrency test is not evidence;
    /// what is evidence is many rounds and a failure count.
    ///
    /// ⚠ **The line that decides this flag is whether the compare happens
    /// inside the store's transaction, not inside its request.** On that same
    /// backend both were built and measured against each other. Compare and
    /// write within one transaction: 43 rounds, up to sixteen concurrent
    /// writers, zero failures, and the losers refused cleanly. Compare, then
    /// let the ordinary write proceed: at sixteen writers it returned 1, 2, 7,
    /// 6, 1 and 7 winners over six rounds — **two of the six looked perfectly
    /// correct**, and every failing round advanced the version once while
    /// telling several writers they had succeeded. That is a lost update
    /// wearing a success code, and it is the reason a passing round of the
    /// wrong design is more dangerous than an outright failure.
    pub compare_and_swap: bool,
    /// History cannot be rewritten or deleted by the actors that write it.
    /// Note what this does *not* claim: an administrator credential that can
    /// edit the records themselves makes guarding their history meaningless,
    /// so this is a defence against mistakes and not against malice.
    pub append_only_history: bool,
}

/// One thing a store was asked about, and whether it actually answered.
///
/// ⚠⚠ **THREE ANSWERS, NOT TWO, BECAUSE TWO OF THEM LOOK IDENTICAL AND MEAN
/// OPPOSITE THINGS.** "This column takes any string, so no state can be wrong"
/// and "nobody could tell me what this column takes" both reduce to *nothing
/// to report* if the type only has room for present and absent — and one of
/// them is a verified all-clear while the other is an unasked question wearing
/// its clothes. The first version of this type collapsed them, which is the
/// same defect it exists to prevent, one level up.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Answer<T> {
    /// The store answered, and this is what it said.
    Said(T),
    /// The store answered, and the answer is that it constrains nothing here:
    /// a state column that accepts any string, a write path with no fixed
    /// column list. **Checked** — a definition cannot disagree with it.
    NothingToConstrain,
    /// Nothing was learned. The adapter cannot look, the installed files
    /// predate the route, the credential was refused.
    ///
    /// ⚠ **Never a pass, and a report that renders it as one is broken.** It
    /// is the default so that a partially-filled [`StoreShape`] admits what it
    /// does not know rather than implying it looked.
    #[default]
    Unknown,
}

impl<T> Answer<T> {
    /// What the store said, if it said anything.
    pub fn said(&self) -> Option<&T> {
        match self {
            Answer::Said(value) => Some(value),
            _ => None,
        }
    }

    /// Whether this question went unanswered — the one case a caller must not
    /// silently fold into "fine".
    pub fn is_unknown(&self) -> bool {
        matches!(self, Answer::Unknown)
    }
}

/// What a store will actually accept — the other half of [`Capabilities`].
///
/// [`Capabilities`] says what an adapter can *do*; this says what the schema
/// behind it will *take*. A definition asserts things about a store without
/// ever checking them: its states must be values the state column accepts, and
/// where an adapter maps counters and scope labels onto real columns, those
/// columns must exist. Until this existed nothing verified any of it, and the
/// drift ran in the direction nothing goes red in — the JSON looks right, the
/// tests pass against the JSON, and the disagreement arrives as a refused
/// write on the first live transition.
///
/// ⚠⚠ **EVERY FIELD IS AN [`Answer`], SO "NOTHING CONSTRAINS THIS" AND
/// "NOBODY TOLD ME" CANNOT BE CONFUSED.** Only the first is a verified
/// all-clear. A checker that collapses them reports a clean bill of health it
/// never obtained — which is the failure mode that looks like success, and
/// therefore the one worth spending a type on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoreShape {
    /// Which collection, table or bucket this describes, in the store's own
    /// words. A report that names a fault has to name where it is.
    pub subject: String,
    /// The values the state column will accept.
    ///
    /// [`Answer::NothingToConstrain`] is the honest answer for a plain text
    /// column: it takes any string, so no state in any definition can be
    /// wrong against it, and saying so is a real result.
    pub accepted_states: Answer<Vec<String>>,
    /// Columns that exist, by name, paired with the store's own word for the
    /// type. The type is carried to be *shown*, not judged: an adapter knows
    /// what its store calls a column and does not know what the column being
    /// that type would cost.
    pub columns: Answer<BTreeMap<String, String>>,
    /// The column names the *installed* write path admits, grouped by whatever
    /// the adapter calls those groups.
    ///
    /// ⚠ Deliberately not interpreted here. This crate has no opinion about an
    /// adapter's grouping words, and a checker's job is to compare **names**
    /// and print the group it found them under. What makes this worth carrying
    /// at all is that an installed write path is deployed separately from the
    /// binary talking to it, so it can be older than the mapping it serves —
    /// and the failure that produces is a write that is accepted, dropped, and
    /// answered success.
    ///
    /// [`Answer::NothingToConstrain`] where the adapter writes columns
    /// directly and has no separately-deployed half that could be stale.
    pub writable: Answer<BTreeMap<String, Vec<String>>>,
}

/// Why a ledger operation could not be completed.
#[derive(Debug, Clone, PartialEq)]
pub enum LedgerError {
    NotFound(RecordId),
    /// Somebody else moved this record since it was read. The caller re-reads,
    /// asks the engine again, and retries — it must never overwrite, because
    /// the decision it holds was made about a record that no longer exists in
    /// that form.
    VersionConflict { id: RecordId, expected: Version },
    /// A decision that changes nothing reached [`Ledger::apply`]. A denial is
    /// not an event; there is nothing to persist and nothing to append.
    NothingToApply,
    /// The store cannot do what was asked. Saying so beats approximating it.
    ///
    /// ⚠ **Owned, so a refusal can name the thing it refused.** It was
    /// `&'static str`, which forced every refusal to be a fixed sentence — and
    /// the one that mattered most could not say WHICH column an installed file
    /// was unable to write, only that some column was. An adapter's whole job
    /// here is to state capabilities honestly; a refusal that cannot name its
    /// subject sends the reader looking, which is the cost the refusal exists
    /// to save.
    Unsupported(String),
    /// The record exists but is not in a shape this adapter understands.
    Malformed { id: RecordId, detail: String },
    /// The store could not be reached, or answered in a way that was not
    /// about this request.
    Transport(String),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use LedgerError::*;
        match self {
            NotFound(id) => write!(f, "no record '{}'", id.0),
            VersionConflict { id, expected } => write!(
                f,
                "record '{}' changed since it was read at version '{}'; re-read and decide again",
                id.0, expected.0
            ),
            NothingToApply => write!(f, "a denial has nothing to persist"),
            Unsupported(what) => write!(f, "this ledger cannot {what}"),
            Malformed { id, detail } => write!(f, "record '{}' is not readable: {detail}", id.0),
            Transport(detail) => write!(f, "could not reach the ledger: {detail}"),
        }
    }
}

impl std::error::Error for LedgerError {}

/// A store the engine's decisions can be applied to.
///
/// Implementations are expected to be cheap to construct and safe to share; a
/// caller performs one decision and moves on, so nothing here is async. The
/// actors in a FerroStep loop are separate processes, which means concurrency
/// is the store's problem — and [`Capabilities::compare_and_swap`] is how an
/// adapter says whether the store is actually solving it.
pub trait Ledger {
    /// What this adapter can promise. Callers that surface guarantees to a
    /// person should read this rather than assume the strongest reading.
    fn capabilities(&self) -> Capabilities;

    /// Read one record, with the token needed to move it.
    fn load(&self, id: &RecordId) -> Result<Record, LedgerError>;

    /// File a new record into `scope`, in the state the decision names, and
    /// open its history with `event`.
    ///
    /// ⚠ A filing decision's counter updates are **scope-level, not
    /// record-level**. A budget on how much work a round may create belongs to
    /// the branch or the cycle, never to the record being created — so an
    /// adapter that persists them onto the new record has stored them where
    /// nothing will find them again.
    fn create(
        &self,
        scope: &Scope,
        decision: &Decision,
        event: &Event,
    ) -> Result<Record, LedgerError>;

    /// Persist a decision and append its event, against the version the record
    /// was read at.
    ///
    /// The decision travels inside the event, so the history and the record
    /// cannot disagree about what happened — they are written from one value.
    /// Returns the record's new version.
    ///
    /// Fails with [`LedgerError::VersionConflict`] if the record moved since
    /// it was read, and with [`LedgerError::NothingToApply`] if handed a
    /// denial.
    fn apply(&self, record: &Record, event: &Event) -> Result<Version, LedgerError>;

    /// Every record within `scope` currently in one of `states`.
    ///
    /// The caller decides which states matter — the definition knows which are
    /// pauses and which are endings, and the adapter does not. Results are
    /// complete: an adapter whose store pages must follow the pages, because a
    /// truncated answer is indistinguishable from a short one.
    fn select(&self, scope: &Scope, states: &[String]) -> Result<Vec<Record>, LedgerError>;

    /// One record's history, oldest first.
    fn history(&self, id: &RecordId) -> Result<Vec<StoredEvent>, LedgerError>;

    /// What the store behind this adapter will accept, read from the store
    /// itself — so a definition can be checked against it before a transition
    /// proves it wrong.
    ///
    /// ⚠⚠ **PROVIDED, AND REFUSING BY DEFAULT, DELIBERATELY.** An adapter that
    /// has not implemented this must not be indistinguishable from one that
    /// looked and found nothing wrong. The default is the only safe answer,
    /// and every other spelling of it — an empty [`StoreShape`], an `Ok` with
    /// `None` everywhere — reads as a clean result to a caller that is not
    /// being careful. A refusal cannot be misread that way.
    ///
    /// Read-only. Nothing here writes, and a caller may run it against a live
    /// store without taking a lock, spending a ceiling or appending an event.
    fn store_shape(&self) -> Result<StoreShape, LedgerError> {
        Err(LedgerError::Unsupported("describe the schema it stores into".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrostep_core::Decision;

    fn allow() -> Decision {
        Decision::allow("working", BTreeMap::from([("passes".to_string(), 1)]))
    }

    #[test]
    fn an_event_round_trips_with_its_decision_inside_it() {
        let event = Event {
            actor: "ada".to_string(),
            role: "worker".to_string(),
            from_state: Some("open".to_string()),
            decision: allow(),
            note: Some("picked this up after the outage".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
        // The decision is nested rather than flattened, so a reader can hand
        // the same bytes back to code that switches on Decision.
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["decision"]["kind"], "allow");
    }

    #[test]
    fn a_note_is_absent_rather_than_null_when_there_is_none() {
        let event = Event {
            actor: "worker-1".to_string(),
            role: "worker".to_string(),
            from_state: Some("open".to_string()),
            decision: allow(),
            note: None,
        };
        let value = serde_json::to_value(&event).unwrap();
        assert!(!value.as_object().unwrap().contains_key("note"));
    }

    #[test]
    fn a_stored_event_carries_its_ordering_alongside_the_event() {
        let stored = StoredEvent {
            seq: 12,
            at: "2026-08-21 14:02:11.412Z".to_string(),
            event: Event {
                actor: "a-person".to_string(),
                role: "operator".to_string(),
                from_state: Some("escalated".to_string()),
                decision: allow(),
                note: None,
            },
        };
        // Flattened: one object, not an event nested under a wrapper, so a
        // store's own row maps to it without a level of indirection.
        let value = serde_json::to_value(&stored).unwrap();
        assert_eq!(value["seq"], 12);
        assert_eq!(value["role"], "operator");
        assert_eq!(serde_json::from_value::<StoredEvent>(value).unwrap(), stored);
    }

    #[test]
    fn a_filed_record_came_from_nowhere_rather_than_from_nothing() {
        // A creation event has no prior state, and the difference between that
        // and a blank one is the difference between "this began here" and "a
        // state name went missing".
        let filed = Event {
            actor: "reviewer-1".to_string(),
            role: "reviewer".to_string(),
            from_state: None,
            decision: allow(),
            note: Some("the gate is enumerated by nothing".to_string()),
        };
        let value = serde_json::to_value(&filed).unwrap();
        assert!(!value.as_object().unwrap().contains_key("from_state"));
        assert_eq!(serde_json::from_value::<Event>(value).unwrap(), filed);
    }

    #[test]
    fn a_scope_narrows_and_reports_what_it_narrowed_on() {
        assert!(Scope::all().is_empty());
        let scope = Scope::all().with("repo", "example/thing").with("branch", "main");
        assert_eq!(scope.filters().len(), 2);
        assert_eq!(scope.filters()["branch"], "main");
    }

    #[test]
    fn a_decision_becomes_the_snapshot_it_promised() {
        let current = Snapshot {
            state: "awaiting_worker".to_string(),
            counters: BTreeMap::from([("passes".to_string(), 2), ("filings".to_string(), 1)]),
        };
        // Allow: state moves, named counters take their new values, unnamed
        // counters survive untouched.
        let next = decided_snapshot(&current, &allow()).unwrap();
        assert_eq!(next.state, "working");
        assert_eq!(next.counters["passes"], 1);
        assert_eq!(next.counters["filings"], 1);
        // Exhausted: the record is routed, and nothing is spent by routing.
        let routed = decided_snapshot(
            &current,
            &Decision::Exhausted { to: "escalated".to_string(), counter: "passes".to_string() },
        )
        .unwrap();
        assert_eq!(routed.state, "escalated");
        assert_eq!(routed.counters, current.counters);
        // Deny: nothing to persist.
        let denied = Decision::Deny { reason: "not yours".to_string() };
        assert_eq!(decided_snapshot(&current, &denied), None);
    }

    /// The companion to the test above, and it exists for the same reason:
    /// this is the *one* implementation of "what does applying mean", so an
    /// adapter that reads it wrong is every adapter reading it wrong.
    ///
    /// ⚠ The instruction is **absolute writes to named labels, never a
    /// merge of whole scopes**. A rescope says what it moves and nothing
    /// about the rest, which is what stops an adapter holding a stale copy of
    /// a record's other labels from dropping them.
    #[test]
    fn a_decision_hands_over_the_scope_labels_it_moves_and_no_others() {
        // The ordinary move: empty, so an adapter can skip touching scope at
        // all — a write that always fires is a write that can always go wrong.
        assert!(decided_scope_updates(&allow()).is_empty());
        // Routing does not move a record between units of work, and a denial
        // persists nothing.
        assert!(
            decided_scope_updates(&Decision::Exhausted {
                to: "escalated".to_string(),
                counter: "passes".to_string(),
            })
            .is_empty()
        );
        assert!(
            decided_scope_updates(&Decision::Deny { reason: "not yours".to_string() }).is_empty()
        );

        // A rescope: the labels it names, and only those. `cycle` is absent
        // rather than carried over, because this is an instruction and not a
        // resulting scope — the adapter merges it against what is stored.
        let rescope = Decision::Allow {
            to: "awaiting_review".to_string(),
            counter_updates: BTreeMap::new(),
            scope_updates: BTreeMap::from([("branch".to_string(), "release-2".to_string())]),
        };
        let moved = decided_scope_updates(&rescope);
        assert_eq!(moved.len(), 1, "only the named label travels: {moved:?}");
        assert_eq!(moved["branch"], "release-2");
        assert!(!moved.contains_key("cycle"), "an unnamed label is not an instruction");
    }

    #[test]
    fn a_scope_matches_on_every_filter_and_an_empty_one_matches_all() {
        let labels = BTreeMap::from([
            ("repo".to_string(), "example/thing".to_string()),
            ("branch".to_string(), "main".to_string()),
        ]);
        assert!(Scope::all().matches(&labels));
        assert!(Scope::all().with("branch", "main").matches(&labels));
        assert!(!Scope::all().with("branch", "other").matches(&labels));
        assert!(!Scope::all().with("cycle", "7").matches(&labels), "an absent key is not a match");
    }

    #[test]
    fn a_version_conflict_says_what_to_do_about_it() {
        let e = LedgerError::VersionConflict {
            id: RecordId("abc".to_string()),
            expected: Version("7".to_string()),
        };
        let message = e.to_string();
        assert!(message.contains("re-read"), "the message must name the remedy: {message}");
    }

    /// ⚠⚠ **AN ADAPTER THAT HAS NOT LOOKED MUST NOT ANSWER LIKE ONE THAT DID.**
    /// The default is a refusal rather than an empty [`StoreShape`], and the
    /// difference is the whole point: an empty shape is a value a caller can
    /// render as "no problems found", and this repo has already shipped one
    /// green signal that came from a check that never ran. A refusal cannot be
    /// formatted into a pass.
    ///
    /// Written against a bare implementation — one that overrides nothing —
    /// because that is exactly the adapter this protects a caller from.
    #[test]
    fn an_adapter_that_cannot_describe_its_store_refuses_rather_than_reporting_nothing_wrong() {
        struct Bare;
        impl Ledger for Bare {
            fn capabilities(&self) -> Capabilities {
                Capabilities {
                    atomic_apply: false,
                    compare_and_swap: false,
                    append_only_history: false,
                }
            }
            fn load(&self, id: &RecordId) -> Result<Record, LedgerError> {
                Err(LedgerError::NotFound(id.clone()))
            }
            fn create(
                &self,
                _scope: &Scope,
                _decision: &Decision,
                _event: &Event,
            ) -> Result<Record, LedgerError> {
                Err(LedgerError::NothingToApply)
            }
            fn apply(&self, _record: &Record, _event: &Event) -> Result<Version, LedgerError> {
                Err(LedgerError::NothingToApply)
            }
            fn select(&self, _scope: &Scope, _states: &[String]) -> Result<Vec<Record>, LedgerError> {
                Ok(Vec::new())
            }
            fn history(&self, id: &RecordId) -> Result<Vec<StoredEvent>, LedgerError> {
                Err(LedgerError::NotFound(id.clone()))
            }
        }

        let err = Bare.store_shape().unwrap_err();
        assert!(
            matches!(err, LedgerError::Unsupported(_)),
            "a store that cannot be described must refuse, got {err:?}"
        );
        // And the refusal has to read as one to a person, not as an absence.
        let said = err.to_string();
        assert!(said.contains("cannot"), "{said}");
        assert!(said.contains("schema"), "{said}");
    }

    /// ⚠⚠ **THE THREE ANSWERS A CHECKER MUST NEVER COLLAPSE**, held apart by
    /// the type rather than by everyone remembering. The first draft of
    /// [`StoreShape`] used `Option`, which had room for two of them — and the
    /// pair it merged was *verified all-clear* with *never asked*.
    #[test]
    fn nothing_to_constrain_and_nobody_said_are_different_answers() {
        let never_asked: Answer<Vec<String>> = Answer::Unknown;
        let nothing_constrains: Answer<Vec<String>> = Answer::NothingToConstrain;
        let constrains_nothing_through: Answer<Vec<String>> = Answer::Said(Vec::new());

        assert_ne!(never_asked, nothing_constrains);
        assert_ne!(nothing_constrains, constrains_nothing_through);
        assert_ne!(never_asked, constrains_nothing_through);

        // Only one of the three is the unasked question, and it is the only
        // one `is_unknown` may claim.
        assert!(never_asked.is_unknown());
        assert!(!nothing_constrains.is_unknown());
        assert!(!constrains_nothing_through.is_unknown());

        // `said` is for reading a list, and must not manufacture one.
        assert_eq!(never_asked.said(), None);
        assert_eq!(nothing_constrains.said(), None);
        assert_eq!(constrains_nothing_through.said(), Some(&Vec::new()));
    }

    /// ⚠ A shape nobody filled in must admit that, field by field. The
    /// default exists so a half-built report cannot imply it looked.
    #[test]
    fn a_shape_nobody_filled_in_admits_it_knows_nothing() {
        let blank = StoreShape::default();
        assert!(blank.accepted_states.is_unknown());
        assert!(blank.columns.is_unknown());
        assert!(blank.writable.is_unknown());
    }
}

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
    pub compare_and_swap: bool,
    /// History cannot be rewritten or deleted by the actors that write it.
    /// Note what this does *not* claim: an administrator credential that can
    /// edit the records themselves makes guarding their history meaningless,
    /// so this is a defence against mistakes and not against malice.
    pub append_only_history: bool,
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
    Unsupported(&'static str),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrostep_core::Decision;

    fn allow() -> Decision {
        Decision::Allow {
            to: "working".to_string(),
            counter_updates: BTreeMap::from([("passes".to_string(), 1)]),
        }
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
                actor: "lmcfarlin".to_string(),
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
    fn a_version_conflict_says_what_to_do_about_it() {
        let e = LedgerError::VersionConflict {
            id: RecordId("abc".to_string()),
            expected: Version("7".to_string()),
        };
        let message = e.to_string();
        assert!(message.contains("re-read"), "the message must name the remedy: {message}");
    }
}

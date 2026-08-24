//! ferrostep-sqlite — the ledger on a file, for loops on one host.
//!
//! This is the zero-install path: no server, no account, no configuration — a
//! database file in WAL mode, which supports exactly the deployment it exists
//! for (every actor a separate process on the same host; readers and writers
//! concurrent, one writer at a time). The moment actors span machines this is
//! the wrong adapter by SQLite's own rule: WAL needs shared memory between
//! processes, and a database file on a network share is corruption waiting
//! rather than a small-team deployment.
//!
//! The guarantee story, which is the point of an adapter:
//!
//! * **Apply is one `IMMEDIATE` transaction.** The compare, the record write
//!   and the event append land together or not at all.
//! * **Compare-and-swap by construction**: `UPDATE … WHERE id = ? AND
//!   version = ?` — the compare and the write are one statement inside that
//!   transaction, which is the side of the line the measured record says a
//!   compare must be on.
//! * **History is append-only because the storage says so**: triggers refuse
//!   `UPDATE` and `DELETE` on the event table. A defence against mistakes,
//!   not malice — whoever holds the file can drop the trigger, which is the
//!   caveat [`Capabilities::append_only_history`] itself documents.
//!
//! Every operation opens its own connection. That is the honest shape of the
//! deployment — each actor is a separate process holding nothing shared — and
//! it keeps this type trivially safe to share across threads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ferrostep_core::{Decision, Snapshot};
use ferrostep_ledger::{
    decided_scope_updates, decided_snapshot, Capabilities, Event, Ledger, LedgerError, Record,
    RecordId, Scope, StoredEvent, Version,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

/// Applied idempotently on every open: `IF NOT EXISTS` throughout, so opening
/// an existing ledger is a no-op and two processes racing to open cannot hurt
/// each other.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS ferrostep_records (
    id       INTEGER PRIMARY KEY,
    state    TEXT NOT NULL,
    counters TEXT NOT NULL DEFAULT '{}',
    scope    TEXT NOT NULL DEFAULT '{}',
    version  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS ferrostep_events (
    record_id  INTEGER NOT NULL REFERENCES ferrostep_records(id),
    seq        INTEGER NOT NULL,
    at         TEXT NOT NULL,
    actor      TEXT NOT NULL,
    role       TEXT NOT NULL,
    from_state TEXT,
    decision   TEXT NOT NULL,
    note       TEXT,
    PRIMARY KEY (record_id, seq)
);
CREATE TRIGGER IF NOT EXISTS ferrostep_events_no_update
BEFORE UPDATE ON ferrostep_events
BEGIN SELECT RAISE(ABORT, 'ferrostep_events is append-only'); END;
CREATE TRIGGER IF NOT EXISTS ferrostep_events_no_delete
BEFORE DELETE ON ferrostep_events
BEGIN SELECT RAISE(ABORT, 'ferrostep_events is append-only'); END;
";

/// A FerroStep ledger in a SQLite database file.
#[derive(Debug, Clone)]
pub struct SqliteLedger {
    path: PathBuf,
}

impl SqliteLedger {
    /// Open a ledger at `path`, creating the file and schema if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, LedgerError> {
        let ledger = SqliteLedger { path: path.as_ref().to_path_buf() };
        let conn = ledger.conn()?;
        conn.execute_batch(SCHEMA).map_err(transport)?;
        Ok(ledger)
    }

    fn conn(&self) -> Result<Connection, LedgerError> {
        let conn = Connection::open(&self.path).map_err(transport)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(transport)?;
        // WAL is what lets concurrent actors on one host coexist; a no-op on
        // a database already in WAL mode.
        conn.pragma_update(None, "journal_mode", "WAL").map_err(transport)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(transport)?;
        Ok(conn)
    }
}

fn transport(e: rusqlite::Error) -> LedgerError {
    LedgerError::Transport(e.to_string())
}

/// This adapter's ids are the table's integer keys. An id of another shape
/// names a record no ledger of this shape ever issued, which is a not-found
/// rather than a malfunction.
fn row_key(id: &RecordId) -> Result<i64, LedgerError> {
    id.0.parse().map_err(|_| LedgerError::NotFound(id.clone()))
}

fn counters_json(counters: &BTreeMap<String, u32>) -> String {
    serde_json::to_string(counters).expect("a map of strings to integers serializes")
}

fn parse_counters(id: &RecordId, json: &str) -> Result<BTreeMap<String, u32>, LedgerError> {
    serde_json::from_str(json).map_err(|e| LedgerError::Malformed {
        id: id.clone(),
        detail: format!("counters column: {e}"),
    })
}

fn insert_event(
    tx: &Transaction<'_>,
    record: i64,
    seq: i64,
    event: &Event,
) -> Result<(), LedgerError> {
    let decision = serde_json::to_string(&event.decision)
        .map_err(|e| LedgerError::Transport(format!("decision does not serialize: {e}")))?;
    tx.execute(
        "INSERT INTO ferrostep_events (record_id, seq, at, actor, role, from_state, decision, note)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?3, ?4, ?5, ?6, ?7)",
        params![record, seq, event.actor, event.role, event.from_state, decision, event.note],
    )
    .map_err(transport)?;
    Ok(())
}

impl Ledger for SqliteLedger {
    fn capabilities(&self) -> Capabilities {
        // Each flag is earned by this crate's own tests rather than asserted:
        // the battery for compare_and_swap, the trigger test for append-only,
        // the conflict tests for atomicity of the paired writes.
        Capabilities {
            atomic_apply: true,
            compare_and_swap: true,
            append_only_history: true,
        }
    }

    fn load(&self, id: &RecordId) -> Result<Record, LedgerError> {
        let key = row_key(id)?;
        let conn = self.conn()?;
        let row = conn
            .query_row(
                "SELECT state, counters, version FROM ferrostep_records WHERE id = ?1",
                [key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
            )
            .optional()
            .map_err(transport)?;
        let Some((state, counters, version)) = row else {
            return Err(LedgerError::NotFound(id.clone()));
        };
        Ok(Record {
            id: id.clone(),
            snapshot: Snapshot { state, counters: parse_counters(id, &counters)? },
            version: Version(version.to_string()),
        })
    }

    fn create(
        &self,
        scope: &Scope,
        decision: &Decision,
        event: &Event,
    ) -> Result<Record, LedgerError> {
        let Decision::Allow { to, .. } = decision else {
            // A denial has nothing to persist; an exhausted filing budget
            // means no record is created — the matter escalates, the record
            // does not exist to carry it.
            return Err(LedgerError::NothingToApply);
        };
        // The decision's counter updates are deliberately NOT written to the
        // new record: a filing spend is scope-level (the interface's own
        // warning), and a caller reads such counters from where it computes
        // them, never from a record that happened to be filed under them.
        let scope_json = serde_json::to_string(scope.filters())
            .expect("a map of strings to strings serializes");
        let mut conn = self.conn()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(transport)?;
        tx.execute(
            "INSERT INTO ferrostep_records (state, counters, scope, version) VALUES (?1, '{}', ?2, 1)",
            params![to, scope_json],
        )
        .map_err(transport)?;
        let key = tx.last_insert_rowid();
        insert_event(&tx, key, 1, event)?;
        tx.commit().map_err(transport)?;
        Ok(Record {
            id: RecordId(key.to_string()),
            snapshot: Snapshot { state: to.clone(), counters: BTreeMap::new() },
            version: Version("1".to_string()),
        })
    }

    fn apply(&self, record: &Record, event: &Event) -> Result<Version, LedgerError> {
        let Some(next) = decided_snapshot(&record.snapshot, &event.decision) else {
            return Err(LedgerError::NothingToApply);
        };
        let key = row_key(&record.id)?;
        let expected: i64 = record.version.0.parse().map_err(|_| LedgerError::Malformed {
            id: record.id.clone(),
            detail: format!("version token '{}' is not this adapter's shape", record.version.0),
        })?;
        let mut conn = self.conn()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(transport)?;
        // A rescope names labels rather than a whole scope, so the stored
        // labels are read and merged here. Safe to read-then-write only
        // because this transaction is IMMEDIATE — the write lock is already
        // held, so nothing can slip between the read and the update — and the
        // version compare below still guards the whole thing.
        let scope_updates = decided_scope_updates(&event.decision);
        let next_scope = if scope_updates.is_empty() {
            None
        } else {
            let stored: String = tx
                .query_row("SELECT scope FROM ferrostep_records WHERE id = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(transport)?
                .ok_or_else(|| LedgerError::NotFound(record.id.clone()))?;
            let mut labels: BTreeMap<String, String> =
                serde_json::from_str(&stored).map_err(|e| LedgerError::Malformed {
                    id: record.id.clone(),
                    detail: format!("stored scope is not a label map: {e}"),
                })?;
            labels.extend(scope_updates.iter().map(|(k, v)| (k.clone(), v.clone())));
            Some(serde_json::to_string(&labels).unwrap_or_else(|_| "{}".to_string()))
        };
        // The compare and the write are one statement, inside the transaction
        // the event append also lives in. `WHERE version = ?` is the entire
        // compare-and-swap; a stale caller changes zero rows.
        //
        // ⚠ Scope is written by the SAME statement as the state and counters.
        // A rescope that landed in its own write could half-apply, and a
        // record whose scope moved while its history says it did not is
        // invisible to every query that looks for it.
        let moved = match &next_scope {
            None => tx.execute(
                "UPDATE ferrostep_records SET state = ?1, counters = ?2, version = version + 1
                 WHERE id = ?3 AND version = ?4",
                params![next.state, counters_json(&next.counters), key, expected],
            ),
            Some(scope) => tx.execute(
                "UPDATE ferrostep_records
                 SET state = ?1, counters = ?2, scope = ?3, version = version + 1
                 WHERE id = ?4 AND version = ?5",
                params![next.state, counters_json(&next.counters), scope, key, expected],
            ),
        }
        .map_err(transport)?;
        if moved == 0 {
            let held: Option<i64> = tx
                .query_row("SELECT version FROM ferrostep_records WHERE id = ?1", [key], |r| {
                    r.get(0)
                })
                .optional()
                .map_err(transport)?;
            return Err(match held {
                None => LedgerError::NotFound(record.id.clone()),
                Some(_) => LedgerError::VersionConflict {
                    id: record.id.clone(),
                    expected: record.version.clone(),
                },
            });
        }
        let seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(seq), 0) + 1 FROM ferrostep_events WHERE record_id = ?1",
                [key],
                |r| r.get(0),
            )
            .map_err(transport)?;
        insert_event(&tx, key, seq, event)?;
        tx.commit().map_err(transport)?;
        Ok(Version((expected + 1).to_string()))
    }

    fn select(&self, scope: &Scope, states: &[String]) -> Result<Vec<Record>, LedgerError> {
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;
        let placeholders = vec!["?"; states.len()].join(", ");
        let sql = format!(
            "SELECT id, state, counters, scope, version FROM ferrostep_records
             WHERE state IN ({placeholders}) ORDER BY id"
        );
        let mut stmt = conn.prepare(&sql).map_err(transport)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(states), |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })
            .map_err(transport)?;
        let mut out = Vec::new();
        for row in rows {
            let (key, state, counters, scope_json, version) = row.map_err(transport)?;
            let id = RecordId(key.to_string());
            // Scope filtering happens here, in the adapter's own language,
            // rather than through a JSON path expression — a label key that
            // happens to contain path syntax cannot be misread, and the
            // result is complete by construction.
            let labels: BTreeMap<String, String> =
                serde_json::from_str(&scope_json).map_err(|e| LedgerError::Malformed {
                    id: id.clone(),
                    detail: format!("scope column: {e}"),
                })?;
            if !scope.matches(&labels) {
                continue;
            }
            let snapshot = Snapshot { state, counters: parse_counters(&id, &counters)? };
            out.push(Record { id, snapshot, version: Version(version.to_string()) });
        }
        Ok(out)
    }

    fn history(&self, id: &RecordId) -> Result<Vec<StoredEvent>, LedgerError> {
        let key = row_key(id)?;
        let conn = self.conn()?;
        let exists: Option<i64> = conn
            .query_row("SELECT 1 FROM ferrostep_records WHERE id = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(transport)?;
        if exists.is_none() {
            return Err(LedgerError::NotFound(id.clone()));
        }
        let mut stmt = conn
            .prepare(
                "SELECT seq, at, actor, role, from_state, decision, note
                 FROM ferrostep_events WHERE record_id = ?1 ORDER BY seq",
            )
            .map_err(transport)?;
        let rows = stmt
            .query_map([key], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(transport)?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, at, actor, role, from_state, decision, note) = row.map_err(transport)?;
            let decision: Decision =
                serde_json::from_str(&decision).map_err(|e| LedgerError::Malformed {
                    id: id.clone(),
                    detail: format!("event {seq} decision: {e}"),
                })?;
            out.push(StoredEvent {
                seq: seq as u64,
                at,
                event: Event { actor, role, from_state, decision, note },
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    use ferrostep_core::{Attempt, Engine, WorkflowDef};

    fn temp_ledger() -> (tempfile::TempDir, SqliteLedger) {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SqliteLedger::open(dir.path().join("ledger.db")).unwrap();
        (dir, ledger)
    }

    fn event(actor: &str, role: &str, from: Option<&str>, decision: Decision) -> Event {
        Event {
            actor: actor.to_string(),
            role: role.to_string(),
            from_state: from.map(str::to_string),
            decision,
            note: None,
        }
    }

    fn filed(to: &str) -> Decision {
        Decision::allow(to, BTreeMap::new())
    }

    /// Seed one record the way a harness does for a workflow with no
    /// `creation` clause: an operator files it by hand.
    fn seed(ledger: &SqliteLedger, state: &str) -> Record {
        ledger
            .create(
                &Scope::all().with("branch", "fix/gate"),
                &filed(state),
                &event("lauren", "operator", None, filed(state)),
            )
            .unwrap()
    }

    #[test]
    fn the_reference_loop_runs_end_to_end_on_this_ledger() {
        let def = WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        let engine = Engine::new(def).unwrap();
        let (_dir, ledger) = temp_ledger();
        let record = seed(&ledger, &engine.def().initial);

        // One step of the loop: authorize against the loaded record, persist
        // what the decision says, reload. Exactly what a harness does.
        let step = |id: &RecordId, actor: &str, role: &str, to: &str, note: Option<&str>| {
            let current = ledger.load(id).unwrap();
            let mut attempt = Attempt::new(role, to);
            if let Some(n) = note {
                attempt = attempt.saying(n);
            }
            let decision = engine.authorize(&current.snapshot, &attempt);
            let event = Event {
                actor: actor.to_string(),
                role: role.to_string(),
                from_state: Some(current.snapshot.state.clone()),
                decision,
                note: note.map(str::to_string),
            };
            ledger.apply(&current, &event).map(|v| (v, event.decision))
        };

        // Three worker passes, each claimed (spending), submitted, sent back.
        for pass in 1..=3u32 {
            let (_, d) = step(&record.id, "cyndi", "worker", "working", None).unwrap();
            assert!(matches!(d, Decision::Allow { .. }), "pass {pass} claim: {d:?}");
            let after = ledger.load(&record.id).unwrap();
            assert_eq!(after.snapshot.counters["agent_passes"], pass, "spend lands on claim");
            step(&record.id, "cyndi", "worker", "awaiting_review", None).unwrap();
            if pass < 3 {
                step(&record.id, "sam", "reviewer", "awaiting_worker", None).unwrap();
            }
        }
        // Reviewer sends the third pass back too; the fourth claim exhausts.
        step(&record.id, "sam", "reviewer", "awaiting_worker", None).unwrap();
        let (_, d) = step(&record.id, "cyndi", "worker", "working", None).unwrap();
        assert!(
            matches!(d, Decision::Exhausted { .. }),
            "the ceiling routes instead of allowing: {d:?}"
        );
        let escalated = ledger.load(&record.id).unwrap();
        assert_eq!(escalated.snapshot.state, "escalated");
        assert_eq!(
            escalated.snapshot.counters["agent_passes"], 3,
            "routing to escalation spends nothing"
        );
        assert_eq!(engine.status(&escalated.snapshot), ferrostep_core::Status::NeedsPerson);

        // The operator releases the halt: state move and counter clear in one
        // decision, so the ledger takes both in one write.
        step(&record.id, "lauren", "operator", "awaiting_worker", Some("one more try")).unwrap();
        let released = ledger.load(&record.id).unwrap();
        assert_eq!(released.snapshot.counters["agent_passes"], 0);

        // A fresh pass gets approved.
        step(&record.id, "cyndi", "worker", "working", None).unwrap();
        step(&record.id, "cyndi", "worker", "awaiting_review", None).unwrap();
        step(&record.id, "sam", "reviewer", "approved", None).unwrap();
        let done = ledger.load(&record.id).unwrap();
        assert_eq!(engine.status(&done.snapshot), ferrostep_core::Status::Ended);

        // The history replays: contiguous sequence, every decision inside it,
        // and the version counted every write.
        let history = ledger.history(&record.id).unwrap();
        assert!(history.len() > 3, "the loop above wrote more history than this");
        for (i, e) in history.iter().enumerate() {
            assert_eq!(e.seq, i as u64 + 1, "seq is contiguous from 1");
        }
        assert_eq!(history[0].event.from_state, None, "the filing came from nowhere");
        assert_eq!(done.version.0, history.len().to_string(), "one version step per write");
        assert_eq!(
            history.iter().filter(|e| matches!(e.event.decision, Decision::Exhausted { .. })).count(),
            1,
            "exactly one exhaustion in this history"
        );
    }

    #[test]
    fn a_stale_writer_is_refused_and_a_reread_recovers() {
        let (_dir, ledger) = temp_ledger();
        let record = seed(&ledger, "open");
        let first = ledger.load(&record.id).unwrap();
        let second = ledger.load(&record.id).unwrap();

        let claim = Decision::allow("working", BTreeMap::from([("passes".to_string(), 1)]));
        ledger.apply(&first, &event("a", "worker", Some("open"), claim.clone())).unwrap();
        let refused = ledger.apply(&second, &event("b", "worker", Some("open"), claim.clone()));
        assert!(
            matches!(refused, Err(LedgerError::VersionConflict { .. })),
            "the second writer held a stale version: {refused:?}"
        );

        // The remedy the error names: re-read and decide again.
        let reread = ledger.load(&record.id).unwrap();
        assert_eq!(reread.snapshot.state, "working");
        assert_eq!(reread.snapshot.counters["passes"], 1, "exactly one spend survived");
        ledger
            .apply(&reread, &event("b", "worker", Some("working"), filed("review")))
            .unwrap();
    }

    #[test]
    fn compare_and_swap_holds_under_concurrent_writers_over_repeated_rounds() {
        // One green round of a concurrency test is a coin landing your way
        // (AGENTS.md); this runs many and counts. Each writer opens its own
        // ledger over the same file — the separate-processes shape.
        const WRITERS: usize = 8;
        const ROUNDS: usize = 20;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.db");
        let ledger = SqliteLedger::open(&path).unwrap();
        let record = seed(&ledger, "spin");

        let mut attempts = 0usize;
        for round in 0..ROUNDS {
            let current = Arc::new(ledger.load(&record.id).unwrap());
            let barrier = Arc::new(Barrier::new(WRITERS));
            let results: Vec<Result<Version, LedgerError>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..WRITERS)
                    .map(|w| {
                        let path = path.clone();
                        let current = Arc::clone(&current);
                        let barrier = Arc::clone(&barrier);
                        s.spawn(move || {
                            let own = SqliteLedger::open(&path).unwrap();
                            let claim = Decision::allow(
                                "spin",
                                BTreeMap::from([("wins".to_string(), round as u32 + 1)]),
                            );
                            barrier.wait();
                            own.apply(
                                &current,
                                &event(&format!("w{w}"), "worker", Some("spin"), claim),
                            )
                        })
                    })
                    .collect();
                handles.into_iter().map(|h| h.join().unwrap()).collect()
            });

            attempts += results.len();
            let wins = results.iter().filter(|r| r.is_ok()).count();
            let refusals = results
                .iter()
                .filter(|r| matches!(r, Err(LedgerError::VersionConflict { .. })))
                .count();
            assert_eq!(wins, 1, "round {round}: exactly one writer may win, got {results:?}");
            assert_eq!(refusals, WRITERS - 1, "round {round}: every loser refused cleanly");
        }
        // The floor, on the population that matters: the battery must have
        // actually run every writer in every round.
        assert_eq!(attempts, WRITERS * ROUNDS);
        let final_version: usize = ledger.load(&record.id).unwrap().version.0.parse().unwrap();
        assert_eq!(final_version, 1 + ROUNDS, "one version step per round, none lost");
    }

    #[test]
    fn a_denial_has_nothing_to_persist() {
        let (_dir, ledger) = temp_ledger();
        let record = seed(&ledger, "open");
        let deny = Decision::Deny { reason: "not yours".to_string() };
        let refused = ledger.apply(&record, &event("a", "worker", Some("open"), deny.clone()));
        assert!(matches!(refused, Err(LedgerError::NothingToApply)));
        let refused = ledger.create(&Scope::all(), &deny, &event("a", "worker", None, deny.clone()));
        assert!(matches!(refused, Err(LedgerError::NothingToApply)));
        // And an exhausted filing creates no record either: the budget being
        // spent means the matter escalates, not that a record exists.
        let spent = Decision::Exhausted { to: "escalated".to_string(), counter: "filings".to_string() };
        let refused = ledger.create(&Scope::all(), &spent, &event("a", "reviewer", None, spent.clone()));
        assert!(matches!(refused, Err(LedgerError::NothingToApply)));
    }

    #[test]
    fn a_filing_spend_is_not_stored_on_the_record_it_filed() {
        let (_dir, ledger) = temp_ledger();
        let filing = Decision::allow("open", BTreeMap::from([("filings".to_string(), 1)]));
        let record = ledger
            .create(&Scope::all(), &filing, &event("r", "reviewer", None, filing.clone()))
            .unwrap();
        assert!(record.snapshot.counters.is_empty(), "scope-level spends stay off the record");
        assert!(ledger.load(&record.id).unwrap().snapshot.counters.is_empty());
        // The spend is still in the history, where an adapter that derives
        // scope counters goes to count it.
        let history = ledger.history(&record.id).unwrap();
        assert!(matches!(&history[0].event.decision, Decision::Allow { counter_updates, .. }
            if counter_updates.get("filings") == Some(&1)));
    }

    #[test]
    fn select_narrows_by_state_and_scope_and_an_empty_state_list_is_empty() {
        let (_dir, ledger) = temp_ledger();
        let a = ledger
            .create(&Scope::all().with("repo", "a"), &filed("open"), &event("o", "op", None, filed("open")))
            .unwrap();
        ledger
            .create(&Scope::all().with("repo", "b"), &filed("open"), &event("o", "op", None, filed("open")))
            .unwrap();
        ledger
            .create(&Scope::all().with("repo", "a"), &filed("done"), &event("o", "op", None, filed("done")))
            .unwrap();

        let both = ledger.select(&Scope::all(), &["open".to_string()]).unwrap();
        assert_eq!(both.len(), 2);
        let only_a = ledger
            .select(&Scope::all().with("repo", "a"), &["open".to_string()])
            .unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].id, a.id);
        let none = ledger.select(&Scope::all(), &[]).unwrap();
        assert!(none.is_empty(), "no states asked for, none returned");
    }

    #[test]
    fn history_is_append_only_because_the_storage_says_so() {
        let (dir, ledger) = temp_ledger();
        let record = seed(&ledger, "open");
        ledger
            .apply(&record, &event("a", "worker", Some("open"), filed("working")))
            .unwrap();

        // Not through the adapter: straight at the file, the way a buggy
        // script would arrive.
        let conn = Connection::open(dir.path().join("ledger.db")).unwrap();
        let rewrite = conn.execute("UPDATE ferrostep_events SET actor = 'nobody'", []);
        assert!(rewrite.unwrap_err().to_string().contains("append-only"));
        let erase = conn.execute("DELETE FROM ferrostep_events", []);
        assert!(erase.unwrap_err().to_string().contains("append-only"));
    }

    #[test]
    fn a_record_the_adapter_cannot_read_says_so_by_name() {
        let (dir, ledger) = temp_ledger();
        let record = seed(&ledger, "open");
        let conn = Connection::open(dir.path().join("ledger.db")).unwrap();
        conn.execute("UPDATE ferrostep_records SET counters = 'not json'", []).unwrap();
        let read = ledger.load(&record.id);
        assert!(matches!(read, Err(LedgerError::Malformed { .. })), "{read:?}");
    }

    #[test]
    fn unknown_and_foreign_ids_are_not_found() {
        let (_dir, ledger) = temp_ledger();
        assert!(matches!(
            ledger.load(&RecordId("999".to_string())),
            Err(LedgerError::NotFound(_))
        ));
        assert!(matches!(
            ledger.load(&RecordId("pb_a1b2c3".to_string())),
            Err(LedgerError::NotFound(_)),
        ));
        assert!(matches!(
            ledger.history(&RecordId("999".to_string())),
            Err(LedgerError::NotFound(_))
        ));
    }

    #[test]
    fn a_foreign_version_token_is_malformed_not_a_conflict() {
        let (_dir, ledger) = temp_ledger();
        let mut record = seed(&ledger, "open");
        record.version = Version("etag-xyz".to_string());
        let refused = ledger.apply(&record, &event("a", "worker", Some("open"), filed("working")));
        assert!(matches!(refused, Err(LedgerError::Malformed { .. })), "{refused:?}");
    }

    /// A rescope's whole point is that the queries which find work start
    /// finding the record somewhere else. Asserting the stored label would
    /// only prove a column changed; this asks the question the lane actually
    /// asks — *is it in this unit of work?* — from both sides.
    #[test]
    fn a_rescope_moves_the_record_between_units_of_work() {
        let (_dir, ledger) = temp_ledger();
        let record = seed(&ledger, "open");
        let moved = Decision::Allow {
            to: "open".to_string(),
            counter_updates: BTreeMap::new(),
            scope_updates: BTreeMap::from([("branch".to_string(), "follow-up".to_string())]),
        };
        let version = ledger
            .apply(&record, &event("lauren", "operator", Some("open"), moved))
            .unwrap();

        let here = |branch: &str| {
            ledger
                .select(&Scope::all().with("branch", branch), &["open".to_string()])
                .unwrap()
                .len()
        };
        assert_eq!(here("follow-up"), 1, "the record did not arrive in the new unit");
        assert_eq!(here("fix/gate"), 0, "the record is still in the unit it left");
        assert_ne!(version, record.version, "a rescope must consume a version");

        // The move is in the history like any other, so an audit shows why a
        // record left a unit rather than it silently vanishing from a queue.
        let history = ledger.history(&record.id).unwrap();
        let last = history.last().expect("the rescope was not recorded");
        assert!(
            matches!(&last.event.decision, Decision::Allow { scope_updates, .. }
                     if scope_updates.get("branch").map(String::as_str) == Some("follow-up")),
            "{:?}",
            last.event.decision
        );
    }

    /// ⚠ A rescope names labels; it does not replace a scope. A record that
    /// loses an unrelated label is a record that falls out of every other
    /// query filtering on it — silently, because nothing asked about that
    /// label.
    #[test]
    fn a_rescope_leaves_the_labels_nobody_named_alone() {
        let (_dir, ledger) = temp_ledger();
        let record = ledger
            .create(
                &Scope::all().with("branch", "fix/gate").with("repo", "acme/widgets"),
                &filed("open"),
                &event("lauren", "operator", None, filed("open")),
            )
            .unwrap();
        let moved = Decision::Allow {
            to: "open".to_string(),
            counter_updates: BTreeMap::new(),
            scope_updates: BTreeMap::from([("branch".to_string(), "follow-up".to_string())]),
        };
        ledger.apply(&record, &event("lauren", "operator", Some("open"), moved)).unwrap();

        let still_scoped = ledger
            .select(
                &Scope::all().with("branch", "follow-up").with("repo", "acme/widgets"),
                &["open".to_string()],
            )
            .unwrap();
        assert_eq!(still_scoped.len(), 1, "an untouched label was lost by the rescope");
    }
}

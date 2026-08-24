//! ferrostep-pocketbase — the ledger on a PocketBase instance.
//!
//! The shape is the one the measured record settled: **a stock instance, plus
//! a generated transactional route for applying decisions**. The REST API has
//! no conditional update, and the obvious workaround — compare in a rule or a
//! request hook, then let the ordinary write proceed — was measured failing
//! under two concurrent writers *and intermittently passing*, which is worse.
//! The only write path this adapter ships is the one that held: a custom
//! route whose compare runs **inside the store's own transaction**, beside
//! the record write and the event append.
//!
//! Consequently the adapter has two modes, detected at connect time, and it
//! **says which it is in** rather than degrading quietly:
//!
//! * **Full** — the generated hooks answer; reads and writes work and the
//!   capability flags hold as measured.
//! * **ReadOnly** — the hooks are not installed. Loads, enumeration and
//!   history work over plain REST; `apply` and `create` are refused with
//!   [`LedgerError::Unsupported`] naming the remedy. Refusing beats
//!   approximating: the write path that a REST-only adapter would need is
//!   the design the measurement rejected, so it is not shipped at all.
//!
//! Install is two generated files, written by [`install_files`]: a migration
//! that creates the collections (rules deliberately `null`, never `""` — an
//! empty-string rule means *public*, and the two read rules require an
//! authenticated actor) and the hook file with the routes. ⚠ Writing a hook
//! file makes a watching server restart itself; a health check fired
//! immediately after the write can answer before the restart begins.
//!
//! Error mapping is measured for conflicts and not-founds, and **inferred**
//! for field-validation failures — both generated routes compute the guarded
//! values server-side and discard the caller's, which is exactly why a
//! validation failure could not be provoked. A refusal message arrives
//! normalized (first letter capitalized, period appended), so mapping matches
//! case-insensitively and never on the tail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ferrostep_core::{Decision, Snapshot};
use ferrostep_ledger::{
    decided_snapshot, Capabilities, Event, Ledger, LedgerError, Record, RecordId, Scope,
    StoredEvent, Version,
};
use serde_json::{json, Value};

/// Collection names the generated migration creates and every query uses.
pub const RECORDS_COLLECTION: &str = "ferrostep_records";
pub const EVENTS_COLLECTION: &str = "ferrostep_events";

/// PocketBase's page-size cap. The *default* is 30 and silently truncates;
/// every enumeration here asks for the cap, never skips the total count, and
/// verifies it read as many as the store said existed.
const PER_PAGE: u32 = 500;

/// Which write path answered at connect time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The generated routes are installed: full ledger, flags as measured.
    Full,
    /// Plain REST only: reads work, writes are refused by name.
    ReadOnly,
}

/// A FerroStep ledger on a PocketBase instance.
pub struct PocketBaseLedger {
    base: String,
    token: String,
    agent: ureq::Agent,
    mode: Mode,
}

fn agent() -> ureq::Agent {
    // Non-2xx statuses are answers to read, not transport failures: the
    // refusal body is where a conflict names the held version.
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder().http_status_as_error(false).build(),
    )
}

/// Read a response into (status, body). An empty or non-JSON body becomes
/// `Null` — several refusals arrive with one, and the status still speaks.
fn read(mut resp: ureq::http::Response<ureq::Body>) -> (u16, Value) {
    let status = resp.status().as_u16();
    let body = resp.body_mut().read_json().unwrap_or(Value::Null);
    (status, body)
}

fn transport(e: ureq::Error) -> LedgerError {
    LedgerError::Transport(e.to_string())
}

/// The `message` of a PocketBase error body, lowercased for matching: the
/// store normalizes messages (capitalizes, appends a period), so matching is
/// case-insensitive and never depends on the tail.
fn refusal(body: &Value) -> String {
    body.get("message").and_then(Value::as_str).unwrap_or("").to_lowercase()
}

impl PocketBaseLedger {
    /// Connect to `base_url` with a PocketBase auth token (a role-scoped
    /// account's, ideally — an administrator's works but bypasses the
    /// collection rules). Probes the generated routes and records the mode.
    pub fn connect(base_url: &str, token: &str) -> Result<Self, LedgerError> {
        let base = base_url.trim_end_matches('/').to_string();
        let agent = agent();
        let resp = agent
            .get(format!("{base}/api/ferrostep/ping"))
            .call()
            .map_err(transport)?;
        let (status, body) = read(resp);
        let mode = if status == 200 && body.get("ferrostep").is_some() {
            Mode::Full
        } else {
            Mode::ReadOnly
        };
        Ok(PocketBaseLedger { base, token: token.to_string(), agent, mode })
    }

    /// Which write path answered at connect time. Callers that surface
    /// guarantees to a person should surface this beside them.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Obtain a token the way an actor does: password auth against an auth
    /// collection (`_superusers` for an administrator, a role-scoped
    /// collection for everyone else).
    pub fn auth_with_password(
        base_url: &str,
        collection: &str,
        identity: &str,
        password: &str,
    ) -> Result<String, LedgerError> {
        let base = base_url.trim_end_matches('/');
        let resp = agent()
            .post(format!("{base}/api/collections/{collection}/auth-with-password"))
            .send_json(json!({ "identity": identity, "password": password }))
            .map_err(transport)?;
        let (status, body) = read(resp);
        if status != 200 {
            return Err(LedgerError::Transport(format!(
                "auth against '{collection}' answered {status}: {}",
                refusal(&body)
            )));
        }
        body.get("token")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| LedgerError::Transport("auth answered without a token".to_string()))
    }

    fn record_from_value(&self, value: &Value) -> Result<Record, LedgerError> {
        let id = RecordId(
            value.get("id").and_then(Value::as_str).unwrap_or_default().to_string(),
        );
        let malformed = |detail: String| LedgerError::Malformed { id: id.clone(), detail };
        let state = value
            .get("state")
            .and_then(Value::as_str)
            .ok_or_else(|| malformed("no state field".to_string()))?
            .to_string();
        let version = value
            .get("version")
            .and_then(Value::as_i64)
            .ok_or_else(|| malformed("no integer version field".to_string()))?;
        let mut counters = BTreeMap::new();
        if let Some(map) = value.get("counters").and_then(Value::as_object) {
            for (name, v) in map {
                let n = v
                    .as_u64()
                    .ok_or_else(|| malformed(format!("counter '{name}' is not an integer")))?;
                counters.insert(name.clone(), n as u32);
            }
        }
        Ok(Record {
            id,
            snapshot: Snapshot { state, counters },
            version: Version(version.to_string()),
        })
    }

    /// One collection enumeration, paged to completion and checked against
    /// the total the store reported — the check, not the discipline.
    fn list_all(&self, collection: &str, filter: &str, sort: &str) -> Result<Vec<Value>, LedgerError> {
        let mut items: Vec<Value> = Vec::new();
        let mut page = 1u32;
        loop {
            let resp = self
                .agent
                .get(format!("{}/api/collections/{collection}/records", self.base))
                .header("Authorization", &self.token)
                .query("page", page.to_string())
                .query("perPage", PER_PAGE.to_string())
                // Explicitly not skipped: the total is what makes truncation
                // detectable, and at least one client in the wild defaults to
                // skipping it.
                .query("skipTotal", "0")
                .query("filter", filter)
                .query("sort", sort)
                .call()
                .map_err(transport)?;
            let (status, body) = read(resp);
            if status != 200 {
                return Err(LedgerError::Transport(format!(
                    "list of '{collection}' answered {status}: {}",
                    refusal(&body)
                )));
            }
            let total = body.get("totalItems").and_then(Value::as_u64).ok_or_else(|| {
                LedgerError::Transport("list answered without totalItems".to_string())
            })?;
            let batch = body
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if batch.is_empty() && (items.len() as u64) < total {
                return Err(LedgerError::Transport(format!(
                    "list of '{collection}' truncated: {} of {total} before an empty page",
                    items.len()
                )));
            }
            items.extend(batch);
            if items.len() as u64 >= total {
                if items.len() as u64 > total {
                    return Err(LedgerError::Transport(format!(
                        "list of '{collection}' overran its own total: {} of {total}",
                        items.len()
                    )));
                }
                return Ok(items);
            }
            page += 1;
        }
    }

    fn require_full(&self, what: &'static str) -> Result<(), LedgerError> {
        match self.mode {
            Mode::Full => Ok(()),
            // The write path a REST-only adapter would need is the design the
            // measurement rejected; refusing names the remedy instead.
            Mode::ReadOnly => Err(LedgerError::Unsupported(what)),
        }
    }
}

/// PocketBase filter-string escaping for a single-quoted literal.
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn event_payload(event: &Event) -> Value {
    json!({
        "actor": event.actor,
        "role": event.role,
        "from_state": event.from_state,
        "decision": event.decision,
        "note": event.note,
    })
}

impl Ledger for PocketBaseLedger {
    fn capabilities(&self) -> Capabilities {
        match self.mode {
            // Measured on this backend: the in-transaction compare held over
            // repeated rounds at up to sixteen writers, administrators
            // included. Append-only holds for role-scoped actors through the
            // shipped rules (write rules null), with the flag's own caveat: a
            // store administrator is out of scope by the flag's definition.
            Mode::Full => Capabilities {
                atomic_apply: true,
                compare_and_swap: true,
                append_only_history: true,
            },
            Mode::ReadOnly => Capabilities {
                atomic_apply: false,
                compare_and_swap: false,
                append_only_history: false,
            },
        }
    }

    fn load(&self, id: &RecordId) -> Result<Record, LedgerError> {
        let resp = self
            .agent
            .get(format!(
                "{}/api/collections/{RECORDS_COLLECTION}/records/{}",
                self.base, id.0
            ))
            .header("Authorization", &self.token)
            .call()
            .map_err(transport)?;
        let (status, body) = read(resp);
        match status {
            200 => self.record_from_value(&body),
            404 => Err(LedgerError::NotFound(id.clone())),
            _ => Err(LedgerError::Transport(format!(
                "load answered {status}: {}",
                refusal(&body)
            ))),
        }
    }

    fn create(
        &self,
        scope: &Scope,
        decision: &Decision,
        event: &Event,
    ) -> Result<Record, LedgerError> {
        self.require_full("file a record without the ferrostep hooks installed")?;
        let Decision::Allow { to, .. } = decision else {
            return Err(LedgerError::NothingToApply);
        };
        // As everywhere: a filing decision's counter updates are scope-level
        // and are not persisted onto the record being filed.
        let resp = self
            .agent
            .post(format!("{}/api/ferrostep/create", self.base))
            .header("Authorization", &self.token)
            .send_json(json!({
                "state": to,
                "scope": scope.filters(),
                "event": event_payload(event),
            }))
            .map_err(transport)?;
        let (status, body) = read(resp);
        if status != 200 {
            return Err(LedgerError::Transport(format!(
                "create answered {status}: {}",
                refusal(&body)
            )));
        }
        let id = body.get("id").and_then(Value::as_str).unwrap_or_default().to_string();
        Ok(Record {
            id: RecordId(id),
            snapshot: Snapshot { state: to.clone(), counters: BTreeMap::new() },
            version: Version("1".to_string()),
        })
    }

    fn apply(&self, record: &Record, event: &Event) -> Result<Version, LedgerError> {
        self.require_full("apply a decision without the ferrostep hooks installed")?;
        let Some(next) = decided_snapshot(&record.snapshot, &event.decision) else {
            return Err(LedgerError::NothingToApply);
        };
        let expected: i64 = record.version.0.parse().map_err(|_| LedgerError::Malformed {
            id: record.id.clone(),
            detail: format!("version token '{}' is not this adapter's shape", record.version.0),
        })?;
        let resp = self
            .agent
            .post(format!("{}/api/ferrostep/apply", self.base))
            .header("Authorization", &self.token)
            .send_json(json!({
                "record_id": record.id.0,
                "expected_version": expected,
                "state": next.state,
                "counters": next.counters,
                "event": event_payload(event),
            }))
            .map_err(transport)?;
        let (status, body) = read(resp);
        let message = refusal(&body);
        if status == 200 {
            let version = body.get("version").and_then(Value::as_i64).ok_or_else(|| {
                LedgerError::Transport("apply answered without a version".to_string())
            })?;
            return Ok(Version(version.to_string()));
        }
        // Measured mappings first; anything else is inferred and says so by
        // carrying the raw status and message.
        if message.contains("cas_conflict") {
            return Err(LedgerError::VersionConflict {
                id: record.id.clone(),
                expected: record.version.clone(),
            });
        }
        if status == 404 || message.contains("no_record") {
            return Err(LedgerError::NotFound(record.id.clone()));
        }
        Err(LedgerError::Transport(format!("apply answered {status}: {message}")))
    }

    fn select(&self, scope: &Scope, states: &[String]) -> Result<Vec<Record>, LedgerError> {
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let filter = states
            .iter()
            .map(|s| format!("state = {}", quoted(s)))
            .collect::<Vec<_>>()
            .join(" || ");
        let items = self.list_all(RECORDS_COLLECTION, &format!("({filter})"), "id")?;
        let mut out = Vec::new();
        for item in &items {
            // Scope narrowing happens here, in the adapter's own language,
            // exactly as in the SQLite adapter: a label key containing filter
            // syntax cannot be misread, at the cost of reading the state-wide
            // set — which the completeness check above already paid for.
            let mut labels = BTreeMap::new();
            if let Some(map) = item.get("scope").and_then(Value::as_object) {
                for (k, v) in map {
                    labels.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                }
            }
            if !scope.matches(&labels) {
                continue;
            }
            out.push(self.record_from_value(item)?);
        }
        Ok(out)
    }

    fn history(&self, id: &RecordId) -> Result<Vec<StoredEvent>, LedgerError> {
        // A record with no history and no record at all must answer apart.
        self.load(id)?;
        let items = self.list_all(
            EVENTS_COLLECTION,
            &format!("(record = {})", quoted(&id.0)),
            "seq",
        )?;
        let mut out = Vec::new();
        for item in &items {
            let malformed = |detail: String| LedgerError::Malformed { id: id.clone(), detail };
            let seq = item
                .get("seq")
                .and_then(Value::as_u64)
                .ok_or_else(|| malformed("event without an integer seq".to_string()))?;
            let decision: Decision =
                serde_json::from_value(item.get("decision").cloned().unwrap_or(Value::Null))
                    .map_err(|e| malformed(format!("event {seq} decision: {e}")))?;
            // PocketBase returns an absent text field as "", which is not a
            // state name or a note anybody wrote.
            let text = |key: &str| {
                item.get(key)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            out.push(StoredEvent {
                seq,
                at: item.get("at").and_then(Value::as_str).unwrap_or_default().to_string(),
                event: Event {
                    actor: item.get("actor").and_then(Value::as_str).unwrap_or_default().to_string(),
                    role: item.get("role").and_then(Value::as_str).unwrap_or_default().to_string(),
                    from_state: text("from_state"),
                    decision,
                    note: text("note"),
                },
            });
        }
        Ok(out)
    }
}

/// The generated hook file: the transactional apply/create routes and the
/// ping the adapter probes for. ⚠ Every handler is deliberately
/// self-contained — hook callbacks run in isolated runtimes where file-scope
/// helpers are not visible, so the duplication between the two write routes
/// is load-bearing, not tidiness waiting to happen.
pub fn hooks_file() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"// ferrostep.pb.js — generated by ferrostep-pocketbase v{version}.
// Do not hand-edit; regenerate and reinstall instead. Each handler is
// self-contained on purpose: hook callbacks run in isolated runtimes where
// file-scope helpers are not visible, so shared logic here would fail on
// every call while reading perfectly.

routerAdd("GET", "/api/ferrostep/ping", (e) => {{
    return e.json(200, {{ "ferrostep": "{version}" }});
}});

routerAdd("POST", "/api/ferrostep/apply", (e) => {{
    const body = e.requestInfo().body;
    const recordId = String(body.record_id || "");
    const expected = Number(body.expected_version);
    let version = 0;
    $app.runInTransaction((txApp) => {{
        let rec;
        try {{
            rec = txApp.findRecordById("ferrostep_records", recordId);
        }} catch (err) {{
            throw new NotFoundError("no_record: " + recordId);
        }}
        const held = rec.getInt("version");
        if (held !== expected) {{
            // The compare lives INSIDE the transaction. Measured as the only
            // placement that survives concurrent writers; the same check
            // outside it intermittently passes while losing updates.
            throw new BadRequestError("cas_conflict: expected " + expected + ", found " + held);
        }}
        rec.set("state", String(body.state));
        rec.set("counters", body.counters || {{}});
        rec.set("version", held + 1);
        txApp.save(rec);
        let seq = 1;
        const last = txApp.findRecordsByFilter("ferrostep_events", "record = {{:id}}", "-seq", 1, 0, {{ "id": recordId }});
        if (last.length > 0) {{
            seq = last[0].getInt("seq") + 1;
        }}
        const ev = new Record(txApp.findCollectionByNameOrId("ferrostep_events"));
        ev.set("record", recordId);
        ev.set("seq", seq);
        ev.set("actor", String((body.event && body.event.actor) || ""));
        ev.set("role", String((body.event && body.event.role) || ""));
        if (body.event && body.event.from_state) {{
            ev.set("from_state", String(body.event.from_state));
        }}
        ev.set("decision", (body.event && body.event.decision) || {{}});
        if (body.event && body.event.note) {{
            ev.set("note", String(body.event.note));
        }}
        txApp.save(ev);
        version = held + 1;
    }});
    return e.json(200, {{ "version": version }});
}}, $apis.requireAuth());

routerAdd("POST", "/api/ferrostep/create", (e) => {{
    const body = e.requestInfo().body;
    let out = {{}};
    $app.runInTransaction((txApp) => {{
        const rec = new Record(txApp.findCollectionByNameOrId("ferrostep_records"));
        rec.set("state", String(body.state));
        rec.set("counters", {{}});
        rec.set("scope", body.scope || {{}});
        rec.set("version", 1);
        txApp.save(rec);
        const ev = new Record(txApp.findCollectionByNameOrId("ferrostep_events"));
        ev.set("record", rec.id);
        ev.set("seq", 1);
        ev.set("actor", String((body.event && body.event.actor) || ""));
        ev.set("role", String((body.event && body.event.role) || ""));
        ev.set("decision", (body.event && body.event.decision) || {{}});
        if (body.event && body.event.note) {{
            ev.set("note", String(body.event.note));
        }}
        txApp.save(ev);
        out = {{ "id": rec.id, "version": 1 }};
    }});
    return e.json(200, out);
}}, $apis.requireAuth());
"#
    )
}

/// The generated migration: both collections, the unique `(record, seq)`
/// index that referees concurrent appends, and rules that are `null` (writes:
/// nobody over REST) or auth-gated (reads) — never `""`, which would mean
/// *public*.
pub fn migration_file() -> String {
    let version = env!("CARGO_PKG_VERSION");
    format!(
        r#"// ferrostep collections — generated by ferrostep-pocketbase v{version}.
// Do not hand-edit; regenerate and reinstall instead.
migrate((app) => {{
    const records = new Collection({{
        "name": "ferrostep_records",
        "type": "base",
        "fields": [
            {{ "name": "state", "type": "text", "required": true }},
            {{ "name": "counters", "type": "json" }},
            {{ "name": "scope", "type": "json" }},
            {{ "name": "version", "type": "number", "required": true }},
            {{ "name": "at", "type": "autodate", "onCreate": true, "onUpdate": false }}
        ],
        "listRule": "@request.auth.id != ''",
        "viewRule": "@request.auth.id != ''",
        "createRule": null,
        "updateRule": null,
        "deleteRule": null
    }});
    app.save(records);
    const events = new Collection({{
        "name": "ferrostep_events",
        "type": "base",
        "fields": [
            {{ "name": "record", "type": "text", "required": true }},
            {{ "name": "seq", "type": "number", "required": true }},
            {{ "name": "actor", "type": "text", "required": true }},
            {{ "name": "role", "type": "text", "required": true }},
            {{ "name": "from_state", "type": "text" }},
            {{ "name": "decision", "type": "json", "required": true }},
            {{ "name": "note", "type": "text", "max": 50000 }},
            {{ "name": "at", "type": "autodate", "onCreate": true, "onUpdate": false }}
        ],
        "indexes": [
            "CREATE UNIQUE INDEX idx_ferrostep_events_record_seq ON ferrostep_events (record, seq)"
        ],
        "listRule": "@request.auth.id != ''",
        "viewRule": "@request.auth.id != ''",
        "createRule": null,
        "updateRule": null,
        "deleteRule": null
    }});
    app.save(events);
}}, (app) => {{
    for (const name of ["ferrostep_events", "ferrostep_records"]) {{
        try {{
            app.delete(app.findCollectionByNameOrId(name));
        }} catch (err) {{}}
    }}
}});
"#
    )
}

/// Write both generated files under a PocketBase working directory:
/// `pb_migrations/…_ferrostep.js` and `pb_hooks/ferrostep.pb.js`. Returns
/// the two paths, migration first.
///
/// ⚠ A server watching its hooks directory restarts itself when the hook
/// file lands; a health check fired immediately after this returns can
/// answer before that restart begins, which is not evidence of anything.
pub fn install_files(pb_dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
    let migrations = pb_dir.join("pb_migrations");
    let hooks = pb_dir.join("pb_hooks");
    std::fs::create_dir_all(&migrations)?;
    std::fs::create_dir_all(&hooks)?;
    // The numeric prefix is PocketBase's ordering convention; fixed, because
    // regenerating must overwrite rather than accumulate.
    let migration_path = migrations.join("1756000000_ferrostep.js");
    let hooks_path = hooks.join("ferrostep.pb.js");
    std::fs::write(&migration_path, migration_file())?;
    std::fs::write(&hooks_path, hooks_file())?;
    Ok((migration_path, hooks_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    /// A one-thread HTTP server answering from a routing table, closing each
    /// connection so the client cannot pipeline past it. Stops after serving
    /// `hits` requests.
    fn serve(routes: Vec<(&'static str, u16, String)>, hits: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for _ in 0..hits {
                let Ok((stream, _)) = listener.accept() else { return };
                let mut reader = BufReader::new(stream);
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let path = request_line.split_whitespace().nth(1).unwrap_or("").to_string();
                // Drain headers (and any body — none of these tests needs it).
                let mut content_length = 0usize;
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        content_length = v.trim().parse().unwrap_or(0);
                    }
                }
                if content_length > 0 {
                    let mut body = vec![0u8; content_length];
                    let _ = reader.read_exact(&mut body);
                }
                let (status, body) = routes
                    .iter()
                    .find(|(prefix, _, _)| path.starts_with(prefix))
                    .map(|(_, s, b)| (*s, b.clone()))
                    .unwrap_or((404, r#"{"message":"Missing route."}"#.to_string()));
                let mut stream = reader.into_inner();
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        base
    }

    fn ping_full() -> (&'static str, u16, String) {
        ("/api/ferrostep/ping", 200, r#"{"ferrostep":"test"}"#.to_string())
    }

    fn an_event(decision: Decision) -> Event {
        Event {
            actor: "a".to_string(),
            role: "worker".to_string(),
            from_state: Some("open".to_string()),
            decision,
            note: None,
        }
    }

    fn a_record(version: &str) -> Record {
        Record {
            id: RecordId("abc123".to_string()),
            snapshot: Snapshot { state: "open".to_string(), counters: BTreeMap::new() },
            version: Version(version.to_string()),
        }
    }

    fn allow(to: &str) -> Decision {
        Decision::Allow { to: to.to_string(), counter_updates: BTreeMap::new() }
    }

    #[test]
    fn without_the_hooks_the_adapter_is_read_only_and_says_so() {
        // Ping 404s: a stock instance with nothing installed.
        let base = serve(vec![], 1);
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        assert_eq!(ledger.mode(), Mode::ReadOnly);
        let caps = ledger.capabilities();
        assert!(!caps.atomic_apply && !caps.compare_and_swap && !caps.append_only_history);
        let refused = ledger.apply(&a_record("1"), &an_event(allow("working")));
        assert!(matches!(refused, Err(LedgerError::Unsupported(_))), "{refused:?}");
        let refused = ledger.create(&Scope::all(), &allow("open"), &an_event(allow("open")));
        assert!(matches!(refused, Err(LedgerError::Unsupported(_))), "{refused:?}");
    }

    #[test]
    fn with_the_hooks_the_adapter_reports_full_and_its_measured_flags() {
        let base = serve(vec![ping_full()], 1);
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        assert_eq!(ledger.mode(), Mode::Full);
        let caps = ledger.capabilities();
        assert!(caps.atomic_apply && caps.compare_and_swap && caps.append_only_history);
    }

    #[test]
    fn a_normalized_refusal_still_reads_as_a_conflict() {
        // The store capitalizes the first letter and appends a period; the
        // mapping must match anyway, and must not depend on the tail.
        let base = serve(
            vec![
                ping_full(),
                (
                    "/api/ferrostep/apply",
                    400,
                    r#"{"message":"Cas_conflict: expected 3, found 5."}"#.to_string(),
                ),
            ],
            2,
        );
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let refused = ledger.apply(&a_record("3"), &an_event(allow("working")));
        assert!(matches!(refused, Err(LedgerError::VersionConflict { .. })), "{refused:?}");
    }

    #[test]
    fn a_missing_record_maps_to_not_found_from_either_signal() {
        let base = serve(
            vec![
                ping_full(),
                ("/api/ferrostep/apply", 404, r#"{"message":"No_record: abc123."}"#.to_string()),
                ("/api/collections/ferrostep_records/records/", 404, r#"{"message":"Missing."}"#.to_string()),
            ],
            3,
        );
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let refused = ledger.apply(&a_record("1"), &an_event(allow("working")));
        assert!(matches!(refused, Err(LedgerError::NotFound(_))), "{refused:?}");
        let missing = ledger.load(&RecordId("abc123".to_string()));
        assert!(matches!(missing, Err(LedgerError::NotFound(_))), "{missing:?}");
    }

    #[test]
    fn a_denial_is_refused_before_any_request_leaves() {
        // No apply route in the table: reaching the wire would 404 as a
        // Transport error, so passing proves the refusal is local.
        let base = serve(vec![ping_full()], 1);
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let deny = Decision::Deny { reason: "no".to_string() };
        let refused = ledger.apply(&a_record("1"), &an_event(deny));
        assert!(matches!(refused, Err(LedgerError::NothingToApply)), "{refused:?}");
    }

    #[test]
    fn enumeration_that_cannot_account_for_the_total_is_an_error_not_a_short_list() {
        // The store claims three items and produces two, then an empty page:
        // the adapter must refuse to present that as a complete answer.
        let page = r#"{"page":1,"perPage":500,"totalItems":3,"totalPages":1,"items":[
            {"id":"r1","state":"open","counters":{},"scope":{},"version":1},
            {"id":"r2","state":"open","counters":{},"scope":{},"version":1}]}"#;
        let empty = r#"{"page":2,"perPage":500,"totalItems":3,"totalPages":1,"items":[]}"#;
        let base = serve(
            vec![
                ping_full(),
                ("/api/collections/ferrostep_records/records?page=1", 200, page.to_string()),
                ("/api/collections/ferrostep_records/records?page=2", 200, empty.to_string()),
            ],
            3,
        );
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let result = ledger.select(&Scope::all(), &["open".to_string()]);
        match result {
            Err(LedgerError::Transport(detail)) => {
                assert!(detail.contains("truncated"), "{detail}")
            }
            other => panic!("a shortfall must be an error, got {other:?}"),
        }
    }

    #[test]
    fn select_narrows_scope_on_the_client_side_of_the_wire() {
        let page = r#"{"page":1,"perPage":500,"totalItems":2,"totalPages":1,"items":[
            {"id":"r1","state":"open","counters":{"passes":2},"scope":{"repo":"a"},"version":4},
            {"id":"r2","state":"open","counters":{},"scope":{"repo":"b"},"version":1}]}"#;
        let base = serve(
            vec![
                ping_full(),
                ("/api/collections/ferrostep_records/records", 200, page.to_string()),
            ],
            2,
        );
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let only_a = ledger
            .select(&Scope::all().with("repo", "a"), &["open".to_string()])
            .unwrap();
        assert_eq!(only_a.len(), 1);
        assert_eq!(only_a[0].id.0, "r1");
        assert_eq!(only_a[0].snapshot.counters["passes"], 2);
        assert_eq!(only_a[0].version.0, "4");
    }

    #[test]
    fn history_reads_empty_text_fields_as_absent() {
        let record = r#"{"id":"r1","state":"open","counters":{},"scope":{},"version":2}"#;
        let events = r#"{"page":1,"perPage":500,"totalItems":1,"totalPages":1,"items":[
            {"id":"e1","record":"r1","seq":1,"at":"2026-08-24 10:00:00.000Z","actor":"a","role":"worker",
             "from_state":"","decision":{"kind":"allow","to":"open","counter_updates":{}},"note":""}]}"#;
        let base = serve(
            vec![
                ping_full(),
                ("/api/collections/ferrostep_records/records/r1", 200, record.to_string()),
                ("/api/collections/ferrostep_events/records", 200, events.to_string()),
            ],
            3,
        );
        let ledger = PocketBaseLedger::connect(&base, "tok").unwrap();
        let history = ledger.history(&RecordId("r1".to_string())).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].event.from_state, None, "an empty string is not a state name");
        assert_eq!(history[0].event.note, None);
        assert!(matches!(history[0].event.decision, Decision::Allow { .. }));
    }

    #[test]
    fn the_generated_files_carry_their_load_bearing_shapes() {
        let hooks = hooks_file();
        // Both write routes are transactional, authenticated, and the ping
        // answers what connect() probes for.
        assert_eq!(hooks.matches("runInTransaction").count(), 2);
        assert_eq!(hooks.matches("$apis.requireAuth()").count(), 2);
        assert!(hooks.contains(r#"routerAdd("GET", "/api/ferrostep/ping""#));
        assert!(hooks.contains("cas_conflict"));
        let migration = migration_file();
        assert!(migration.contains("CREATE UNIQUE INDEX"), "the (record, seq) referee");
        // The empty-string trap: "" means PUBLIC. Every rule is either null
        // or a real expression.
        assert!(!migration.contains(r#"Rule": """#), "an empty-string rule is public");
        assert!(migration.contains(r#""createRule": null"#));
    }

    #[test]
    fn install_writes_both_files_where_pocketbase_looks() {
        let dir = std::env::temp_dir().join(format!("ferrostep-pb-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (migration, hooks) = install_files(&dir).unwrap();
        assert!(migration.ends_with("pb_migrations/1756000000_ferrostep.js"));
        assert!(hooks.ends_with("pb_hooks/ferrostep.pb.js"));
        assert_eq!(std::fs::read_to_string(&hooks).unwrap(), hooks_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------
    // Live tests: a real PocketBase with the generated files installed.
    // Ignored by default so an offline run reports them as ignored rather
    // than silently green; run with:
    //   FERROSTEP_POCKETBASE_URL=… FERROSTEP_POCKETBASE_TOKEN=… \
    //     cargo test -p ferrostep-pocketbase -- --ignored
    // ------------------------------------------------------------------

    fn live() -> PocketBaseLedger {
        let url = std::env::var("FERROSTEP_POCKETBASE_URL")
            .expect("live test requires FERROSTEP_POCKETBASE_URL");
        let token = std::env::var("FERROSTEP_POCKETBASE_TOKEN")
            .expect("live test requires FERROSTEP_POCKETBASE_TOKEN");
        let ledger = PocketBaseLedger::connect(&url, &token).unwrap();
        assert_eq!(ledger.mode(), Mode::Full, "live tests need the hooks installed");
        ledger
    }

    #[test]
    #[ignore = "needs a live PocketBase with the ferrostep files installed; set FERROSTEP_POCKETBASE_URL and FERROSTEP_POCKETBASE_TOKEN and run with --ignored"]
    fn live_reference_loop_runs_end_to_end() {
        use ferrostep_core::{Attempt, Engine, Status, WorkflowDef};
        let def = WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        let engine = Engine::new(def).unwrap();
        let ledger = live();
        let record = ledger
            .create(
                &Scope::all().with("branch", "live-e2e"),
                &allow(&engine.def().initial),
                &Event {
                    actor: "lauren".to_string(),
                    role: "operator".to_string(),
                    from_state: None,
                    decision: allow(&engine.def().initial),
                    note: Some("live end-to-end".to_string()),
                },
            )
            .unwrap();

        let step = |role: &str, to: &str, note: Option<&str>| {
            let current = ledger.load(&record.id).unwrap();
            let mut attempt = Attempt::new(role, to);
            if let Some(n) = note {
                attempt = attempt.saying(n);
            }
            let decision = engine.authorize(&current.snapshot, &attempt);
            let event = Event {
                actor: role.to_string(),
                role: role.to_string(),
                from_state: Some(current.snapshot.state.clone()),
                decision: decision.clone(),
                note: note.map(str::to_string),
            };
            ledger.apply(&current, &event).map(|v| (v, decision))
        };

        for pass in 1..=3u32 {
            step("worker", "working", None).unwrap();
            assert_eq!(
                ledger.load(&record.id).unwrap().snapshot.counters["agent_passes"],
                pass
            );
            step("worker", "awaiting_review", None).unwrap();
            step("reviewer", "awaiting_worker", None).unwrap();
        }
        let (_, d) = step("worker", "working", None).unwrap();
        assert!(matches!(d, Decision::Exhausted { .. }), "{d:?}");
        let escalated = ledger.load(&record.id).unwrap();
        assert_eq!(escalated.snapshot.state, "escalated");
        assert_eq!(engine.status(&escalated.snapshot), Status::NeedsPerson);
        step("operator", "awaiting_worker", Some("released for one more pass")).unwrap();
        assert_eq!(ledger.load(&record.id).unwrap().snapshot.counters["agent_passes"], 0);
        step("worker", "working", None).unwrap();
        step("worker", "awaiting_review", None).unwrap();
        step("reviewer", "approved", None).unwrap();
        let done = ledger.load(&record.id).unwrap();
        assert_eq!(engine.status(&done.snapshot), Status::Ended);

        let history = ledger.history(&record.id).unwrap();
        for (i, e) in history.iter().enumerate() {
            assert_eq!(e.seq, i as u64 + 1, "seq is contiguous from 1");
        }
        assert_eq!(done.version.0, history.len().to_string(), "one version step per write");
    }

    #[test]
    #[ignore = "needs a live PocketBase with the ferrostep files installed; set FERROSTEP_POCKETBASE_URL and FERROSTEP_POCKETBASE_TOKEN and run with --ignored"]
    fn live_compare_and_swap_holds_under_concurrent_writers_over_repeated_rounds() {
        use std::sync::{Arc, Barrier};
        const WRITERS: usize = 6;
        const ROUNDS: usize = 15;
        let url = std::env::var("FERROSTEP_POCKETBASE_URL").unwrap();
        let token = std::env::var("FERROSTEP_POCKETBASE_TOKEN").unwrap();
        let ledger = live();
        let record = ledger
            .create(
                &Scope::all().with("branch", "live-battery"),
                &allow("spin"),
                &an_event(allow("spin")),
            )
            .unwrap();

        let mut attempts = 0usize;
        for round in 0..ROUNDS {
            let current = Arc::new(ledger.load(&record.id).unwrap());
            let barrier = Arc::new(Barrier::new(WRITERS));
            let results: Vec<Result<Version, LedgerError>> = std::thread::scope(|s| {
                let handles: Vec<_> = (0..WRITERS)
                    .map(|w| {
                        let (url, token) = (url.clone(), token.clone());
                        let current = Arc::clone(&current);
                        let barrier = Arc::clone(&barrier);
                        s.spawn(move || {
                            let own = PocketBaseLedger::connect(&url, &token).unwrap();
                            let claim = Decision::Allow {
                                to: "spin".to_string(),
                                counter_updates: BTreeMap::from([(
                                    "wins".to_string(),
                                    round as u32 + 1,
                                )]),
                            };
                            barrier.wait();
                            own.apply(&current, &an_event(claim))
                                .map(|v| {
                                    let _ = w;
                                    v
                                })
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
            assert_eq!(wins, 1, "round {round}: exactly one winner, got {results:?}");
            assert_eq!(refusals, WRITERS - 1, "round {round}: every loser refused cleanly");
        }
        assert_eq!(attempts, WRITERS * ROUNDS, "the battery ran its whole population");
        let final_version: usize = ledger.load(&record.id).unwrap().version.0.parse().unwrap();
        assert_eq!(final_version, 1 + ROUNDS, "one version step per round, none lost");
    }
}

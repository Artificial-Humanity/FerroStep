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
//! Two deployment shapes, one wire contract:
//!
//! * **Generic** ([`PocketBaseLedger::connect`]) — the adapter's own
//!   collections, created by the generated migration: a records collection
//!   with `counters`/`scope` as JSON, and an events collection.
//! * **Mapped** ([`PocketBaseLedger::connect_mapped`]) — an existing
//!   collection the deployment already lives in becomes the refereed record:
//!   a [`CollectionMap`] names which columns hold the state, the version
//!   token, the counters and the scope labels. The store's console stays the
//!   human view of the same rows — one record, one truth, no second
//!   chronology beside the first. Filing stays with the collection's own
//!   procedure, so `create` is refused by name in this shape.
//!
//! Generated routes are **collection-scoped**
//! (`/api/ferrostep/<records>/…`), so one instance can carry more than one
//! refereed collection without the routes colliding.
//!
//! The adapter has two modes, detected at connect time, and it **says which
//! it is in** rather than degrading quietly:
//!
//! * **Full** — the generated routes answer; reads and writes work and the
//!   capability flags hold as measured.
//! * **ReadOnly** — the routes are not installed. Loads, enumeration and
//!   history work over plain REST; `apply` and `create` are refused with
//!   [`LedgerError::Unsupported`] naming the remedy. Refusing beats
//!   approximating: the write path a REST-only adapter would need is the
//!   design the measurement rejected, so it is not shipped at all.
//!
//! Error mapping is measured for conflicts and not-founds, and **inferred**
//! for field-validation failures — the generated routes compute the guarded
//! values server-side and discard the caller's, which is exactly why a
//! validation failure could not be provoked. A refusal message arrives
//! normalized (first letter capitalized, period appended), so mapping matches
//! case-insensitively and never on the tail.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ferrostep_core::{Decision, Snapshot};
use ferrostep_ledger::{
    decided_scope_updates, decided_snapshot, Capabilities, Event, Ledger, LedgerError, Record,
    Answer, RecordId, Scope, StoreShape, StoredEvent, Version,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Collection names the generated migration creates for the generic shape.
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

/// How an existing collection maps onto the ledger's record shape.
///
/// Column names double as ledger names: a counter column called
/// `agent_passes` is the counter `agent_passes` in every snapshot, and a
/// scope column called `branch_name` answers `Scope::with("branch_name", …)`.
/// The mapping is deployment configuration — it travels as a JSON file beside
/// the workflow definition, never as code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionMap {
    /// The collection whose rows are the refereed records.
    pub records: String,
    /// The event collection beside it (the generic event shape).
    pub events: String,
    /// The column holding the record's state.
    pub state_field: String,
    /// The integer column holding the compare-and-swap token. Its default of
    /// `0` on rows that predate the mapping is a valid starting token — no
    /// backfill is required.
    pub version_field: String,
    /// Integer columns that are counters, each under its own name.
    pub counter_fields: Vec<String>,
    /// Text columns that are scope labels, each under its own name.
    pub scope_fields: Vec<String>,
    /// ⚠⚠ **A STOPGAP, AND LABELLED AS ONE (owner, 2026-08-27).** Columns that
    /// are refereed but whose *meaning* this engine has no opinion about.
    ///
    /// It exists because a lane can gate on a column that is none of state,
    /// version, counter or scope — the first adopter's merge gate reads a
    /// **severity** grade, and below a floor a finding rides while at or above
    /// it the branch is blocked. That column was outside the referee entirely,
    /// so the only thing standing between a developer and clearing their own
    /// gate was a **self-declared author flag** in the adopter's own script,
    /// which its docstring correctly called *a convention, not a mechanism*.
    ///
    /// ⚠⚠ **WHAT THIS BUYS IS AUTHENTICATION AND AUDIT, NOT AUTHORISATION.**
    /// Listing a column here closes it to direct writes and routes it through
    /// apply, so the writer is the holder of a token rather than whoever typed
    /// a name, and the write lands as an event. **It does not say who may set
    /// which value, or in which direction** — and for a graded column that is
    /// the half that matters, because raising a grade cannot clear a gate and
    /// lowering it can.
    ///
    /// ⚠ **The successor is a definition-level concept, and it SUBSUMES this
    /// rather than competing with it.** An ordered ladder with directional
    /// grants (*raising is anyone's, lowering is the reviewer's, with a note*)
    /// belongs in the workflow definition, where rules live; this list belongs
    /// in the map, where column names live — the same split that already puts
    /// `counter_fields` here and each counter's `max` in the definition. So a
    /// graded attribute will still need its column named here. Nothing built
    /// on this list has to be unbuilt.
    ///
    /// ⚠ `#[serde(default)]` because deployment maps written before this field
    /// existed must keep loading. Generated files outlive the binary; so do
    /// their configs.
    #[serde(default)]
    pub attribute_fields: Vec<String>,
    /// Whether a direct write to a refereed column is refused.
    ///
    /// The engine is consulted, not in the write path — so by default a
    /// client holding credentials can edit `state` or a counter straight on
    /// the row and the referee never hears about it. Turning this on refuses
    /// that at the request layer, leaving the apply route as the only way
    /// those columns move.
    ///
    /// ⚠ **It constrains administrators, which an access rule cannot** —
    /// measured on this backend. That is the whole reason it is a hook.
    ///
    /// ⚠ **Off by default, because on is a behaviour change for a running
    /// deployment**, exactly like [`ActorBinding::allow_unbound`]. Turn it on
    /// deliberately, and know what it costs: a console hand-edit of a counter
    /// stops working too. The operator's supported path becomes the release
    /// hook and the routes — which is the point, and is not free.
    ///
    /// ⚠⚠ **Audit for dormant generic writers before turning it on, not just
    /// live ones.** Reported by the first adopter's resident while checking
    /// their own lane: the code most likely to trip this guard first was a
    /// `patch(id, body)` helper that was *defined and never called* — a
    /// "PATCH any field onto a record" method with zero call sites. Reviewing
    /// the existing call sites finds nothing, because there are none. What
    /// happens instead is that the next person needing a PATCH reaches for
    /// the obvious helper, and the guard's first violation arrives as a
    /// runtime refusal in new code rather than as a review comment on old
    /// code. **Grep for methods that can write any field, not only for
    /// writes that happen today.**
    ///
    /// ⚠⚠ **Audit the PROMPTS as well as the code: an agent handed an
    /// instruction naming a write tool is an adapter nobody wrote.** Measured
    /// by the first adopter on the pass after the one above. Their audit
    /// enumerated the lane's four scripted call sites and turned the guard on;
    /// the first refusal came from none of them. It came from a persona file
    /// telling a reviewing agent, in prose, to move `state` with a generic
    /// record-mutation tool — a write path with no call site, no import, and
    /// **no authentication step to grep for**, because the tool server had
    /// already authenticated. Enumerating the code found four of five writers
    /// and reported completeness.
    ///
    /// ⚠⚠ **And the refusal can be misread into a much larger loss than one
    /// write.** That persona also told its agent — correctly — that an
    /// unreachable tracker means findings must be abandoned to a summary. An
    /// agent meeting this guard reports *what it concluded*, not what it read,
    /// so a stale instruction can turn one refused field into a whole review
    /// discarded. **Before turning the guard on, reread every persona that
    /// names a write tool** — the ones that describe a fallback for
    /// "the store is refusing me" are the expensive ones.
    #[serde(default)]
    pub guard_refereed_fields: bool,
}

impl CollectionMap {
    /// Every column the referee owns: the ones the guard closes, and the ones
    /// an adopter must go hunting for before closing them.
    ///
    /// ⚠ **One derivation, deliberately, because the guard and the hunting
    /// list disagreeing is the whole failure they exist to prevent.** A second
    /// copy would go stale the first time a counter is added to a map — and it
    /// would go stale in the direction that reports a clean sweep, which is
    /// the direction nothing goes red in.
    pub fn refereed_fields(&self) -> Vec<String> {
        std::iter::once(&self.state_field)
            .chain(std::iter::once(&self.version_field))
            .chain(self.counter_fields.iter())
            .chain(self.scope_fields.iter())
            .chain(self.attribute_fields.iter())
            .cloned()
            .collect()
    }

    /// The route those columns move through once the guard is on — the one
    /// place a refused writer has to be pointed at.
    pub fn apply_route(&self) -> String {
        format!("/api/ferrostep/{}/apply", self.records)
    }
}

/// A store-side release: writing a decision field *is* taking a transition,
/// so the console's one-save flow survives the cutover with the referee's
/// bookkeeping attached. Generated into the hooks file from the definition's
/// own release transition — maintained *with* the definition, never beside
/// it, which is what keeps this one referee rather than two.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseHook {
    /// The field whose (changed, non-empty) write fires the release.
    pub decision_field: String,
    /// The paused state the release leaves.
    pub from_state: String,
    /// Where the release sends the record.
    pub to_state: String,
    /// Counters returned to zero by the release — and by a re-arm, a changed
    /// decision written while the record already sits in `to_state`.
    pub reset_counters: Vec<String>,
    /// Who may write the decision field. An allowlist, fail-closed: an
    /// account added to the store later is refused here until somebody
    /// deliberately adds it.
    pub writers: Vec<String>,
    /// The role the release event records — the definition's human role.
    pub role: String,
}

/// How the store recognises an actor, and the role it may act in.
///
/// **Bind, don't mint.** This names an auth collection the deployment already
/// has; it does not create identities and is not an account store. The store
/// authenticates whoever it authenticates — a password, an OAuth provider, a
/// directory federated in behind it — and the only thing read here is *which
/// role that principal may act in*. Authentication stays somebody else's job.
///
/// ⚠ **The reason is that the actors are not knowable when a loop is
/// designed** (`docs/prior-art.md`, requirement 9). Owning an account store
/// would mean enumerating them up front, which is the assumption that fails
/// first: an agent nobody foresaw should be a new principal in a directory
/// that already exists plus one row naming its role — no release here.
///
/// The defaults work on a stock instance with nothing configured. Point
/// `collection` at an auth collection you already run to use that instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActorBinding {
    /// The auth collection whose records are actors.
    pub collection: String,
    /// The field on an actor's record naming the role it may act in.
    pub role_field: String,
    /// Whether a principal carrying no role may still act — a store
    /// administrator, or an account from some other collection.
    ///
    /// ⚠ **True by default, and that is a transition rather than a
    /// position.** A deployment that has not created actors yet authenticates
    /// as an administrator, and a default of `false` would refuse every write
    /// the moment these hooks were installed. Set it `false` once your actors
    /// exist: from then on a principal with no role cannot move a record even
    /// holding administrator credentials, which is the whole point of putting
    /// the check in a hook rather than in an access rule.
    pub allow_unbound: bool,
}

impl Default for ActorBinding {
    fn default() -> Self {
        ActorBinding {
            collection: "ferrostep_actors".to_string(),
            role_field: "role".to_string(),
            allow_unbound: true,
        }
    }
}

/// The generated check that binds a request's claimed role to the
/// authenticated principal.
///
/// ⚠ **Emitted from one place into every write route.** Three hand-written
/// copies of an authorization check is three chances for one to drift, and
/// the one that drifts is not the one anybody tests.
fn role_binding_js(actors: &ActorBinding) -> String {
    // Bound locally so the generated text and the adapter's matcher are
    // ONE derivation — see `CAS_CONFLICT`.
    let role_not_yours = ROLE_NOT_YOURS;
    let ActorBinding { role_field, allow_unbound, .. } = actors;
    let unbound = if *allow_unbound {
        r#"        // This deployment still permits an unbound principal (see
        // ActorBinding::allow_unbound). The claimed role stands, and the
        // event records it as claimed."#
            .to_string()
    } else {
        r#"        throw new BadRequestError(
            "unbound_principal: this deployment requires an actor account carrying a role; " +
            "the authenticated principal has none"
        );"#
        .to_string()
    };
    format!(
        r#"    // ⚠ The acting role comes from the ACCOUNT, never from the request.
    // A route that authenticates and then believes `body.event.role` lets any
    // authenticated caller act as any role — which is invisible while every
    // actor shares one credential, and is the entire point once they do not.
    // Structural rather than remembered, like the scope allowlist.
    const claimedRole = String((body.event && body.event.role) || "");
    const boundRole = e.auth ? String(e.auth.getString("{role_field}") || "") : "";
    let actingRole = claimedRole;
    if (boundRole) {{
        if (claimedRole && claimedRole !== boundRole) {{
            throw new BadRequestError(
                "{role_not_yours}: this account acts as '" + boundRole +
                "', the request claimed '" + claimedRole + "'"
            );
        }}
        actingRole = boundRole;
    }} else {{
{unbound}
    }}
"#
    )
}

#[derive(Debug, Clone)]
enum Shape {
    Generic,
    Mapped(CollectionMap),
}

/// A FerroStep ledger on a PocketBase instance.
/// ⚠⚠ **The wire's refusal prefixes — ONE derivation, because a caller has to
/// tell a RETRY from a DENIAL and both arrive as a 400.**
///
/// A compare-and-swap that lost a race is *re-read and try again*; a role that
/// may not make the move is *stop*. Same status, different remedy, and a caller
/// that cannot tell them apart prints the wrong instruction — which is the
/// recurring shape in this workspace: a right classification with a wrong
/// instruction beside it.
///
/// ⚠ These were **two copies** until 2026-08-27 — emitted as literals into the
/// generated JavaScript and grepped for as separate literals in the adapter,
/// with no test asserting the two spellings matched. Nothing would have gone
/// red if they drifted; the adapter would simply have stopped recognising a
/// conflict and reported it as a transport error, and every consumer keying on
/// the prefix would have broken silently. **They are a public contract now, not
/// a convention**: adapters in other languages match on them too.
pub const CAS_CONFLICT: &str = "cas_conflict";
/// The record named by the request does not exist. See [`CAS_CONFLICT`].
pub const NO_RECORD: &str = "no_record";
/// The authenticated account does not act as the role the request claimed.
/// **Not retryable** — the opposite of [`CAS_CONFLICT`].
pub const ROLE_NOT_YOURS: &str = "role_not_yours";

pub struct PocketBaseLedger {
    base: String,
    token: String,
    agent: ureq::Agent,
    mode: Mode,
    shape: Shape,
    /// Whether the *installed* hooks said they write scope labels. Read from
    /// the ping at connect time rather than assumed from this crate's own
    /// version, because the two are deployed separately.
    writes_scope: bool,
    /// ⚠⚠ **Which COLUMNS the installed file admits, when it says.** `writes`
    /// answers in kinds — *state, counters, scope* — and that granularity was
    /// measured wrong: a mapped file emits one branch per column name known at
    /// generation time, so a counter added to a map afterwards is **accepted,
    /// silently dropped, and answered 200**, while `writes` still says
    /// "counters" and the adapter is told yes.
    ///
    /// Found by the first adopter, 2026-08-27, adding a counter to a live lane.
    /// Their ceiling would have read zero forever and the column would have
    /// stayed unguarded. **The only thing that caught it was a person diffing
    /// the generated file before installing**, which is exactly the vigilance
    /// this project exists to replace.
    ///
    /// `None` means the file did not say. That is **not** "writes nothing" —
    /// it is an older mapped file, whose names cannot be checked, or the
    /// generic shape, which stores counters and scope as JSON and therefore
    /// admits any name. The two are told apart by [`Shape`], never by this
    /// field alone.
    writable: Option<WritableColumns>,
}

/// The column names an installed mapped file said it can write.
#[derive(Debug, Clone, Default, PartialEq)]
struct WritableColumns {
    counters: Vec<String>,
    scope: Vec<String>,
    attributes: Vec<String>,
}

impl WritableColumns {
    fn from_ping(body: &Value) -> Option<Self> {
        let columns = body.get("columns")?.as_object()?;
        let names = |key: &str| -> Vec<String> {
            columns
                .get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default()
        };
        Some(WritableColumns {
            counters: names("counters"),
            scope: names("scope"),
            attributes: names("attributes"),
        })
    }
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
    /// Connect to `base_url` with a PocketBase auth token, using the generic
    /// collections the generated migration creates. Probes the generated
    /// routes and records the mode.
    pub fn connect(base_url: &str, token: &str) -> Result<Self, LedgerError> {
        Self::open(base_url, token, Shape::Generic)
    }

    /// Connect to `base_url`, refereeing the existing collection `map`
    /// describes. The console stays the human view of the same rows; filing
    /// stays with the collection's own procedure.
    pub fn connect_mapped(
        base_url: &str,
        token: &str,
        map: CollectionMap,
    ) -> Result<Self, LedgerError> {
        Self::open(base_url, token, Shape::Mapped(map))
    }

    fn open(base_url: &str, token: &str, shape: Shape) -> Result<Self, LedgerError> {
        let base = base_url.trim_end_matches('/').to_string();
        let agent = agent();
        let records = match &shape {
            Shape::Generic => RECORDS_COLLECTION,
            Shape::Mapped(map) => &map.records,
        };
        let resp = agent
            .get(format!("{base}/api/ferrostep/{records}/ping"))
            .call()
            .map_err(transport)?;
        let (status, body) = read(resp);
        let mode = if status == 200 && body.get("ferrostep").is_some() {
            Mode::Full
        } else {
            Mode::ReadOnly
        };
        // ⚠ Installed hooks outlive the binary that generated them. A file
        // written before rescope existed answers an apply carrying scope
        // updates with a perfectly cheerful 200 and writes no label — so the
        // caller is told a record moved between units of work when it did
        // not, which is worse than any refusal. The ping says what it can
        // write; anything that does not say is assumed not to.
        let writes_scope = body
            .get("writes")
            .and_then(Value::as_array)
            .is_some_and(|w| w.iter().any(|v| v.as_str() == Some("scope")));
        // ⚠ Column names when the file states them; `None` when it does not.
        // See `PocketBaseLedger::writable` for why absence is not "nothing".
        let writable = WritableColumns::from_ping(&body);
        Ok(PocketBaseLedger {
            base,
            token: token.to_string(),
            agent,
            mode,
            shape,
            writes_scope,
            writable,
        })
    }

    /// ⚠⚠ **Refuse a write to a column the installed file cannot reach.**
    /// `writes` answers in kinds, and a mapped file's real limit is a list of
    /// names fixed when it was generated — so "yes, counters" is true and
    /// useless when the counter in question has no branch in that file.
    ///
    /// Silence here is not permission. A mapped file that states no columns is
    /// older than this check, so its names cannot be verified and this returns
    /// `Ok` — the honest answer, and the reason the refusal text below tells
    /// the operator to regenerate rather than implying the column is wrong.
    fn refuse_unwritable(&self, kind: &str, names: Vec<&String>) -> Result<(), LedgerError> {
        let Shape::Mapped(_) = &self.shape else { return Ok(()) };
        let Some(known) = &self.writable else { return Ok(()) };
        let admitted = match kind {
            "counter" => &known.counters,
            "scope label" => &known.scope,
            _ => &known.attributes,
        };
        if let Some(missing) = names.into_iter().find(|n| !admitted.contains(n)) {
            return Err(LedgerError::Unsupported(format!(
                "write the {kind} '{missing}': the installed ferrostep hooks were generated \
                 before it was mapped, and would accept the request, ignore that column and \
                 answer 200 — regenerate them and reinstall"
            )));
        }
        Ok(())
    }

    /// Which write path answered at connect time. Callers that surface
    /// guarantees to a person should surface this beside them.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn records_collection(&self) -> &str {
        match &self.shape {
            Shape::Generic => RECORDS_COLLECTION,
            Shape::Mapped(map) => &map.records,
        }
    }

    fn events_collection(&self) -> &str {
        match &self.shape {
            Shape::Generic => EVENTS_COLLECTION,
            Shape::Mapped(map) => &map.events,
        }
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
        match &self.shape {
            Shape::Generic => {
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
                        let n = v.as_u64().ok_or_else(|| {
                            malformed(format!("counter '{name}' is not an integer"))
                        })?;
                        counters.insert(name.clone(), n as u32);
                    }
                }
                Ok(Record {
                    id,
                    snapshot: Snapshot { state, counters },
                    version: Version(version.to_string()),
                })
            }
            Shape::Mapped(map) => {
                let state = value
                    .get(&map.state_field)
                    .and_then(Value::as_str)
                    .ok_or_else(|| malformed(format!("no '{}' field", map.state_field)))?
                    .to_string();
                // An integer column absent from the row (or predating the
                // mapping) reads 0 — a valid starting token and a valid
                // never-spent counter.
                let version = value.get(&map.version_field).and_then(Value::as_i64).unwrap_or(0);
                let mut counters = BTreeMap::new();
                for name in &map.counter_fields {
                    let held = value.get(name).and_then(Value::as_u64).unwrap_or(0);
                    counters.insert(name.clone(), held as u32);
                }
                Ok(Record {
                    id,
                    snapshot: Snapshot { state, counters },
                    version: Version(version.to_string()),
                })
            }
        }
    }

    fn scope_labels(&self, value: &Value) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        match &self.shape {
            Shape::Generic => {
                if let Some(map) = value.get("scope").and_then(Value::as_object) {
                    for (k, v) in map {
                        labels.insert(k.clone(), v.as_str().unwrap_or_default().to_string());
                    }
                }
            }
            Shape::Mapped(map) => {
                for name in &map.scope_fields {
                    if let Some(v) = value.get(name).and_then(Value::as_str) {
                        labels.insert(name.clone(), v.to_string());
                    }
                }
            }
        }
        labels
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
            Mode::ReadOnly => Err(LedgerError::Unsupported(what.to_string())),
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
                "{}/api/collections/{}/records/{}",
                self.base,
                self.records_collection(),
                id.0
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
        if let Shape::Mapped(_) = &self.shape {
            // A mapped collection has its own filing procedure and its own
            // required fields; a record filed here would be a hollow row.
            return Err(LedgerError::Unsupported(
                "file records into a mapped collection; use the collection's own filing procedure"
                    .to_string(),
            ));
        }
        self.require_full("file a record without the ferrostep hooks installed")?;
        let Decision::Allow { to, .. } = decision else {
            return Err(LedgerError::NothingToApply);
        };
        // As everywhere: a filing decision's counter updates are scope-level
        // and are not persisted onto the record being filed.
        let resp = self
            .agent
            .post(format!(
                "{}/api/ferrostep/{}/create",
                self.base,
                self.records_collection()
            ))
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
        // Refuse rather than approximate — the same rule this adapter applies
        // to a missing write path, for the same reason. Hooks installed before
        // rescope existed accept this request and ignore the labels, and a
        // silent no-op reported as success is the one outcome worth failing to
        // avoid.
        // ⚠ By NAME, before the kind check below. "Yes, counters" is true and
        // useless when the counter in this request has no branch in the
        // installed file — the request would be accepted, that column ignored,
        // and 200 returned.
        self.refuse_unwritable("counter", next.counters.keys().collect())?;
        self.refuse_unwritable(
            "scope label",
            decided_scope_updates(&event.decision).keys().collect(),
        )?;
        if !decided_scope_updates(&event.decision).is_empty() && !self.writes_scope {
            return Err(LedgerError::Unsupported(
                "write scope labels: the installed ferrostep hooks predate rescope — \
                 regenerate them and reinstall"
                    .to_string(),
            ));
        }
        let resp = self
            .agent
            .post(format!(
                "{}/api/ferrostep/{}/apply",
                self.base,
                self.records_collection()
            ))
            .header("Authorization", &self.token)
            .send_json(json!({
                "record_id": record.id.0,
                "expected_version": expected,
                "state": next.state,
                "counters": next.counters,
                // Named labels, not a whole scope: the route writes exactly
                // these and leaves the record's other labels alone. Empty for
                // every move that is not a rescope.
                "scope": decided_scope_updates(&event.decision),
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
        if message.contains(CAS_CONFLICT) {
            return Err(LedgerError::VersionConflict {
                id: record.id.clone(),
                expected: record.version.clone(),
            });
        }
        if status == 404 || message.contains(NO_RECORD) {
            return Err(LedgerError::NotFound(record.id.clone()));
        }
        Err(LedgerError::Transport(format!("apply answered {status}: {message}")))
    }

    fn select(&self, scope: &Scope, states: &[String]) -> Result<Vec<Record>, LedgerError> {
        if states.is_empty() {
            return Ok(Vec::new());
        }
        let state_field = match &self.shape {
            Shape::Generic => "state",
            Shape::Mapped(map) => &map.state_field,
        };
        let filter = states
            .iter()
            .map(|s| format!("{state_field} = {}", quoted(s)))
            .collect::<Vec<_>>()
            .join(" || ");
        let items = self.list_all(self.records_collection(), &format!("({filter})"), "id")?;
        let mut out = Vec::new();
        for item in &items {
            // Scope narrowing happens here, in the adapter's own language,
            // exactly as in the SQLite adapter: a label key containing filter
            // syntax cannot be misread, at the cost of reading the state-wide
            // set — which the completeness check above already paid for.
            if !scope.matches(&self.scope_labels(item)) {
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
            self.events_collection(),
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

    /// Ask the installed hooks what the collection accepts right now.
    ///
    /// ⚠⚠ **TWO DIFFERENT AGES OF TRUTH LAND IN ONE VALUE HERE, AND KEEPING
    /// THEM APART IS THE POINT.** [`StoreShape::columns`] and
    /// [`StoreShape::accepted_states`] are read live by the route, so they
    /// describe the collection as it stands. [`StoreShape::writable`] comes
    /// from the ping and describes the *file that was installed*, which may
    /// be older than both the collection and this binary. A checker needs
    /// both because the failures are opposite: a column the collection lacks
    /// is refused loudly, and a column the installed file lacks is accepted,
    /// dropped and answered 200.
    ///
    /// Every way this can fail returns a refusal that names what went wrong,
    /// because the caller's next move differs in each case and "could not
    /// check" with no reason is the same as no answer at all.
    fn store_shape(&self) -> Result<StoreShape, LedgerError> {
        let records = self.records_collection().to_string();
        let resp = self
            .agent
            .get(format!("{}/api/ferrostep/{records}/schema", self.base))
            .header("Authorization", &self.token)
            .call()
            .map_err(transport)?;
        let (status, body) = read(resp);
        match status {
            200 => {}
            // ⚠ The route is newer than the first generated files, so an
            // installed deployment can be missing it entirely. That is not a
            // fault in the definition and must not be reported as one.
            404 => {
                return Err(LedgerError::Unsupported(format!(
                    "state the schema of '{records}': the installed ferrostep hooks predate the \
                     schema route, so nothing about the store was checked — regenerate them and \
                     reinstall"
                )));
            }
            401 | 403 => {
                return Err(LedgerError::Unsupported(format!(
                    "read the schema of '{records}' with this token: the route requires an \
                     authenticated caller and this one was refused — nothing was checked"
                )));
            }
            other => {
                return Err(LedgerError::Transport(format!(
                    "schema answered {other}: {}",
                    refusal(&body)
                )));
            }
        }
        // ⚠ The route reports a store-side failure inside a 200, because the
        // alternative shapes are worse: a 404 would be indistinguishable from
        // the route being absent, and a 500 from the store being down. Read it
        // as the refusal it is rather than as an empty schema — an empty
        // `columns` and a missing collection are nearly the same JSON and
        // opposite facts.
        if let Some(failed) = body.get("error").and_then(Value::as_str) {
            return Err(LedgerError::Unsupported(format!(
                "describe '{records}': the store answered '{failed}' — most often the collection \
                 named in the map does not exist under that name"
            )));
        }
        // ⚠ A route that answered without a `columns` key is one this build
        // does not understand; that is unknown, not empty.
        let columns = match body.get("columns").and_then(Value::as_object) {
            Some(map) => Answer::Said(
                map.iter()
                    .map(|(name, ty)| (name.clone(), ty.as_str().unwrap_or("?").to_string()))
                    .collect::<BTreeMap<String, String>>(),
            ),
            None => Answer::Unknown,
        };
        // ⚠⚠ THE THREE-WAY DISTINCTION IS MADE HERE AND NOWHERE ELSE, so it is
        // worth being explicit about which wire value is which. A `states`
        // ARRAY is a select column stating its values. `states: null` is the
        // route saying it looked and the column constrains nothing — a text
        // column takes any string, and every definition passes against it. A
        // MISSING key is an installed file too old to have been asked, and
        // that one is not a pass.
        let accepted_states = match body.get("states") {
            Some(Value::Array(values)) => Answer::Said(
                values.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            ),
            Some(Value::Null) => Answer::NothingToConstrain,
            _ => Answer::Unknown,
        };
        // From the ping, not from this route: what the INSTALLED file admits.
        // ⚠ The generic shape stores counters and scope as JSON on the row, so
        // there is no per-name column list that could go stale — that is
        // "nothing to constrain", and reporting it as unknown would send an
        // operator hunting for a file to regenerate that would not help.
        let writable = match (&self.shape, self.writable.as_ref()) {
            (Shape::Generic, _) => Answer::NothingToConstrain,
            (Shape::Mapped(_), Some(known)) => Answer::Said(BTreeMap::from([
                ("counters".to_string(), known.counters.clone()),
                ("scope".to_string(), known.scope.clone()),
                ("attributes".to_string(), known.attributes.clone()),
            ])),
            (Shape::Mapped(_), None) => Answer::Unknown,
        };
        Ok(StoreShape {
            subject: records,
            accepted_states,
            columns,
            writable,
        })
    }
}

/// The generated hook file for the **generic** shape: the transactional
/// apply/create routes and the ping the adapter probes for, all scoped under
/// the generic records collection. ⚠ Every handler is deliberately
/// self-contained — hook callbacks run in isolated runtimes where file-scope
/// helpers are not visible, so the duplication between the routes is
/// load-bearing, not tidiness waiting to happen.
/// The `schema` route both generated files carry: what the collection will
/// accept **right now**, read from the store at request time.
///
/// ⚠⚠ **THIS IS THE ONE ROUTE WHOSE ANSWER IS NOT FIXED AT GENERATION TIME**,
/// and that is the entire reason it exists. The ping's `columns` states the
/// names this file was *written* with — the right answer to "what can the
/// installed file write". This states what the *collection* has, which is the
/// half that moves without anyone regenerating anything: a select column gains
/// a value, a column is renamed, a counter column is never created. A
/// definition asserts things about exactly that half and nothing checked it,
/// so a state the column would refuse read as a documented move right up until
/// the first live transition failed.
///
/// ⚠ **Authenticated, deliberately.** The ping is anonymous because liveness
/// and this file's own abilities are not worth protecting; a collection's field
/// names and the accepted values of its select columns are a different
/// disclosure, and widening an anonymous route to carry them would be a
/// decision made by accident. A caller running `doctor` holds a token already.
///
/// ⚠⚠ **AND AN ORDINARY ONE IS ENOUGH, WHICH IS WHY THIS ROUTE EXISTS RATHER
/// THAN A CALL TO THE COLLECTIONS API.** Measured on a live instance,
/// 2026-08-27: an ordinary actor token is refused **403** by
/// `/api/collections/<name>` and answered **200** here. Reading the schema
/// through the admin API would have required the checker's most likely caller —
/// an agent holding the loop's own credential — to be an administrator, and
/// "could not check" would have been the normal answer for everyone else.
///
/// ⚠ **Only values that came back through `JSON.parse` go into the response.**
/// The store's native collection object is reachable here and handing one
/// straight to `e.json` was measured to work — but the parsed copy is plain
/// data with no behaviour, which is what a wire format should carry. ⚠ Key
/// order in the emitted object is **not** stable (measured against a live
/// instance, 2026-08-27): nothing may parse this by position.
///
/// A collection this file names but the store does not have throws, and the
/// route answers with the error rather than an empty schema — because an empty
/// schema and a missing collection serialize to nearly the same JSON and are
/// opposite facts.
fn schema_route_js(path: &str, records: &str, state_field: &str, version: &str) -> String {
    format!(
        r##"
routerAdd("GET", "/api/ferrostep/{path}/schema", (e) => {{
    // Read at request time, never baked in: this answers what the collection
    // accepts NOW, which is the only version of the question worth asking.
    const columns = {{}};
    let states = null;
    let failed = "";
    try {{
        const col = JSON.parse(JSON.stringify($app.findCollectionByNameOrId("{records}")));
        const fields = col.fields || [];
        for (let i = 0; i < fields.length; i++) {{
            columns[fields[i].name] = String(fields[i].type);
            // `states` stays null unless the column actually constrains its
            // values. A text column takes any string, and reporting that as
            // "no accepted states" would invent a fault in every definition.
            if (fields[i].name === "{state_field}" && fields[i].type === "select") {{
                states = fields[i].values || [];
            }}
        }}
    }} catch (err) {{
        failed = String(err);
    }}
    if (failed !== "") {{
        return e.json(200, {{ "ferrostep": "{version}", "collection": "{records}", "error": failed }});
    }}
    return e.json(200, {{
        "ferrostep": "{version}",
        "collection": "{records}",
        "state_field": "{state_field}",
        "columns": columns,
        "states": states
    }});
}}, $apis.requireAuth());
"##
    )
}

pub fn hooks_file(actors: &ActorBinding) -> String {
    // Bound locally so the generated text and the adapter's matcher are
    // ONE derivation — see `CAS_CONFLICT`.
    let (cas_conflict, no_record) = (CAS_CONFLICT, NO_RECORD);
    let version = env!("CARGO_PKG_VERSION");
    let binding = role_binding_js(actors);
    // The generic collection stores state as text and counters and scope as
    // JSON, so this route will report a state column that constrains nothing
    // and no per-name counter columns. That is the honest answer and it is
    // worth stating: "checked, and this shape has nothing to disagree with"
    // is a different result from "could not check", and only one of them
    // should let a person stop looking.
    let schema_route = schema_route_js(RECORDS_COLLECTION, RECORDS_COLLECTION, "state", version);
    format!(
        r#"// ferrostep.pb.js — generated by ferrostep-pocketbase v{version}.
// Do not hand-edit; regenerate and reinstall instead. Each handler is
// self-contained on purpose: hook callbacks run in isolated runtimes where
// file-scope helpers are not visible, so shared logic here would fail on
// every call while reading perfectly.

routerAdd("GET", "/api/ferrostep/ferrostep_records/ping", (e) => {{
    // `writes` is how an adapter learns what an INSTALLED file can do. Hooks
    // outlive the binary that generated them, so a newer adapter must be able
    // to find out that an older deployment cannot honour part of a request —
    // rather than sending it and being told 200.
    return e.json(200, {{ "ferrostep": "{version}", "writes": ["state", "counters", "scope"] }});
}});
{schema_route}
routerAdd("POST", "/api/ferrostep/ferrostep_records/apply", (e) => {{
    const body = e.requestInfo().body;
{binding}
    const recordId = String(body.record_id || "");
    const expected = Number(body.expected_version);
    let version = 0;
    $app.runInTransaction((txApp) => {{
        let rec;
        try {{
            rec = txApp.findRecordById("ferrostep_records", recordId);
        }} catch (err) {{
            throw new NotFoundError("{no_record}: " + recordId);
        }}
        const held = rec.getInt("version");
        if (held !== expected) {{
            // The compare lives INSIDE the transaction. Measured as the only
            // placement that survives concurrent writers; the same check
            // outside it intermittently passes while losing updates.
            throw new BadRequestError("{cas_conflict}: expected " + expected + ", found " + held);
        }}
        rec.set("state", String(body.state));
        rec.set("counters", body.counters || {{}});
        // A rescope names labels, so the stored map is merged rather than
        // replaced — a record keeps the parts of its identity nobody asked to
        // change. Skipped entirely when nothing was named, which is every
        // ordinary move.
        const scopeIn = body.scope || {{}};
        const scopeKeys = Object.keys(scopeIn);
        if (scopeKeys.length > 0) {{
            const labels = rec.get("scope") || {{}};
            for (let i = 0; i < scopeKeys.length; i++) {{
                labels[scopeKeys[i]] = String(scopeIn[scopeKeys[i]]);
            }}
            rec.set("scope", labels);
        }}
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
        ev.set("role", actingRole);
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

routerAdd("POST", "/api/ferrostep/ferrostep_records/create", (e) => {{
    const body = e.requestInfo().body;
{binding}
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
        ev.set("role", actingRole);
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

/// The generated hook file for a **mapped** collection: the ping and the
/// transactional apply route writing the mapped columns, plus — when the
/// deployment asks for one — the store-side release: writing the decision
/// field performs the definition's release transition with the referee's
/// bookkeeping attached (version bump, event append), so the console's
/// one-save flow survives the cutover under a single referee.
///
/// ⚠ One caveat the file also states: the release's event append runs after
/// the row's own save commits (the row itself — state, counters, decision,
/// version — is one atomic write). A crash in between leaves a correct row
/// whose release event is missing. The apply route has no such window.
pub fn hooks_file_mapped(
    map: &CollectionMap,
    release: Option<&ReleaseHook>,
    actors: &ActorBinding,
) -> String {
    // Bound locally so the generated text and the adapter's matcher are
    // ONE derivation — see `CAS_CONFLICT`.
    let (cas_conflict, no_record) = (CAS_CONFLICT, NO_RECORD);
    let version = env!("CARGO_PKG_VERSION");
    let binding = role_binding_js(actors);
    let records = &map.records;
    let events = &map.events;
    let state = &map.state_field;
    let version_field = &map.version_field;
    let counter_sets = map
        .counter_fields
        .iter()
        .map(|name| {
            format!(
                r#"        if (body.counters && body.counters["{name}"] !== undefined) {{
            rec.set("{name}", Number(body.counters["{name}"]));
        }}"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    // ⚠ One `if` per DECLARED label, rather than a loop over whatever the
    // caller sent. That makes the map's `scope_fields` an allowlist by
    // construction: a request naming any other column cannot write it, because
    // no line exists that would. An authenticated route is not an unconstrained
    // one, and the difference has to be structural rather than remembered.
    let scope_sets = map
        .scope_fields
        .iter()
        .map(|name| {
            format!(
                r#"        if (body.scope && body.scope["{name}"] !== undefined) {{
            rec.set("{name}", String(body.scope["{name}"]));
        }}"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // ⚠⚠ THE GUARD CLOSES THESE COLUMNS, SO THE ROUTE HAS TO OPEN ONE. Listing
    // a column in `attribute_fields` puts it in `REFEREED`, and a guarded
    // column with no write path is a documented, unreachable operation — the
    // defect this pair has now paid for repeatedly. Measured while building
    // this: with the guard on and these lines absent, the first adopter's
    // grade command would have had no way to reach the store at all.
    //
    // ⚠ Same one-`if`-per-DECLARED-name shape as the two above, and for the
    // same reason: the map's list is an allowlist by construction, so a request
    // naming any other column cannot write it.
    let attribute_sets = map
        .attribute_fields
        .iter()
        .map(|name| {
            format!(
                r#"        if (body.attributes && body.attributes["{name}"] !== undefined) {{
            rec.set("{name}", String(body.attributes["{name}"]));
        }}"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    // ⚠ The ping says what an INSTALLED file can write, and an older file must
    // not be read as able to write attributes. Advertised only when the map
    // declares some, so a deployment without them keeps answering exactly what
    // it answered before.
    // ⚠⚠ NAMES, NOT ONLY KINDS. `writes` says "counters"; a mapped file's real
    // limit is the list of column names it was generated with, because the
    // route carries one branch per name. Measured 2026-08-27 on a live lane: a
    // counter added to a map after the file was installed is accepted, dropped,
    // and answered 200 — the ceiling never fires and the column stays
    // unguarded — while `writes` still says "counters" and the adapter is told
    // yes. A person diffing the generated file was the only thing that caught
    // it.
    //
    // ⚠ A NEW KEY rather than a changed one, deliberately: an older adapter
    // reading `writes` sees exactly what it saw before, and a newer one that
    // finds no `columns` knows it cannot verify rather than concluding there
    // is nothing to write. Fixing a compatibility defect incompatibly would be
    // the same mistake twice.
    let json_list = |names: &[String]| -> String {
        names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
    };
    let columns_json = format!(
        r#"{{ "counters": [{}], "scope": [{}], "attributes": [{}] }}"#,
        json_list(&map.counter_fields),
        json_list(&map.scope_fields),
        json_list(&map.attribute_fields),
    );
    // ⚠ Joined, and empties dropped, so a map that declares no columns of some
    // kind does not leave a blank line behind in the generated file. Cosmetic,
    // and worth it for one reason: this file is READ AS A DIFF before it is
    // installed, and every line of noise in that diff is a line an operator has
    // to account for before they can approve the real change. The first adopter
    // read a three-change diff and had to rule out a stray blank line to get
    // there.
    let column_sets = [counter_sets, scope_sets, attribute_sets]
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let schema_route = schema_route_js(records, records, state, version);
    let writes_list = if map.attribute_fields.is_empty() {
        r#"["state", "counters", "scope"]"#
    } else {
        r#"["state", "counters", "scope", "attributes"]"#
    };

    let mut out = format!(
        r#"// ferrostep.{records}.pb.js — generated by ferrostep-pocketbase v{version}
// for the mapped collection '{records}'. Do not hand-edit; regenerate and
// reinstall instead. Each handler is self-contained on purpose: hook
// callbacks run in isolated runtimes where file-scope helpers are not
// visible, so shared logic here would fail on every call while reading
// perfectly.

routerAdd("GET", "/api/ferrostep/{records}/ping", (e) => {{
    // See the generic file: this is how an adapter learns what the INSTALLED
    // routes can write, rather than assuming its own generation's abilities.
    return e.json(200, {{ "ferrostep": "{version}", "writes": {writes_list}, "columns": {columns_json} }});
}});
{schema_route}
routerAdd("POST", "/api/ferrostep/{records}/apply", (e) => {{
    const body = e.requestInfo().body;
{binding}
    const recordId = String(body.record_id || "");
    const expected = Number(body.expected_version);
    let next = 0;
    $app.runInTransaction((txApp) => {{
        let rec;
        try {{
            rec = txApp.findRecordById("{records}", recordId);
        }} catch (err) {{
            throw new NotFoundError("{no_record}: " + recordId);
        }}
        const held = rec.getInt("{version_field}");
        if (held !== expected) {{
            // The compare lives INSIDE the transaction. Measured as the only
            // placement that survives concurrent writers; the same check
            // outside it intermittently passes while losing updates.
            throw new BadRequestError("{cas_conflict}: expected " + expected + ", found " + held);
        }}
        rec.set("{state}", String(body.state));
{column_sets}
        rec.set("{version_field}", held + 1);
        txApp.save(rec);
        let seq = 1;
        const last = txApp.findRecordsByFilter("{events}", "record = {{:id}}", "-seq", 1, 0, {{ "id": recordId }});
        if (last.length > 0) {{
            seq = last[0].getInt("seq") + 1;
        }}
        const ev = new Record(txApp.findCollectionByNameOrId("{events}"));
        ev.set("record", recordId);
        ev.set("seq", seq);
        ev.set("actor", String((body.event && body.event.actor) || ""));
        ev.set("role", actingRole);
        if (body.event && body.event.from_state) {{
            ev.set("from_state", String(body.event.from_state));
        }}
        ev.set("decision", (body.event && body.event.decision) || {{}});
        if (body.event && body.event.note) {{
            ev.set("note", String(body.event.note));
        }}
        txApp.save(ev);
        next = held + 1;
    }});
    return e.json(200, {{ "version": next }});
}}, $apis.requireAuth());
"#
    );

    // ⚠ Registered BEFORE the release hook, deliberately. Handlers chain
    // through `e.next()`, so this one sees the client's own changes and
    // passes control on; the release hook's writes happen downstream of it
    // and are not its business. Reversing the order would have the guard
    // refuse the release it is supposed to permit.
    if map.guard_refereed_fields {
        // ⚠ Through `refereed_fields`, never inline — `ferrostep explain`
        // prints the same list as the set to sweep for before this is turned
        // on, and a guard closing one set while the list names another is the
        // failure both halves exist to prevent.
        let refereed: Vec<String> =
            map.refereed_fields().iter().map(|f| format!("\"{f}\"")).collect();
        let list = refereed.join(", ");
        out.push_str(&format!(
            r#"
// ⚠⚠ The refereed columns move through the apply route or they do not move.
// Without this, a client holding credentials edits `{state}` straight on the
// row and the referee never hears about it — no version bump, no event, and
// every later compare-and-swap arguing about a number that changed behind it.
//
// It is a HOOK rather than an access rule because a store administrator
// bypasses rules and does not bypass this — measured on this backend.
//
// The route's own writes are internal saves and never reach a request hook,
// so the referee is unaffected; only a direct edit is refused. Same for the
// release hook below, which runs after this one and writes downstream of it.
onRecordUpdateRequest((e) => {{
    const REFEREED = [{list}];
    const changed = [];
    for (let i = 0; i < REFEREED.length; i++) {{
        const f = REFEREED[i];
        if (String(e.record.get(f)) !== String(e.record.original().get(f))) {{
            changed.push(f);
        }}
    }}
    if (changed.length > 0) {{
        throw new BadRequestError(
            "refereed_field: " + changed.join(", ") + " may only change through " +
            "/api/ferrostep/{records}/apply, which records who moved the record and why. " +
            "A direct write here would leave the ledger's history disagreeing with the row."
        );
    }}
    e.next();
}}, "{records}");
"#
        ));
    }

    if let Some(release) = release {
        let decision_field = &release.decision_field;
        let from_state = &release.from_state;
        let to_state = &release.to_state;
        let role = &release.role;
        let writers = release
            .writers
            .iter()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let resets = release
            .reset_counters
            .iter()
            .map(|name| format!("            e.record.set(\"{name}\", 0);"))
            .collect::<Vec<_>>()
            .join("\n");
        let reset_updates = release
            .reset_counters
            .iter()
            .map(|name| format!("\"{name}\": 0"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            r#"
// The store-side release: writing '{decision_field}' IS taking the
// definition's release transition, so the console's one-save flow keeps
// working with the referee's bookkeeping attached. Guarded as a transition:
// only from '{from_state}' does the state move; a decision on a record
// already at '{to_state}' re-arms its counters (a revised decision is a new
// instruction and needs attempts); a decision anywhere else is a note and
// moves nothing. Only the allowlisted writers may change the field at all —
// fail-closed: an account added later is refused here until somebody
// deliberately adds it.
onRecordUpdateRequest((e) => {{
    const WRITERS = [{writers}];
    const after = (e.record.getString("{decision_field}") || "").trim();
    const before = (e.record.original().getString("{decision_field}") || "").trim();
    let releasedFrom = null;
    let releaseNote = "";
    let releasedBy = "";
    if (after !== before) {{
        const who = e.auth ? (e.auth.getString("email") || "") : "";
        if (WRITERS.indexOf(who) === -1) {{
            throw new BadRequestError(
                "{decision_field} is the owner's field. Agents read it and never write it — " +
                "an agent writing here would forge the answer to a question it raised. " +
                "(authenticated as: " + (who || "anonymous") + ")"
            );
        }}
        if (after !== "") {{
            const from = e.record.original().getString("{state}");
            if (from === "{from_state}" || from === "{to_state}") {{
                if (from === "{from_state}") {{
                    e.record.set("{state}", "{to_state}");
                }}
{resets}
                e.record.set("{version_field}", e.record.original().getInt("{version_field}") + 1);
                releasedFrom = from;
                releaseNote = after;
                releasedBy = who;
            }}
        }}
    }}
    e.next();
    // The row above committed as one save. The event line lands after it —
    // a crash between the two loses the event, never the row.
    if (releasedFrom !== null) {{
        try {{
            $app.runInTransaction((txApp) => {{
                let seq = 1;
                const last = txApp.findRecordsByFilter("{events}", "record = {{:id}}", "-seq", 1, 0, {{ "id": e.record.id }});
                if (last.length > 0) {{
                    seq = last[0].getInt("seq") + 1;
                }}
                const ev = new Record(txApp.findCollectionByNameOrId("{events}"));
                ev.set("record", e.record.id);
                ev.set("seq", seq);
                ev.set("actor", releasedBy);
                ev.set("role", "{role}");
                ev.set("from_state", releasedFrom);
                ev.set("decision", {{ "kind": "allow", "to": "{to_state}", "counter_updates": {{ {reset_updates} }} }});
                ev.set("note", releaseNote);
                txApp.save(ev);
            }});
        }} catch (err) {{
            console.log("ferrostep: release event append failed for " + e.record.id + ": " + err);
        }}
    }}
}}, "{records}");

// Creation parity for the refusal: the decision field is the owner's from
// the first save, not only after it.
onRecordCreateRequest((e) => {{
    const WRITERS = [{writers}];
    const after = (e.record.getString("{decision_field}") || "").trim();
    if (after !== "") {{
        const who = e.auth ? (e.auth.getString("email") || "") : "";
        if (WRITERS.indexOf(who) === -1) {{
            throw new BadRequestError(
                "{decision_field} is the owner's field. Agents read it and never write it — " +
                "an agent writing here would forge the answer to a question it raised. " +
                "(authenticated as: " + (who || "anonymous") + ")"
            );
        }}
    }}
    e.next();
}}, "{records}");
"#
        ));
    }
    out
}

/// The generated migration for the **generic** shape: both collections, the
/// unique `(record, seq)` index that referees concurrent appends, and rules
/// that are `null` (writes: nobody over REST) or auth-gated (reads) — never
/// `""`, which would mean *public*.
pub fn migration_file(actors: &ActorBinding) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let actors_block = actors_collection_js(actors);
    let actor_collection = &actors.collection;
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
        // ⚠ Authenticated-read here and NOT in the mapped migration, on
        // purpose. This shape creates both collections, so the history and
        // the records it describes carry the same rule by construction and
        // neither can outrank the other. A mapped deployment refers to a
        // collection somebody else made, which is why it cannot assume.
        "listRule": "@request.auth.id != ''",
        "viewRule": "@request.auth.id != ''",
        "createRule": null,
        "updateRule": null,
        "deleteRule": null
    }});
    app.save(events);
{actors_block}}}, (app) => {{
    for (const name of ["{actor_collection}", "ferrostep_events", "ferrostep_records"]) {{
        try {{
            app.delete(app.findCollectionByNameOrId(name));
        }} catch (err) {{}}
    }}
}});
"#
    )
}

/// The generated migration for a **mapped** deployment: adds the version
/// column to the mapped collection (guarded, so re-running is harmless) and
/// creates the event collection beside it. Applied the way the store's
/// schema has always evolved — a `pb_migrations` file, run at the next
/// start with full privileges — which pairs naturally with the restart the
/// hooks install already causes.
pub fn migration_file_mapped(map: &CollectionMap, actors: &ActorBinding) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let actors_block = actors_collection_js(actors);
    let actor_collection = &actors.collection;
    let records = &map.records;
    let events = &map.events;
    let version_field = &map.version_field;
    format!(
        r#"// ferrostep mapping for '{records}' — generated by ferrostep-pocketbase v{version}.
// Do not hand-edit; regenerate and reinstall instead.
migrate((app) => {{
    const records = app.findCollectionByNameOrId("{records}");
    if (!records.fields.getByName("{version_field}")) {{
        records.fields.add(new NumberField({{ "name": "{version_field}", "onlyInt": true }}));
        app.save(records);
    }}
    let haveEvents = true;
    try {{
        app.findCollectionByNameOrId("{events}");
    }} catch (err) {{
        haveEvents = false;
    }}
    if (!haveEvents) {{
        const events = new Collection({{
            "name": "{events}",
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
                "CREATE UNIQUE INDEX idx_{events}_record_seq ON {events} (record, seq)"
            ],
            // ⚠ Superuser-only reads, deliberately. This collection describes
            // rows in "{records}" — YOUR collection, under YOUR rules, which
            // this migration cannot read the meaning of. Anything laxer risks
            // a history more readable than its subject: every state change,
            // actor, role and note about records the reader may not open.
            // Widen it in the admin UI if you mean to; the `haveEvents` guard
            // above means a later regeneration will not undo that.
            "listRule": null,
            "viewRule": null,
            "createRule": null,
            "updateRule": null,
            "deleteRule": null
        }});
        app.save(events);
    }}
{actors_block}}}, (app) => {{
    try {{
        const records = app.findCollectionByNameOrId("{records}");
        records.fields.removeByName("{version_field}");
        app.save(records);
    }} catch (err) {{}}
    for (const name of ["{actor_collection}", "{events}"]) {{
        try {{
            app.delete(app.findCollectionByNameOrId(name));
        }} catch (err) {{}}
    }}
}});
"#
    )
}

/// The JSON body that creates a mapped deployment's event collection over
/// the collections API — for deployments that provision by API call rather
/// than migration file. Same shape and rules as the generic migration's
/// event collection, under the mapped name.
/// The generated JS that creates the actor collection when it is absent.
///
/// ⚠ **Creating it is not minting identities.** The collection is an empty
/// auth collection with a role field — a place for the deployment to say
/// which role a principal it already has may act in. No account is created
/// here, and this is the last point at which this crate has an opinion about
/// who exists. A deployment pointing `ActorBinding::collection` at an auth
/// collection it already runs gets this block skipped by the same guard.
///
/// Reads are superuser-only for the reason [`events_collection_body`] gives:
/// a list of who may act in which role is not something to hand out by
/// default, and widening is the deployment's deliberate act.
fn actors_collection_js(actors: &ActorBinding) -> String {
    let ActorBinding { collection, role_field, .. } = actors;
    format!(
        r#"    let haveActors = true;
    try {{
        app.findCollectionByNameOrId("{collection}");
    }} catch (err) {{
        haveActors = false;
    }}
    if (!haveActors) {{
        const actors = new Collection({{
            "name": "{collection}",
            "type": "auth",
            "fields": [
                {{ "name": "{role_field}", "type": "text", "required": true }}
            ],
            "listRule": null,
            "viewRule": null,
            "createRule": null,
            "updateRule": null,
            "deleteRule": null
        }});
        app.save(actors);
    }}
"#
    )
}

/// The event collection's shape, for creating one outside a migration.
///
/// ⚠ **Reads are superuser-only, and that is the only default this can
/// safely carry.** It is handed a name and nothing else, so it cannot know
/// what it will sit beside — and beside a collection with stricter rules, a
/// laxer history is every state change, actor, role and note about records
/// the reader may not open. The strict end is the only one that is right in
/// every case; widening is the deployment's deliberate act.
pub fn events_collection_body(events: &str) -> Value {
    json!({
        "name": events,
        "type": "base",
        "fields": [
            { "name": "record", "type": "text", "required": true },
            { "name": "seq", "type": "number", "required": true },
            { "name": "actor", "type": "text", "required": true },
            { "name": "role", "type": "text", "required": true },
            { "name": "from_state", "type": "text" },
            { "name": "decision", "type": "json", "required": true },
            { "name": "note", "type": "text", "max": 50000 },
            { "name": "at", "type": "autodate", "onCreate": true, "onUpdate": false }
        ],
        "indexes": [
            format!("CREATE UNIQUE INDEX idx_{events}_record_seq ON {events} (record, seq)")
        ],
        "listRule": null,
        "viewRule": null,
        "createRule": null,
        "updateRule": null,
        "deleteRule": null
    })
}

/// Write the generic shape's generated files under a PocketBase working
/// directory: `pb_migrations/…_ferrostep.js` and `pb_hooks/ferrostep.pb.js`.
/// Returns the two paths, migration first.
///
/// ⚠ A server watching its hooks directory restarts itself when the hook
/// file lands; a health check fired immediately after this returns can
/// answer before that restart begins, which is not evidence of anything.
pub fn install_files(pb_dir: &Path, actors: &ActorBinding) -> std::io::Result<(PathBuf, PathBuf)> {
    let migrations = pb_dir.join("pb_migrations");
    let hooks = pb_dir.join("pb_hooks");
    std::fs::create_dir_all(&migrations)?;
    std::fs::create_dir_all(&hooks)?;
    // The numeric prefix is PocketBase's ordering convention; fixed, because
    // regenerating must overwrite rather than accumulate.
    let migration_path = migrations.join("1756000000_ferrostep.js");
    let hooks_path = hooks.join("ferrostep.pb.js");
    std::fs::write(&migration_path, migration_file(actors))?;
    std::fs::write(&hooks_path, hooks_file(actors))?;
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

    /// A ping from hooks generated before rescope existed. Deliberately the
    /// default fixture: it is what every already-installed deployment answers,
    /// and the adapter has to behave correctly against those first.
    fn ping_full() -> (&'static str, u16, String) {
        ("/api/ferrostep/ferrostep_records/ping", 200, r#"{"ferrostep":"test"}"#.to_string())
    }

    /// A ping from hooks that can write scope labels.
    fn ping_with_scope() -> (&'static str, u16, String) {
        (
            "/api/ferrostep/ferrostep_records/ping",
            200,
            r#"{"ferrostep":"test","writes":["state","counters","scope"]}"#.to_string(),
        )
    }

    /// ⚠⚠ **SLICE ON AN ANCHOR ONLY IF THE ANCHOR IS UNIQUE.** A guard that
    /// searches for its subject takes the first match for the only match, and
    /// the earliest occurrence of a name in a self-documenting file is very
    /// often the **comment or help text that names it first** — so the better a
    /// file documents itself, the more reliably a guard written against it
    /// inspects the prose about the mechanism instead of the mechanism.
    ///
    /// ⚠ **It survives the obvious mutation.** Deleting the thing under guard
    /// leaves its name behind in the comment, so a test written this way passes
    /// its own mutation check at the moment it is written and is wrong later.
    ///
    /// ⚠ This is [[empty enumeration]] with one variable changed: there the
    /// population is empty and every assertion is vacuously true; here it is
    /// non-empty and **wrong**, so the assertions are meaningfully true about
    /// the wrong region — which reads better in a green run, not worse.
    ///
    /// Found by the adopting loop's resident, 2026-08-27, as three independent
    /// instances on one branch — all caught by review, none by the author. The
    /// anchors below are unique today; this makes that a checked property
    /// rather than a lucky one.
    fn slice_once<'a>(text: &'a str, open: &str, close: char) -> &'a str {
        let hits = text.matches(open).count();
        assert_eq!(hits, 1, "anchor {open:?} occurs {hits} times — a slice on it would read the wrong region");
        let (_, tail) = text.split_once(open).expect("anchor counted but not found");
        // ⚠⚠ THE MATCHING CLOSE, NOT THE FIRST ONE. Slicing to the first `]`
        // or `}` is the span variant of the same defect: nest one structure
        // inside the region and the span **silently shrinks**, so the guard
        // goes on asserting about a region that no longer holds its subject —
        // and it shrinks in the direction that keeps the test green.
        //
        // ⚠ `open` must end with the opening delimiter (so `tail` begins INSIDE
        // the region at depth zero); the assertion below is what enforces that
        // rather than trusting each call site to remember.
        let opener = match close {
            ']' => '[',
            '}' => '{',
            ')' => '(',
            other => panic!("slice_once has no opener for {other:?}"),
        };
        assert!(
            open.ends_with(opener),
            "anchor {open:?} must end with {opener:?} so the scan starts inside the region"
        );
        let mut depth = 0usize;
        for (i, ch) in tail.char_indices() {
            if ch == opener {
                depth += 1;
            } else if ch == close {
                if depth == 0 {
                    assert!(!tail[..i].is_empty(), "anchor {open:?} spans nothing");
                    return &tail[..i];
                }
                depth -= 1;
            }
        }
        panic!("anchor {open:?} has no matching {close:?}")
    }

    fn tickets_map() -> CollectionMap {
        CollectionMap {
            records: "tickets".to_string(),
            events: "ticket_events".to_string(),
            state_field: "stage".to_string(),
            version_field: "fs_version".to_string(),
            counter_fields: vec!["attempts".to_string()],
            scope_fields: vec!["lane".to_string()],
            // ⚠ One of every category, deliberately. The known-answer test
            // below asserts this list by value, and a category the fixture
            // leaves empty is a category that test cannot see.
            attribute_fields: vec!["severity".to_string()],
            guard_refereed_fields: false,
        }
    }

    /// The same map with the direct-write guard on — the hardened shape a
    /// deployment opts into once its actors exist.
    fn guarded_map() -> CollectionMap {
        CollectionMap { guard_refereed_fields: true, ..tickets_map() }
    }

    /// ⚠⚠ **A GUARDED COLUMN WITH NO WRITE PATH IS A DOCUMENTED, UNREACHABLE
    /// OPERATION**, and this pair has paid for that shape more than once. When
    /// `attribute_fields` was added, the guard closed the column immediately —
    /// because the refereed list is one derivation — while the apply route
    /// could not write it, so the adopter's grade command would have had no way
    /// to reach the store at all. **Measured in exactly that broken state
    /// before the route was taught the branch.** Both halves, asserted
    /// together, so neither can ship without the other.
    #[test]
    fn a_guarded_attribute_column_is_also_writable_through_the_route() {
        let hooks =
            hooks_file_mapped(&guarded_map(), None, &ActorBinding::default());
        let refereed = slice_once(&hooks, "const REFEREED = [", ']').to_string();
        assert!(refereed.contains("\"severity\""), "guard does not close it: {refereed}");
        assert!(
            hooks.contains(r#"body.attributes["severity"]"#),
            "guarded but unreachable — the route cannot write it"
        );
    }

    /// ⚠ **An installed file must not be read as able to do something it
    /// cannot.** The ping is how an adapter learns that, so a map declaring no
    /// attributes has to answer exactly what it answered before this category
    /// existed — otherwise every older deployment starts claiming a capability
    /// its generated file does not have.
    /// ⚠⚠ **The adapter matches on these prefixes and the generated file emits
    /// them; asserted against the GENERATED TEXT, not against a second copy of
    /// the spellings.** They were two independent literals until 2026-08-27 —
    /// emitted into the JavaScript, grepped for in the adapter, with nothing
    /// asserting the two agreed. Drift would have gone unnoticed in the
    /// direction that always does: the adapter would have stopped recognising a
    /// conflict and reported a plain transport error, and the caller's remedy
    /// text — *re-read and retry* — would have vanished with it.
    ///
    /// ⚠ It matters beyond this crate. A caller has to tell a **retryable**
    /// refusal from a **denial**, both of which arrive as a 400, and adapters
    /// in other languages key on the same prefixes. That makes them a wire
    /// contract, and a contract with two spellings is a contract with none.
    #[test]
    fn the_wire_prefixes_the_adapter_matches_are_the_ones_the_hooks_emit() {
        for hooks in [
            hooks_file(&ActorBinding::default()),
            hooks_file_mapped(&guarded_map(), None, &ActorBinding::default()),
        ] {
            assert!(
                hooks.contains(&format!("\"{CAS_CONFLICT}: ")),
                "the generated file does not emit the prefix the adapter matches"
            );
            assert!(hooks.contains(&format!("\"{NO_RECORD}: ")), "{NO_RECORD} not emitted");
        }
        // The role refusal only exists where an actor binding does.
        let bound = ActorBinding { role_field: "role".to_string(), ..ActorBinding::default() };
        assert!(
            hooks_file_mapped(&guarded_map(), None, &bound).contains(ROLE_NOT_YOURS),
            "{ROLE_NOT_YOURS} not emitted where roles are bound"
        );
        // ⚠ And the two must not be the same string, or a caller keying on the
        // prefix cannot tell a retry from a denial — the whole point of them.
        assert_ne!(CAS_CONFLICT, ROLE_NOT_YOURS);
    }

    /// Reads the ping's `writes` list out of the generated file — the KIND
    /// list, which is the thing this test is about.
    ///
    /// ⚠ **This helper exists because the first version of the test searched
    /// the whole file for `"attributes"` and was right by accident.** Once the
    /// ping also carried a `columns` object, that string appeared in every
    /// generated file as an always-present key holding an empty array, and the
    /// negative assertion failed on correct output. **The property is "does
    /// the kind list name it", not "does the token occur somewhere"** — the
    /// same axis a guard in this workspace already died on.
    fn ping_writes(hooks: &str) -> String {
        slice_once(hooks, r#""writes": ["#, ']').to_string()
    }

    #[test]
    fn the_ping_advertises_attributes_only_when_the_map_declares_some() {
        let with = hooks_file_mapped(&guarded_map(), None, &ActorBinding::default());
        assert!(ping_writes(&with).contains("attributes"), "declared, but not advertised");

        let without = CollectionMap { attribute_fields: vec![], ..guarded_map() };
        let plain = hooks_file_mapped(&without, None, &ActorBinding::default());
        assert!(
            !ping_writes(&plain).contains("attributes"),
            "a map with no attributes advertises the capability: {}",
            ping_writes(&plain)
        );
        // ⚠ Floor: `ping_writes` panics if the ping moved, so both assertions
        // above are known to have read a real list rather than an empty string.
        assert!(ping_writes(&plain).contains("state"), "the ping lists nothing at all");
    }

    /// ⚠⚠ **The kind list is the thing that was measured wrong.** "Yes,
    /// counters" is true and useless when the counter in a request has no
    /// branch in the installed file. The ping now also states the column
    /// NAMES, so an adapter can refuse by name instead of being told yes.
    #[test]
    fn the_ping_states_the_column_names_it_can_actually_write() {
        let hooks = hooks_file_mapped(&guarded_map(), None, &ActorBinding::default());
        let columns = slice_once(&hooks, r#""columns": {"#, '}').to_string();
        assert!(columns.contains(r#""attempts""#), "counter not named: {columns}");
        assert!(columns.contains(r#""lane""#), "scope label not named: {columns}");
        assert!(columns.contains(r#""severity""#), "attribute not named: {columns}");
        // ⚠ And a column the map does NOT declare must be absent, or the list
        // is decoration rather than a limit.
        assert!(!columns.contains(r#""disputes""#), "named a column it cannot write");
    }

    /// The refusal this whole change exists for, exercised through the parser
    /// an installed file's answer actually goes through.
    #[test]
    fn a_column_the_installed_file_never_heard_of_is_refused_by_name() {
        let stale = serde_json::json!({
            "ferrostep": "0.1.0",
            "writes": ["state", "counters", "scope"],
            "columns": { "counters": ["attempts"], "scope": ["lane"], "attributes": [] }
        });
        let known = WritableColumns::from_ping(&stale).expect("columns should parse");
        assert_eq!(known.counters, vec!["attempts".to_string()]);
        assert!(known.attributes.is_empty());

        // ⚠ And an older file that states no columns must parse as `None` —
        // "did not say", which is not the same as "writes nothing". Reading
        // silence as refusal would break every deployment installed before
        // this key existed.
        let older = serde_json::json!({ "ferrostep": "0.1.0", "writes": ["state"] });
        assert!(WritableColumns::from_ping(&older).is_none(), "silence read as an answer");
    }

    /// ⚠ Deployment maps written before this category existed must keep
    /// loading. Generated files outlive the binary that made them, and so do
    /// their configs — a map that fails to parse after an upgrade takes the
    /// whole lane down.
    #[test]
    fn a_map_written_before_attributes_existed_still_loads() {
        let json = r#"{"records":"tickets","events":"ticket_events",
            "state_field":"stage","version_field":"fs_version",
            "counter_fields":["attempts"],"scope_fields":["lane"],
            "guard_refereed_fields":true}"#;
        let map: CollectionMap = serde_json::from_str(json).expect("old map must still parse");
        assert!(map.attribute_fields.is_empty());
        assert_eq!(
            map.refereed_fields(),
            vec!["stage", "fs_version", "attempts", "lane"],
            "an old map gained a refereed column it never declared"
        );
    }

    /// ⚠⚠ **A KNOWN-ANSWER TEST, AND THE SECOND COPY OF THESE NAMES IS THE
    /// POINT.** `refereed_fields`'s doc comment says one derivation and no
    /// second copy — that rule is about *shipping* code, and this is a
    /// fixture. Every other test of this function asks whether the derivation
    /// AGREES WITH ITSELF: the hunting list and the guard's list are both read
    /// out of `refereed_fields()`, so a change that drops a whole category
    /// changes both together and every containment check still passes.
    /// Measured 2026-08-27 on this repo, not reasoned: delete
    /// `.chain(self.scope_fields.iter())` and the cross-crate agreement test
    /// in `ferrostep-cli` goes **green** as soon as the fixture carries a
    /// second counter — the guard silently stops closing scope columns,
    /// `explain` silently stops listing them, and the two agree perfectly
    /// about the wrong answer.
    ///
    /// So this asserts the ANSWER, not the agreement, against an input whose
    /// answer cannot drift: one field of each kind, named. **Order is part of
    /// the contract** — it is the order the generated hook lists and the order
    /// `explain` prints, and a reader diffing the two reads them in sequence.
    ///
    /// ⚠ If you add a category to `CollectionMap`, this test fails, and the
    /// fix is to add the field here **after** checking the guard closes it.
    /// Do not delete the assertion to make it pass.
    #[test]
    fn refereed_fields_is_one_field_of_every_kind_in_hook_order() {
        assert_eq!(
            tickets_map().refereed_fields(),
            vec![
                "stage".to_string(),      // state_field
                "fs_version".to_string(), // version_field
                "attempts".to_string(),   // counter_fields
                "lane".to_string(),       // scope_fields
                // ⚠ Added 2026-08-27 when `attribute_fields` was introduced,
                // and added AFTER checking the guard closes it and the apply
                // route can write it — which is the order this test's own
                // docstring asks for, because a category that is guarded with
                // no write path is a documented, unreachable operation.
                "severity".to_string(),   // attribute_fields
            ],
            "the refereed set changed; a dropped category disarms the guard and the sweep \
             together, and they will keep agreeing while they do it"
        );
    }

    /// Every `routerAdd(...)` block in a generated file, each cut at **its
    /// own** terminator rather than at the start of the next one — so the last
    /// block cannot absorb the record hooks that follow it, which do write and
    /// would make any "this route does not write" assertion below fail on
    /// correct output.
    ///
    /// A route's close is a `}` in the first column followed by `)` (`});`) or
    /// by `,` (`}, $apis.requireAuth());`). Everything nested inside a handler
    /// is indented, so column zero is the route's own brace.
    fn route_blocks(hooks: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = hooks;
        while let Some(start) = rest.find("routerAdd(") {
            let tail = &rest[start..];
            let end = tail
                .match_indices("\n}")
                .find(|(i, _)| {
                    let after = &tail[i + 2..];
                    after.starts_with(')') || after.starts_with(',')
                })
                // ⚠ To the END OF THAT LINE, not to the brace: the
                // terminator is `}, $apis.requireAuth());`, so cutting at the
                // brace drops the middleware and every route reads as
                // anonymous. Caught by the guard below going red on correct
                // output — which is the guard doing its job on its own tool.
                .map(|(i, _)| {
                    let after = i + 2;
                    tail[after..].find('\n').map(|n| after + n).unwrap_or(tail.len())
                })
                .unwrap_or(tail.len());
            out.push(&tail[..end]);
            rest = &tail[end..];
        }
        out
    }

    /// Every route that writes: the population the assertions about role
    /// binding and transactions are actually about.
    fn write_routes(hooks: &str) -> Vec<&str> {
        route_blocks(hooks)
            .into_iter()
            .filter(|block| block.starts_with(r#"routerAdd("POST""#))
            .collect()
    }

    /// One `routerAdd` block, from the call that names `path` up to the next
    /// `routerAdd` or the end of the file.
    ///
    /// ⚠ **The anchor is asserted unique before it is used.** A guard that
    /// searches for its subject takes the first match for the only match —
    /// usually the file's own comment about the subject — and then survives
    /// the subject being deleted. That shape has already been found in this
    /// repo's own checks three times.
    fn route_block<'a>(hooks: &'a str, path: &str) -> &'a str {
        // ⚠ The METHOD is not part of the anchor. It used to be, hardcoded to
        // GET, so asking for a POST route sliced nothing and the uniqueness
        // assertion below reported "0 occurrences" — which is the assertion
        // doing its job, and the reason it is worth having.
        let anchor = format!("\"{path}\"");
        assert_eq!(
            hooks.matches(&anchor).count(),
            1,
            "route anchor {anchor:?} must appear exactly once to be sliced on"
        );
        // ⚠ One splitter, deliberately. This used to cut to the next
        // `routerAdd(` while `route_blocks` cut at each route's own
        // terminator, and the two disagreed about whether the middleware was
        // part of the block — so one of them saw an authenticated route where
        // the other saw an anonymous one.
        route_blocks(hooks)
            .into_iter()
            .find(|block| block.contains(&anchor))
            .expect("the block naming a unique path")
    }

    /// ⚠ **The generated file is READ AS A DIFF before it is installed**, so
    /// every line of noise in it is a line an operator has to rule out before
    /// they can approve the real change. A map that declares no columns of some
    /// kind used to leave a blank line where that kind's branches would have
    /// been — cosmetic, and it cost the first adopter a line of a three-change
    /// diff on a shared service.
    #[test]
    fn a_kind_the_map_declares_nothing_for_leaves_no_gap_in_the_generated_file() {
        let mut map = tickets_map();
        map.attribute_fields.clear();
        let hooks = hooks_file_mapped(&map, None, &ActorBinding::default());

        let apply = route_block(&hooks, "/api/ferrostep/tickets/apply");
        let body = apply
            .split_once(r#"rec.set("stage", String(body.state));"#)
            .expect("the apply route sets the state column")
            .1;
        let sets = body
            .split_once(r#"rec.set("fs_version""#)
            .expect("and then the version column")
            .0;
        assert!(
            !sets.contains("\n\n"),
            "an undeclared kind left a blank line in the column writes: {sets:?}"
        );

        // ⚠ Floor: the assertion above passes on an empty region, so prove the
        // region is the one that carries the writes.
        assert!(sets.contains(r#"body.counters["attempts"]"#), "{sets}");
        assert!(sets.contains(r#"body.scope["lane"]"#), "{sets}");
        assert!(!sets.contains("severity"), "the fixture cleared attributes: {sets}");
    }

    /// ⚠⚠ **THE ONE ROUTE WHOSE ANSWER MUST NOT BE A CONSTANT.** Every other
    /// generated route can honestly answer from what was known when it was
    /// written; this one exists to report what the collection accepts *now*,
    /// because the collection is the half that changes without anybody
    /// regenerating a file. Baking the value in at generation time would
    /// produce a check that always agrees with the map it was generated from —
    /// an agreement test wearing a store's clothes.
    #[test]
    fn the_schema_route_reads_the_collection_instead_of_reciting_generation_time_values() {
        let hooks = hooks_file_mapped(&tickets_map(), None, &ActorBinding::default());
        let block = route_block(&hooks, "/api/ferrostep/tickets/schema");

        assert!(
            block.contains("findCollectionByNameOrId(\"tickets\")"),
            "the route must ask the store: {block}"
        );
        // The map's own state values are not in the definition, so the strong
        // statement available here is that the accepted values are read off a
        // field rather than written into the file.
        assert!(block.contains("fields[i].values"), "the values come from the field: {block}");
        assert!(
            block.contains("fields[i].type === \"select\""),
            "and only from a column that actually constrains: {block}"
        );
    }

    /// ⚠ **The ping is anonymous and this route is not, and that difference is
    /// a decision rather than an accident.** Liveness and a file's own
    /// abilities are not worth protecting; a collection's field names and the
    /// accepted values of its select columns are a different disclosure, and
    /// widening the anonymous route to carry them would have been the easy
    /// way to build this.
    #[test]
    fn the_schema_route_is_authenticated_where_the_ping_deliberately_is_not() {
        let hooks = hooks_file_mapped(&tickets_map(), None, &ActorBinding::default());

        let schema = route_block(&hooks, "/api/ferrostep/tickets/schema");
        assert!(schema.contains("$apis.requireAuth()"), "schema must require a caller: {schema}");

        let ping = route_block(&hooks, "/api/ferrostep/tickets/ping");
        assert!(
            !ping.contains("requireAuth"),
            "the ping stays anonymous — an adapter reads it before it has done anything: {ping}"
        );
        // ⚠ Floor: both assertions above are satisfied by a file with no
        // routes at all, so prove the blocks are the routes they claim to be.
        assert!(ping.contains("\"writes\""), "{ping}");
        assert!(schema.contains("\"columns\""), "{schema}");
    }

    /// The generic file carries the route too, so a generic deployment gets
    /// "checked, and this shape constrains nothing" rather than "could not
    /// check" — different results, and only one of them lets a reader stop.
    #[test]
    fn the_generic_file_carries_the_route_as_well() {
        let hooks = hooks_file(&ActorBinding::default());
        let block = route_block(&hooks, "/api/ferrostep/ferrostep_records/schema");
        assert!(block.contains("findCollectionByNameOrId(\"ferrostep_records\")"), "{block}");
        assert!(block.contains("$apis.requireAuth()"), "{block}");
    }

    fn schema_route(body: &str) -> (&'static str, u16, String) {
        ("/api/ferrostep/tickets/schema", 200, body.to_string())
    }

    fn tickets_ping() -> (&'static str, u16, String) {
        (
            "/api/ferrostep/tickets/ping",
            200,
            r#"{"ferrostep":"test","writes":["state","counters","scope","attributes"],
                "columns":{"counters":["attempts"],"scope":["lane"],"attributes":["severity"]}}"#
                .to_string(),
        )
    }

    /// ⚠⚠ **AN INSTALLED FILE TOO OLD TO ANSWER MUST NOT READ AS AGREEMENT.**
    /// The generated files outlive the binary that wrote them, so a deployment
    /// installed before this route existed is the ordinary case rather than
    /// the exotic one — and the natural spelling of a missing route is an
    /// empty schema, which a checker renders as nothing to complain about.
    #[test]
    fn an_installed_file_that_predates_the_route_refuses_and_says_what_to_do() {
        let base = serve(vec![tickets_ping()], 2);
        let ledger = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map()).unwrap();
        let err = ledger.store_shape().unwrap_err();
        assert!(matches!(err, LedgerError::Unsupported(_)), "{err:?}");
        let said = err.to_string();
        assert!(said.contains("predate"), "{said}");
        assert!(said.contains("regenerate"), "the refusal must say what to do: {said}");
        assert!(said.contains("nothing about the store was checked"), "{said}");
    }

    /// ⚠⚠ **THE THREE ANSWERS, OFF THE WIRE.** An array is a column stating
    /// its values; `null` is the route reporting that the column constrains
    /// nothing; a missing key is a file too old to have been asked. The middle
    /// one is a verified all-clear and the last one is not, and they are one
    /// character apart in the JSON.
    #[test]
    fn a_null_state_list_is_an_answer_and_a_missing_one_is_not() {
        let constrains = r#"{"ferrostep":"t","collection":"tickets","columns":{"stage":"select"},
                             "states":["open","closed"]}"#;
        let base = serve(vec![tickets_ping(), schema_route(constrains)], 2);
        let shape = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map())
            .unwrap()
            .store_shape()
            .unwrap();
        assert_eq!(
            shape.accepted_states,
            Answer::Said(vec!["open".to_string(), "closed".to_string()])
        );

        let unconstrained =
            r#"{"ferrostep":"t","collection":"tickets","columns":{"stage":"text"},"states":null}"#;
        let base = serve(vec![tickets_ping(), schema_route(unconstrained)], 2);
        let shape = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map())
            .unwrap()
            .store_shape()
            .unwrap();
        assert_eq!(shape.accepted_states, Answer::NothingToConstrain);
        assert!(!shape.accepted_states.is_unknown(), "null is an answer");

        let silent = r#"{"ferrostep":"t","collection":"tickets","columns":{"stage":"text"}}"#;
        let base = serve(vec![tickets_ping(), schema_route(silent)], 2);
        let shape = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map())
            .unwrap()
            .store_shape()
            .unwrap();
        assert!(shape.accepted_states.is_unknown(), "a missing key is not an answer");
    }

    /// A collection the map names and the store does not have comes back
    /// inside a 200, because the alternatives are worse: a 404 cannot be told
    /// from the route being absent, and a 500 cannot be told from the store
    /// being down. It still has to reach the caller as a refusal.
    #[test]
    fn a_collection_the_store_does_not_have_is_a_refusal_not_an_empty_schema() {
        let missing = r#"{"ferrostep":"t","collection":"tickets","error":"GoError: sql: no rows in result set"}"#;
        let base = serve(vec![tickets_ping(), schema_route(missing)], 2);
        let err = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map())
            .unwrap()
            .store_shape()
            .unwrap_err();
        assert!(matches!(err, LedgerError::Unsupported(_)), "{err:?}");
        assert!(err.to_string().contains("no rows"), "it carries the store's words: {err}");
        assert!(err.to_string().contains("does not exist under that name"), "{err}");
    }

    /// ⚠ The generic shape stores counters and scope as JSON on the row, so
    /// there is no per-name column list that could be older than the mapping.
    /// That is *nothing to constrain*, not *unknown* — reporting it as unknown
    /// would send an operator hunting for a file to regenerate that could not
    /// help them.
    #[test]
    fn the_generic_shape_has_no_installed_column_list_that_could_go_stale() {
        let schema = (
            "/api/ferrostep/ferrostep_records/schema",
            200,
            r#"{"ferrostep":"t","collection":"ferrostep_records",
                "columns":{"state":"text","counters":"json"},"states":null}"#
                .to_string(),
        );
        let base = serve(vec![ping_with_scope(), schema], 2);
        let shape = PocketBaseLedger::connect(&base, "tok").unwrap().store_shape().unwrap();
        assert_eq!(shape.writable, Answer::NothingToConstrain);
        assert!(!shape.writable.is_unknown());
    }

    /// A mapped deployment whose ping states its columns hands them through as
    /// the installed write path's limit — the half of the answer that is fixed
    /// at generation time, carried beside the half that is read live.
    #[test]
    fn a_mapped_shape_carries_what_the_installed_file_said_it_can_write() {
        let body = r#"{"ferrostep":"t","collection":"tickets",
                       "columns":{"stage":"select","attempts":"number"},"states":["open"]}"#;
        let base = serve(vec![tickets_ping(), schema_route(body)], 2);
        let shape = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map())
            .unwrap()
            .store_shape()
            .unwrap();
        let writable = shape.writable.said().expect("the ping stated its columns");
        assert_eq!(writable.get("counters"), Some(&vec!["attempts".to_string()]));
        assert_eq!(writable.get("attributes"), Some(&vec!["severity".to_string()]));
        // And the live half is the collection's, not the ping's.
        let columns = shape.columns.said().expect("the route enumerated the columns");
        assert_eq!(columns.get("stage"), Some(&"select".to_string()));
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
        Decision::allow(to, BTreeMap::new())
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
                    "/api/ferrostep/ferrostep_records/apply",
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
                ("/api/ferrostep/ferrostep_records/apply", 404, r#"{"message":"No_record: abc123."}"#.to_string()),
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
    fn a_mapped_collection_reads_its_own_columns_as_the_record() {
        let row = r#"{"id":"t1","stage":"open","attempts":2,"lane":"alpha","fs_version":7,
                      "title":"unrelated content stays unread"}"#;
        let page = format!(
            r#"{{"page":1,"perPage":500,"totalItems":1,"totalPages":1,"items":[{row}]}}"#
        );
        let base = serve(
            vec![
                ("/api/ferrostep/tickets/ping", 200, r#"{"ferrostep":"test"}"#.to_string()),
                ("/api/collections/tickets/records/t1", 200, row.to_string()),
                ("/api/collections/tickets/records", 200, page),
            ],
            4,
        );
        let ledger = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map()).unwrap();
        assert_eq!(ledger.mode(), Mode::Full);
        let record = ledger.load(&RecordId("t1".to_string())).unwrap();
        assert_eq!(record.snapshot.state, "open");
        assert_eq!(record.snapshot.counters["attempts"], 2);
        assert_eq!(record.version.0, "7");
        let in_lane = ledger
            .select(&Scope::all().with("lane", "alpha"), &["open".to_string()])
            .unwrap();
        assert_eq!(in_lane.len(), 1);
        let elsewhere = ledger
            .select(&Scope::all().with("lane", "beta"), &["open".to_string()])
            .unwrap();
        assert!(elsewhere.is_empty());
    }

    #[test]
    fn a_row_predating_the_mapping_reads_version_zero_not_an_error() {
        // The mapped version column defaults to 0 on old rows; 0 is a valid
        // starting token, so no backfill pass is required before cutover.
        let row = r#"{"id":"t9","stage":"open","lane":"alpha"}"#;
        let base = serve(
            vec![
                ("/api/ferrostep/tickets/ping", 200, r#"{"ferrostep":"test"}"#.to_string()),
                ("/api/collections/tickets/records/t9", 200, row.to_string()),
            ],
            2,
        );
        let ledger = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map()).unwrap();
        let record = ledger.load(&RecordId("t9".to_string())).unwrap();
        assert_eq!(record.version.0, "0");
        assert_eq!(record.snapshot.counters["attempts"], 0, "absent counter reads as never spent");
    }

    #[test]
    fn a_mapped_collection_refuses_filing_by_name() {
        let base = serve(
            vec![("/api/ferrostep/tickets/ping", 200, r#"{"ferrostep":"test"}"#.to_string())],
            1,
        );
        let ledger = PocketBaseLedger::connect_mapped(&base, "tok", tickets_map()).unwrap();
        let refused = ledger.create(&Scope::all(), &allow("open"), &an_event(allow("open")));
        match refused {
            Err(LedgerError::Unsupported(what)) => {
                assert!(what.contains("own filing procedure"), "{what}")
            }
            other => panic!("mapped filing must be refused by name, got {other:?}"),
        }
    }

    #[test]
    fn the_generated_files_carry_their_load_bearing_shapes() {
        let hooks = hooks_file(&ActorBinding::default());
        // Both write routes are transactional, authenticated, and the ping
        // answers what connect() probes for — all scoped to the collection.
        // ⚠ Counted over WRITE routes rather than over authenticated ones: a
        // read-only authenticated route is legitimate (`schema` is one), and a
        // count that cannot tell the two apart goes red on correct output.
        let writes = write_routes(&hooks).len();
        assert_eq!(writes, 2, "the generic file's write routes");
        assert_eq!(hooks.matches("runInTransaction").count(), writes);
        assert!(hooks.matches("$apis.requireAuth()").count() >= writes);
        assert!(hooks.contains(r#"routerAdd("GET", "/api/ferrostep/ferrostep_records/ping""#));
        assert!(hooks.contains("cas_conflict"));
        let migration = migration_file(&ActorBinding::default());
        assert!(migration.contains("CREATE UNIQUE INDEX"), "the (record, seq) referee");
        // The empty-string trap: "" means PUBLIC. Every rule is either null
        // or a real expression.
        assert!(!migration.contains(r#"Rule": """#), "an empty-string rule is public");
        assert!(migration.contains(r#""createRule": null"#));
        let events = serde_json::to_string(&events_collection_body("ticket_events")).unwrap();
        assert!(events.contains("idx_ticket_events_record_seq"));
        assert!(!events.contains(r#"Rule":"""#), "an empty-string rule is public");
    }

    #[test]
    fn the_mapped_migration_guards_both_of_its_changes() {
        let migration = migration_file_mapped(&tickets_map(), &ActorBinding::default());
        // Re-running must be harmless: the field add and the collection
        // create are both conditional.
        assert!(migration.contains(r#"if (!records.fields.getByName("fs_version"))"#));
        assert!(migration.contains("haveEvents"));
        assert!(migration.contains("idx_ticket_events_record_seq"), "the (record, seq) referee");
        assert!(!migration.contains(r#"Rule": """#), "an empty-string rule is public");
        assert!(migration.contains(r#"records.fields.removeByName("fs_version")"#), "a down path");
    }

    /// ⚠⚠ **The engine is consulted, not in the write path** — so a client
    /// with credentials can edit `state` straight on the row and the referee
    /// never hears: no version bump, no event, and every later
    /// compare-and-swap arguing about a number that moved behind it. A
    /// deployment can now close that, and the closing has to be a **hook**
    /// rather than an access rule, because an administrator bypasses rules
    /// and does not bypass hooks.
    #[test]
    fn the_refereed_columns_can_be_closed_to_direct_writes() {
        let open = hooks_file_mapped(&tickets_map(), None, &ActorBinding::default());
        assert!(!open.contains("refereed_field"), "off unless asked for: {open}");

        let guarded = hooks_file_mapped(&guarded_map(), None, &ActorBinding::default());
        assert!(guarded.contains("refereed_field"), "{guarded}");
        // ⚠ Every column the map declares refereed, derived from the map
        // rather than listed here — a counter or scope label added later is
        // guarded because it is in the map, not because anyone remembered.
        for field in ["stage", "fs_version", "attempts", "lane"] {
            assert!(
                guarded.contains(&format!("\"{field}\"")),
                "{field} is refereed and unguarded: {guarded}"
            );
        }
        // The refusal names the way through, not just the refusal.
        assert!(guarded.contains("/api/ferrostep/tickets/apply"), "{guarded}");
    }

    /// ⚠ Handlers chain through `e.next()`, so the guard must be registered
    /// BEFORE the release hook: it sees the client's own change and passes
    /// control on, and the release's writes happen downstream of it.
    /// Reversed, the guard would refuse the release it exists to permit.
    #[test]
    fn the_guard_is_registered_ahead_of_the_release_it_must_not_refuse() {
        let release = ReleaseHook {
            decision_field: "verdict".to_string(),
            from_state: "stalled".to_string(),
            to_state: "queued".to_string(),
            reset_counters: vec!["attempts".to_string()],
            writers: vec!["a-person@example.invalid".to_string()],
            role: "owner".to_string(),
        };
        let hooks = hooks_file_mapped(&guarded_map(), Some(&release), &ActorBinding::default());
        let guard = hooks.find("refereed_field").expect("the guard is emitted");
        let release_at = hooks.find("verdict is the owner's field").expect("the release is emitted");
        assert!(guard < release_at, "the guard must be registered first, or it refuses the release");
        assert_eq!(hooks.matches("onRecordUpdateRequest").count(), 2, "two chained handlers");
    }

    /// ⚠⚠ **The route authenticated, and then believed the request about who
    /// was asking.** `ev.set("role", body.event.role)` lets any authenticated
    /// caller act as any role — invisible while every actor shares one
    /// credential, and the entire point once they do not.
    #[test]
    fn a_write_route_takes_the_acting_role_from_the_account_not_the_request() {
        let binding = ActorBinding::default();
        for (shape, hooks) in [
            ("generic", hooks_file(&binding)),
            ("mapped", hooks_file_mapped(&tickets_map(), None, &binding)),
        ] {
            assert!(
                !hooks.contains(r#"ev.set("role", String((body.event"#),
                "{shape}: the event still records the role the REQUEST claimed"
            );
            assert!(hooks.contains(r#"ev.set("role", actingRole)"#), "{shape}");
            assert!(
                hooks.contains("role_not_yours"),
                "{shape}: a claim that contradicts the account must be refused by name"
            );
            // ⚠ Derived, not stated: a route that writes must carry the
            // binding. A write route added later without it is the failure
            // this checks rather than trusts anyone to remember.
            //
            // ⚠⚠ **THIS USED TO COUNT AUTHENTICATED ROUTES**, which was exact
            // only while every authenticated route was a write route. The
            // first authenticated READ route — `schema`, which needs a caller
            // but binds no role because it cannot write — made the count fail
            // on correct output. The population was the proxy, not the
            // property, so the property is checked directly now, in both
            // directions.
            let writes = write_routes(&hooks);
            assert!(!writes.is_empty(), "{shape}: no write routes were enumerated");
            for route in &writes {
                assert!(
                    route.contains("$apis.requireAuth()"),
                    "{shape}: a write route is anonymous: {route}"
                );
                // ⚠ `const boundRole =`, with the assignment. Matching the
                // bare name passes on `const boundRole2`, which declares a
                // variable nothing reads and leaves the route trusting the
                // request — measured by mutation, and true of the check this
                // replaced as well.
                assert!(
                    route.contains("const boundRole ="),
                    "{shape}: a write route does not bind a role: {route}"
                );
            }
            // The converse, which is what the old count was really defending:
            // a route that binds no role must not be able to write one.
            let unbound: Vec<&str> = route_blocks(&hooks)
                .into_iter()
                .filter(|b| !b.contains("const boundRole ="))
                .collect();
            // ⚠ Floor: the loop below is vacuous unless such a route exists,
            // and a vacuous loop is a passing test that checks nothing.
            assert!(!unbound.is_empty(), "{shape}: no unbound routes to check");
            for route in unbound {
                assert!(!route.contains(".save("), "{shape}: an unbound route saves: {route}");
                assert!(
                    !route.contains("rec.set("),
                    "{shape}: an unbound route sets a column: {route}"
                );
            }
        }
    }

    /// The transition this default exists for: a deployment with no actors
    /// yet authenticates as an administrator, so refusing unbound principals
    /// on install would break every write the moment the hooks landed.
    #[test]
    fn a_deployment_can_require_every_actor_to_be_bound_but_is_not_forced_to() {
        let strict = ActorBinding { allow_unbound: false, ..Default::default() };
        assert!(hooks_file(&strict).contains("unbound_principal"), "strict must refuse");
        assert!(
            !hooks_file(&ActorBinding::default()).contains("unbound_principal"),
            "the default must not refuse, or installing these hooks is an outage"
        );
    }

    /// ⚠ **Bind, don't mint.** The collection is named by configuration and
    /// created only when absent, so a deployment with an auth collection of
    /// its own — including one federated to a directory — points at that
    /// instead of gaining a second place identities live.
    #[test]
    fn the_actor_collection_is_configured_and_the_route_reads_what_the_migration_wrote() {
        let theirs = ActorBinding {
            collection: "staff".to_string(),
            role_field: "job".to_string(),
            allow_unbound: true,
        };
        let migration = migration_file(&theirs);
        assert!(migration.contains(r#"findCollectionByNameOrId("staff")"#), "{migration}");
        assert!(migration.contains("haveActors"), "created only when absent: {migration}");
        assert!(
            migration.contains(r#"{ "name": "job", "type": "text", "required": true }"#),
            "{migration}"
        );
        // ⚠ The drift that would be silent: the route reading one field name
        // while the migration created another. Both come from one value, and
        // this is what says so.
        assert!(
            hooks_file(&theirs).contains(r#"e.auth.getString("job")"#),
            "the route reads a different field than the migration wrote"
        );
        // Superuser-only, for a sharper version of the events collection's
        // reason: this is the list of who may act as what.
        assert!(migration.contains(r#""listRule": null"#), "{migration}");
    }

    /// ⚠⚠ **A generated history must never be more readable than the records
    /// it describes.** The mapped shape attaches to a collection somebody
    /// else made, under rules this migration cannot read the meaning of — so
    /// the only read rule that is right for every adopter is the strict one.
    /// It shipped with an authenticated-user rule instead, which matched in
    /// the generic case that gets tested and inverted the mapped one: every
    /// state change, actor, role and note about records the reader may not
    /// open.
    ///
    /// The two shapes differ deliberately, and the test says why rather than
    /// pinning two constants: the generic migration creates BOTH collections,
    /// so they match by construction and neither can outrank the other.
    #[test]
    fn a_mapped_history_never_outranks_the_records_it_describes() {
        let mapped = migration_file_mapped(&tickets_map(), &ActorBinding::default());
        // The events collection is the second block; the records collection
        // in this shape is not created at all, only altered.
        assert!(
            !mapped.contains(r#""listRule": "@request.auth.id"#),
            "the mapped events collection must not assume a read rule: {mapped}"
        );
        assert!(mapped.contains(r#""listRule": null"#), "{mapped}");
        assert!(mapped.contains(r#""viewRule": null"#), "{mapped}");

        // Same requirement, same reason, for the helper that builds the
        // collection outside a migration — it is handed a name and nothing
        // else, so it can know even less about what it sits beside.
        let body = events_collection_body("ticket_events");
        assert_eq!(body["listRule"], Value::Null, "{body}");
        assert_eq!(body["viewRule"], Value::Null, "{body}");

        // ⚠ The generic shape keeps its authenticated read, and that is not
        // an exception to the rule above — it creates the records collection
        // too, with the same rule, so the invariant holds by construction.
        let generic = migration_file(&ActorBinding::default());
        assert_eq!(
            generic.matches(r#""listRule": "@request.auth.id != ''""#).count(),
            2,
            "records and events must carry the SAME rule, or one outranks the other"
        );
    }

    #[test]
    fn the_mapped_hooks_write_the_mapped_columns_and_only_those() {
        let hooks = hooks_file_mapped(&tickets_map(), None, &ActorBinding::default());
        assert!(hooks.contains(r#"routerAdd("GET", "/api/ferrostep/tickets/ping""#));
        assert!(hooks.contains(r#"routerAdd("POST", "/api/ferrostep/tickets/apply""#));
        assert!(hooks.contains(r#"rec.getInt("fs_version")"#), "compare on the mapped token");
        assert!(hooks.contains(r#"rec.set("stage", String(body.state))"#));
        assert!(hooks.contains(r#"body.counters["attempts"]"#));
        assert!(!hooks.contains(r#"rec.set("counters""#), "no generic json write in mapped mode");
        assert!(!hooks.contains("/create"), "filing stays with the collection's own procedure");
        assert!(!hooks.contains("onRecordUpdateRequest"), "no release hook unless asked for");
    }

    /// ⚠⚠ Hooks are deployed separately from the binary, so a NEW adapter
    /// routinely meets OLD routes. An apply carrying scope updates against
    /// routes that ignore them answers 200 with a fresh version — so the
    /// caller reports a record moved between units of work while its label
    /// never changed, and every query keeps finding it where it was. Refusing
    /// by name is the only honest answer, and it names the remedy.
    #[test]
    fn a_rescope_against_hooks_that_predate_it_is_refused_rather_than_lost() {
        let base = serve(vec![ping_full()], 1);
        let ledger = PocketBaseLedger::connect(&base, "token").unwrap();
        assert_eq!(ledger.mode(), Mode::Full, "the routes are installed, just older");

        let record = Record {
            id: RecordId("r1".to_string()),
            snapshot: Snapshot { state: "open".to_string(), counters: BTreeMap::new() },
            version: Version("1".to_string()),
        };
        let moving = Decision::Allow {
            to: "open".to_string(),
            counter_updates: BTreeMap::new(),
            scope_updates: BTreeMap::from([("branch".to_string(), "follow-up".to_string())]),
        };
        let Err(refused) = ledger.apply(&record, &an_event(moving)) else {
            panic!("a rescope was sent to routes that cannot write it");
        };
        let message = refused.to_string();
        assert!(message.contains("scope labels"), "{message}");
        assert!(message.contains("regenerate"), "{message}");
        // ⚠ The refusal must be local. Reaching the network first would spend
        // the record's version on a write that did nothing.
        assert!(
            matches!(refused, LedgerError::Unsupported(_)),
            "expected a refusal by name, got {refused:?}"
        );
    }

    /// The other half: the refusal must not fire where the routes can do it,
    /// or rescope is unreachable everywhere and the test above passes for the
    /// wrong reason.
    #[test]
    fn a_rescope_is_sent_where_the_installed_hooks_say_they_write_scope() {
        let base = serve(
            vec![
                ping_with_scope(),
                ("/api/ferrostep/ferrostep_records/apply", 200, r#"{"version":2}"#.to_string()),
            ],
            2,
        );
        let ledger = PocketBaseLedger::connect(&base, "token").unwrap();
        let record = Record {
            id: RecordId("r1".to_string()),
            snapshot: Snapshot { state: "open".to_string(), counters: BTreeMap::new() },
            version: Version("1".to_string()),
        };
        let moving = Decision::Allow {
            to: "open".to_string(),
            counter_updates: BTreeMap::new(),
            scope_updates: BTreeMap::from([("branch".to_string(), "follow-up".to_string())]),
        };
        assert_eq!(ledger.apply(&record, &an_event(moving)).unwrap(), Version("2".to_string()));
    }

    /// ⚠⚠ The route is authenticated, which is not the same as constrained.
    /// Every actor in the loop holds a token, so "what may this request write"
    /// has to be answered by the generated text rather than by trust: one
    /// `if` per DECLARED label means a request naming any other column has no
    /// line that would write it.
    #[test]
    fn a_mapped_rescope_can_only_write_the_labels_the_map_declares() {
        let hooks = hooks_file_mapped(&tickets_map(), None, &ActorBinding::default());
        assert!(hooks.contains(r#"body.scope["lane"]"#), "the declared label is writable");
        assert!(hooks.contains(r#"rec.set("lane", String(body.scope["lane"]))"#));
        // The shape that would make it general — and therefore unbounded.
        assert!(
            !hooks.contains("Object.keys(body.scope"),
            "mapped mode must never loop over caller-supplied label names"
        );
        for forbidden in ["stage", "fs_version", "attempts", "id"] {
            assert!(
                !hooks.contains(&format!(r#"body.scope["{forbidden}"]"#)),
                "'{forbidden}' is not a declared scope label and must not be writable through scope"
            );
        }
    }

    /// The generic shape keeps its labels in one JSON field, so the same rule
    /// is expressed differently: merge into what is stored rather than
    /// replace it. A record that loses an unnamed label falls out of every
    /// query filtering on it, silently.
    #[test]
    fn the_generic_route_merges_scope_labels_rather_than_replacing_them() {
        let hooks = hooks_file(&ActorBinding::default());
        assert!(hooks.contains(r#"const labels = rec.get("scope") || {}"#));
        assert!(hooks.contains("labels[scopeKeys[i]] = String(scopeIn[scopeKeys[i]])"));
        assert!(
            hooks.contains("if (scopeKeys.length > 0)"),
            "an ordinary move must not touch scope at all"
        );
    }

    #[test]
    fn the_release_hook_is_a_guarded_transition_with_the_bookkeeping_attached() {
        let release = ReleaseHook {
            decision_field: "verdict".to_string(),
            from_state: "parked".to_string(),
            to_state: "open".to_string(),
            reset_counters: vec!["attempts".to_string()],
            writers: vec!["a-person@example.invalid".to_string()],
            role: "owner".to_string(),
        };
        let hooks = hooks_file_mapped(&tickets_map(), Some(&release), &ActorBinding::default());
        assert!(hooks.contains("onRecordUpdateRequest"));
        assert!(hooks.contains("onRecordCreateRequest"), "the refusal holds from the first save");
        assert!(hooks.contains(r#"WRITERS = ["a-person@example.invalid"]"#), "fail-closed allowlist");
        assert!(hooks.contains(r#"e.record.set("attempts", 0)"#), "the reset rides the release");
        assert!(
            hooks.contains(r#"e.record.set("fs_version", e.record.original().getInt("fs_version") + 1)"#),
            "the release bumps the version like any other move"
        );
        assert!(hooks.contains(r#""kind": "allow", "to": "open""#), "the event carries the decision");
        assert!(hooks.contains(r#"ev.set("role", "owner")"#));
    }

    #[test]
    fn install_writes_both_files_where_pocketbase_looks() {
        let dir = std::env::temp_dir().join(format!("ferrostep-pb-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (migration, hooks) = install_files(&dir, &ActorBinding::default()).unwrap();
        assert!(migration.ends_with("pb_migrations/1756000000_ferrostep.js"));
        assert!(hooks.ends_with("pb_hooks/ferrostep.pb.js"));
        assert_eq!(std::fs::read_to_string(&hooks).unwrap(), hooks_file(&ActorBinding::default()));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // ------------------------------------------------------------------
    // Live tests: a real PocketBase with the generated files installed.
    // Ignored by default so an offline run reports them as ignored rather
    // than silently green; run with:
    //   FERROSTEP_POCKETBASE_URL=… FERROSTEP_POCKETBASE_TOKEN=… \
    //     cargo test -p ferrostep-pocketbase -- --ignored
    // The mapped test additionally expects the fixture from
    // `live_mapped_setup` below: a `tickets` collection and the mapped hooks
    // file for it.
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
                            let claim = Decision::allow(
                                "spin",
                                BTreeMap::from([("wins".to_string(), round as u32 + 1)]),
                            );
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

    /// ⚠⚠ **THE ROUTE THAT CANNOT BE VERIFIED BY READING IT.** Every other
    /// assertion about a generated file in this module is about its *text* —
    /// which is the right level for a file that is deployed, not run, here.
    /// This one is different: the route's whole value is that a real JSVM can
    /// enumerate a collection's fields and read a select column's accepted
    /// values, and that is a property of the store, not of the string.
    ///
    /// **Measured against a live instance, 2026-08-27**, before the route
    /// shipped: `JSON.parse(JSON.stringify(collection)).fields` yields
    /// `{name, type}` per field with `values` present on a select, a
    /// collection the file names and the store lacks throws
    /// `GoError: sql: no rows in result set`, and the anonymous caller is
    /// refused 401. The fixture is the one `live_mapped_collection_moves_under_the_referee`
    /// expects, with the CURRENT generated hooks installed — an older file has
    /// no such route and this test will correctly report that instead.
    #[test]
    #[ignore = "needs a live PocketBase with the mapped tickets fixture and CURRENT hooks installed; set FERROSTEP_POCKETBASE_URL and FERROSTEP_POCKETBASE_TOKEN and run with --ignored"]
    fn live_the_schema_route_reads_the_collection_the_definition_will_meet() {
        let url = std::env::var("FERROSTEP_POCKETBASE_URL").unwrap();
        let token = std::env::var("FERROSTEP_POCKETBASE_TOKEN").unwrap();
        let ledger = PocketBaseLedger::connect_mapped(&url, &token, tickets_map()).unwrap();

        let shape = ledger.store_shape().expect("the installed hooks carry the schema route");
        assert_eq!(shape.subject, "tickets");

        let columns = shape.columns.said().expect("the route enumerated the columns");
        for (kind, name) in [
            ("state", &tickets_map().state_field),
            ("version", &tickets_map().version_field),
        ] {
            assert!(columns.contains_key(name), "the {kind} column '{name}' is missing: {columns:?}");
        }

        // ⚠ The accepted-state list is the half that only a live store can
        // answer, and the fixture's `stage` may legitimately be either a
        // select or a text column — so this asserts the ANSWER IS AN ANSWER,
        // never that it is a particular one. `Unknown` is the failure: it
        // means the installed file could not be asked.
        assert!(
            !shape.accepted_states.is_unknown(),
            "the route must say whether the state column constrains its values, got {:?}",
            shape.accepted_states
        );
        if let Some(accepted) = shape.accepted_states.said() {
            assert!(!accepted.is_empty(), "a select column with no values accepts nothing");
        }

        // The installed file's own limits arrive from the ping, beside the
        // collection's — two different ages of truth in one value.
        assert!(!shape.writable.is_unknown(), "a current mapped file states its columns");
    }

    #[test]
    #[ignore = "needs a live PocketBase with the mapped tickets fixture and hooks installed; set FERROSTEP_POCKETBASE_URL and FERROSTEP_POCKETBASE_TOKEN and run with --ignored"]
    fn live_mapped_collection_moves_under_the_referee() {
        // The mapped fixture: a `tickets` collection (stage select-or-text,
        // attempts number, lane text, fs_version number) with the mapped
        // hooks for it installed, and a `ticket_events` collection from
        // events_collection_body. Rows are filed by the collection's own
        // procedure — plain REST here, as superuser.
        let url = std::env::var("FERROSTEP_POCKETBASE_URL").unwrap();
        let token = std::env::var("FERROSTEP_POCKETBASE_TOKEN").unwrap();
        let ledger = PocketBaseLedger::connect_mapped(&url, &token, tickets_map()).unwrap();
        assert_eq!(ledger.mode(), Mode::Full, "the mapped hooks must be installed");

        let client = agent();
        let resp = client
            .post(format!("{url}/api/collections/tickets/records"))
            .header("Authorization", &token)
            .send_json(json!({ "stage": "open", "attempts": 0, "lane": "live", "fs_version": 0 }))
            .unwrap();
        let (status, filed) = read(resp);
        assert_eq!(status, 200, "filing through the collection's own procedure: {filed}");
        let id = RecordId(filed["id"].as_str().unwrap().to_string());

        // A row filed outside the referee starts at version 0 — valid token.
        let record = ledger.load(&id).unwrap();
        assert_eq!(record.version.0, "0");

        // Claim: spend the counter, stage stays open (a self-move).
        let claim = Decision::allow("open", BTreeMap::from([("attempts".to_string(), 1)]));
        let stale = record.clone();
        ledger.apply(&record, &an_event(claim.clone())).unwrap();
        let after = ledger.load(&id).unwrap();
        assert_eq!(after.snapshot.counters["attempts"], 1);
        assert_eq!(after.version.0, "1");

        // The stale copy is refused — same CAS, mapped columns.
        let refused = ledger.apply(&stale, &an_event(claim));
        assert!(matches!(refused, Err(LedgerError::VersionConflict { .. })), "{refused:?}");

        // The history landed beside the mapped row, in its own collection.
        let history = ledger.history(&id).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].seq, 1);
    }
}

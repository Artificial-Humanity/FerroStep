//! ferrostep — the person-facing surface over a refereed ledger.
//!
//! Four subcommands, one set of primitives:
//!
//! * `awaiting` — the decision surface (ROADMAP B2): which records await a
//!   person, and which moves their role has, each annotated with what it
//!   would actually do. Built on one enumeration plus [`Engine::status`] and
//!   [`Engine::next_moves`].
//! * `move` — the resolution: authorize one attempt and persist what the
//!   decision says. This is how a person resolves an escalation without
//!   opening a database console.
//! * `audit` — the report (ROADMAP B4): what happened, per record — moves,
//!   escalations, releases, the last human note — a *reader of the same
//!   enumeration `awaiting` uses*, so the two views cannot disagree about
//!   the ledger.
//! * `notify` — B3's wiring: one notification per awaiting record, through
//!   the notifier adapter. Invoked when the caller decides; nothing here
//!   polls or schedules.
//!
//! The store is named per invocation (`sqlite:<path>` or
//! `pocketbase:<url>`) — an application-layer choice among adapters, which
//! is exactly where naming an adapter belongs.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use ferrostep_core::{Attempt, Decision, Engine, Status, WorkflowDef};
use ferrostep_ledger::{Event, Ledger, Record, RecordId, Scope};
use ferrostep_notify::{Notification, Notifier, Ntfy, Urgency};

const USAGE: &str = "ferrostep — the person-facing surface over a FerroStep-refereed ledger

USAGE:
  ferrostep <awaiting|audit|move|notify> --workflow <def.json> --store <target> [options]

COMMON:
  --workflow <path>     the workflow definition JSON the ledger is refereed by
  --store <target>      sqlite:<path> or pocketbase:<url>
  --token <token>       auth token for pocketbase: stores
                        (or the FERROSTEP_POCKETBASE_TOKEN environment variable)
  --map <path>          collection-mapping JSON for a pocketbase: store that
                        referees an existing collection instead of the
                        generic ferrostep ones
  --scope <key=value>   narrow to records labelled key=value (repeatable)

move:
  --record <id> --role <role> --to <state> [--note <text>] [--actor <name>]

notify:
  --ntfy <server> --topic <topic> [--ntfy-token <token>]
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Flags parsed positionally: every flag takes exactly one value, and a flag
/// may repeat where that means something (`--scope`).
struct Flags(BTreeMap<String, Vec<String>>);

impl Flags {
    fn parse(args: &[String]) -> Result<Flags, String> {
        let mut flags: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let Some(name) = flag.strip_prefix("--") else {
                return Err(format!("unexpected argument '{flag}'\n\n{USAGE}"));
            };
            let value = it.next().ok_or(format!("--{name} needs a value"))?;
            flags.entry(name.to_string()).or_default().push(value.clone());
        }
        Ok(Flags(flags))
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(|v| v.last()).map(String::as_str)
    }

    fn require(&self, name: &str) -> Result<&str, String> {
        self.get(name).ok_or(format!("--{name} is required\n\n{USAGE}"))
    }

    fn all(&self, name: &str) -> &[String] {
        self.0.get(name).map(Vec::as_slice).unwrap_or_default()
    }
}

fn run(args: &[String]) -> Result<String, String> {
    let Some(command) = args.first() else {
        return Err(format!("no subcommand given\n\n{USAGE}"));
    };
    let flags = Flags::parse(&args[1..])?;
    let engine = load_engine(flags.require("workflow")?)?;
    let ledger = open_ledger(flags.require("store")?, flags.get("token"), flags.get("map"))?;
    let mut scope = Scope::all();
    for pair in flags.all("scope") {
        let (key, value) = pair
            .split_once('=')
            .ok_or(format!("--scope takes key=value, got '{pair}'"))?;
        scope = scope.with(key, value);
    }
    match command.as_str() {
        "awaiting" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, false)?;
            Ok(render_awaiting(&engine, &rows))
        }
        "audit" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, true)?;
            render_audit(&engine, ledger.as_ref(), &rows)
        }
        "move" => {
            let role = flags.require("role")?;
            do_move(
                &engine,
                ledger.as_ref(),
                flags.require("record")?,
                role,
                flags.require("to")?,
                flags.get("note"),
                flags.get("actor").unwrap_or(role),
            )
        }
        "notify" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, false)?;
            let mut notifier = Ntfy::new(flags.require("ntfy")?, flags.require("topic")?);
            if let Some(token) = flags.get("ntfy-token") {
                notifier = notifier.with_token(token);
            }
            send_notifications(&engine, &rows, &notifier)
        }
        other => Err(format!("unknown subcommand '{other}'\n\n{USAGE}")),
    }
}

fn load_engine(path: &str) -> Result<Engine, String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read workflow '{path}': {e}"))?;
    let def = WorkflowDef::from_json(&source)
        .map_err(|e| format!("workflow '{path}' does not parse: {e}"))?;
    Engine::new(def).map_err(|e| format!("workflow '{path}' does not validate: {e}"))
}

fn open_ledger(
    store: &str,
    token: Option<&str>,
    map: Option<&str>,
) -> Result<Box<dyn Ledger>, String> {
    if let Some(path) = store.strip_prefix("sqlite:") {
        if map.is_some() {
            return Err("--map applies to pocketbase: stores only".to_string());
        }
        return Ok(Box::new(
            ferrostep_sqlite::SqliteLedger::open(path).map_err(|e| e.to_string())?,
        ));
    }
    if let Some(url) = store.strip_prefix("pocketbase:") {
        let env_token = std::env::var("FERROSTEP_POCKETBASE_TOKEN").ok();
        let token = token
            .map(str::to_string)
            .or(env_token)
            .ok_or("a pocketbase: store needs --token or FERROSTEP_POCKETBASE_TOKEN")?;
        let ledger = match map {
            Some(path) => {
                let source = std::fs::read_to_string(path)
                    .map_err(|e| format!("cannot read map '{path}': {e}"))?;
                let map: ferrostep_pocketbase::CollectionMap = serde_json::from_str(&source)
                    .map_err(|e| format!("map '{path}' does not parse: {e}"))?;
                ferrostep_pocketbase::PocketBaseLedger::connect_mapped(url, &token, map)
            }
            None => ferrostep_pocketbase::PocketBaseLedger::connect(url, &token),
        }
        .map_err(|e| e.to_string())?;
        return Ok(Box::new(ledger));
    }
    Err(format!("--store must be sqlite:<path> or pocketbase:<url>, got '{store}'"))
}

/// The one enumeration both views read: every record in scope, in every
/// non-terminal state — and, for the audit, the endings too — paired with
/// what can happen to it next.
fn records_with_status(
    engine: &Engine,
    ledger: &dyn Ledger,
    scope: &Scope,
    include_ended: bool,
) -> Result<Vec<(Record, Status)>, String> {
    let def = engine.def();
    let states: Vec<String> = def
        .states
        .iter()
        .filter(|s| include_ended || !def.terminal.contains(s))
        .cloned()
        .collect();
    let records = ledger.select(scope, &states).map_err(|e| e.to_string())?;
    Ok(records
        .into_iter()
        .map(|record| {
            let status = engine.status(&record.snapshot);
            (record, status)
        })
        .collect())
}

fn waiting_reason(engine: &Engine, record: &Record, status: &Status) -> &'static str {
    match status {
        Status::WillEscalate => "every remaining move would escalate",
        Status::NeedsPerson if engine.def().halted.contains(&record.snapshot.state) => {
            "paused; a person decides what happens next"
        }
        Status::NeedsPerson => "only a person can act",
        _ => "does not await a person",
    }
}

fn render_awaiting(engine: &Engine, rows: &[(Record, Status)]) -> String {
    let def = engine.def();
    let waiting: Vec<&(Record, Status)> = rows
        .iter()
        .filter(|(_, s)| matches!(s, Status::NeedsPerson | Status::WillEscalate))
        .collect();
    let mut out = String::new();
    // The population is named so an empty answer reads as "checked and none"
    // rather than "checked nothing".
    let _ = writeln!(
        out,
        "{} of {} open record(s) await a person in '{}'",
        waiting.len(),
        rows.len(),
        def.name
    );
    for (record, status) in waiting {
        let _ = writeln!(
            out,
            "\n  record {} — {} ({})",
            record.id.0,
            record.snapshot.state,
            waiting_reason(engine, record, status)
        );
        for counter in &def.counters {
            let held = record.snapshot.counters.get(&counter.name).copied().unwrap_or(0);
            let _ = writeln!(out, "    {}: {held} of {} spent", counter.name, counter.max);
        }
        for role in def.human_roles() {
            let moves = engine.next_moves(&record.snapshot, role);
            if moves.is_empty() {
                continue;
            }
            let _ = writeln!(out, "    {role} may:");
            for (transition, decision) in moves {
                let mut line = format!("      -> {}", transition.to);
                if !transition.resets.is_empty() {
                    let _ = write!(line, " (resets {})", transition.resets.join(", "));
                }
                if transition.requires_note {
                    let _ = write!(line, " [note required]");
                }
                if let Decision::Exhausted { to, counter } = &decision {
                    let _ = write!(line, " — would route to '{to}' instead: '{counter}' is spent");
                }
                let _ = writeln!(out, "{line}");
            }
        }
    }
    out
}

fn render_audit(
    engine: &Engine,
    ledger: &dyn Ledger,
    rows: &[(Record, Status)],
) -> Result<String, String> {
    let def = engine.def();
    let mut out = String::new();
    let _ = writeln!(out, "'{}' audit — {} record(s) in scope", def.name, rows.len());

    let mut by_state: BTreeMap<&str, usize> = BTreeMap::new();
    for (record, _) in rows {
        *by_state.entry(record.snapshot.state.as_str()).or_default() += 1;
    }
    let states = by_state
        .iter()
        .map(|(state, n)| format!("{state} {n}"))
        .collect::<Vec<_>>()
        .join(" · ");
    let _ = writeln!(out, "  by state: {states}");

    for (record, status) in rows {
        let history = ledger.history(&record.id).map_err(|e| e.to_string())?;
        // An escalation is any arrival in a halted state, by whichever door:
        // a spent ceiling routing there, or an actor sending it there.
        let escalations = history
            .iter()
            .filter(|e| match &e.event.decision {
                Decision::Exhausted { .. } => true,
                Decision::Allow { to, .. } => def.halted.contains(to),
                Decision::Deny { .. } => false,
            })
            .count();
        let releases = history
            .iter()
            .filter(|e| e.event.from_state.as_ref().is_some_and(|s| def.halted.contains(s)))
            .count();
        let status_word = match status {
            Status::Ended => "ended",
            Status::NeedsPerson => "NEEDS PERSON",
            Status::WillEscalate => "WILL ESCALATE",
            Status::Live => "live",
        };
        let mut line = format!(
            "  record {}: {}  [{}]  {} move(s)",
            record.id.0,
            record.snapshot.state,
            status_word,
            history.len()
        );
        if escalations > 0 {
            let _ = write!(line, " · {escalations} escalation(s) · {releases} release(s)");
        }
        if let Some(noted) = history.iter().rev().find(|e| e.event.note.is_some()) {
            let _ = write!(
                line,
                " · last note: \"{}\" ({})",
                noted.event.note.as_deref().unwrap_or_default(),
                noted.event.actor
            );
        }
        let _ = writeln!(out, "{line}");
    }
    Ok(out)
}

fn do_move(
    engine: &Engine,
    ledger: &dyn Ledger,
    record_id: &str,
    role: &str,
    to: &str,
    note: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let record = ledger.load(&RecordId(record_id.to_string())).map_err(|e| e.to_string())?;
    let mut attempt = Attempt::new(role, to);
    if let Some(text) = note {
        attempt = attempt.saying(text);
    }
    let decision = engine.authorize(&record.snapshot, &attempt);
    if let Decision::Deny { reason } = &decision {
        return Err(format!("refused: {reason}"));
    }
    let outcome = match &decision {
        Decision::Allow { to, .. } => format!("record {} moved to '{to}'", record.id.0),
        Decision::Exhausted { to, counter } => format!(
            "'{counter}' is spent: record {} routed to '{to}' instead",
            record.id.0
        ),
        Decision::Deny { .. } => unreachable!("denials returned above"),
    };
    let event = Event {
        actor: actor.to_string(),
        role: role.to_string(),
        from_state: Some(record.snapshot.state.clone()),
        decision,
        note: note.map(str::to_string),
    };
    let version = ledger.apply(&record, &event).map_err(|e| e.to_string())?;
    Ok(format!("{outcome} (version {})", version.0))
}

/// One notification per awaiting record. Pure assembly; the send is the
/// adapter's.
fn notifications_for(engine: &Engine, rows: &[(Record, Status)]) -> Vec<Notification> {
    let def = engine.def();
    rows.iter()
        .filter(|(_, s)| matches!(s, Status::NeedsPerson | Status::WillEscalate))
        .map(|(record, status)| {
            let spent: Vec<String> = def
                .counters
                .iter()
                .filter_map(|c| {
                    let held = record.snapshot.counters.get(&c.name).copied().unwrap_or(0);
                    (held >= c.max).then(|| format!("{}: {held} of {} spent", c.name, c.max))
                })
                .collect();
            let mut reason = waiting_reason(engine, record, status).to_string();
            if !spent.is_empty() {
                reason = format!("{reason} — {}", spent.join(", "));
            }
            Notification {
                workflow: def.name.clone(),
                record: record.id.0.clone(),
                state: record.snapshot.state.clone(),
                reason,
                // A pause blocks its whole loop until a person acts; the
                // will-escalate case merely predicts one.
                urgency: if def.halted.contains(&record.snapshot.state) {
                    Urgency::High
                } else {
                    Urgency::Normal
                },
                link: None,
            }
        })
        .collect()
}

fn send_notifications(
    engine: &Engine,
    rows: &[(Record, Status)],
    notifier: &dyn Notifier,
) -> Result<String, String> {
    let notifications = notifications_for(engine, rows);
    let population = notifications.len();
    for n in &notifications {
        notifier
            .notify(n)
            .map_err(|e| format!("record {}: {e}", n.record))?;
    }
    Ok(format!(
        "notified {population} of {} open record(s) in '{}'",
        rows.len(),
        engine.def().name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use ferrostep_sqlite::SqliteLedger;

    fn engine() -> Engine {
        let def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        Engine::new(def).unwrap()
    }

    fn allow(to: &str, counters: &[(&str, u32)]) -> Decision {
        Decision::Allow {
            to: to.to_string(),
            counter_updates: counters
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    fn event(actor: &str, role: &str, from: Option<&str>, decision: Decision, note: Option<&str>) -> Event {
        Event {
            actor: actor.to_string(),
            role: role.to_string(),
            from_state: from.map(str::to_string),
            decision,
            note: note.map(str::to_string),
        }
    }

    /// A ledger holding one record per interesting disposition: live, paused
    /// (with its ceiling spent), will-escalate, and ended.
    fn seeded() -> (tempfile::TempDir, SqliteLedger, BTreeMap<&'static str, RecordId>) {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SqliteLedger::open(dir.path().join("ledger.db")).unwrap();
        let mut ids = BTreeMap::new();
        let file = |state: &str| {
            ledger
                .create(
                    &Scope::all().with("branch", "main"),
                    &allow(state, &[]),
                    &event("op", "operator", None, allow(state, &[]), None),
                )
                .unwrap()
        };

        ids.insert("live", file("awaiting_worker").id);

        let paused = file("awaiting_worker");
        ledger
            .apply(
                &paused,
                &event(
                    "engine",
                    "worker",
                    Some("awaiting_worker"),
                    allow("escalated", &[("agent_passes", 3)]),
                    Some("three passes spent with findings still open"),
                ),
            )
            .unwrap();
        ids.insert("paused", paused.id);

        let stuck = file("awaiting_worker");
        ledger
            .apply(
                &stuck,
                &event(
                    "engine",
                    "worker",
                    Some("awaiting_worker"),
                    allow("awaiting_worker", &[("agent_passes", 3)]),
                    None,
                ),
            )
            .unwrap();
        ids.insert("stuck", stuck.id);

        let ended = file("awaiting_review");
        ledger
            .apply(
                &ended,
                &event("sam", "reviewer", Some("awaiting_review"), allow("approved", &[]), None),
            )
            .unwrap();
        ids.insert("ended", ended.id);

        (dir, ledger, ids)
    }

    #[test]
    fn awaiting_shows_the_waiting_the_reason_and_the_person_moves() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), false).unwrap();
        let rendered = render_awaiting(&engine, &rows);

        assert!(rendered.starts_with("2 of 3 open record(s)"), "{rendered}");
        assert!(rendered.contains(&format!("record {} — escalated", ids["paused"].0)));
        assert!(rendered.contains("paused; a person decides"), "{rendered}");
        assert!(rendered.contains("every remaining move would escalate"), "{rendered}");
        assert!(rendered.contains("agent_passes: 3 of 3 spent"));
        // The operator's release, with what it does and what it needs.
        assert!(rendered.contains("operator may:"));
        assert!(rendered.contains("-> awaiting_worker (resets agent_passes)"), "{rendered}");
        // The live record is not offered to anyone here, and the ended one
        // is not even enumerated.
        assert!(!rendered.contains(&format!("record {} ", ids["live"].0)));
        assert!(!rendered.contains(&format!("record {} ", ids["ended"].0)));
    }

    #[test]
    fn awaiting_reports_the_population_even_when_nobody_waits() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SqliteLedger::open(dir.path().join("ledger.db")).unwrap();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), false).unwrap();
        let rendered = render_awaiting(&engine, &rows);
        assert!(
            rendered.starts_with("0 of 0 open record(s)"),
            "an empty answer names its population: {rendered}"
        );
    }

    #[test]
    fn audit_reads_the_same_enumeration_and_tells_the_paths_apart() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), true).unwrap();
        let rendered = render_audit(&engine, &ledger, &rows).unwrap();

        assert!(rendered.contains("4 record(s) in scope"), "{rendered}");
        assert!(rendered.contains("by state:"), "{rendered}");
        assert!(rendered.contains("approved 1"), "{rendered}");
        // The escalated record shows its escalation and carries the note.
        let paused_line = rendered
            .lines()
            .find(|l| l.starts_with(&format!("  record {}:", ids["paused"].0)))
            .expect("the paused record is in the report");
        assert!(paused_line.contains("1 escalation(s)"), "{paused_line}");
        assert!(paused_line.contains("three passes spent"), "{paused_line}");
        // The ended record reads as ended, with no escalations to mention.
        let ended_line = rendered
            .lines()
            .find(|l| l.starts_with(&format!("  record {}:", ids["ended"].0)))
            .expect("the ended record is in the report");
        assert!(ended_line.contains("[ended]"), "{ended_line}");
        assert!(!ended_line.contains("escalation"), "{ended_line}");
    }

    #[test]
    fn a_move_resolves_an_escalation_without_a_database_console() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let outcome = do_move(
            &engine,
            &ledger,
            &ids["paused"].0,
            "operator",
            "awaiting_worker",
            Some("worth one more pass"),
            "lauren",
        )
        .unwrap();
        assert!(outcome.contains("moved to 'awaiting_worker'"), "{outcome}");
        let released = ledger.load(&ids["paused"]).unwrap();
        assert_eq!(released.snapshot.state, "awaiting_worker");
        assert_eq!(released.snapshot.counters["agent_passes"], 0, "the release re-armed");
        let history = ledger.history(&ids["paused"]).unwrap();
        let last = history.last().unwrap();
        assert_eq!(last.event.actor, "lauren");
        assert_eq!(last.event.note.as_deref(), Some("worth one more pass"));
    }

    #[test]
    fn a_refused_move_persists_nothing_and_says_why() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let before = ledger.history(&ids["paused"]).unwrap().len();
        // A worker cannot release a halt.
        let refused = do_move(
            &engine,
            &ledger,
            &ids["paused"].0,
            "worker",
            "awaiting_worker",
            None,
            "impatient-agent",
        );
        let message = refused.unwrap_err();
        assert!(message.starts_with("refused:"), "{message}");
        assert_eq!(ledger.history(&ids["paused"]).unwrap().len(), before, "nothing persisted");
    }

    #[test]
    fn an_exhausted_move_routes_and_says_it_routed() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        // The stuck record: claiming with the ceiling spent routes it.
        let outcome = do_move(
            &engine,
            &ledger,
            &ids["stuck"].0,
            "worker",
            "working",
            None,
            "worker-1",
        )
        .unwrap();
        assert!(outcome.contains("routed to 'escalated' instead"), "{outcome}");
        assert_eq!(ledger.load(&ids["stuck"]).unwrap().snapshot.state, "escalated");
    }

    #[test]
    fn notifications_carry_the_reason_and_grade_a_pause_above_a_prediction() {
        let (_dir, ledger, _ids) = seeded();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), false).unwrap();
        let notifications = notifications_for(&engine, &rows);
        assert_eq!(notifications.len(), 2, "one per waiting record, none for the live one");
        let paused = notifications.iter().find(|n| n.state == "escalated").unwrap();
        assert_eq!(paused.urgency, Urgency::High);
        assert!(paused.reason.contains("agent_passes: 3 of 3 spent"), "{}", paused.reason);
        let stuck = notifications.iter().find(|n| n.state == "awaiting_worker").unwrap();
        assert_eq!(stuck.urgency, Urgency::Normal);
        assert!(stuck.reason.contains("would escalate"), "{}", stuck.reason);
    }

    #[test]
    fn scope_narrows_every_view_the_same_way() {
        let (_dir, ledger, _ids) = seeded();
        let engine = engine();
        let elsewhere = Scope::all().with("branch", "other");
        let rows = records_with_status(&engine, &ledger, &elsewhere, true).unwrap();
        assert!(rows.is_empty(), "everything seeded lives on branch=main");
    }

    #[test]
    fn the_store_argument_names_its_own_remedy() {
        let Err(refused) = open_ledger("postgres:somewhere", None, None) else {
            panic!("an unknown store scheme must be refused");
        };
        assert!(refused.contains("sqlite:<path> or pocketbase:<url>"), "{refused}");
        let Err(refused) = open_ledger("sqlite:/tmp/x.db", None, Some("map.json")) else {
            panic!("a map on a sqlite store must be refused");
        };
        assert!(refused.contains("pocketbase: stores only"), "{refused}");
    }
}

//! ferrostep — the person-facing surface over a refereed ledger.
//!
//! Most subcommands are one set of primitives over a ledger. Two are not, and
//! neither needs a store: `explain` reads a definition, and `agent-env` answers
//! the other half of an actor's question — the ledger says *what may be done*,
//! the roster says *who is doing it*. (⚠ No count is stated here on purpose:
//! a number in prose goes stale silently, and this line has already been wrong
//! once.)
//!
//! * `awaiting` — the decision surface (ROADMAP B2): which records await a
//!   person, and which moves their role has, each annotated with what it
//!   would actually do. Built on one enumeration plus [`Engine::status`] and
//!   [`Engine::next_moves`].
//! * `file` — the way in. A store with a console of its own can be handed a
//!   record without the referee ever being asked; SQLite has none, so the
//!   zero-install path had no way to get a first record in short of writing
//!   a program.
//! * `move` — the resolution: authorize one attempt and persist what the
//!   decision says. This is how a person resolves an escalation without
//!   opening a database console.
//! * `audit` — the report (ROADMAP B4): what happened, per record — moves,
//!   escalations, releases, the last human note — a *reader of the same
//!   enumeration `awaiting` uses*, so the two views cannot disagree about
//!   the ledger.
//! * `rescope` — the other kind of move: not where a record goes next, but
//!   which unit of work it belongs to. Refereed like any other move, so the
//!   operation that was a raw database write to the field every query filters
//!   on is now versioned, evented, and refused where it should be.
//! * `notify` — B3's wiring: one notification per awaiting record, through
//!   the notifier adapter. Invoked when the caller decides; nothing here
//!   polls or schedules.
//! * `explain` — what a definition permits, readable without a ledger. Its
//!   numbers section exists because a ceiling that moves into a definition
//!   leaves derived arithmetic behind elsewhere, and that arithmetic does not
//!   contain the value it came from.
//! * `agent-env` — the roster: an agent's title, the identity it signs work
//!   under, and the persona document a launcher hands it, as shell
//!   assignments. It touches no ledger and takes no store, because who the
//!   actors are is knowable without one. This is the surface that lets a
//!   repo with no Rust toolchain — or no FerroStep-refereed ledger yet —
//!   resolve an actor at all.
//!
//! The store is named per invocation (`sqlite:<path>` or
//! `pocketbase:<url>`) — an application-layer choice among adapters, which
//! is exactly where naming an adapter belongs.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use ferrostep_core::{Attempt, Decision, Engine, Status, WorkflowDef};
use ferrostep_ledger::{Answer, Event, Ledger, Record, RecordId, Scope, StoreShape};
use ferrostep_notify::{Notification, Notifier, Ntfy, Urgency};
use ferrostep_roster::Roster;

const USAGE: &str = "ferrostep — the person-facing surface over a FerroStep-refereed ledger

USAGE:
  ferrostep <awaiting|audit|file|move|rescope|grade|notify|doctor> --workflow <def.json> --store <target> [options]
  ferrostep explain --workflow <def.json> [--map <map.json>]
  ferrostep agent-env [--agent <title>] [--roster <config.yaml>]

COMMON:
  --workflow <path>     the workflow definition JSON the ledger is refereed by
  --store <target>      sqlite:<path> or pocketbase:<url>
  --token <token>       auth token for pocketbase: stores
                        (or the FERROSTEP_POCKETBASE_TOKEN environment variable)
  --map <path>          collection-mapping JSON for a pocketbase: store that
                        referees an existing collection instead of the
                        generic ferrostep ones
  --scope <key=value>   narrow to records labelled key=value (repeatable)
  --role <role>         for `awaiting` and `notify`: whose queue to report.
                        ⚠ Without it they ask whether a PERSON must act, which
                        cannot see a record handed from one agent to another —
                        the ordinary handover in a worker/reviewer loop. Name a
                        role to ask what is waiting on that actor instead.

file:                          (also spelled `create`; files a new record)
  --role <role> [--note <text> | --note-file <path>] [--actor <name>] [--scope …]
  [--counter <name=value> …]   what a filing ceiling is measured against.
                               Required for every counter the definition's
                               `creation.spends` names: the count bounds a
                               branch or a cycle, not the new record, so this
                               tool cannot take it for you — see `explain`.

move:
  --record <id> --role <role> --to <state> [--note <text> | --note-file <path>] [--actor <name>]

grade:                         (move one graded attribute along its ladder)
  --record <id> --role <role> --attribute <name> --to <value>
  [--note <text> | --note-file <path>] [--actor <name>]
                        ⚠ One attribute per invocation: each has its own
                        ladder and its own grants per DIRECTION, and the
                        definition says who may raise and who may lower — the
                        engine never assumes which of those is the safe one.
                        See `explain`.

rescope:                       (move a record to a different unit of work)
  --record <id> --role <role> --set <label=value> [--set …] [--note <text> | --note-file <path>]
  [--actor <name>]

notify:
  --ntfy <server> --topic <topic> [--ntfy-token <token>]

doctor:                        (read-only; exits non-zero on a fault OR on
                               anything it could not check)
  --store <target> [--map <path>] [--token <token>]
                        whether this definition is satisfiable against this
                        store: are its states values the state column accepts,
                        do its counters and scope labels have columns, and can
                        the INSTALLED write path reach them. ⚠ A question this
                        cannot answer is reported as unchecked and fails — a
                        gate that passes because it could not look is worse
                        than no gate.

explain:                       (takes no --store)
  what the definition permits, and the numbers it asserts — including the
  off-by-one neighbours that derived arithmetic hides behind
  --map <path>          also list the columns the referee owns, and the sweep
                        to run before closing them to direct writes. ⚠ Sweep
                        PROSE as well as code: a persona or a skill file that
                        tells an actor to write one of those columns is a
                        write path with no call site to find.

agent-env:                     (takes no --workflow and no --store)
  --agent <title>       the roster entry to resolve (default: its default_agent)
  --roster <path>       the roster file (default: the nearest config.yaml at
                        or above the working directory)
  --format shell|json   shell assignments to eval (default), or JSON for a
                        caller that is not a shell
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

    /// Refuse any flag this subcommand does not read.
    ///
    /// ⚠⚠ **Without this a flag is silently ignored, and the two ways that
    /// bites are both expensive.** A *typo* — `--scpoe branch=main` — quietly
    /// widens a scoped query, so an audit reports on records it was never
    /// asked about and exits 0. And a **binary older than the flag** accepts
    /// it, ignores it, and answers confidently: `--role developer` against a
    /// build that predates role scoping returned "0 of 12 await a person",
    /// which is *correct for the question it actually asked* and completely
    /// wrong for the one that was asked of it. Measured on this workspace's
    /// own installed binary, 2026-08-26.
    ///
    /// ⚠ This is the convention AGENTS.md already holds for generated files,
    /// arriving at a surface that had not been held to it: **an older thing
    /// meeting a newer request must refuse the part it does not understand,
    /// never accept and ignore it.** The ping's `writes` exists for exactly
    /// this reason. A CLI's flags outlive the binary that parses them the
    /// same way a hook outlives the adapter that generated it.
    ///
    /// The refusal doubles as the version diagnostic: a caller that gets
    /// "this build does not accept --role" knows immediately what is wrong,
    /// where a silent zero tells them nothing and looks like an answer.
    fn reject_unknown(&self, command: &str, accepted: &[&str]) -> Result<(), String> {
        let unknown: Vec<&str> =
            self.0.keys().map(String::as_str).filter(|f| !accepted.contains(f)).collect();
        if unknown.is_empty() {
            return Ok(());
        }
        Err(format!(
            "'{command}' does not accept: {}\n  it accepts: {}\n\n\
             ⚠ If you expected this flag to work, this build may predate it — \
             check `ferrostep --help` against the version you meant to run. \
             A flag that is accepted and ignored answers the wrong question \
             confidently, which is why this refuses instead.",
            unknown.iter().map(|f| format!("--{f}")).collect::<Vec<_>>().join(", "),
            accepted.iter().map(|f| format!("--{f}")).collect::<Vec<_>>().join(", "),
        ))
    }
}

/// Every flag each subcommand reads. ⚠ Derived into the refusal message, so a
/// flag added to a command without being listed here is refused rather than
/// silently working — which is the safe direction for the mistake to fall.
/// Resolve the move's reason from `--note` or `--note-file`.
///
/// ⚠ **Both set is a refusal, not a preference.** Silently choosing one is how
/// a caller ends up recording a reason it did not write, and which one a reader
/// would guess is not something to leave to a reader.
///
/// ⚠⚠ **An unreadable or empty file is a refusal too, and that is the load-
/// bearing part.** Some moves REQUIRE a note. If a missing path or an empty
/// file quietly resolved to "no note", the engine would refuse with *needs a
/// reason* — a true statement pointing at the wrong cause, and the caller would
/// go looking at their definition instead of their path. Worse where a note is
/// optional: the move would land with its reason silently dropped.
fn note_text(flags: &Flags) -> Result<Option<String>, String> {
    match (flags.get("note"), flags.get("note-file")) {
        (Some(_), Some(_)) => Err(
            "--note and --note-file are both set; use one. The reason for a move is \
             recorded verbatim, so which of the two was meant is not a guess worth making."
                .to_string(),
        ),
        (_, Some(path)) => {
            let body = std::fs::read_to_string(path)
                .map_err(|e| format!("--note-file '{path}' could not be read: {e}"))?;
            if body.trim().is_empty() {
                return Err(format!(
                    "--note-file '{path}' is empty. A move that requires a reason would be \
                     refused for the wrong cause, and one that does not would record nothing."
                ));
            }
            Ok(Some(body.trim_end().to_string()))
        }
        (note, None) => Ok(note.map(str::to_string)),
    }
}

fn accepted_flags(command: &str) -> &'static [&'static str] {
    const LEDGER: [&str; 5] = ["workflow", "store", "token", "map", "scope"];
    match command {
        "awaiting" => &["workflow", "store", "token", "map", "scope", "role"],
        "audit" => &LEDGER,
        "file" | "create" => {
            &["workflow", "store", "token", "map", "scope", "role", "counter", "note", "actor", "note-file"]
        }
        "move" => {
            &["workflow", "store", "token", "map", "scope", "record", "role", "to", "note", "actor", "note-file"]
        }
        "rescope" => {
            &["workflow", "store", "token", "map", "scope", "record", "role", "set", "note", "actor", "note-file"]
        }
        "grade" => &[
            "workflow", "store", "token", "map", "scope", "record", "role", "attribute", "to",
            "note", "actor", "note-file",
        ],
        "notify" => {
            &["workflow", "store", "token", "map", "scope", "role", "ntfy", "topic", "ntfy-token"]
        }
        "doctor" => &["workflow", "store", "token", "map"],
        "explain" => &["workflow", "map"],
        "agent-env" => &["agent", "roster", "format"],
        _ => &[],
    }
}

fn run(args: &[String]) -> Result<String, String> {
    // ⚠ Asking for help must not be parsed as *using* the tool. Every flag
    // here takes a value, so `--help` used to be read as a flag missing its
    // argument and answered "--help needs a value" — and `help <subcommand>`
    // was an unexpected argument. Both spellings of "explain this to me"
    // failed, at the moment a person had already admitted to not knowing.
    // Reported by the first adopter, who worked around it silently.
    if args.iter().any(|a| a == "--help" || a == "-h") || args.first().is_some_and(|a| a == "help") {
        return Ok(USAGE.to_string());
    }
    let Some(command) = args.first() else {
        return Err(format!("no subcommand given\n\n{USAGE}"));
    };
    let flags = Flags::parse(&args[1..])?;
    // ⚠ Before anything is opened or read. An unknown flag is refused rather
    // than ignored — see `Flags::reject_unknown`. Checked only for commands
    // this build knows, so an unknown SUBCOMMAND still gets its own error
    // rather than a confusing complaint about its flags.
    let accepted = accepted_flags(command);
    if !accepted.is_empty() {
        flags.reject_unknown(command, accepted)?;
    }
    // The roster is not a question about a ledger, so it answers before one
    // is opened. Requiring a workflow and a store to ask who the developer is
    // would put the actor's own identity behind the very machinery an actor
    // needs its identity to operate.
    if command == "agent-env" {
        return agent_env(&flags);
    }
    let engine = load_engine(flags.require("workflow")?)?;
    // A definition is readable without a ledger, and the person most in need
    // of reading one has not connected anything yet.
    if command == "explain" {
        // The map is deployment configuration, not part of the definition —
        // but the columns it names are the ones an adopter has to sweep their
        // tree for before closing them, and this is the subcommand that hands
        // over lists to go hunting with. Optional: a definition explains
        // itself without one.
        let map = match flags.get("map") {
            Some(path) => Some(load_map(path)?),
            None => None,
        };
        return Ok(explain(&engine, map.as_ref()));
    }
    let ledger = open_ledger(flags.require("store")?, flags.get("token"), flags.get("map"))?;
    let mut scope = Scope::all();
    for pair in flags.all("scope") {
        let (key, value) = pair
            .split_once('=')
            .ok_or(format!("--scope takes key=value, got '{pair}'"))?;
        scope = scope.with(key, value);
    }
    // ⚠⚠ A NOTE THAT CANNOT SURVIVE A SHELL IS A NOTE PEOPLE DO NOT WRITE.
    // Every note-bearing move takes its reason as a command-line string, so a
    // reason containing backticks or quotes needs a heredoc to post safely —
    // and the first adopter hit exactly that posting the comment that reported
    // the same defect in their own tooling. This repo already ruled that a
    // commit message goes in a file and never in a quoted `-m`, for precisely
    // this reason; the rule existed and the surface did not.
    let note = note_text(&flags)?;

    match command.as_str() {
        // ⚠ Before anything is moved, and never as part of moving something.
        // Read-only by construction: it asks the store what it accepts and
        // compares that against the definition, and a run of it cannot spend a
        // ceiling, append an event or change a record.
        "doctor" => {
            let map = match flags.get("map") {
                Some(path) => Some(load_map(path)?),
                None => None,
            };
            doctor(&engine, ledger.as_ref(), map.as_ref())
        }
        "awaiting" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, false)?;
            Ok(render_awaiting(&engine, &rows, flags.get("role")))
        }
        "audit" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, true)?;
            render_audit(&engine, ledger.as_ref(), &rows)
        }
        // `create` is the ledger interface's word and `file` is the
        // definition's; a person reaching for either has said the same thing,
        // and answering only the one we happen to prefer is the mistake
        // `--help` already made once.
        "file" | "create" => {
            let role = flags.require("role")?;
            do_file(
                &engine,
                ledger.as_ref(),
                &scope,
                role,
                flags.all("counter"),
                note.as_deref(),
                flags.get("actor").unwrap_or(role),
            )
        }
        "move" => {
            let role = flags.require("role")?;
            do_move(
                &engine,
                ledger.as_ref(),
                flags.require("record")?,
                role,
                flags.require("to")?,
                note.as_deref(),
                flags.get("actor").unwrap_or(role),
            )
        }
        "grade" => {
            let role = flags.require("role")?;
            do_grade(
                &engine,
                ledger.as_ref(),
                flags.require("record")?,
                role,
                (flags.require("attribute")?, flags.require("to")?),
                note.as_deref(),
                flags.get("actor").unwrap_or(role),
            )
        }
        "rescope" => {
            let role = flags.require("role")?;
            // ⚠ Loaded for the address, not for the connection. A record's unit
            // of work is the whole tuple of scope labels, and only the map says
            // what that tuple is.
            let map = match flags.get("map") {
                Some(path) => Some(load_map(path)?),
                None => None,
            };
            let sets = flags.all("set");
            let mut out = do_rescope(
                &engine,
                ledger.as_ref(),
                flags.require("record")?,
                role,
                sets,
                note.as_deref(),
                flags.get("actor").unwrap_or(role),
            )?;
            if let Some(warning) = partial_rescope_warning(map.as_ref(), sets) {
                out.push_str(&warning);
            }
            Ok(out)
        }
        "notify" => {
            let rows = records_with_status(&engine, ledger.as_ref(), &scope, false)?;
            let mut notifier = Ntfy::new(flags.require("ntfy")?, flags.require("topic")?);
            if let Some(token) = flags.get("ntfy-token") {
                notifier = notifier.with_token(token);
            }
            send_notifications(&engine, &rows, &notifier, flags.get("role"))
        }
        other => Err(format!("unknown subcommand '{other}'\n\n{USAGE}")),
    }
}

// ----------------------------------------------------------------------
// doctor — is this definition satisfiable against this store?
// ----------------------------------------------------------------------

/// What one of `doctor`'s checks concluded.
///
/// ⚠⚠ **[`Level::Unchecked`] IS NOT A PASS, AND IT IS A SEPARATE LEVEL SO
/// THAT IT CANNOT BE RENDERED AS ONE.** Every other design collapses under its
/// own convenience: a check that could not run has nothing to print, so it
/// prints nothing, so the report reads clean. This repo has already shipped a
/// green verdict from a check that never executed, and the lesson each time is
/// the same — an instrument that cannot fail loudly must at least say when it
/// did not look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    /// The definition and the store disagree. A live move would be refused, or
    /// worse, accepted and dropped.
    Fault,
    /// The question could not be answered. The report says why, and the exit
    /// status is the same as a fault: nothing was verified.
    Unchecked,
    /// True, worth knowing, and not a problem.
    Note,
    /// A check that ran and agreed. ⚠ Carried and printed deliberately: the
    /// difference between "this agreed" and "this was never asked" is invisible
    /// unless the agreement is stated, and that difference is the whole subject
    /// of this command.
    Agreed,
}

impl Level {
    fn mark(self) -> char {
        match self {
            Level::Fault => '✗',
            Level::Unchecked => '?',
            Level::Note => '·',
            Level::Agreed => '✓',
        }
    }
}

struct Finding {
    level: Level,
    section: &'static str,
    text: String,
}

const DEF_MAP: &str = "definition ↔ mapping";
const DEF_STORE: &str = "definition ↔ store";
const MAP_STORE: &str = "mapping ↔ store";
const MAP_INSTALLED: &str = "mapping ↔ installed write path";

/// The sections in report order, so a section with no findings can still be
/// printed as such rather than vanishing.
const SECTIONS: [&str; 4] = [DEF_MAP, DEF_STORE, MAP_STORE, MAP_INSTALLED];

/// Every check, against a definition, an optional mapping, and whatever the
/// store was able to say about itself.
///
/// ⚠ **Pure, and takes the store's answer as a value rather than a
/// connection**, so the whole matrix — including every way the store can fail
/// to answer — is reachable from a test with no store in it. A checker whose
/// failure paths can only be exercised against live infrastructure is a
/// checker whose failure paths are not exercised.
///
/// ⚠⚠ **The checks that need no store run first and run always.** A store that
/// cannot be read still leaves the definition-versus-mapping half completely
/// answerable, and answering half a question beats refusing the whole one —
/// the counter that is declared in a definition and has no column is the
/// cheapest, most certain fault here and it needs nothing but two files.
fn diagnose(
    def: &WorkflowDef,
    map: Option<&ferrostep_pocketbase::CollectionMap>,
    shape: &Result<StoreShape, String>,
) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut say = |level: Level, section: &'static str, text: String| {
        out.push(Finding { level, section, text });
    };

    // ---- definition ↔ mapping: no store needed, so never skipped ----
    if let Some(map) = map {
        for counter in &def.counters {
            if map.counter_fields.contains(&counter.name) {
                say(
                    Level::Agreed,
                    DEF_MAP,
                    format!("counter '{}' is mapped to a column of the same name", counter.name),
                );
            } else {
                say(
                    Level::Fault,
                    DEF_MAP,
                    format!(
                        "counter '{}' has no column in the map — every spend of it would be \
                         dropped, so its ceiling of {} can never fire",
                        counter.name, counter.max
                    ),
                );
            }
        }
        for rescope in &def.rescopes {
            if !map.scope_fields.contains(&rescope.label) {
                say(
                    Level::Fault,
                    DEF_MAP,
                    format!(
                        "scope label '{}' is rescopable by '{}' and has no column in the map — \
                         a documented move with nowhere to land",
                        rescope.label, rescope.role
                    ),
                );
            }
        }
        for column in &map.counter_fields {
            if !def.counters.iter().any(|c| &c.name == column) {
                say(
                    Level::Note,
                    DEF_MAP,
                    format!(
                        "column '{column}' is refereed as a counter and this definition never \
                         spends it — harmless if another workflow does"
                    ),
                );
            }
        }
        // ⚠ This check did not exist until graded attributes did. Before that
        // the engine had no vocabulary for an attribute, so `doctor` said so
        // rather than silently covering three kinds of four — and that note is
        // now replaced by the check it was standing in for.
        for grade in &def.grades {
            if map.attribute_fields.contains(&grade.attribute) {
                say(
                    Level::Agreed,
                    DEF_MAP,
                    format!(
                        "graded attribute '{}' is mapped to a column of the same name",
                        grade.attribute
                    ),
                );
            } else {
                say(
                    Level::Fault,
                    DEF_MAP,
                    format!(
                        "graded attribute '{}' has no column in the map — the ladder is \
                         documented and every grade of it would be dropped",
                        grade.attribute
                    ),
                );
            }
        }
        // ⚠⚠ AND THE OTHER DIRECTION, which is the one that matters more. A
        // column refereed as an attribute with no ladder behind it is the
        // STOPGAP shape: closed to direct writes, so the write is
        // authenticated and evented, and with nothing saying who may set which
        // value or in which direction. That is a real deployment state and not
        // a fault — it is what the stopgap was for — but a report that stayed
        // silent about it would let an adopter believe the ladder is guarding
        // a column it has never heard of.
        for column in &map.attribute_fields {
            if !def.grades.iter().any(|g| &g.attribute == column) {
                say(
                    Level::Note,
                    DEF_MAP,
                    format!(
                        "column '{column}' is refereed as an attribute and no ladder grades it: \
                         writes to it are authenticated and evented, and nothing says who may \
                         set which value"
                    ),
                );
            }
        }
    } else {
        say(
            Level::Note,
            DEF_MAP,
            "no --map given, so there are no per-name columns to check a definition against"
                .to_string(),
        );
    }

    // ---- everything below needs the store to have answered ----
    let shape = match shape {
        Ok(shape) => shape,
        // ⚠ One finding per QUESTION rather than one repetition of the reason.
        // The reason is printed once in the header; what a reader needs here
        // is the list of things nobody established, because that list is what
        // they are on the hook for checking by hand.
        Err(_) => {
            for (section, question) in [
                (DEF_STORE, "which values the state column accepts"),
                (MAP_STORE, "whether the mapped columns exist"),
                (MAP_INSTALLED, "which columns the installed write path can reach"),
            ] {
                say(Level::Unchecked, section, format!("{question} — never established"));
            }
            return out;
        }
    };

    // ---- definition ↔ store ----
    match &shape.accepted_states {
        Answer::Said(accepted) => {
            let refused: Vec<&String> =
                def.states.iter().filter(|s| !accepted.contains(s)).collect();
            for state in &refused {
                say(
                    Level::Fault,
                    DEF_STORE,
                    format!(
                        "state '{state}' is not an accepted value of the state column in '{}' — \
                         every transition into it would be refused by the store",
                        shape.subject
                    ),
                );
            }
            if refused.is_empty() {
                say(
                    Level::Agreed,
                    DEF_STORE,
                    format!(
                        "all {} of this definition's states are accepted values of the state \
                         column",
                        def.states.len()
                    ),
                );
            }
            let unused: Vec<&String> =
                accepted.iter().filter(|s| !def.states.contains(s)).collect();
            if !unused.is_empty() {
                say(
                    Level::Note,
                    DEF_STORE,
                    format!(
                        "the state column also accepts {}, which this definition never uses",
                        unused.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", ")
                    ),
                );
            }
        }
        Answer::NothingToConstrain => say(
            Level::Note,
            DEF_STORE,
            "the state column constrains nothing, so no state in this definition can be refused \
             by it"
                .to_string(),
        ),
        Answer::Unknown => say(
            Level::Unchecked,
            DEF_STORE,
            "the store did not say which values its state column accepts, so a state it would \
             refuse is still possible"
                .to_string(),
        ),
    }

    // ⚠⚠ **THE SAME QUESTION AS THE BLOCK ABOVE, ASKED OF EVERY LADDER.** A
    // definition's states have to be values the state column accepts; a
    // ladder's values have to be values ITS column accepts. Checking the first
    // and not the structurally identical second is a checker whose population
    // is narrower than its subject — and the fault it misses is the one that
    // created this tool, arriving at the first grade instead of the first
    // transition. Raised by the adopter who was about to declare such a
    // column, whose ladder happened to match it exactly: the state in which
    // the gap is invisible and stays invisible until someone edits either
    // list.
    if !def.grades.is_empty() {
        match &shape.accepted_values {
            Answer::Said(by_column) => {
                for grade in &def.grades {
                    match by_column.get(&grade.attribute) {
                        Some(Answer::Said(accepted)) => {
                            let refused: Vec<&String> =
                                grade.ladder.iter().filter(|v| !accepted.contains(v)).collect();
                            for value in &refused {
                                say(
                                    Level::Fault,
                                    DEF_STORE,
                                    format!(
                                        "'{value}' is on the '{}' ladder and is not an accepted \
                                         value of that column in '{}' — every grade to it would \
                                         be refused by the store",
                                        grade.attribute, shape.subject
                                    ),
                                );
                            }
                            if refused.is_empty() {
                                say(
                                    Level::Agreed,
                                    DEF_STORE,
                                    format!(
                                        "all {} values of the '{}' ladder are accepted by its \
                                         column",
                                        grade.ladder.len(),
                                        grade.attribute
                                    ),
                                );
                            }
                            // ⚠ Mirrors the note the state block already makes,
                            // and means more here: a value the column accepts
                            // and no ladder names is one the referee has no
                            // opinion about, so nothing says who may set it.
                            let unused: Vec<&String> =
                                accepted.iter().filter(|v| !grade.ladder.contains(v)).collect();
                            if !unused.is_empty() {
                                say(
                                    Level::Note,
                                    DEF_STORE,
                                    format!(
                                        "the '{}' column also accepts {}, which its ladder never \
                                         uses — values the referee has no opinion about, \
                                         reachable only by a writer going around it",
                                        grade.attribute,
                                        unused
                                            .iter()
                                            .map(|v| format!("'{v}'"))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ),
                                );
                            }
                        }
                        Some(Answer::NothingToConstrain) => say(
                            Level::Note,
                            DEF_STORE,
                            format!(
                                "the '{}' column does not enumerate its accepted values, so no \
                                 value on its ladder can be refused by it",
                                grade.attribute
                            ),
                        ),
                        // ⚠ Deliberately not a second fault: a column that does
                        // not exist is reported where columns are compared.
                        // What is true HERE is that the ladder was checked
                        // against nothing, and an unchecked question must not
                        // read as a pass.
                        Some(Answer::Unknown) | None => say(
                            Level::Unchecked,
                            DEF_STORE,
                            format!(
                                "'{}' has no column in '{}', so its ladder was checked against \
                                 nothing",
                                grade.attribute, shape.subject
                            ),
                        ),
                    }
                }
            }
            Answer::NothingToConstrain => say(
                Level::Note,
                DEF_STORE,
                "no column in this store enumerates its accepted values, so no ladder value can \
                 be refused by one"
                    .to_string(),
            ),
            Answer::Unknown => say(
                Level::Unchecked,
                DEF_STORE,
                format!(
                    "the store did not say which values its columns accept, so a ladder value it \
                     would refuse is still possible ({} ladder{} unchecked)",
                    def.grades.len(),
                    if def.grades.len() == 1 { "" } else { "s" }
                ),
            ),
        }
    }

    // ---- mapping ↔ store ----
    match (&shape.columns, map) {
        (Answer::Said(columns), Some(map)) => {
            let mut missing = 0;
            for (kind, name) in refereed_by_kind(map) {
                match columns.get(&name) {
                    Some(kind_in_store) => say(
                        Level::Agreed,
                        MAP_STORE,
                        format!("{kind} '{name}' exists in '{}' as {kind_in_store}", shape.subject),
                    ),
                    None => {
                        missing += 1;
                        say(
                            Level::Fault,
                            MAP_STORE,
                            format!(
                                "{kind} '{name}' is refereed by the map and does not exist in \
                                 '{}' — a write to it would be refused",
                                shape.subject
                            ),
                        );
                    }
                }
            }
            let _ = missing;
        }
        (Answer::Said(columns), None) => say(
            Level::Note,
            MAP_STORE,
            format!(
                "'{}' has: {}",
                shape.subject,
                columns
                    .iter()
                    .map(|(n, t)| format!("{n} ({t})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (Answer::NothingToConstrain, _) => say(
            Level::Note,
            MAP_STORE,
            "the store has no fixed columns to disagree with a mapping".to_string(),
        ),
        (Answer::Unknown, _) => say(
            Level::Unchecked,
            MAP_STORE,
            "the store did not enumerate its columns, so a mapped column that does not exist is \
             still possible"
                .to_string(),
        ),
    }

    // ---- mapping ↔ installed write path ----
    match (&shape.writable, map) {
        (Answer::Said(groups), Some(map)) => {
            for (kind, declared) in [
                ("counters", &map.counter_fields),
                ("scope", &map.scope_fields),
                ("attributes", &map.attribute_fields),
            ] {
                let admitted = groups.get(kind).cloned().unwrap_or_default();
                for name in declared {
                    if admitted.contains(name) {
                        say(
                            Level::Agreed,
                            MAP_INSTALLED,
                            format!("the installed write path can write {kind} '{name}'"),
                        );
                    } else {
                        say(
                            Level::Fault,
                            MAP_INSTALLED,
                            format!(
                                "'{name}' is declared in the map under {kind} and the installed \
                                 write path cannot reach it, so a write to it will not land. \
                                 ⚠ A file old enough to lack the column allowlist will not even \
                                 say so — it drops the column and answers 200. Regenerate the \
                                 generated files and reinstall."
                            ),
                        );
                    }
                }
                for name in &admitted {
                    if !declared.contains(name) {
                        say(
                            Level::Note,
                            MAP_INSTALLED,
                            format!(
                                "the installed write path still admits {kind} '{name}', which \
                                 this map no longer declares — it is stale, not broken"
                            ),
                        );
                    }
                }
            }
        }
        (Answer::Said(_), None) => say(
            Level::Note,
            MAP_INSTALLED,
            "no --map given, so there are no declared column names to compare".to_string(),
        ),
        (Answer::NothingToConstrain, _) => say(
            Level::Note,
            MAP_INSTALLED,
            "this adapter writes columns itself, so there is no separately-installed half that \
             could be older than the mapping"
                .to_string(),
        ),
        (Answer::Unknown, _) => say(
            Level::Unchecked,
            MAP_INSTALLED,
            "the installed write path did not state the column names it can write, so a column \
             it would silently drop is still possible — regenerating the generated files and \
             reinstalling enables this check"
                .to_string(),
        ),
    }

    out
}

/// Every column the map refereed, paired with the word for what it is.
///
/// ⚠ **Built from the map's own fields rather than from
/// `CollectionMap::refereed_fields`**, which returns names with the kinds
/// flattened away — and a report that cannot say whether 'severity' is a
/// counter or an attribute sends the reader back to the map to find out. The
/// two lists are checked against each other by a test below, so this one
/// cannot quietly stop covering a kind that `refereed_fields` gained.
fn refereed_by_kind(map: &ferrostep_pocketbase::CollectionMap) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("the state column", map.state_field.clone()),
        ("the version column", map.version_field.clone()),
    ];
    out.extend(map.counter_fields.iter().map(|n| ("counter", n.clone())));
    out.extend(map.scope_fields.iter().map(|n| ("scope label", n.clone())));
    out.extend(map.attribute_fields.iter().map(|n| ("attribute", n.clone())));
    out
}

/// Run every check and render the report. `Err` when anything is a fault **or
/// went unchecked** — a gate that passes because it could not look is the
/// defect this command exists to remove, so both outcomes exit non-zero and
/// the summary says which happened.
fn doctor(
    engine: &Engine,
    ledger: &dyn Ledger,
    map: Option<&ferrostep_pocketbase::CollectionMap>,
) -> Result<String, String> {
    // The one line that touches a store. Everything that decides an outcome
    // is below it and takes the answer as a value.
    let shape = ledger.store_shape().map_err(|e| e.to_string());
    doctor_report(engine.def(), map, &shape)
}

/// The report and the verdict, from a definition, a mapping and whatever the
/// store said — with no store in the signature.
///
/// ⚠ Split from [`doctor`] so the *verdict* is testable, not just the
/// findings. The rule that an unchecked question fails is the one thing here
/// most likely to be softened later by someone tidying up noisy output, and a
/// test can only defend it if it can reach it.
fn doctor_report(
    def: &WorkflowDef,
    map: Option<&ferrostep_pocketbase::CollectionMap>,
    shape: &Result<StoreShape, String>,
) -> Result<String, String> {
    let findings = diagnose(def, map, shape);
    let report = render_doctor(def, shape, &findings);
    let faults = findings.iter().filter(|f| f.level == Level::Fault).count();
    let unchecked = findings.iter().filter(|f| f.level == Level::Unchecked).count();
    if faults + unchecked > 0 {
        return Err(report);
    }
    Ok(report)
}

fn render_doctor(
    def: &WorkflowDef,
    shape: &Result<StoreShape, String>,
    findings: &[Finding],
) -> String {
    let mut out = String::new();
    match shape {
        Ok(shape) => {
            let _ = writeln!(out, "workflow '{}' against '{}'", def.name, shape.subject);
        }
        Err(why) => {
            let _ = writeln!(out, "workflow '{}' against a store that did not answer", def.name);
            let _ = writeln!(out, "  ⚠ {why}");
        }
    }

    for section in SECTIONS {
        let mine: Vec<&Finding> = findings.iter().filter(|f| f.section == section).collect();
        if mine.is_empty() {
            continue;
        }
        let _ = writeln!(out, "\n{section}:");
        for finding in mine {
            // Wrapped by hand rather than by a formatter: these lines are read
            // in a terminal beside the file they are about.
            let _ = writeln!(out, "  {} {}", finding.level.mark(), finding.text);
        }
    }

    let count = |level: Level| findings.iter().filter(|f| f.level == level).count();
    let (faults, unchecked, agreed) =
        (count(Level::Fault), count(Level::Unchecked), count(Level::Agreed));
    let _ = writeln!(out, "\n{faults} fault(s), {unchecked} unchecked, {agreed} agreed");
    if unchecked > 0 {
        let _ = writeln!(
            out,
            "⚠ An unchecked question is not a passing one. This exits non-zero for the same \
             reason a fault does: nothing about it was verified."
        );
    }
    out.trim_end().to_string()
}

/// What a definition asserts, in a form a person can read and search for.
///
/// ⚠ **The numbers section is the reason this exists**, and it comes from a
/// migrating loop rather than from taste. When a ceiling moves into a
/// definition, FerroStep owns the *number* and knows nothing about the
/// *arithmetic derived from it* elsewhere in the adopter's tree — `max + 1` in
/// a guard, a range in a help string, a sentence in a brief to an actor. Those
/// do not contain the value, so searching for it finds none of them. Three
/// times in one migration, the search term that worked was a number the
/// definition never states.
///
/// So this prints the asserted values *and* their off-by-one neighbours: not
/// because the engine knows what an adopter derived, but because it is the
/// list they need in hand before they can go looking.
fn explain(engine: &Engine, map: Option<&ferrostep_pocketbase::CollectionMap>) -> String {
    let def = engine.def();
    let mut out = String::new();
    let _ = writeln!(out, "workflow '{}'", def.name);
    if let Some(purpose) = &def.purpose {
        let _ = writeln!(out, "  purpose: {purpose}  (carried, never interpreted)");
    }

    let mark = |s: &String| {
        let mut tags = Vec::new();
        if s == &def.initial {
            tags.push("initial");
        }
        if def.terminal.contains(s) {
            tags.push("ending");
        }
        if def.halted.contains(s) {
            tags.push("pause");
        }
        if tags.is_empty() { s.clone() } else { format!("{s} [{}]", tags.join(",")) }
    };
    let _ = writeln!(
        out,
        "\nstates: {}",
        def.states.iter().map(mark).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(
        out,
        "roles:  {}",
        def.roles
            .iter()
            .map(|r| if r.human { format!("{} [person]", r.name) } else { r.name.clone() })
            .collect::<Vec<_>>()
            .join(", ")
    );

    let _ = writeln!(out, "\nmoves:");
    for t in &def.transitions {
        let mut notes = Vec::new();
        if !t.spends.is_empty() {
            notes.push(format!("spends {}", t.spends.join("+")));
        }
        if !t.resets.is_empty() {
            notes.push(format!("clears {}", t.resets.join("+")));
        }
        if t.requires_note {
            notes.push("needs a reason".to_string());
        }
        let suffix = if notes.is_empty() { String::new() } else { format!("  ({})", notes.join(", ")) };
        let _ = writeln!(out, "  {} : {} -> {}{}", t.role, t.from, t.to, suffix);
    }

    // Filing is a permission like any other, and default-deny like any
    // other. Saying "nobody" out loud beats leaving a reader to conclude it
    // from a heading that is not there.
    match &def.creation {
        Some(creation) => {
            let mut notes = Vec::new();
            if !creation.spends.is_empty() {
                notes.push(format!("spends {}", creation.spends.join("+")));
            }
            if creation.requires_note {
                notes.push("needs a reason".to_string());
            }
            let suffix =
                if notes.is_empty() { String::new() } else { format!("  ({})", notes.join(", ")) };
            let _ = writeln!(
                out,
                "\nfiling: {} may file into '{}'{}",
                creation.roles.join(", "),
                def.initial,
                suffix
            );
        }
        None => {
            let _ = writeln!(out, "\nfiling: nobody, through this engine");
        }
    }

    if !def.grades.is_empty() {
        let _ = writeln!(out, "\ngraded attributes:");
        for grade in &def.grades {
            // ⚠ The ladder printed in order, with the direction words attached
            // to the ROLES that hold them. A reader's question here is never
            // "what are the values" alone — it is "who can move this toward
            // the end I care about", and the answer differs per attribute.
            let _ = writeln!(
                out,
                "  {}: {}",
                grade.attribute,
                grade.ladder.join("  ->  ")
            );
            let who = |roles: &[String]| -> String {
                if roles.is_empty() { "nobody".to_string() } else { roles.join(", ") }
            };
            let _ = writeln!(
                out,
                "    raise (toward '{}'): {}",
                grade.ladder.last().map(String::as_str).unwrap_or("?"),
                who(&grade.raise)
            );
            let _ = writeln!(
                out,
                "    lower (toward '{}'): {}",
                grade.ladder.first().map(String::as_str).unwrap_or("?"),
                who(&grade.lower)
            );
            if grade.requires_note {
                let _ = writeln!(out, "    a reason is required, in both directions");
            }
            // ⚠⚠ THE SENTENCE AN ADOPTER MOST NEEDS, and the one a definition
            // cannot state: the engine guards who moves the value and has no
            // opinion about which end of the ladder passes a gate. An adopter
            // reading only the grants will assume the familiar shape — that
            // raising is the safe direction — which is true of a gate with a
            // floor and exactly backwards for one with a minimum.
            let _ = writeln!(
                out,
                "    ⚠ the referee guards WHO MOVES THIS AND WHICH WAY, and has no opinion\n\
                 \x20     about which end of the ladder clears your gate — that is your policy,\n\
                 \x20     and it is what decides which of the two directions above is the\n\
                 \x20     permissive one."
            );
        }
    }

    if !def.rescopes.is_empty() {
        let _ = writeln!(out, "\nunit-of-work moves:");
        for r in &def.rescopes {
            let reason = if r.requires_note { "  (needs a reason)" } else { "" };
            let _ = writeln!(out, "  {} may change '{}'{}", r.role, r.label, reason);
        }
        // ⚠⚠ THE LINES ABOVE READ AS INDEPENDENT PERMISSIONS AND THE LABELS ARE
        // COORDINATES. A record's scope is the whole tuple, and every query that
        // finds work filters on it — so setting one label and leaving the rest
        // does not move the record to a new unit of work, it leaves the record
        // in no consistent unit at all. Measured on the first adopter,
        // 2026-08-27: one label was moved, a tool still selecting on the other
        // counted four records its own queue could not act on, and would have
        // spent every remaining review before reporting it had not converged.
        // Two other tools in the same lane filtered on the full tuple and were
        // right — the disagreement between them is what surfaced it.
        //
        // ⚠ Stated as arithmetic on what the definition declares, NOT as a
        // policy about which labels belong together: this engine has no opinion
        // on that, and some deployments will have genuinely independent facets.
        // What is true either way is that an untouched label keeps naming the
        // old unit.
        let address: Vec<&str> = match map {
            Some(m) => m.scope_fields.iter().map(String::as_str).collect(),
            None => {
                let mut seen: Vec<&str> = Vec::new();
                for r in &def.rescopes {
                    if !seen.contains(&r.label.as_str()) {
                        seen.push(r.label.as_str());
                    }
                }
                seen
            }
        };
        if address.len() > 1 {
            let joined =
                address.iter().map(|l| format!("'{l}'")).collect::<Vec<_>>().join(" + ");
            let _ = writeln!(
                out,
                "\n  ⚠ a record's unit of work is the TUPLE {joined}, not any one of them.\n  \
                 Moving a subset leaves the rest naming the old unit, and every query that\n  \
                 filters on an untouched label still finds the record there. Set them\n  \
                 together in one rescope, or say why the record belongs in both."
            );
        }
    }

    if def.counters.is_empty() {
        let _ = writeln!(out, "\nthis definition asserts no numbers.");
        // ⚠ Falls through rather than returning. This used to return here,
        // which would have skipped every section added after it — and a
        // section that silently does not print for some inputs is the exact
        // shape of defect the columns section below exists to warn about.
        explain_refereed_columns(&mut out, map);
        return out;
    }
    let _ = writeln!(out, "\n⚠ NUMBERS THIS DEFINITION ASSERTS — and what to search for:");
    for c in &def.counters {
        let _ = writeln!(
            out,
            "  {} = {}   (spent, then routes to '{}'{})",
            c.name,
            c.max,
            c.on_exhausted,
            if c.exhausted_requires_note { "; the spending attempt must say why" } else { "" }
        );
        // ⚠ Saturating, not `+ 1`. The ceiling is a number out of a file
        // somebody else wrote, and a maximal one made this panic in a debug
        // build — in the subcommand whose whole audience is a person who has
        // not got the system working yet. At the top of the range there is no
        // next number, and saying so twice beats crashing.
        let neighbour = c.max.saturating_add(1);
        let _ = writeln!(
            out,
            "      search your tree for {} AND for {} — a ceiling of {} usually means {} \
             rounds counting the first, and that derived number is the one that hides",
            c.max, neighbour, c.max, neighbour
        );
    }
    let _ = writeln!(
        out,
        "\n  Derived arithmetic does not contain the value it came from, so a search for\n  \
         the ceiling alone will not find it. Check guards, --help text, defaults,\n  \
         diagrams, and any prose handed to an actor: a refusal announces itself when it\n  \
         fires, a brief never does."
    );
    explain_refereed_columns(&mut out, map);
    out
}

/// The columns the referee owns, and the sweep to run before closing them.
///
/// ⚠ **Same argument as the numbers section, one layer out.** There the engine
/// owns a ceiling and knows nothing about the arithmetic derived from it; here
/// it owns a set of columns and knows nothing about who writes them. Turning on
/// `guard_refereed_fields` closes those columns to every writer at once, and
/// the adopter is the only party who can enumerate the writers. What this can
/// do is hand over the terms to enumerate *with* — which is the artifact that
/// was missing all three times a writer was missed.
///
/// ⚠⚠ **The third instance is why the sweep says "and your prompts".** An
/// adopter enumerated four scripted call sites, a second party checked that
/// enumeration, and the guard's first refusal came from none of them: prose in
/// a persona telling an agent to move the state column with a generic
/// record-mutation tool. No call site, no import, and **no authentication step
/// to grep for** — the tool server had already authenticated. Two correct
/// passes over the same population, and the writer was never in it. The third
/// was worse still: a machine-wide skill file, loaded by sessions with no lane
/// persona and therefore no fallback to stumble onto.
fn explain_refereed_columns(out: &mut String, map: Option<&ferrostep_pocketbase::CollectionMap>) {
    let Some(map) = map else {
        return;
    };
    let fields = map.refereed_fields();
    let route = map.apply_route();
    let _ = writeln!(out, "\n⚠ COLUMNS THIS REFEREE OWNS in '{}':", map.records);
    let _ = writeln!(out, "  {}", fields.join(", "));
    if map.guard_refereed_fields {
        let _ = writeln!(
            out,
            "\n  guard_refereed_fields is ON — these move through {route} or they do not\n  \
             move. Anything still writing them directly is failing NOW, and reporting it\n  \
             as the store refusing writes."
        );
    } else {
        let _ = writeln!(
            out,
            "\n  guard_refereed_fields is OFF — any writer holding credentials can still\n  \
             edit these directly, and the referee never hears about it. The sweep below\n  \
             is what to run before you turn it on."
        );
    }
    let _ = writeln!(
        out,
        "\n  Search your tree for each column name AND for '{}' — code AND PROSE.\n  \
         A persona, a brief, a skill file or a README that tells an actor to write one\n  \
         of these IS a write path: no call site, no import, and no authentication step,\n  \
         because whatever tool it names authenticated already. Enumerating the code\n  \
         finds every writer except those, and reports itself complete.",
        map.records
    );
    let _ = writeln!(
        out,
        "\n  ⚠ Three kinds this sweep will NOT find, so decide about them by reading:\n  \
           • prose naming neither a column nor a tool — \"update its status in the\n    \
             tracker\" is a write path and matches nothing;\n  \
           • files loaded outside the loop — a machine-wide skill or an editor rule\n    \
             reaches sessions that never see your personas;\n  \
           • an actor improvising a direct write because nothing told it not to.\n  \
         The first two are found by reading every file that instructs an actor. The\n  \
         third is what the guard is for."
    );
    let _ = writeln!(
        out,
        "\n  ⚠⚠ Read the FALLBACKS in those files before you flip this. An agent reports\n  \
         what it concluded, not what it read: one told 'if the tracker is unreachable,\n  \
         put the findings in your summary' can answer a refused column by discarding a\n  \
         finished review. A file with no fallback at all fails worse — it has nothing\n  \
         to fall back TO. Point them at {route} first; flip second."
    );
}

/// Resolve one roster entry into shell assignments the caller `eval`s.
///
/// ⚠ **Every failure here is a non-zero exit with a message**, never an
/// empty assignment at status zero. A caller that `eval`s an empty
/// `AGENT_NAME` and commits with it has silently signed the work as whoever
/// the repo is configured for — which is the failure the roster exists to
/// end, arriving through the roster itself.
fn agent_env(flags: &Flags) -> Result<String, String> {
    let roster = match flags.get("roster") {
        Some(path) => Roster::load(path),
        None => Roster::discover_from_cwd(),
    }
    .map_err(|e| e.to_string())?;
    let agent = roster.resolve(flags.get("agent")).map_err(|e| e.to_string())?;
    match flags.get("format").unwrap_or("shell") {
        "shell" => agent.shell_assignments().map_err(|e| e.to_string()),
        // For a caller that is not a shell. Parsing shell quoting in another
        // language to recover a value the emitter had in hand is a decoding
        // step that can go wrong, in a caller that only wanted a name.
        "json" => {
            let persona = agent.require_persona_file().map_err(|e| e.to_string())?;
            let mut out = serde_json::json!({
                "title": agent.title(),
                "name": agent.name(),
                "email": agent.email(),
                "persona": persona.to_string_lossy(),
                "roster": agent.roster().source().to_string_lossy(),
            });
            // ⚠ The credential SOURCE, never the credential — see
            // `Resolved::shell_assignments`. This format is also the one that
            // needs no environment at all: a caller reading it from a pipe
            // gets the answer without exporting anything, so nothing can be
            // inherited by a subprocess acting as a different actor.
            if let Some(auth) = agent.roster().auth() {
                out["auth"] = serde_json::json!({
                    "type": auth.kind(),
                    "path": auth.path().to_string_lossy(),
                });
            }
            Ok(out.to_string())
        }
        other => Err(format!("--format takes shell or json, got '{other}'")),
    }
}

fn load_engine(path: &str) -> Result<Engine, String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read workflow '{path}': {e}"))?;
    let def = WorkflowDef::from_json(&source)
        .map_err(|e| format!("workflow '{path}' does not parse: {e}"))?;
    Engine::new(def).map_err(|e| format!("workflow '{path}' does not validate: {e}"))
}

/// One reader for the mapping file, because two subcommands now want it and
/// only one of them opens a store.
/// ⚠⚠ **A rescope moves the label it is given, and the others then lie.**
/// A record's unit of work is the whole tuple of scope labels, and every query
/// that finds work filters on it — so setting one label and leaving the rest
/// does not relocate the record, it leaves it in no consistent unit at all.
///
/// Measured on the first adopter, 2026-08-27: one label moved, a tool still
/// selecting on the other counted four records its own queue could not act on,
/// and would have spent every remaining review before reporting it had not
/// converged. Two other tools in that lane filtered on the full tuple and were
/// right; the disagreement between them is what surfaced it.
///
/// ⚠ **A warning, not a refusal, deliberately.** Their partial move was
/// legitimate at the time — there was no value for the other label yet — so
/// refusing would have gone red on correct behaviour, which is how a guard gets
/// switched off. Whether a definition should declare labels as one address, and
/// refuse then, is open and belongs with the satisfiability check.
fn partial_rescope_warning(
    map: Option<&ferrostep_pocketbase::CollectionMap>,
    sets: &[String],
) -> Option<String> {
    let map = map?;
    let touched: Vec<&str> = sets.iter().filter_map(|s| s.split_once('=').map(|(l, _)| l)).collect();
    if !map.scope_fields.iter().any(|l| touched.contains(&l.as_str())) {
        return None;
    }
    let untouched: Vec<String> = map
        .scope_fields
        .iter()
        .filter(|l| !touched.contains(&l.as_str()))
        .map(|l| format!("'{l}'"))
        .collect();
    if untouched.is_empty() {
        return None;
    }
    Some(format!(
        "\n\n⚠ {} still names the OLD unit of work. A record's unit is the whole tuple, so \
         this record is now in neither: a query filtering on an untouched label still finds \
         it where it was. Set them together, or say why it belongs in both.",
        untouched.join(", ")
    ))
}

fn load_map(path: &str) -> Result<ferrostep_pocketbase::CollectionMap, String> {
    let source =
        std::fs::read_to_string(path).map_err(|e| format!("cannot read map '{path}': {e}"))?;
    serde_json::from_str(&source).map_err(|e| format!("map '{path}' does not parse: {e}"))
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
                ferrostep_pocketbase::PocketBaseLedger::connect_mapped(url, &token, load_map(path)?)
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

fn waiting_reason(
    engine: &Engine,
    record: &Record,
    status: &Status,
    role: Option<&str>,
) -> &'static str {
    match status {
        Status::WillEscalate => "every remaining move would escalate",
        Status::NeedsPerson if engine.def().halted.contains(&record.snapshot.state) => {
            "paused; a person decides what happens next"
        }
        Status::NeedsPerson => "only a person can act",
        // ⚠ Selected BECAUSE this role has a move, so the person-shaped
        // reason would be both true and useless here: a record waiting on a
        // developer does not await a person, and saying so is not the answer
        // to the question that was asked.
        Status::Live if role.is_some() => "this actor's turn",
        _ => "does not await a person",
    }
}

/// Which records this invocation is about, and what to call them.
///
/// ⚠ **Two different questions, and the second one had no surface at all.**
/// Without `--role` this answers "does a person need to act", which is what
/// B2 was built for. With one it answers "what is waiting on *this* actor" —
/// including a non-human one, whose queue was invisible: a record handed from
/// a reviewer back to a developer reads as `Status::Live`, so it appeared in
/// no listing and raised no notification. In a worker/reviewer loop that is
/// the ordinary handover, not an edge case.
fn select_waiting<'a>(
    engine: &Engine,
    rows: &'a [(Record, Status)],
    role: Option<&str>,
) -> (Vec<&'a (Record, Status)>, String) {
    match role {
        Some(role) => (
            rows.iter().filter(|(r, _)| engine.awaits(&r.snapshot, role)).collect(),
            format!("await '{role}'"),
        ),
        None => (
            rows.iter()
                .filter(|(_, s)| matches!(s, Status::NeedsPerson | Status::WillEscalate))
                .collect(),
            "await a person".to_string(),
        ),
    }
}

fn render_awaiting(engine: &Engine, rows: &[(Record, Status)], role: Option<&str>) -> String {
    let def = engine.def();
    let (waiting, whom) = select_waiting(engine, rows, role);
    let mut out = String::new();
    // The population is named so an empty answer reads as "checked and none"
    // rather than "checked nothing".
    let _ = writeln!(
        out,
        "{} of {} open record(s) {whom} in '{}'",
        waiting.len(),
        rows.len(),
        def.name
    );
    // Whose options to render: the role that asked, or every person when
    // nobody did.
    let audience: Vec<&str> = match role {
        Some(role) => vec![role],
        None => def.human_roles().collect(),
    };
    for (record, status) in waiting {
        let _ = writeln!(
            out,
            "\n  record {} — {} ({})",
            record.id.0,
            record.snapshot.state,
            waiting_reason(engine, record, status, role)
        );
        for counter in &def.counters {
            let held = record.snapshot.counters.get(&counter.name).copied().unwrap_or(0);
            let _ = writeln!(out, "    {}: {held} of {} spent", counter.name, counter.max);
        }
        for role in audience.iter().copied() {
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

/// Whether an event moved the record from one state to another.
///
/// ⚠ **Not every event does, and reading a history as if they all did is how
/// a report invents things that never happened.** A rescope is
/// `Allow { to: <the state the record is already in> }` — it moves the record
/// between units of work, and the record stays exactly where it was. It is
/// therefore neither an arrival nor a departure, and the two tallies below
/// counted it as both: rescoping a paused record reported an escalation *and*
/// a release, which is a plausible story about a record that did not move.
///
/// Asking here rather than testing for scope updates keeps this about the
/// question the tallies actually mean. Any other event that lands a record
/// where it already was is the same non-move, whether or not scope is why.
fn changed_state(event: &Event) -> bool {
    let landed = match &event.decision {
        Decision::Allow { to, .. } | Decision::Exhausted { to, .. } => Some(to.as_str()),
        // A denial persists nothing, so it never reaches a history at all.
        Decision::Deny { .. } => None,
    };
    // A filed record came from nowhere and arrived somewhere, which is a move.
    landed.is_some_and(|to| event.from_state.as_deref() != Some(to))
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
        // a spent ceiling routing there, or an actor sending it there. A
        // release is the departure. Both ask `changed_state` first, because
        // an event in a history is not necessarily a move.
        let escalations = history
            .iter()
            .filter(|e| changed_state(&e.event))
            .filter(|e| match &e.event.decision {
                Decision::Exhausted { .. } => true,
                Decision::Allow { to, .. } => def.halted.contains(to),
                Decision::Deny { .. } => false,
            })
            .count();
        let releases = history
            .iter()
            .filter(|e| changed_state(&e.event))
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

/// File a new record into `scope`.
///
/// The zero-install path is why this exists. A store with a console of its
/// own answers "put a record here" without the referee ever being asked, and
/// SQLite has no console to hide behind — so without this, the deployment
/// shape the roadmap calls first-class had no way to get a first record in
/// at all, short of writing a program.
///
/// ⚠ **A filing ceiling is measured against a number this tool cannot
/// take.** `creation.spends` bounds a *population* — how much work a branch
/// or a cycle may create — and that count belongs to whatever can count it,
/// never to the record being filed; the adapters deliberately do not store it
/// on the new record, and the ledger interface has no operation that reads
/// it back. Defaulting it to zero would be the worst available answer: every
/// filing ceiling would silently never fire, which is a guard reporting
/// success having checked nothing. So the caller supplies it, and a missing
/// one is a refusal that says where the number lives.
fn do_file(
    engine: &Engine,
    ledger: &dyn Ledger,
    scope: &Scope,
    role: &str,
    counters: &[String],
    note: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let def = engine.def();
    let mut measured: BTreeMap<String, u32> = BTreeMap::new();
    for pair in counters {
        let (name, value) = pair
            .split_once('=')
            .ok_or(format!("--counter takes name=value, got '{pair}'"))?;
        let parsed = value
            .parse()
            .map_err(|_| format!("--counter {name} needs a whole number, got '{value}'"))?;
        measured.insert(name.to_string(), parsed);
    }
    if let Some(creation) = &def.creation {
        let unmeasured: Vec<&str> = creation
            .spends
            .iter()
            .filter(|name| !measured.contains_key(name.as_str()))
            .map(String::as_str)
            .collect();
        if !unmeasured.is_empty() {
            return Err(format!(
                "filing spends {}, and this tool cannot count it for you: a filing ceiling \
                 bounds a branch or a cycle rather than the record being filed, so what it \
                 is measured against is yours to derive. Pass each as \
                 --counter <name>=<value>",
                unmeasured.join(", ")
            ));
        }
    }
    let mut attempt = Attempt::new(role, &def.initial);
    if let Some(text) = note {
        attempt = attempt.saying(text);
    }
    let decision = engine.authorize_create(&attempt, &measured);
    match &decision {
        Decision::Deny { reason } => return Err(format!("refused: {reason}")),
        // No record is created: the matter escalates, and there is nothing
        // yet to carry it. Saying which ceiling and where it routes is the
        // whole content of that refusal.
        Decision::Exhausted { to, counter } => {
            return Err(format!(
                "'{counter}' is spent: nothing was filed, and the definition routes that \
                 to '{to}'"
            ));
        }
        Decision::Allow { .. } => {}
    }
    let event = Event {
        actor: actor.to_string(),
        role: role.to_string(),
        // A filed record came from nowhere, which is not the same as coming
        // from a state name that is merely blank.
        from_state: None,
        decision: decision.clone(),
        note: note.map(str::to_string),
    };
    let record = ledger.create(scope, &decision, &event).map_err(|e| e.to_string())?;

    let mut line = format!("record {} filed into '{}'", record.id.0, def.initial);
    if !scope.filters().is_empty() {
        let labels: Vec<String> =
            scope.filters().iter().map(|(k, v)| format!("{k}={v}")).collect();
        let _ = write!(line, " [{}]", labels.join(", "));
    }
    let mut spent = false;
    if let Decision::Allow { counter_updates, .. } = &decision {
        if !counter_updates.is_empty() {
            spent = true;
            let changes: Vec<String> = counter_updates
                .iter()
                .map(|(name, new)| {
                    let old = measured.get(name).copied().unwrap_or(0);
                    format!("{name} {old} → {new}")
                })
                .collect();
            let _ = write!(line, " ({})", changes.join(", "));
        }
    }
    let _ = write!(line, " (version {})", record.version.0);
    // ⚠ Say where that new value went, because it did not go onto a record.
    // An operator who passes the same number next time gets the same answer
    // forever, and a ceiling that never advances is one that never fires.
    // Last, on its own line, so the outcome stays the thing you read first.
    if spent {
        let _ = write!(
            line,
            "\n  ⚠ a filing spend is scope-level: it is in this record's history, not on the \
             record. Derive the next value from there rather than repeating this one."
        );
    }
    Ok(line)
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
        Decision::Allow { to, counter_updates, .. } => {
            // ⚠ Report what the move COST, not just where it went. The
            // hand-rolled tooling this replaces printed the arithmetic
            // ("agent_passes 0 -> 1"), and dropping it made confirming a
            // spend a second read — a regression the first adopter absorbed
            // silently before reporting it. Old and new are both shown, so a
            // spend and a re-arm are told apart by the numbers rather than by
            // a label the engine would have to invent.
            let mut line = format!("record {} moved to '{to}'", record.id.0);
            if !counter_updates.is_empty() {
                let changes: Vec<String> = counter_updates
                    .iter()
                    .map(|(name, new)| {
                        let old = record.snapshot.counters.get(name).copied().unwrap_or(0);
                        format!("{name} {old} → {new}")
                    })
                    .collect();
                let _ = write!(line, " ({})", changes.join(", "));
            }
            line
        }
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

/// Move a record to a different unit of work.
///
/// Separate from `move` because it is a different question — not "where does
/// this record go next" but "which body of work does it belong to" — and
/// because it is the operation that was being done as a raw database write
/// until the referee could answer it.
fn do_rescope(
    engine: &Engine,
    ledger: &dyn Ledger,
    record_id: &str,
    role: &str,
    sets: &[String],
    note: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let mut updates = BTreeMap::new();
    for pair in sets {
        let (label, value) = pair
            .split_once('=')
            .ok_or(format!("--set takes label=value, got '{pair}'"))?;
        updates.insert(label.to_string(), value.to_string());
    }
    let record = ledger.load(&RecordId(record_id.to_string())).map_err(|e| e.to_string())?;
    let decision = engine.authorize_rescope(&record.snapshot, role, &updates, note);
    if let Decision::Deny { reason } = &decision {
        return Err(format!("refused: {reason}"));
    }
    let moved: Vec<String> = updates.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let event = Event {
        actor: actor.to_string(),
        role: role.to_string(),
        // A rescope does not move the record, and saying where it "came from"
        // would read as a state change in the history. It came from, and stays
        // in, the state it is in.
        from_state: Some(record.snapshot.state.clone()),
        decision,
        note: note.map(str::to_string),
    };
    let version = ledger.apply(&record, &event).map_err(|e| e.to_string())?;
    Ok(format!(
        "record {} now {} (version {})",
        record.id.0,
        moved.join(", "),
        version.0
    ))
}

/// Move one graded attribute along its ladder.
///
/// ⚠ **One attribute per invocation, deliberately.** A rescope takes several
/// labels at once because a record's unit of work is the whole tuple and
/// moving part of it leaves the rest lying — the defect `explain` now warns
/// about. A grade is the opposite: each attribute has its own ladder, its own
/// directions and its own grants, so batching them would let one refusal
/// silently decide the fate of the others.
fn do_grade(
    engine: &Engine,
    ledger: &dyn Ledger,
    record_id: &str,
    role: &str,
    // ⚠ Paired rather than passed separately, to stay under the argument
    // ceiling this file already holds itself to — the same reason
    // `partial_rescope_warning` was extracted out of `do_rescope`. The pair is
    // meaningful anyway: an attribute without a target value is not a request.
    change: (&str, &str),
    note: Option<&str>,
    actor: &str,
) -> Result<String, String> {
    let (attribute, value) = change;
    let record = ledger.load(&RecordId(record_id.to_string())).map_err(|e| e.to_string())?;
    let held = record.snapshot.grades.get(attribute).cloned();
    let decision = engine.authorize_grade(&record.snapshot, role, attribute, value, note);
    if let Decision::Deny { reason } = &decision {
        return Err(format!("refused: {reason}"));
    }
    let event = Event {
        actor: actor.to_string(),
        role: role.to_string(),
        // A grade change does not move the record, so its history says it came
        // from — and stays in — the state it is in. Same reasoning as a
        // rescope: a state change in the history that never happened is worse
        // than no entry at all.
        from_state: Some(record.snapshot.state.clone()),
        decision,
        note: note.map(str::to_string),
    };
    let version = ledger.apply(&record, &event).map_err(|e| e.to_string())?;
    // ⚠ The report says where it came FROM, because "severity is now high" is
    // the same sentence whether it was raised from low or lowered from
    // critical — and those are different acts with different grants.
    let from = match held {
        Some(previous) => format!("{previous} -> "),
        None => String::new(),
    };
    Ok(format!(
        "record {} {attribute} {from}{value} (version {})",
        record.id.0, version.0
    ))
}

/// One notification per awaiting record. Pure assembly; the send is the
/// adapter's.
fn notifications_for(
    engine: &Engine,
    rows: &[(Record, Status)],
    role: Option<&str>,
) -> Vec<Notification> {
    let def = engine.def();
    let (waiting, _) = select_waiting(engine, rows, role);
    waiting
        .into_iter()
        .map(|(record, status)| {
            let spent: Vec<String> = def
                .counters
                .iter()
                .filter_map(|c| {
                    let held = record.snapshot.counters.get(&c.name).copied().unwrap_or(0);
                    (held >= c.max).then(|| format!("{}: {held} of {} spent", c.name, c.max))
                })
                .collect();
            let mut reason = waiting_reason(engine, record, status, role).to_string();
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
    role: Option<&str>,
) -> Result<String, String> {
    let notifications = notifications_for(engine, rows, role);
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
        Decision::allow(
            to,
            counters.iter().map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>(),
        )
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
        let rendered = render_awaiting(&engine, &rows, None);

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

    /// ⚠⚠ **The reported gap: a review completes and nobody is told.** Both
    /// surfaces filtered on `NeedsPerson | WillEscalate`, so a record handed
    /// back to an agent — `Status::Live` — appeared in no listing and raised
    /// no notification. The actor whose turn it now was had no way to find
    /// out except by asking the database directly, which is the thing these
    /// surfaces exist to replace.
    #[test]
    fn an_agents_queue_is_visible_and_notified_not_only_a_persons() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), false).unwrap();

        // The person-scoped question is unchanged, and deliberately blind here.
        let person = render_awaiting(&engine, &rows, None);
        assert!(person.contains("await a person"), "{person}");

        // The worker's own queue: records it can actually claim. `live` and
        // `ended` are seeded in `awaiting_worker`; `ended` has moved on, and
        // `stuck` has its ceiling spent so it is NOT the worker's turn.
        let worker = render_awaiting(&engine, &rows, Some("worker"));
        assert!(worker.contains("await 'worker'"), "{worker}");
        assert!(worker.contains(&format!("record {}", ids["live"].0)), "{worker}");
        assert!(
            !worker.contains(&format!("record {}", ids["stuck"].0)),
            "a spent ceiling is not a turn — it would send an actor to be refused: {worker}"
        );
        // And the reason names the turn rather than reporting the absence of
        // a person, which is true and answers a question nobody asked.
        assert!(worker.contains("this actor's turn"), "{worker}");

        // ⚠ Notification follows the same selection, or the listing and the
        // doorbell disagree about who is waiting.
        let for_person = notifications_for(&engine, &rows, None);
        let for_worker = notifications_for(&engine, &rows, Some("worker"));
        assert!(!for_worker.is_empty(), "an agent's turn must be notifiable");
        assert_ne!(
            for_person.iter().map(|n| &n.record).collect::<Vec<_>>(),
            for_worker.iter().map(|n| &n.record).collect::<Vec<_>>(),
            "the two questions must select different records, or one of them is not being asked"
        );
    }

    #[test]
    fn awaiting_reports_the_population_even_when_nobody_waits() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SqliteLedger::open(dir.path().join("ledger.db")).unwrap();
        let engine = engine();
        let rows = records_with_status(&engine, &ledger, &Scope::all(), false).unwrap();
        let rendered = render_awaiting(&engine, &rows, None);
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
        let notifications = notifications_for(&engine, &rows, None);
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

    /// The reference loop plus permission to move a record between units of
    /// work, which the shipped example deliberately does not grant.
    /// ⚠⚠ **A FLAG THE HELP TEXT NEVER MENTIONS IS A FLAG NOBODY RUNS.** This
    /// workspace spent a day on the same shape one layer up: a 469-line driver
    /// that was named in three files, in the third person, and driven by hand
    /// for six reviews because **no line ever showed a reader a command they
    /// could copy.** The string being present is not the affordance.
    ///
    /// ⚠ The population is derived from `accepted_flags`, so adding a
    /// subcommand or a flag arms this check by existing — nobody has to
    /// remember a list. `ntfy-token` is the one exemption and it is named
    /// rather than filtered by a pattern, so an exemption cannot grow silently.
    #[test]
    fn every_accepted_flag_is_named_in_the_usage_text() {
        const COMMANDS: [&str; 8] =
            ["awaiting", "audit", "file", "create", "move", "rescope", "notify", "explain"];
        let mut missing: Vec<String> = Vec::new();
        let mut checked = 0usize;
        for command in COMMANDS {
            for flag in accepted_flags(command) {
                if *flag == "ntfy-token" {
                    continue; // documented as part of `--ntfy`'s paragraph.
                }
                checked += 1;
                if !USAGE.contains(&format!("--{flag}")) {
                    missing.push(format!("{command}: --{flag}"));
                }
            }
        }
        // ⚠ Floor FIRST. `accepted_flags` returning empty slices would leave
        // `missing` empty and this test green while checking nothing — the
        // vacuous pass this repo keeps re-finding.
        assert!(checked >= 20, "only {checked} flags enumerated; the listing is broken");
        assert!(missing.is_empty(), "accepted but undocumented: {missing:?}");
    }

    /// ⚠⚠ The case that cost a real loop a cycle: one label moved, the other
    /// left naming the old unit, and a tool filtering on the untouched one
    /// counting records its own queue could not act on.
    #[test]
    fn a_rescope_that_moves_part_of_the_address_says_what_it_left_behind() {
        let map = map_with_scope(&["repo", "branch"]);
        let w = partial_rescope_warning(Some(&map), &[String::from("repo=other/thing")])
            .expect("a partial move must warn");
        assert!(w.contains("'branch'"), "does not name what was left: {w}");
        assert!(!w.contains("'repo'"), "names the label that DID move: {w}");
    }

    /// The negative controls, and each is a distinct way to be wrong.
    #[test]
    fn a_whole_address_a_single_label_and_an_unrelated_set_stay_quiet() {
        let two = map_with_scope(&["repo", "branch"]);
        // Whole tuple moved together — the thing the warning asks for.
        assert!(
            partial_rescope_warning(
                Some(&two),
                &[String::from("repo=x"), String::from("branch=y")]
            )
            .is_none(),
            "warned about a complete move"
        );
        // One label cannot be out of step with itself.
        let one = map_with_scope(&["branch"]);
        assert!(
            partial_rescope_warning(Some(&one), &[String::from("branch=y")]).is_none(),
            "warned about a single-label address"
        );
        // ⚠ A set touching NO scope label is not a partial move of the address.
        assert!(
            partial_rescope_warning(Some(&two), &[String::from("unrelated=z")]).is_none(),
            "warned when the address was not touched at all"
        );
        // No map means no address to reason about; silence is the honest answer.
        assert!(partial_rescope_warning(None, &[String::from("repo=x")]).is_none());
        // ⚠ Floor: the positive case must still fire, or every assertion above
        // is satisfied by a function that never warns.
        assert!(partial_rescope_warning(Some(&two), &[String::from("repo=x")]).is_some());
    }

    fn flags_of(pairs: &[(&str, &str)]) -> Flags {
        let mut argv: Vec<String> = Vec::new();
        for (k, v) in pairs {
            argv.push(format!("--{k}"));
            argv.push(v.to_string());
        }
        Flags::parse(&argv).expect("fixture flags must parse")
    }

    /// ⚠ A reason containing backticks or quotes cannot go on a command line
    /// without a heredoc, and the first adopter hit that posting the comment
    /// that reported the same defect in their own tooling.
    #[test]
    fn a_note_can_come_from_a_file_and_keeps_what_a_shell_would_have_eaten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        let awkward = "the `if` at 209 — \"quoted\", $HOME, and a trailing newline\n";
        std::fs::write(&path, awkward).unwrap();
        let note = note_text(&flags_of(&[("note-file", path.to_str().unwrap())])).unwrap();
        assert_eq!(note.as_deref(), Some(awkward.trim_end()));
    }

    /// ⚠⚠ Both set is a refusal. Silently preferring one records a reason the
    /// caller did not write, and which one a reader would guess is not
    /// something to leave to a reader.
    #[test]
    fn a_note_given_twice_is_refused_rather_than_resolved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        std::fs::write(&path, "from the file").unwrap();
        let err = note_text(&flags_of(&[
            ("note", "from the flag"),
            ("note-file", path.to_str().unwrap()),
        ]))
        .expect_err("both set must refuse");
        assert!(err.contains("both set"), "{err}");
    }

    /// ⚠⚠ The load-bearing refusals. A missing path or an empty file resolving
    /// to "no note" makes the engine refuse a required-note move for the wrong
    /// cause — a true message pointing at the definition when the fault is the
    /// path — or lets an optional-note move land with its reason dropped.
    #[test]
    fn an_unreadable_or_empty_note_file_is_refused_and_names_the_path() {
        let missing = note_text(&flags_of(&[("note-file", "/nonexistent/note.md")]))
            .expect_err("a missing file must refuse");
        assert!(missing.contains("/nonexistent/note.md"), "does not name the path: {missing}");

        let dir = tempfile::tempdir().unwrap();
        let blank = dir.path().join("blank.md");
        std::fs::write(&blank, "   \n\t\n").unwrap();
        let err = note_text(&flags_of(&[("note-file", blank.to_str().unwrap())]))
            .expect_err("whitespace is not a reason");
        assert!(err.contains("is empty"), "{err}");

        // ⚠ Floor: the happy path must still work, or every assertion above is
        // satisfied by a resolver that refuses everything.
        let ok = dir.path().join("ok.md");
        std::fs::write(&ok, "a real reason").unwrap();
        assert_eq!(
            note_text(&flags_of(&[("note-file", ok.to_str().unwrap())])).unwrap().as_deref(),
            Some("a real reason")
        );
    }

    // ------------------------------------------------------------------
    // doctor
    // ------------------------------------------------------------------

    /// A map with **one column of every kind**, deliberately. A kind the
    /// fixture leaves empty is a kind these tests cannot see.
    fn doctor_map() -> ferrostep_pocketbase::CollectionMap {
        ferrostep_pocketbase::CollectionMap {
            records: "tickets".to_string(),
            events: "ticket_events".to_string(),
            state_field: "stage".to_string(),
            version_field: "fs_version".to_string(),
            counter_fields: vec!["agent_passes".to_string()],
            scope_fields: vec!["branch".to_string()],
            attribute_fields: vec!["severity".to_string()],
            guard_refereed_fields: false,
        }
    }

    /// A store that agrees with [`doctor_map`] and with `review-loop.json`.
    fn agreeing_shape() -> StoreShape {
        let def = engine();
        StoreShape {
            subject: "tickets".to_string(),
            accepted_states: Answer::Said(def.def().states.clone()),
            columns: Answer::Said(BTreeMap::from([
                ("stage".to_string(), "select".to_string()),
                ("fs_version".to_string(), "number".to_string()),
                ("agent_passes".to_string(), "number".to_string()),
                ("branch".to_string(), "text".to_string()),
                ("severity".to_string(), "text".to_string()),
            ])),
            // ⚠ One entry per column that exists, matching `columns` above:
            // the state column enumerates its values, the rest do not. A
            // column missing from here is one the store does not have.
            accepted_values: Answer::Said(BTreeMap::from([
                ("stage".to_string(), Answer::Said(def.def().states.clone())),
                ("fs_version".to_string(), Answer::NothingToConstrain),
                ("agent_passes".to_string(), Answer::NothingToConstrain),
                ("branch".to_string(), Answer::NothingToConstrain),
                ("severity".to_string(), Answer::NothingToConstrain),
            ])),
            writable: Answer::Said(BTreeMap::from([
                ("counters".to_string(), vec!["agent_passes".to_string()]),
                ("scope".to_string(), vec!["branch".to_string()]),
                ("attributes".to_string(), vec!["severity".to_string()]),
            ])),
        }
    }

    /// The review loop plus a ladder on the column [`doctor_map`] already
    /// referees, which is the shape an adopter reaches after declaring a
    /// graded column.
    fn graded_engine() -> Engine {
        let mut def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        def.grades = vec![ferrostep_core::GradeDef {
            attribute: "severity".to_string(),
            ladder: vec!["low".to_string(), "high".to_string(), "critical".to_string()],
            raise: vec!["worker".to_string()],
            lower: vec!["reviewer".to_string()],
            requires_note: false,
        }];
        Engine::new(def).unwrap()
    }

    /// [`agreeing_shape`] with the graded column stating the values given.
    fn shape_accepting(severity: Answer<Vec<String>>) -> StoreShape {
        let mut shape = agreeing_shape();
        if let Answer::Said(by_column) = &mut shape.accepted_values {
            by_column.insert("severity".to_string(), severity);
        }
        if let Answer::Said(columns) = &mut shape.columns {
            columns.insert("severity".to_string(), "select".to_string());
        }
        shape
    }

    /// ⚠⚠ **THE SAME FAULT `doctor` WAS BUILT FOR, ONE COLUMN OVER.** The
    /// entry that created this tool was a definition naming a state value the
    /// select column would refuse: it passed every check and failed at the
    /// first transition. A ladder value the column would refuse is that fault
    /// exactly, and it would have failed at the first grade — while the
    /// instrument built to catch it reported clean, because it checked
    /// accepted values for one column and not for the structurally identical
    /// other one.
    #[test]
    fn a_ladder_value_the_column_would_refuse_is_a_fault() {
        let engine = graded_engine();
        let shape = shape_accepting(Answer::Said(vec!["low".to_string(), "high".to_string()]));
        let report = doctor_report(engine.def(), Some(&doctor_map()), &Ok(shape))
            .expect_err("a ladder value the store refuses is a fault");
        assert!(report.contains("critical"), "the refused value is named: {report}");
        assert!(report.contains("severity"), "and its ladder: {report}");
        // ⚠ The two values the column DOES accept must not be reported. A
        // check that faults on the whole ladder is as useless as none.
        assert!(!report.contains("'low'"), "'low' is accepted: {report}");
    }

    /// The other side, and the one that stops the check above from being
    /// satisfied by faulting on every ladder. ⚠ It also asserts the NOTE: a
    /// value the column accepts and no ladder uses is reachable only by a
    /// writer going around the referee, which an adopter should be told.
    #[test]
    fn a_ladder_the_column_accepts_is_agreed_and_its_spare_values_noted() {
        let engine = graded_engine();
        let shape = shape_accepting(Answer::Said(vec![
            "low".to_string(),
            "high".to_string(),
            "critical".to_string(),
            "cosmetic".to_string(),
        ]));
        let report = doctor_report(engine.def(), Some(&doctor_map()), &Ok(shape))
            .expect("every ladder value is accepted");
        assert!(report.contains("cosmetic"), "the spare value is named: {report}");
        assert!(
            report.contains("all 3 values of the 'severity' ladder"),
            "the agreement is counted and shown: {report}"
        );
    }

    /// ⚠⚠ **AN INSTALLED FILE THAT NEVER ENUMERATED ITS COLUMNS' VALUES IS
    /// UNCHECKED, NOT CLEAN.** This is the ordinary case, not the exotic one:
    /// the `values` key is newer than the schema route, which is newer than
    /// the hooks most deployments are running. Reporting it as a pass is
    /// exactly the failure `Answer` exists to make unspellable.
    #[test]
    fn a_store_that_never_said_leaves_the_ladder_unchecked_rather_than_clean() {
        let engine = graded_engine();
        let mut shape = agreeing_shape();
        shape.accepted_values = Answer::Unknown;
        let report = doctor_report(engine.def(), Some(&doctor_map()), &Ok(shape))
            .expect_err("unchecked is not a pass");
        assert!(report.contains("0 fault(s)"), "the premise: no faults: {report}");
        assert!(report.contains("1 unchecked"), "{report}");
        assert!(report.contains("ladder"), "and it says which question: {report}");
    }

    /// ⚠ A column that does not enumerate its values cannot refuse anything,
    /// so a ladder against it is a real all-clear rather than an unchecked
    /// one — the distinction the whole nested `Answer` exists for. Without
    /// this, the honest text-column case would read as a failure to check.
    #[test]
    fn a_column_that_enumerates_nothing_cannot_refuse_a_ladder_value() {
        let engine = graded_engine();
        let report =
            doctor_report(engine.def(), Some(&doctor_map()), &Ok(agreeing_shape()))
                .expect("a text column refuses nothing, so nothing is faulted");
        assert!(report.contains("0 fault(s), 0 unchecked"), "{report}");
        assert!(
            report.contains("does not enumerate"),
            "but the reader is told WHY it passed: {report}"
        );
    }

    /// The floor under every other test here: when everything agrees, this
    /// passes and says so. Without it, a `doctor` that reported a fault on all
    /// input would satisfy the negative tests below.
    #[test]
    fn a_definition_its_store_agrees_with_passes_and_states_what_it_checked() {
        let engine = engine();
        let report =
            doctor_report(engine.def(), Some(&doctor_map()), &Ok(agreeing_shape())).expect("clean");
        assert!(report.contains("0 fault(s), 0 unchecked"), "{report}");
        // ⚠ Not merely "no complaints": the agreements are counted and shown,
        // because a run that checked nothing also has no complaints.
        let agreed: usize = report
            .rsplit_once("unchecked, ")
            .and_then(|(_, tail)| tail.split_whitespace().next())
            .and_then(|n| n.parse().ok())
            .expect("the summary states an agreed count");
        assert!(agreed >= 8, "expected the checks to actually run, got {agreed}: {report}");
    }

    /// ⚠⚠ **THE FAILURE THIS COMMAND WAS BUILT FOR**, reported by the first
    /// adopter at the moment of impact, 2026-08-27: a state added to a
    /// definition whose store keeps its state column as a fixed value list.
    /// Every transition into it would have been refused on the wire, and the
    /// only thing that prevented it was the adopter happening to patch the
    /// store by hand first, in the right order.
    #[test]
    fn a_state_the_store_would_refuse_is_a_fault_that_names_the_state() {
        let mut def = engine().def().clone();
        def.states.push("disputed".to_string());
        let mut shape = agreeing_shape();
        // The store is unchanged — which is the whole scenario.
        shape.accepted_states = Answer::Said(engine().def().states.clone());

        let report = doctor_report(&def, Some(&doctor_map()), &Ok(shape))
            .expect_err("a state the store refuses is a fault");
        assert!(report.contains("'disputed'"), "the fault must name the state: {report}");
        assert!(report.contains("would be refused"), "{report}");
        assert!(report.contains("1 fault(s)"), "{report}");
    }

    /// A ceiling that cannot be spent is not a ceiling. This one needs no
    /// store at all, which is why it still runs when the store cannot answer.
    #[test]
    fn a_counter_with_no_column_is_a_fault_before_any_store_is_consulted() {
        let mut map = doctor_map();
        map.counter_fields.clear();

        let unreachable: Result<StoreShape, String> = Err("no store here".to_string());
        let report = doctor_report(engine().def(), Some(&map), &unreachable)
            .expect_err("an unspendable ceiling is a fault");
        assert!(report.contains("agent_passes"), "{report}");
        assert!(report.contains("can never fire"), "{report}");
        // And the store half is reported as unasked rather than omitted.
        assert!(report.contains("never established"), "{report}");
    }

    /// ⚠⚠ **THE ONE THAT USED TO BE SILENT.** A column declared in the map
    /// that the installed file was generated before does not get written — the
    /// defect that cost the first adopter a ceiling reading zero forever. A
    /// file old enough to lack the column allowlist does not even say so: it
    /// drops the column and answers 200.
    #[test]
    fn a_column_the_installed_write_path_cannot_reach_is_a_fault_that_says_it_would_answer_200() {
        let mut shape = agreeing_shape();
        shape.writable = Answer::Said(BTreeMap::from([
            ("counters".to_string(), Vec::new()),
            ("scope".to_string(), vec!["branch".to_string()]),
            ("attributes".to_string(), vec!["severity".to_string()]),
        ]));

        let report = doctor_report(engine().def(), Some(&doctor_map()), &Ok(shape))
            .expect_err("an unreachable column is a fault");
        assert!(report.contains("agent_passes"), "{report}");
        assert!(report.contains("answers 200"), "{report}");
        assert!(report.contains("Regenerate"), "the fault must say what to do: {report}");
    }

    /// ⚠⚠ **AN UNANSWERED QUESTION MUST NOT PASS.** This is the rule most
    /// likely to be softened by somebody tidying up a noisy report, so it is
    /// asserted on the *verdict* and not only on the text: a shape that knows
    /// nothing produces **zero faults**, and must still fail.
    #[test]
    fn a_store_that_answered_nothing_fails_even_though_it_reported_no_faults() {
        let knows_nothing = StoreShape {
            subject: "tickets".to_string(),
            ..Default::default()
        };
        let report = doctor_report(engine().def(), Some(&doctor_map()), &Ok(knows_nothing))
            .expect_err("nothing verified is not a pass");
        assert!(report.contains("0 fault(s)"), "the premise: no faults were found: {report}");
        assert!(report.contains("3 unchecked"), "{report}");
        assert!(report.contains("not a passing one"), "{report}");
    }

    /// The other side of that line, and the reason [`Answer`] has three
    /// variants: a store that says it constrains nothing has **answered**, and
    /// a definition cannot disagree with it. Reporting that as unchecked would
    /// make `doctor` fail permanently on the zero-install path.
    #[test]
    fn a_store_that_constrains_nothing_passes_because_that_is_an_answer() {
        let unconstrained = StoreShape {
            subject: "ferrostep_records".to_string(),
            accepted_states: Answer::NothingToConstrain,
            columns: Answer::Said(BTreeMap::from([
                ("state".to_string(), "text".to_string()),
                ("counters".to_string(), "text".to_string()),
            ])),
            // The SQLite shape: this adapter owns the DDL, which declares no
            // enumerated type, so every column taking any value is a checked
            // fact rather than a shrug.
            accepted_values: Answer::Said(BTreeMap::from([
                ("state".to_string(), Answer::NothingToConstrain),
                ("counters".to_string(), Answer::NothingToConstrain),
            ])),
            writable: Answer::NothingToConstrain,
        };
        let report = doctor_report(engine().def(), None, &Ok(unconstrained))
            .expect("an unconstrained store is a pass, not an unknown");
        assert!(report.contains("0 fault(s), 0 unchecked"), "{report}");
        assert!(report.contains("constrains nothing"), "{report}");
    }

    /// ⚠⚠ **A REPORT THAT SKIPS A KIND OF COLUMN IS A CLEAN REPORT.** The
    /// checker walks the map's fields by kind so it can say whether 'severity'
    /// is a counter or an attribute; `refereed_fields` walks the same fields
    /// with the kinds flattened away. Two walks over one structure is exactly
    /// the shape that goes stale in the silent direction — a kind added to the
    /// map reaches the guard, and the report keeps passing because it never
    /// looked at it.
    ///
    /// The fixture holds one column of every kind on purpose: this assertion
    /// is only as wide as the fixture is.
    #[test]
    fn the_report_covers_every_column_kind_the_referee_owns() {
        let map = doctor_map();
        let by_kind: std::collections::BTreeSet<String> =
            refereed_by_kind(&map).into_iter().map(|(_, name)| name).collect();
        let flattened: std::collections::BTreeSet<String> =
            map.refereed_fields().into_iter().collect();
        assert_eq!(
            by_kind, flattened,
            "every refereed column must be checked, and named with its kind"
        );
        // Floor: the sets agreeing is only meaningful if they are not empty,
        // and the count is the number of kinds the fixture exercises.
        assert_eq!(flattened.len(), 5, "the fixture must hold one column of every kind");
    }

    fn engine_with_grades() -> Engine {
        let mut def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        def.grades = vec![ferrostep_core::GradeDef {
            attribute: "severity".to_string(),
            ladder: vec!["low".to_string(), "medium".to_string(), "high".to_string()],
            raise: vec!["worker".to_string()],
            lower: vec!["reviewer".to_string()],
            requires_note: false,
        }];
        Engine::new(def).unwrap()
    }

    /// The whole point of the subcommand: a grade moves through the referee
    /// rather than through a database console, and the report says which way
    /// it went.
    #[test]
    fn grade_moves_one_attribute_and_says_which_way_it_went() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine_with_grades();
        let id = ids["live"].0.clone();

        let opened =
            do_grade(&engine, &ledger, &id, "worker", ("severity", "medium"), None, "worker").unwrap();
        assert!(opened.contains("severity medium"), "{opened}");
        // ⚠ Opening reports no origin, because there was none — reporting a
        // fabricated "low -> medium" would claim the record had been graded
        // before, which is exactly the distinction the engine keeps.
        assert!(!opened.contains("->"), "an opening grade has nowhere to come from: {opened}");

        let raised =
            do_grade(&engine, &ledger, &id, "worker", ("severity", "high"), None, "worker").unwrap();
        assert!(raised.contains("medium -> high"), "a move states its origin: {raised}");

        // And it landed, rather than being reported and dropped.
        let record = ledger.load(&RecordId(id.clone())).unwrap();
        assert_eq!(record.snapshot.grades["severity"], "high");
    }

    /// ⚠⚠ The refusal a person acts on names the DIRECTION and who holds it.
    /// "role 'worker' may not grade severity" is true and useless when the
    /// worker may raise it and this was a lower.
    #[test]
    fn a_direction_the_role_does_not_hold_is_refused_by_direction() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine_with_grades();
        let id = ids["live"].0.clone();
        do_grade(&engine, &ledger, &id, "worker", ("severity", "high"), None, "worker").unwrap();

        let refused =
            do_grade(&engine, &ledger, &id, "worker", ("severity", "low"), None, "worker")
                .expect_err("the worker holds raise, not lower");
        assert!(refused.contains("lower"), "{refused}");
        assert!(refused.contains("reviewer"), "and names who does hold it: {refused}");

        // ⚠ Floor: the role that does hold it succeeds, or the assertion above
        // is satisfied by a command that refuses everything.
        let allowed =
            do_grade(&engine, &ledger, &id, "reviewer", ("severity", "low"), None, "reviewer")
                .unwrap();
        assert!(allowed.contains("high -> low"), "{allowed}");
    }

    /// ⚠⚠ **THE SENTENCE `explain` MUST CARRY.** An adopter reading only the
    /// grants will assume the familiar shape — that raising is the safe
    /// direction — which is true of a gate with a floor and exactly backwards
    /// for one requiring a minimum. The engine has no opinion, and a surface
    /// that prints the grants without saying so invites the reader to supply
    /// the missing opinion themselves.
    #[test]
    fn explain_prints_the_ladder_and_refuses_to_imply_which_end_is_safe() {
        let out = explain(&engine_with_grades(), None);
        assert!(out.contains("graded attributes:"), "{out}");
        assert!(out.contains("low  ->  medium  ->  high"), "the ladder, in order: {out}");
        assert!(out.contains("raise (toward 'high'): worker"), "{out}");
        assert!(out.contains("lower (toward 'low'): reviewer"), "{out}");
        // ⚠ Both halves. Anchoring only on the tail survived a mutation that
        // deleted "has no opinion" and left the rest — the assertion passed
        // while the load-bearing clause was gone.
        assert!(out.contains("has no opinion"), "the disclaimer's subject: {out}");
        assert!(
            out.contains("about which end of the ladder clears your gate"),
            "the warning that keeps a reader from supplying the missing opinion: {out}"
        );
        // ⚠ A definition with no grades prints no section, rather than an
        // empty heading that reads as "there are none, checked".
        let plain = explain(&engine(), None);
        assert!(!plain.contains("graded attributes:"), "{plain}");
    }

    /// The definition-side check `doctor` said it could not do until the
    /// engine had a vocabulary for attributes.
    #[test]
    fn doctor_checks_a_ladder_against_the_column_it_needs() {
        let graded = engine_with_grades();
        let mut map = doctor_map();
        map.attribute_fields.clear();

        let report = doctor_report(graded.def(), Some(&map), &Ok(agreeing_shape()))
            .expect_err("a ladder with no column is a fault");
        assert!(report.contains("severity"), "{report}");
        assert!(report.contains("would be dropped"), "{report}");

        // ⚠ And the other direction is a NOTE, not a fault: a refereed
        // attribute with no ladder is the stopgap shape, which is a real
        // deployment state. Reporting it as broken would go red on a
        // deployment that is exactly as its owner intended.
        let ungraded = doctor_report(engine().def(), Some(&doctor_map()), &Ok(agreeing_shape()));
        let text = ungraded.unwrap_or_else(|e| e);
        assert!(text.contains("no ladder grades it"), "{text}");
        assert!(text.contains("nothing says who may set which value"), "{text}");
    }

    fn engine_with_rescopes() -> Engine {
        let mut def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        def.rescopes = vec![ferrostep_core::RescopeDef {
            label: "branch".to_string(),
            role: "worker".to_string(),
            requires_note: true,
        }];
        Engine::new(def).unwrap()
    }

    /// The whole point of the subcommand: a record stops being found in one
    /// unit of work and starts being found in another, through the referee
    /// rather than through a database console.
    #[test]
    fn rescope_moves_a_record_between_units_of_work() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine_with_rescopes();
        let id = ids["live"].0.clone();

        let out = do_rescope(
            &engine,
            &ledger,
            &id,
            "worker",
            &[String::from("branch=follow-up")],
            Some("below the floor; rides to the follow-up branch"),
            "Ada",
        )
        .unwrap();
        assert!(out.contains("branch=follow-up"), "{out}");

        let in_scope = |branch: &str| {
            records_with_status(&engine, &ledger, &Scope::all().with("branch", branch), true)
                .unwrap()
                .len()
        };
        // Four records were seeded onto `main`; exactly one left.
        assert_eq!(in_scope("follow-up"), 1, "the record did not arrive");
        assert_eq!(in_scope("main"), 3, "the record is still in the unit it left");
    }

    /// ⚠ Each refusal has a different fix, so each has to say which it is.
    /// "Refused" alone sends a reader to re-read the definition looking for
    /// the wrong thing.
    #[test]
    fn rescope_refusals_name_what_is_wrong_and_persist_nothing() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine_with_rescopes();
        let id = ids["live"].0.clone();
        let set = [String::from("branch=follow-up")];

        for (case, role, sets, note, expect) in [
            ("no note", "worker", &set[..], None, "requires a note"),
            ("wrong role", "reviewer", &set[..], Some("why"), "may not change scope label"),
            (
                "undeclared label",
                "worker",
                &[String::from("repo=elsewhere")][..],
                Some("why"),
                "does not say who may change",
            ),
        ] {
            let refused = do_rescope(&engine, &ledger, &id, role, sets, note, "Ada");
            let Err(message) = refused else {
                panic!("{case}: allowed");
            };
            assert!(message.contains(expect), "{case}: {message}");
        }

        // A malformed --set is caught before the ledger is touched at all.
        let refused =
            do_rescope(&engine, &ledger, &id, "worker", &[String::from("branch")], Some("w"), "Ada");
        assert!(refused.unwrap_err().contains("label=value"));

        // Nothing above moved the record.
        assert_eq!(
            records_with_status(&engine, &ledger, &Scope::all().with("branch", "main"), true)
                .unwrap()
                .len(),
            4,
            "a refused rescope changed the ledger"
        );
    }

    /// The shipped example that says who may file, and what filing costs.
    fn filing_engine() -> Engine {
        let def =
            WorkflowDef::from_json(include_str!("../../examples/product-review.json")).unwrap();
        Engine::new(def).unwrap()
    }

    fn empty_ledger() -> (tempfile::TempDir, SqliteLedger) {
        let dir = tempfile::tempdir().unwrap();
        let ledger = SqliteLedger::open(dir.path().join("ledger.db")).unwrap();
        (dir, ledger)
    }

    /// ⚠⚠ The zero-install path had no way in. A store with a console of its
    /// own can be handed a record without the referee ever being asked —
    /// which is exactly why the roadmap wanted a second adapter — and SQLite
    /// has no console to hide behind. So a stranger following the README
    /// could reach every surface here except the one that starts a loop.
    #[test]
    fn filing_is_how_a_record_reaches_a_ledger_that_has_no_console() {
        let (_dir, ledger) = empty_ledger();
        let engine = filing_engine();
        let scope = Scope::all().with("release_line", "0.1.x");
        let out = do_file(
            &engine,
            &ledger,
            &scope,
            "owner",
            &[String::from("reviews_queued=0")],
            Some("cutting 0.1.1"),
            "Ada",
        )
        .unwrap();
        assert!(out.contains("filed into 'queued'"), "{out}");
        assert!(out.contains("reviews_queued 0 → 1"), "a filing that costs must say so: {out}");
        // ⚠ And must say where that number went, because it did not go onto
        // the record. An operator who passes the same value next time gets
        // the same answer forever, and a ceiling that never advances never
        // fires.
        assert!(out.contains("scope-level"), "the spend's home is not stated: {out}");

        let rows = records_with_status(&engine, &ledger, &scope, true).unwrap();
        assert_eq!(rows.len(), 1, "the record is in the ledger");
        assert_eq!(rows[0].0.snapshot.state, "queued", "filed into the initial state");
        assert!(
            rows[0].0.snapshot.counters.is_empty(),
            "a filing spend bounds a population, so it is not stored on the one record it filed"
        );
        // Filed into a unit of work, and invisible from another.
        let elsewhere = Scope::all().with("release_line", "0.2.x");
        assert!(records_with_status(&engine, &ledger, &elsewhere, true).unwrap().is_empty());
    }

    /// Every way filing is refused, and the shared requirement: none of them
    /// leaves a record behind.
    #[test]
    fn every_filing_refusal_names_what_is_wrong_and_files_nothing() {
        let (_dir, ledger) = empty_ledger();
        let filing = filing_engine();
        let scope = Scope::all();
        let measured = || vec![String::from("reviews_queued=0")];

        // ⚠ The one this tool has to get right by itself. A ceiling measured
        // against a number nobody supplied would pass every time — a guard
        // reporting success having checked nothing.
        let unmeasured =
            do_file(&filing, &ledger, &scope, "owner", &[], Some("why"), "Ada").unwrap_err();
        assert!(unmeasured.contains("--counter"), "the remedy is not in the message: {unmeasured}");

        let malformed = do_file(
            &filing,
            &ledger,
            &scope,
            "owner",
            &[String::from("reviews_queued=lots")],
            Some("why"),
            "Ada",
        )
        .unwrap_err();
        assert!(malformed.contains("whole number"), "{malformed}");

        let wrong_role =
            do_file(&filing, &ledger, &scope, "product_reviewer", &measured(), Some("why"), "Ada")
                .unwrap_err();
        assert!(wrong_role.contains("may not file"), "{wrong_role}");

        let silent =
            do_file(&filing, &ledger, &scope, "owner", &measured(), None, "Ada").unwrap_err();
        assert!(silent.contains("requires a note"), "{silent}");

        // Exhaustion here means the matter escalates and no record exists to
        // carry it — different from every other exhausted decision.
        let spent = do_file(
            &filing,
            &ledger,
            &scope,
            "owner",
            &[String::from("reviews_queued=4")],
            Some("why"),
            "Ada",
        )
        .unwrap_err();
        assert!(spent.contains("nothing was filed"), "{spent}");
        assert!(spent.contains("stalled"), "the refusal must say where that routes: {spent}");

        // A definition that never said who may file grants it to nobody,
        // rather than to anybody.
        let nobody =
            do_file(&engine(), &ledger, &scope, "reviewer", &[], Some("why"), "Ada").unwrap_err();
        assert!(nobody.contains("does not say who may file"), "{nobody}");

        assert!(
            records_with_status(&filing, &ledger, &Scope::all(), true).unwrap().is_empty(),
            "a refused filing left a record behind"
        );
    }

    /// `create` is the ledger interface's word, `file` is the definition's,
    /// and a person reaching for either has said the same thing. Answering
    /// only the preferred spelling is the mistake `--help` already made.
    #[test]
    fn both_spellings_of_filing_reach_the_same_command() {
        let dir = tempfile::tempdir().unwrap();
        let store = format!("sqlite:{}", dir.path().join("ledger.db").display());
        for (n, spelling) in ["file", "create"].iter().enumerate() {
            let counter = format!("reviews_queued={n}");
            let out = run(&argv(&[
                spelling,
                "--workflow",
                "../examples/product-review.json",
                "--store",
                &store,
                "--role",
                "owner",
                "--counter",
                &counter,
                "--note",
                "either word means this",
            ]))
            .unwrap();
            assert!(out.contains("filed into 'queued'"), "{spelling}: {out}");
        }
    }

    /// Filing is a permission like any other and default-deny like any
    /// other, so the surface that says what a definition permits has to say
    /// so — not least because the `file` usage text points here.
    #[test]
    fn explain_says_who_may_file_and_says_nobody_out_loud() {
        let granted = explain(&filing_engine(), None);
        assert!(
            granted.contains("filing: owner may file into 'queued'"),
            "{granted}"
        );
        assert!(granted.contains("spends reviews_queued"), "filing's cost is missing: {granted}");
        assert!(granted.contains("needs a reason"), "{granted}");
        // ⚠ The absent case is the one worth printing: a heading that is not
        // there leaves a reader to conclude default-deny for themselves.
        assert!(explain(&engine(), None).contains("filing: nobody"), "default-deny is not stated");
    }

    /// ⚠⚠ A rescope moves a record between units of work, not between
    /// states — so it is neither an arrival nor a departure, and the audit
    /// must not read one out of it. It used to read BOTH: rescoping a record
    /// that sits in a halted state satisfied the escalation test (`to` is
    /// halted) and the release test (`from_state` is halted) at once, and
    /// reported an escalation and a release for a record that had not moved.
    ///
    /// That is the dangerous shape for a report: not a crash and not an
    /// obviously wrong number, but a plausible story a reader cannot tell
    /// from a true one — on the surface B4 offers to somebody who is
    /// deliberately not opening a database console to check.
    ///
    /// The halted state is the case that must be tested, because it is the
    /// only one where both tallies fire, and it is an ordinary thing to want:
    /// a record parked for a person is exactly the kind that gets moved to
    /// another unit of work.
    #[test]
    fn rescoping_a_paused_record_is_neither_an_escalation_nor_a_release() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine_with_rescopes();
        let paused = ids["paused"].0.clone();
        let line_for = |out: &str| {
            out.lines()
                .find(|l| l.starts_with(&format!("  record {paused}:")))
                .expect("the paused record is in the report")
                .to_string()
        };
        let audit = || {
            let rows = records_with_status(&engine, &ledger, &Scope::all(), true).unwrap();
            line_for(&render_audit(&engine, &ledger, &rows).unwrap())
        };

        let before = audit();
        assert!(
            before.contains("1 escalation(s)") && before.contains("0 release(s)"),
            "fixture: the record escalated once and was never released: {before}"
        );

        do_rescope(
            &engine,
            &ledger,
            &paused,
            "worker",
            &[String::from("branch=release-2")],
            Some("moved to the release branch"),
            "Ada",
        )
        .unwrap();

        let after = audit();
        // The move count DOES rise: a rescope is a real event with real
        // history. What must not rise is the reading of it as a state change.
        assert!(after.contains("3 move(s)"), "the rescope is in the history: {after}");
        assert!(
            after.contains("1 escalation(s)"),
            "a rescope was counted as an escalation: {after}"
        );
        assert!(
            after.contains("0 release(s)"),
            "a rescope was counted as a release: {after}"
        );
    }

    /// ⚠⚠ The point of `explain` is the number the definition does NOT
    /// contain. A ceiling of 3 becomes a 4 somewhere in the adopter's tree —
    /// `max + 1` in a guard, a range in help text, a sentence in a brief — and
    /// searching for 3 finds none of them. Measured three times in one
    /// migration, where the search term that worked was never the stored value.
    #[test]
    fn explain_names_the_derived_number_a_search_would_miss() {
        let engine = engine_with_rescopes();
        let out = explain(&engine, None);
        assert!(out.contains("agent_passes = 3"), "the asserted value is missing: {out}");
        assert!(
            out.contains("for 3 AND for 4"),
            "the off-by-one neighbour is the whole feature: {out}"
        );
        // Readable without a ledger: the person who needs this has not
        // connected anything yet.
        let out = run(&argv(&["explain", "--workflow", "../examples/review-loop.json"])).unwrap();
        assert!(out.contains("workflow 'review-loop'"), "{out}");
        // A pause and an ending are different things, and the whole point of
        // distinguishing them is lost if a reader cannot see which is which.
        assert!(out.contains("[initial]") && out.contains("[ending]") && out.contains("[pause]"),
            "states are not marked: {out}");
        assert!(out.contains("[person]"), "human roles are not marked: {out}");
    }

    fn map_with_scope(labels: &[&str]) -> ferrostep_pocketbase::CollectionMap {
        ferrostep_pocketbase::CollectionMap {
            scope_fields: labels.iter().map(|s| s.to_string()).collect(),
            ..cli_test_map(false)
        }
    }

    /// ⚠⚠ **The per-grant lines read as independent permissions, and the labels
    /// are coordinates of one address.** Measured on the first adopter,
    /// 2026-08-27: one scope label was moved, a tool still selecting on the
    /// other counted four records its own queue could not act on, and would
    /// have spent every remaining review before reporting it had not converged.
    ///
    /// ⚠ The assertions name BOTH labels rather than checking that a warning
    /// appeared. A warning that fires without saying which labels are out of
    /// step sends the reader back to the definition to work it out, which is
    /// the work the line exists to save.
    #[test]
    fn explain_says_the_scope_labels_are_one_address_when_there_is_more_than_one() {
        let map = map_with_scope(&["lane", "release_line"]);
        let out = explain(&engine_with_rescopes(), Some(&map));
        assert!(out.contains("TUPLE"), "no tuple warning at all: {out}");
        assert!(out.contains("'lane'"), "warning does not name 'lane': {out}");
        assert!(out.contains("'release_line'"), "does not name 'release_line': {out}");
    }

    /// The negative control. One label cannot be out of step with itself, and a
    /// warning that fires on every definition is one readers learn to skip.
    #[test]
    fn explain_stays_quiet_about_tuples_when_the_scope_is_a_single_label() {
        let single = explain(&engine_with_rescopes(), Some(&map_with_scope(&["lane"])));
        assert!(!single.contains("TUPLE"), "warned about a one-label scope: {single}");
        // With no map, the fallback counts DISTINCT rescope labels — this
        // fixture grants exactly one, so it must stay quiet too.
        let no_map = explain(&engine_with_rescopes(), None);
        assert!(!no_map.contains("TUPLE"), "warned with one rescope label: {no_map}");
        // ⚠ Floor. Without this, both assertions above pass on output that
        // never reached the section at all — a negative control that proves
        // nothing is the failure this repo keeps re-finding.
        assert!(single.contains("unit-of-work moves:"), "{single}");
        assert!(no_map.contains("unit-of-work moves:"), "{no_map}");
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

    fn cli_test_map(guard: bool) -> ferrostep_pocketbase::CollectionMap {
        ferrostep_pocketbase::CollectionMap {
            records: "tickets".to_string(),
            events: "ticket_events".to_string(),
            state_field: "stage".to_string(),
            version_field: "fs_version".to_string(),
            counter_fields: vec!["attempts".to_string()],
            scope_fields: vec!["lane".to_string()],
            attribute_fields: vec!["severity".to_string()],
            guard_refereed_fields: guard,
        }
    }

    /// ⚠⚠ **The list the guard closes and the list this prints must be the
    /// same list, and this is the test that says so.** Two derivations would
    /// drift the first time a counter is added to a map — and they would drift
    /// in the direction that reports a clean sweep, which is the direction
    /// nothing goes red in. Asserted against the generated hook text rather
    /// than against a second copy of the field names, so a change to either
    /// side has to keep them agreeing.
    #[test]
    fn the_columns_explain_lists_are_exactly_the_columns_the_guard_closes() {
        let map = cli_test_map(true);
        let hooks = ferrostep_pocketbase::hooks_file_mapped(
            &map,
            None,
            &ferrostep_pocketbase::ActorBinding::default(),
        );
        let guarded = slice_once(&hooks, "const REFEREED = [", ']').to_string();
        let out = explain(&engine(), Some(&map));
        // ⚠ A floor first: an empty list would satisfy every containment
        // check below and prove nothing.
        //
        // ⚠⚠ AND THE FLOOR IS NOT THE GUARD AGAINST A DROPPED CATEGORY, though
        // it looks like one while the fixture happens to carry exactly four
        // fields. Both lists below are read out of `refereed_fields()`, so a
        // derivation that stops emitting scope columns shortens both and they
        // keep agreeing; add a second counter to the fixture and this floor
        // passes while the guard closes nothing on scope. Measured 2026-08-27.
        // The known-answer assertion lives beside the derivation, in
        // `ferrostep-pocketbase`: `refereed_fields_is_one_field_of_every_kind_in_hook_order`.
        let fields = map.refereed_fields();
        assert!(
            fields.len() >= 4,
            "the fixture stopped exercising this — or a category was dropped from the \
             derivation, which the pocketbase known-answer test names precisely: {fields:?}"
        );
        // ⚠⚠ EQUALITY, NOT CONTAINMENT, AND THE EXTRA DIRECTION IS THE ONE
        // THAT HURTS AN ADOPTER. Containment only asked whether the guard
        // closes everything the sweep names; it never asked whether it closes
        // anything the sweep does NOT. Measured 2026-08-27: push one more
        // column into the generated list and the whole workspace suite stays
        // green — the guard then refuses writes to a column no hunting list
        // ever told the adopter to sweep for, which reads to them as the
        // referee breaking rather than as a documented rule.
        let closed: Vec<String> = guarded
            .split(',')
            .map(|f| f.trim().trim_matches('"').to_string())
            .filter(|f| !f.is_empty())
            .collect();
        assert_eq!(closed, fields, "the guard closes a different set than the sweep names");
        for field in &fields {
            assert!(out.contains(field.as_str()), "explain does not list '{field}': {out}");
        }
    }

    /// ⚠⚠ The sweep has to say PROSE out loud. An adopter enumerated four
    /// scripted call sites, a second party checked that enumeration, and the
    /// guard's first refusal came from neither list — it came from a persona
    /// naming a record-mutation tool, and later from a machine-wide skill
    /// file. A hunting list that implies "grep your code" reproduces exactly
    /// the sweep that missed them twice.
    #[test]
    fn the_column_sweep_says_prose_and_says_what_it_cannot_find() {
        let out = explain(&engine(), Some(&cli_test_map(false)));
        assert!(out.contains("PROSE"), "the sweep reads as a code sweep: {out}");
        assert!(out.contains("skill file"), "{out}");
        assert!(
            out.contains("update its status in the\n    tracker"),
            "the un-greppable case has to be named, not implied: {out}"
        );
        assert!(
            out.contains("no authentication step"),
            "why a code sweep misses it is the actionable half: {out}"
        );
        // The state of the flag changes what the reader should do next, so it
        // is reported rather than assumed.
        assert!(out.contains("is OFF"), "{out}");
        assert!(out.contains("/api/ferrostep/tickets/apply"), "{out}");
        let on = explain(&engine(), Some(&cli_test_map(true)));
        assert!(on.contains("is ON") && on.contains("failing NOW"), "{on}");
        // ⚠ NEGATIVE CONTROL. Without it every assertion above would pass on a
        // section that printed unconditionally, which is a section that cannot
        // be wrong and cannot be right.
        let bare = explain(&engine(), None);
        assert!(!bare.contains("COLUMNS THIS REFEREE OWNS"), "printed without a map: {bare}");
    }

    /// ⚠ The columns section is emitted after the numbers section, and the
    /// numbers section used to `return` early when a definition asserted none.
    /// A section that silently does not print for some inputs is the exact
    /// defect it exists to warn about, so the no-counters path is asserted
    /// rather than assumed.
    #[test]
    fn a_definition_with_no_numbers_still_gets_its_columns() {
        let mut def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        def.counters.clear();
        for t in &mut def.transitions {
            t.spends.clear();
            t.resets.clear();
        }
        if let Some(creation) = &mut def.creation {
            creation.spends.clear();
        }
        let out = explain(&Engine::new(def).unwrap(), Some(&cli_test_map(false)));
        assert!(out.contains("asserts no numbers"), "{out}");
        assert!(out.contains("COLUMNS THIS REFEREE OWNS"), "the early return is back: {out}");
    }

    /// ⚠ A ceiling is a number out of a file somebody else wrote, and this is
    /// the subcommand aimed at a person who has not got the system working
    /// yet — the worst possible audience for a crash. A maximal ceiling used
    /// to panic here on `max + 1` in a debug build, and in release would have
    /// wrapped, sending the reader hunting through their tree for `0`.
    #[test]
    fn explain_survives_a_ceiling_with_no_next_number() {
        let mut def =
            WorkflowDef::from_json(include_str!("../../examples/review-loop.json")).unwrap();
        def.counters[0].max = u32::MAX;
        let out = explain(&Engine::new(def).unwrap(), None);
        assert!(out.contains(&u32::MAX.to_string()), "the asserted value is missing: {out}");
        assert!(
            !out.contains("AND for 0"),
            "a wrapped neighbour would send the reader hunting for 0: {out}"
        );
    }

    /// ⚠ A move that costs something must say what it cost. Confirming a
    /// spend by reading the record back is a second round trip, and the
    /// tooling this replaces printed the arithmetic — so omitting it made the
    /// referee a regression at the one moment an operator most wants the
    /// number. Old → new, so a spend and a re-arm are distinguishable without
    /// the engine inventing a label for them.
    #[test]
    fn a_move_reports_what_it_spent_not_just_where_it_went() {
        let (_dir, ledger, ids) = seeded();
        let engine = engine();
        let out = do_move(
            &engine,
            &ledger,
            &ids["live"].0,
            "worker",
            "working",
            None,
            "Ada",
        )
        .unwrap();
        assert!(out.contains("moved to 'working'"), "{out}");
        assert!(out.contains("agent_passes 0 → 1"), "the spend's arithmetic is missing: {out}");

        // A move that costs nothing must not claim a cost. (The version is
        // always reported, so the absence being checked is the counter
        // fragment, not the parenthesis around the version.)
        let out = do_move(&engine, &ledger, &ids["live"].0, "worker", "awaiting_review", None, "Ada")
            .unwrap();
        assert!(!out.contains('→'), "a costless move reported a cost: {out}");
        assert!(out.contains("version"), "the version is still reported: {out}");
    }

    /// ⚠ Every spelling of "explain this to me" has to answer, because the
    /// person typing it has already said they do not know how this works.
    /// Answering "--help needs a value" to `--help` is the tool being clever
    /// at the exact moment it should be plain.
    /// ⚠⚠ **An ignored flag answers the wrong question confidently.** Two
    /// ways it bit, both measured on this workspace 2026-08-26: a *typo*
    /// (`--scpoe`) silently widened a scoped audit to every record and exited
    /// 0; and a *binary older than a flag* accepted `--role`, ignored it, and
    /// reported "0 of 12 await a person" — correct for the question it
    /// actually asked, and completely wrong for the one asked of it.
    ///
    /// This is AGENTS.md's generated-files convention arriving at a surface
    /// that had not been held to it: an older thing meeting a newer request
    /// refuses the part it does not understand rather than accepting and
    /// ignoring it. The refusal doubles as the version diagnostic, which is
    /// why no separate capability probe is needed.
    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        let store = "sqlite:/tmp/ferrostep-unknown-flag.db";
        // A flag no build has ever had.
        let refused = run(&argv(&[
            "awaiting", "--workflow", "../examples/review-loop.json", "--store", store,
            "--totally-made-up", "v",
        ]))
        .unwrap_err();
        assert!(refused.contains("--totally-made-up"), "{refused}");
        assert!(refused.contains("it accepts:"), "the remedy must be in the message: {refused}");
        assert!(refused.contains("predate"), "the version case must be named: {refused}");

        // ⚠ The typo case, which is worse than the version case because it is
        // silent in BOTH builds: `--scpoe` widened the query and exited 0.
        let typo = run(&argv(&[
            "audit", "--workflow", "../examples/review-loop.json", "--store", store,
            "--scpoe", "branch=main",
        ]))
        .unwrap_err();
        assert!(typo.contains("--scpoe"), "{typo}");

        // A flag that IS accepted here still works, and one accepted by a
        // different subcommand is still refused by this one.
        run(&argv(&[
            "awaiting", "--workflow", "../examples/review-loop.json", "--store", store,
            "--role", "worker",
        ]))
        .expect("--role is accepted by awaiting");
        let wrong_command = run(&argv(&[
            "explain", "--workflow", "../examples/review-loop.json", "--record", "1",
        ]))
        .unwrap_err();
        assert!(wrong_command.contains("--record"), "explain takes no --record: {wrong_command}");
    }

    #[test]
    fn every_way_of_asking_for_help_gets_help() {
        for spelling in [
            vec!["--help"],
            vec!["-h"],
            vec!["help"],
            vec!["help", "awaiting"],
            vec!["awaiting", "--help"],
            vec!["rescope", "--record", "1", "--help"],
        ] {
            let out = run(&argv(&spelling));
            let Ok(text) = out else {
                panic!("{spelling:?} was refused instead of answered: {}", out.unwrap_err());
            };
            assert!(text.contains("USAGE:"), "{spelling:?} answered without usage");
            // Help must not need a workflow or a store either — the person
            // asking does not have one yet.
            assert!(text.contains("agent-env"), "{spelling:?} answered a truncated usage");
        }
    }

    /// A roster with both entries and the persona files they name.
    fn roster_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("workflow")).unwrap();
        std::fs::write(dir.path().join("workflow/DEVELOPER.md"), "# developer").unwrap();
        std::fs::write(dir.path().join("workflow/REVIEWER.md"), "# reviewer").unwrap();
        std::fs::write(
            dir.path().join("config.yaml"),
            "default_agent: developer\n\
             agents:\n\
            \x20 developer:\n\
            \x20   name: Ada\n\
            \x20   email: ada@example.com\n\
            \x20   persona: workflow/DEVELOPER.md\n\
            \x20 reviewer:\n\
            \x20   name: Grace\n\
            \x20   email: grace@example.com\n\
            \x20   persona: workflow/REVIEWER.md\n",
        )
        .unwrap();
        dir
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    /// ⚠ The regression this pins is structural: every other subcommand
    /// requires a workflow and a store, and those are resolved before the
    /// match. Asking who the developer is must not require a refereed ledger
    /// to exist — a repo adopts the roster before it adopts the referee, and
    /// that ordering is the whole reason this ships in the binary.
    #[test]
    fn agent_env_answers_without_a_workflow_or_a_store() {
        let dir = roster_dir();
        let roster = dir.path().join("config.yaml");
        let out = run(&argv(&["agent-env", "--roster", roster.to_str().unwrap()])).unwrap();
        assert!(out.contains("AGENT_TITLE='developer'"), "{out}");
        assert!(out.contains("AGENT_NAME='Ada'"), "{out}");
        assert!(out.contains("AGENT_EMAIL='ada@example.com'"), "{out}");
        assert!(out.contains("workflow/DEVELOPER.md'"), "{out}");

        let out = run(&argv(&[
            "agent-env",
            "--agent",
            "reviewer",
            "--roster",
            roster.to_str().unwrap(),
        ]))
        .unwrap();
        assert!(out.contains("AGENT_NAME='Grace'"), "{out}");
        assert!(out.contains("workflow/REVIEWER.md'"), "{out}");
    }

    /// ⚠⚠ **Refuse, never fall back.** The caller `eval`s this output and
    /// then commits with it, so an empty `AGENT_NAME` emitted at status zero
    /// signs the work as whoever the repo is configured for — the exact
    /// silent misattribution the roster exists to end. Every failure must
    /// reach `main` as an `Err`, which is the non-zero exit.
    #[test]
    fn every_roster_failure_is_a_refusal_rather_than_an_empty_variable() {
        let dir = roster_dir();
        let roster = dir.path().join("config.yaml");
        let missing = dir.path().join("nowhere/config.yaml");
        std::fs::write(dir.path().join("broken.yaml"), "agents: [this is not a map]\n").unwrap();

        for (case, args) in [
            ("no such roster", vec!["agent-env", "--roster", missing.to_str().unwrap()]),
            (
                "unparseable roster",
                vec!["agent-env", "--roster", dir.path().join("broken.yaml").to_str().unwrap()],
            ),
            (
                "unknown title",
                vec!["agent-env", "--agent", "archivist", "--roster", roster.to_str().unwrap()],
            ),
        ] {
            let result = run(&argv(&args));
            let Err(refused) = result else {
                panic!("{case}: emitted '{}' instead of refusing", result.unwrap());
            };
            assert!(!refused.is_empty(), "{case}: refused with no message");
        }
    }

    /// A caller that is not a shell should not have to decode shell quoting
    /// to recover a value the emitter had in hand. Both formats answer from
    /// one resolution, so they cannot disagree about who an agent is.
    #[test]
    fn json_carries_the_same_entry_as_the_shell_form() {
        let dir = roster_dir();
        let roster = dir.path().join("config.yaml");
        let json = run(&argv(&[
            "agent-env",
            "--agent",
            "reviewer",
            "--roster",
            roster.to_str().unwrap(),
            "--format",
            "json",
        ]))
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["title"], "reviewer");
        assert_eq!(parsed["name"], "Grace");
        assert_eq!(parsed["email"], "grace@example.com");
        assert_eq!(
            parsed["persona"],
            dir.path().join("workflow/REVIEWER.md").to_string_lossy().as_ref()
        );

        let Err(refused) = run(&argv(&[
            "agent-env",
            "--roster",
            roster.to_str().unwrap(),
            "--format",
            "yaml",
        ])) else {
            panic!("an unknown format was accepted");
        };
        assert!(refused.contains("shell or json"), "{refused}");
    }

    /// ⚠ The persona check is the reason a launcher can trust the path it is
    /// handed. A second output format is exactly where such a check gets
    /// forgotten, so it is asked of both.
    #[test]
    fn neither_format_emits_a_persona_that_does_not_exist() {
        let dir = roster_dir();
        std::fs::remove_file(dir.path().join("workflow/REVIEWER.md")).unwrap();
        let roster = dir.path().join("config.yaml");
        for format in ["shell", "json"] {
            let result = run(&argv(&[
                "agent-env",
                "--agent",
                "reviewer",
                "--roster",
                roster.to_str().unwrap(),
                "--format",
                format,
            ]));
            let Err(refused) = result else {
                panic!("--format {format} emitted a persona that does not exist");
            };
            assert!(refused.contains("does not exist"), "{format}: {refused}");
        }
    }
}

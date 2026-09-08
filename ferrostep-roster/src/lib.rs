//! ferrostep-roster — the actors of a refereed loop.
//!
//! The engine answers *what may be done*, from a workflow definition. This
//! answers *who is doing it*: an agent's title, the identity it signs work
//! under, and the document that tells it how to behave. Both are data a
//! deployment configures, and neither is compiled in.
//!
//! A roster is a `config.yaml`, found as `<repo>/FerroStep/config.yaml` — the
//! standard deployment folder, alongside `workflow/` (scripts) and
//! `personas/` — or, for a repo that has not adopted that folder (this one
//! included), bare at the repo root:
//!
//! ```yaml
//! default_agent: developer
//! agents:
//!   developer:
//!     name: Ada
//!     email: ada@example.com
//!     persona: personas/DEVELOPER.md
//!   reviewer:
//!     name: Grace
//!     email: grace@example.com
//!     persona: personas/REVIEWER.md
//! ```
//!
//! Entries are keyed by **title**, and a title is a configured value rather
//! than vocabulary this crate knows: nothing here means anything by
//! "developer". A persona document finds its own entry the same way — as the
//! one whose `persona` names that file — so the documents stay portable
//! between deployments that call the roles different things.
//!
//! Unknown keys are tolerated on purpose. A deployment's `config.yaml` is
//! where *its* configurable values live, and most of them are none of this
//! reader's business.
//!
//! **Everything here fails loudly.** An identity that fails open signs work
//! under the wrong author and nothing downstream notices, so a missing file,
//! an unknown title, an incomplete entry and a persona path that points at
//! nothing are each an error — never an empty string.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file a roster lives in.
pub const ROSTER_FILE: &str = "config.yaml";

/// The standard deployment folder a consumer repo installs FerroStep's
/// working files into: `<repo>/FerroStep/config.yaml`, alongside `workflow/`
/// and `personas/`. Checked first, at every level of the upward walk, before
/// the bare [`ROSTER_FILE`] at that same level — so a repo that has adopted
/// the folder is found without a flag, and a repo that has not (this one
/// included: `FerroStep/config.yaml` here would mean a folder named
/// `FerroStep` inside FerroStep) keeps working unchanged.
pub const ROSTER_DIR: &str = "FerroStep";

/// What a deployment folder holds besides its roster, and therefore what
/// tells one from a directory that merely shares the name.
///
/// ⚠⚠ **THE NAME ALONE IS NOT ENOUGH, AND ASSUMING IT WAS PUT AN AGENT UNDER
/// SOMEBODY ELSE'S IDENTITY.** A workspace holding several repos side by side
/// can contain a checkout *of FerroStep itself*, and `<workspace>/FerroStep/`
/// then matches [`ROSTER_DIR`] while being a repository rather than anything
/// anyone installed. Its roster — the one `docs/deployment-map.md` lists under
/// *never ships* — won at that level, the workspace's own file was skipped,
/// and every sibling repo without a roster resolved as FerroStep's own agent
/// and exited 0. Reported by an adopter 2026-09-08, from a repo where a
/// resident following the documented commit procedure would have signed as
/// somebody else.
pub const DEPLOYMENT_MARKER: &str = "personas";

/// A parsed roster: every file that contributed to it, and what they said.
///
/// **Layered.** A workspace holding several repos can put shared values in a
/// `config.yaml` above them and let each repo's own file override what it
/// needs. Discovery collects every file from the working directory upward,
/// nearest last, and the nearest wins.
///
/// ⚠ **Every value remembers the file it came from**, because a relative
/// path is resolved against *that* file's directory and against nothing else.
/// A parent's `workflow/DEVELOPER.md` means the parent's `workflow/`, whether
/// it is read from the parent or inherited by a repo three levels down.
#[derive(Debug, Clone)]
pub struct Roster {
    /// Contributing files, furthest first — so the last is the nearest.
    sources: Vec<PathBuf>,
    default_agent: Option<String>,
    agents: BTreeMap<String, Entry>,
    auth: Option<Auth>,
    agents_reach: Reach,
}

/// One agent's entry, and the file that supplied it.
#[derive(Debug, Clone)]
struct Entry {
    agent: Agent,
    source: PathBuf,
}

/// How far down a roster's *agents* apply.
///
/// ⚠ **Identity is the one thing that must not be inherited by accident.**
/// Shared settings layering down a workspace is the feature; an agent list
/// doing it is how a repo that never declared an identity answers with
/// somebody else's. So a roster says which it is, and the answer travels with
/// the file rather than being guessed from where it sits.
///
/// ⚠ Nothing here changes how `auth` layers — a credential *source* is not an
/// identity, and it goes on reaching every level beneath the file that
/// declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    /// Agents apply at this file's own level and no further. A workspace
    /// roster serving an agent that has not yet been pointed at a project is
    /// this: it answers where it sits, and a repo beneath it has to declare
    /// its own.
    Here,
    /// Agents apply to this file's level and everything beneath it — a repo
    /// roster answering from any of its own subdirectories.
    Below,
}

impl Default for Reach {
    /// ⚠ **`Below`, and the choice is a compatibility one rather than the
    /// safe one.** Every roster written before this key existed means
    /// `Below` — that is what it did — so defaulting the other way would
    /// refuse in repos that work today, including from any subdirectory. The
    /// fail-open case it leaves is a *new* parent roster nobody marked, and
    /// that is what `Here` is for.
    fn default() -> Self {
        Reach::Below
    }
}

/// One agent's entry.
#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    name: String,
    email: String,
    persona: String,
    /// What a launcher may spend on one run of this agent, in US dollars.
    ///
    /// ⚠ **Absent is no ceiling, and that is the state every deployment
    /// starts in.** A cap is opted into per entry; nothing here invents one,
    /// and nothing here defaults one, because a default ceiling would be a
    /// blessed number and this crate does not hold those.
    ///
    /// ⚠ **A quantity, never a switch.** This says how many dollars. Which
    /// flag carries them belongs to whatever a launcher runs — the program on
    /// the far side is something an adapter speaks to, so naming its spelling
    /// here would put one vendor's command line in the format every other
    /// deployment has to write.
    ///
    /// ⚠ **It sits on the agent, not on a role.** Titles are configured
    /// values and this crate means nothing by any of them, so a ceiling is
    /// available to every entry and a deployment decides which of its actors
    /// carries one.
    #[serde(default)]
    budget_usd: Option<f64>,
    /// Whether a launcher records what a run of this agent cost, onto the
    /// ledger event for the move that run produced.
    ///
    /// ⚠ **Off unless asked for, and it is a per-agent question rather than a
    /// deployment-wide one** — a deployment may want the cost of the actor it
    /// is trying to bound without keeping it for every other.
    #[serde(default)]
    capture_cost: Option<bool>,
    /// Whether a launcher labels this agent's run so external observation can
    /// be attributed back to the refereed work it was doing.
    ///
    /// ⚠ **Opt-in because it EXPORTS identifiers.** Labelling a run means the
    /// record and the title leave this deployment and land wherever the
    /// telemetry goes — which is somebody else's store, on somebody else's
    /// access rules. That is a reasonable trade and it is not one to make on
    /// an operator's behalf.
    ///
    /// ⚠ **What the label is carried BY is not this crate's business**, the
    /// same as [`Agent::budget_usd`]: this says whether to label, never which
    /// mechanism does it.
    #[serde(default)]
    tag_runs: Option<bool>,
}

/// Where an actor's credential comes from.
///
/// **A type, not a lookup.** The first one is a file, which is right for one
/// operator on one host and honest about being that. Naming it as a *kind* is
/// what leaves room for a keyring, an environment source, or a secrets
/// service without every consumer having to learn a new shape.
///
/// ⚠ **This crate never reads the secret.** It says which identity is acting
/// and where that deployment keeps credentials; the caller does the lookup.
/// That is not squeamishness — it is what keeps a password out of the
/// environment, and an exported password is inherited by every subprocess,
/// including one launched to act as somebody else.
#[derive(Debug, Clone, PartialEq)]
pub enum Auth {
    /// A file of credentials keyed by identity. The path is absolute,
    /// resolved against the config file that named it.
    Simple { path: PathBuf },
}

impl Auth {
    /// The word a deployment writes for this kind, and that a caller
    /// switches on.
    pub fn kind(&self) -> &'static str {
        match self {
            Auth::Simple { .. } => "simple",
        }
    }

    /// Where the credentials live.
    pub fn path(&self) -> &Path {
        match self {
            Auth::Simple { path } => path,
        }
    }
}

/// The wire shape of [`Auth`]: a discriminated union, so an unknown type is a
/// loud parse failure rather than a silently ignored block.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AuthRepr {
    Simple { path: String },
}

/// The deserialized shape. Separate from [`Roster`] so the public type can
/// carry its sources and keep its invariants.
#[derive(Debug, Deserialize)]
struct RosterFile {
    #[serde(default)]
    agents_reach: Reach,
    #[serde(default)]
    default_agent: Option<String>,
    #[serde(default)]
    agents: BTreeMap<String, Agent>,
    #[serde(default)]
    auth: Option<AuthRepr>,
}

impl Roster {
    /// Read a roster from an explicit path.
    pub fn load(path: impl AsRef<Path>) -> Result<Roster, RosterError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|cause| RosterError::Unreadable {
            path: path.to_path_buf(),
            cause: cause.to_string(),
        })?;
        Roster::parse(&text, path)
    }

    /// Find the roster by walking up from `start`, so a caller works from any
    /// subdirectory of the repo. Walking beats a path baked in at build time,
    /// which goes stale the moment a checkout moves.
    ///
    /// ⚠ **Every file on the way up contributes**, not just the first one
    /// found. That is what lets a workspace share values across the repos
    /// beneath it; the nearest file wins wherever two speak.
    ///
    /// ⚠ **At each level, `FerroStep/config.yaml` is checked before the bare
    /// file at that same level.** A repo either has adopted the deployment
    /// folder or has not; it does not have both, so this is "which one" and
    /// not a precedence rule anybody has to reason about across two files
    /// that actually coexist.
    pub fn discover(start: impl AsRef<Path>) -> Result<Roster, RosterError> {
        let start = start.as_ref();
        let mut found: Vec<(PathBuf, bool)> = Vec::new();
        let mut dir = start.to_path_buf();
        let mut at_start = true;
        loop {
            let folder = dir.join(ROSTER_DIR);
            let in_folder = folder.join(ROSTER_FILE);
            let bare = dir.join(ROSTER_FILE);
            match (in_folder.is_file(), bare.is_file()) {
                (true, false) => found.push((in_folder, at_start)),
                (false, true) => found.push((bare, at_start)),
                (false, false) => {}
                // ⚠ Both, which the convention said could not happen — "a repo
                // either has adopted the deployment folder or has not; it does
                // not have both". True of a repo and false of a workspace that
                // contains a checkout named like the folder. So the name is
                // not the test: a deployment folder is one that LOOKS like a
                // deployment, and a directory that merely shares the name
                // leaves the level's own file as the answer.
                (true, true) => {
                    if folder.join(DEPLOYMENT_MARKER).is_dir() {
                        return Err(RosterError::AmbiguousRoster {
                            folder: in_folder,
                            bare,
                        });
                    }
                    found.push((bare, at_start));
                }
            }
            at_start = false;
            if !dir.pop() {
                break;
            }
        }
        if found.is_empty() {
            return Err(RosterError::NotFound { from: start.to_path_buf() });
        }
        // Collected nearest-first by the walk; layering wants furthest-first
        // so the nearest is applied last and wins.
        found.reverse();
        let mut layers = Vec::with_capacity(found.len());
        for (path, at_start) in found {
            layers.push((Roster::load(path)?, at_start));
        }
        Ok(Roster::layer(layers))
    }

    /// Fold rosters together, furthest first, so the last wins. The flag is
    /// whether that layer was found at the directory discovery *started* in.
    ///
    /// ⚠ **A layer that reaches only `Here` contributes agents when it is the
    /// level you are standing in, and never when it is merely above you.**
    /// That is the whole difference between a workspace roster answering an
    /// agent who has not been pointed at a project yet, and the same file
    /// answering for a repo that never declared anyone.
    ///
    /// ⚠ **`agents` merges per title and `auth` does not merge at all.** A
    /// title is taken from the nearest file that names it, *whole* — entries
    /// are never field-merged, because half an identity assembled from two
    /// files is worse than either of them complete. `auth` is replaced as a
    /// block for the sharper version of the same reason: a `type` from one
    /// file meeting a `path` meant for another is a configuration nobody
    /// wrote and nobody can debug.
    fn layer(layers: Vec<(Roster, bool)>) -> Roster {
        let mut merged = Roster {
            sources: Vec::new(),
            default_agent: None,
            agents: BTreeMap::new(),
            auth: None,
            agents_reach: Reach::Below,
        };
        for (layer, at_start) in layers {
            merged.sources.extend(layer.sources);
            // ⚠ Unconditional, and deliberately so: a credential SOURCE is not
            // an identity. It says where a deployment keeps credentials, which
            // is exactly the kind of shared setting a workspace file exists to
            // state once.
            if layer.auth.is_some() {
                merged.auth = layer.auth;
            }
            if !(at_start || layer.agents_reach == Reach::Below) {
                continue;
            }
            if layer.default_agent.is_some() {
                merged.default_agent = layer.default_agent;
            }
            for (title, entry) in layer.agents {
                merged.agents.insert(title, entry);
            }
        }
        merged
    }

    /// Find the roster by walking up from the current directory.
    pub fn discover_from_cwd() -> Result<Roster, RosterError> {
        let cwd = std::env::current_dir().map_err(|cause| RosterError::Unreadable {
            path: PathBuf::from("."),
            cause: cause.to_string(),
        })?;
        Roster::discover(cwd)
    }

    /// Parse roster text that came from `source`.
    pub fn parse(text: &str, source: impl AsRef<Path>) -> Result<Roster, RosterError> {
        let source = source.as_ref().to_path_buf();
        let file: RosterFile =
            serde_norway::from_str(text).map_err(|cause| RosterError::Malformed {
                path: source.clone(),
                cause: cause.to_string(),
            })?;
        let root = source.parent().unwrap_or(Path::new(".")).to_path_buf();
        // ⚠ Resolved here, against THIS file, and not later against whatever
        // file happened to win the merge. A path inherited from a parent still
        // means the parent's directory.
        let auth = file.auth.map(|repr| match repr {
            AuthRepr::Simple { path } => Auth::Simple { path: resolve_against(&root, &path) },
        });
        let agents = file
            .agents
            .into_iter()
            .map(|(title, agent)| (title, Entry { agent, source: source.clone() }))
            .collect();
        Ok(Roster {
            sources: vec![source],
            default_agent: file.default_agent,
            agents,
            auth,
            agents_reach: file.agents_reach,
        })
    }

    /// The nearest file that contributed — the one a reader thinks of as
    /// "the" roster, and the right one to name in an error.
    pub fn source(&self) -> &Path {
        self.sources.last().map(PathBuf::as_path).unwrap_or(Path::new(ROSTER_FILE))
    }

    /// Every file that contributed, furthest first.
    pub fn sources(&self) -> &[PathBuf] {
        &self.sources
    }

    /// The directory the nearest file sits in.
    pub fn root(&self) -> &Path {
        self.source().parent().unwrap_or(Path::new("."))
    }

    /// Where this deployment keeps actor credentials, if it says.
    ///
    /// ⚠ The secret itself is deliberately not read here — see [`Auth`].
    pub fn auth(&self) -> Option<&Auth> {
        self.auth.as_ref()
    }

    /// The title an unmarked session adopts, if the roster names one.
    pub fn default_title(&self) -> Option<&str> {
        self.default_agent.as_deref()
    }

    /// Every title in the roster, sorted.
    pub fn titles(&self) -> Vec<&str> {
        self.agents.keys().map(String::as_str).collect()
    }

    /// Resolve a title, or the default when none is asked for. The returned
    /// entry is known to be complete; an entry missing a field is an error
    /// here rather than an empty variable at the call site.
    pub fn resolve(&self, title: Option<&str>) -> Result<Resolved<'_>, RosterError> {
        let title = match title {
            Some(title) => title,
            None => self.default_title().ok_or_else(|| RosterError::NoDefault {
                path: self.source().to_path_buf(),
            })?,
        };
        let (title, entry) = self.agents.get_key_value(title).ok_or_else(|| {
            RosterError::UnknownTitle {
                title: title.to_string(),
                known: self.titles().into_iter().map(str::to_owned).collect(),
                path: self.source().to_path_buf(),
            }
        })?;
        let agent = &entry.agent;
        for (field, value) in
            [("name", &agent.name), ("email", &agent.email), ("persona", &agent.persona)]
        {
            if value.trim().is_empty() {
                // The file that supplied THIS entry, which in a layered
                // roster is not always the nearest one.
                return Err(RosterError::IncompleteEntry {
                    title: title.clone(),
                    field,
                    path: entry.source.clone(),
                });
            }
        }
        // ⚠ A ceiling that cannot be spent is refused, not rounded and not
        // quietly read as absent. Absent already means "no ceiling"; a zero, a
        // negative or a NaN is somebody meaning something else and getting it
        // wrong, and folding those into "no ceiling" would answer a request to
        // spend less by removing the limit entirely.
        if let Some(written) = agent.budget_usd {
            if !written.is_finite() || written <= 0.0 {
                return Err(RosterError::InvalidBudget {
                    title: title.clone(),
                    written,
                    path: entry.source.clone(),
                });
            }
        }
        Ok(Resolved { roster: self, title, entry })
    }
}

/// Resolve a configured path against the directory of the file that wrote it.
/// An absolute path is already answered.
fn resolve_against(root: &Path, written: &str) -> PathBuf {
    let written = Path::new(written);
    if written.is_absolute() { written.to_path_buf() } else { root.join(written) }
}

/// A title and the complete entry behind it.
#[derive(Debug, Clone, Copy)]
pub struct Resolved<'a> {
    roster: &'a Roster,
    title: &'a str,
    entry: &'a Entry,
}

impl<'a> Resolved<'a> {
    /// The title this entry is keyed by.
    pub fn title(&self) -> &'a str {
        self.title
    }

    /// The name work is signed under.
    pub fn name(&self) -> &'a str {
        &self.entry.agent.name
    }

    /// The address work is signed under, and the key a credential source is
    /// looked up by — see [`Auth`].
    pub fn email(&self) -> &'a str {
        &self.entry.agent.email
    }

    /// The persona path exactly as the roster writes it.
    pub fn persona(&self) -> &'a str {
        &self.entry.agent.persona
    }

    /// The ceiling a launcher may spend on one run of this agent, in US
    /// dollars. `None` is no ceiling, and is what an entry that says nothing
    /// resolves to.
    pub fn budget_usd(&self) -> Option<f64> {
        self.entry.agent.budget_usd
    }

    /// Whether a launcher records what this agent's run cost. `None` is off.
    pub fn capture_cost(&self) -> bool {
        self.entry.agent.capture_cost.unwrap_or(false)
    }

    /// Whether a launcher labels this agent's runs for external observation.
    /// `None` is off — see [`Agent::tag_runs`] for why the default is not the
    /// convenient one.
    pub fn tag_runs(&self) -> bool {
        self.entry.agent.tag_runs.unwrap_or(false)
    }

    /// The file that supplied this entry, which in a layered roster is not
    /// necessarily the nearest one.
    pub fn defined_in(&self) -> &'a Path {
        &self.entry.source
    }

    /// The persona path resolved against the directory of the file that
    /// *wrote* it — not the nearest file, and not the working directory.
    ///
    /// ⚠ An entry inherited from a workspace-level roster names a persona
    /// beside THAT file. Resolving it against the repo that inherited it
    /// yields a path which works from one directory and not another, and
    /// reads as an environment problem for as long as it takes to stop
    /// believing that.
    pub fn persona_path(&self) -> PathBuf {
        let root = self.entry.source.parent().unwrap_or(Path::new("."));
        resolve_against(root, &self.entry.agent.persona)
    }

    /// The roster this entry came from.
    pub fn roster(&self) -> &'a Roster {
        self.roster
    }

    /// The resolved persona path, having checked that it exists.
    ///
    /// This is what a launcher passes to `--system-prompt-file`, and a path
    /// that points at nothing is a lie that surfaces as an actor behaving
    /// like no one in particular. Every emitter goes through here, so no
    /// output format can be the one that skips the check.
    pub fn require_persona_file(&self) -> Result<PathBuf, RosterError> {
        let persona = self.persona_path();
        if !persona.is_file() {
            return Err(RosterError::MissingPersona {
                title: self.title.to_string(),
                written: self.entry.agent.persona.clone(),
                resolved: persona,
                path: self.entry.source.clone(),
            });
        }
        Ok(persona)
    }

    /// The entry as shell variable assignments, for `eval "$(…)"`.
    pub fn shell_assignments(&self) -> Result<String, RosterError> {
        let persona = self.require_persona_file()?;
        // ⚠ Every emitted key is a literal written here. Deriving one from
        // the file would let a roster introduce an identifier into the
        // caller's shell, and the caller `eval`s this.
        let mut out = format!(
            "AGENT_TITLE={}\nAGENT_NAME={}\nAGENT_EMAIL={}\nAGENT_PERSONA={}\nAGENT_ROSTER={}",
            shell_quote(self.title),
            shell_quote(self.name()),
            shell_quote(self.email()),
            shell_quote(&persona.to_string_lossy()),
            shell_quote(&self.roster.source().to_string_lossy()),
        );
        // ⚠ Absent rather than empty when no ceiling is set, like the
        // credential source below. A launcher tests whether the variable
        // arrived and adds its spend flag only then; emitting `''` would hand
        // an empty argument to every launcher that forgot to check, and an
        // empty argument is the shape a spend limit fails open in.
        if let Some(budget) = self.budget_usd() {
            out.push_str(&format!("\nAGENT_BUDGET_USD={}", shell_quote(&budget.to_string())));
        }
        // ⚠ Present only when switched on, like the ceiling above and the
        // credential below. A launcher tests whether the variable arrived.
        // Emitting `''` for "off" makes every consumer parse a falsy string,
        // and `AGENT_CAPTURE_COST=''` is one careless `[ -n ]` away from
        // meaning its opposite.
        if self.capture_cost() {
            out.push_str("\nAGENT_CAPTURE_COST='1'");
        }
        if self.tag_runs() {
            out.push_str("\nAGENT_TAG_RUNS='1'");
        }
        // ⚠⚠ The credential source, and never the credential. A password put
        // in the environment is inherited by every subprocess — including one
        // launched to act as somebody *else*, which is how an actor ends up
        // authenticating as whoever spawned it while everything appears to
        // work. What is emitted is where to look and which identity to look
        // up (`AGENT_EMAIL`); the lookup is the caller's.
        //
        // Absent rather than empty when unconfigured, so a consumer under
        // `set -u` fails loudly instead of authenticating as nobody.
        //
        // ⚠ Configured-and-missing is emitted like configured-and-present,
        // on purpose, and unlike the persona above. The persona is consumed
        // by every caller of this reader; the credential file only by the
        // callers that authenticate, and the commit-identity flow — the most
        // common caller — never opens it. A refusal here would stop every
        // `agent-env` under a workspace whose file is declared but not yet
        // created, which is the ordinary state between declaring the source
        // and provisioning it. The caller that does look up gets its own
        // refusal from a file that is not there. Revisit if a caller is found
        // that authenticates without checking the open — then the refusal
        // belongs at that open, not here.
        if let Some(auth) = self.roster.auth() {
            out.push_str(&format!(
                "\nAGENT_AUTH_TYPE={}\nAGENT_AUTH_PATH={}",
                shell_quote(auth.kind()),
                shell_quote(&auth.path().to_string_lossy()),
            ));
        }
        Ok(out)
    }
}

/// POSIX single-quoting, `'` escaped as `'\''`. The caller `eval`s the
/// output, so quoting is correctness rather than cosmetics.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

/// Every way reading a roster fails. Each names the file, because "the
/// identity was wrong" is only actionable once you know which roster said so.
#[derive(Debug, Clone)]
pub enum RosterError {
    NotFound { from: PathBuf },
    Unreadable { path: PathBuf, cause: String },
    Malformed { path: PathBuf, cause: String },
    NoDefault { path: PathBuf },
    UnknownTitle { title: String, known: Vec<String>, path: PathBuf },
    IncompleteEntry { title: String, field: &'static str, path: PathBuf },
    MissingPersona { title: String, written: String, resolved: PathBuf, path: PathBuf },
    InvalidBudget { title: String, written: f64, path: PathBuf },
    AmbiguousRoster { folder: PathBuf, bare: PathBuf },
}

impl fmt::Display for RosterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RosterError::NotFound { from } => write!(
                f,
                "no {ROSTER_FILE} found between {} and the filesystem root",
                from.display()
            ),
            RosterError::Unreadable { path, cause } => {
                write!(f, "cannot read {}: {cause}", path.display())
            }
            RosterError::Malformed { path, cause } => {
                write!(f, "{} is not a valid roster: {cause}", path.display())
            }
            RosterError::NoDefault { path } => write!(
                f,
                "no agent asked for and {} sets no default_agent",
                path.display()
            ),
            RosterError::UnknownTitle { title, known, path } => {
                let known =
                    if known.is_empty() { "none".to_string() } else { known.join(", ") };
                write!(
                    f,
                    "no agent titled '{title}' in {} (known: {known})",
                    path.display()
                )
            }
            RosterError::IncompleteEntry { title, field, path } => {
                write!(f, "agent '{title}' has an empty {field} in {}", path.display())
            }
            RosterError::AmbiguousRoster { folder, bare } => write!(
                f,
                "two rosters answer for the same directory and nothing says which is \
                 meant: {} and {}. One of them is a deployment folder and one is that \
                 directory's own roster — resolving either way would assign an identity \
                 nobody chose, so neither is used",
                folder.display(),
                bare.display()
            ),
            RosterError::InvalidBudget { title, written, path } => write!(
                f,
                "agent '{title}' in {} sets budget_usd to {written}, which is not an \
                 amount anything can spend — omit the key for no ceiling",
                path.display()
            ),
            RosterError::MissingPersona { title, written, resolved, path } => write!(
                f,
                "agent '{title}' in {} names the persona '{written}', which resolves to \
                 {} and does not exist",
                path.display(),
                resolved.display()
            ),
        }
    }
}

impl std::error::Error for RosterError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    const SAMPLE: &str = "
default_agent: developer
agents:
  developer:
    name: Ada
    email: ada@example.com
    persona: workflow/DEVELOPER.md
  reviewer:
    name: Grace
    email: grace@example.com
    persona: workflow/REVIEWER.md
";

    /// A workspace holding a parent roster and a child repo beneath it, each
    /// with its own persona directory. Returns the temp dir and the child's
    /// working directory to discover from.
    fn layered(parent: &str, child: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        for (root, text) in [(dir.path().to_path_buf(), parent), (repo.clone(), child)] {
            std::fs::create_dir_all(root.join("workflow")).unwrap();
            std::fs::write(root.join("workflow/DEVELOPER.md"), "# persona").unwrap();
            std::fs::write(root.join("workflow/REVIEWER.md"), "# persona").unwrap();
            std::fs::write(root.join(ROSTER_FILE), text).unwrap();
        }
        (dir, repo)
    }

    /// ⚠⚠ The correctness property layering turns on. A parent's
    /// `workflow/DEVELOPER.md` means the PARENT's `workflow/`, and resolving
    /// an inherited entry against the repo that inherited it gives a path
    /// that works from one directory and not another.
    #[test]
    fn an_inherited_entry_resolves_its_persona_against_the_file_that_wrote_it() {
        let (dir, repo) = layered(
            "agents:\n  reviewer:\n    name: Grace\n    email: g@example.com\n    persona: workflow/REVIEWER.md\n",
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    email: a@example.com\n    persona: workflow/DEVELOPER.md\n",
        );
        let roster = Roster::discover(&repo).unwrap();

        // The child's own entry resolves under the child.
        let dev = roster.resolve(Some("developer")).unwrap();
        assert_eq!(dev.persona_path(), repo.join("workflow/DEVELOPER.md"));
        assert_eq!(dev.defined_in(), repo.join(ROSTER_FILE));

        // The inherited one resolves under the PARENT, not the child — and
        // both files exist, so a wrong join would still be a real file and
        // the mistake would be invisible.
        let rev = roster.resolve(Some("reviewer")).unwrap();
        assert_eq!(rev.persona_path(), dir.path().join("workflow/REVIEWER.md"));
        assert_ne!(rev.persona_path(), repo.join("workflow/REVIEWER.md"));
        rev.require_persona_file().unwrap();
    }

    /// The nearest file wins a title outright — whole, never field-merged.
    #[test]
    fn the_nearest_file_wins_a_title_and_takes_the_entry_whole() {
        let (_dir, repo) = layered(
            "default_agent: reviewer\nagents:\n  developer:\n    name: Parent\n    email: parent@example.com\n    persona: workflow/DEVELOPER.md\n",
            "default_agent: developer\nagents:\n  developer:\n    name: Child\n    email: child@example.com\n    persona: workflow/DEVELOPER.md\n",
        );
        let roster = Roster::discover(&repo).unwrap();
        assert_eq!(roster.default_title(), Some("developer"), "the nearer default wins");
        let dev = roster.resolve(None).unwrap();
        assert_eq!(dev.name(), "Child");
        // ⚠ The whole entry came from the child. A field-merge would have
        // left the parent's address on the child's name — half an identity,
        // which is worse than either of them complete.
        assert_eq!(dev.email(), "child@example.com");
        assert_eq!(roster.sources().len(), 2, "both files contributed");
    }

    /// ⚠ `auth` is replaced as a block, never field-merged: a `type` from one
    /// file meeting a `path` meant for another is configuration nobody wrote.
    /// And its path resolves against its own file, like a persona does.
    #[test]
    fn auth_is_taken_whole_from_the_nearest_file_that_names_it() {
        let (dir, repo) = layered(
            "auth:\n  type: simple\n  path: secrets/actors.json\nagents: {}\n",
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    email: a@example.com\n    persona: workflow/DEVELOPER.md\n",
        );
        // Only the parent names auth, so the child inherits it — resolved
        // against the PARENT's directory.
        let roster = Roster::discover(&repo).unwrap();
        let auth = roster.auth().expect("inherited from the parent");
        assert_eq!(auth.kind(), "simple");
        assert_eq!(auth.path(), dir.path().join("secrets/actors.json"));

        // A child that names its own replaces the block entirely.
        std::fs::write(
            repo.join(ROSTER_FILE),
            "auth:\n  type: simple\n  path: own.json\nagents:\n  developer:\n    name: Ada\n    email: a@example.com\n    persona: workflow/DEVELOPER.md\n",
        )
        .unwrap();
        let roster = Roster::discover(&repo).unwrap();
        assert_eq!(roster.auth().unwrap().path(), repo.join("own.json"));
    }

    /// An auth type this build does not implement is a refusal naming the
    /// file, not a silently ignored block — a deployment that thinks it
    /// configured a keyring and got nothing is the failure to avoid.
    #[test]
    fn an_unknown_auth_type_is_refused_rather_than_ignored() {
        let text = "auth:\n  type: vault\n  path: x\nagents: {}\n";
        let err = Roster::parse(text, "/tmp/config.yaml").unwrap_err();
        assert!(matches!(err, RosterError::Malformed { .. }), "{err}");
        assert!(err.to_string().contains("config.yaml"), "the file must be named: {err}");
    }

    /// ⚠⚠ The emitters hand over the credential SOURCE and the identity to
    /// look up — never the secret. An exported password is inherited by every
    /// subprocess, including one launched to act as a different actor, and
    /// that failure works perfectly while being completely wrong.
    #[test]
    fn the_emitters_carry_where_credentials_live_and_never_a_credential() {
        let (dir, repo) = layered(
            "auth:\n  type: simple\n  path: secrets/actors.json\nagents: {}\n",
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    email: a@example.com\n    persona: workflow/DEVELOPER.md\n",
        );
        // A real credential file sitting where the config points.
        std::fs::create_dir_all(dir.path().join("secrets")).unwrap();
        std::fs::write(
            dir.path().join("secrets/actors.json"),
            r#"{"accounts":{"a@example.com":{"password":"hunter2"}}}"#,
        )
        .unwrap();

        let roster = Roster::discover(&repo).unwrap();
        let block = roster.resolve(None).unwrap().shell_assignments().unwrap();
        assert!(block.contains("AGENT_AUTH_TYPE='simple'"), "{block}");
        assert!(block.contains("AGENT_AUTH_PATH="), "{block}");
        assert!(!block.contains("hunter2"), "a secret reached the environment: {block}");
        assert!(!block.contains("PASSWORD"), "{block}");

        // Unconfigured: absent, not empty. An empty value under `set -u`
        // reads as configured-and-blank, which authenticates as nobody.
        let (_d2, plain) = roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md"]);
        let block = plain.resolve(None).unwrap().shell_assignments().unwrap();
        assert!(!block.contains("AGENT_AUTH"), "{block}");
    }

    /// A declared credential source that does not exist yet is still
    /// emitted — the reader reports where to look, and only a caller that
    /// looks can know whether it needed to. Refusing here would stop the
    /// commit-identity flow, which never opens the file, for as long as a
    /// workspace has declared its source and not provisioned it.
    #[test]
    fn a_declared_auth_path_is_emitted_whether_or_not_the_file_exists_yet() {
        let (dir, repo) = layered(
            "auth:\n  type: simple\n  path: secrets/actors.json\nagents: {}\n",
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    email: a@example.com\n    persona: workflow/DEVELOPER.md\n",
        );
        let missing = dir.path().join("secrets/actors.json");
        assert!(!missing.exists(), "the fixture must declare a file that is not there");

        let roster = Roster::discover(&repo).unwrap();
        let block = roster.resolve(None).unwrap().shell_assignments().unwrap();
        assert!(block.contains("AGENT_AUTH_TYPE='simple'"), "{block}");
        assert!(
            block.contains(&format!("AGENT_AUTH_PATH={}", shell_quote(&missing.to_string_lossy()))),
            "the declared path is reported as declared: {block}"
        );
    }

    /// A roster on disk, with the persona files its entries name.
    fn roster_on_disk(text: &str, personas: &[&str]) -> (tempfile::TempDir, Roster) {
        let dir = tempfile::tempdir().unwrap();
        for persona in personas {
            let path = dir.path().join(persona);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut file = std::fs::File::create(path).unwrap();
            writeln!(file, "# persona").unwrap();
        }
        let source = dir.path().join(ROSTER_FILE);
        std::fs::write(&source, text).unwrap();
        let roster = Roster::load(&source).unwrap();
        (dir, roster)
    }

    #[test]
    fn the_default_entry_resolves_without_being_named() {
        let roster = Roster::parse(SAMPLE, "config.yaml").unwrap();
        let agent = roster.resolve(None).unwrap();
        assert_eq!(agent.title(), "developer");
        assert_eq!(agent.name(), "Ada");
        assert_eq!(agent.email(), "ada@example.com");
        assert_eq!(agent.persona(), "workflow/DEVELOPER.md");
    }

    #[test]
    fn a_named_entry_resolves_past_the_default() {
        let roster = Roster::parse(SAMPLE, "config.yaml").unwrap();
        assert_eq!(roster.resolve(Some("reviewer")).unwrap().name(), "Grace");
    }

    #[test]
    fn a_title_the_roster_does_not_have_names_the_ones_it_does() {
        let roster = Roster::parse(SAMPLE, "config.yaml").unwrap();
        let Err(error) = roster.resolve(Some("archivist")) else {
            panic!("an unknown title resolved");
        };
        let message = error.to_string();
        assert!(message.contains("archivist"), "{message}");
        assert!(message.contains("developer, reviewer"), "{message}");
    }

    /// An identity that fails open is the failure this crate exists to
    /// prevent, so each incomplete shape is an error rather than an empty
    /// variable that reaches a `git -c user.name=` unnoticed.
    #[test]
    fn an_incomplete_entry_is_an_error_not_an_empty_variable() {
        for (missing, text) in [
            ("name", "agents:\n  dev:\n    name: ''\n    email: a@b.c\n    persona: P.md\n"),
            ("email", "agents:\n  dev:\n    name: A\n    email: '  '\n    persona: P.md\n"),
            ("persona", "agents:\n  dev:\n    name: A\n    email: a@b.c\n    persona: ''\n"),
        ] {
            let roster = Roster::parse(text, "config.yaml").unwrap();
            let Err(error) = roster.resolve(Some("dev")) else {
                panic!("an entry with an empty {missing} resolved");
            };
            assert!(error.to_string().contains(missing), "{error}");
        }
    }

    #[test]
    fn a_roster_with_no_default_says_so_rather_than_picking_one() {
        let roster =
            Roster::parse("agents:\n  dev:\n    name: A\n    email: a@b.c\n    persona: P.md\n", "config.yaml")
                .unwrap();
        let Err(error) = roster.resolve(None) else { panic!("a default was invented") };
        assert!(error.to_string().contains("default_agent"), "{error}");
    }

    #[test]
    fn unknown_keys_are_tolerated_because_the_file_is_the_deployments_own() {
        let text = format!("{SAMPLE}\nmerge_severity_floor: medium\nnotify:\n  topic: t\n");
        let roster = Roster::parse(&text, "config.yaml").unwrap();
        assert_eq!(roster.resolve(None).unwrap().title(), "developer");
    }

    #[test]
    fn the_persona_resolves_against_the_roster_not_the_working_directory() {
        let (dir, roster) =
            roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md", "workflow/REVIEWER.md"]);
        let agent = roster.resolve(None).unwrap();
        assert_eq!(agent.persona_path(), dir.path().join("workflow/DEVELOPER.md"));
        assert!(agent.shell_assignments().unwrap().contains("workflow/DEVELOPER.md"));
    }

    /// The emitted persona is what a launcher hands to `--system-prompt-file`.
    /// Emitting a path that points at nothing produces an actor with no
    /// persona and no error, which is the shape worth failing on.
    #[test]
    fn a_persona_path_that_points_at_nothing_refuses_to_emit() {
        let (_dir, roster) = roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md"]);
        let Err(error) = roster.resolve(Some("reviewer")).unwrap().shell_assignments() else {
            panic!("a missing persona emitted anyway");
        };
        let message = error.to_string();
        assert!(message.contains("REVIEWER.md"), "{message}");
        assert!(message.contains("does not exist"), "{message}");
    }

    #[test]
    fn discover_walks_up_from_a_subdirectory() {
        let (dir, _) = roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md", "workflow/REVIEWER.md"]);
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        let found = Roster::discover(&deep).unwrap();
        assert_eq!(found.source(), dir.path().join(ROSTER_FILE));
        assert_eq!(found.resolve(None).unwrap().name(), "Ada");
    }

    #[test]
    fn discover_with_no_roster_above_it_says_where_it_looked() {
        let dir = tempfile::tempdir().unwrap();
        let Err(error) = Roster::discover(dir.path()) else { panic!("a roster was invented") };
        assert!(error.to_string().contains(ROSTER_FILE), "{error}");
    }

    #[test]
    fn discover_finds_the_ferrostep_folder_convention() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("FerroStep/personas")).unwrap();
        std::fs::write(dir.path().join("FerroStep/personas/DEVELOPER.md"), "# persona").unwrap();
        std::fs::write(
            dir.path().join("FerroStep/config.yaml"),
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    \
             email: a@example.com\n    persona: personas/DEVELOPER.md\n",
        )
        .unwrap();

        let deep = dir.path().join("a/b");
        std::fs::create_dir_all(&deep).unwrap();
        let found = Roster::discover(&deep).unwrap();

        assert_eq!(found.source(), dir.path().join("FerroStep/config.yaml"));
        let dev = found.resolve(None).unwrap();
        assert_eq!(dev.name(), "Ada");
        assert_eq!(dev.persona_path(), dir.path().join("FerroStep/personas/DEVELOPER.md"));
    }

    /// A directory carrying both shapes is not a real deployment state (a
    /// repo adopts the folder or it does not), but the precedence has to be
    /// *something* rather than an arbitrary directory-read order, so this
    /// pins the folder as the winner.
    #[test]
    fn a_deployment_folder_and_a_bare_file_at_one_level_is_refused_not_ranked() {
        // ⚠ This asserted the opposite until 2026-09-08: that the folder simply
        // won. That ranking is what let a checkout sharing the name shadow a
        // workspace's own roster, so it is gone. A folder that looks like a
        // deployment, beside a bare file, is two answers for one directory and
        // is refused; a folder that does not look like one is not a deployment
        // folder at all, and the level's own file answers.
        let dir = tempfile::tempdir().unwrap();
        let marked = dir.path().join(ROSTER_DIR).join(DEPLOYMENT_MARKER);
        std::fs::create_dir_all(&marked).unwrap();
        std::fs::write(dir.path().join("FerroStep/config.yaml"), "default_agent: folder\n").unwrap();
        std::fs::write(dir.path().join(ROSTER_FILE), "default_agent: bare\n").unwrap();
        assert!(matches!(
            Roster::discover(dir.path()).unwrap_err(),
            RosterError::AmbiguousRoster { .. }
        ));

        std::fs::remove_dir(&marked).unwrap();
        let found = Roster::discover(dir.path()).unwrap();
        assert_eq!(found.source(), dir.path().join(ROSTER_FILE));
        assert_eq!(found.default_title(), Some("bare"));
    }

    /// A repo that has not migrated keeps working unchanged — this is the
    /// back-compat half, and it is what lets FerroStep's own repo (whose
    /// `config.yaml` stays at its root) and a migrated consumer coexist in
    /// the same workspace with the same reader.
    #[test]
    fn a_bare_file_is_still_found_when_no_ferrostep_folder_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(ROSTER_FILE), "default_agent: bare\n").unwrap();
        let found = Roster::discover(dir.path()).unwrap();
        assert_eq!(found.source(), dir.path().join(ROSTER_FILE));
    }

    /// The two shapes layer across levels exactly like two bare files do —
    /// a workspace layer does not have to have migrated for a repo beneath
    /// it to, or the other way round.
    #[test]
    fn the_two_shapes_layer_across_levels_like_one_shape_does() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(ROSTER_FILE),
            "agents:\n  reviewer:\n    name: Grace\n    email: g@example.com\n    \
             persona: workflow/REVIEWER.md\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("repo/FerroStep")).unwrap();
        std::fs::write(
            dir.path().join("repo/FerroStep/config.yaml"),
            "default_agent: developer\nagents:\n  developer:\n    name: Ada\n    \
             email: a@example.com\n    persona: personas/DEVELOPER.md\n",
        )
        .unwrap();

        let found = Roster::discover(dir.path().join("repo")).unwrap();
        assert_eq!(found.resolve(Some("developer")).unwrap().name(), "Ada");
        // Inherited from the parent's bare file, resolved against the PARENT.
        let rev = found.resolve(Some("reviewer")).unwrap();
        assert_eq!(rev.name(), "Grace");
        assert_eq!(rev.persona_path(), dir.path().join("workflow/REVIEWER.md"));
    }

    #[test]
    fn shell_quote_survives_embedded_quotes() {
        assert_eq!(shell_quote("O'Malley"), r"'O'\''Malley'");
    }

    /// The emitted text is `eval`ed by the caller's shell, so a roster's
    /// contents become executable text and quoting is the only thing between
    /// the two. Asserting the quoted *form* only tests what we believe a
    /// shell does with it; this hands it to a real one and reads it back.
    #[test]
    fn a_value_reaches_the_shell_as_itself_whatever_is_in_it() {
        for hostile in [
            "O'Malley",
            "; rm -rf /",
            "$(id)",
            "`id`",
            "a\nb",
            "$HOME",
            r#"double " and single ' together"#,
            "",
        ] {
            let script = format!("V={}; printf %s \"$V\"", shell_quote(hostile));
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .output()
                .expect("sh is available");
            assert!(out.status.success(), "sh refused: {script}");
            assert_eq!(String::from_utf8_lossy(&out.stdout), hostile, "did not survive: {script}");
        }
    }

    /// A roster cannot introduce an identifier into the caller's shell. The
    /// title is the one roster-controlled value that could plausibly reach
    /// key position, since it is the only one the emitter also prints as a
    /// name; a hostile one must arrive as the *value* of `AGENT_TITLE` and
    /// define nothing.
    ///
    /// ⚠ Asked of a real shell on purpose. The first version of this test
    /// scanned this file for interpolated keys and failed on its own
    /// assertion text — a guard that reads source is testing the source, not
    /// the behaviour.
    #[test]
    fn a_hostile_title_becomes_a_value_and_never_a_variable() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("P.md"), "# persona").unwrap();
        let source = dir.path().join(ROSTER_FILE);
        std::fs::write(
            &source,
            "agents:\n  \"EVIL=owned; x\":\n    name: A\n    email: a@b.c\n    persona: P.md\n",
        )
        .unwrap();
        let roster = Roster::load(&source).unwrap();
        let block =
            roster.resolve(Some("EVIL=owned; x")).unwrap().shell_assignments().unwrap();
        let script = format!("{block}\nprintf '%s|%s' \"$AGENT_TITLE\" \"${{EVIL-unset}}\"");
        let out = std::process::Command::new("sh").arg("-c").arg(&script).output().unwrap();
        assert!(out.status.success(), "sh refused the emitted block");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "EVIL=owned; x|unset");
    }

    /// A caller `eval`s this, so what matters is the variables a real shell
    /// ends up holding — not the text.
    #[test]
    fn the_emitted_block_evals_to_the_entry() {
        let (dir, roster) =
            roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md", "workflow/REVIEWER.md"]);
        let block = roster.resolve(Some("reviewer")).unwrap().shell_assignments().unwrap();
        let script = format!(
            "{block}\nprintf '%s|%s|%s|%s|%s' \
             \"$AGENT_TITLE\" \"$AGENT_NAME\" \"$AGENT_EMAIL\" \"$AGENT_PERSONA\" \"$AGENT_ROSTER\""
        );
        let out = std::process::Command::new("sh").arg("-c").arg(&script).output().unwrap();
        assert!(out.status.success(), "sh refused the emitted block");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!(
                "reviewer|Grace|grace@example.com|{}|{}",
                dir.path().join("workflow/REVIEWER.md").display(),
                dir.path().join(ROSTER_FILE).display()
            )
        );
    }

    /// SAMPLE with a ceiling on the reviewer — the owner's case, though the
    /// key is available to any entry.
    fn with_budget(written: &str) -> String {
        SAMPLE.replace(
            "    persona: workflow/REVIEWER.md",
            &format!("    persona: workflow/REVIEWER.md\n    budget_usd: {written}"),
        )
    }

    #[test]
    fn a_budget_is_absent_from_the_block_until_a_roster_sets_one() {
        let (_dir, roster) = roster_on_disk(SAMPLE, &["workflow/REVIEWER.md"]);
        let entry = roster.resolve(Some("reviewer")).unwrap();
        assert_eq!(entry.budget_usd(), None);
        assert!(!entry.shell_assignments().unwrap().contains("AGENT_BUDGET_USD"));
    }

    /// ⚠ The flag below is a stand-in and deliberately not any real launcher's
    /// spelling. What is under test is the *shape* of the caller's idiom — no
    /// variable, no flag; a variable, the amount — because the reader emits a
    /// quantity and the launcher owns the command line.
    #[test]
    fn a_budget_reaches_the_shell_as_the_amount_written() {
        let (_dir, roster) = roster_on_disk(&with_budget("12.5"), &["workflow/REVIEWER.md"]);
        let entry = roster.resolve(Some("reviewer")).unwrap();
        assert_eq!(entry.budget_usd(), Some(12.5));
        let block = entry.shell_assignments().unwrap();
        let script = format!(
            "{block}\nset -u\nprintf '%s' \"${{AGENT_BUDGET_USD:+--spend-cap $AGENT_BUDGET_USD}}\""
        );
        let out = std::process::Command::new("sh").arg("-c").arg(&script).output().unwrap();
        assert!(out.status.success(), "sh refused the emitted block");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "--spend-cap 12.5");
    }

    /// ⚠⚠ The direction that matters: an unspendable ceiling must not fold
    /// into "no ceiling". Reading a `0` as absent would answer a request to
    /// spend less by removing the limit, and nothing downstream would say so.
    #[test]
    fn a_budget_that_cannot_be_spent_is_refused_rather_than_read_as_no_ceiling() {
        for written in ["0", "-5", "0.0", ".nan", ".inf"] {
            let (_dir, roster) = roster_on_disk(&with_budget(written), &["workflow/REVIEWER.md"]);
            let err = match roster.resolve(Some("reviewer")) {
                Err(err) => err,
                Ok(entry) => panic!("budget_usd: {written} resolved to {:?}", entry.budget_usd()),
            };
            assert!(matches!(err, RosterError::InvalidBudget { .. }), "{written}: {err}");
            assert!(err.to_string().contains(ROSTER_FILE), "the refusal names no file: {err}");
        }
    }

    #[test]
    fn a_switch_is_absent_from_the_block_until_a_roster_turns_it_on() {
        let (_dir, roster) = roster_on_disk(SAMPLE, &["workflow/REVIEWER.md"]);
        let entry = roster.resolve(Some("reviewer")).unwrap();
        assert!(!entry.capture_cost());
        assert!(!entry.tag_runs());
        let block = entry.shell_assignments().unwrap();
        assert!(!block.contains("AGENT_CAPTURE_COST"), "{block}");
        assert!(!block.contains("AGENT_TAG_RUNS"), "{block}");
    }

    /// ⚠ The caller's idiom is "did the variable arrive", so what is under
    /// test is that an off switch leaves nothing behind for `-n` to find —
    /// not that it emits a falsy string, which is the shape that reads as
    /// its own opposite one careless test later.
    #[test]
    fn a_switch_that_is_on_reaches_the_shell_as_a_variable_that_is_set() {
        let text = SAMPLE.replace(
            "    persona: workflow/REVIEWER.md",
            "    persona: workflow/REVIEWER.md\n    capture_cost: true\n    tag_runs: false",
        );
        let (_dir, roster) = roster_on_disk(&text, &["workflow/REVIEWER.md"]);
        let entry = roster.resolve(Some("reviewer")).unwrap();
        assert!(entry.capture_cost());
        assert!(!entry.tag_runs(), "an explicit false is off, like an absent key");
        let block = entry.shell_assignments().unwrap();
        let script = format!(
            "{block}\nset -u\nprintf '%s|%s' \"${{AGENT_CAPTURE_COST:+capture}}\" \"${{AGENT_TAG_RUNS:+tag}}\""
        );
        let out = std::process::Command::new("sh").arg("-c").arg(&script).output().unwrap();
        assert!(out.status.success(), "sh refused the emitted block");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "capture|");
    }

    /// ⚠⚠ **THE ADOPTER'S CASE, 2026-09-08.** A workspace holding several
    /// repos side by side contained a checkout named like the deployment
    /// folder. It matched, it won, the workspace's own roster was skipped, and
    /// every sibling repo without a roster resolved as that checkout's agent
    /// and exited 0.
    #[test]
    fn a_directory_that_only_shares_the_folder_name_does_not_shadow_the_level_s_own_roster() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // A checkout that happens to be called FerroStep: a roster, no personas.
        std::fs::create_dir_all(root.join(ROSTER_DIR)).unwrap();
        std::fs::write(root.join(ROSTER_DIR).join(ROSTER_FILE), SAMPLE).unwrap();
        // The level's own roster, which is the one that means this directory.
        std::fs::write(
            root.join(ROSTER_FILE),
            "default_agent: floater\nagents:\n  floater:\n    name: Workspace\n    email: ws@example.com\n    persona: W.md\n",
        )
        .unwrap();
        std::fs::write(root.join("W.md"), "# w").unwrap();
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();

        let roster = Roster::discover(&child).unwrap();
        assert_eq!(
            roster.resolve(None).unwrap().name(),
            "Workspace",
            "the checkout shadowed the workspace's own roster again"
        );
    }

    /// ⚠ And the other half: a folder that really is a deployment folder,
    /// beside a bare file at the same level, is the case the convention said
    /// could not happen. Nobody can say which was meant, so neither is used —
    /// an identity nobody chose is the failure this crate exists to refuse.
    #[test]
    fn a_real_deployment_folder_beside_a_bare_roster_refuses_and_names_both() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(ROSTER_DIR).join(DEPLOYMENT_MARKER)).unwrap();
        std::fs::write(root.join(ROSTER_DIR).join(ROSTER_FILE), SAMPLE).unwrap();
        std::fs::write(root.join(ROSTER_FILE), SAMPLE).unwrap();

        let err = Roster::discover(root).unwrap_err();
        assert!(matches!(err, RosterError::AmbiguousRoster { .. }), "{err}");
        let said = err.to_string();
        assert!(said.contains(ROSTER_DIR), "the refusal does not name the folder: {said}");
        assert!(said.contains(&root.join(ROSTER_FILE).display().to_string()), "{said}");
    }

    /// ⚠⚠ **Identity does not inherit; a credential SOURCE does.** A roster
    /// that reaches only its own level answers an agent standing in it and
    /// refuses for a repo beneath it that declared nobody — while its `auth`
    /// goes on layering, because where a deployment keeps credentials is not
    /// an identity.
    #[test]
    fn a_roster_that_reaches_here_answers_its_own_level_and_not_the_one_below() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join(ROSTER_FILE),
            "agents_reach: here\ndefault_agent: floater\nagents:\n  floater:\n    name: Workspace\n    email: ws@example.com\n    persona: W.md\nauth:\n  type: simple\n  path: creds.yaml\n",
        )
        .unwrap();
        std::fs::write(root.join("W.md"), "# w").unwrap();
        let child = root.join("child");
        std::fs::create_dir_all(&child).unwrap();

        // Standing in it: answered.
        assert_eq!(Roster::discover(root).unwrap().resolve(None).unwrap().name(), "Workspace");

        // A directory beneath it that declared nobody: refused, not inherited.
        let below = Roster::discover(&child).unwrap();
        assert!(below.resolve(None).is_err(), "identity inherited into a repo that declared none");
        // ...and the credential source still reached it.
        assert!(below.auth().is_some(), "auth stopped layering, which is not identity");
    }

    /// The default is `below`, so everything written before the key existed
    /// keeps resolving from its own subdirectories.
    #[test]
    fn a_roster_that_says_nothing_still_reaches_its_own_subdirectories() {
        let (dir, roster) = roster_on_disk(SAMPLE, &["workflow/DEVELOPER.md"]);
        let deep = dir.path().join("a/b/c");
        std::fs::create_dir_all(&deep).unwrap();
        let _ = roster;
        assert_eq!(Roster::discover(&deep).unwrap().resolve(None).unwrap().name(), "Ada");
    }

    /// ⚠ The guard AGENTS.md names. Every emitted key is written out as a
    /// literal, so this pins the whole set: a sixth, seventh or eighth cannot
    /// appear without a line here changing. A count belongs in a test and not
    /// in prose precisely because this fails when reality moves.
    #[test]
    fn the_emitted_keys_are_fixed_not_derived() {
        let keys = |block: &str| -> Vec<String> {
            block.lines().filter_map(|l| l.split_once('=').map(|(k, _)| k.to_string())).collect()
        };

        let (_dir, plain) = roster_on_disk(SAMPLE, &["workflow/REVIEWER.md"]);
        let block = plain.resolve(Some("reviewer")).unwrap().shell_assignments().unwrap();
        assert_eq!(
            keys(&block),
            ["AGENT_TITLE", "AGENT_NAME", "AGENT_EMAIL", "AGENT_PERSONA", "AGENT_ROSTER"],
            "an entry configuring nothing optional emits the identity keys and no others"
        );

        let text = with_budget("5").replace(
            "    budget_usd: 5",
            "    budget_usd: 5\n    capture_cost: true\n    tag_runs: true",
        );
        let text = format!("{text}\nauth:\n  type: simple\n  path: creds.yaml\n");
        let (_dir2, full) = roster_on_disk(&text, &["workflow/REVIEWER.md"]);
        let block = full.resolve(Some("reviewer")).unwrap().shell_assignments().unwrap();
        assert_eq!(
            keys(&block),
            [
                "AGENT_TITLE",
                "AGENT_NAME",
                "AGENT_EMAIL",
                "AGENT_PERSONA",
                "AGENT_ROSTER",
                "AGENT_BUDGET_USD",
                "AGENT_CAPTURE_COST",
                "AGENT_TAG_RUNS",
                "AGENT_AUTH_TYPE",
                "AGENT_AUTH_PATH",
            ],
            "every optional key configured at once — the widest this block gets"
        );
    }
}

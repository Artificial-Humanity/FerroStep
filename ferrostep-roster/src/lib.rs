//! ferrostep-roster — the actors of a refereed loop.
//!
//! The engine answers *what may be done*, from a workflow definition. This
//! answers *who is doing it*: an agent's title, the identity it signs work
//! under, and the document that tells it how to behave. Both are data a
//! deployment configures, and neither is compiled in.
//!
//! A roster is a `config.yaml` at the root of the repo the loop works on:
//!
//! ```yaml
//! default_agent: developer
//! agents:
//!   developer:
//!     name: Ada
//!     email: ada@example.com
//!     persona: workflow/DEVELOPER.md
//!   reviewer:
//!     name: Grace
//!     email: grace@example.com
//!     persona: workflow/REVIEWER.md
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

/// The file a roster lives in, at the root of the repo it describes.
pub const ROSTER_FILE: &str = "config.yaml";

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
}

/// One agent's entry, and the file that supplied it.
#[derive(Debug, Clone)]
struct Entry {
    agent: Agent,
    source: PathBuf,
}

/// One agent's entry.
#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    name: String,
    email: String,
    persona: String,
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
    pub fn discover(start: impl AsRef<Path>) -> Result<Roster, RosterError> {
        let start = start.as_ref();
        let mut found = Vec::new();
        let mut dir = start.to_path_buf();
        loop {
            let candidate = dir.join(ROSTER_FILE);
            if candidate.is_file() {
                found.push(candidate);
            }
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
        for path in found {
            layers.push(Roster::load(path)?);
        }
        Ok(Roster::layer(layers))
    }

    /// Fold rosters together, furthest first, so the last wins.
    ///
    /// ⚠ **`agents` merges per title and `auth` does not merge at all.** A
    /// title is taken from the nearest file that names it, *whole* — entries
    /// are never field-merged, because half an identity assembled from two
    /// files is worse than either of them complete. `auth` is replaced as a
    /// block for the sharper version of the same reason: a `type` from one
    /// file meeting a `path` meant for another is a configuration nobody
    /// wrote and nobody can debug.
    fn layer(layers: Vec<Roster>) -> Roster {
        let mut merged = Roster {
            sources: Vec::new(),
            default_agent: None,
            agents: BTreeMap::new(),
            auth: None,
        };
        for layer in layers {
            merged.sources.extend(layer.sources);
            if layer.default_agent.is_some() {
                merged.default_agent = layer.default_agent;
            }
            if layer.auth.is_some() {
                merged.auth = layer.auth;
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
        Ok(Roster { sources: vec![source], default_agent: file.default_agent, agents, auth })
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
        // ⚠⚠ The credential source, and never the credential. A password put
        // in the environment is inherited by every subprocess — including one
        // launched to act as somebody *else*, which is how an actor ends up
        // authenticating as whoever spawned it while everything appears to
        // work. What is emitted is where to look and which identity to look
        // up (`AGENT_EMAIL`); the lookup is the caller's.
        //
        // Absent rather than empty when unconfigured, so a consumer under
        // `set -u` fails loudly instead of authenticating as nobody.
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
}

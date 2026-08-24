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

/// A parsed roster, and the file it was read from. The source travels with
/// it because a `persona` is written relative to the roster's own directory,
/// and because an error that cannot name the file it read is hard to act on.
#[derive(Debug, Clone)]
pub struct Roster {
    source: PathBuf,
    default_agent: Option<String>,
    agents: BTreeMap<String, Agent>,
}

/// One agent's entry.
#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    name: String,
    email: String,
    persona: String,
}

/// The deserialized shape. Separate from [`Roster`] so the public type can
/// carry its source and keep its invariants.
#[derive(Debug, Deserialize)]
struct RosterFile {
    #[serde(default)]
    default_agent: Option<String>,
    #[serde(default)]
    agents: BTreeMap<String, Agent>,
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
    pub fn discover(start: impl AsRef<Path>) -> Result<Roster, RosterError> {
        let start = start.as_ref();
        let mut dir = start.to_path_buf();
        loop {
            let candidate = dir.join(ROSTER_FILE);
            if candidate.is_file() {
                return Roster::load(candidate);
            }
            if !dir.pop() {
                return Err(RosterError::NotFound { from: start.to_path_buf() });
            }
        }
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
        Ok(Roster { source, default_agent: file.default_agent, agents: file.agents })
    }

    /// The file this roster was read from.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// The directory a `persona` path is relative to — the roster's own.
    pub fn root(&self) -> &Path {
        self.source.parent().unwrap_or(Path::new("."))
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
                path: self.source.clone(),
            })?,
        };
        let (title, agent) = self.agents.get_key_value(title).ok_or_else(|| {
            RosterError::UnknownTitle {
                title: title.to_string(),
                known: self.titles().into_iter().map(str::to_owned).collect(),
                path: self.source.clone(),
            }
        })?;
        for (field, value) in
            [("name", &agent.name), ("email", &agent.email), ("persona", &agent.persona)]
        {
            if value.trim().is_empty() {
                return Err(RosterError::IncompleteEntry {
                    title: title.clone(),
                    field,
                    path: self.source.clone(),
                });
            }
        }
        Ok(Resolved { roster: self, title, agent })
    }
}

/// A title and the complete entry behind it.
#[derive(Debug, Clone, Copy)]
pub struct Resolved<'a> {
    roster: &'a Roster,
    title: &'a str,
    agent: &'a Agent,
}

impl<'a> Resolved<'a> {
    /// The title this entry is keyed by.
    pub fn title(&self) -> &'a str {
        self.title
    }

    /// The name work is signed under.
    pub fn name(&self) -> &'a str {
        &self.agent.name
    }

    /// The address work is signed under.
    pub fn email(&self) -> &'a str {
        &self.agent.email
    }

    /// The persona path exactly as the roster writes it.
    pub fn persona(&self) -> &'a str {
        &self.agent.persona
    }

    /// The persona path resolved against the roster's directory. A caller
    /// hands this to a launcher without having to know where the roster was
    /// found, which is the join it would otherwise get wrong.
    pub fn persona_path(&self) -> PathBuf {
        let written = Path::new(&self.agent.persona);
        if written.is_absolute() { written.to_path_buf() } else { self.roster.root().join(written) }
    }

    /// The roster this entry came from.
    pub fn roster(&self) -> &'a Roster {
        self.roster
    }

    /// The entry as shell variable assignments, for `eval "$(…)"`.
    ///
    /// The persona is emitted resolved and is **checked to exist**: this
    /// output is what a launcher passes to `--system-prompt-file`, and a path
    /// that points at nothing is a lie that surfaces as an actor behaving
    /// like no one in particular.
    pub fn shell_assignments(&self) -> Result<String, RosterError> {
        let persona = self.persona_path();
        if !persona.is_file() {
            return Err(RosterError::MissingPersona {
                title: self.title.to_string(),
                written: self.agent.persona.clone(),
                resolved: persona,
                path: self.roster.source.clone(),
            });
        }
        // ⚠ Every emitted key is a literal written here. Deriving one from
        // the file would let a roster introduce an identifier into the
        // caller's shell, and the caller `eval`s this.
        Ok(format!(
            "AGENT_TITLE={}\nAGENT_NAME={}\nAGENT_EMAIL={}\nAGENT_PERSONA={}\nAGENT_ROSTER={}",
            shell_quote(self.title),
            shell_quote(self.name()),
            shell_quote(self.email()),
            shell_quote(&persona.to_string_lossy()),
            shell_quote(&self.roster.source.to_string_lossy()),
        ))
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

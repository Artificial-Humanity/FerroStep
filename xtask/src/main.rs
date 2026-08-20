//! `cargo xtask` — repo tooling for FerroStep. Not a product crate.
//!
//! Subcommands:
//!   agent-env [--agent <title>]   Print an agent's config.yaml entry as
//!                                 shell-safe variable assignments, for
//!                                 `eval "$(cargo xtask agent-env)"`.
//!
//! Everything here fails loudly. An identity that fails open commits as the
//! wrong author silently, so a missing file, an unknown title, or an
//! incomplete entry is an error on stderr and a non-zero exit — never an
//! empty string.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use serde::Deserialize;

/// The repo's working configuration (`config.yaml` at the root). Unknown keys
/// are deliberately tolerated: the config may grow values this reader has no
/// business rejecting.
#[derive(Debug, Deserialize)]
struct Config {
    default_agent: Option<String>,
    #[serde(default)]
    agents: BTreeMap<String, Agent>,
}

#[derive(Debug, Deserialize)]
struct Agent {
    name: String,
    email: String,
    persona: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("agent-env") => agent_env(args),
        Some(other) => Err(format!("unknown subcommand '{other}' (expected: agent-env)")),
        None => Err("no subcommand given (expected: agent-env)".into()),
    }
}

fn agent_env(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let requested = match args.next().as_deref() {
        Some("--agent") => Some(args.next().ok_or("--agent needs a title")?),
        Some(other) => return Err(format!("unknown argument '{other}'")),
        None => None,
    };

    let path = find_config()?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let config: Config = serde_norway::from_str(&text)
        .map_err(|e| format!("{} is not a valid config: {e}", path.display()))?;

    let title = requested
        .or(config.default_agent)
        .ok_or("no --agent given and config.yaml sets no default_agent")?;
    let agent = config.agents.get(&title).ok_or_else(|| {
        let known: Vec<&str> = config.agents.keys().map(String::as_str).collect();
        let known = if known.is_empty() { "none".into() } else { known.join(", ") };
        format!("no agent titled '{title}' in config.yaml (known: {known})")
    })?;
    for (label, value) in [
        ("name", &agent.name),
        ("email", &agent.email),
        ("persona", &agent.persona),
    ] {
        if value.trim().is_empty() {
            return Err(format!("agent '{title}' has an empty {label} in config.yaml"));
        }
    }

    println!("AGENT_TITLE={}", shell_quote(&title));
    println!("AGENT_NAME={}", shell_quote(&agent.name));
    println!("AGENT_EMAIL={}", shell_quote(&agent.email));
    println!("AGENT_PERSONA={}", shell_quote(&agent.persona));
    Ok(())
}

/// Walk up from the current directory, so `cargo xtask` works from any
/// subdirectory of the repo. An absolute path baked in at compile time would
/// go stale if the checkout moved.
fn find_config() -> Result<PathBuf, String> {
    let mut dir =
        std::env::current_dir().map_err(|e| format!("cannot read current dir: {e}"))?;
    loop {
        let candidate = dir.join("config.yaml");
        if candidate.is_file() {
            return Ok(candidate);
        }
        if !dir.pop() {
            return Err("no config.yaml found between here and the filesystem root".into());
        }
    }
}

/// POSIX single-quoting, `'` escaped as `'\''`. The caller `eval`s this
/// output, so quoting is correctness, not cosmetics.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Configurable" is a claim to test: this proves the shipped config.yaml
    /// parses, its default agent resolves to a complete entry, and the
    /// persona path points at a real file — the roster cannot silently rot.
    #[test]
    fn repo_config_parses_and_default_agent_resolves() {
        let text = include_str!("../../config.yaml");
        let config: Config = serde_norway::from_str(text).unwrap();
        let title = config.default_agent.expect("config.yaml sets no default_agent");
        let agent = config
            .agents
            .get(&title)
            .unwrap_or_else(|| panic!("default_agent '{title}' has no agents entry"));
        for (label, value) in [
            ("name", &agent.name),
            ("email", &agent.email),
            ("persona", &agent.persona),
        ] {
            assert!(!value.trim().is_empty(), "default agent has an empty {label}");
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert!(
            root.join(&agent.persona).is_file(),
            "persona file '{}' does not exist",
            agent.persona
        );
    }

    #[test]
    fn shell_quote_survives_embedded_quotes() {
        assert_eq!(shell_quote("O'Malley"), r"'O'\''Malley'");
    }
}

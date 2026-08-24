//! `cargo xtask` — repo tooling for FerroStep. Not a product crate.
//!
//! Subcommands:
//!   agent-env [--agent <title>]   Print an agent's config.yaml entry as
//!                                 shell-safe variable assignments, for
//!                                 `eval "$(cargo xtask agent-env)"`.
//!
//! ⚠ **The reading is `ferrostep-roster`'s, not this file's.** `ferrostep
//! agent-env` is the same command for a repo with no Rust toolchain, and two
//! readers of one file format would drift the moment either grew a rule. This
//! exists because working *inside* FerroStep should not require installing
//! FerroStep first.
//!
//! Everything here fails loudly. An identity that fails open commits as the
//! wrong author silently, so a missing file, an unknown title, or an
//! incomplete entry is an error on stderr and a non-zero exit — never an
//! empty string.

use std::process::ExitCode;

use ferrostep_roster::Roster;

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

    let roster = Roster::discover_from_cwd().map_err(|e| e.to_string())?;
    let agent = roster.resolve(requested.as_deref()).map_err(|e| e.to_string())?;
    println!("{}", agent.shell_assignments().map_err(|e| e.to_string())?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// "Configurable" is a claim to test: this proves the shipped config.yaml
    /// parses, its default agent resolves to a complete entry, and the
    /// persona path points at a real file — the roster cannot silently rot.
    ///
    /// The parsing rules themselves are `ferrostep-roster`'s to test. What is
    /// tested here is *this repo's* roster, which that crate knows nothing
    /// about.
    #[test]
    fn repo_config_parses_and_default_agent_resolves() {
        let roster = Roster::load(repo_root().join("config.yaml")).unwrap();
        // `shell_assignments` is what the emitter actually calls, and it is
        // where completeness and the persona's existence are checked. Asking
        // for it here tests the same path a caller takes rather than a
        // weaker restatement of it.
        let agent = roster.resolve(None).unwrap();
        agent.shell_assignments().unwrap();
        // ⚠ Existing is not the requirement — being TRACKED is. The persona is
        // what CLAUDE.md imports, so an untracked one works perfectly for the
        // check above and leaves a fresh clone with an agent that has no
        // procedures. Those two questions are asked of different things:
        // `is_file` reads the working tree, this reads the index, and they
        // disagree in exactly the case worth catching. Measured elsewhere in
        // this workspace: a full suite passed green while the file it
        // depended on was untracked.
        assert!(
            tracked_files(&repo_root()).contains(agent.persona()),
            "persona file '{}' exists but is not tracked by git — it would be \
             absent from a fresh clone, and CLAUDE.md imports it",
            agent.persona()
        );
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
    }

    /// The repo's tracked paths, from the index rather than the working tree.
    fn tracked_files(root: &std::path::Path) -> std::collections::BTreeSet<String> {
        let output = std::process::Command::new("git")
            .arg("ls-files")
            .current_dir(root)
            .output()
            .expect("could not run git ls-files");
        assert!(output.status.success(), "git ls-files exited nonzero");
        String::from_utf8(output.stdout)
            .expect("git ls-files returned non-utf8")
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Every tracked top-level path must be mentioned in the deployment map,
    /// so adding a directory forces classifying it (Ships or Never ships).
    /// This is a MENTION check, not a truth check: green means "nothing is
    /// unclassified", never "every classification is right". It reads the
    /// git index, so an untracked path is invisible to it until `git add`.
    #[test]
    fn deployment_map_covers_the_tree() {
        let root = repo_root();
        let map = std::fs::read_to_string(root.join("docs/deployment-map.md"))
            .expect("docs/deployment-map.md is missing");
        let tracked = tracked_files(&root);
        let top_level: std::collections::BTreeSet<&str> = tracked
            .iter()
            .filter_map(|line| line.split('/').next())
            .filter(|segment| !segment.is_empty())
            .collect();
        // ⚠ An enumerating guard needs a floor, and the floor belongs on the
        // population that MATTERS rather than on whatever list came before it.
        // With nothing enumerated there is nothing to find missing, so this
        // test reports success for the one input that proves it checked
        // nothing at all.
        assert!(
            !top_level.is_empty(),
            "no tracked top-level paths were enumerated — this guard checked \
             nothing, which is not the same as finding nothing wrong"
        );
        let missing: Vec<&str> = top_level
            .into_iter()
            .filter(|name| !map.contains(name))
            .collect();
        assert!(
            missing.is_empty(),
            "top-level paths with no mention in docs/deployment-map.md: {missing:?} — \
             classify each under Ships or Never ships"
        );
    }
}

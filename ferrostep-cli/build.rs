//! Captures which commit this binary was built from.
//!
//! ⚠⚠ **The crate version alone cannot answer the question this exists for.**
//! Between releases the workspace carries a pre-release number, so every build
//! in that window reports the same string — and what a reporter is being asked
//! is *which build are you running*, which that string cannot distinguish. A
//! version flag printing only the package version would look like it worked
//! and answer nothing.
//!
//! ⚠ Best effort, and it says so rather than guessing: a build with no git
//! available reports the commit as unknown, which is a worse answer than a
//! commit and a far better one than a confident wrong commit.

use std::process::Command;

fn main() {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let build = match git(&["rev-parse", "--short=7", "HEAD"]) {
        None => "commit unknown".to_string(),
        Some(commit) => match git(&["status", "--porcelain"]) {
            // ⚠ A build from a modified tree is not any commit, and saying so
            // is the difference between a version string and a claim. Measured
            // here 2026-09-08: an installed binary predated its own fix by
            // sixteen minutes while everything about it looked current, and
            // only a timestamp comparison nobody is prompted to make caught it.
            Some(status) if !status.is_empty() => format!("{commit}+changes"),
            _ => commit,
        },
    };
    println!("cargo:rustc-env=FERROSTEP_BUILD={build}");
}

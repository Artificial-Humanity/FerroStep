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

    // ⚠⚠ **A stamp that cannot go stale is the whole point, and cargo does not
    // give you that for free.** A build script naming no input is rerun when a
    // file in *its own package* changes — and on nothing else. Measured here
    // 2026-09-09: between 5fed239 and f8d34c8 no file under `ferrostep-cli/`
    // changed, so `cargo install --path . --force` relinked the binary and
    // reused the previous run's `FERROSTEP_BUILD`. The flag then reported a
    // commit the binary was not built from — the confident wrong commit the
    // header calls the worst of the three answers, arrived at by caching.
    //
    // ⚠ Naming the refs below is what makes the stamp follow HEAD. Naming the
    // sources beside them is not decoration: **the first `rerun-if-changed`
    // line switches the package-wide default off**, so leaving them out would
    // buy commit tracking by giving up the edit tracking that already worked.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if let Some(dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        // HEAD alone is enough while it is detached, because it then holds the
        // commit itself. Attached, it holds a ref name that does not change as
        // the branch advances, so the ref has to be named too — as a loose file
        // and again in `packed-refs`, since only one of the two will exist and
        // which one is git's business, not ours.
        println!("cargo:rerun-if-changed={dir}/HEAD");
        let common = git(&["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .unwrap_or_else(|| dir.clone());
        println!("cargo:rerun-if-changed={common}/packed-refs");
        if let Some(head_ref) = git(&["symbolic-ref", "--quiet", "HEAD"]) {
            println!("cargo:rerun-if-changed={common}/{head_ref}");
        }
    }

    let build = match git(&["rev-parse", "--short=7", "HEAD"]) {
        None => "commit unknown".to_string(),
        Some(commit) => match git(&["status", "--porcelain"]) {
            // ⚠ A build from a modified tree is not any commit, and saying so
            // is the difference between a version string and a claim. Measured
            // here 2026-09-08: an installed binary predated its own fix by
            // sixteen minutes while everything about it looked current, and
            // only a timestamp comparison nobody is prompted to make caught it.
            //
            // ⚠ This half stays best-effort even now. It reads the whole
            // workspace, and the inputs named above are this package plus the
            // refs — so an uncommitted edit in a *sibling* crate can leave the
            // marker off a binary that contains it. The commit is the part that
            // is now sound; treat a bare commit as "no edits **here**".
            Some(status) if !status.is_empty() => format!("{commit}+changes"),
            _ => commit,
        },
    };
    println!("cargo:rustc-env=FERROSTEP_BUILD={build}");
}

//! ferrostep-github — the agent roster, represented to GitHub.
//!
//! Agents in a FerroStep-governed repo commit under roster identities that
//! GitHub cannot verify: the author line is a convention, the push
//! authenticates as whichever human's credential is loaded, and the actors are
//! invisible to the org's audit surface. A GitHub App is the robust form of
//! the same idea — one registered identity that agents act through, with the
//! per-actor attribution kept by the roster.
//!
//! v0 does exactly one thing: emit the App manifest so the owner can register
//! the App in one step (GitHub's manifest flow). Token minting, the push
//! credential helper, and GitHub-side agents are phased in
//! `docs/github-agents-roadmap.md` — deliberately not here yet.
//!
//! Subcommands:
//!   manifest --name <app-name> [--org <org>] [--url <homepage>] [--html <path>]
//!       Print the GitHub App manifest JSON, plus the registration URL. With
//!       --html, also write a self-submitting form page that posts the
//!       manifest to GitHub — open it in a browser and click once.

use std::process::ExitCode;

use serde::Serialize;

/// A GitHub App manifest, per GitHub's app-creation-from-manifest flow.
/// Narrow on purpose: contents (write) to act on repos, metadata (read)
/// because GitHub requires it; no webhook, no events, not public.
#[derive(Debug, Serialize)]
struct Manifest {
    name: String,
    url: String,
    public: bool,
    hook_attributes: HookAttributes,
    default_permissions: Permissions,
    default_events: Vec<String>,
}

#[derive(Debug, Serialize)]
struct HookAttributes {
    active: bool,
}

#[derive(Debug, Serialize)]
struct Permissions {
    contents: String,
    metadata: String,
}

impl Manifest {
    fn new(name: String, url: String) -> Self {
        Manifest {
            name,
            url,
            public: false,
            hook_attributes: HookAttributes { active: false },
            default_permissions: Permissions {
                contents: "write".into(),
                metadata: "read".into(),
            },
            default_events: Vec::new(),
        }
    }
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
        Some("manifest") => manifest(args),
        Some(other) => Err(format!("unknown subcommand '{other}' (expected: manifest)")),
        None => Err("no subcommand given (expected: manifest)".into()),
    }
}

fn manifest(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut name: Option<String> = None;
    let mut org: Option<String> = None;
    let mut url: Option<String> = None;
    let mut html: Option<String> = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--name" => name = Some(value()?),
            "--org" => org = Some(value()?),
            "--url" => url = Some(value()?),
            "--html" => html = Some(value()?),
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    // The App's name is deployment configuration — the owner's to choose at
    // registration, never hardcoded here.
    let name = name.ok_or("--name is required: the App's name is the owner's choice")?;
    let url = url.unwrap_or_else(|| env!("CARGO_PKG_REPOSITORY").to_string());

    let manifest = Manifest::new(name, url);
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let register_url = registration_url(org.as_deref());

    println!("{json}");
    eprintln!();
    eprintln!("Register by POSTing this manifest to: {register_url}");
    if let Some(path) = html {
        std::fs::write(&path, form_page(&register_url, &json))
            .map_err(|e| format!("cannot write {path}: {e}"))?;
        eprintln!("Wrote {path} — open it in a browser and click once.");
    } else {
        eprintln!("(or re-run with --html <path> for a one-click form page)");
    }
    Ok(())
}

fn registration_url(org: Option<&str>) -> String {
    match org {
        Some(org) => format!("https://github.com/organizations/{org}/settings/apps/new"),
        None => "https://github.com/settings/apps/new".to_string(),
    }
}

/// A minimal page whose only job is to POST the manifest to GitHub's
/// registration endpoint from the owner's logged-in browser.
fn form_page(register_url: &str, manifest_json: &str) -> String {
    let escaped = html_escape(manifest_json);
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n<title>Register GitHub App</title>\n\
         <form action=\"{register_url}\" method=\"post\">\n\
         <input type=\"hidden\" name=\"manifest\" value=\"{escaped}\">\n\
         <button type=\"submit\">Create the GitHub App</button>\n</form>\n"
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_is_narrow_by_default() {
        let m = Manifest::new("example-app".into(), "https://example.invalid".into());
        let json = serde_json::to_value(&m).unwrap();
        assert_eq!(json["public"], false);
        assert_eq!(json["hook_attributes"]["active"], false);
        assert_eq!(json["default_permissions"]["contents"], "write");
        assert_eq!(json["default_permissions"]["metadata"], "read");
        assert_eq!(json["default_events"], serde_json::json!([]));
    }

    #[test]
    fn org_changes_registration_endpoint() {
        assert_eq!(
            registration_url(Some("Example-Org")),
            "https://github.com/organizations/Example-Org/settings/apps/new"
        );
        assert_eq!(registration_url(None), "https://github.com/settings/apps/new");
    }

    #[test]
    fn form_page_escapes_the_manifest() {
        let page = form_page("https://example.invalid/new", "{\"name\":\"a<b>&\\\"c\"}");
        assert!(!page.contains("<b>"), "unescaped markup leaked into the attribute");
        assert!(page.contains("&quot;"), "quotes must be escaped inside the value attribute");
    }
}

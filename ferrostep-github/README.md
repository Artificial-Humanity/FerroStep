# ferrostep-github

**The agent roster, represented to GitHub.**

Agents in a FerroStep-governed repo commit under roster identities
(`config.yaml`) that GitHub cannot see: author lines are a convention, pushes
authenticate as whichever human credential is loaded, and the actors never
appear on the org's audit surface. This sub-project makes the same
arrangement robust on GitHub's side — one registered **GitHub App** that
agents act through, while per-actor attribution stays with the roster.

## Today (v0)

One command, no credentials required:

```sh
cargo run -p ferrostep-github -- manifest --name <app-name> --org <org> --html register.html
```

Emits the App manifest (narrow by design: `contents: write`,
`metadata: read`, no webhook, not public) and a one-click page that posts it
to GitHub's registration endpoint. The App's name is the owner's choice at
registration — deployment configuration, never code.

⚠ Registration does not set the App's avatar. GitHub's manifest carries no
logo field, so the badge stays the generated default until someone uploads
one by hand under the App's Settings → Display information. Use
[`assets/icon.png`](../assets/icon.png): GitHub accepts PNG, JPG or GIF
under 1 MB, so `icon.svg` cannot serve here however much it would like to.

⚠ Install the App per-repo and deliberately: never on a repository where a
push to the default branch deploys anything.

## What comes next

Phased in [`docs/github-agents-roadmap.md`](../docs/github-agents-roadmap.md)
— token minting and push-as-App, attribution hardening, and eventually
GitHub-side agents. Deliberately absent until their phase arrives.

## Language and tooling

This sub-project is Rust, and the phases above stay Rust. **Prefer Rust over
Python unless a specific choice clearly favors Python.** Worth saying here
rather than leaving to the repo-wide default, because GitHub-App territory
pulls harder toward Python and JavaScript than anywhere else in this
codebase — Probot, Octokit and PyGithub are the well-worn paths, and a
well-worn path is not by itself a reason.

Where a choice *does* clearly favor Python, it runs under
[uv](https://docs.astral.sh/uv/) — `uv venv`, `uv pip install` — never bare
pip, venv or poetry.

[`AGENTS.md`](../AGENTS.md) holds both as repo-wide conventions. This section
says how they land on this sub-project; it does not override them.

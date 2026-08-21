# GitHub agents — roadmap

How `ferrostep-github` grows from an identity shim into GitHub-side agents.
Each phase lands only when something real consumes it — the same admission bar
as `examples/` and `skills/` — and each later phase depends on the ones before.

## P0 — one App as the actors' umbrella *(current)*

A single GitHub App, registered from the manifest `ferrostep-github manifest`
emits, installed per-repo with deliberately narrow permissions (`contents:
write`, `metadata: read`). The App is the GitHub-side counterpart of a repo's
agent roster: humans act as themselves, agents act through the App.

⚠ Installation is opt-in per repository, and never on a repository where a
push to the default branch deploys anything.

## P1 — push as the App

Installation-token minting (App JWT → installation token) and a git
credential helper, so agent pushes authenticate as the App instead of a
human's credential. Author lines stay per-actor from the roster; the
committer and pusher become the App. This is the phase that ends "the push
authenticates as the owner" — until it lands, the author line remains
attribution only.

Adds crypto/HTTP dependencies; the Apache-2.0 license audit gates them like
any other.

## P2 — attribution hardening

Make the actors first-class on GitHub's surface: commits created or verified
through the API (the `verified` badge), App-scoped noreply author emails, or
commit signing with App-held keys. The goal is that a roster identity on a
commit is checkable, not just written.

## P3 — GitHub-side agents

The App becomes a transport for actors that live on GitHub: webhook events
(an issue opened, a review requested) arrive as proposed transitions in a
FerroStep-refereed loop, and GitHub Issues/PRs become one more ledger surface
beside PocketBase/SQLite/Postgres. The engine referees exactly as it does
anywhere else; GitHub is the UI some actors and humans happen to live in.
This is where "agents under the product" stops meaning identity and starts
meaning work.

The expected first case: a repo's reviewing persona spun up at GitHub's end
as part of the PR-review process — a roster identity arriving as a PR
reviewer, filing findings through the App, with its passes and escalations
refereed like any other loop. Inevitable-feeling, and still a roadmap item:
nothing in P0–P2 pre-builds for it.

# Layered rosters, and where an actor's credential comes from

Working note. Owner brief, 2026-08-25, in two parts:

1. **An authorization *type*, not a hardcoded lookup.** Configuration names a
   `type` — `simple` to begin with — with a sub-key for a path. "Different
   setups may call for different placement or might even need these things to
   live in a service." The first type is the simple one; the point of naming
   it is that it is not the only one.
2. **Parent project → subprojects.** Credentials for our own deployment live
   in a directory under the workspace that holds cross-project working files,
   not inside any one project. "We will need to account for this sort of
   multi-level configuration."

Both are the adapter rule arriving somewhere new: a credential source is a
thing an adapter reaches, and shaping the interface around the file we happen
to write first is how it stops reaching a service later.

---

## The trap: titles are project-local, credentials are not

The obvious design collides on its first day. A roster is keyed by **title**,
and a title means something only inside its own repo — this workspace has two
rosters and both name a `developer`, who are different agents with different
identities. A shared credential file keyed by title therefore has one
`developer` entry and two claimants.

⚠ **It would not fail loudly.** The wrong password refuses; the *right
password for the wrong agent* authenticates, and the loop then attributes
work to whoever that credential belongs to. The roster exists to stop exactly
that, so reintroducing it underneath the roster would be a poor joke.

**Resolution: key credentials by the identity, not the title.** A roster
entry already carries a globally unique one — its `email`, which is what work
is signed under and what a store authenticates. So:

```json
// <workspace>/workflow/actors.json — the `simple` credential source
{ "accounts": {
    "ada@example.com":   { "password": "…" },
    "grace@example.com": { "password": "…" }
} }
```

The title→identity mapping stays where it already is, in each project's own
roster. Nothing new is keyed, and **the email that was attribution becomes
authentication** — same word, same entry, one of them now enforceable. That
convergence is the whole reason the roster and the actor binding are not two
systems.

⚠ Assumed, and cheap to correct if wrong: an agent's commit address and its
store login are the same string. If a deployment needs them different, that
is an `account:` override on the roster entry, and it should not be built
before a deployment needs it.

## The auth block

```yaml
auth:
  type: simple
  path: workflow/actors.json    # relative to THE FILE THIS APPEARS IN
```

⚠ **Relative to the file it is written in, never to the merged result or the
working directory.** A parent's `workflow/actors.json` means the *parent's*
`workflow/`, and a child inheriting that line must still resolve it against
the parent. Getting this wrong produces a path that works from one directory
and not another — which reads as an environment problem for as long as it
takes to stop believing that.

`type` is the discriminator. `simple` is a file of passwords, appropriate for
one operator on one host and honest about being that. What it opens the door
to is the reason it exists: a keyring, an environment source, a secrets
service, a directory that issues short-lived tokens. ⚠ **A type is added when
a deployment needs it**, and `simple` must not acquire options that only make
sense for it — that is how a discriminated interface quietly becomes one
implementation's shape.

## Layering rules

Discovery walks up from the working directory and collects **every**
`config.yaml`, nearest last. Today it stops at the first, which is what makes
a shared parent impossible.

| key | rule |
|---|---|
| `agents` | merged per title; the nearest file wins a title outright — entries are not field-merged, because a half-inherited identity is worse than either whole one |
| `default_agent` | nearest wins |
| `auth` | nearest wins **as a block** — never field-merged, so a `type` from one file can never meet a `path` meant for another |

Every value keeps the path of the file it came from, because relative paths
are resolved against it, and because "which file said this" is the first
question anybody asks of merged configuration.

⚠ **Walking to the filesystem root is inherited behaviour, not a decision.**
It is what discovery already does; layering makes it more consequential,
because now a stray `config.yaml` in a home directory contributes instead of
being shadowed. Worth a stop marker if it ever bites — not worth inventing
one first.

## Constraints from the consuming loop

Reported 2026-08-25 by the resident of the loop that would use this, checked
by them rather than recalled. Each one closes off a design that would
otherwise have looked fine here.

**The credential file is JSON, and that is not a preference.** The scripts
that authenticate are plain bash and a **system-python, stdlib-only** script
— no venv, no third-party imports. YAML was the obvious choice next to a
YAML roster and it is unreadable to the consumer: Python's standard library
has no YAML parser. ⚠ **The roster and the credential source do not have to
share a format, and here they must not.** The roster is read by this project's
own Rust; the credential source is read by whatever a deployment's actors
happen to be written in, which is the wider audience and the stricter bar.

⚠⚠ **Selection is per-invocation and by title; the credential must never
arrive as an inherited environment variable.** In the reporting loop the
reviewer is a *nested subprocess of the developer* — one process spawns the
other with a different persona. An exported credential is inherited across
that spawn, so the reviewer would authenticate as the developer **and
everything would work**: the move succeeds, the event is written, the role is
plausible. It is the failure most likely to survive testing, because the test
that proves "an actor can move a record" passes with one principal doing both
jobs. So each call site names the title it is acting as, and resolution
happens there.

**Never prompt, and fail fast.** One call site is a paid unattended run and
another is a merge gate. A credential path that waits on a TTY, an unlock or a
confirmation does not fail in either — it hangs, which costs money in the
first and blocks landing in the second. A non-zero exit with a message beats
blocking in every case.

**One resolution path, or a transition half-applies.** A script mints its own
token for its reads and writes and *shells out to the binary* for the
transitions. If those two resolve different actors after enforcement is
strict, a move succeeds and the write beside it is refused — a live record
left between two states, which is worse than either failing alone.

⚠ **State honestly what this does not buy.** Both actors run as the same
operating-system user, so file permissions cannot stop one reading the
other's credential. Separation here is **convention, not enforcement**. What
it does buy is real and worth having — correct attribution, and a refusal
when a request claims a role its account does not hold — but describing it as
secrecy between roles would be a lie the design cannot support.

## What this does not do

Not a secrets manager, and `simple` should never grow toward one. The
credential file is a file; protecting it is the deployment's business, and the
type discriminator is the seam where a deployment that needs more reaches for
something that is.

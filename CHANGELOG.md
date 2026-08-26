# Changelog

Notable changes, per release. What a version *means* is defined in
[docs/ROADMAP.md](docs/ROADMAP.md) §Releases — outcomes, not dates. The
Decision JSON rule in [AGENTS.md](AGENTS.md) §Conventions is one reason an
entry here is mandatory rather than courtesy.

## Unreleased

- `ferrostep-cli`: **an unknown flag is refused, not ignored.** `Flags::parse`
  accepted any `--name value` and the code read only what it asked for, so
  anything else was silently dropped. Two ways that bit, both measured
  2026-08-26: a **typo** — `--scpoe branch=main` — quietly widened a scoped
  audit to every record and exited 0; and a **binary older than a flag**
  accepted `--role`, ignored it, and reported *"0 of 12 await a person"*, which
  is correct for the question it actually asked and completely wrong for the
  one asked of it. The second was found by an adopter whose installed binary
  predated the flag by two days.
  ⚠ This is AGENTS.md's generated-files convention arriving at a surface that
  had never been held to it: **an older thing meeting a newer request refuses
  the part it does not understand, rather than accepting and ignoring it.** The
  PocketBase ping's `writes` exists for exactly this reason; a CLI's flags
  outlive the binary that parses them the same way a hook outlives its adapter.
  The refusal names the flag, lists what the subcommand accepts, and says the
  build may predate it — so it doubles as the version diagnostic and no
  separate capability probe is needed.

- `ferrostep-core` / `ferrostep-cli`: **an agent's queue is visible and
  notifiable — it was neither.** `awaiting` and `notify` both selected on
  "does this need a person", so a record handed from a reviewer back to a
  developer appeared in no listing and rang no doorbell. It is `Status::Live`,
  which is true and useless: `Live` says *some* automated role can act and
  never which, so a loop with two agent roles has two queues and the
  enumeration could see neither. ⚠ **In a worker/reviewer loop that handover
  is the ordinary case, not an edge** — the actor whose turn it now was had no
  way to find out except by querying the database directly, which is the thing
  these surfaces exist to replace.
  `Engine::awaits(snapshot, role)` answers *whose turn is it*, asked of one
  role, and `--role` on both subcommands scopes them to it. Without the flag
  they behave exactly as before, so the person-scoped question B2 was built
  for is unchanged. ⚠ An **exhausted** move is not a turn: a role whose every
  option would route the record away is not waiting, and reporting otherwise
  sends an actor to do work the referee is about to refuse.

- `ferrostep-pocketbase`: **`CollectionMap::guard_refereed_fields` — the
  refereed columns can be closed to direct writes.** The engine is consulted,
  not in the write path, so a client holding credentials could edit `state` or
  a counter straight on the row: no version bump, no event, and every later
  compare-and-swap arguing about a number that moved behind it. With the guard
  on, those columns change through the apply route or they do not change.
  ⚠ **It is a hook rather than an access rule because an administrator
  bypasses rules and does not bypass hooks** — measured on this backend, and
  the whole reason the placement matters. The route's own writes are internal
  saves that never reach a request hook, so the referee is unaffected; only a
  direct edit is refused. Registered *ahead of* the release hook, since
  handlers chain and a guard running second would refuse the release it exists
  to permit.
  ⚠ **Off by default**, like `ActorBinding::allow_unbound`, because on is a
  behaviour change for a running deployment — and it is not free: a console
  hand-edit of a counter stops working too, leaving the release hook and the
  routes as the operator's supported path. The guarded columns are derived
  from the map, so a counter or scope label added later is covered because it
  is declared, not because somebody remembered.
- `ferrostep-roster`: **layered rosters and a credential *source*.** Discovery
  collected the first `config.yaml` above the working directory and stopped;
  it now collects every one and layers them, nearest last. That is what lets a
  workspace of several repos share values from a file above them while each
  repo overrides what it needs. ⚠ **`agents` merges per title and takes an
  entry whole** — never field-merged, because half an identity assembled from
  two files is worse than either of them complete — and **`auth` is replaced
  as a block**, so a `type` from one file can never meet a `path` meant for
  another.
  ⚠⚠ **Every value resolves its relative paths against the file that WROTE
  it**, not against the nearest file and not against the working directory. An
  entry inherited from a workspace roster names a persona beside *that* file.
  Getting this wrong is close to invisible: in a layered tree the wrong join
  frequently lands on a real file, so the actor loads somebody else's persona
  and nothing fails.
  `auth` names a **type** with a path — `simple` to begin with, a file of
  credentials keyed by the identity in the roster's `email`. An unrecognised
  type is a refusal naming the file rather than an ignored block, because a
  deployment that believes it configured something and got nothing is the
  failure worth preventing.
  ⚠⚠ **This crate never reads the secret.** `agent-env` emits where credentials
  live and which identity to look up; the lookup is the caller's. A password
  put in the environment is inherited by every subprocess — including one
  launched to act as a *different* actor, which authenticates as whoever
  spawned it while everything appears to work. `--format json` is the
  inheritance-proof path: a caller reads it from a pipe and exports nothing.
  Absent rather than empty when unconfigured, so a consumer under `set -u`
  fails loudly instead of authenticating as nobody.
- `ferrostep-pocketbase`: **role-scoped actors — the write routes stop
  believing the request about who is asking.** Every route authenticated and
  then wrote `role` straight from the request body, so any authenticated
  caller could act as any role. That is invisible while every actor shares one
  credential and is the entire point once they do not. The acting role is now
  read from the **authenticated principal**, and a request claiming a
  different one is refused by name.
  ⚠ **Bind, don't mint** (`docs/prior-art.md` §requirement 9). `ActorBinding`
  names an auth collection the deployment already has and one field on it; it
  creates no identities and is not an account store. The store authenticates
  whoever it authenticates — a password, an OAuth provider, a directory
  federated behind it — and the only thing read here is which role that
  principal may act in. Owning accounts would mean enumerating the actors when
  the loop is designed, and the actors are exactly what a deployment cannot
  enumerate up front: an agent nobody foresaw should be a new principal in a
  directory that already exists plus one row naming its role.
  **Defaults work on a stock instance** — a `ferrostep_actors` auth collection
  with a `role` field, created by the migration only when absent, superuser-only
  to read. `allow_unbound` is `true` by default and that is a *transition, not
  a position*: a deployment with no actors yet authenticates as an
  administrator, so refusing unbound principals on install would be an outage.
  Set it `false` once your actors exist — from then on a principal with no role
  cannot move a record even holding administrator credentials, which is why the
  check lives in a hook rather than an access rule.
- `ferrostep-pocketbase`: **a generated history no longer outranks the records
  it describes.** The mapped migration created its events collection with an
  authenticated-user read rule — matching in the generic shape, which creates
  *both* collections and where the two therefore agree by construction, and
  wrong in the mapped one, where the refereed records are a collection the
  adopter already had under rules commonly stricter. That shipped an inversion
  by default and in silence: every state change, actor, role and human note
  about records the reader may not open, in a collection any authenticated
  account may list. The mapped shape now creates it **superuser-only**, which
  is the only end of the range that is right whatever the adopter's rules say;
  widening is their deliberate act in the admin UI, and the create-if-absent
  guard means a later regeneration will not undo it. `events_collection_body`
  is strict for the same reason and a stronger one — it is handed a name and
  nothing else, so it can know even less about what it sits beside.
  ⚠ The general shape, and it is not about access rules: **generated output
  that attaches to something the adopter owns cannot carry a constant.** The
  invariant here is relational — *no more visible than its subject* — and one
  value was shipped for two situations, then tested in the one where it holds.
- `ferrostep-cli`: `file` (also spelled `create`) — the way into a ledger.
  `authorize_create` had been in the engine, the Python binding and both
  adapters since 0.1.0, and was reachable from the person-facing surface
  nowhere: a store with a console of its own can be handed a record without
  the referee ever being asked, and **SQLite has no console to hide behind**,
  so the deployment shape the roadmap calls first-class could not get a first
  record in short of writing a program. ⚠ A filing ceiling is **measured
  against a count this binary cannot take** — it bounds a branch or a cycle
  rather than the record being filed — so it is passed with `--counter` and a
  missing one is a refusal naming the remedy. Defaulting it to zero would mean
  every filing ceiling silently never fires, which is a guard reporting
  success having checked nothing.
- `ferrostep-cli`: `explain` now says who may file and what filing costs, and
  says **"nobody"** out loud where a definition grants it to no one. Filing is
  default-deny like a rescope, and a heading that is not there leaves a reader
  to work that out for themselves.
- `examples/product-review.json` grows a `creation` block, so the permission
  has an illustration and the guard that keeps `examples/` honest validates
  one. Its filing ceiling is the shape the field exists for: every record
  individually bounded, the population bounded separately.
- `ferrostep-py`: `authorize_rescope`. The core, both ledger adapters and the
  CLI grew rescope; the binding did not, and nothing said so — which left the
  Decision JSON contract a strict superset of what a first-class binding could
  produce, and left a Python consumer wanting to move a record between units
  of work with the raw store write as its only option. That is precisely the
  hole rescope exists to close, still open in one language. No counters are
  asked for, because a rescope spends nothing.
- `examples/product-review.json` grows a `rescopes` rule: a review belongs to
  a release line, and only the owner may move it to another, with a reason.
  The newest concept in the engine had no illustration, and the guard that
  keeps `examples/` honest had never validated a definition carrying one.
  `review-loop.json` deliberately keeps none, so the pair also shows what
  absent means — nobody may, rather than anybody.
- `ferrostep-cli`: `explain` no longer panics on a maximal ceiling. It prints
  each asserted number *and its off-by-one neighbour*, and computed that
  neighbour with `max + 1` — which overflows at the top of the range: a crash
  in a debug build, and in release a wrap to `0` that would have sent the
  reader hunting their tree for the wrong number entirely. A ceiling is a
  value out of a file somebody else wrote, and this is the subcommand whose
  audience is a person who has not got the system working yet.
- `ferrostep-cli`: the audit no longer reads a state change out of a rescope.
  A rescope moves a record between units of work and leaves it in the state it
  was already in, so its event carries `to` equal to `from_state` — which
  satisfied both the escalation test (arrived somewhere halted) and the release
  test (departed somewhere halted) at once. Rescoping a paused record therefore
  reported an escalation *and* a release for a record that had not moved. Not a
  crash and not a visibly wrong number: a **plausible story**, on the one
  surface offered to a person who is deliberately not opening a database
  console to check. Both tallies now ask whether the record moved at all.
- `ferrostep-roster`: the actor roster as a product surface. A deployment's
  `config.yaml` names its agents by title; each entry carries the identity work
  is signed under and the persona document that tells that agent how to behave.
  Titles are configured values and the crate knows nothing by any of them. The
  persona resolves against the roster's own directory and is checked to exist
  before it is emitted, because that path is what a launcher hands to
  `--system-prompt-file`.
- `ferrostep-cli`: `agent-env` — the roster as shell assignments, taking no
  workflow and no store. A repo adopts a roster before it adopts a referee, and
  a repo with no Rust toolchain could not reach the reader at all while it was
  an `xtask` subcommand. Every failure is a refusal with a message rather than
  an empty assignment at status zero: a caller `eval`s this and then commits
  with it, so falling back is how work gets signed under the wrong name.
  `--format json` answers the same resolution for a caller that is not a
  shell, so recovering a name does not require decoding shell quoting in
  another language.
- `ferrostep-cli`: `explain` — what a definition permits, readable without a
  store. Its numbers section exists because of a migrating loop, not taste: when
  a ceiling moves into a definition, FerroStep owns the *value* and knows
  nothing about the *arithmetic derived from it* elsewhere in the adopter's tree
  — `max + 1` in a guard, a range in help text, a sentence in a brief handed to
  an actor. That arithmetic does not contain the value it came from, so a search
  for the ceiling finds none of it. Three times in one migration the search term
  that worked was a number the definition never states, so `explain` prints the
  asserted values *and* their off-by-one neighbours.
- `xtask agent-env` now delegates to `ferrostep-roster` rather than carrying a
  second reader of the same format.
- **Rescope: moving a record between units of work is now a refereed
  operation.** A record's scope decides which queries find it, so a record
  whose scope names a finished unit is invisible to all of them — and until
  now nothing could move one, so consuming loops did it as un-versioned,
  un-evented writes to the field every query depends on. A definition grants
  it per label and per role (`rescopes`), or nobody has it; `ferrostep
  rescope` performs it; it lands versioned and evented like any other move and
  shows up in `audit`. ⚠ Refused on terminal records, and that is not
  configurable: a finished record's scope is the provenance of what it was
  resolved against.
- `Decision::Allow` grows `scope_updates`, omitted from the JSON when empty —
  so a consumer written before rescope existed reads byte-identical JSON for
  everything it already handled, and no fourth `kind` was added for every
  binding to learn.
- `CounterDef` grows `exhausted_requires_note`: the attempt that finds a ceiling
  spent can be required to say what decision is being asked for. Exhaustion
  routes a record to a person, and an automatic route arrives in front of them
  with **no question attached** — which is the whole content of the handover.
  The actor that just ran out of attempts is the one who knows what cannot be
  settled, and that is the moment it knows it. Deliberately not `requires_note`
  on the spending transition, which would tax every attempt when only the last
  one is addressed to anybody; the decision surface still shows where a spent
  ceiling would route, because it offers moves with a note already attached.
- `ferrostep-pocketbase`: the generated ping now states what the installed
  routes can write, and the adapter reads it. Hooks are deployed separately
  from the binary, so a current adapter meets older routes routinely — and
  those answer an apply carrying scope updates with a cheerful 200 while
  writing no label. That is now refused by name, with the remedy in the
  message, instead of being reported as a move that happened. In mapped
  deployments the writable labels are the map's `scope_fields` and nothing
  else, as one generated line per declared label rather than a loop over
  whatever a request names.

## 0.1.0 — 2026-08-24

The internal MVP ([ROADMAP §Releases](docs/ROADMAP.md)): cut on the owner's
judgment after the lane's store was provisioned live and a real record ran
the full refereed cycle — a pass claimed and spent, a genuine design
escalation, the owner's release through the generated hook, and a close —
all of it in the ledger's own history.

- `ferrostep-ledger`: `Scope::matches` and `decided_snapshot` — the one shared
  meaning of "apply this decision to this snapshot".
- `ferrostep-sqlite`: the first ledger adapter. WAL-mode SQLite on one host;
  atomic apply and compare-and-swap by construction, append-only history
  enforced by triggers, all three capability flags earned by tests including
  a repeated-rounds concurrency battery.
- `ferrostep-pocketbase`: the second ledger adapter — a stock instance plus a
  generated migration and transactional apply/create routes (the compare
  inside the store's transaction, the only placement that measured sound).
  Detects at connect time whether the routes are installed and says which
  mode it is in; without them it is read-only and refuses writes by name.
  Live end-to-end loop and concurrency battery ship as ignored-by-default
  tests, run against a real instance.
- `ferrostep-pocketbase`, again: **mapped deployments** — a `CollectionMap`
  referees an existing collection's own columns (state, counters, version
  token, scope labels), so a loop already living in a collection keeps one
  truth and its console view; filing stays with the collection's own
  procedure and is refused by name. Generated routes became
  collection-scoped so refereed collections cannot collide. An optional
  generated release hook makes writing a decision field perform the
  definition's release transition with the referee's bookkeeping (version
  bump, event append) — the store-side transition B5 warns about, as
  generated output instead of a hand-written peer.
- `ferrostep-notify`: the notification message — which record, why, how
  urgently, how to get back — and the `Notifier` adapter boundary, with ntfy
  as the maintained default. Nothing polls or schedules; callers decide when.
- `ferrostep-cli`: the `ferrostep` binary. `awaiting` renders which records
  await a person and what their moves would actually do; `move` resolves one
  without a database console; `audit` reports what happened (moves,
  escalations, releases, last note) from the same enumeration `awaiting`
  reads; `notify` sends one notification per awaiting record.

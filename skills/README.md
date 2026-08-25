# skills/

Skills that **deploy with FerroStep** — loadable instruction packages (open
`SKILL.md` format) for actors *inside* the loops the engine referees: how to
read the ledger, claim work before doing it, request transitions, escalate,
and stay within a role's authority.

FerroStep is a referee, not a runtime, so it cannot install behavior into the
agents that act in its loops — a skill an actor loads is how that behavior
ships. These are product artifacts, like `examples/`.

**Empty now, deliberately** — a judgment about skills, not an instance of a
general rule (the repo has no standing admission bar; see
[ROADMAP](../docs/ROADMAP.md) §Non-goals). The reason is specific to what a
skill is: it tells an actor how to work a particular loop, so writing one
before a real loop exists means inventing the loop it instructs. This is not
the place for skills that help build FerroStep itself — see
[AGENTS.md](../AGENTS.md) §Layout for that distinction.

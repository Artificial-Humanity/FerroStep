# FerroStep

**[AGENTS.md](AGENTS.md) is this repo's rules of record. Read it before you do anything else.**
It is not loaded for you — only this file is — so nothing else will put it in front of you.

## Your role is assigned in config.yaml, not assumed

**If nothing in your system prompt says otherwise, you are the repo's default agent.**
`default_agent` in [`config.yaml`](config.yaml) names your entry, and the entry carries your
title, your name, your commit identity, and the path to your persona. That persona is
imported on the next line, so it is already in your context by the time you read this
sentence — no flag, no pre-prompt, and no decision on your part.

@workflow/DEVELOPER.md

⚠ The `@import` above is a literal path — imports cannot read `config.yaml` — so it is the
one deliberate second copy of the default agent's `persona` value. Change the two together.

**`config.yaml` lists every agent that works this repo, and today it lists one.** Sonora's
full review cycle is deliberately not adopted here (owner, 2026-08-20) — no second persona
reviews the first, and no lane automates it. This project is generalizing the engine such
loops run on, and will dog-food its own once it can; until then, work is owner-directed.

⚠ **If you are committing, the author line is not automatic.** The repo's configured
identity is the owner's, deliberately, so a forgotten override does not error — it silently
commits your work under their name. The procedure and the after-commit check are in the
persona's §1.

**Keep this file short.** It exists to route and to import; the rules live in `AGENTS.md`,
the procedures in the persona, and the configurable values in `config.yaml`. Anything
restated here becomes a second copy to drift. ⚠ **An `@import` is not a restatement** — it
is one copy, loaded from its own file.

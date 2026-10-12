# AGENTS.md

Rules for coding seats working on **cmux-beads**.

## Fleet Beads (HARD standing)

- Fleet Beads = **central Dolt SQL** over Tailscale at `beads.jaguar-fish.ts.net:3306` (k8s ns `beads`, STS `beads`).
- **Never** start or rely on `bd serve` HTTP for Beads storage, sync, or status pills.
- Per-repo `.beads` uses `dolt_mode: server`. Client env:
  - `BEADS_DOLT_SERVER_HOST=beads.jaguar-fish.ts.net`
  - `BEADS_DOLT_SERVER_PORT=3306`
  - `BEADS_DOLT_SERVER_USER=root`
  - Password/creds from the secret store only — **never** commit plaintext secrets.
- This repo owns **cmux status-pills only**. `cmux-beads watch` uses local `bd` CLI / events journal. Storage/sync against central Dolt is a `bd` / `.beads` concern, orthogonal to the pill projector.

## Worktrees

Use git worktrees only under `<project>/.worktrees/<name>` (e.g. `.worktrees/docs-fleet-beads`).

## Scope

- Prefer docs-only or minimal diffs unless the task says otherwise.
- Do **not** use Cursor Cloud Agents or paid on-demand for this repo.

## Naming (HARD)

Name every new identifier with this grammar. Do not invent a second style.

Semantic skill: https://github.com/thonymg/skills/tree/main/skills/naming-convention
Go only, extra: https://github.com/cxuu/golang-skills/blob/main/skills/go-naming/SKILL.md

- A function is a verb: `fetchUser`, not `userData`.
- A boolean starts with `is`, `has`, or `can`.
- A file is named after the thing inside it. One type per file.
- One word for one concept. If the repo says `user`, do not also say `account` or `member` for the same thing.
- Forbidden: `data`, `info`, `temp`, `helper`, `manager`, `utils`, `misc`, `common`, `stuff`.
- Do not repeat the owner in the name: `user.name`, not `user.userName`.
- Abbreviate only words already used in this repo (`id`, `url`, `ssh`). Otherwise spell them out.
- A new term is written down once, then used. Do not invent a synonym in passing.
- An existing file in the same directory wins when that directory is already consistent.
- Casing follows the language in the file you are editing:
  - Rust: `snake_case` functions, `PascalCase` types, `SCREAMING_SNAKE_CASE` consts. Clippy wins.
  - Go: `MixedCaps` / `mixedCaps`. Constants are `MaxSize`, never `MAX_SIZE`. Packages are lowercase, one word.
  - Swift: types `PascalCase`, methods and variables `camelCase`, file name matches the type.
  - TypeScript: `camelCase` variables and functions, `PascalCase` types and components, `kebab-case` module files.
  - Shell: `lower_snake_case` variables and functions, `UPPER_SNAKE_CASE` constants.

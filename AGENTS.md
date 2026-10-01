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

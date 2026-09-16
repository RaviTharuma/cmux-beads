# Security

## Scope

`cmux-beads` bridges three local trust domains:

1. **Beads (`bd`)** — the issue database for a repo (or `bd --global`).
2. **cmux** — workspace status pills, identify, and (for the TUI) live panes via `CMUX_TUI_SOCKET`.
3. **Interpreted sidebars** — restricted JS/Swift scenes that only read live cmux context and call `cmux()`.

There is no network server in this repository. The CLI and scenes run on the user’s machine with the same privileges as the invoking shell / cmux host.

## Design controls

### Argv-only `bd` bridge

All `bd` invocations are built as argv vectors (`src/bd/mod.rs`). Titles, close reasons, notes, comments, and assignees are single elements — never concatenated into a shell string. This blocks shell injection through issue text.

### Sidebar sandbox expectations

`sidebars/beads.js` and `sidebars/beads.swift` are written for cmux’s restricted custom-sidebar interpreters:

- No process spawn, `require`, `fetch`, or filesystem access.
- Bind only live workspaces / tabs / agents / `statuses`.
- Never invent teams, hardcoded demo titles, emails, or home paths in the scene.
- Status persistence into Beads is **out of band** (`cmux-beads update` or the keyboard TUI).

### Projection hygiene

- Status keys are only `bead:<id>` with a strict id charset (`src/project.rs`).
- At most 24 pills; value length capped (48 display chars).
- Focus affinity uses a compact `◈{workspace}/{pane}` tag — not emails, filesystem paths, or a leading `cmux:` string on the pill.
- `clear` / sync stale cleanup only removes `bead:*` keys, not unrelated cmux statuses.
- Workspace selection refuses to guess: `--workspace`, `CMUX_WORKSPACE_ID`, or `cmux identify --json`.

### TUI / PTY

When hosted as a sidebar plugin (`CMUX_SIDEBAR=1`), the binary runs the keyboard TUI only. Mouse capture is not enabled; drag-and-drop is not invented for the PTY path.

## Threat notes

| Risk | Mitigation / residual |
| --- | --- |
| Malicious issue title / note | Passed as argv to `bd`; still displayed in UI — treat Beads content as untrusted display text |
| Wrong workspace pills | Explicit workspace resolution; dry-run flags on sync helpers |
| Sidebar overreach | Restricted interpreter + no `bd` from the scene; host must keep that contract |
| Supply chain | Depend on published crates; CI builds `--locked`; review `Cargo.lock` changes |
| Plugin install from git | Installing via `cmux sidebar plugin install <url>` runs this repo’s build — pin to a known commit/tag when possible |

## Reporting

Report security issues privately to the repository maintainers via GitHub Security Advisories on [RaviTharuma/cmux-beads](https://github.com/RaviTharuma/cmux-beads) when available, or by contacting the owner listed on the GitHub repository. Please do not open a public issue for exploitable bugs until a fix is available.

## Supported versions

Security fixes target the latest release on `main` (see [CHANGELOG.md](CHANGELOG.md) and GitHub Releases). Older tags are not maintained as separate lines.

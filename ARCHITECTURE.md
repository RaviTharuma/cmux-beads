# Architecture

`cmux-beads` is the official [Beads](https://github.com/steveyegge/beads) experience for [cmux](https://github.com/manaflow-ai/cmux): a **right-sidebar tab** (sibling of Files / Find / Dock), not a Bonsplit pane and not the left workspace list.

The interpreted sidebar cannot spawn `bd`. Live issue cards come from `cmux-beads watch` / `sync`, which project `bd` issues into workspace status pills (`bead:<id>`). The tab only reads live cmux context and runs `cmux()` actions.

## Layers

```
┌─────────────────────────────────────────────────────────────┐
│  cmux host                                                  │
│  RightSidebarMode.beads  (built-in)                         │
│  or plugin manager PTY fallback / Custom-slot scenes        │
└────────────────────────────▲────────────────────────────────┘
                             │ reads w.statuses / workspaces
                             │ cmux("workspace.select" | …)
┌────────────────────────────┴────────────────────────────────┐
│  Projection CLI  (this repo: Rust binary cmux-beads)        │
│  watch / sync / status / clear / update                     │
│  bd list --json  →  cmux set-status bead:<id>               │
└────────────────────────────▲────────────────────────────────┘
                             │ argv only (no shell strings)
┌────────────────────────────┴────────────────────────────────┐
│  Beads store  (bd CLI v0.60+, repo .beads or --global)      │
└─────────────────────────────────────────────────────────────┘
```

| Layer | Role | Code |
| --- | --- | --- |
| Host Beads tab | Product GUI: Board / List, Host / Focus / Assigned, native chrome | Shipped in cmux (`RightSidebarMode.beads`); see manaflow-ai/cmux#11707 / #11709 |
| Interpreted scenes | Contrib/legacy reference UI for the generic Custom slot | `sidebars/beads.js`, `sidebars/beads.swift` |
| Projection CLI | `bd` → `bead:<id>` pills; status writes back via argv | `src/sync.rs`, `src/project.rs`, `src/bd/` |
| Keyboard TUI | PTY fallback (plugin `kind = "sidebar"`); no mouse | `src/app.rs`, `src/ui.rs`, `src/board.rs`, `src/keys.rs` |

## Product vs not-the-product

**Product**

1. `cmux right-sidebar set beads` — built-in host tab.
2. `cmux sidebar plugin install …` / `use cmux-beads` — plugin package (PTY TUI is keyboard-only fallback when hosted with `CMUX_SIDEBAR=1`).
3. `cmux-beads watch` — required projection for live pills on either path.

**Not the product** (commands exist in cmux but are the wrong surface)

- `cmux sidebar open beads` — Bonsplit pane
- `cmux sidebar select beads` — left workspace list
- `cmux right-sidebar set custom beads` — generic Custom slot (where the in-tree JS/Swift scenes apply)

## Data flow: projection

1. Resolve workspace: `--workspace` → `CMUX_WORKSPACE_ID` → `cmux identify --json`. Never guess a host.
2. Load issues: `bd list --json` in the resolved cwd (`--cwd`, focused pane cwd via `CMUX_TUI_SOCKET`, or process cwd). Optional closed merge via `bd list --status closed --json`.
3. Build up to `MAX_PILLS` (24) pills (`src/project.rs`):
   - key: `bead:<id>` (id must be safe: alphanumeric start, then `A-Za-z0-9._-`, ≤64)
   - value: `{status} · {title}` plus optional ` · ◈{workspace}/{pane}` when assignee is `cmux:{ws}/{pane}`
   - icon / color / priority from status style
4. Diff against `cmux list-status --workspace …`; apply `cmux set-status` / `cmux clear-status` for stale `bead:*` keys only.
5. Best-effort `cmux set-progress` from in-progress / non-closed counts.

Writes into Beads (`bd update`, `claim`, create, close, note, comment) stay on CLI argv or the keyboard TUI — never from the interpreted sidebar scene.

## Sidebar UI (Board / List)

Interpreted scenes and the intended host Beads tab share the same UX contract:

| Control | Behavior |
| --- | --- |
| Board | Status columns (`open`, `in_progress`, `blocked`, `deferred`, `pinned`, `hooked`, `closed`); empty columns hidden |
| List | Same rows, denser single scroll grouped by status |
| Host / Focus / Assigned | View filters over projected pills — not a second store |
| Host strip | Live workspaces; `workspace.select`, `workspace.reorder`, `workspace.action` (pin / mark_read / move) |
| Surfaces | Tabs on the selected host; `surface.focus` |
| Focus strip | Selected host + focused surface / agent when cmux exposes them |
| Cards | Flat rows, 3pt tinted rail from pill `color` (fallback `accent`); tap selects workspace |

Chrome matches built-in right-sidebar panels: `surface: "glass"`, Ghostty/cmux tokens (`accent` / `primary` / `secondary` / `tertiary`), washes `#7f7f7f14` … `#7f7f7f3d`. No brand palette, card fills, or shadows.

Focus modes (sidebar + TUI `f`):

- **Host** — all projected beads for the selected workspace.
- **Focus** — beads tagged for the active pane (`◈ws/pane` on the pill, or TUI assignee ↔ live focused pane).
- **Assigned** — any bead with a pane focus tag / `cmux:` assignee.

Status drag-to-change is **not** implemented: there is no host write hook that lets the scene call `bd`. Use `cmux-beads update <id> --status …` or the TUI (`v` move mode / `s`).

## Keyboard TUI (fallback)

When the plugin manager hosts the binary with `CMUX_SIDEBAR=1`, or when invoked with no subcommand, the process runs a ratatui board (`List` / `Table` / `Kanban`) that talks to `bd` over argv and to live panes via `CMUX_TUI_SOCKET`. Mouse is not captured; PTYs do not receive drag-and-drop.

## Trust boundaries

| Boundary | Rule |
| --- | --- |
| Sidebar JS/Swift | No `spawn`, `require`, `fetch`, filesystem, or invented teams/titles |
| `bd` bridge | Every call is an argv vector (`src/bd/mod.rs`); user text is never shell-joined |
| Projection | Only `bead:*` keys; pills never embed emails, home paths, or raw `cmux:` strings |
| Workspace | Explicit or identify — refuse to pick a random host |

## Repo map

```
cmux-plugin.toml     plugin manifest (kind = sidebar, run release binary)
src/main.rs          dispatch: TUI vs sync/watch/update/install
src/cli.rs           argv parsing + help (product paths first)
src/sync.rs          cmux host adapter + watch loop
src/project.rs       pill plan, focus tags, status argv
src/bd/              bd argv bridge + Bead types (v0.60+ JSON)
src/board.rs         TUI views + FocusScope
src/app.rs / ui.rs   keyboard TUI state and draw
src/install.rs       legacy copy of sidebars/* → ~/.config/cmux/sidebars/
sidebars/            contrib/legacy interpreted scenes
scripts/install.sh   contributor CLI symlink helper (not end-user install)
```

## Related docs

- [README.md](README.md) — install and usage
- [STACK.md](STACK.md) — languages and dependencies
- [SECURITY.md](SECURITY.md) — threat model and reporting
- [DISCLAIMER.md](DISCLAIMER.md) — product boundaries
- [CHANGELOG.md](CHANGELOG.md) — version history

# cmux-beads

[![Release](https://img.shields.io/github/v/release/RaviTharuma/cmux-beads)](https://github.com/RaviTharuma/cmux-beads/releases/latest)

A **cmux right-sidebar tab**: the official [Beads](https://github.com/steveyegge/beads) (`bd` CLI) GUI inside [cmux](https://github.com/manaflow-ai/cmux). Beads is a **tab on the existing right sidebar**, a sibling of Files / Find / Dock — not a Bonsplit pane, not a replacement for the left workspace list, not an iframe or WKWebView.

The sidebar interpreter cannot spawn `bd`. `cmux-beads watch` projects issues into live `bead:<id>` status pills; the tab renders them.

Requires **`bd` v0.60+**.

## Install

### 1. Host tab (built-in)

Native `RightSidebarMode.beads`, shipped with cmux.app:

```sh
cmux right-sidebar set beads
```

Tracking: [manaflow-ai/cmux#11707](https://github.com/manaflow-ai/cmux/issues/11707) (issue), [#11709](https://github.com/manaflow-ai/cmux/pull/11709) (PR).

### 2. Plugin package (official plugin manager)

```sh
cmux sidebar plugin install https://github.com/RaviTharuma/cmux-beads.git
cmux sidebar plugin use cmux-beads
```

### 3. Projection CLI (both paths)

Keep this running in the repo (or pass `--cwd` / `--workspace`):

```sh
cmux-beads watch
```

Each tick writes `cmux set-status bead:<id>` and clears stale `bead:*` keys.

The PTY TUI hosted by the plugin manager is a keyboard-only fallback (no mouse). It is not the GUI product.

## Not the product

These commands exist in cmux but are **not** how Beads ships:

| Command | Why not |
| --- | --- |
| `cmux sidebar open beads` | Opens a Bonsplit **pane**, not a sidebar tab |
| `cmux sidebar select beads` | Replaces the **left workspace list** |
| `cmux right-sidebar set custom beads` | Generic **Custom** slot, not a Beads sibling tab |

`sidebars/beads.js` and `sidebars/beads.swift` remain in-tree as **contrib/legacy** interpreted-sidebar scenes for the generic Custom slot. They are kept for reference and are not the product. `cmux-beads install` copies them for that legacy path only.

## What you see

- **Beads** — the board is the product. Projected `bead:<id>` cards with a 3pt tinted rail, grouped by status.
- **Host** — live cmux workspaces under the board. Drag to reorder (`workspace.reorder`). Click to select (`cmux("workspace.select")`). Right-click for Pin / Move.
- **Surfaces** — tabs on the selected host run `surface.focus`.
- Built-in chrome: glass surface, Ghostty/cmux tokens (`accent` / `primary` / `secondary` / `tertiary`) plus host washes (`#7f7f7f14` / `#7f7f7f24` / `#7f7f7f28` / `#7f7f7f3d`).

The tab never invents a team, never hardcodes titles, and never touches the filesystem. Product screenshots, if added later, are lab captures of live cmux only.

## CLI

| Command | What it does |
| --- | --- |
| `sync` | One-shot `bd` → `cmux set-status` (`bead:<id>`) |
| `watch` | Loop sync (default 3s) |
| `status` | Print the projection plan |
| `clear` | Remove `bead:*` keys only |
| `update <id> --status S` | Persist via `bd update -s` (not the sidebar), then refresh pills |
| `update <id> --claim` | Persist via `bd update --claim` |
| `install` | Contrib/legacy: copy the interpreted scenes to `~/.config/cmux/sidebars/` |

```sh
cmux-beads sync --workspace <id>
cmux-beads watch --cwd . --interval 3
cmux-beads update lab-2 --status in_progress
```

`--workspace` or `CMUX_WORKSPACE_ID` selects the host. If both are missing, `cmux identify --json` is used. The CLI will not guess a random workspace.

`scripts/install.sh` is a contributor/dev helper: it builds the release binary and symlinks it into `~/.local/bin`. It is not an end-user install path.

## Manifest

```toml
[plugin]
name = "cmux-beads"
kind = "sidebar"
version = "0.2.2"
description = "Official Beads tab on the cmux right sidebar; PTY TUI is keyboard-only fallback"

[run]
command = ["target/release/cmux-beads"]

[build]
command = ["cargo", "build", "--release"]
```

## Test

```sh
cargo fmt --check
cargo clippy --all-targets -- --deny warnings
cargo test
cargo build --release
```

## License

[MIT](LICENSE). See [CHANGELOG](CHANGELOG.md).

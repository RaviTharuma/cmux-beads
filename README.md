# cmux-beads

[![Release](https://img.shields.io/github/v/release/RaviTharuma/cmux-beads)](https://github.com/RaviTharuma/cmux-beads/releases/latest)

The official [Beads](https://github.com/steveyegge/beads) GUI inside [cmux](https://github.com/manaflow-ai/cmux). A native right-sidebar panel — not an iframe, not a terminal board stuffed in a pane. Mouse, click, drag-and-drop. People should know it is Beads.

Custom sidebars cannot spawn `bd`. `cmux-beads watch` projects issues into live `bead:<id>` pills; the sidebar is the Beads board, reading cmux context and running `cmux()` taps.

Requires **`bd` v0.60+** and a cmux build with custom sidebars.

## Install (native UI)

```sh
# from a clone
./scripts/install.sh
# or, after cargo install / plugin build
cmux-beads install

cmux right-sidebar set custom beads
# older builds: cmux sidebar open beads

cmux-beads watch
```

`install` copies `sidebars/beads.js` (and `beads.swift`) to `~/.config/cmux/sidebars/`. `.js` wins: Reorderable rows keep identity across live status updates.

Then keep `cmux-beads watch` running in the repo (or pass `--cwd` / `--workspace`). Each tick writes `cmux set-status bead:<id>` and clears stale `bead:*` keys.

## What you see

- **Beads** — the board is the product. Projected `bead:<id>` rows, grouped by status.
- **Host** — live cmux workspaces under the board. Drag to reorder (`workspace.reorder`). Click to select (`workspace.select`).
- **Surfaces** — tabs on the selected host run `surface.focus`.
- Theme tokens only in the sidebar file: `accent`, `primary`, `secondary`, `tertiary`.

The sidebar never invents a team, never hardcodes titles, and never touches the filesystem.

## CLI

| Command | What it does |
| --- | --- |
| `install` | Copy native sidebar files to `~/.config/cmux/sidebars/` |
| `sync` | One-shot `bd` → `cmux set-status` (`bead:<id>`) |
| `watch` | Loop sync (default 3s) |
| `status` | Print the projection plan |
| `clear` | Remove `bead:*` keys only |
| `update <id> --status S` | Persist via `bd update -s` (not the sidebar), then refresh pills |
| `update <id> --claim` | Persist via `bd update --claim` |

```sh
cmux-beads sync --workspace <id>
cmux-beads watch --cwd . --interval 3
cmux-beads update lab-2 --status in_progress
```

`--workspace` or `CMUX_WORKSPACE_ID` selects the host. If both are missing, `cmux identify --json` is used. The CLI will not guess a random workspace.

## Keyboard-only fallback

PTY sidebar plugins do **not** get mouse. Do not expect drag-and-drop there.

```sh
cmux sidebar plugin install https://github.com/RaviTharuma/cmux-beads.git
cmux sidebar plugin use cmux-beads
```

That path is the v0.1 TUI: keys only, Ghostty TERM colors, `prefix-S` to focus. `Esc` never quits.

## Manifest

```toml
[plugin]
name = "cmux-beads"
kind = "sidebar"
version = "0.2.0"
description = "Native Beads sidebar for cmux (watch + custom beads panel); PTY TUI is keyboard-only fallback"

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

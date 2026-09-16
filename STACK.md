# Stack

What this repository actually uses. Versions are pinned in `Cargo.toml` / `Cargo.lock` unless noted.

## Runtime product surface

| Piece | Technology | Notes |
| --- | --- | --- |
| Built-in Beads tab | cmux host (`RightSidebarMode.beads`) | Sibling of Files / Find / Dock; not implemented in this repo |
| Interpreted Board / List UI | Restricted JS + Swift subset | `sidebars/beads.js`, `sidebars/beads.swift` — contrib/legacy for Custom slot; chrome reference for the host tab |
| Projection + writes | Rust CLI `cmux-beads` | `sync` / `watch` / `status` / `clear` / `update` / `install` |
| Keyboard fallback | ratatui + crossterm | PTY plugin path when `CMUX_SIDEBAR=1`; no mouse capture |
| Issue store | Beads `bd` CLI **v0.60+** | JSON via `list` / `ready` / `show`; writes via argv |
| Host IPC | `cmux` CLI + `cmux-client` | `set-status`, `clear-status`, `list-status`, `set-progress`, `identify --json`; live cwd via `CMUX_TUI_SOCKET` |

## Languages

| Language | Where | Edition / constraints |
| --- | --- | --- |
| Rust | `src/**` | edition `2024`, `rust-version = "1.88"`, toolchain `stable` (`rust-toolchain.toml`) |
| JavaScript | `sidebars/beads.js` | cmux restricted custom-sidebar scene (signals, `cmux()`, no Node APIs) |
| Swift | `sidebars/beads.swift` | cmux restricted Swift subset (expression root, not a full app) |
| Bash | `scripts/install.sh`, `scripts/uninstall.sh` | Contributor helpers |

## Rust crate dependencies

From `Cargo.toml` (see lockfile for exact revisions):

| Crate | Role |
| --- | --- |
| `anyhow` | Error propagation in CLI / sync |
| `cmux-client` | Live pane / cwd when a TUI socket is present |
| `crossterm` | Terminal events for the keyboard TUI |
| `ratatui` | Keyboard Board / List / Table / Kanban UI |
| `serde` + `serde_json` | `bd --json` and sync report encoding |

Release profile: `lto = true`, `strip = true`, `opt-level = "z"`.

## Plugin packaging

`cmux-plugin.toml`:

- `kind = "sidebar"`
- `version` aligned with crate (`0.2.4` at time of writing)
- `build.command`: `cargo build --release`
- `run.command`: `target/release/cmux-beads`

Official install path for end users is the cmux plugin manager (or the built-in host tab), not `cargo install` alone. `scripts/install.sh` only builds/symlinks the CLI for contributors.

## External tools expected on PATH

| Tool | Required for |
| --- | --- |
| `bd` (≥ 0.60) | Listing and mutating Beads issues |
| `cmux` | Status projection and workspace identify |
| `cargo` | Building the plugin / contributor install |

## CI

GitHub Actions (`.github/workflows/ci.yml`): Ubuntu + macOS — `cargo fmt --check`, `clippy --deny warnings`, `test --locked`, `build --release --locked`.

## Explicit non-stack

This project does **not** use a web frontend framework, Electron, iframes, WKWebView, a private cloud API, or a second issue database. The sidebar never talks to Beads directly; projection and argv writes do.

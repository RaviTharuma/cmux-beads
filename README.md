# cmux-beads

A [Beads](https://github.com/steveyegge/beads) (`bd`) board for the [cmux](https://github.com/manaflow-ai/cmux) right sidebar.

`bd` is where the work lives. This plugin is the window onto it. Cards are `bd` issues, columns are `bd` statuses, and every write is a `bd` argv call. The board never invents its own store.

The herdr 0.7 analog is the **cmux plugin manager**: `cmux sidebar plugin install` / `use` / uninstall, not `herdr plugin install`.

Requires **`bd` v0.60+** on `PATH` (`list --json`, `ready --json`, `show --json`, `update --claim`, `note`, `comment`).

## Install

Plugin installation needs a cmux build that ships sidebar plugins. After you publish this repository:

```sh
cmux sidebar plugin install <git-url>
cmux sidebar plugin use cmux-beads
```

`<git-url>` is this repo once it exists on GitHub (or any git remote). Example once published:

```sh
cmux sidebar plugin install https://github.com/<you>/cmux-beads
cmux sidebar plugin use cmux-beads
```

Install clones to `~/.local/share/cmux/mux-plugins/cmux-beads` (or `$XDG_DATA_HOME/cmux/mux-plugins/cmux-beads`), runs `cargo build --release`, and records the binary in the cmux-tui config.

A Rust toolchain (1.88+) is required on install because the plugin builds from source.

Then focus the sidebar with **prefix-S** (cmux default). The plugin runs in the sidebar PTY (`CMUX_SIDEBAR=1`).

### Uninstall

```sh
cmux sidebar plugin uninstall cmux-beads
```

If your cmux build uses a different verb, remove `~/.local/share/cmux/mux-plugins/cmux-beads` and drop `sidebar.plugin` from `~/.config/cmux/cmux-tui.json`.

## What you see

The sidebar lists issues for the **focused pane's working directory**.

1. Socket path from `CMUX_TUI_SOCKET` (legacy `CMUX_MUX_SOCKET`), same contract as [cmux-sidebar-fzf](https://github.com/manaflow-ai/cmux-sidebar-fzf).
2. `list-workspaces` to find the active pane and every live workspace/screen/pane.
3. `process-info` on the focused pane's active tab surface for `cwd`.
4. `bd` is spawned with that `cwd` and an argv vector — never a shell string.

Three views, one keystroke apart (`K` / `Tab`), switched in-process:

- **List** — grouped by status.
- **Table** — flat columns; `o` cycles sort (status, priority, changed).
- **Kanban** — one column per `bd` status. Narrow sidebars show one column at a time (`h`/`l` to change). `v` then `h`/`l` writes `bd update -s`.

If `bd` is missing, the sidebar shows an install hint and stays up. The mux does not crash.

## Assign to a live cmux pane

`bd` has no first-class chat or pane id. Assignment is stored **on the issue**:

| Field | Value |
| --- | --- |
| `bd` assignee | `cmux:{workspace_id}/{pane_id}` |

Example: pane 12 in workspace 1 becomes assignee `cmux:1/12`.

Press **`A`** to pick a running workspace/screen/pane from the live cmux tree (`list-workspaces`). The picker shows names; the write stores the id form so a rename does not lose the link. The header marks assigned beads with `*`. Detail resolves the id back to `workspace > screen > pane` while that session is still live.

No second database. If you need a human name as well, put it in a note.

## Keys

Mouse is not forwarded to sidebar plugins. Everything is a key.

| Key | Action |
| --- | --- |
| `K` / `Tab` | cycle List / Table / Kanban |
| `Shift+Tab` | cycle view backwards |
| `j` `k` / arrows | move selection |
| `h` `l` | kanban: change column |
| `v` | move mode: `h`/`l` retags status via `bd` |
| `gg` / `G` | first / last |
| `Enter` | detail modal (`bd show --json`) |
| `d` | toggle inline detail pane |
| `c` | claim (`bd update --claim`) |
| `x` | close (`bd close -r`, reason required) |
| `s` then `1`-`9` | set status (`bd update -s`) |
| `p` then `0`-`4` | set priority (`bd update -p`) |
| `a` | create (`bd create`) |
| `e` | edit title/description/type/priority/labels/parent/assignee |
| `n` | note (`bd note`) |
| `m` | comment (`bd comment`) |
| `A` | assign to a live cmux pane (`bd update -a`) |
| `/` | filter. `Esc` clears |
| `R` | toggle ready (`bd ready --json`) |
| `C` | show or hide closed |
| `S` | scope: repo ↔ `bd --global` |
| `o` | table: cycle sort |
| `r` | refresh from `bd` |
| `?` | help |
| `q` / `Ctrl-C` | quit |
| `Esc` | back out a layer — **never quits** |

Create / edit form: `Tab` moves fields, `←` `→` cycle type / priority / parent epic, `Space` toggles start-in-backlog, `Enter` writes.

cmux's prefix chord leaves the sidebar. `Esc` is not the host escape hatch.

## Standalone development

```sh
# Against a live cmux session
CMUX_TUI_SOCKET=/path/to/cmux-tui.sock cargo run

# Against a fixture repo, no socket
cargo run -- --cwd /path/to/a/repo/with/.beads

# Headless dump of the same board the TUI would draw
cargo run -- --selftest --cwd /path/to/a/repo/with/.beads
```

Without a socket the plugin uses the process working directory and labels itself `solo`. Assign-to-pane needs a live socket.

## Manifest

`cmux-plugin.toml` follows the published sidebar contract:

```toml
[plugin]
name = "cmux-beads"
kind = "sidebar"
version = "0.1.0"
description = "Beads (bd) board for the cmux right sidebar: list, table, kanban"

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

CI cannot talk to a live cmux socket. Unit tests cover:

- `bd list --json` / `bd ready --json` / `bd show --json` parsers (including notes)
- argv builders (user text stays one argument), including note/comment/assignee
- filter, kanban columns, view cycling, live-pane assignee mapping
- the plugin manifest (`kind = sidebar`, name `cmux-beads`)

### Manual smoke

1. `cmux sidebar plugin install <git-url>` then `cmux sidebar plugin use cmux-beads`.
2. In a repo with beads:

   ```sh
   bd init
   bd create "Smoke test from cmux sidebar" -p 2
   bd list --json
   ```

3. Open that repo in a cmux pane. Press **prefix-S**.
4. The sidebar should list the same ids/titles/statuses/priorities as `bd list --json`.
5. `K` switches List → Table → Kanban. `c` claims, `x` closes (reason required), `A` assigns to a live pane, `r` refreshes. Confirm with another `bd list --json` in the pane.

## How it works

| Board | `bd` |
| --- | --- |
| List / Table / Kanban | `bd list --json` |
| Ready filter | `bd ready --json` |
| Detail | `bd show <id> --json` |
| Claim | `bd update <id> --claim` |
| Close | `bd close <id> -r <reason>` |
| Status / kanban move | `bd update <id> -s <status>` |
| Priority | `bd update <id> -p <n>` |
| Note | `bd note <id> <text>` |
| Comment | `bd comment <id> <text>` |
| Create | `bd create <title> -t … -p … [--description] [-a] [--parent] [-l]` |
| Edit | `bd update <id> --title … -t … -p …` |
| Assign to pane | `bd update <id> -a cmux:{workspace_id}/{pane_id}` |
| Global scope | `bd --global …` |

Writes are local. Syncing (`bd dolt push`) stays your deliberate step.

## License

[MIT](LICENSE).

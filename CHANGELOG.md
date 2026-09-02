# Changelog

All notable changes to this project are documented here.

## [0.2.0] - 2026-09-02

Native Beads GUI in cmux is the product. The PTY TUI stays as a keyboard-only fallback.

- Official Beads branding: the right-sidebar panel *is* Beads, not an iframe and not a terminal board stuffed in a pane.
- Interpreted custom sidebar: `sidebars/beads.js` (reactive JS, Reorderable host list survives live updates) and `sidebars/beads.swift` (herdr-shaped fallback). Mouse, click, drag-and-drop. Theme tokens `accent` / `primary` / `secondary` / `tertiary`.
- Live workspaces, tabs, agents, and `w.statuses` only. Taps run `cmux()` (`workspace.select`, `surface.focus`, `workspace.reorder`). No `bd`, no filesystem, no invented team.
- `cmux-beads install` copies the sidebar files to `~/.config/cmux/sidebars/`.
- `cmux-beads sync` / `watch` project `bd` issues into `cmux set-status` keys (`bead:<id>`). Stale `bead:*` keys are cleared. Status writes back to `bd` stay on CLI argv (`update`).
- Product path: install the native sidebar, `cmux right-sidebar set custom beads`, then `cmux-beads watch`.
- Official `cmux sidebar plugin install` remains the keyboard-only PTY fallback (no mouse).
- Docs do not ship invented sidebar screenshots. A real cmux capture can be added later; until then the README has no product images.

## [0.1.0] - 2026-09-02

First public sidebar plugin release.

- Beads board for the cmux right sidebar: List, Table, and Kanban, switched in-process.
- Every write is a `bd` argv call (`list` / `ready` / `show` / `update` / `create` / `close` / `note` / `comment`).
- Official install through the cmux plugin manager (`cmux sidebar plugin install` / `use` / `update`).
- Live workspaces and panes from `CMUX_TUI_SOCKET` (`list-workspaces`); assign stores `cmux:{workspace_id}/{pane_id}` on the issue.
- Colors inherit the Ghostty / cmux TERM palette (named ANSI + reverse/dim). No private RGB theme.
- Keyboard only. Sidebar PTYs do not receive mouse, so there is no drag-and-drop.

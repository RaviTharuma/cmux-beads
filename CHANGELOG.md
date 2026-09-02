# Changelog

All notable changes to this project are documented here.

## [0.1.0] - 2026-09-02

First public sidebar plugin release.

- Beads board for the cmux right sidebar: List, Table, and Kanban, switched in-process.
- Every write is a `bd` argv call (`list` / `ready` / `show` / `update` / `create` / `close` / `note` / `comment`).
- Official install through the cmux plugin manager (`cmux sidebar plugin install` / `use` / `update`).
- Live workspaces and panes from `CMUX_TUI_SOCKET` (`list-workspaces`); assign stores `cmux:{workspace_id}/{pane_id}` on the issue.
- Colors inherit the Ghostty / cmux TERM palette (named ANSI + reverse/dim). No private RGB theme.
- Keyboard only. Sidebar PTYs do not receive mouse, so there is no drag-and-drop.

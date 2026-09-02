# Changelog

All notable changes to this project are documented here.

## [0.2.2] - 2026-09-02

Beads is a **tab on the existing right sidebar** (sibling of Files / Find /
Dock), not a pane and not the left workspace list. Docs, CLI help, and scripts
now say so.

### Changed

- Product paths are the built-in host tab (`cmux right-sidebar set beads`,
  manaflow-ai/cmux#11707 / #11709) and the official plugin manager
  (`cmux sidebar plugin install …` / `use cmux-beads`). `cmux-beads watch`
  still projects `bd` issues into `bead:<id>` pills for both.
- `cmux-beads --help` leads with those paths and lists
  `cmux sidebar open beads` (pane), `cmux sidebar select beads` (left workspace
  list), and `cmux right-sidebar set custom beads` (generic Custom slot) under
  an explicit "Not the product" heading. Help is now rendered by
  `cli::help_text()` so tests assert the rendered string, not source text.
- `scripts/install.sh` is a contributor/dev CLI symlink helper. It no longer
  copies interpreted sidebar scenes by default; pass `--legacy-custom-sidebar`
  for that. Next steps point at the right-sidebar tab and plugin manager.
- `cmux-beads install` is labeled contrib/legacy and prints the product paths
  after copying.
- `sidebars/beads.js` and `sidebars/beads.swift` carry a
  `CONTRIB / LEGACY — NOT THE PRODUCT` header. They remain in-tree for
  reference against the generic Custom slot.

## [0.2.1] - 2026-09-02

Deeper native right-sidebar chrome. The Beads panel matches built-in cmux
examples: glass surface, host-chrome washes, optimistic Reorderable drag,
context menus, and a 3pt tinted rail on live bead cards. Keyboard TUI is
not the product.

### Changed

- `sidebars/beads.js` uses the same glass / `#7f7f7f**` wash language as
  built-in `panel-todo.js`, `agents-cards.js`, and `workspaces.js`.
- Host switcher uses `Image("line.3.horizontal")`, `hoverBackground`, pin and
  unread badges, and `workspace.action` context menus (Pin, Mark read, Move).
- Optimistic `selectOverride` / `orderOverride` so mouse select and drag stay
  visible while cmux context refreshes.
- Live bead cards use a 3pt rail tinted from the projected `s.color` (fallback
  `accent`). Cards tap through `cmux("workspace.select")`.
- `sidebars/beads.swift` matches: board first, Reorderable host list with a
  drag handle, HOST / SURFACES labels, no clock-as-hero.
- README no longer presents the keyboard TUI as a first-class product path.

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

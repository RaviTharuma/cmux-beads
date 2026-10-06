# Architecture

```
bd CLI (Beads) ──(bd events tail --follow | 3s poll fallback)──▶ cmux-beads watch
                                                                  │ cmux-client
                                                                  ▼
                                   cmux status pills `bead:<id>`  ──▶  right-sidebar Beads tab
                                                                       (sidebars/beads.swift|.js)
cmux-beads (TUI) ── ratatui Board/List kanban (src/board.rs, ui.rs, form.rs) ── bd create/update/close
```

- The sidebar interpreter cannot spawn `bd`, so the Rust binary projects issues into pills that the tab renders.
- `src/bd/` wraps the CLI; `project.rs`/`cwd.rs` scope to the focused workspace; `install.rs` + `scripts/` manage plugin install.

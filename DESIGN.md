# Design

Right-sidebar **Beads tab** + Board/List kanban (Trello-like) with focus scoping.
- **Native first**: inherit the cmux sidebar chrome and the Ghostty/terminal theme (colours, fonts, spacing); no custom palette.
- Status is shown as compact pills; colour carries state only together with a text/glyph (never colour alone).
- Must stay legible in both light and dark cmux themes; test at narrow sidebar widths.
- Board columns follow Beads status; cards show id, title, priority; keyboard-first navigation (see `src/keys.rs`).

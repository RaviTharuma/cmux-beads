# Integrations

External software this project talks to. No secret values here — credentials are declared (by name only) in Varlock's `.env.schema` or the platform's credential store.

| System | Purpose | How |
| --- | --- | --- |
| cmux | Host app: right-sidebar tab, status pills, plugin manager | `cmux-client`, `cmux right-sidebar set beads` |
| Beads (`bd`) | Issue source of truth | CLI: list/show/create/update/close, `events tail --follow` |
| Ghostty | Terminal colours inherited by the TUI | TERM/theme |

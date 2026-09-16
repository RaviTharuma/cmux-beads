# Disclaimer

## What this project is

**cmux-beads** is the official Beads right-sidebar experience for [cmux](https://github.com/manaflow-ai/cmux): a Board / List kanban tab driven by live `bead:<id>` status pills, with Host / Focus / Assigned scoping and native cmux chrome. Projection is performed by the `cmux-beads` Rust CLI (`watch` / `sync`). Optional interpreted scenes (`sidebars/beads.js`, `sidebars/beads.swift`) document the Custom-slot UI contract.

Beads itself is the [steveyegge/beads](https://github.com/steveyegge/beads) issue system (`bd` CLI). cmux is the [manaflow-ai/cmux](https://github.com/manaflow-ai/cmux) multiplexer. This repository does not replace either product.

## What this project is not

- Not a Bonsplit pane (`cmux sidebar open beads`).
- Not a replacement for the left workspace list (`cmux sidebar select beads`).
- Not an iframe, WKWebView, or embedded browser.
- Not a hosted SaaS, team cloud, or second issue database.
- Not a guarantee that every cmux build already ships `RightSidebarMode.beads` — host integration is tracked upstream (manaflow-ai/cmux#11707 / #11709).
- Not a path for the interpreted sidebar to mutate Beads: status changes go through `cmux-beads update` or the keyboard TUI.

The in-tree JS/Swift sidebars are **contrib/legacy** for the generic Custom slot (`cmux right-sidebar set custom beads`). They are kept for reference and visual parity; they are not the primary product path.

## Warranty

This software is provided under the [MIT License](LICENSE), **as is**, without warranty of any kind. Use at your own risk. The authors and contributors are not liable for data loss, incorrect status projection, or disruption of your Beads database or cmux workspaces.

## Trademarks / names

“Beads”, “bd”, “cmux”, and related names belong to their respective owners. Use of those names here describes interoperability only.

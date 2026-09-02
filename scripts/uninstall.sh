#!/usr/bin/env bash
# Remove the ~/.local/bin contributor symlink and any contrib/legacy custom
# sidebar scenes copied into ~/.config/cmux/sidebars/.
#
# Does not touch the built-in Beads right-sidebar tab and does not uninstall a
# plugin-manager clone (use `cmux sidebar plugin` for that).
set -euo pipefail

TARGET="${HOME}/.local/bin/cmux-beads"
SIDEBAR_DST_DIR="${HOME}/.config/cmux/sidebars"

echo "cmux-beads uninstall"

if [[ -L "${TARGET}" || -f "${TARGET}" ]]; then
  rm -f "${TARGET}"
  echo "  cli: removed ${TARGET}"
fi

for name in beads.js beads.swift; do
  path="${SIDEBAR_DST_DIR}/${name}"
  if [[ -f "${path}" ]]; then
    rm -f "${path}"
    echo "  legacy custom scene: removed ${path}"
  fi
done

echo "Done."

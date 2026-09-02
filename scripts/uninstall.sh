#!/usr/bin/env bash
# Remove the native sidebar copies and the ~/.local/bin symlink.
# Does not uninstall a cmux sidebar plugin clone.
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
    echo "  sidebar: removed ${path}"
  fi
done
echo "Done."

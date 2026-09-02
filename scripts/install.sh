#!/usr/bin/env bash
# Contributor/dev install (CLI symlink + native custom sidebar).
# End-user product path: cmux-beads install && cmux right-sidebar set custom beads
# Keyboard-only fallback: cmux sidebar plugin install <this-repo.git>
# No root required.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_SRC="${ROOT}/target/release/cmux-beads"
LOCAL_BIN="${HOME}/.local/bin"
TARGET="${LOCAL_BIN}/cmux-beads"
SIDEBAR_SRC="${ROOT}/sidebars"
SIDEBAR_DST_DIR="${HOME}/.config/cmux/sidebars"

echo "cmux-beads native sidebar install"
echo "  repo: ${ROOT}"

if [[ ! -x "${BIN_SRC}" ]]; then
  echo "  building release CLI…"
  (cd "${ROOT}" && cargo build --release)
fi
if [[ ! -x "${BIN_SRC}" ]]; then
  echo "error: missing ${BIN_SRC}" >&2
  exit 1
fi

mkdir -p "${LOCAL_BIN}"
if ln -sfn "${BIN_SRC}" "${TARGET}" 2>/dev/null; then
  echo "  cli:  ${TARGET} -> ${BIN_SRC}"
else
  cp "${BIN_SRC}" "${TARGET}"
  chmod +x "${TARGET}"
  echo "  cli:  ${TARGET} (copied)"
fi

if [[ -f "${SIDEBAR_SRC}/beads.js" && -f "${SIDEBAR_SRC}/beads.swift" ]]; then
  mkdir -p "${SIDEBAR_DST_DIR}"
  cp "${SIDEBAR_SRC}/beads.js" "${SIDEBAR_DST_DIR}/beads.js"
  cp "${SIDEBAR_SRC}/beads.swift" "${SIDEBAR_DST_DIR}/beads.swift"
  echo "  sidebar: ${SIDEBAR_DST_DIR}/beads.js"
  echo "  sidebar: ${SIDEBAR_DST_DIR}/beads.swift"
else
  echo "error: missing ${SIDEBAR_SRC}/beads.js" >&2
  exit 1
fi

echo
echo "Next steps:"
echo "  1. Ensure ~/.local/bin is on PATH"
echo "  2. cmux right-sidebar set custom beads"
echo "  3. In the repo: cmux-beads watch"
echo
echo "Keyboard-only fallback (no mouse):"
echo "  cmux sidebar plugin install https://github.com/RaviTharuma/cmux-beads.git"
echo "  cmux sidebar plugin use cmux-beads"
echo
echo "Plugin installed."

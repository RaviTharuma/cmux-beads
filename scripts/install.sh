#!/usr/bin/env bash
# Contributor/dev helper: build the release CLI and symlink it into ~/.local/bin.
#
# This is NOT an end-user install path. Beads ships as a tab on the existing
# cmux right sidebar (sibling of Files / Find / Dock):
#   cmux right-sidebar set beads                       host tab (built-in)
#   cmux sidebar plugin install <this-repo>.git        plugin package
#   cmux sidebar plugin use cmux-beads
#
# Pass --legacy-custom-sidebar to also copy sidebars/beads.js and beads.swift
# into ~/.config/cmux/sidebars/. Those scenes target the generic Custom slot
# and are contrib/legacy, not the product.
#
# No root required.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_SRC="${ROOT}/target/release/cmux-beads"
LOCAL_BIN="${HOME}/.local/bin"
TARGET="${LOCAL_BIN}/cmux-beads"
SIDEBAR_SRC="${ROOT}/sidebars"
SIDEBAR_DST_DIR="${HOME}/.config/cmux/sidebars"

LEGACY_CUSTOM=0
for arg in "$@"; do
  case "${arg}" in
    --legacy-custom-sidebar) LEGACY_CUSTOM=1 ;;
    -h|--help)
      sed -n '2,14p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "error: unknown argument ${arg}" >&2
      exit 2
      ;;
  esac
done

echo "cmux-beads contributor install"
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

if [[ "${LEGACY_CUSTOM}" -eq 1 ]]; then
  if [[ -f "${SIDEBAR_SRC}/beads.js" && -f "${SIDEBAR_SRC}/beads.swift" ]]; then
    mkdir -p "${SIDEBAR_DST_DIR}"
    cp "${SIDEBAR_SRC}/beads.js" "${SIDEBAR_DST_DIR}/beads.js"
    cp "${SIDEBAR_SRC}/beads.swift" "${SIDEBAR_DST_DIR}/beads.swift"
    echo "  legacy custom scene: ${SIDEBAR_DST_DIR}/beads.js"
    echo "  legacy custom scene: ${SIDEBAR_DST_DIR}/beads.swift"
    echo "  (generic Custom slot — contrib/legacy, not the product)"
  else
    echo "error: missing ${SIDEBAR_SRC}/beads.js" >&2
    exit 1
  fi
fi

echo
echo "Next steps:"
echo "  1. Ensure ~/.local/bin is on PATH"
echo "  2. Open the Beads tab on the right sidebar:"
echo "       cmux right-sidebar set beads"
echo "     or install the plugin package:"
echo "       cmux sidebar plugin install https://github.com/RaviTharuma/cmux-beads.git"
echo "       cmux sidebar plugin use cmux-beads"
echo "  3. In the repo: cmux-beads watch"
echo
echo "Contributor CLI installed."

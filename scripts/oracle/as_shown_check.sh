#!/usr/bin/env bash
# LibreOffice "as shown" cross-check of the displayed values (bd-jg43.2); see as_shown_check.py.
# usage: scripts/oracle/as_shown_check.sh [--values] <file.xlsx>...   (SOFFICE=<path> to choose LibreOffice)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
soffice="${SOFFICE:-$(command -v soffice || command -v libreoffice || echo /Applications/LibreOffice.app/Contents/MacOS/soffice)}"
"$soffice" --version >&2 || true
exec python3 -I "$here/as_shown_check.py" --soffice "$soffice" "$@"

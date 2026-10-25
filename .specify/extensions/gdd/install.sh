#!/bin/bash
# Installs (or reinstalls) the GDD kit into this Spec Kit project for every integration in use.
#
# Spec Kit 1.2.x registers extension commands only for the active integration. This script
# installs the extension once per integration with the public CLI, keeps the rendered command
# files of each one, and leaves the default integration active.
#
# Usage: gdd-kit/install.sh [integrations...]   (default: claude codex opencode; the first is the default)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
INTEGRATIONS=("${@:-claude}")
[ $# -eq 0 ] && INTEGRATIONS=(claude codex opencode)
DEFAULT="${INTEGRATIONS[0]}"
KEEP="$(mktemp -d "${TMPDIR:-/var/tmp}/gdd-install.XXXXXX")"
trap 'rm -rf "$KEEP"' EXIT

command -v specify >/dev/null || { echo "specify (Spec Kit) is not on PATH"; exit 1; }

# Paths where each integration keeps the rendered GDD commands.
gdd_paths() {
  ls -d .claude/skills/speckit-gdd-* .agents/skills/speckit-gdd-* .opencode/commands/speckit.gdd.* 2>/dev/null || true
}

specify extension remove gdd --force >/dev/null 2>&1 || true
# Leftovers from earlier installs (dev-mode links point into the extension folder).
find .claude/skills .agents/skills .opencode/commands -maxdepth 1 -name 'speckit*gdd*' -exec rm -rf {} + 2>/dev/null || true
for i in "${INTEGRATIONS[@]}"; do
  specify integration use "$i" >/dev/null
  specify extension add --dev ./gdd-kit --force >/dev/null
  for p in $(gdd_paths); do mkdir -p "$KEEP/$(dirname "$p")"; cp -rL "$p" "$KEEP/$(dirname "$p")/"; done
done
specify integration use "$DEFAULT" >/dev/null
specify extension add --dev ./gdd-kit --force >/dev/null
for p in $(cd "$KEEP" && ls -d .claude/skills/* .agents/skills/* .opencode/commands/* 2>/dev/null); do [ -e "$p" ] || { rm -rf "$p"; cp -rL "$KEEP/$p" "$p"; }; done

specify workflow remove gdd-change >/dev/null 2>&1 || true
specify workflow add --dev ./gdd-kit/workflows/gdd-change >/dev/null

echo "GDD kit installed for: ${INTEGRATIONS[*]} (default: $DEFAULT)"
for i in "${INTEGRATIONS[@]}"; do
  case "$i" in
    claude) n=$(ls -d .claude/skills/speckit-gdd-* 2>/dev/null | wc -l) ;;
    codex) n=$(ls -d .agents/skills/speckit-gdd-* 2>/dev/null | wc -l) ;;
    opencode) n=$(ls .opencode/commands/speckit.gdd.* 2>/dev/null | wc -l) ;;
    *) n="?" ;;
  esac
  echo "  $i: $n commands"
done

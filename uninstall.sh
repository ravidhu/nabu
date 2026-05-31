#!/usr/bin/env bash
# nabu uninstaller — removes the binary, launcher, and caches.
# Inspired by the `make clean` and `make install` targets.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/uninstall.sh | bash
#   # or, after cloning the repo:
#   ./uninstall.sh
#
# Optional flags:
#   --yes   skip the confirmation prompt
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
RESET='\033[0m'

ASSUME_YES=false
for arg in "$@"; do
  case "$arg" in
    -y|--yes) ASSUME_YES=true ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

BINARY="/usr/local/bin/nabu"
APP_PATH="$HOME/Applications/nabu.app"
NABU_HOME="$HOME/.nabu"
WESPEAKER_DIR="$HOME/.wespeaker"
SESSIONS_DIR="$HOME/nabu_data"
HF_WHISPER_GLOB="$HOME/.cache/huggingface/hub/models--mlx-community--*whisper*"
HF_PYANNOTE_GLOB="$HOME/.cache/huggingface/hub/models--pyannote--*"

hr() { echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; }
confirm() {
  $ASSUME_YES && return 0
  read -r -p "$1 [y/N] " r
  [[ "$r" =~ ^([yY]|[yY][eE][sS])$ ]]
}

echo ""
echo -e "${BOLD}nabu uninstaller${RESET}"
hr

echo "The following will be removed:"
echo "  • $BINARY"
echo "  • $APP_PATH"
echo "  • $NABU_HOME"
echo "  • $WESPEAKER_DIR"
echo ""
echo "You will be asked separately about:"
echo "  • $SESSIONS_DIR  (your recordings + transcripts)"
echo "  • HuggingFace model caches (whisper, pyannote — may be shared with other tools)"
echo ""

if ! confirm "Continue?"; then
  echo "Aborted."
  exit 0
fi

# ── Stop running processes ────────────────────────────────────────────────────

if pgrep -x nabu >/dev/null 2>&1; then
  echo "Stopping running nabu processes..."
  pkill -x nabu || true
  sleep 1
fi

# ── Remove binary ─────────────────────────────────────────────────────────────

if [[ -f "$BINARY" ]]; then
  echo "Removing $BINARY (sudo may prompt)..."
  sudo rm -f "$BINARY"
  echo -e "${GREEN}✓ Removed binary${RESET}"
else
  echo "Skipping binary — not found at $BINARY"
fi

# ── Remove launcher app ───────────────────────────────────────────────────────

if [[ -d "$APP_PATH" ]]; then
  rm -rf "$APP_PATH"
  echo -e "${GREEN}✓ Removed launcher${RESET}"
else
  echo "Skipping launcher — not found at $APP_PATH"
fi

# ── Remove nabu caches ────────────────────────────────────────────────────────

for d in "$NABU_HOME" "$WESPEAKER_DIR"; do
  if [[ -d "$d" ]]; then
    rm -rf "$d"
    echo -e "${GREEN}✓ Removed $d${RESET}"
  fi
done

# ── Sessions (recordings + transcripts) ───────────────────────────────────────

if [[ -d "$SESSIONS_DIR" ]]; then
  echo ""
  echo -e "${YELLOW}Recordings live in $SESSIONS_DIR.${RESET}"
  if confirm "Delete recordings?"; then
    rm -rf "$SESSIONS_DIR"
    echo -e "${GREEN}✓ Removed recordings${RESET}"
  else
    echo "Keeping recordings."
  fi
fi

# ── HuggingFace model cache (shared) ──────────────────────────────────────────

if compgen -G "$HF_WHISPER_GLOB" > /dev/null || compgen -G "$HF_PYANNOTE_GLOB" > /dev/null; then
  echo ""
  echo -e "${YELLOW}HuggingFace cached models found.${RESET}"
  echo "  ($HOME/.cache/huggingface/hub/models--mlx-community--*whisper*)"
  echo "  ($HOME/.cache/huggingface/hub/models--pyannote--*)"
  echo "These may be shared with other tools that use HuggingFace."
  if confirm "Delete these model caches?"; then
    rm -rf $HF_WHISPER_GLOB $HF_PYANNOTE_GLOB
    echo -e "${GREEN}✓ Removed model caches${RESET}"
  else
    echo "Keeping HuggingFace caches."
  fi
fi

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
hr
echo -e "${GREEN}${BOLD}nabu uninstalled.${RESET}"
echo ""
echo -e "${YELLOW}macOS Privacy permissions stay until you remove them manually:${RESET}"
echo "  • System Settings → Privacy & Security → Microphone"
echo "  • System Settings → Privacy & Security → Screen Recording"
echo ""

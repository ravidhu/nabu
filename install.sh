#!/usr/bin/env bash
# nabu installer
# Usage: curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
RESET='\033[0m'

BINARY_URL="https://github.com/ravidhu/nabu/releases/latest/download/nabu-aarch64-apple-darwin"
APP_PATH="$HOME/Applications/nabu.app"
TOTAL_STEPS=5

hr() { echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"; }

echo ""
echo -e "${BOLD}nabu installer${RESET}"
hr

# ── Guards ────────────────────────────────────────────────────────────────────

if [[ "$(uname)" != "Darwin" ]]; then
  echo -e "${RED}Error: nabu requires macOS.${RESET}" && exit 1
fi

if [[ "$(uname -m)" != "arm64" ]]; then
  echo -e "${RED}Error: nabu requires Apple Silicon (M1/M2/M3/M4).${RESET}"
  echo "This Mac has architecture: $(uname -m)"
  exit 1
fi

# ── Step 1 — Download binary ──────────────────────────────────────────────────

echo -e "\n${BOLD}[1/${TOTAL_STEPS}]${RESET} Downloading nabu binary..."
TMP_BIN=$(mktemp)
if ! curl -fsSL "$BINARY_URL" -o "$TMP_BIN"; then
  echo -e "${RED}Error: download failed. Check your internet connection and that the repo is public.${RESET}"
  exit 1
fi
chmod +x "$TMP_BIN"
echo -e "${GREEN}✓ Downloaded${RESET}"

# ── Step 2 — Install binary ───────────────────────────────────────────────────

echo -e "\n${BOLD}[2/${TOTAL_STEPS}]${RESET} Installing to /usr/local/bin/nabu..."
echo "      (macOS will ask for your password)"
sudo mkdir -p /usr/local/bin
sudo mv "$TMP_BIN" /usr/local/bin/nabu
# Remove quarantine flag set by curl/browser downloads
sudo xattr -d com.apple.quarantine /usr/local/bin/nabu 2>/dev/null || true
echo -e "${GREEN}✓ Installed — nabu $(nabu --version 2>/dev/null || echo '')${RESET}"

# ── Step 3 — Download AI models ───────────────────────────────────────────────

echo -e "\n${BOLD}[3/${TOTAL_STEPS}]${RESET} Downloading AI models (~3 GB, one-time)..."
echo "      This takes a few minutes — grab a coffee."
nabu --setup
echo -e "${GREEN}✓ Models ready${RESET}"

# ── Step 4 — Create launcher app ──────────────────────────────────────────────

echo -e "\n${BOLD}[4/${TOTAL_STEPS}]${RESET} Creating nabu.app launcher..."
mkdir -p "$HOME/Applications"

osacompile -o "$APP_PATH" -e '
tell application "Terminal"
    activate
    do script "nabu"
end tell
' 2>/dev/null

echo -e "${GREEN}✓ nabu.app created in ~/Applications${RESET}"

# ── Step 5 — Verify with nabu --doctor ────────────────────────────────────────
#
# `nabu --doctor` runs nine checks and exits non-zero on FAIL. We capture the
# exit code so `set -e` does not abort the script — a failed check is expected
# the first time around (Screen Recording cannot be granted by an installer).
# The closing message is tailored to whether the doctor passed or not.

echo -e "\n${BOLD}[5/${TOTAL_STEPS}]${RESET} Verifying with nabu --doctor..."
echo ""
DOCTOR_RC=0
nabu --doctor || DOCTOR_RC=$?

# ── Done ──────────────────────────────────────────────────────────────────────

echo ""
hr
if [[ $DOCTOR_RC -eq 0 ]]; then
  echo -e "${GREEN}${BOLD}All done — nabu is ready to record.${RESET}"
else
  echo -e "${YELLOW}${BOLD}Almost there — a few permissions still need granting.${RESET}"
fi
echo ""
echo "How to start a recording:"
echo ""
echo -e "  ${BOLD}Option A${RESET}  Double-click nabu.app in ~/Applications"
echo -e "            (drag it to your Dock for one-click access)"
echo ""
echo -e "  ${BOLD}Option B${RESET}  Press Cmd+Space, type ${BOLD}nabu${RESET}, press Enter"
echo ""
echo -e "  ${BOLD}Option C${RESET}  Type ${BOLD}nabu${RESET} in any Terminal window"
echo ""
echo -e "Press ${BOLD}Ctrl-C${RESET} to stop. Transcripts are saved to ~/nabu_data/"
echo ""

if [[ $DOCTOR_RC -ne 0 ]]; then
  echo -e "${YELLOW}Next: grant the permissions flagged above.${RESET}"
  echo "Each [FAIL] line above has a numbered 1-2-3 walkthrough."
  echo ""
  echo "To save you a click, opening the Screen Recording pane now..."
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture" 2>/dev/null || true
  echo ""
  echo -e "Re-run ${BOLD}nabu --doctor${RESET} any time to re-check."
  echo ""
fi

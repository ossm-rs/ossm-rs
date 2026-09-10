#!/usr/bin/env bash
set -euo pipefail

ok=true

version() {
    "$1" --version 2>/dev/null | head -1
}

check() {
    if command -v "$1" &>/dev/null; then
        printf '  \033[32m✓\033[0m %-12s %s\n' "$1" "$(version "$1")"
    else
        printf '  \033[31m✗\033[0m %-12s %s\n' "$1" "$2"
        ok=false
    fi
}

check_optional() {
    if command -v "$1" &>/dev/null; then
        printf '  \033[32m✓\033[0m %-12s %s\n' "$1" "$(version "$1")"
    else
        printf '  \033[33m~\033[0m %-12s %s (optional)\n' "$1" "$2"
    fi
}

check_esp_toolchain() {
    if cargo +esp --version &>/dev/null; then
        printf '  \033[32m✓\033[0m %-12s %s\n' "+esp" "$(cargo +esp --version 2>/dev/null | head -1)"
    else
        printf '  \033[31m✗\033[0m %-12s %s\n' "+esp" "needed to cross-compile for ESP32 targets"
        ok=false
    fi
}

check_export_esp() {
    if [ -f "$HOME/export-esp.sh" ]; then
        printf '  \033[32m✓\033[0m ~/export-esp.sh\n'
    else
        printf '  \033[31m✗\033[0m ~/export-esp.sh not found - needed to set ESP toolchain paths\n'
        ok=false
    fi
}

check_wasm_target() {
    if rustup +stable target list --installed 2>/dev/null | grep -q wasm32-unknown-unknown; then
        printf '  \033[32m✓\033[0m %-12s %s\n' "wasm32" "wasm32-unknown-unknown target installed"
    else
        printf '  \033[31m✗\033[0m %-12s %s\n' "wasm32" "missing wasm32-unknown-unknown target (rustup +stable target add wasm32-unknown-unknown)"
        ok=false
    fi
}

# nvm is a shell function, not a binary - check for its install directory instead
check_nvm() {
    if [ -d "${NVM_DIR:-$HOME/.nvm}" ]; then
        nvm_ver=""
        if [ -f "${NVM_DIR:-$HOME/.nvm}/nvm.sh" ]; then
            # shellcheck disable=SC1091
            . "${NVM_DIR:-$HOME/.nvm}/nvm.sh" 2>/dev/null
            nvm_ver="$(nvm --version 2>/dev/null)"
        fi
        printf '  \033[32m✓\033[0m %-12s %s\n' "nvm" "${nvm_ver:-installed}"
    else
        printf '  \033[33m~\033[0m %-12s %s (optional)\n' "nvm" "manages Node.js versions"
    fi
}

echo "Firmware..."
check cargo            "needed to compile Rust crates"
check espup            "needed to install the ESP Rust toolchain"
check espflash         "needed to flash firmware to ESP32 boards"
check_esp_toolchain
check_export_esp

echo ""
echo "Tools..."
check jq               "needed by 'just focus' to update editor settings"

echo ""
echo "Web simulator..."
check_nvm
check node             "needed as the JS runtime for pnpm"
check pnpm             "needed to run the web simulator dev server"
check wasm-bindgen     "needed to build the WASM simulator (cargo install wasm-bindgen-cli)"
check wasm-opt         "needed to optimise WASM output (install binaryen)"
check_wasm_target

echo ""
if $ok; then
    printf '\033[32mAll good!\033[0m\n'
else
    printf '\033[31mSome tools are missing. See above for install instructions.\033[0m\n'
    exit 1
fi

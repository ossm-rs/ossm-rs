#!/usr/bin/env bash
set -euo pipefail

crate="$1"
motor="${2:-}"

case "$crate" in
    esp32s3)
        default_motor="rs485"
        ;;
    esp32)
        default_motor="stepdir"
        ;;
    *)
        echo "Error: unknown arch '$crate'" >&2
        echo "Valid arches: esp32, esp32s3" >&2
        exit 1
        ;;
esac

motor="${motor:-$default_motor}"
feature="motor-${motor}"

jq --arg proj "firmware/${crate}/Cargo.toml" --arg feat "$feature" \
   '. + {
     "rust-analyzer.linkedProjects": [$proj],
     "rust-analyzer.cargo.features": [$feat]
   }' .vscode/settings.template.json > .vscode/settings.json

jq --arg proj "firmware/${crate}/Cargo.toml" --arg feat "$feature" \
   '.lsp["rust-analyzer"].initialization_options.linkedProjects = [$proj]
    | .lsp["rust-analyzer"].initialization_options.cargo.features = [$feat]' \
   .zed/settings.template.json > .zed/settings.json

echo "rust-analyzer focused on ${crate} with --features ${feature}"

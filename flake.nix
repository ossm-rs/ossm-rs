{
  description = "OSSM - Rust firmware, WASM and web tools";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # Pinned VSCodium extensions, bump with `nix flake update`
        pinnedExtensions = with pkgs.vscode-extensions; [
          rust-lang.rust-analyzer
          tamasfe.even-better-toml
          vadimcn.vscode-lldb
          streetsidesoftware.code-spell-checker
          jnoortheen.nix-ide
          dbaeumer.vscode-eslint
          esbenp.prettier-vscode
        ];

        pinnedExtensionsDir = pkgs.symlinkJoin {
          name = "ossm-vscode-extensions";
          paths = pinnedExtensions;
        };

        # `codium` wrapper: keeps user-data and extensions under
        # .vscode-local/ so VS Codium state is project-local, and seeds the
        # extensions dir with recommended defaults.
        codium-local = pkgs.writeShellScriptBin "codium" ''
          set -eu
          ROOT="''${OSSM_VSCODE_ROOT:-$PWD}"
          EXT_DIR="$ROOT/.vscode-local/extensions"
          UD_DIR="$ROOT/.vscode-local/user-data"
          mkdir -p "$EXT_DIR" "$UD_DIR"
          for src in ${pinnedExtensionsDir}/share/vscode/extensions/*; do
            ln -sfn "$src" "$EXT_DIR/$(basename "$src")"
          done
          exec ${pkgs.vscodium}/bin/codium \
            --user-data-dir "$UD_DIR" \
            --extensions-dir "$EXT_DIR" \
            "$@"
        '';
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            just

            rustup

            espup
            espflash

            wasm-bindgen-cli
            binaryen

            nodejs_22
            pnpm

            jq

            # Editor
            codium-local
          ];

          shellHook = ''
            # Ensure the stable toolchain and wasm32 target are present so that
            # `just doctor`'s rustup checks succeed.
            if ! rustup toolchain list 2>/dev/null | grep -q '^stable'; then
              echo "Installing stable Rust toolchain via rustup..."
              rustup toolchain install stable --profile minimal
            fi
            if ! rustup +stable target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$'; then
              echo "Adding wasm32-unknown-unknown target..."
              rustup +stable target add wasm32-unknown-unknown
            fi

            # The ESP Rust toolchain is too large/custom to package in Nix; espup
            # downloads it on demand. Prompt the user to run it once if missing.
            if [ ! -f "$HOME/export-esp.sh" ] || ! cargo +esp --version >/dev/null 2>&1; then
              cat <<'EOF'

ESP toolchain not installed. Run once:
    espup install --export-file "$HOME/export-esp.sh"

EOF
            fi
          '';
        };
      });
}

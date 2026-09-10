set windows-shell := ["powershell.exe", "-NoLogo", "-c", "if (Test-Path \"$env:USERPROFILE\\export-esp.ps1\") { . \"$env:USERPROFILE\\export-esp.ps1\" };"]
set shell := ["bash", "-c", ". $HOME/export-esp.sh 2>/dev/null; eval \"$0\""]
set dotenv-load := true

default:
    @just --list

# Build a firmware variant. Variants: ossm-alt, waveshare, seeed-xiao, ossm-reference.
# Pass `sim` as the second arg to swap in the simulated motor (e.g. `just build seeed-xiao sim`).
build variant motor="":
    cargo run --quiet -p ossm-flash -- {{ variant }} --build-only {{ if motor != "" { "--motor " + motor } else { "" } }}

# Build, flash, and monitor a firmware variant. Variants: ossm-alt, waveshare, seeed-xiao, ossm-reference.
# Pass `sim` as the second arg to flash a build with the simulated motor.
flash variant motor="":
    cargo run --quiet -p ossm-flash -- {{ variant }} {{ if motor != "" { "--motor " + motor } else { "" } }}


# WASM Simulator
[working-directory: 'bindings/web-simulator']
build-wasm-simulator:
    cargo +stable build --release
    wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/web_simulator.wasm
    wasm-opt -O --all-features -o pkg/web_simulator_bg.wasm pkg/web_simulator_bg.wasm
    echo '{"name":"@ossm-rs/web-simulator","type":"module","main":"web_simulator.js","types":"web_simulator.d.ts"}' > pkg/package.json

# WASM Trajectory Recorder
[working-directory: 'bindings/trajectory-recorder']
build-wasm-recorder:
    cargo +stable build --release
    wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/trajectory_recorder.wasm
    wasm-opt -O --all-features -o pkg/trajectory_recorder_bg.wasm pkg/trajectory_recorder_bg.wasm
    echo '{"name":"@ossm-rs/trajectory-recorder","type":"module","main":"trajectory_recorder.js","types":"trajectory_recorder.d.ts"}' > pkg/package.json

build-wasm: build-wasm-simulator build-wasm-recorder

# Dev server (watches Rust sources and hot-reloads WASM)
[working-directory: 'apps/web-tools']
web-tools: build-wasm
    pnpm dev --host

# All
[parallel]
build-all: (build "ossm-alt") (build "waveshare") (build "seeed-xiao") (build "ossm-reference") build-wasm

# Check that all required tools are installed
[unix]
doctor:
    scripts/doctor.sh

[windows]
doctor:
    powershell.exe -NoLogo -ExecutionPolicy Bypass -File scripts/doctor.ps1

# Focus rust-analyzer on a firmware crate by generating editor settings from templates
[unix]
focus crate:
    scripts/focus.sh {{ crate }}

[windows]
focus crate:
    powershell.exe -NoLogo -ExecutionPolicy Bypass -File scripts/focus.ps1 -Crate {{ crate }}

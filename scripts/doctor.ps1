$ok = $true

function Get-ToolVersion($cmd) {
    try { @(& $cmd --version 2>$null)[0] } catch { "" }
}

function Check($name, $reason) {
    $found = Get-Command $name -ErrorAction SilentlyContinue
    if ($found) {
        $ver = Get-ToolVersion $name
        Write-Host "  + " -ForegroundColor Green -NoNewline
        Write-Host ("{0,-12} {1}" -f $name, $ver)
    } else {
        Write-Host "  x " -ForegroundColor Red -NoNewline
        Write-Host ("{0,-12} {1}" -f $name, $reason)
        $script:ok = $false
    }
}

function CheckOptional($name, $reason) {
    $found = Get-Command $name -ErrorAction SilentlyContinue
    if ($found) {
        $ver = Get-ToolVersion $name
        Write-Host "  + " -ForegroundColor Green -NoNewline
        Write-Host ("{0,-12} {1}" -f $name, $ver)
    } else {
        Write-Host "  ~ " -ForegroundColor Yellow -NoNewline
        Write-Host ("{0,-12} {1} (optional)" -f $name, $reason)
    }
}

function CheckEspToolchain() {
    $espVer = @(& cargo +esp --version 2>&1)[0]
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  + " -ForegroundColor Green -NoNewline
        Write-Host ("{0,-12} {1}" -f "+esp", $espVer)
    } else {
        Write-Host "  x " -ForegroundColor Red -NoNewline
        Write-Host ("{0,-12} {1}" -f "+esp", "needed to cross-compile for ESP32 targets")
        $script:ok = $false
    }
}

function CheckExportEsp() {
    $exportPath = Join-Path $env:USERPROFILE "export-esp.ps1"
    if (Test-Path $exportPath) {
        Write-Host "  + " -ForegroundColor Green -NoNewline
        Write-Host "~/export-esp.ps1"
    } else {
        Write-Host "  x " -ForegroundColor Red -NoNewline
        Write-Host "~/export-esp.ps1 not found - needed to set ESP toolchain paths"
        $script:ok = $false
    }
}

function CheckWasmTarget() {
    $targets = @(& rustup +stable target list --installed 2>$null)
    if ($targets -contains "wasm32-unknown-unknown") {
        Write-Host "  + " -ForegroundColor Green -NoNewline
        Write-Host ("{0,-12} {1}" -f "wasm32", "wasm32-unknown-unknown target installed")
    } else {
        Write-Host "  x " -ForegroundColor Red -NoNewline
        Write-Host ("{0,-12} {1}" -f "wasm32", "missing wasm32-unknown-unknown target (rustup +stable target add wasm32-unknown-unknown)")
        $script:ok = $false
    }
}

Write-Host "Firmware..."
Check cargo            "needed to compile Rust crates"
Check espup            "needed to install the ESP Rust toolchain"
Check espflash         "needed to flash firmware to ESP32 boards"
CheckEspToolchain
CheckExportEsp

Write-Host ""
Write-Host "Web simulator..."
Check node             "needed as the JS runtime for pnpm"
Check pnpm             "needed to run the web simulator dev server"
Check wasm-bindgen     "needed to build the WASM simulator (cargo install wasm-bindgen-cli)"
Check wasm-opt         "needed to optimise WASM output (install binaryen)"
CheckWasmTarget

Write-Host ""
if ($ok) {
    Write-Host "All good!" -ForegroundColor Green
} else {
    Write-Host "Some tools are missing. See above for details." -ForegroundColor Red
    exit 1
}

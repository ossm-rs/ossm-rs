param(
    [Parameter(Mandatory=$true)]
    [string]$Crate,
    [string]$Motor = ""
)

$defaults = @{
    "esp32s3" = "rs485"
    "esp32"   = "stepdir"
}

if (-not $defaults.ContainsKey($Crate)) {
    Write-Error "Unknown arch '$Crate'. Valid: $($defaults.Keys -join ', ')"
    exit 1
}

if ([string]::IsNullOrEmpty($Motor)) {
    $Motor = $defaults[$Crate]
}
$feature = "motor-$Motor"

$vscode = Get-Content ".vscode/settings.template.json" | ConvertFrom-Json
$vscode | Add-Member -Force "rust-analyzer.linkedProjects" @("firmware/$Crate/Cargo.toml")
$vscode | Add-Member -Force "rust-analyzer.cargo.features" @($feature)
$vscode | ConvertTo-Json -Depth 10 | Set-Content ".vscode/settings.json"

$zed = Get-Content ".zed/settings.template.json" | ConvertFrom-Json
$zed.lsp.'rust-analyzer'.initialization_options | Add-Member -Force "linkedProjects" @("firmware/$Crate/Cargo.toml")
$zed.lsp.'rust-analyzer'.initialization_options | Add-Member -Force "cargo" ([pscustomobject]@{ features = @($feature) })
$zed | ConvertTo-Json -Depth 10 | Set-Content ".zed/settings.json"

Write-Host "rust-analyzer focused on $Crate with --features $feature"

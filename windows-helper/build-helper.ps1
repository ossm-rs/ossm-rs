$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
dotnet build "$root\OssmOwner090\OssmOwner090.csproj" -c Release
Write-Host "Built. EXE is under windows-helper\OssmOwner090\bin\Release\net8.0-windows\"

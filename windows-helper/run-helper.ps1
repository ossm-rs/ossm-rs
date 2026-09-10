$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
dotnet run --project "$root\OssmOwner090\OssmOwner090.csproj" -c Release

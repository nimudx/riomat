# Compila y prueba Riomat en Windows (PowerShell). Uso: .\scripts\build-windows.ps1 [-Run]
param([switch]$Run)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
cargo test --workspace --release
cargo build -p riomat --release
New-Item -ItemType Directory -Force dist | Out-Null
Copy-Item target\release\riomat.exe dist\riomat.exe -Force
Copy-Item README.md dist\README.md -Force
Write-Host "Listo: dist\riomat.exe"
if ($Run) { & dist\riomat.exe }

# Build ModlandSearch and its Windows installer in one step.
# Usage:  powershell -ExecutionPolicy Bypass -File build-installer.ps1

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

$iscc = Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
if (-not (Test-Path -LiteralPath $iscc)) {
    $iscc = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
}
if (-not (Test-Path -LiteralPath $iscc)) {
    throw 'Inno Setup 6 (ISCC.exe) was not found.'
}

if (-not (Test-Path -LiteralPath 'Cargo.toml')) {
    throw "Cargo.toml not found in $PSScriptRoot - build from the real project folder."
}

Write-Host '==> cargo build --release'
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed ($LASTEXITCODE)" }

$exe = Join-Path $PSScriptRoot 'target\release\ModlandZoeker.exe'
if (-not (Test-Path -LiteralPath $exe)) {
    throw "ModlandZoeker.exe was not produced at $exe"
}

Write-Host '==> ISCC installer.iss'
& $iscc (Join-Path $PSScriptRoot 'installer.iss')
if ($LASTEXITCODE -ne 0) { throw "ISCC failed ($LASTEXITCODE)" }

Write-Host "==> done: $(Join-Path $PSScriptRoot 'dist\ModlandSearchSetup-1.0.0.exe')"

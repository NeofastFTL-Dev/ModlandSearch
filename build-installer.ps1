# Build ModlandSearch and its Windows installer in one step.
# Usage:  powershell -ExecutionPolicy Bypass -File build-installer.ps1
#
# The libopenmpt DLLs are not tracked in git. If they are missing this script runs
# scripts/fetch-libopenmpt.sh (the same fetch build.sh uses), or stops with an
# explanation when bash is not available.

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

# The DLLs that have to sit next to the exe (installer.iss copies them from here too).
$dllNames = @('libopenmpt.dll', 'openmpt-mpg123.dll', 'openmpt-ogg.dll', 'openmpt-vorbis.dll', 'openmpt-zlib.dll')
$dllDir = Join-Path $PSScriptRoot 'third_party\libopenmpt\bin\amd64'
$missing = @($dllNames | Where-Object { -not (Test-Path -LiteralPath (Join-Path $dllDir $_)) })
if ($missing.Count -gt 0) {
    $bash = Get-Command bash -ErrorAction SilentlyContinue
    if (-not $bash) {
        throw @"
The libopenmpt DLLs are missing from $dllDir and 'bash' was not found, so they cannot be
fetched automatically. They are not stored in git. Either:
  * run ./build.sh first (Git Bash or WSL) and then re-run this script, or
  * install Git for Windows (which provides bash) and run: bash scripts/fetch-libopenmpt.sh
"@
    }
    Write-Host '==> libopenmpt DLLs missing, fetching them (not stored in git)'
    & $bash.Source (Join-Path $PSScriptRoot 'scripts\fetch-libopenmpt.sh')
    if ($LASTEXITCODE -ne 0) {
        throw "scripts/fetch-libopenmpt.sh failed ($LASTEXITCODE) - run ./build.sh to see the full error."
    }
    $stillMissing = @($dllNames | Where-Object { -not (Test-Path -LiteralPath (Join-Path $dllDir $_)) })
    if ($stillMissing.Count -gt 0) {
        throw "Still missing after the fetch: $($stillMissing -join ', ') in $dllDir"
    }
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

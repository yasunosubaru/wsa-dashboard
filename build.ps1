# SPDX-FileCopyrightText: Copyright (c) 2026
# SPDX-License-Identifier: GPL-3.0-only
<#
.SYNOPSIS
  Single entry point for every build of this repo.

.DESCRIPTION
  Three problems this script owns, so no human has to remember them:

  1. SKIA_BINARIES_URL must be a TEMPLATE containing the literal placeholders
     {tag} and {key} -- skia-bindings' build script does a plain
     .replace("{tag}").replace("{key}") on it
     (build_support/binary_cache/binaries.rs:104-110). It is NOT a full URL.

  2. The scheme prefix must be exactly `file://` (TWO slashes).
     skia-bindings strips that prefix literally:
         url.strip_prefix("file://")      (build_support/binary_cache/utils.rs:18)
     so `file:///C:/...` (three slashes) yields the path `/C:/...` and the
     build dies with "os error 123".

  3. Capturing cargo's output. cargo writes progress to STDERR, so `2>&1` is
     required for streaming -- but PowerShell 5.1 turns each redirected native
     stderr line into an ErrorRecord, which terminates the script while
     $ErrorActionPreference is 'Stop'. The pipeline below therefore flips EAP to
     'Continue' for the duration and unwraps ErrorRecords to their raw message.
     (Start-Process file redirection was tried instead: it streams fine but
     returns a NULL ExitCode once the child has exited.)

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File build.ps1
  powershell -ExecutionPolicy Bypass -File build.ps1 -CargoArgs @('--bin','wsadash')
#>
[CmdletBinding()]
param(
    # Forwarded verbatim to cargo, e.g. @('--bin','wsadash'), @('--locked').
    [string[]]$CargoArgs = @(),

    # Directory holding prebuilt skia-binaries-*.tar.gz archives.
    [string]$SkiaCacheDir = "$env:LOCALAPPDATA\Temp\opencode\skia-cache\0.99.0"
)

$ErrorActionPreference = 'Stop'
$RepoRoot = $PSScriptRoot
$KeepLog  = Join-Path $env:TEMP 'wsadashboard-build.log'

# ---------------------------------------------------------------- Skia guard --

if (-not (Test-Path $SkiaCacheDir)) {
    Write-Host "FATAL: Skia binary cache not found:" -ForegroundColor Red
    Write-Host "  $SkiaCacheDir"
    Write-Host ""
    Write-Host "Download the archive for skia-bindings 0.99.0 and place it there:"
    Write-Host "  https://github.com/rust-skia/skia-binaries/releases/download/0.99.0/<name>.tar.gz"
    exit 1
}

$archives = @(Get-ChildItem -Path $SkiaCacheDir -Filter 'skia-binaries-*.tar.gz' -ErrorAction SilentlyContinue)
if ($archives.Count -eq 0) {
    Write-Host "FATAL: no skia-binaries-*.tar.gz in:" -ForegroundColor Red
    Write-Host "  $SkiaCacheDir"
    Write-Host ""
    Write-Host "Without it the build falls back to compiling Skia from source,"
    Write-Host "which fails here with 'Unable to locate LLVM installation'."
    exit 1
}

# NOTE: no third slash. `file://` + `C:/...` == `file://C:/...`
$cacheUrl = 'file://' + ($SkiaCacheDir -replace '\\', '/')
$env:SKIA_BINARIES_URL = "$cacheUrl/skia-binaries-{key}.tar.gz"

Write-Host "Skia cache    : $SkiaCacheDir ($($archives.Count) archive)"
Write-Host "Binaries URL  : $env:SKIA_BINARIES_URL"
Write-Host ""

# -------------------------------------------------------------------- build ---

$collected   = New-Object System.Collections.Generic.List[string]
$prevEAP     = $ErrorActionPreference
$exitCode    = 1

Push-Location $RepoRoot
try {
    # See .DESCRIPTION point 3: EAP must be Continue while 2>&1 is in play.
    $ErrorActionPreference = 'Continue'
    & cargo build --release @CargoArgs 2>&1 | ForEach-Object {
        $line = if ($_ -is [System.Management.Automation.ErrorRecord]) {
                    if ($null -ne $_.Exception) { $_.Exception.Message } else { $_.ToString() }
                } else {
                    $_.ToString()
                }
        if ($line) {
            $collected.Add($line)
            Write-Host $line
        }
    }
    $exitCode = $LASTEXITCODE
    if ($null -eq $exitCode) { $exitCode = 1 }
}
finally {
    $ErrorActionPreference = $prevEAP
    Pop-Location
}

$collected | Set-Content -Path $KeepLog -Encoding UTF8

# --------------------------------------------------------------- diagnostics --

if ($exitCode -ne 0) {
    # skia-bindings prints its chosen download URL on build-script stdout, which
    # cargo only replays under -vv; the source-build fallback is easier to spot.
    if ($collected -match 'Unable to locate LLVM installation') {
        Write-Host ""
        Write-Host "Skia tried to compile from source, which needs LLVM." -ForegroundColor Yellow
        Write-Host "That means the prebuilt archive in the cache was not accepted."
        Write-Host "Find the expected archive name with:"
        Write-Host "  cargo build --release -vv 2>&1 | findstr /i SKIA_BINARIES"
    }
    Write-Host ""
    Write-Host "BUILD FAILED (cargo exit $exitCode)." -ForegroundColor Red
    Write-Host "Full log: $KeepLog"
}

exit $exitCode

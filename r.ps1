#!/usr/bin/env pwsh
# r.ps1 - Rebuild SFX Root (release) and deploy the exe to the run location.
#
# Usage:  ./r.ps1
#
# Builds the frontend + release binary (skips installers), closes the app if
# it's running, then copies the exe over the one you launch from.

$ErrorActionPreference = 'Stop'

$RepoRoot = $PSScriptRoot
$SrcExe   = Join-Path $RepoRoot 'src-tauri\target\release\sfxroot.exe'
$DestExe  = 'C:\Users\Nitropc\stuff\software\sfxroot.exe'
$DestDir  = Split-Path $DestExe -Parent

Push-Location $RepoRoot
try {
    # 1. Ensure frontend dependencies are installed and in sync with the lockfile
    Write-Host '==> Installing dependencies (pnpm)...' -ForegroundColor Cyan
    pnpm install
    if ($LASTEXITCODE -ne 0) { throw "pnpm install failed (exit $LASTEXITCODE)" }

    # 2. Build frontend + release exe (skip MSI/NSIS bundling - we run the bare exe)
    Write-Host '==> Building release (no bundle)...' -ForegroundColor Cyan
    pnpm tauri build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed (exit $LASTEXITCODE)" }

    if (-not (Test-Path $SrcExe)) { throw "Built exe not found at $SrcExe" }

    # 3. Close the running app so its exe isn't locked
    $running = Get-Process -Name 'sfxroot' -ErrorAction SilentlyContinue
    if ($running) {
        Write-Host '==> Closing running SFX Root...' -ForegroundColor Yellow
        $running | Stop-Process -Force
        $running | Wait-Process -Timeout 10 -ErrorAction SilentlyContinue
    }

    # 4. Deploy
    if (-not (Test-Path $DestDir)) {
        New-Item -ItemType Directory -Path $DestDir -Force | Out-Null
    }
    Copy-Item -Path $SrcExe -Destination $DestExe -Force

    $size = [math]::Round((Get-Item $DestExe).Length / 1MB, 1)
    Write-Host "==> Deployed $size MB -> $DestExe" -ForegroundColor Green
}
finally {
    Pop-Location
}

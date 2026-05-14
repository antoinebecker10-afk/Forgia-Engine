# Forgia V2 — Debug launcher (PowerShell version)
# Usage : Right-click → Run with PowerShell, OU `.\run_debug.ps1` in pwsh

Set-Location $PSScriptRoot

# Backup
if (Test-Path forgia2_run.log) {
    Copy-Item -Force forgia2_run.log forgia2_run.log.previous
}

# Env debug
$env:RUST_BACKTRACE = "full"
$env:RUST_LOG = "forgia_game=debug,forgia_player=debug,forgia_ui=debug,forgia_combat=debug,forgia_effects=info,forgia_core=info,bevy_app=info,bevy_render=warn,wgpu=warn,naga=warn,bevy_winit=info"

Write-Host "[run_debug] Starting Forgia V2..." -ForegroundColor Cyan
Write-Host "[run_debug] Log    : forgia2_run.log" -ForegroundColor Gray
Write-Host "[run_debug] Backtrace : full" -ForegroundColor Gray
Write-Host "[run_debug] ESC pour quitter, Alt+F4 si menu mort" -ForegroundColor Yellow
Write-Host ""

# Run with full output capture
& "target\release-fast\forgia-game.exe" *> forgia2_run.log
$exitCode = $LASTEXITCODE

Write-Host ""
Write-Host "[run_debug] Forgia V2 closed (exit $exitCode)" -ForegroundColor Cyan
Write-Host "[run_debug] Log saved : forgia2_run.log" -ForegroundColor Green
Write-Host "[run_debug] Previous  : forgia2_run.log.previous" -ForegroundColor Gray

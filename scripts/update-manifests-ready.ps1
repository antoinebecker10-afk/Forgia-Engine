# Mark manifests as 'wip' or 'ready' for crates we just implemented.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$crates = Join-Path $root "crates"

$ready = @(
    "forgia-damage",
    "forgia-weapon-hitscan",
    "forgia-ai-arena-bot",
    "forgia-juice-camera-shake",
    "forgia-vfx-tracers",
    "forgia-damage-numbers"
)

foreach ($c in $ready) {
    $m = Join-Path $crates "$c/manifest.toml"
    if (-not (Test-Path $m)) { continue }
    $content = Get-Content $m -Raw
    $content = $content -replace 'status = "stub"', 'status = "ready"'
    Set-Content -Path $m -Value $content -Encoding UTF8
    Write-Host "[ready] $c"
}

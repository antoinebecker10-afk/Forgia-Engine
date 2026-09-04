$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$crates = Join-Path $root "crates"

$ready = @(
    "forgia-rpg",
    "forgia-inventory",
    "forgia-quests",
    "forgia-dialogue",
    "forgia-xp-curves"
)

foreach ($c in $ready) {
    $m = Join-Path $crates "$c/manifest.toml"
    if (-not (Test-Path $m)) { continue }
    $content = Get-Content $m -Raw
    $content = $content -replace 'status = "stub"', 'status = "ready"'
    Set-Content -Path $m -Value $content -Encoding UTF8
    Write-Host "[ready] $c"
}

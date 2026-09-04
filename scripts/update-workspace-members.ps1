# Update [workspace] members in root Cargo.toml to include all crates/ subdirs.
# Idempotent.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cargoToml = Join-Path $root "Cargo.toml"
$cratesDir = Join-Path $root "crates"

$members = Get-ChildItem -Path $cratesDir -Directory | Sort-Object Name | ForEach-Object { "crates/$($_.Name)" }
$members += "xtask"

$membersBlock = ($members | ForEach-Object { "    `"$_`"," }) -join "`n"

$content = Get-Content $cargoToml -Raw

$pattern = '(?s)members = \[.*?\]'
$replacement = "members = [`n$membersBlock`n]"

$new = [regex]::Replace($content, $pattern, $replacement)
Set-Content -Path $cargoToml -Value $new -Encoding UTF8 -NoNewline

Write-Host "Workspace members updated : $($members.Count) entries." -ForegroundColor Green

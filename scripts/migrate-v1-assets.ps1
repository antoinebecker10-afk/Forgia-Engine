# Migration V1 → V2 assets.
# Strategy :
#   - Junctions (zero-copy) for BIG dirs : models, textures, hdri
#   - Copy for small : audio, ui, shaders, environment_maps, icon, ai_generated, bundles
#   - Skip : imported_assets, .freestylized_tmp, datasets, _manifest_v2.json (will rebuild)
#   - Genomes : full copy with category dispatch
# Idempotent.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$v2 = Join-Path $root "assets"
$v1 = "D:\Forgia\RUST\Forgia\Forgia\assets"
$v1Cfg = "D:\Forgia\RUST\Forgia\Forgia\config\genomes"

Write-Host "Forgia V2 — Migrate V1 assets" -ForegroundColor Cyan
Write-Host "Source : $v1" -ForegroundColor Gray
Write-Host "Target : $v2" -ForegroundColor Gray
Write-Host ""

# ─────────────────────────────────────────────────────────────────────────────
# 1. Junctions for big dirs
# ─────────────────────────────────────────────────────────────────────────────
$junctions = @(
    @{n="models-v1";   s="$v1\models"},
    @{n="textures-v1"; s="$v1\textures"},
    @{n="hdri-v1";     s="$v1\hdri"}
)

foreach ($j in $junctions) {
    $dst = Join-Path $v2 $j.n
    if (Test-Path $dst) {
        Write-Host "[skip junction] $($j.n) (exists)" -ForegroundColor DarkGray
        continue
    }
    if (-not (Test-Path $j.s)) {
        Write-Host "[miss] source not found : $($j.s)" -ForegroundColor Yellow
        continue
    }
    cmd /c mklink /J "$dst" "$($j.s)" | Out-Null
    Write-Host "[junction] $($j.n) -> $($j.s)" -ForegroundColor Green
}

# ─────────────────────────────────────────────────────────────────────────────
# 2. Copy small dirs (selective)
# ─────────────────────────────────────────────────────────────────────────────
$copyDirs = @(
    @{src="audio";            dst="audio-v1"},
    @{src="ui";               dst="textures/ui-v1"},
    @{src="environment_maps"; dst="hdri/env-maps-v1"},
    @{src="icon";             dst="icons/v1"},
    @{src="ai_generated";     dst="models/ai-generated-v1"},
    @{src="bundles";          dst="bundles-v1"},
    @{src="shaders";          dst="shaders/v1-port"}
)

foreach ($c in $copyDirs) {
    $s = Join-Path $v1 $c.src
    $d = Join-Path $v2 $c.dst
    if (-not (Test-Path $s)) {
        Write-Host "[miss] $($c.src) not found in V1" -ForegroundColor Yellow
        continue
    }
    if (Test-Path $d) {
        Write-Host "[skip copy] $($c.dst) (exists)" -ForegroundColor DarkGray
        continue
    }
    Write-Host "[copy] $($c.src) -> $($c.dst) ..." -ForegroundColor Green -NoNewline
    $parent = Split-Path $d -Parent
    if (-not (Test-Path $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    robocopy "$s" "$d" /E /NFL /NDL /NJH /NJS /NC /NS /NP | Out-Null
    Write-Host " done" -ForegroundColor Green
}

# ─────────────────────────────────────────────────────────────────────────────
# 3. Genomes : copy with category dispatch
# ─────────────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Migrating genomes" -ForegroundColor Cyan

if (-not (Test-Path $v1Cfg)) {
    Write-Host "[miss] $v1Cfg" -ForegroundColor Yellow
} else {
    # Category mapping from prefix
    $catMap = @{
        "weapon_"      = "weapons"
        "biome_"       = "biomes"
        "enemy_"       = "enemies"
        "ai_"          = "ai"
        "arena_bot"    = "ai"
        "atmosphere"   = "atmosphere"
        "biome_audio"  = "audio"
        "audio"        = "audio"
        "damage_feedback" = "damage_feedback"
        "debug_monitor"= "debug_monitor"
        "economy_global"= "economy_global"
        "economy"      = "economy"
        "industry"     = "industry"
        "input"        = "input"
        "map_gen"      = "map_gen"
        "particles"    = "particles"
        "biome_particles" = "particles"
        "perfs"        = "perfs"
        "sky"          = "sky"
        "steam"        = "steam"
        "quality"      = "quality"
        "ui"           = "ui"
        "vfx"          = "vfx"
        "weapon_vfx"   = "vfx"
    }

    $migrated = 0
    $unmapped = @()
    Get-ChildItem $v1Cfg -Filter "*.toml" -File | ForEach-Object {
        $name = $_.Name
        $cat = $null
        # Find longest-prefix match
        foreach ($k in ($catMap.Keys | Sort-Object Length -Descending)) {
            if ($name.StartsWith($k)) { $cat = $catMap[$k]; break }
        }
        if (-not $cat) {
            # Fallback : misc
            $cat = "_misc"
            $unmapped += $name
        }
        $targetDir = Join-Path $v2 "genomes/$cat"
        if (-not (Test-Path $targetDir)) { New-Item -ItemType Directory -Path $targetDir -Force | Out-Null }
        $target = Join-Path $targetDir $name
        if (-not (Test-Path $target)) {
            Copy-Item $_.FullName $target
            $migrated++
        }
    }

    # Also copy _meta if exists
    $metaSrc = Join-Path $v1Cfg "_meta"
    if (Test-Path $metaSrc) {
        $metaDst = Join-Path $v2 "genomes/_meta"
        if (-not (Test-Path $metaDst)) {
            robocopy "$metaSrc" "$metaDst" /E /NFL /NDL /NJH /NJS /NC /NS /NP | Out-Null
        }
    }

    Write-Host "[genomes] migrated : $migrated TOML files" -ForegroundColor Green
    if ($unmapped.Count -gt 0) {
        Write-Host "[genomes] unmapped (-> _misc) : $($unmapped.Count) files" -ForegroundColor Yellow
        $unmapped | Select-Object -First 10 | ForEach-Object { Write-Host "    $_" -ForegroundColor DarkYellow }
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# 4. Scripts (Luau) — copy V1
# ─────────────────────────────────────────────────────────────────────────────
$v1Scripts = Join-Path $v1 "scripts"
if (Test-Path $v1Scripts) {
    $count = 0
    Get-ChildItem $v1Scripts -File -Recurse | ForEach-Object {
        $rel = $_.FullName.Substring($v1Scripts.Length + 1)
        $dst = Join-Path $v2 "scripts/v1/$rel"
        $dstDir = Split-Path $dst -Parent
        if (-not (Test-Path $dstDir)) { New-Item -ItemType Directory -Path $dstDir -Force | Out-Null }
        if (-not (Test-Path $dst)) {
            Copy-Item $_.FullName $dst
            $count++
        }
    }
    Write-Host "[scripts] $count Luau files copied to scripts/v1/" -ForegroundColor Green
}

Write-Host ""
Write-Host "Migration done." -ForegroundColor Cyan

# build-dist.ps1 — Pipeline de build distribuable Roguelite v0.1 (zip portable itch.io).
#
# Produit dist/forgia-roguelite-<Version>/ (forgia.exe + assets/ whitelistés) + .zip.
# Cible : zip portable, décompressable et jouable direct (cwd = dossier). Zéro coût.
#
# Usage :
#   pwsh scripts/build-dist.ps1                 # build release + assemble + zip
#   pwsh scripts/build-dist.ps1 -SkipBuild      # ré-assemble sans rebuild (itère la denylist)
#   pwsh scripts/build-dist.ps1 -DryRun         # mesure la taille assets post-denylist, ne build/copie rien
#   pwsh scripts/build-dist.ps1 -NoZip          # dossier sans zip
#
# NOTE portabilité : les genomes se lisent en chemin RELATIF (fs::read "assets/...")
# et les capteurs écrivent dans le cwd → lancer forgia.exe DEPUIS son dossier
# (double-clic / itch = cwd correct). Le LISEZMOI le rappelle.
param(
    [string]$Version = "v0.1",
    [switch]$SkipBuild,
    [switch]$DryRun,
    [switch]$NoZip
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$distName = "forgia-roguelite-$Version"
$distDir = Join-Path $repo "dist\$distName"
$zipPath = Join-Path $repo "dist\$distName.zip"
$srcAssets = Join-Path $repo "assets"

# ── Denylist : assets NON utilisés par le Roguelite (preuves dans le code) ───────
#   cyberpunk_city.glb : démo CyberCity, gated OnEnter(CyberCity)         (~185 MB)
#   ai-generated-v1    : V1 AI-gen, non référencé roguelite               (~58 MB)
#   textures-v1        : PBR terrain RPG (forgia-rpg/terrain), arène=sol plat (~214 MB)
#   (audio-v1 GARDÉ : le roguelite y prend musique+jingles ; trim fin = P2)
$assetDeny = @(
    "models\environment\cyberpunk_city.glb",
    "models\ai-generated-v1",
    "textures-v1"
)

function Measure-DirMB($path) {
    if (-not (Test-Path $path)) { return 0.0 }
    $s = (Get-ChildItem $path -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
    return [math]::Round($s / 1MB, 1)
}

# ── Dry-run : taille assets après denylist, sans rien construire ─────────────────
$totalAssets = Measure-DirMB $srcAssets
$denied = 0.0
Write-Host "[dist] === Denylist ==="
foreach ($d in $assetDeny) {
    $mb = Measure-DirMB (Join-Path $srcAssets $d)
    $denied += $mb
    Write-Host ("  - {0,8:N1} MB  {1}" -f $mb, $d)
}
Write-Host ("[dist] assets total={0:N0} MB · retirés={1:N0} MB · embarqués≈{2:N0} MB" -f $totalAssets, $denied, ($totalAssets - $denied))
if ($DryRun) { Write-Host "[dist] DryRun — stop."; return }

# ── 1. Build release ────────────────────────────────────────────────────────────
if (-not $SkipBuild) {
    Write-Host "[dist] cargo build -p forgia --release -j 4 (long : LTO + codegen-units=1)..."
    cargo build -p forgia --release -j 4
    if ($LASTEXITCODE -ne 0) { throw "cargo build a échoué ($LASTEXITCODE)" }
}
$exe = Join-Path $repo "target\release\forgia.exe"
if (-not (Test-Path $exe)) { throw "exe introuvable: $exe — build d'abord (sans -SkipBuild)" }

# ── 2. Reset dossier dist ─────────────────────────────────────────────────────────
if (Test-Path $distDir) { Remove-Item $distDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $distDir | Out-Null

# ── 3. Exe ────────────────────────────────────────────────────────────────────────
Copy-Item $exe (Join-Path $distDir "forgia.exe")

# ── 4. Assets moins denylist (robocopy /E + /XD dirs + /XF fichiers) ──────────────
$dstAssets = Join-Path $distDir "assets"
$rc = @($srcAssets, $dstAssets, "/E", "/NFL", "/NDL", "/NJH", "/NJS", "/NP", "/R:1", "/W:1")
foreach ($d in $assetDeny) {
    $full = Join-Path $srcAssets $d
    if (Test-Path $full -PathType Container) { $rc += "/XD"; $rc += $full }
    else { $rc += "/XF"; $rc += $full }
}
robocopy @rc | Out-Null
if ($LASTEXITCODE -ge 8) { throw "robocopy a échoué (code $LASTEXITCODE)" }
$global:LASTEXITCODE = 0  # robocopy 0-7 = succès, ne pas polluer

# ── 5. LISEZMOI joueur ────────────────────────────────────────────────────────────
@"
Forgia — Roguelite $Version

LANCEMENT : double-clique forgia.exe (garde le dossier "assets" juste à côté).
Le jeu écrit ses fichiers de diagnostic (forgia2_*.json) dans ce dossier — normal.

Contrôles : ZQSD déplacement · souris viser · clic tirer · Shift dash · ESC menu.
"@ | Set-Content -Encoding UTF8 (Join-Path $distDir "LISEZMOI.txt")

# ── 6. Taille + zip ───────────────────────────────────────────────────────────────
$folderMB = Measure-DirMB $distDir
Write-Host ("[dist] dossier prêt : {0:N0} MB — {1}" -f $folderMB, $distDir)
if (-not $NoZip) {
    if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
    Write-Host "[dist] compression (peut prendre 1-2 min)..."
    Compress-Archive -Path (Join-Path $distDir "*") -DestinationPath $zipPath
    $zipMB = [math]::Round((Get-Item $zipPath).Length / 1MB, 1)
    Write-Host ("[dist] ZIP prêt : {0:N0} MB — {1}" -f $zipMB, $zipPath)
}
Write-Host "[dist] OK. Teste : décompresse ailleurs, lance forgia.exe."

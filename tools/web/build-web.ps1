# build-web.ps1 — Forgia : build + service de la version web (story-695).
# Se lance depuis n'importe ou : le script se cale sur la racine du repo.
#
# Usage :
#   tools\web\build-web.ps1              # build + bindgen + assets + serveur + tunnel
#   tools\web\build-web.ps1 -Opt         # + wasm-opt -Oz (~108 -> ~63 MB, publication/telephone)
#   tools\web\build-web.ps1 -ServeOnly   # (re)demarre serveur + tunnel sans rebuild
#
# Prerequis one-shot :
#   rustup target add wasm32-unknown-unknown
#   wasm-bindgen-cli 0.2.121 + binaryen (wasm-opt) + cloudflared dans $ToolsDir
#   (versions : wasm-bindgen DOIT matcher la version du Cargo.lock)
#
# Sortie : dossier web-demo/ a la racine du repo (gitignore), servi sur :8907
# et expose en https par un quick tunnel cloudflared (URL affichee).
# Publication perenne : push du contenu de web-demo/ vers le depot
# antoinebecker10-afk.github.io (GitHub Pages auto).

param(
    [switch]$Opt,
    [switch]$ServeOnly
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Demo = "$Root\web-demo"
$ToolsDir = if ($env:FORGIA_WEBTOOLS) { $env:FORGIA_WEBTOOLS } else {
    "C:\Users\Antoi\AppData\Local\Temp\claude\c--Users-Antoi-Desktop-Forgia-Rewrite\035c40e1-6424-4153-b5ab-926a5446ab6a\scratchpad\webtools"
}
$Port = 8907

if (-not $ServeOnly) {
    Write-Host "[1/4] cargo build wasm release..." -ForegroundColor Cyan
    Set-Location $Root
    cargo build --release --target wasm32-unknown-unknown -p forgia
    if ($LASTEXITCODE -ne 0) { throw "build wasm en echec" }

    Write-Host "[2/4] wasm-bindgen..." -ForegroundColor Cyan
    New-Item -ItemType Directory -Force $Demo | Out-Null
    & "$ToolsDir\wasm-bindgen-0.2.121-x86_64-pc-windows-msvc\wasm-bindgen.exe" `
        --target web --out-dir $Demo --out-name forgia `
        "$Root\target\wasm32-unknown-unknown\release\forgia.wasm"
    Remove-Item "$Demo\*.d.ts" -ErrorAction SilentlyContinue

    if ($Opt) {
        Write-Host "[2b] wasm-opt -Oz (long)..." -ForegroundColor Cyan
        & "$ToolsDir\binaryen-version_119\bin\wasm-opt.exe" -Oz --strip-debug --strip-producers `
            -o "$Demo\forgia_bg.opt.wasm" "$Demo\forgia_bg.wasm"
        Move-Item "$Demo\forgia_bg.opt.wasm" "$Demo\forgia_bg.wasm" -Force
    }

    Write-Host "[3/4] sync assets references par le code ET les genomes..." -ForegroundColor Cyan
    # Inc.5 (story-695) remplacera ce grep par un manifeste declare + valide.
    # Lecon du 2026-08-11 : les GLB d'arene/stage vivent dans les TOML, pas les .rs
    # — les oublier = Hall vide + ecran noir in-game chez les testeurs.
    $refs = Get-ChildItem "$Root\crates" -Recurse -Filter *.rs |
        Select-String -Pattern '"([^"]+\.(glb|gltf|png|webp|jpg|ogg|wav|mp3|ktx2|basis|hdr))"' -AllMatches |
        ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value }
    $genomeFiles = @()
    $genomeFiles += Get-ChildItem "$Root\assets\genomes\roguelite" -Recurse -Filter *.toml -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes" -Filter "roguelite_*.toml" -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes" -Filter "arena*.toml" -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes" -Filter "level_modules.toml" -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes" -Filter "viewmodel*.toml" -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes" -Filter "castle*.toml" -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes\weapons" -Recurse -Filter *.toml -ErrorAction SilentlyContinue
    $genomeFiles += Get-ChildItem "$Root\assets\genomes\enemies" -Recurse -Filter *.toml -ErrorAction SilentlyContinue
    $refs += $genomeFiles |
        Select-String -Pattern '"([^"]+\.(glb|gltf|png|webp|jpg|ogg|wav|mp3|ktx2|basis|hdr))"' -AllMatches |
        ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value }
    $refs = $refs | Sort-Object -Unique
    $n = 0
    foreach ($r in $refs) {
        $src = "$Root\assets\$r"
        if (Test-Path $src) {
            $dst = "$Demo\assets\$r"
            New-Item -ItemType Directory -Force (Split-Path $dst -Parent) | Out-Null
            Copy-Item $src $dst -Force
            $n++
            # .gltf (JSON) = buffers .bin + textures EXTERNES : copier le dossier
            # entier, sinon le modele est vide (avatar invisible, 2026-08-11).
            if ($r -match '\.gltf$') {
                Copy-Item (Split-Path $src -Parent) (Split-Path (Split-Path $dst -Parent) -Parent) -Recurse -Force
            }
        }
    }
    Copy-Item "$Root\assets\genomes" "$Demo\assets\" -Recurse -Force
    Write-Host "  $n assets + genomes synchronises"

    if (-not (Test-Path "$Demo\index.html")) {
        Copy-Item "$Root\tools\web\index.html" "$Demo\index.html"
    }
}

Write-Host "[4/4] serveur + tunnel..." -ForegroundColor Cyan
if (-not (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)) {
    Start-Process -WindowStyle Hidden python -ArgumentList "-m", "http.server", "$Port" -WorkingDirectory $Demo
    Start-Sleep 2
}
if (-not (Get-Process cloudflared -ErrorAction SilentlyContinue)) {
    $log = "$env:TEMP\forgia-tunnel.log"
    Remove-Item $log -ErrorAction SilentlyContinue
    Start-Process -WindowStyle Hidden "$ToolsDir\cloudflared.exe" -ArgumentList "tunnel", "--url", "http://localhost:$Port", "--logfile", $log
    Start-Sleep 12
    $url = (Select-String -Path $log -Pattern "https://[a-z0-9-]+\.trycloudflare\.com" | Select-Object -First 1).Matches.Value
    Write-Host "TUNNEL : $url" -ForegroundColor Green
} else {
    Write-Host "tunnel deja actif"
}
Write-Host "LOCAL  : http://localhost:$Port" -ForegroundColor Green

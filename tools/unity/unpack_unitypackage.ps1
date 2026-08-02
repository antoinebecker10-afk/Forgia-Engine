# unpack_unitypackage.ps1 — Restitue l'arborescence `Assets/` d'un .unitypackage.
#
# Un .unitypackage est un TAR GZIP : un dossier par GUID contenant `asset`,
# `asset.meta` et `pathname`. Le nom d'origine du fichier n'est écrit QUE dans
# `pathname` — extraire l'archive telle quelle donne des dossiers de GUID
# illisibles. Ce script recolle chaque `asset` à son chemin déclaré.
#
# Usage :
#   ./tools/unity/unpack_unitypackage.ps1 -Pkg "D:\pack.unitypackage" -Out ".\out"

param(
    [Parameter(Mandatory = $true)][string]$Pkg,
    [Parameter(Mandatory = $true)][string]$Out
)

$tmp = Join-Path $env:TEMP ("upk_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $tmp | Out-Null
tar -xzf $Pkg -C $tmp 2>$null

New-Item -ItemType Directory -Force $Out | Out-Null
Get-ChildItem $tmp -Directory | ForEach-Object {
    $pathnameFile = Join-Path $_.FullName "pathname"
    $assetFile = Join-Path $_.FullName "asset"
    # Un dossier sans `asset` décrit un répertoire, pas un fichier : on l'ignore.
    if ((Test-Path $pathnameFile) -and (Test-Path $assetFile)) {
        $relative = (Get-Content $pathnameFile -Raw).Trim() -replace '/', '\'
        $destination = Join-Path $Out $relative
        New-Item -ItemType Directory -Force (Split-Path $destination) | Out-Null
        Copy-Item $assetFile $destination -Force
        "{0}  <-  {1}" -f $relative, $_.Name
    }
}
Remove-Item $tmp -Recurse -Force

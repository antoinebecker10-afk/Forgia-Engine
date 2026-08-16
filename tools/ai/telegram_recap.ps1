#Requires -Version 5.1
<#
.SYNOPSIS
  Recap Forgia sur Telegram a l'ouverture d'une session : etat du chantier + veille jamais envoyee.

.DESCRIPTION
  Se greffe sur le bot @ForgierBot DEJA en place : memes secrets DPAPI
  (`$HOME\.forgia\veille\{token,chat_id}.dpapi`), meme garde de destinataire.
  Aucun second bot, aucun second jeton a gerer.

  La regle qui compte est la regle de SILENCE. Un recap qui part a chaque
  ouverture de terminal devient du bruit, et du bruit se desactive au bout
  d'une semaine — donc il n'envoie que s'il a quelque chose de neuf a dire :

    - des entrees de veille jamais poussees (dedupe par `veille_registre.py`), OU
    - des commits depuis le dernier envoi, OU
    - une alerte capteur qui n'etait pas la au dernier envoi.

  Sinon il se tait, et rend 0. Un delai plancher (-MinHeures, 4 h par defaut)
  couvre le cas « j'ouvre cinq terminaux d'affilee ».

.PARAMETER DryRun
  Compose et affiche le message, n'envoie rien, ne marque rien.

.PARAMETER Force
  Ignore la regle de silence et le delai plancher.

.EXAMPLE
  pwsh -File tools/ai/telegram_recap.ps1 -DryRun
  pwsh -File tools/ai/telegram_recap.ps1
#>
[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$Force,
    [int]$MinHeures = 4,
    [string]$Racine = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Le journal est relu par le hook (bash) ET par un humain. Sans cette ligne, la
# sortie part dans la page de code console et `tail` refuse le fichier.
try { [Console]::OutputEncoding = New-Object Text.UTF8Encoding $false } catch { }

$VeilleDir  = Join-Path $HOME '.forgia\veille'
$TokenFile  = Join-Path $VeilleDir 'token.dpapi'
$ChatIdFile = Join-Path $VeilleDir 'chat_id.dpapi'
$EtatFile   = Join-Path $Racine '.claude\veille-pousse.json'
$RegistrePy = Join-Path $Racine 'tools\ai\veille_registre.py'

function Get-Python {
    foreach ($c in @('python', 'python3', 'py')) {
        $cmd = Get-Command $c -ErrorAction SilentlyContinue
        if ($cmd) { return $cmd.Source }
    }
    return $null
}

function Read-DpapiSecret {
    param([string]$Path)
    if (-not (Test-Path $Path)) { return $null }
    try {
        $sec  = ConvertTo-SecureString ((Get-Content $Path -Raw).Trim())
        $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
        try   { return [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr) }
        finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
    } catch { return $null }
}

function ConvertTo-TgHtml {
    param([string]$s)
    if ($null -eq $s) { return '' }
    return $s.Replace('&', '&amp;').Replace('<', '&lt;').Replace('>', '&gt;')
}

function Get-Node {
    $c = Get-Command node -ErrorAction SilentlyContinue
    if ($c) { return $c.Source } else { return $null }
}

function New-Jauge {
    # Telegram n'a pas de barres : on en dessine une en caracteres, 10 crans.
    param([int]$Pct)
    $pleins = [Math]::Round($Pct / 10.0)
    if ($pleins -lt 0) { $pleins = 0 }; if ($pleins -gt 10) { $pleins = 10 }
    return ('█' * $pleins) + ('░' * (10 - $pleins))
}

function New-MessageEtabli {
    <#
      Message SEPARE : l'image fixe du projet (moteur + jeu), pas le delta de session.
      Tous les chiffres sont DERIVES de `docs/etabli/etabli-forgia.html` via
      `etabli_etat.mjs` — aucun n'est ecrit ici. Corriger l'Etabli suffit.
    #>
    param($E)
    $L = New-Object System.Collections.Generic.List[string]
    $L.Add("<b>📊 ÉTABLI FORGIA</b> · $(Get-Date -Format 'dd/MM')")

    $L.Add('')
    $L.Add("<b>⚙️ LE MOTEUR — $($E.moteur.pct) %</b>")
    $L.Add("<code>$(New-Jauge $E.moteur.pct)</code>")
    $L.Add("$($E.moteur.prod) en production · $($E.moteur.part) partielles · <b>$($E.moteur.abs) absentes</b> sur $($E.moteur.total)")
    $L.Add("<i>absentes : $(ConvertTo-TgHtml (($E.moteur.absentes) -join ', '))</i>")
    $L.Add('')
    $L.Add('<u>Chantiers moteur</u>')
    foreach ($c in ($E.moteur.chantiers | Sort-Object -Property p -Descending)) {
        $L.Add("· <b>$($c.p) %</b> $(ConvertTo-TgHtml $c.t) — <i>$(ConvertTo-TgHtml $c.gate)</i>")
    }

    $L.Add('')
    $L.Add("<b>🎮 LE JEU — $($E.jeu.pct) %</b>")
    $L.Add("<code>$(New-Jauge $E.jeu.pct)</code>")
    $L.Add("$($E.jeu.prod) systèmes en production · $($E.jeu.part) partiels · <b>$($E.jeu.abs) absents</b> sur $($E.jeu.total)")
    $L.Add("<i>absents : $(ConvertTo-TgHtml (($E.jeu.absents) -join ', '))</i>")
    $L.Add('')
    $L.Add('<u>Phases vers la v1</u>')
    foreach ($p in $E.jeu.phases) {
        $L.Add("· <b>$($p.p) %</b> ph.$($p.n) $(ConvertTo-TgHtml $p.t)")
    }

    $L.Add('')
    $L.Add("<b>🧾 DETTE — $($E.dette.ouverte) ouvertes</b> ($($E.dette.soldee) soldées)")
    $L.Add("risque haut $($E.dette.haut) · moyen $($E.dette.moyen) · bas $($E.dette.bas)")
    foreach ($d in ($E.dette.top | Select-Object -First 4)) { $L.Add("· $(ConvertTo-TgHtml $d)") }

    $L.Add('')
    $L.Add('<b>🛫 EN VOL</b>')
    foreach ($w in $E.wip) { $L.Add("· <code>$($w.s)</code> story $($w.id) — $(ConvertTo-TgHtml $w.t)") }

    return ($L -join "`n")
}

function Invoke-Git {
    # NE PAS nommer ce parametre $Args : c'est une variable automatique PowerShell,
    # et la fonction recevait alors ses propres arguments ecrases (git repondait
    # par sa page d'aide, comptee ensuite comme 46 fichiers modifies).
    param([string[]]$GitArgs)
    try { return (& git -C $Racine @GitArgs 2>$null) } catch { return $null }
}

# ══════════════════ etat precedent ══════════════════
$etat = @{ ids = @(); dernier_envoi = $null; dernier_sha = $null; alertes = @()
           hash_etabli = $null; dernier_etabli = $null }
if (Test-Path $EtatFile) {
    try {
        $j = Get-Content $EtatFile -Raw | ConvertFrom-Json
        foreach ($k in @('ids', 'dernier_envoi', 'dernier_sha', 'alertes', 'hash_etabli', 'dernier_etabli')) {
            if ($j.PSObject.Properties.Name -contains $k -and $null -ne $j.$k) { $etat[$k] = $j.$k }
        }
    } catch { }
}

# ══════════════════ 1. le chantier ══════════════════
$sha     = (Invoke-Git @('rev-parse', '--short', 'HEAD'))
$branche = (Invoke-Git @('rev-parse', '--abbrev-ref', 'HEAD'))
$sales   = @(Invoke-Git @('status', '--porcelain')).Count

$nouveauxCommits = @()
if ($etat.dernier_sha) {
    $r = Invoke-Git @('log', "$($etat.dernier_sha)..HEAD", '--pretty=format:%h|%s')
    if ($r) { $nouveauxCommits = @($r) }
}

# Stories ouvertes. `_index.md` est genere par xtask et peut etre perime (il
# l'etait le 12/08) : on lit les fichiers.
#
# Deux formats coexistent et il FAUT reconnaitre les deux — un parseur qui n'en
# connait qu'un a compte 13 stories ouvertes sur 3 :
#   1. `**Statut** : DONE (2026-08-12) — ...`
#   2. la banniere posee par la purge : `> ⛔ **CANCELLED 2026-08-12 — ...`
# D'ou : premier mot-cle de statut rencontre dans l'en-tete, quel que soit sa forme.
# Les statuts herites comptent AUSSI : `CODE COMPLETE` et `EN COURS` sont du
# travail ouvert, et la ROADMAP demande explicitement de les mapper sur
# REVIEW / IN_PROGRESS. Les omettre rendait 3 stories muettes.
$MOTS_STATUT = 'CANCELLED|DONE|IN[_ ]PROGRESS|CODE[- ]COMPLETE|REVIEW|READY|DRAFT|BLOCKED|TODO|EN COURS'
$ouvertes = 0
$sansStatut = 0
$dirStories = Join-Path $Racine 'docs\stories'
if (Test-Path $dirStories) {
    foreach ($f in Get-ChildItem $dirStories -Filter 'story-*.md' -File) {
        $tete = (Get-Content $f.FullName -TotalCount 15 -ErrorAction SilentlyContinue) -join "`n"
        $m = [regex]::Match($tete, "\b($MOTS_STATUT)\b", 'IgnoreCase')
        if (-not $m.Success) { $sansStatut++ }          # « 0 mesure » n'est pas « 0 ouverte »
        elseif ($m.Groups[1].Value.ToUpper() -notin @('DONE', 'CANCELLED')) { $ouvertes++ }
    }
}

# Capteurs — un capteur qui alerte doit se voir sur le telephone, pas seulement
# dans un JSON que personne n'ouvre.
$capteurs = @(Get-ChildItem $Racine -Filter 'forgia2_*.json' -File -ErrorAction SilentlyContinue)
$alertes = @()
foreach ($c in $capteurs) {
    $t = Get-Content $c.FullName -Raw -ErrorAction SilentlyContinue
    if ($t -and $t -match '"severity"\s*:\s*"(warn|warning|error|critical)"') {
        # `.previous` = instantane precedent du meme capteur, pas un capteur de plus.
        $alertes += (($c.BaseName -replace '^forgia2_', '') -replace '\.previous$', '')
    }
}
$alertes = @($alertes | Sort-Object -Unique)
$nouvellesAlertes = @($alertes | Where-Object { $_ -notin $etat.alertes })

# Binaire perime — dire « teste en jeu » sur un exe plus vieux que les sources
# est un faux diagnostic garanti.
$perime = 0
$exe = @(Get-ChildItem (Join-Path $Racine 'target') -Filter 'forgia.exe' -Recurse -File -ErrorAction SilentlyContinue |
         Sort-Object LastWriteTime -Descending | Select-Object -First 1)
if ($exe.Count -eq 1) {
    $perime = @(Get-ChildItem (Join-Path $Racine 'crates') -Filter '*.rs' -Recurse -File -ErrorAction SilentlyContinue |
                Where-Object { $_.LastWriteTime -gt $exe[0].LastWriteTime }).Count
}

# ══════════════════ 2. la veille jamais poussee ══════════════════
$neuf = @()
$totalRegistre = 0
$py = Get-Python
if ($py -and (Test-Path $RegistrePy)) {
    try {
        $brut = & $py $RegistrePy nouveau --json 2>$null
        if ($brut) {
            $parsed = ($brut -join "`n") | ConvertFrom-Json
            if ($null -ne $parsed) { $neuf = @($parsed) }
        }
        $st = & $py $RegistrePy stats 2>$null
        $ligne = $st | Select-String -Pattern '^entrees\s+(\d+)' | Select-Object -First 1
        if ($ligne) { $totalRegistre = [int]$ligne.Matches[0].Groups[1].Value }
    } catch { }
}

# ══════════════════ 2 bis. l'etat de l'Etabli (moteur + jeu) ══════════════════
# Derive de docs/etabli/etabli-forgia.html — jamais recopie ici.
$etabli = $null; $hashEtabli = $null
$node = Get-Node
$extracteur = Join-Path $Racine 'tools\ai\etabli_etat.mjs'
if ($node -and (Test-Path $extracteur)) {
    try {
        $brut = (& $node $extracteur 2>$null) -join "`n"
        if ($brut) {
            $etabli = $brut | ConvertFrom-Json
            # NE PAS nommer ceci $sha : $sha porte deja le HEAD git plus bas, et les
            # noms PowerShell sont insensibles a la casse — l'ecrasement serait silencieux.
            $hasheur = [Security.Cryptography.SHA1]::Create()
            $hashEtabli = [BitConverter]::ToString($hasheur.ComputeHash([Text.Encoding]::UTF8.GetBytes($brut))).Replace('-','').Substring(0,12)
            $hasheur.Dispose()
        }
    } catch { Write-Host "[recap] etat de l'Etabli illisible : $($_.Exception.Message)" }
}
# L'image fixe ne part pas a chaque ouverture : seulement si elle a BOUGE, ou une
# fois par jour. Sinon c'est le meme tableau cinq fois, donc du bruit.
$etabliABouge = $etabli -and ($hashEtabli -ne $etat.hash_etabli)
$etabliVieux  = $etabli -and $etat.dernier_etabli -and
                (((Get-Date) - [datetime]::Parse($etat.dernier_etabli)).TotalHours -ge 24)
$envoyerEtabli = $etabli -and ($Force -or $etabliABouge -or (-not $etat.dernier_etabli) -or $etabliVieux)

# ══════════════════ 3. la regle de silence ══════════════════
$aDireQuelqueChose = ($neuf.Count -gt 0) -or ($nouveauxCommits.Count -gt 0) -or ($nouvellesAlertes.Count -gt 0) -or $envoyerEtabli
$tropTot = $false
if ($etat.dernier_envoi) {
    try {
        $ecoule = (Get-Date) - [datetime]::Parse($etat.dernier_envoi)
        $tropTot = $ecoule.TotalHours -lt $MinHeures
    } catch { }
}
if (-not $Force) {
    if (-not $aDireQuelqueChose) { Write-Host '[recap] rien de neuf — silence.'; exit 0 }
    if ($tropTot -and $neuf.Count -eq 0) { Write-Host "[recap] < $MinHeures h et aucune veille neuve — silence."; exit 0 }
}

# ══════════════════ 4. composition ══════════════════
$ICONE = @{ 'bevy' = '🔧'; 'moteurs-rust' = '🦀'; 'jeux-ia' = '🤖' }
$L = New-Object System.Collections.Generic.List[string]

$L.Add("<b>⚒ Forgia</b> · $(Get-Date -Format 'dd/MM HH:mm') · <code>$(ConvertTo-TgHtml $branche)</code>")
$L.Add('')
$L.Add('<b>CHANTIER</b>')

$etatLigne = "$ouvertes story(s) ouverte(s)"
if ($ouvertes -gt 3) { $etatLigne += ' ⚠️ <i>limite WIP 3</i>' }
if ($sansStatut -gt 0) { $etatLigne += " · $sansStatut sans statut lisible" }
if ($sales -gt 0) { $etatLigne += " · $sales fichier(s) modifie(s)" }
$L.Add($etatLigne)

if ($perime -gt 0) {
    $L.Add("⚠️ binaire perime — $perime source(s) plus recente(s), ne rien conclure d'un test en jeu")
} elseif ($exe.Count -eq 1) {
    $L.Add('✅ binaire a jour')
}

if ($alertes.Count -gt 0) {
    $apercu = ($alertes | Select-Object -First 5) -join ', '
    if ($alertes.Count -gt 5) { $apercu += "… (+$($alertes.Count - 5))" }
    $marque = ''
    if ($nouvellesAlertes.Count -gt 0) { $marque = " · $($nouvellesAlertes.Count) nouvelle(s)" }
    $L.Add("⚠️ $($alertes.Count)/$($capteurs.Count) capteurs en alerte$marque : $(ConvertTo-TgHtml $apercu)")
} elseif ($capteurs.Count -gt 0) {
    $L.Add("✅ $($capteurs.Count) capteurs, aucune alerte")
}

if ($nouveauxCommits.Count -gt 0) {
    $L.Add('')
    $L.Add("<b>+$($nouveauxCommits.Count) commit(s)</b> depuis le dernier envoi")
    foreach ($c in ($nouveauxCommits | Select-Object -First 4)) {
        $p = $c -split '\|', 2
        $sujet = $p[1]
        if ($sujet.Length -gt 74) { $sujet = $sujet.Substring(0, 71) + '…' }
        $L.Add("· <code>$(ConvertTo-TgHtml $p[0])</code> $(ConvertTo-TgHtml $sujet)")
    }
    if ($nouveauxCommits.Count -gt 4) { $L.Add("· <i>… et $($nouveauxCommits.Count - 4) autres</i>") }
}

if ($neuf.Count -gt 0) {
    $L.Add('')
    $L.Add("<b>VEILLE · $($neuf.Count) nouvelle(s)</b>")
    foreach ($e in ($neuf | Select-Object -First 6)) {
        $ic = $ICONE[$e.axe]; if (-not $ic) { $ic = '•' }
        $titre = $e.titre
        if ($e.source) { $titre = "<a href=`"$(ConvertTo-TgHtml $e.source)`">$(ConvertTo-TgHtml $e.titre)</a>" }
        else { $titre = ConvertTo-TgHtml $e.titre }
        $quoi = $e.quoi
        if ($quoi.Length -gt 150) { $quoi = $quoi.Substring(0, 147) + '…' }
        $L.Add('')
        $L.Add("$ic <b>$titre</b>")
        $L.Add("$(ConvertTo-TgHtml $quoi)")
        $L.Add("<i>impact $($e.impact) · $($e.action)</i>")
    }
    if ($neuf.Count -gt 6) { $L.Add(''); $L.Add("<i>… et $($neuf.Count - 6) autres au registre</i>") }
}

$L.Add('')
$L.Add("<i>registre : $totalRegistre entrée(s) · <code>/veille</code> pour en ajouter</i>")

$message = ($L -join "`n")
if ($message.Length -gt 4000) { $message = $message.Substring(0, 3980) + "`n<i>… tronque</i>" }

# Deux messages DISTINCTS, dans cet ordre : l'image fixe d'abord, le delta de
# session ensuite. Sur un telephone, le dernier arrive est celui qu'on lit —
# donc le plus perissable en dernier.
$envois = @()
if ($envoyerEtabli) { $envois += ,@('etabli', (New-MessageEtabli $etabli)) }
$envois += ,@('session', $message)

if ($DryRun) {
    foreach ($e in $envois) {
        Write-Host "───────── message « $($e[0]) » (non envoye) ─────────"
        Write-Host $e[1]
    }
    Write-Host '────────────────────────────────────────'
    Write-Host "[recap] $($envois.Count) message(s) · $($neuf.Count) veille · $($nouveauxCommits.Count) commits · $($alertes.Count) alertes"
    exit 0
}

# ══════════════════ 5. envoi ══════════════════
$token  = Read-DpapiSecret -Path $TokenFile
$chatId = Read-DpapiSecret -Path $ChatIdFile
if (-not $token)  { $token  = $env:TELEGRAM_BOT_TOKEN }
if (-not $chatId) { $chatId = $env:TELEGRAM_CHAT_ID }
if (-not $token -or -not $chatId) {
    Write-Host '[recap] secrets absents. Lance : pwsh -File "D:/IA Antoine/veille/scripts/secrets-setup.ps1"'
    exit 1
}

$envoyes = 0
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    foreach ($e in $envois) {
        $corps = @{
            chat_id                  = $chatId
            text                     = $e[1]
            parse_mode               = 'HTML'
            disable_web_page_preview = $true
        } | ConvertTo-Json -Compress
        $rep = Invoke-RestMethod -Uri "https://api.telegram.org/bot$token/sendMessage" `
                                 -Method Post -ContentType 'application/json; charset=utf-8' `
                                 -Body ([Text.Encoding]::UTF8.GetBytes($corps)) -TimeoutSec 20
        if (-not $rep.ok) { throw "Telegram a refuse le message « $($e[0]) » : $($rep | ConvertTo-Json -Compress)" }
        $envoyes++
        # Telegram ordonne par heure d'arrivee : sans pause, les deux messages
        # peuvent s'afficher dans le desordre sur le telephone.
        if ($envoyes -lt $envois.Count) { Start-Sleep -Milliseconds 600 }
    }
} catch {
    # Un echec reseau ne doit RIEN marquer : sinon la veille est perdue en silence
    # (c'est exactement ce qui a tue le pipeline precedent le 22/06 — DNS injoignable,
    # et personne n'a su qu'il fallait relancer).
    Write-Host "[recap] envoi ECHOUE, rien marque : $($_.Exception.Message)"
    exit 1
} finally {
    $token = $null; $chatId = $null; [GC]::Collect()
}

# ══════════════════ 6. memoire du dernier envoi ══════════════════
$etat.ids = @(@($etat.ids) + @($neuf | ForEach-Object { $_.id }) | Sort-Object -Unique)
$etat.dernier_envoi = (Get-Date).ToString('o')
$etat.dernier_sha = $sha
$etat.alertes = $alertes
if ($envoyerEtabli) { $etat.hash_etabli = $hashEtabli; $etat.dernier_etabli = (Get-Date).ToString('o') }
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $EtatFile) | Out-Null
($etat | ConvertTo-Json -Depth 4) | Set-Content -Path $EtatFile -Encoding UTF8

Write-Host "[recap] envoye — $envoyes message(s) · $($neuf.Count) veille · $($nouveauxCommits.Count) commits"
exit 0

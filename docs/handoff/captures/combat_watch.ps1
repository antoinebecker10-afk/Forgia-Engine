# combat_watch.ps1 — OBSERVE une run forgia lancée à part (ex. `cargo run ... --features profile-tracy`).
# NE lance PAS le jeu. Attend qu'un process `forgia` apparaisse, puis échantillonne
# WS/Private + capteurs (VRAM, entités, colliders) toutes les 3 s jusqu'à sa fin.
# Lancer CE script AVANT ou juste après ton `cargo run` (il attend le process).
param([int]$MaxSeconds = 400)
$repo = "C:\Users\Antoi\Desktop\Forgia Rewrite"
$cap  = "$repo\docs\handoff\captures\combat"
New-Item -ItemType Directory -Force -Path $cap | Out-Null
$csv  = "$cap\samples.csv"
$mem="$repo\forgia2_memory.json"; $vram="$repo\forgia2_vram.json"
$perf="$repo\forgia2_perf_diag.json"; $load="$repo\forgia2_load_timing.json"
"t_s,ws_mb,priv_mb,sensor_ram_mb,sev,vram_mb,img_n,mesh_n,entities,enemies,colliders" | Out-File $csv -Encoding utf8

Write-Output "En attente du process 'forgia' (lance ton cargo run avec Tracy)..."
$p = $null; $waited = 0
while (-not $p -and $waited -lt 300) {
  $p = Get-Process forgia -ErrorAction SilentlyContinue | Sort-Object StartTime -Descending | Select-Object -First 1
  if (-not $p) { Start-Sleep -Seconds 2; $waited += 2 }
}
if (-not $p) { Write-Output "Aucun process forgia apparu en 300 s. Abandon."; return }
Write-Output "Process forgia PID=$($p.Id) détecté. Échantillonnage (JOUE : franchis 3-4 salles)."
Write-Output ("{0,-5} {1,-7} {2,-8} {3,-8} {4,-8} {5,-7} {6,-6} {7,-7} {8}" -f "t","WS","Priv","vramMB","imgN","meshN","ent","enem","coll")
$t0 = Get-Date; $t = 0
while ($t -le $MaxSeconds) {
  $proc = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
  if (-not $proc) {
    $el = ((Get-Date)-$t0).TotalSeconds
    Write-Output (">>> forgia a quitté à elapsed={0:N1}s. Si crash OOM, copie les ~15 dernières lignes de ton terminal cargo run." -f $el)
    break
  }
  $ws=[int]($proc.WorkingSet64/1MB); $pv=[int]($proc.PrivateMemorySize64/1MB)
  $sram="?";$sev="?";$vm="?";$imgn="?";$meshn="?";$ent="?";$enem="?";$coll="?"
  if(Test-Path $mem){try{$j=gc $mem -Raw|ConvertFrom-Json;$sram=[int]$j.ram_mb;$sev=$j.severity}catch{}}
  if(Test-Path $vram){try{$v=gc $vram -Raw|ConvertFrom-Json;$vm=[int]$v.total_estimated_mb;$imgn=$v.images_count;$meshn=$v.meshes_count}catch{}}
  if(Test-Path $perf){try{$pf=gc $perf -Raw|ConvertFrom-Json;$ent=$pf.load.total_entities;$enem=$pf.load.enemies}catch{}}
  if(Test-Path $load){try{$ld=gc $load -Raw|ConvertFrom-Json;$coll=$ld.colliders_now}catch{}}
  Write-Output ("{0,-5} {1,-7} {2,-8} {3,-8} {4,-8} {5,-7} {6,-6} {7,-7} {8}" -f $t,$ws,$pv,$vm,$imgn,$meshn,$ent,$enem,$coll)
  "$t,$ws,$pv,$sram,$sev,$vm,$imgn,$meshn,$ent,$enem,$coll" | Out-File $csv -Append -Encoding utf8
  Start-Sleep -Seconds 3; $t += 3
}
foreach($f in @($mem,$vram,$perf,$load)){ if(Test-Path $f){ Copy-Item $f (Join-Path $cap ("FINAL_"+(Split-Path $f -Leaf))) -Force } }
Write-Output "CSV -> $csv  (pings-moi, j'analyse la trajectoire assets-vs-hors-assets)"

# combat_capture.ps1 — mesure décisive du bloat mémoire combat.
# Lance forgia.exe, échantillonne mémoire process (WS/Private) + capteurs
# (VRAM total/images/meshes, entités, colliders) toutes les 3 s.
# Objectif : jouer ~2-3 min, franchir 3-4 salles jusqu'à ce que la sévérité
# mémoire passe 'critical'. Verdict : la VRAM (assets) grimpe-t-elle avec la RAM ?
param([int]$Seconds = 210)
$repo = "C:\Users\Antoi\Desktop\Forgia Rewrite"
$cap = "$repo\docs\handoff\captures\combat"
New-Item -ItemType Directory -Force -Path $cap | Out-Null
$csv = "$cap\samples.csv"; $out = "$cap\stdout.log"; $err = "$cap\stderr.log"
$mem="$repo\forgia2_memory.json"; $vram="$repo\forgia2_vram.json"
$perf="$repo\forgia2_perf_diag.json"; $load="$repo\forgia2_load_timing.json"
$env:RUST_BACKTRACE="1"; $env:RUST_LOG="warn"
foreach($f in @($vram)){ Remove-Item $f -ErrorAction SilentlyContinue }
"t_s,ws_mb,priv_mb,sensor_ram_mb,sev,vram_mb,img_n,mesh_n,entities,enemies,colliders" | Out-File $csv -Encoding utf8
$t0 = Get-Date
Write-Output "LAUNCH $($t0.ToString('HH:mm:ss'))  — JOUE : franchis 3-4 salles, monte les vagues."
$p = Start-Process -FilePath "$repo\target\release-fast\forgia.exe" -WorkingDirectory $repo -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
Write-Output ("{0,-5} {1,-7} {2,-8} {3,-8} {4,-8} {5,-7} {6,-6} {7,-7} {8}" -f "t","WS","Priv","vramMB","imgN","meshN","ent","enem","coll")
$t=0
while ($t -le $Seconds) {
  if ($p.HasExited) {
    $el=((Get-Date)-$t0).TotalSeconds
    Write-Output (">>> CRASH/EXIT elapsed={0:N1}s exit=0x{1:X8}" -f $el, ($p.ExitCode -band 0xFFFFFFFF))
    if (Test-Path $err){ Write-Output "--- stderr tail ---"; Get-Content $err -Tail 8 }
    break
  }
  $proc = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
  if ($proc) {
    $ws=[int]($proc.WorkingSet64/1MB); $pv=[int]($proc.PrivateMemorySize64/1MB)
    $sram="?";$sev="?";$vm="?";$imgn="?";$meshn="?";$ent="?";$enem="?";$coll="?"
    if(Test-Path $mem){try{$j=gc $mem -Raw|ConvertFrom-Json;$sram=[int]$j.ram_mb;$sev=$j.severity}catch{}}
    if(Test-Path $vram){try{$v=gc $vram -Raw|ConvertFrom-Json;$vm=[int]$v.total_estimated_mb;$imgn=$v.images_count;$meshn=$v.meshes_count}catch{}}
    if(Test-Path $perf){try{$pf=gc $perf -Raw|ConvertFrom-Json;$ent=$pf.load.total_entities;$enem=$pf.load.enemies}catch{}}
    if(Test-Path $load){try{$ld=gc $load -Raw|ConvertFrom-Json;$coll=$ld.colliders_now}catch{}}
    Write-Output ("{0,-5} {1,-7} {2,-8} {3,-8} {4,-8} {5,-7} {6,-6} {7,-7} {8}" -f $t,$ws,$pv,$vm,$imgn,$meshn,$ent,$enem,$coll)
    "$t,$ws,$pv,$sram,$sev,$vm,$imgn,$meshn,$ent,$enem,$coll" | Out-File $csv -Append -Encoding utf8
  }
  Start-Sleep -Seconds 3; $t += 3
}
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force; Write-Output "stopped at t~$Seconds (no crash)" }
foreach($f in @($mem,$vram,$perf,$load)){ if(Test-Path $f){ Copy-Item $f (Join-Path $cap ("FINAL_"+(Split-Path $f -Leaf))) -Force } }
Write-Output "CSV -> $csv"

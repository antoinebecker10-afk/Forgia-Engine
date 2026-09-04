$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$g = Join-Path $root "assets/genomes"

$mv = @{
  "async_pipeline_default.toml" = "perfs"
  "boss_default.toml"           = "enemies"
  "castle_medieval.toml"        = "biomes"
  "caves_default.toml"          = "map_gen"
  "character_default.toml"      = "ai"
  "clouds_default.toml"         = "sky"
  "combat_default.toml"         = "weapons"
  "dungeon_default.toml"        = "map_gen"
  "editor_default.toml"         = "ui"
  "erosion_advanced.toml"       = "map_gen"
  "grass_default.toml"          = "biomes"
  "island_mvp.toml"             = "map_gen"
  "npc_default.toml"            = "ai"
  "pack_nature.toml"            = "biomes"
  "path_default.toml"           = "map_gen"
  "qa_anomalies.toml"           = "debug_monitor"
  "terrain_lod.toml"            = "perfs"
  "terrain_materials.toml"      = "biomes"
  "vegetation_default.toml"     = "biomes"
  "vehicle_default.toml"        = "ai"
  "village_default.toml"        = "biomes"
  "water_advanced.toml"         = "biomes"
  "water_default.toml"          = "biomes"
  "wind_default.toml"           = "sky"
  "world_default.toml"          = "map_gen"
}

foreach ($k in $mv.Keys) {
    $src = Join-Path $g "_misc/$k"
    $dst = Join-Path $g "$($mv[$k])/$k"
    if (Test-Path $src) {
        Move-Item $src $dst -Force
        Write-Host "moved $k -> $($mv[$k])"
    }
}
$misc = Join-Path $g "_misc"
if (Test-Path $misc) { Remove-Item $misc -Recurse -Force }
Write-Host "Done."

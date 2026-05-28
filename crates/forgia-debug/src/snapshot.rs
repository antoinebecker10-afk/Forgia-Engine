//! Sensor snapshot — re-poll des `forgia*_*.json` racine workspace, 1Hz.
//!
//! Pas de dep `forgia-observability` (blast radius minimal). Lecture defensive :
//! sensor absent ou schéma changé → champ `None`, overlay continue de marcher.

use bevy::prelude::*;
use serde_json::Value;

#[derive(Resource, Debug, Default, Clone)]
pub struct SensorSnapshot {
    pub perf: PerfSlice,
    pub player: PlayerSlice,
    pub combat: CombatSlice,
    pub terrain: TerrainSlice,
    pub anim: AnimSlice,
    pub audio: AudioSlice,
    pub system: SystemSlice,
}

#[derive(Debug, Default, Clone)]
pub struct PerfSlice {
    pub fps: Option<f64>,
    pub frame_ms: Option<f64>,
}

#[derive(Debug, Default, Clone)]
pub struct PlayerSlice {
    pub app_mode: Option<String>,
    pub game_mode: Option<String>,
    pub position: Option<[f64; 3]>,
    pub velocity: Option<[f64; 3]>,
    pub grounded: Option<bool>,
    pub last_hp_current: Option<f64>,
    pub last_hp_max: Option<f64>,
    // forgia2_lifecycle
    pub players_added: Option<i64>,
    pub players_removed: Option<i64>,
    pub bots_added: Option<i64>,
    pub bots_removed: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct CombatSlice {
    // forgia2_combat.sources.damage_dir
    pub damage_events_received: Option<i64>,
    // forgia2_combat.sources.hitscan
    pub total_shots: Option<i64>,
    pub hits_with_damage: Option<i64>,
    pub hits_blocked_by_world: Option<i64>,
    pub missed_no_hit: Option<i64>,
    pub current_weapon: Option<String>,
    // forgia2_combat.sources.screen_flash
    pub damage_flashes_session: Option<i64>,
    pub kill_flashes_session: Option<i64>,
    pub low_hp_active: Option<bool>,
    // forgia2_combat.sources.killfeed
    pub total_kills_session: Option<i64>,
    pub streak_current: Option<i64>,
    // forgia_bot_ai
    pub bots_alive: Option<i64>,
    pub bots_with_los: Option<i64>,
    pub alerts_triggered_session: Option<i64>,
    pub los_checks_session: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct TerrainSlice {
    // forgia_chunks_snapshot
    pub total_chunks: Option<i64>,
    pub vegetation_total: Option<i64>,
    // forgia_terrain_lod
    pub lod0_count: Option<i64>,
    pub lod1_count: Option<i64>,
    // forgia_chunk_stream
    pub chunks_loading: Option<i64>,
    pub chunks_loaded: Option<i64>,
    // forgia_foliage_fallback events_last_30s
    pub foliage_fallback_events_30s: Option<i64>,
    pub foliage_fallback_total: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct AnimSlice {
    // forgia_foot_ik
    pub foot_ik_bones_missing: Option<bool>,
    pub foot_ik_active: Option<bool>,
    // forgia2_walk_pose
    pub walk_phase: Option<f64>,
    pub walk_speed_mps: Option<f64>,
    // forgia_anim_layer
    pub anim_layer_active: Option<String>,
    pub anim_layer_blend: Option<f64>,
    // forgia2_rex_bones_live
    pub rex_bones_present: Option<i64>,
}

#[derive(Debug, Default, Clone)]
pub struct AudioSlice {
    // forgia2_audio
    pub channels_active: Option<i64>,
    pub master_volume: Option<f64>,
    // forgia_music_state
    pub music_track: Option<String>,
    pub music_intensity: Option<f64>,
    // forgia_voicelines
    pub voicelines_played_session: Option<i64>,
    pub last_voiceline: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct SystemSlice {
    // forgia2_health
    pub health_severity: Option<String>,
    pub recent_alerts: Vec<String>,
    // forgia2_sensor_health
    pub sensors_stale: Option<i64>,
    pub sensors_total: Option<i64>,
    // forgia2_memory
    pub ram_mb: Option<f64>,
    // forgia2_entities
    pub entities_total: Option<i64>,
    // forgia2_lag_events
    pub lag_events_last_30s: Option<i64>,
    // forgia2_watchdog
    pub watchdog_seconds_in_emergency: Option<f64>,
}

/// Lit l'ensemble des sensors et renvoie un snapshot. Best-effort : tout sensor
/// absent / schéma changé / JSON invalide → champs `None`.
pub fn read_all() -> SensorSnapshot {
    let mut snap = SensorSnapshot::default();
    read_perf(&mut snap);
    read_player(&mut snap);
    read_combat(&mut snap);
    read_terrain(&mut snap);
    read_anim(&mut snap);
    read_audio(&mut snap);
    read_system(&mut snap);
    snap
}

fn read_json(path: &str) -> Option<Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn read_perf(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia2_perf.json") {
        snap.perf.fps = v.get("fps").and_then(|x| x.as_f64());
        snap.perf.frame_ms = v.get("frame_ms").and_then(|x| x.as_f64());
    }
}

fn read_player(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia2_player_state.json") {
        snap.player.app_mode = v.get("app_mode").and_then(|x| x.as_str()).map(String::from);
        snap.player.game_mode = v.get("game_mode").and_then(|x| x.as_str()).map(String::from);
        snap.player.position = read_vec3(&v, "position");
        snap.player.velocity = read_vec3(&v, "velocity");
        snap.player.grounded = v.get("grounded").and_then(|x| x.as_bool());
    }
    if let Some(v) = read_json("forgia2_player_hp_diag.json") {
        snap.player.last_hp_current = v.get("last_hp_current").and_then(|x| x.as_f64());
        snap.player.last_hp_max = v.get("last_hp_max").and_then(|x| x.as_f64());
        // Override game_mode si pas déjà set
        if snap.player.game_mode.is_none() {
            snap.player.game_mode = v
                .get("last_game_mode")
                .and_then(|x| x.as_str())
                .map(String::from);
        }
    }
    if let Some(v) = read_json("forgia2_lifecycle.json") {
        snap.player.players_added = v.get("players_added").and_then(|x| x.as_i64());
        snap.player.players_removed = v.get("players_removed").and_then(|x| x.as_i64());
        snap.player.bots_added = v.get("arena_bots_added").and_then(|x| x.as_i64());
        snap.player.bots_removed = v.get("arena_bots_removed").and_then(|x| x.as_i64());
    }
}

fn read_combat(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia2_combat.json") {
        let sources = v.get("sources");
        if let Some(dd) = sources.and_then(|s| s.get("damage_dir")) {
            snap.combat.damage_events_received = dd.get("events_received").and_then(|x| x.as_i64());
        }
        if let Some(hs) = sources.and_then(|s| s.get("hitscan")) {
            snap.combat.total_shots = hs.get("total_shots").and_then(|x| x.as_i64());
            snap.combat.hits_with_damage = hs.get("hits_with_damage").and_then(|x| x.as_i64());
            snap.combat.hits_blocked_by_world =
                hs.get("hits_blocked_by_world").and_then(|x| x.as_i64());
            snap.combat.missed_no_hit = hs.get("missed_no_hit").and_then(|x| x.as_i64());
            snap.combat.current_weapon = hs
                .get("current_weapon")
                .and_then(|x| x.as_str())
                .map(String::from);
        }
        if let Some(sf) = sources.and_then(|s| s.get("screen_flash")) {
            snap.combat.damage_flashes_session =
                sf.get("damage_flashes_session").and_then(|x| x.as_i64());
            snap.combat.kill_flashes_session =
                sf.get("kill_flashes_session").and_then(|x| x.as_i64());
            snap.combat.low_hp_active = sf.get("low_hp_active").and_then(|x| x.as_bool());
        }
        if let Some(kf) = sources.and_then(|s| s.get("killfeed")) {
            snap.combat.total_kills_session =
                kf.get("total_kills_session").and_then(|x| x.as_i64());
            snap.combat.streak_current = kf.get("streak_current").and_then(|x| x.as_i64());
        }
    }
    if let Some(v) = read_json("forgia_bot_ai.json") {
        snap.combat.bots_alive = v.get("bots_alive").and_then(|x| x.as_i64());
        snap.combat.bots_with_los = v.get("bots_with_los").and_then(|x| x.as_i64());
        snap.combat.alerts_triggered_session = v
            .get("alerts_triggered_session")
            .and_then(|x| x.as_i64());
        snap.combat.los_checks_session = v.get("los_checks_session").and_then(|x| x.as_i64());
    }
}

fn read_terrain(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia_chunks_snapshot.json") {
        snap.terrain.total_chunks = v.get("total_chunks").and_then(|x| x.as_i64());
        snap.terrain.vegetation_total = v.get("vegetation_total").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia_terrain_lod.json") {
        snap.terrain.lod0_count = v.get("lod0_count").and_then(|x| x.as_i64());
        snap.terrain.lod1_count = v.get("lod1_count").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia_chunk_stream.json") {
        snap.terrain.chunks_loading = v.get("chunks_loading").and_then(|x| x.as_i64());
        snap.terrain.chunks_loaded = v.get("chunks_loaded").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia_foliage_fallback.json") {
        snap.terrain.foliage_fallback_events_30s =
            v.get("events_last_30s").and_then(|x| x.as_i64());
        snap.terrain.foliage_fallback_total = v.get("total_recorded").and_then(|x| x.as_i64());
    }
}

fn read_anim(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia_foot_ik.json") {
        snap.anim.foot_ik_bones_missing = v.get("bones_missing").and_then(|x| x.as_bool());
        snap.anim.foot_ik_active = v.get("active").and_then(|x| x.as_bool());
    }
    if let Some(v) = read_json("forgia2_walk_pose.json") {
        snap.anim.walk_phase = v.get("phase").and_then(|x| x.as_f64());
        snap.anim.walk_speed_mps = v.get("speed_mps").and_then(|x| x.as_f64());
    }
    if let Some(v) = read_json("forgia_anim_layer.json") {
        snap.anim.anim_layer_active = v
            .get("active_layer")
            .and_then(|x| x.as_str())
            .map(String::from);
        snap.anim.anim_layer_blend = v.get("blend").and_then(|x| x.as_f64());
    }
    if let Some(v) = read_json("forgia2_rex_bones_live.json") {
        snap.anim.rex_bones_present = v
            .get("bones")
            .and_then(|x| x.as_object())
            .map(|o| o.len() as i64);
    }
}

fn read_audio(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia2_audio.json") {
        snap.audio.channels_active = v.get("channels_active").and_then(|x| x.as_i64());
        snap.audio.master_volume = v.get("master_volume").and_then(|x| x.as_f64());
    }
    if let Some(v) = read_json("forgia_music_state.json") {
        snap.audio.music_track = v
            .get("track")
            .and_then(|x| x.as_str())
            .map(String::from);
        snap.audio.music_intensity = v.get("intensity").and_then(|x| x.as_f64());
    }
    if let Some(v) = read_json("forgia_voicelines.json") {
        snap.audio.voicelines_played_session =
            v.get("played_session").and_then(|x| x.as_i64());
        snap.audio.last_voiceline = v
            .get("last_voiceline")
            .and_then(|x| x.as_str())
            .map(String::from);
    }
}

fn read_system(snap: &mut SensorSnapshot) {
    if let Some(v) = read_json("forgia2_health.json") {
        snap.system.health_severity = v
            .get("severity")
            .and_then(|x| x.as_str())
            .map(String::from);
        if let Some(arr) = v.get("recent_alerts").and_then(|x| x.as_array()) {
            snap.system.recent_alerts = arr
                .iter()
                .take(3)
                .filter_map(|a| a.as_str().map(String::from))
                .collect();
        }
    }
    if let Some(v) = read_json("forgia2_sensor_health.json") {
        snap.system.sensors_stale = v.get("stale_count").and_then(|x| x.as_i64());
        snap.system.sensors_total = v.get("total_count").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia2_memory.json") {
        snap.system.ram_mb = v.get("ram_mb").and_then(|x| x.as_f64());
    }
    if let Some(v) = read_json("forgia2_entities.json") {
        snap.system.entities_total = v.get("total").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia2_lag_events.json") {
        snap.system.lag_events_last_30s = v.get("events_last_30s").and_then(|x| x.as_i64());
    }
    if let Some(v) = read_json("forgia2_watchdog.json") {
        snap.system.watchdog_seconds_in_emergency = v
            .get("seconds_in_emergency")
            .and_then(|x| x.as_f64());
    }
}

fn read_vec3(v: &Value, key: &str) -> Option<[f64; 3]> {
    let arr = v.get(key)?.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([
        arr[0].as_f64()?,
        arr[1].as_f64()?,
        arr[2].as_f64()?,
    ])
}

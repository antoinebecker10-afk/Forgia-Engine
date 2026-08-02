//! enemies.rs — M2 step 2 : 3 archetypes ennemis Roguelite (data-driven).
//!
//! Story-470 M2 step 2 — résout le bug "ennemis trop collés" en variant
//! `stop_distance` et `attack_range` par archetype : naturellement dispersés
//! par design plutôt que via Boids separation (option B reportée).
//!
//! | Archetype | HP   | Speed | Stop | Attack | Cooldown | Couleur     | Rôle |
//! |-----------|------|-------|------|--------|----------|-------------|------|
//! | Tank      |  120 |  2.8  |  3.0 |   4.0  |   1.8s   | rouge sombre | front-line tank |
//! | Runner    |   35 |  7.0  |  6.0 |   7.0  |   0.7s   | orange       | rusher proche |
//! | Sniper    |   45 |  3.2  | 22.0 |  24.0  |   1.6s   | violet       | back-line ranged |
//!
//! Hot-reload TOML : reporté M2 step 3 (`config/genomes/roguelite_enemies.toml`).

use bevy::color::LinearRgba;
use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_enemies.toml";
const POLL_PERIOD_SEC: f32 = 1.0;
const SENSOR_PATH: &str = "forgia2_enemies.json";

/// Marker Component pour identifier l'archetype runtime (debug + futur loot/xp).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyArchetype {
    Tank,
    Runner,
    Sniper,
    /// M3 step 1 — boss final wave 3. Unique, tanky, 2 phases (enrage à 50% HP).
    Boss,
}

impl EnemyArchetype {
    /// Nom court pour logs / sensor / debug.
    pub fn label(self) -> &'static str {
        match self {
            Self::Tank => "tank",
            Self::Runner => "runner",
            Self::Sniper => "sniper",
            Self::Boss => "boss",
        }
    }
}

/// Stats d'un archetype — data-driven (genome `roguelite_enemies.toml`, hot-reload).
impl EnemyStats {
    /// Altitude du CENTRE de la capsule quand les pieds touchent le sol (m).
    ///
    /// Story-685 — **source unique**. Cette expression était écrite dans
    /// `waves.rs` (le `y` de spawn) ET dans `arena_bot()` (l'offset du suivi de
    /// sol). Deux copies de la même grandeur finissent toujours par diverger :
    /// le bot s'enterrerait d'un demi-corps ou léviterait, et rien ne dirait
    /// pourquoi. C'est la même classe de défaut que le « SALLE 5 / 4 ».
    ///
    /// Les 5 cm de marge évitent que le collider naisse en contact exact avec
    /// le sol.
    pub fn foot_offset_m(&self) -> f32 {
        self.capsule_half_height + self.capsule_radius + 0.05
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub stop_distance: f32,
    pub attack_range: f32,
    pub detect_range: f32,
    pub attack_cooldown: f32,
    pub warmup_secs: f32,
    pub capsule_radius: f32,
    pub capsule_half_height: f32,
    /// Rayon override de la hitbox tête (m monde). None = auto : crâne mesuré du
    /// GLB × skeleton_scale (story-652, cf `head_hitbox.rs`). Champ optionnel dans
    /// le TOML (absent → None → auto).
    #[serde(default)]
    pub head_radius: Option<f32>,
    /// Couleur base RGB linéaire (StandardMaterial.base_color).
    pub color_rgb: [f32; 3],
    /// Couleur émissive RGB linéaire (faible — silhouette pop sans flash).
    pub emissive_rgb: [f32; 3],
    /// Dégâts du tir du bot (`BotShootConfig.damage`).
    pub shoot_damage: f32,
    /// Portée effective du tir (m).
    pub shoot_range: f32,
    /// Dispersion du tir (degrés → radians à la conversion).
    pub shoot_jitter_deg: f32,
}

/// Config des stats ennemis — Resource data-driven (genome TOML, hot-reload mtime).
/// `Default` = miroir EXACT des valeurs historiques (M2 step 2) → zéro régression si
/// le TOML disparaît. Calqué sur `elements.rs` / `ultimate_config.rs`.
#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct EnemyStatsConfig {
    pub tank: EnemyStats,
    pub runner: EnemyStats,
    pub sniper: EnemyStats,
    pub boss: EnemyStats,
}

impl Default for EnemyStatsConfig {
    fn default() -> Self {
        // Story-652 — capsules recalibrées sur les meshs KayKit mesurés : la capsule
        // body s'arrête aux ÉPAULES (H = 1.40 × skeleton_scale, bas du crâne à
        // 1.31 × scale + chevauchement anti-trou-du-cou) ; le crâne est couvert par la
        // sphère tête qui DÉPASSE (sinon headshot impossible, cf head_hitbox.rs).
        // hh = H/2 − radius. Avant : tank 0.75 / runner 0.60 / sniper 0.85 / boss 2.2
        // (le Runner avait 33 cm de crâne sans collider, le Boss 2 m de vide touchable).
        Self {
            tank: EnemyStats {
                hp: 120.0,
                speed: 2.8,
                stop_distance: 3.0,
                attack_range: 4.0,
                detect_range: 22.0,
                attack_cooldown: 1.8,
                warmup_secs: 2.0,
                capsule_radius: 0.55,
                capsule_half_height: 0.43,
                head_radius: None,
                color_rgb: [0.55, 0.10, 0.10],
                emissive_rgb: [0.25, 0.03, 0.03],
                shoot_damage: 25.0,
                shoot_range: 5.0,
                shoot_jitter_deg: 6.0,
            },
            runner: EnemyStats {
                hp: 35.0,
                speed: 7.0,
                stop_distance: 6.0,
                attack_range: 7.0,
                detect_range: 40.0,
                attack_cooldown: 0.7,
                warmup_secs: 1.2,
                capsule_radius: 0.32,
                capsule_half_height: 0.38,
                head_radius: None,
                color_rgb: [0.95, 0.45, 0.10],
                emissive_rgb: [0.40, 0.18, 0.04],
                shoot_damage: 8.0,
                shoot_range: 8.0,
                shoot_jitter_deg: 5.0,
            },
            sniper: EnemyStats {
                hp: 45.0,
                speed: 3.2,
                stop_distance: 22.0,
                attack_range: 24.0,
                detect_range: 55.0,
                attack_cooldown: 1.6,
                warmup_secs: 1.8,
                capsule_radius: 0.30,
                capsule_half_height: 0.47,
                head_radius: None,
                color_rgb: [0.55, 0.20, 0.80],
                emissive_rgb: [0.22, 0.08, 0.35],
                shoot_damage: 18.0,
                shoot_range: 28.0,
                shoot_jitter_deg: 1.5,
            },
            // Boss : tanky, mi-range, intimidant. Phase 2 enrage à 50% HP :
            // speed×1.8, cooldown×0.55 (cf sys_boss_enrage).
            boss: EnemyStats {
                hp: 800.0,
                speed: 3.5,
                stop_distance: 10.0,
                attack_range: 30.0,
                detect_range: 80.0,
                attack_cooldown: 1.3,
                warmup_secs: 2.5,
                capsule_radius: 1.4,
                capsule_half_height: 0.35,
                head_radius: None,
                color_rgb: [0.90, 0.10, 0.50],
                emissive_rgb: [0.50, 0.05, 0.25],
                shoot_damage: 22.0,
                shoot_range: 32.0,
                shoot_jitter_deg: 3.0,
            },
        }
    }
}

impl EnemyStatsConfig {
    /// Stats de l'archétype (data live).
    pub fn for_archetype(&self, a: EnemyArchetype) -> EnemyStats {
        match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            EnemyArchetype::Boss => self.boss,
        }
    }

    /// `ArenaBot` configuré depuis les stats live de l'archétype.
    pub fn arena_bot(&self, a: EnemyArchetype) -> ArenaBot {
        let s = self.for_archetype(a);
        ArenaBot {
            speed: s.speed,
            detect_range: s.detect_range,
            attack_range: s.attack_range,
            attack_cooldown: s.attack_cooldown,
            attack_left: s.warmup_secs,
            stop_distance: s.stop_distance,
            // Story-685 — MÊME expression que le `y` de spawn (`waves.rs`) :
            // une seule source, sinon les deux divergent et le bot s'enterre ou
            // flotte d'un demi-corps.
            foot_offset_m: s.foot_offset_m(),
            ..ArenaBot::default()
        }
    }

    /// `BotShootConfig` : scalaires de combat depuis le genome ; couleur du tracer +
    /// hauteur d'épaule restent en code (visuel, non gameplay).
    pub fn bot_shoot(&self, a: EnemyArchetype) -> forgia_ai_arena_bot::BotShootConfig {
        use forgia_ai_arena_bot::BotShootConfig;
        let s = self.for_archetype(a);
        let (tracer_emissive, shoulder_y) = match a {
            EnemyArchetype::Tank => (LinearRgba::new(5.0, 0.5, 0.5, 1.0), 1.2),
            EnemyArchetype::Runner => (LinearRgba::new(4.0, 2.0, 0.5, 1.0), 0.9),
            EnemyArchetype::Sniper => (LinearRgba::new(3.0, 0.5, 5.0, 1.0), 1.1),
            EnemyArchetype::Boss => (LinearRgba::new(5.0, 0.5, 3.0, 1.0), 2.4),
        };
        BotShootConfig {
            damage: s.shoot_damage,
            range: s.shoot_range,
            jitter_rad: s.shoot_jitter_deg.to_radians(),
            tracer_emissive,
            shoulder_y,
            target_torso_y: 1.0,
        }
    }

    /// Pur — testable headless. Fallback `Default` sur toute erreur de parse.
    pub fn parse_toml(content: &str) -> Self {
        toml::from_str(content).unwrap_or_else(|_| Self::default())
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Watch mtime du TOML (hot-reload, miroir `ElementGenomeWatch`).
#[derive(Resource, Default, Debug)]
pub struct EnemyGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Stats par archetype — délègue au `Default` de la config (compat tests + appelants
/// purs). Le spawn LIVE lit `Res<EnemyStatsConfig>` (cf `waves.rs`).
pub fn stats_for(archetype: EnemyArchetype) -> EnemyStats {
    EnemyStatsConfig::default().for_archetype(archetype)
}

/// KayKit Skeleton GLB asset path par archetype (story-517 fix).
/// Mapping : Tank=Warrior, Runner=Minion, Sniper=Mage, Boss=Warrior×2.5.
/// Memory ref : reference_roguelite_enemy_skeleton_mapping.md.
pub fn skeleton_asset_path(archetype: EnemyArchetype) -> &'static str {
    match archetype {
        EnemyArchetype::Tank => "models/kaykit/skeletons/Skeleton_Warrior.glb",
        EnemyArchetype::Runner => "models/kaykit/skeletons/Skeleton_Minion.glb",
        EnemyArchetype::Sniper => "models/kaykit/skeletons/Skeleton_Mage.glb",
        EnemyArchetype::Boss => "models/kaykit/skeletons/Skeleton_Warrior.glb",
    }
}

/// Scale uniforme à appliquer au SceneRoot KayKit pour matcher la silhouette
/// capsule originale (visual cohérence avec les colliders existants).
pub fn skeleton_scale(archetype: EnemyArchetype) -> f32 {
    match archetype {
        EnemyArchetype::Tank => 1.4,
        EnemyArchetype::Runner => 1.0,
        EnemyArchetype::Sniper => 1.1,
        EnemyArchetype::Boss => 2.5,
    }
}

/// BotShootConfig par archetype — délègue au `Default` de la config (scalaires de
/// combat data-driven ; couleur tracer + épaule = visuel en code). Le spawn LIVE
/// passe par `EnemyStatsConfig::bot_shoot` pour le hot-reload.
pub fn bot_shoot_for(archetype: EnemyArchetype) -> forgia_ai_arena_bot::BotShootConfig {
    EnemyStatsConfig::default().bot_shoot(archetype)
}

/// Construit un `ArenaBot` configuré pour l'archetype donné.
/// Réutilise les champs default pour les LOS/strafe/alert (préservés inchangés).
pub fn arena_bot_for(archetype: EnemyArchetype) -> ArenaBot {
    let s = stats_for(archetype);
    ArenaBot {
        speed: s.speed,
        detect_range: s.detect_range,
        attack_range: s.attack_range,
        attack_cooldown: s.attack_cooldown,
        attack_left: s.warmup_secs,
        stop_distance: s.stop_distance,
        ..ArenaBot::default()
    }
}

// ─── Systems : genome load + hot-reload + sensor (story-638) ─────────────────

/// Startup : charge le TOML + init le watch mtime.
pub fn sys_init_enemy_genome(mut commands: Commands) {
    let cfg = EnemyStatsConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    info!(
        "[enemies] genome chargé — tank hp{:.0}/spd{:.1} runner hp{:.0} sniper hp{:.0} boss hp{:.0}",
        cfg.tank.hp, cfg.tank.speed, cfg.runner.hp, cfg.sniper.hp, cfg.boss.hp,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(EnemyGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
}

/// Poll mtime 1Hz, re-parse si changé (hot-reload).
pub fn sys_hot_reload_enemy_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<EnemyStatsConfig>,
    mut watch: ResMut<EnemyGenomeWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(meta) = fs::metadata(GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = EnemyStatsConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[enemies] HOT-RELOADED #{} — tank hp{:.0} runner hp{:.0} sniper hp{:.0} boss hp{:.0}",
            watch.reload_count,
            new_cfg.tank.hp,
            new_cfg.runner.hp,
            new_cfg.sniper.hp,
            new_cfg.boss.hp,
        );
        *cfg = new_cfg;
    }
}

/// Sensor `forgia2_enemies.json` 1Hz : stats live par archétype + reload_count.
pub fn sys_write_enemies_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Res<EnemyStatsConfig>,
    watch: Res<EnemyGenomeWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    // spawn_live=true (story-640 P0-2) : le SPAWN consomme désormais `Res<EnemyStatsConfig>`
    // live (waves.rs::spawn_wave_enemies) → un hot-reload de `roguelite_enemies.toml`
    // change les ennemis des prochaines vagues (HP/speed/dmg/capsule) + attache la
    // DefenseLayer. Les vagues déjà spawnées gardent leurs stats (pas de re-bain live).
    let json = format!(
        r#"{{"id":"enemies","severity":"ok","next_step":"","spawn_live":true,"reload_count":{},"tank":{{"hp":{:.0},"speed":{:.1},"dmg":{:.0}}},"runner":{{"hp":{:.0},"speed":{:.1},"dmg":{:.0}}},"sniper":{{"hp":{:.0},"speed":{:.1},"dmg":{:.0}}},"boss":{{"hp":{:.0},"speed":{:.1},"dmg":{:.0}}}}}"#,
        watch.reload_count,
        cfg.tank.hp,
        cfg.tank.speed,
        cfg.tank.shoot_damage,
        cfg.runner.hp,
        cfg.runner.speed,
        cfg.runner.shoot_damage,
        cfg.sniper.hp,
        cfg.sniper.speed,
        cfg.sniper.shoot_damage,
        cfg.boss.hp,
        cfg.boss.speed,
        cfg.boss.shoot_damage,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[enemies] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archetype_labels_distinct() {
        assert_ne!(EnemyArchetype::Tank.label(), EnemyArchetype::Runner.label());
        assert_ne!(
            EnemyArchetype::Runner.label(),
            EnemyArchetype::Sniper.label()
        );
        assert_eq!(EnemyArchetype::Tank.label(), "tank");
    }

    #[test]
    fn stats_tank_high_hp_low_speed() {
        let t = stats_for(EnemyArchetype::Tank);
        let r = stats_for(EnemyArchetype::Runner);
        assert!(t.hp > r.hp, "Tank doit avoir plus d'HP qu'un Runner");
        assert!(t.speed < r.speed, "Tank doit être plus lent qu'un Runner");
    }

    #[test]
    fn stats_sniper_long_range() {
        let s = stats_for(EnemyArchetype::Sniper);
        let t = stats_for(EnemyArchetype::Tank);
        assert!(
            s.attack_range > t.attack_range * 4.0,
            "Sniper doit avoir une range >4× celle d'un Tank"
        );
        assert!(s.stop_distance >= 20.0, "Sniper s'arrête au moins à 20m");
    }

    #[test]
    fn stop_distances_distinct_for_spread() {
        // Cœur du fix sticky AI : les 3 archetypes ont des stop_distance distincts
        // donc se positionnent naturellement à des cercles différents autour du player.
        let t = stats_for(EnemyArchetype::Tank).stop_distance;
        let r = stats_for(EnemyArchetype::Runner).stop_distance;
        let s = stats_for(EnemyArchetype::Sniper).stop_distance;
        assert!(t < r, "Tank stop < Runner stop");
        assert!(r < s, "Runner stop < Sniper stop");
        assert!(s - t > 15.0, "Spread total entre Tank et Sniper > 15m");
    }

    #[test]
    fn arena_bot_for_uses_stats() {
        let bot = arena_bot_for(EnemyArchetype::Runner);
        let stats = stats_for(EnemyArchetype::Runner);
        assert!((bot.speed - stats.speed).abs() < 0.001);
        assert!((bot.stop_distance - stats.stop_distance).abs() < 0.001);
        assert!((bot.attack_range - stats.attack_range).abs() < 0.001);
        assert!((bot.attack_left - stats.warmup_secs).abs() < 0.001);
    }

    // ── Config data-driven (story-638) ──

    #[test]
    fn config_default_mirrors_free_stats_for() {
        // Le Default de la config == les valeurs historiques exposées par stats_for.
        let c = EnemyStatsConfig::default();
        for a in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ] {
            assert_eq!(c.for_archetype(a), stats_for(a));
        }
        assert_eq!(c.tank.hp, 120.0);
        assert_eq!(c.boss.hp, 800.0);
        assert_eq!(c.runner.shoot_damage, 8.0);
    }

    #[test]
    fn config_parse_garbage_falls_back_to_default() {
        assert_eq!(
            EnemyStatsConfig::parse_toml("pas du TOML [[["),
            EnemyStatsConfig::default()
        );
    }

    #[test]
    fn config_parse_full_toml_roundtrip() {
        let toml = r#"
[tank]
hp = 200.0
speed = 2.0
stop_distance = 3.0
attack_range = 4.0
detect_range = 22.0
attack_cooldown = 1.8
warmup_secs = 2.0
capsule_radius = 0.55
capsule_half_height = 0.75
color_rgb = [0.5, 0.1, 0.1]
emissive_rgb = [0.2, 0.0, 0.0]
shoot_damage = 30.0
shoot_range = 5.0
shoot_jitter_deg = 6.0
[runner]
hp = 35.0
speed = 7.0
stop_distance = 6.0
attack_range = 7.0
detect_range = 40.0
attack_cooldown = 0.7
warmup_secs = 1.2
capsule_radius = 0.32
capsule_half_height = 0.60
color_rgb = [0.9, 0.4, 0.1]
emissive_rgb = [0.4, 0.1, 0.0]
shoot_damage = 8.0
shoot_range = 8.0
shoot_jitter_deg = 5.0
[sniper]
hp = 45.0
speed = 3.2
stop_distance = 22.0
attack_range = 24.0
detect_range = 55.0
attack_cooldown = 1.6
warmup_secs = 1.8
capsule_radius = 0.30
capsule_half_height = 0.85
color_rgb = [0.5, 0.2, 0.8]
emissive_rgb = [0.2, 0.0, 0.3]
shoot_damage = 18.0
shoot_range = 28.0
shoot_jitter_deg = 1.5
[boss]
hp = 800.0
speed = 3.5
stop_distance = 10.0
attack_range = 30.0
detect_range = 80.0
attack_cooldown = 1.3
warmup_secs = 2.5
capsule_radius = 1.4
capsule_half_height = 2.2
color_rgb = [0.9, 0.1, 0.5]
emissive_rgb = [0.5, 0.0, 0.2]
shoot_damage = 22.0
shoot_range = 32.0
shoot_jitter_deg = 3.0
"#;
        let c = EnemyStatsConfig::parse_toml(toml);
        assert_eq!(c.tank.hp, 200.0, "override tank hp lu du TOML");
        assert_eq!(c.tank.shoot_damage, 30.0);
        assert_ne!(
            c,
            EnemyStatsConfig::default(),
            "diff du Default → hot-reload déclenchable"
        );
    }

    #[test]
    fn config_bot_shoot_reads_genome_scalars() {
        let c = EnemyStatsConfig::default();
        let bs = c.bot_shoot(EnemyArchetype::Sniper);
        assert_eq!(bs.damage, c.sniper.shoot_damage);
        assert_eq!(bs.range, c.sniper.shoot_range);
        assert!((bs.jitter_rad - c.sniper.shoot_jitter_deg.to_radians()).abs() < 1e-6);
    }
}

#[cfg(test)]
mod foot_offset_tests {
    use super::*;

    /// Story-685 — l'offset porté par le bot DOIT être celui de son spawn.
    ///
    /// Le suivi de sol repose entièrement sur cette égalité : si les deux
    /// divergent, chaque bot s'enterre d'un demi-corps ou lévite, et rien ne dit
    /// pourquoi. L'expression est désormais unique (`EnemyStats::foot_offset_m`) ;
    /// ce test interdit qu'on la recopie ailleurs sans s'en apercevoir.
    #[test]
    fn the_bot_carries_exactly_its_spawn_offset() {
        let cfg = EnemyStatsConfig::default();
        for a in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ] {
            let s = cfg.for_archetype(a);
            assert!(
                (cfg.arena_bot(a).foot_offset_m - s.foot_offset_m()).abs() < 1e-6,
                "{a:?} : l'offset du bot et celui du spawn ont divergé"
            );
        }
    }

    /// Un offset nul enterrerait le bot ; un offset démesuré le ferait léviter.
    #[test]
    fn every_archetype_has_a_plausible_offset() {
        let cfg = EnemyStatsConfig::default();
        for a in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ] {
            let o = cfg.for_archetype(a).foot_offset_m();
            assert!(
                (0.2..=4.0).contains(&o),
                "{a:?} : offset {o:.2} m — enterré ou en lévitation"
            );
        }
    }
}

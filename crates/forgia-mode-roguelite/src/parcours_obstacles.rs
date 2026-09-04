//! parcours_obstacles.rs — Story-590 (2026-06-09). Obstacles animés du parcours.
//!
//! Complexifie le parcours façon **Fall Guys** : les obstacles du GLB
//! `platformer_underworld.glb` qui ÉTAIENT statiques se mettent à bouger.
//! Trois familles (taxonomie déduite des AABB du kit) :
//! - **Marteau pendule** (`obstacle_1`) : balance autour du pivot du haut (la
//!   masse pend dessous) → sweep latéral qui pousse le joueur.
//! - **Balayeur** (`obstacle_3/4/6`) : barre/croix qui tourne sur Y (existant
//!   obstacle_3/6 + croix obstacle_4 ajoutée).
//! - **Bloc coulissant** (`obstacle_2`) : translation sinusoïdale en travers.
//!
//! ## Colliders
//!
//! Les obstacles gardent leur `ConvexHull` (posé par `loot_room`). Animer leur
//! `Transform` déplace le collider via propagation `GlobalTransform` → le joueur
//! (KCC) est poussé/bloqué = vraie difficulté. Pattern prouvé par les spinners.
//!
//! ## Data-driven
//!
//! `assets/genomes/roguelite/roguelite_obstacles.toml` (mtime hot-reload, miroir
//! `ObstacleConfig::default()`) → tuner swing/spin/slide sans rebuild.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, QueryFilter, ReadRapierContext};
use forgia_core::prelude::*;
use forgia_player::Player;
use serde::Deserialize;
use std::collections::HashSet;
use std::f32::consts::TAU;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_obstacles.toml";
const SENSOR_PATH: &str = "forgia2_obstacles.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── Composants d'animation (tag posé par loot_room::sys_mark_demo_meshes) ───

/// Marteau pendule (`obstacle_1`) : la masse pend SOUS le pivot (origine du
/// node). `base` = rotation authored, capturée au tag (on l'écrase chaque frame).
#[derive(Component)]
pub struct SwingingHammer {
    pub base: Quat,
    pub phase: f32,
}

/// Barre/croix qui tourne sur l'axe Y (balayeur). Rotation CUMULÉE (pas de base).
#[derive(Component)]
pub struct RotatingObstacle;

/// Bloc coulissant : translation sinusoïdale le long de `axis` autour de `base`,
/// + rotation sur Y (le « maillet » tourne sur son axe en glissant — demande user).
#[derive(Component)]
pub struct SlidingObstacle {
    pub base: Vec3,
    pub axis: Vec3,
    pub phase: f32,
}

/// Marqueur sur TOUT obstacle animé (marteau/balayeur/coulissant) — cible du
/// système de push physique [`sys_obstacle_push`].
#[derive(Component)]
pub struct AnimatedObstacle;

/// Famille d'animation déduite du nom de node GLB.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ObstacleAnim {
    Hammer,
    Spinner,
    Slider,
}

/// Mappe un nom de node → animation (None = statique). **Source unique** de la
/// taxonomie `obstacle_<type>_…`. Le `_` final évite que `obstacle_1` matche
/// `obstacle_11` (plateforme, à laisser statique).
pub fn classify(name: &str) -> Option<ObstacleAnim> {
    if name.starts_with("obstacle_1_") {
        Some(ObstacleAnim::Hammer)
    } else if name.starts_with("obstacle_3_")
        || name.starts_with("obstacle_4_")
        || name.starts_with("obstacle_6_")
    {
        Some(ObstacleAnim::Spinner)
    } else if name.starts_with("obstacle_2_") {
        Some(ObstacleAnim::Slider)
    } else {
        None
    }
}

/// Phase déterministe `[0,TAU)` dérivée de la position → désynchronise les
/// obstacles (swing/slide non synchrones = plus organique et plus dur).
pub fn phase_from_pos(p: Vec3) -> f32 {
    (p.x * 0.7 + p.z * 1.3).rem_euclid(TAU)
}

// ─── Config genome (mtime, miroir elements.rs) ──────────────────────────────

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct HammerParams {
    pub enabled: bool,
    /// Amplitude de balancement de part et d'autre (degrés).
    pub swing_deg: f32,
    /// Pulsation (rad/s).
    pub freq: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct SpinnerParams {
    pub enabled: bool,
    /// Vitesse de rotation Y (rad/s).
    pub speed: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct SliderParams {
    pub enabled: bool,
    /// Amplitude de translation (m, de part et d'autre).
    pub amplitude: f32,
    pub freq: f32,
    /// Vitesse de rotation Y du bloc pendant qu'il glisse (rad/s).
    pub spin_speed: f32,
}

/// Push physique : repousse le joueur tant qu'il chevauche un obstacle (même à
/// l'arrêt). Détection via le vrai collider (Rapier `intersection_with_shape`).
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct PushParams {
    pub enabled: bool,
    /// Vitesse de répulsion (m/s) appliquée tant que le joueur chevauche.
    pub speed: f32,
}

#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct ObstacleConfig {
    pub hammer: HammerParams,
    pub spinner: SpinnerParams,
    pub slider: SliderParams,
    pub push: PushParams,
}

impl Default for ObstacleConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_obstacles.toml.
        Self {
            hammer: HammerParams {
                enabled: true,
                swing_deg: 62.0,
                freq: 1.6,
            },
            spinner: SpinnerParams {
                enabled: true,
                speed: 1.0,
            },
            slider: SliderParams {
                enabled: true,
                amplitude: 2.5,
                freq: 1.1,
                spin_speed: 2.0,
            },
            push: PushParams {
                enabled: true,
                speed: 7.0,
            },
        }
    }
}

impl ObstacleConfig {
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

#[derive(Resource, Default, Debug)]
pub struct ObstacleGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Compteurs (sensor) : nombre d'obstacles de chaque famille tagués au marquage
/// du niveau. Renseignés par `loot_room::sys_mark_demo_meshes`.
#[derive(Resource, Default, Debug, Clone)]
pub struct ObstacleStats {
    pub hammers: u32,
    pub spinners: u32,
    pub sliders: u32,
}

/// État du push (sensor) : le joueur est-il en train d'être repoussé, et combien
/// de frames de push depuis le début du run.
#[derive(Resource, Default, Debug, Clone)]
pub struct ObstaclePushState {
    pub pushing: bool,
    pub last_dir: Vec3,
    pub events: u32,
}

// ─── Systems : genome ───────────────────────────────────────────────────────

pub fn sys_init_obstacle_genome(mut commands: Commands) {
    let cfg = ObstacleConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    info!(
        "[obstacles] genome loaded — hammer({} {:.0}° {:.1}rad/s) spinner({} {:.1}) slider({} {:.1}m)",
        cfg.hammer.enabled, cfg.hammer.swing_deg, cfg.hammer.freq,
        cfg.spinner.enabled, cfg.spinner.speed,
        cfg.slider.enabled, cfg.slider.amplitude,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(ObstacleGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
}

pub fn sys_hot_reload_obstacle_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<ObstacleConfig>,
    mut watch: ResMut<ObstacleGenomeWatch>,
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
    let new_cfg = ObstacleConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[obstacles] HOT-RELOADED #{} — hammer {:.0}°/{:.1} spinner {:.1} slider {:.1}m/{:.1}",
            watch.reload_count,
            new_cfg.hammer.swing_deg,
            new_cfg.hammer.freq,
            new_cfg.spinner.speed,
            new_cfg.slider.amplitude,
            new_cfg.slider.freq,
        );
        *cfg = new_cfg;
    }
}

/// OnEnter Roguelite — reset des compteurs (le niveau est remarqué à chaque run).
pub fn sys_reset_obstacle_stats(
    mut stats: ResMut<ObstacleStats>,
    mut push: ResMut<ObstaclePushState>,
) {
    *stats = ObstacleStats::default();
    *push = ObstaclePushState::default();
}

// ─── Systems : animation (GameSet::Movement → collider sync après) ──────────

pub fn sys_swing_hammers(
    time: Res<Time>,
    cfg: Res<ObstacleConfig>,
    mut q: Query<(&SwingingHammer, &mut Transform)>,
) {
    if !cfg.hammer.enabled {
        return;
    }
    let t = time.elapsed_secs();
    let amp = cfg.hammer.swing_deg.to_radians();
    for (h, mut tf) in &mut q {
        let angle = amp * (t * cfg.hammer.freq + h.phase).sin();
        tf.rotation = h.base * Quat::from_rotation_z(angle);
    }
}

pub fn sys_rotate_obstacles(
    time: Res<Time>,
    cfg: Res<ObstacleConfig>,
    mut q: Query<&mut Transform, With<RotatingObstacle>>,
) {
    if !cfg.spinner.enabled {
        return;
    }
    let dr = time.delta_secs() * cfg.spinner.speed;
    for mut tf in &mut q {
        tf.rotate_local_y(dr);
    }
}

pub fn sys_slide_obstacles(
    time: Res<Time>,
    cfg: Res<ObstacleConfig>,
    mut q: Query<(&SlidingObstacle, &mut Transform)>,
) {
    if !cfg.slider.enabled {
        return;
    }
    let t = time.elapsed_secs();
    let spin = time.delta_secs() * cfg.slider.spin_speed;
    for (s, mut tf) in &mut q {
        let off = cfg.slider.amplitude * (t * cfg.slider.freq + s.phase).sin();
        tf.translation = s.base + s.axis * off;
        // Le maillet tourne aussi sur son axe Y en glissant (demande user).
        tf.rotate_local_y(spin);
    }
}

/// Repousse le joueur tant qu'il **chevauche** un obstacle — même à l'arrêt. Le
/// KCC empêche déjà de traverser quand le joueur BOUGE ; ici on couvre le cas
/// « un obstacle vient percuter un joueur immobile ». Détection via le VRAI
/// collider (Rapier `intersection_with_shape` sur la capsule joueur, filtrée aux
/// obstacles) → exact même pour la tête de marteau qui balaie. Le push déplace
/// directement le Transform du joueur (KinematicPositionBased) en `GameSet::Physics`
/// (après l'anim) ; le solve KCC (PostUpdate) repart de la position poussée.
pub fn sys_obstacle_push(
    time: Res<Time>,
    cfg: Res<ObstacleConfig>,
    rapier: ReadRapierContext,
    q_obstacles: Query<Entity, With<AnimatedObstacle>>,
    q_obs_gt: Query<&GlobalTransform>,
    mut q_player: Query<(&mut Transform, &Collider), With<Player>>,
    mut push: ResMut<ObstaclePushState>,
    mut set_buf: Local<HashSet<Entity>>,
) {
    push.pushing = false;
    if !cfg.push.enabled {
        return;
    }
    let Ok((mut tf, shape)) = q_player.single_mut() else {
        return;
    };
    let Ok(ctx) = rapier.single() else {
        return;
    };
    set_buf.clear();
    set_buf.extend(q_obstacles.iter());
    if set_buf.is_empty() {
        return;
    }
    let pos = tf.translation;
    let rot = tf.rotation;
    // Predicate : ne considérer QUE les colliders d'obstacles animés.
    let pred = |e: Entity| set_buf.contains(&e);
    let filter = QueryFilter::default().predicate(&pred);
    // intersect_shape (bevy_rapier 0.33) = callback par entité chevauchée ; on
    // garde la 1re (return false = stop).
    let mut obs_hit: Option<Entity> = None;
    ctx.intersect_shape(pos, rot, &*shape.raw, filter, |e| {
        obs_hit = Some(e);
        false
    });
    let Some(obs_e) = obs_hit else {
        return;
    };
    // Direction de répulsion : horizontalement, de l'obstacle vers le joueur.
    let obs_pos = q_obs_gt
        .get(obs_e)
        .map(|gt| gt.translation())
        .unwrap_or(pos);
    let mut dir = Vec3::new(pos.x - obs_pos.x, 0.0, pos.z - obs_pos.z);
    if dir.length_squared() < 1e-4 {
        // Joueur pile sous/au pivot → fallback : pousser vers l'arrière du joueur.
        dir = -tf.forward().as_vec3();
        dir.y = 0.0;
    }
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }
    tf.translation += dir * cfg.push.speed * time.delta_secs();
    push.pushing = true;
    push.last_dir = dir;
    push.events = push.events.saturating_add(1);
}

// ─── Sensor forgia2_obstacles.json ──────────────────────────────────────────

pub fn sys_write_obstacles_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Res<ObstacleConfig>,
    stats: Res<ObstacleStats>,
    push: Res<ObstaclePushState>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let total = stats.hammers + stats.spinners + stats.sliders;
    let (severity, next_step) = if total > 0 {
        ("ok", "")
    } else {
        (
            "warn",
            "0 obstacle animé — niveau pas (encore) marqué ou noms de nodes changés (classify)",
        )
    };

    let json = format!(
        r#"{{"id":"obstacles","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"counts":{{"hammers":{},"spinners":{},"sliders":{}}},"hammer":{{"enabled":{},"swing_deg":{:.1},"freq":{:.2}}},"spinner":{{"enabled":{},"speed":{:.2}}},"slider":{{"enabled":{},"amplitude":{:.2},"freq":{:.2},"spin_speed":{:.2}}},"push":{{"enabled":{},"speed":{:.2},"pushing":{},"events":{},"dir":[{:.2},{:.2}]}}}}"#,
        time.elapsed_secs(),
        stats.hammers,
        stats.spinners,
        stats.sliders,
        cfg.hammer.enabled,
        cfg.hammer.swing_deg,
        cfg.hammer.freq,
        cfg.spinner.enabled,
        cfg.spinner.speed,
        cfg.slider.enabled,
        cfg.slider.amplitude,
        cfg.slider.freq,
        cfg.slider.spin_speed,
        cfg.push.enabled,
        cfg.push.speed,
        push.pushing,
        push.events,
        push.last_dir.x,
        push.last_dir.z,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[obstacles] sensor write failed: {e}");
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ParcoursObstaclesPlugin;

impl Plugin for ParcoursObstaclesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ObstacleConfig>();
        app.init_resource::<ObstacleGenomeWatch>();
        app.init_resource::<ObstacleStats>();
        app.init_resource::<ObstaclePushState>();
        app.add_systems(Startup, sys_init_obstacle_genome);
        app.add_systems(OnEnter(GameMode::Roguelite), sys_reset_obstacle_stats);
        app.add_systems(
            Update,
            (
                sys_hot_reload_obstacle_genome,
                sys_swing_hammers,
                sys_rotate_obstacles,
                sys_slide_obstacles,
            )
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Push physique : APRÈS l'anim (Physics > Movement), AVANT le solve KCC
        // (PostUpdate). Repousse le joueur qui chevauche un obstacle (même à l'arrêt).
        app.add_systems(
            Update,
            sys_obstacle_push
                .in_set(GameSet::Physics)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_obstacles_sensor
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Roguelite)),
        );
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_obstacle_types() {
        assert_eq!(classify("obstacle_1_001.001"), Some(ObstacleAnim::Hammer));
        assert_eq!(classify("obstacle_3_001"), Some(ObstacleAnim::Spinner));
        assert_eq!(classify("obstacle_4_001.002"), Some(ObstacleAnim::Spinner));
        assert_eq!(classify("obstacle_6_001"), Some(ObstacleAnim::Spinner));
        assert_eq!(classify("obstacle_2_001.003"), Some(ObstacleAnim::Slider));
    }

    #[test]
    fn classify_leaves_platforms_and_unknown_static() {
        // obstacle_11 = plateforme (10×4.86×10) — NE doit PAS matcher Hammer.
        assert_eq!(classify("obstacle_11_001"), None);
        assert_eq!(classify("obstacle_7_001"), None);
        assert_eq!(classify("obstacle_9_001"), None);
        assert_eq!(classify("platform_001"), None);
        assert_eq!(classify("circle_001"), None);
    }

    #[test]
    fn phase_is_deterministic_and_bounded() {
        let p = Vec3::new(12.0, 4.0, -57.0);
        let a = phase_from_pos(p);
        let b = phase_from_pos(p);
        assert_eq!(a, b, "déterministe");
        assert!((0.0..TAU).contains(&a), "borné [0,TAU)");
        // Deux positions distinctes → phases distinctes (désync).
        assert_ne!(
            phase_from_pos(p),
            phase_from_pos(Vec3::new(0.0, 4.0, -30.0))
        );
    }

    #[test]
    fn default_matches_sane_ranges() {
        let c = ObstacleConfig::default();
        assert!(c.hammer.enabled && c.hammer.swing_deg > 0.0 && c.hammer.freq > 0.0);
        assert!(c.spinner.speed > 0.0);
        assert!(c.slider.amplitude > 0.0 && c.slider.spin_speed > 0.0);
        assert!(c.push.enabled && c.push.speed > 0.0);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        assert_eq!(
            ObstacleConfig::parse_toml("pas du toml [[["),
            ObstacleConfig::default()
        );
    }
}

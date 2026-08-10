//! death_ascension.rs — l'envol des morts (2026-08-05).
//!
//! Quand un ennemi meurt, son corps ne disparaît plus sèchement : il **bascule,
//! se redresse, monte au ciel en accélérant et rétrécit**, en laissant une
//! **traînée lumineuse** derrière lui.
//!
//! ## Origine — c'est le comportement V1, pas une invention
//!
//! Le RPG V1 (`forgia-game/src/combat/mod.rs::goblin_death_system`) faisait déjà
//! exactement cela, en 3 phases (`DyingPhase` Falling/OnGround/Ascending) : chute
//! avant de −90° en X, temps au sol, puis redressement + montée accélérée de
//! 1 → 6 m/s avec `scale` 1.0 → 0.2. **Aucune impulsion physique n'a jamais
//! existé** : tout est piloté en `Transform`, et c'est justement ce qui rend
//! l'effet lisible et déterministe. On reprend le même modèle.
//!
//! Ce qui CHANGE par rapport au V1 :
//! - Les durées V1 (0.8 / **10.0** / 3.0 s) sont calibrées pour un RPG. Dans une
//!   arène roguelite où une vague entière meurt en quelques secondes, 10 s de
//!   cadavre au sol empileraient les corps. Les défauts ici sont resserrés, et
//!   tout est en couche definition (genome, hot-reload).
//! - Le V1 accrochait un émetteur de particules en ENFANT du corps. On produit
//!   ici une vraie **traînée continue** : des segments de cylindre émissifs
//!   tendus entre deux positions successives, qui rétrécissent jusqu'à zéro.
//!
//! ## Perf — la leçon des tracers (story-432)
//!
//! `weapon_vfx/tracer.rs` documente un freeze de 82 ms causé par la création
//! d'assets par tir. Ici : **un seul mesh** (cylindre unitaire) et **un material
//! par teinte** (4 archétypes = 4 materials, créés une fois puis mis en cache).
//! Le fondu se fait en **rétrécissant le Transform**, jamais en mutant un
//! material par segment — donc zéro `Assets::add()` dans le chemin chaud.
//!
//! ## Contrat d'utilisation (côté appelant)
//!
//! 1. À l'apparition, poser [`AscendsOnDeath`] sur l'entité.
//! 2. À la mort, l'appelant **retire ce qui doit cesser** (IA, collision, ciblage)
//!    puis insère [`Ascending::new`]. C'est l'appelant qui strippe, parce que lui
//!    seul connaît ses composants.
//! 3. Ce module fait le reste, y compris le `despawn` final.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use serde::Deserialize;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_gamefeel.toml";
const SENSOR_PATH: &str = "forgia2_death_ascension.json";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Garde-fou : au-delà, on cesse d'émettre des segments pour cette frame.
/// Une vague entière qui meurt d'un coup ne doit pas noyer le rendu.
const MAX_TRAIL_SEGMENTS: usize = 512;

// ─── Composants ───────────────────────────────────────────────────────────────

/// Opt-in : cette entité **s'envole** à sa mort au lieu de disparaître.
///
/// `tint` colore la traînée — on la dérive de l'archétype côté appelant, pour
/// que chaque famille d'ennemi laisse sa propre signature lumineuse.
#[derive(Component, Clone, Copy, Debug)]
pub struct AscendsOnDeath {
    pub tint: LinearRgba,
}

impl Default for AscendsOnDeath {
    fn default() -> Self {
        // Doré chaud — la teinte « âme » du V1 (`ascend_sparkles`).
        Self {
            tint: LinearRgba::new(4.0, 3.0, 1.2, 1.0),
        }
    }
}

/// Les trois temps de l'envol. Reprise fidèle de `DyingPhase` (V1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscendPhase {
    /// Le corps bascule vers l'avant et s'affaisse.
    Falling,
    /// Bref temps mort au sol — c'est lui qui rend la montée lisible.
    OnGround,
    /// Redressement + montée accélérée + rétrécissement.
    Rising,
}

/// État runtime de l'envol, inséré au moment de la mort.
#[derive(Component)]
pub struct Ascending {
    pub phase: AscendPhase,
    timer: Timer,
    original_rotation: Quat,
    base_y: f32,
    /// Dernière position où un segment de traînée a été émis (échantillonnage
    /// par DISTANCE, pas par frame : la traînée est indépendante du framerate).
    last_emit: Vec3,
}

impl Ascending {
    /// À insérer par l'appelant au moment de la mort.
    ///
    /// `original_rotation` et `base_y` sont lus sur le `Transform` du corps —
    /// la bascule et la montée s'expriment RELATIVEMENT à eux (sinon un ennemi
    /// mort en pente se redresserait de travers).
    pub fn new(original_rotation: Quat, base_y: f32, pos: Vec3, fall_secs: f32) -> Self {
        Self {
            phase: AscendPhase::Falling,
            timer: Timer::from_seconds(fall_secs.max(0.01), TimerMode::Once),
            original_rotation,
            base_y,
            last_emit: pos,
        }
    }
}

/// Un segment de traînée : il ne fait que rétrécir jusqu'à disparaître.
#[derive(Component)]
pub struct TrailSegment {
    life: Timer,
    /// Rayon initial, en mètres — le fondu est un `lerp` vers 0 sur ce rayon.
    radius: f32,
    length: f32,
}

// ─── Tuning (genome plat, même fichier que le knockback) ──────────────────────

#[derive(Resource, Debug, Clone, Copy)]
pub struct DeathAscensionTuning {
    pub fall_secs: f32,
    pub ground_secs: f32,
    pub rise_secs: f32,
    /// Vitesse de montée au début puis à la fin (le V1 allait de 1 → 6 m/s).
    pub rise_speed_start: f32,
    pub rise_speed_end: f32,
    /// Affaissement pendant la chute (V1 : 1.0 m, codé en dur là-bas).
    pub fall_drop_m: f32,
    /// Échelle finale avant disparition (V1 : 0.2).
    pub shrink_to: f32,
    /// Distance entre deux segments de traînée.
    pub trail_step_m: f32,
    pub trail_life_secs: f32,
    pub trail_radius_m: f32,
}

impl Default for DeathAscensionTuning {
    fn default() -> Self {
        Self {
            // Resserré vs V1 (0.8 / 10.0 / 3.0) : rythme d'arène, pas de RPG.
            fall_secs: 0.35,
            ground_secs: 0.25,
            rise_secs: 1.30,
            rise_speed_start: 1.5,
            rise_speed_end: 7.0,
            fall_drop_m: 0.55,
            shrink_to: 0.15,
            trail_step_m: 0.18,
            trail_life_secs: 0.45,
            trail_radius_m: 0.16,
        }
    }
}

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct GamefeelGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

impl DeathAscensionTuning {
    /// Pur — genes plats, ids préfixés `death_`. Même fichier et même forme que
    /// le knockback (`forgia-juice-lib`), pour qu'il n'y ait qu'UN genome de
    /// game feel roguelite à connaître.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: GamefeelGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut t = Self::default();
        for gene in &parsed.genes {
            let v = gene.default;
            match gene.id.as_str() {
                "death_fall_secs" => t.fall_secs = v.clamp(0.05, 3.0),
                "death_ground_secs" => t.ground_secs = v.clamp(0.0, 10.0),
                "death_rise_secs" => t.rise_secs = v.clamp(0.2, 8.0),
                "death_rise_speed_start" => t.rise_speed_start = v.clamp(0.0, 30.0),
                "death_rise_speed_end" => t.rise_speed_end = v.clamp(0.0, 40.0),
                "death_fall_drop_m" => t.fall_drop_m = v.clamp(0.0, 3.0),
                "death_shrink_to" => t.shrink_to = v.clamp(0.01, 1.0),
                "death_trail_step_m" => t.trail_step_m = v.clamp(0.04, 2.0),
                "death_trail_life_secs" => t.trail_life_secs = v.clamp(0.05, 3.0),
                "death_trail_radius_m" => t.trail_radius_m = v.clamp(0.01, 1.0),
                _ => {}
            }
        }
        t
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

// ─── Ressources partagées (anti-churn d'assets) ───────────────────────────────

/// Mesh unique + materials mis en cache par teinte.
///
/// Le mesh est un cylindre **unitaire** (rayon 1, hauteur 1) aligné sur Y : un
/// segment se pose en `scale = (rayon, longueur, rayon)` et une rotation qui
/// aligne Y sur la direction du mouvement. Une seule géométrie pour tout le jeu.
#[derive(Resource)]
pub struct AscensionTrailAssets {
    mesh: Handle<Mesh>,
    /// Clé = bits de la teinte. 4 archétypes → 4 materials, créés une seule fois.
    materials: HashMap<u32, Handle<StandardMaterial>>,
}

impl AscensionTrailAssets {
    fn material_for(
        &mut self,
        materials: &mut Assets<StandardMaterial>,
        tint: LinearRgba,
    ) -> Handle<StandardMaterial> {
        let key = tint_key(tint);
        if let Some(h) = self.materials.get(&key) {
            return h.clone();
        }
        let handle = materials.add(StandardMaterial {
            base_color: Color::srgba(tint.red * 0.25, tint.green * 0.25, tint.blue * 0.25, 1.0),
            emissive: tint,
            // Additif : une traînée est de la LUMIÈRE, elle s'ajoute au décor.
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            ..default()
        });
        self.materials.insert(key, handle.clone());
        handle
    }
}

/// Quantifie une teinte en clé de cache (2 décimales par canal suffisent : on
/// veut regrouper les archétypes, pas distinguer des nuances imperceptibles).
fn tint_key(t: LinearRgba) -> u32 {
    let q = |v: f32| ((v.clamp(0.0, 15.0) * 16.0) as u32) & 0xFF;
    (q(t.red) << 16) | (q(t.green) << 8) | q(t.blue)
}

#[derive(Resource, Default)]
pub struct DeathAscensionStats {
    /// Envols démarrés depuis le lancement (cumul — un capteur instantané ne
    /// prouverait rien une fois la vague finie).
    pub started_total: u32,
    pub completed_total: u32,
    pub active_now: u32,
    pub segments_now: u32,
    pub segments_total: u32,
    pub reload_count: u32,
}

#[derive(Resource)]
struct TuningWatch {
    last_mtime: Option<SystemTime>,
    accum: f32,
}

// ─── Courbe ───────────────────────────────────────────────────────────────────

/// `ease_out_quad` — la courbe du V1 (`combat/mod.rs:583`). Départ franc, fin
/// douce : c'est elle qui donne l'impression que le corps « part ».
pub fn ease_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Hauteur gagnée et vitesse courante de la montée, pour un avancement `t`.
/// Extrait pur : c'est la seule vraie règle de l'envol, elle doit être testable
/// sans moteur.
pub fn rise_speed_at(t: f32, start: f32, end: f32) -> f32 {
    start + (end - start) * ease_out_quad(t)
}

// ─── Systèmes ─────────────────────────────────────────────────────────────────

fn sys_setup_trail_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    // Cylindre unitaire : rayon 1, hauteur 1, axe Y (convention Bevy).
    let mesh = meshes.add(Cylinder::new(1.0, 1.0));
    commands.insert_resource(AscensionTrailAssets {
        mesh,
        materials: HashMap::default(),
    });
    commands.insert_resource(DeathAscensionTuning::load_or_default());
    commands.insert_resource(TuningWatch {
        last_mtime: fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok(),
        accum: 0.0,
    });
}

fn sys_hot_reload_tuning(
    time: Res<Time>,
    mut watch: ResMut<TuningWatch>,
    mut tuning: ResMut<DeathAscensionTuning>,
    mut stats: ResMut<DeathAscensionStats>,
) {
    watch.accum += time.delta_secs();
    if watch.accum < POLL_PERIOD_SEC {
        return;
    }
    watch.accum = 0.0;
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    if mtime == watch.last_mtime {
        return;
    }
    watch.last_mtime = mtime;
    *tuning = DeathAscensionTuning::load_or_default();
    stats.reload_count = stats.reload_count.saturating_add(1);
    info!("[death-ascension] tuning rechargé ({GENOME_PATH})");
}

/// Fait avancer les trois phases et émet la traînée pendant la montée.
#[allow(clippy::too_many_arguments)]
fn sys_advance_ascension(
    mut commands: Commands,
    time: Res<Time>,
    tuning: Res<DeathAscensionTuning>,
    mut assets: ResMut<AscensionTrailAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut stats: ResMut<DeathAscensionStats>,
    mut q: Query<(Entity, &mut Transform, &mut Ascending, Option<&AscendsOnDeath>)>,
) {
    let dt = time.delta_secs();
    let mut active = 0u32;
    let mut emitted_this_frame = 0usize;

    for (entity, mut xf, mut dying, tint_src) in &mut q {
        active += 1;
        dying.timer.tick(time.delta());

        match dying.phase {
            AscendPhase::Falling => {
                // Bascule avant + affaissement, relatifs à la pose de mort.
                let t = ease_out_quad(dying.timer.fraction());
                let fall = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * t);
                xf.rotation = dying.original_rotation * fall;
                xf.translation.y = dying.base_y - tuning.fall_drop_m * t;
                if dying.timer.is_finished() {
                    dying.phase = AscendPhase::OnGround;
                    dying.timer =
                        Timer::from_seconds(tuning.ground_secs.max(0.01), TimerMode::Once);
                }
            }
            AscendPhase::OnGround => {
                if dying.timer.is_finished() {
                    dying.phase = AscendPhase::Rising;
                    dying.timer = Timer::from_seconds(tuning.rise_secs.max(0.05), TimerMode::Once);
                    // La traînée part d'ici, pas de la position d'avant la chute.
                    dying.last_emit = xf.translation;
                }
            }
            AscendPhase::Rising => {
                let t = dying.timer.fraction();
                let eased = ease_out_quad(t);
                // Se redresse pendant qu'il monte (−90° → 0).
                let angle = -std::f32::consts::FRAC_PI_2 * (1.0 - eased);
                xf.rotation = dying.original_rotation * Quat::from_rotation_x(angle);
                xf.translation.y +=
                    rise_speed_at(t, tuning.rise_speed_start, tuning.rise_speed_end) * dt;
                let scale = (1.0 - t * (1.0 - tuning.shrink_to)).max(tuning.shrink_to);
                xf.scale = Vec3::splat(scale);

                // Traînée : échantillonnée par DISTANCE (indépendante du framerate).
                let delta = xf.translation - dying.last_emit;
                if delta.length() >= tuning.trail_step_m
                    && stats.segments_now as usize + emitted_this_frame < MAX_TRAIL_SEGMENTS
                {
                    let tint = tint_src.copied().unwrap_or_default().tint;
                    let mat = assets.material_for(&mut materials, tint);
                    spawn_trail_segment(
                        &mut commands,
                        assets.mesh.clone(),
                        mat,
                        dying.last_emit,
                        xf.translation,
                        // La traînée s'affine avec le corps qui rétrécit.
                        tuning.trail_radius_m * scale,
                        tuning.trail_life_secs,
                    );
                    dying.last_emit = xf.translation;
                    emitted_this_frame += 1;
                    stats.segments_total = stats.segments_total.saturating_add(1);
                }

                if dying.timer.is_finished() {
                    stats.completed_total = stats.completed_total.saturating_add(1);
                    if let Ok(mut ec) = commands.get_entity(entity) {
                        ec.try_despawn();
                    }
                }
            }
        }
    }
    stats.active_now = active;
}

/// Pose un segment tendu entre deux positions successives.
fn spawn_trail_segment(
    commands: &mut Commands,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    from: Vec3,
    to: Vec3,
    radius: f32,
    life_secs: f32,
) {
    let delta = to - from;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let rotation = Quat::from_rotation_arc(Vec3::Y, delta / length);
    commands.spawn((
        Name::new("DeathTrailSegment"),
        TrailSegment {
            life: Timer::from_seconds(life_secs.max(0.02), TimerMode::Once),
            radius,
            length,
        },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform {
            translation: from + delta * 0.5,
            rotation,
            scale: Vec3::new(radius, length, radius),
        },
    ));
}

/// Le fondu de la traînée est un RÉTRÉCISSEMENT, pas une mutation de material :
/// c'est ce qui garde un seul material par teinte (cf. leçon des tracers).
fn sys_fade_trail(
    mut commands: Commands,
    time: Res<Time>,
    mut stats: ResMut<DeathAscensionStats>,
    mut q: Query<(Entity, &mut Transform, &mut TrailSegment)>,
) {
    let mut alive = 0u32;
    for (entity, mut xf, mut seg) in &mut q {
        seg.life.tick(time.delta());
        if seg.life.is_finished() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            continue;
        }
        alive += 1;
        let remaining = 1.0 - seg.life.fraction();
        let r = seg.radius * remaining;
        xf.scale = Vec3::new(r, seg.length, r);
    }
    stats.segments_now = alive;
}

fn sys_write_sensor(
    time: Res<Time>,
    stats: Res<DeathAscensionStats>,
    tuning: Res<DeathAscensionTuning>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    // `started_total == 0` alors que des ennemis meurent = l'appelant n'insère
    // pas `Ascending` : c'est le seul échec silencieux possible ici, il doit se
    // lire dans le capteur (règle observability-required).
    let (severity, next_step) = if stats.started_total == 0 {
        (
            "info",
            "Aucun envol démarré — normal hors run ; sinon vérifier que le mode insère Ascending à la mort.",
        )
    } else {
        ("ok", "-")
    };
    let json = format!(
        r#"{{"id":"death_ascension","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"started_total":{},"completed_total":{},"active_now":{},"segments_now":{},"segments_total":{},"fall_secs":{:.2},"ground_secs":{:.2},"rise_secs":{:.2},"reload_count":{}}}"#,
        time.elapsed_secs(),
        stats.started_total,
        stats.completed_total,
        stats.active_now,
        stats.segments_now,
        stats.segments_total,
        tuning.fall_secs,
        tuning.ground_secs,
        tuning.rise_secs,
        stats.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[death-ascension] sensor write failed: {e}");
    }
}

// ─── Plugin ───────────────────────────────────────────────────────────────────

pub struct DeathAscensionPlugin;

impl Plugin for DeathAscensionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DeathAscensionStats>()
            .add_systems(Startup, sys_setup_trail_assets)
            .add_systems(
                Update,
                (
                    sys_hot_reload_tuning,
                    sys_advance_ascension,
                    sys_fade_trail,
                    sys_write_sensor,
                )
                    .chain()
                    // Les ressources naissent au Startup : sans cette garde, la
                    // toute première frame paniquerait sur `Res<...>` absente.
                    .run_if(resource_exists::<AscensionTrailAssets>),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_quad_matches_v1_curve() {
        // Formule V1 verbatim : 1 - (1-t)².
        assert!((ease_out_quad(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_quad(1.0) - 1.0).abs() < 1e-6);
        assert!((ease_out_quad(0.5) - 0.75).abs() < 1e-6);
        // Monotone et bornée, y compris hors [0,1].
        assert_eq!(ease_out_quad(-1.0), 0.0);
        assert_eq!(ease_out_quad(2.0), 1.0);
    }

    #[test]
    fn rise_accelerates_from_start_to_end() {
        let (a, b) = (1.5, 7.0);
        assert!((rise_speed_at(0.0, a, b) - a).abs() < 1e-6);
        assert!((rise_speed_at(1.0, a, b) - b).abs() < 1e-6);
        // À mi-course l'ease_out a déjà donné l'essentiel de l'accélération :
        // c'est ce qui fait « partir » le corps au lieu de le faire flotter.
        assert!(rise_speed_at(0.5, a, b) > (a + b) * 0.5);
    }

    #[test]
    fn tuning_parses_death_genes_and_ignores_others() {
        let t = DeathAscensionTuning::parse_toml(
            r#"
            [[genes]]
            id = "knockback_base_m"
            default = 0.3

            [[genes]]
            id = "death_rise_secs"
            default = 2.5

            [[genes]]
            id = "death_trail_step_m"
            default = 0.4
            "#,
        );
        assert!((t.rise_secs - 2.5).abs() < 1e-6);
        assert!((t.trail_step_m - 0.4).abs() < 1e-6);
        // Les genes d'un autre système ne doivent rien perturber.
        assert_eq!(t.fall_secs, DeathAscensionTuning::default().fall_secs);
    }

    #[test]
    fn tuning_falls_back_on_invalid_toml() {
        let t = DeathAscensionTuning::parse_toml("ceci n'est pas du TOML {{{");
        assert_eq!(t.rise_secs, DeathAscensionTuning::default().rise_secs);
    }

    #[test]
    fn out_of_range_genes_are_clamped_not_trusted() {
        let t = DeathAscensionTuning::parse_toml(
            r#"
            [[genes]]
            id = "death_rise_secs"
            default = 9999.0

            [[genes]]
            id = "death_shrink_to"
            default = -5.0
            "#,
        );
        assert!(t.rise_secs <= 8.0, "durée bornée");
        assert!(t.shrink_to > 0.0, "une échelle nulle ferait disparaître le corps");
    }

    #[test]
    fn distinct_tints_get_distinct_cache_keys() {
        // Deux archétypes de couleurs différentes ne doivent pas partager un
        // material (sinon la signature colorée par famille disparaît).
        let gold = LinearRgba::new(4.0, 3.0, 1.2, 1.0);
        let teal = LinearRgba::new(0.4, 2.6, 3.2, 1.0);
        assert_ne!(tint_key(gold), tint_key(teal));
        // …et deux teintes identiques doivent partager la même clé.
        assert_eq!(tint_key(gold), tint_key(LinearRgba::new(4.0, 3.0, 1.2, 0.5)));
    }
}

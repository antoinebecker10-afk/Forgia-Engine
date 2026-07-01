//! waves.rs — M2 step 4 : système 3-wave end-to-end Roguelite.
//!
//! Pipeline :
//! 1. OnEnter(GameMode::Roguelite) : `RogueliteWave::default()` Resource inséré
//! 2. wave 1 spawn auto (caller : `sys_spawn_roguelite_scene` appelle `spawn_wave_enemies(1)`)
//! 3. `sys_wave_orchestrator` poll Query<&ArenaBot> chaque frame :
//!    - si bots_alive == 0 ET wave en cours : démarre break (3s)
//!    - quand break_secs_left <= 0 : spawn next wave (current_wave +=1)
//!    - quand current_wave >= WAVES_TOTAL : `boss_defeated=true` → la porte du
//!      socle s'ouvre (story-603 ; plus d'`EndRunEvent(Victory)` auto)
//!
//! Scaling : wave 1 = 8 ennemis (3T/3R/2S), wave 2 = 12 (4T/4R/4S), wave 3 = 16 (6T/6R/4S).

use crate::enemies::{self, EnemyArchetype};
use crate::run::RogueliteRunMarker;
use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody, Sensor};
use forgia_ai_arena_bot::ArenaBot;
use forgia_core::prelude::*;
use forgia_rpg_data::boons::OpenCoffreRequest;
// Story-490 — Health type swap forgia_damage → forgia_combat pour matcher la
// query `find_health_ancestor` de forgia-fps hitscan (qui scanne
// `Query<&mut forgia_combat::Health, With<TargetCube>>`). Sans ce swap, type
// mismatch silencieux → hits classifiés `BlockerNonZone` au lieu de damage.
// cf memory [[reference-dual-health-type-trap]] et [[reference-bevy-rapier-child-collider-pattern-2026-05-20]].
use crate::defense::DefenseConfig;
use crate::enemies::EnemyStatsConfig;
use forgia_combat::Health;
use forgia_damage::Mortal;
use forgia_mode_fps_arena::TargetCube;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;

pub const WAVES_TOTAL: u8 = 3;
// Story-558 Phase 1 (2026-05-29) — break 3.0 → 15.0s.
// 15s = window prep ammo/heal + Coffre du Forgeron (Phase 3) + HP reset (AC10).
// Best practice industry (audit roguelite-engagement-2026-05-29 §1) : break
// court (3s) ne laisse pas le temps de respirer pour cible enfants/femmes ;
// 15s = sweet spot Hadès Chamber transition.
pub const BREAK_SECS: f32 = 15.0;
const WAVE_BASE_SEED: u64 = 0xC0FF_EE51_C0BA_1700;

#[derive(Resource, Debug, Clone)]
pub struct RogueliteWave {
    pub current_wave: u8,
    pub bots_alive: u32,
    pub break_secs_left: f32,
    /// True quand on est en train d'attendre le prochain spawn (entre 2 vagues).
    pub in_break: bool,
    pub victory_emitted: bool,
    /// Story-603 — true dès que la vague finale (boss) est nettoyée. Ouvre la
    /// porte du socle (`loot_room::sys_reconcile_boss_gate`). Remplace l'ancienne
    /// émission `EndRunEvent(Victory)` (décision user 2026-06-17 : pas de victoire
    /// auto, boucle boss → porte → parcours → arène). Reset au start de run.
    pub boss_defeated: bool,
}

impl Default for RogueliteWave {
    fn default() -> Self {
        Self {
            current_wave: 1,
            bots_alive: 0,
            break_secs_left: 0.0,
            in_break: false,
            victory_emitted: false,
            boss_defeated: false,
        }
    }
}

/// Composition par vague : `(archetype, count, ring_radius)`.
pub fn wave_composition(wave: u8) -> Vec<(EnemyArchetype, u32, f32)> {
    match wave {
        1 => vec![
            (EnemyArchetype::Tank, 3, 12.0),
            (EnemyArchetype::Runner, 3, 25.0),
            (EnemyArchetype::Sniper, 2, 50.0),
        ],
        2 => vec![
            (EnemyArchetype::Tank, 4, 14.0),
            (EnemyArchetype::Runner, 4, 28.0),
            (EnemyArchetype::Sniper, 4, 55.0),
        ],
        _ => vec![
            // Wave 3 (final boss — M3 step 1) : 1 boss + 4 ennemis support pour
            // pression de zone (cohérent climax RoR2 / Hadès).
            (EnemyArchetype::Boss, 1, 12.0),
            (EnemyArchetype::Runner, 4, 28.0),
        ],
    }
}

/// Spawn N ennemis de la composition wave donnée. Caller : OnEnter scene ou
/// orchestrator au passage de vague.
///
/// Story-517 (2026-05-26) : visual KayKit Skeleton SceneRoot + head proxy
/// sensor pour permettre headshots (zone-based damage multiplier via
/// `forgia_damage::HitZoneTag`). Capsule physique conservée (collision OK).
pub fn spawn_wave_enemies(
    commands: &mut Commands,
    asset_server: &AssetServer,
    // Story-640 P0-2 — stats LIVE (hot-reload) au lieu du Default : un hot-reload de
    // `roguelite_enemies.toml` change désormais les ennemis spawnés (spawn_live=true).
    stats_cfg: &EnemyStatsConfig,
    // Story-640 P0-2 — couche défensive par archétype (bouclier/armure).
    def_cfg: &DefenseConfig,
    wave: u8,
) -> u32 {
    let composition = wave_composition(wave);
    let mut yaw_rng = Xoshiro256StarStar::seed_from_u64(WAVE_BASE_SEED ^ u64::from(wave));
    let mut total = 0u32;
    for (archetype, count, ring_radius) in &composition {
        let stats = stats_cfg.for_archetype(*archetype);
        let skeleton_handle: Handle<Scene> = asset_server.load(
            // KayKit GLB scene root : `#Scene0` est la convention Bevy GltfLoader.
            format!("{}#Scene0", enemies::skeleton_asset_path(*archetype)),
        );
        let scene_scale = enemies::skeleton_scale(*archetype);
        // Head proxy : sphère sensor ~80% en haut de la capsule body.
        // Y offset relatif au parent (qui est au centre de la capsule).
        let head_y_offset = stats.capsule_half_height * 0.85;
        let head_radius = stats.capsule_radius * 0.55;
        let yaw0 = (yaw_rng.next_u64() as f64 / u64::MAX as f64) as f32 * std::f32::consts::TAU;
        for i in 0..*count {
            let theta = yaw0 + (i as f32 / *count as f32) * std::f32::consts::TAU;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();
            let y = stats.capsule_half_height + stats.capsule_radius + 0.05;

            // Pattern miroir forgia-mode-fps-arena::wave::spawn_wave_bots:343 :
            // PARENT = Health + TargetCube + RigidBody + ArenaBot (PAS de Collider).
            // CHILD 1 = Collider body capsule (HitZone::Body default).
            // CHILD 2 = SceneRoot KayKit visual (skeleton GLB), rotation_y(PI) pour
            //           aligner KayKit +Z forward vs Bevy -Z forward
            //           (memory : reference_kaykit_skeleton_forward_axis_pi.md).
            // CHILD 3 = Head proxy sensor sphere (HitZone::Head), Y offset.
            let parent = commands
                .spawn((
                    Name::new(format!("RogueliteEnemy_W{wave}_{}_{i}", archetype.label())),
                    RogueliteRunMarker,
                    DespawnOnExit(GameMode::Roguelite),
                    *archetype,
                    TargetCube,
                    Transform::from_xyz(x, y, z),
                    RigidBody::KinematicPositionBased,
                    Health::new(stats.hp),
                    // Story-640 P0-2 — couche défensive (bouclier bleu / armure jaune)
                    // AU-DESSUS de la Vie. Le hit de base (forgia-fps) la draine avant
                    // `combat::Health` ; régén hors combat par `defense::sys_regen_defense`.
                    def_cfg.layer_for(*archetype),
                    Mortal,
                    // Story-640 P0-2 — bot config depuis la config LIVE (hot-reload).
                    stats_cfg.arena_bot(*archetype),
                    // Story-517 fix : ennemis n'avaient pas BotShootConfig → ne
                    // tiraient pas. Damage + range différencié par archetype.
                    stats_cfg.bot_shoot(*archetype),
                    // Story-636 — échantillon de vitesse pour le driver d'anim
                    // squelettique (marche vs course selon le déplacement réel).
                    crate::enemy_anim::EnemyLocoSample::default(),
                ))
                .id();
            // Body collider (capsule), classified HitZone::Body par défaut.
            // Story-517 fix : Sensor → player KCC passe à travers (no contact force)
            // mais raycast hitscan le détecte toujours (QueryFilter::default n'exclut
            // pas les sensors). Permet au joueur de traverser les ennemis en combat
            // rapproché tout en gardant le hitscan body-zone fonctionnel.
            commands.spawn((
                Name::new(format!(
                    "RogueliteEnemy_W{wave}_{}_{i}_body",
                    archetype.label()
                )),
                ChildOf(parent),
                Transform::default(),
                Collider::capsule_y(stats.capsule_half_height, stats.capsule_radius),
                Sensor,
            ));
            // Visual KayKit Skeleton SceneRoot (story-517 — remplace Capsule3d).
            commands
                .spawn((
                    Name::new(format!(
                        "RogueliteEnemy_W{wave}_{}_{i}_visual",
                        archetype.label()
                    )),
                    ChildOf(parent),
                    SceneRoot(skeleton_handle.clone()),
                    // KayKit forward = +Z, Bevy parent yaw uses -Z → rotate PI.
                    // Y offset : KayKit pivot au sol → translate down par
                    // (capsule_half_height + capsule_radius) pour aligner les
                    // pieds avec le BAS de la capsule parent (sinon lévitation).
                    Transform::from_xyz(0.0, -(stats.capsule_half_height + stats.capsule_radius), 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
                        .with_scale(Vec3::splat(scene_scale)),
                ))
                // Story-636 — au scene-ready : rend le mesh translucide (clone de
                // matériau dédupliqué) pour la viz de contrôle du rig.
                .observe(crate::enemy_rig_debug::on_enemy_scene_ready);
            // Head proxy sensor (story-517 headshot — sphère détectée AVANT capsule
            // body si ray traverse les deux. Tagué HitZone::Head → multiplier dégâts.
            commands.spawn((
                Name::new(format!(
                    "RogueliteEnemy_W{wave}_{}_{i}_head_proxy",
                    archetype.label()
                )),
                ChildOf(parent),
                Transform::from_xyz(0.0, head_y_offset, 0.0),
                Collider::ball(head_radius),
                Sensor,
                forgia_damage::HitZoneTag(forgia_damage::HitZone::Head),
            ));
            total += 1;
        }
    }
    info!("[roguelite] Wave {wave} spawned : {total} enemies");
    total
}

/// Tourne chaque frame en GameMode::Roguelite. Update bots_alive et orchestre
/// les transitions de vague.
///
/// 2026-05-29 — `seen_alive` Local<bool> gate anti-race :
/// `Commands.spawn()` est différé jusqu'au prochain ApplyDeferred. Si l'ordre
/// de schedule fait tourner orchestrator AVANT que les spawns de `sys_start_run`
/// (même frame) soient flushés, `Query<&ArenaBot>::iter().count() == 0` →
/// break déclenché à tort → wave 1 cleared instantanément (log montrait
/// `Wave 1 spawned` puis `Wave 1 cleared` à 78µs d'écart). Le gate exige
/// d'avoir VU au moins 1 frame avec `alive > 0` avant de pouvoir clear.
#[allow(clippy::too_many_arguments)]
pub fn sys_wave_orchestrator(
    time: Res<Time>,
    mut wave: ResMut<RogueliteWave>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_bots: Query<&ArenaBot>,
    mut open_coffre: MessageWriter<OpenCoffreRequest>,
    mut seen_alive: Local<bool>,
    // Story-571 — gain de Souls méta en fin de wave/boss (persistant).
    mut meta: ResMut<crate::run::MetaSouls>,
    // Story-640 P0-2 — configs live pour le spawn (stats hot-reload + défense).
    stats_cfg: Res<EnemyStatsConfig>,
    def_cfg: Res<DefenseConfig>,
) {
    let alive = q_bots.iter().count() as u32;
    wave.bots_alive = alive;

    if alive > 0 {
        *seen_alive = true;
    }

    // Victory déjà émise → no-op (évite spam events).
    if wave.victory_emitted {
        return;
    }

    if alive == 0 && *seen_alive && !wave.in_break {
        // Vague nettoyée — démarre break ou victory.
        if wave.current_wave >= WAVES_TOTAL {
            // Story-571 — bonus Souls méta pour le boss/finale (persistant).
            meta.current = meta.current.saturating_add(crate::run::SOULS_PER_BOSS);
            meta.earned_run = meta.earned_run.saturating_add(crate::run::SOULS_PER_BOSS);
            // Story-603 — décision user 2026-06-17 : PLUS d'écran Victoire auto.
            // Tuer le boss ouvre la porte du socle (`loot_room::sys_reconcile_boss_gate`
            // lit `boss_defeated`). `victory_emitted` reste le latch qui stoppe
            // l'orchestrateur (no-op au prochain tick) + gèle `obs_roguelite_player_death`.
            // Boucle : boss → porte → parcours → portail Retour → arène. Condition de
            // fin de run à brancher plus tard.
            wave.victory_emitted = true;
            wave.boss_defeated = true;
            info!(
                "[roguelite] All {WAVES_TOTAL} waves cleared — BOSS DEFEATED (+{} Souls méta) → porte du socle s'ouvre",
                crate::run::SOULS_PER_BOSS
            );
            return;
        }
        // Story-571 — Souls méta pour une wave régulière nettoyée (persistant).
        meta.current = meta.current.saturating_add(crate::run::SOULS_PER_WAVE);
        meta.earned_run = meta.earned_run.saturating_add(crate::run::SOULS_PER_WAVE);
        wave.in_break = true;
        wave.break_secs_left = BREAK_SECS;
        // Story-558 AC10 (2026-05-29) — HP restauré à 100% à l'entrée break.
        // Pattern Hadès "Charon's Boon" : sanctuary moment + window prep.
        // Bible cartoon : encourage risk-taking, pas de save-HP-for-next-wave.
        // commands.queue car forgia_damage::Health pas accessible en SystemParam
        // direct (cf miror pattern sys_start_run run.rs:446-451).
        commands.queue(|world: &mut World| {
            let mut q = world
                .query_filtered::<&mut forgia_damage::Health, With<forgia_player::Player>>();
            if let Ok(mut hp) = q.single_mut(world) {
                hp.current = hp.max;
            }
        });
        // Story-558 Phase 3 (2026-05-29) — ouvre le Coffre du Forgeron. UI
        // (forgia-ui-lib::hud::coffre_forgeron) lit CoffreSession populée par
        // sys_handle_open_coffre (forgia-rpg-data::boons).
        open_coffre.write(OpenCoffreRequest::wave_clear());
        info!(
            "[roguelite] Wave {} cleared — break {BREAK_SECS}s before wave {} (HP restored, Coffre opened)",
            wave.current_wave,
            wave.current_wave + 1
        );
    }

    if wave.in_break {
        wave.break_secs_left -= time.delta_secs();
        if wave.break_secs_left <= 0.0 {
            wave.current_wave += 1;
            wave.in_break = false;
            wave.break_secs_left = 0.0;
            spawn_wave_enemies(
                &mut commands,
                &asset_server,
                &stats_cfg,
                &def_cfg,
                wave.current_wave,
            );
            // Reset gate : la nouvelle wave doit prouver alive>0 avant pouvoir clear.
            *seen_alive = false;
        }
    }
}

/// M3 step 1 — marker enrage phase 2. Inséré quand HP boss < 50%.
#[derive(Component, Default)]
pub struct BossEnraged;

/// Story-558 P3 (2026-05-29) — Message fired par sys_boss_enrage au moment
/// du trigger (transition Without<BossEnraged> → With). Consommé par UI
/// banner + camera shake punch.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossEnrageTriggeredEvent;

/// Détecte boss à ≤50% HP → insert BossEnraged + boost stats AI runtime +
/// fire `BossEnrageTriggeredEvent` (P3 telegraph visuel).
/// Idempotent : `Without<BossEnraged>` filtre évite re-trigger.
pub fn sys_boss_enrage(
    mut commands: Commands,
    mut q_boss: Query<(Entity, &Health, &EnemyArchetype, &mut ArenaBot), Without<BossEnraged>>,
    mut enrage_w: MessageWriter<BossEnrageTriggeredEvent>,
) {
    for (entity, health, archetype, mut bot) in &mut q_boss {
        if *archetype != EnemyArchetype::Boss {
            continue;
        }
        // Story-490 — forgia_combat::Health n'a pas .fraction() ; inline calcul
        // (équivalent à forgia_damage::Health::fraction).
        let fraction = if health.max > 0.0 {
            (health.current / health.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if fraction <= 0.5 {
            let stats = enemies::stats_for(EnemyArchetype::Boss);
            bot.speed = stats.speed * 1.8;
            bot.attack_cooldown = stats.attack_cooldown * 0.55;
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.insert(BossEnraged);
            }
            // P3 telegraph — fire event consommé par UI banner + camera shake.
            enrage_w.write(BossEnrageTriggeredEvent);
            info!(
                "[roguelite] BOSS ENRAGED — phase 2 (HP {:.0}%, speed {:.1}, cooldown {:.2}s)",
                fraction * 100.0,
                bot.speed,
                bot.attack_cooldown
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_composition_grows() {
        let w1: u32 = wave_composition(1).iter().map(|(_, c, _)| *c).sum();
        let w2: u32 = wave_composition(2).iter().map(|(_, c, _)| *c).sum();
        let w3: u32 = wave_composition(3).iter().map(|(_, c, _)| *c).sum();
        assert!(w1 < w2, "wave 2 doit avoir plus d'ennemis que wave 1");
        assert_eq!(w1, 8);
        assert_eq!(w2, 12);
        assert_eq!(w3, 5, "wave 3 = 1 boss + 4 support");
    }

    #[test]
    fn wave_3_contains_boss() {
        let w3 = wave_composition(3);
        assert!(
            w3.iter().any(|(a, _, _)| *a == EnemyArchetype::Boss),
            "wave 3 doit contenir un Boss"
        );
    }

    #[test]
    fn wave_default_state() {
        let w = RogueliteWave::default();
        assert_eq!(w.current_wave, 1);
        assert_eq!(w.bots_alive, 0);
        assert!(!w.in_break);
        assert!(!w.victory_emitted);
        assert!(!w.boss_defeated);
    }

    #[test]
    fn waves_total_is_3() {
        assert_eq!(WAVES_TOTAL, 3);
    }

    #[test]
    fn break_secs_positive() {
        assert!(BREAK_SECS > 0.0);
    }
}

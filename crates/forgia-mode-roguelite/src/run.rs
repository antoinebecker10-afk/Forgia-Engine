//! run.rs — RunState SubStates + Events + RunSeed (V7 M1).
//!
//! Story-470. Pattern Bevy 0.18 :
//! - `SubStates` avec `#[source(GameMode = GameMode::Roguelite)]` (idiom 0.18)
//! - `Message` derive (renommé Event en 0.18 PR #19596) — story-468 §0.1 BufferedEvent
//! - `RunSeed` xoshiro déterministe (audit §0.2 — host-authoritative, pas float det rapier)
//!
//! Cleanup OnExit : `RogueliteRunMarker` Component stub — la logique de despawn est
//! gérée par un terminal parallèle (V7 dédié cleanup orchestration). Ce fichier
//! n'inclut PAS le system `sys_cleanup_run_markers` pour éviter conflit merge.

use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use forgia_core::prelude::*;
// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// use forgia_audio_voicelines::{BarkEvent, BarkKind};
use forgia_combat::prelude::{EquippedWeapons, WeaponType};
use forgia_damage::DeathEvent;
// TODO(story-471..479): PickupAnimState + PickupKind supprimés de forgia_loot_tables
// Story-571 — l'Or in-run EST le `Souls` de forgia-rpg-data, importé `as Gold`
// pour la clarté (la monnaie MÉTA persistante = `MetaSouls`, défini ici).
use forgia_rpg_data::loot_tables::{Pickup, PickupCollector, Souls as Gold};
use forgia_player::Player;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;

/// SubState de `GameMode::Roguelite` — flow de la run en cours.
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameMode = GameMode::Roguelite)]
pub enum RunState {
    /// Lobby pré-run : choix seed, options, ready check.
    #[default]
    Lobby,
    /// Run en cours sur le stage donné (0-indexed).
    InRun { stage: u8 },
    /// Combat boss sur le stage donné.
    Boss { stage: u8 },
    /// Défaite — retour menu après écran result.
    Defeat,
    /// Victoire — retour menu après écran result + récompenses.
    Victory,
}

/// Démarre une run. `seed = None` → seed dérivé timestamp (random).
#[derive(Message, Debug, Clone)]
pub struct StartRunEvent {
    pub seed: Option<u64>,
}

/// Termine la run avec résultat. Trigger transition RunState.
#[derive(Message, Debug, Clone)]
pub struct EndRunEvent {
    pub result: RunResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    Victory,
    Defeat,
    /// Abandon volontaire (return-to-lobby manuel).
    Abort,
}

/// Seed déterministe + état stage. Inséré au démarrage par `sys_start_run`.
#[derive(Resource, Debug, Clone)]
pub struct RunSeed {
    pub seed: u64,
    pub stage_count: u8,
}

impl RunSeed {
    /// Dérivation déterministe par stage : `seed XOR (stage * golden_ratio_u64)`.
    pub fn stage_seed(&self, stage: u8) -> u64 {
        let mixed = self.seed ^ u64::from(stage).wrapping_mul(0x9E3779B97F4A7C15);
        let mut rng = Xoshiro256StarStar::seed_from_u64(mixed);
        rng.next_u64()
    }

    /// Dérivation déterministe par encounter dans un stage.
    pub fn encounter_seed(&self, stage: u8, encounter_idx: u32) -> u64 {
        let mixed =
            self.stage_seed(stage) ^ u64::from(encounter_idx).wrapping_mul(0xBF58476D1CE4E5B9);
        let mut rng = Xoshiro256StarStar::seed_from_u64(mixed);
        rng.next_u64()
    }
}

/// Marker stub — placé sur toutes les entités à cleaner OnExit GameMode::Roguelite.
/// La logique de cleanup (`sys_cleanup_run_markers`) est gérée par un terminal
/// parallèle dédié — ne PAS dupliquer ici.
///
/// En attendant, les entités spawn par `sys_spawn_roguelite_scene` utilisent
/// `DespawnOnExit(GameMode::Roguelite)` Bevy 0.18 natif pour safety cleanup.
#[derive(Component, Default)]
pub struct RogueliteRunMarker;

/// Mapping V1 depth → stage_id. Pattern simple alternance + boss force volcanique.
///
/// Story-483 P2 (2026-05-20). V7 ne shippe que 2 stages TOML (`crypts_of_anvil`
/// + `forge_sanctum`). Mapping :
/// - Boss state → toujours `crypts_of_anvil` (le seul stage avec `boss_pad_required`)
/// - Pair depths (0, 2, ...) → `crypts_of_anvil` (Volcanic, boss-ready)
/// - Impair depths (1, 3, ...) → `forge_sanctum` (Plains, lighter biome)
///
/// V2 : étendre `roguelite_stages.toml` + remplacer ce mapping par lookup
/// `RunGraph::stages[depth].variants[chosen].stage_id_pool` (data-driven).
pub fn stage_id_for_depth(depth: u8, is_boss: bool) -> &'static str {
    if is_boss {
        return "crypts_of_anvil";
    }
    if depth.is_multiple_of(2) {
        "crypts_of_anvil"
    } else {
        "forge_sanctum"
    }
}

/// Story-483 V7 P2 (2026-05-20) — dispatcher stage_id sur transition RunState.
///
/// Watches `Res<State<RunState>>` et insère un `StageLoadRequest` dès qu'on
/// entre dans `InRun{stage}` ou `Boss{stage}`. Lobby = depth 0 (pre-run
/// visual). Defeat/Victory ne déclenchent rien (laisse stage visible jusqu'à
/// OnExit GameMode cleanup).
///
/// Idempotent : `Local<Option<(u8, bool)>>` track last_depth → no-op si
/// inchangé. Le spawn system côté stage-arena détecte stage_id change et
/// cleanup les entités du stage précédent avant spawn du nouveau.
pub fn sys_stage_dispatch(
    mut commands: Commands,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
    // Story-600 (re-appliqué 2026-06-17) : la requête peut avoir été retirée par
    // `cleanup_stage_arena` (OnExit Roguelite). À la ré-entrée au MÊME depth, le
    // `Local` idempotent bloquerait la ré-insertion → arène vide. On la (ré)insère
    // donc si absente, même si le depth n'a pas changé.
    request: Option<Res<forgia_stage::StageLoadRequest>>,
    mut last_depth: Local<Option<(u8, bool)>>,
) {
    let Some(state) = run_state.as_deref().map(|s| s.get()) else {
        return;
    };
    let key = match state {
        RunState::Lobby => Some((0u8, false)),
        RunState::InRun { stage } => Some((*stage, false)),
        RunState::Boss { stage } => Some((*stage, true)),
        // Defeat/Victory : keep last stage visible (no-op).
        _ => return,
    };
    // Idempotent SAUF si la requête a disparu (cleanup) → auto-réparation ré-entrée.
    if key == *last_depth && request.is_some() {
        return;
    }
    let Some((depth, is_boss)) = key else {
        return;
    };
    let stage_id = stage_id_for_depth(depth, is_boss);
    const FALLBACK_SEED: u64 = 0xC0FF_EE51_C0BA_1700;
    let seed = run_seed
        .as_ref()
        .map(|s| s.stage_seed(depth))
        .unwrap_or(FALLBACK_SEED);
    commands.insert_resource(forgia_stage::StageLoadRequest {
        stage_id: stage_id.to_string(),
        seed,
    });
    *last_depth = key;
    info!(
        "[roguelite] Stage dispatch → '{stage_id}' (depth={depth}, boss={is_boss}, seed={seed:#x})"
    );
}

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// parse_music_state désactivé : MusicState n'existe plus dans forgia_audio_music_state (scaffold vide).
// Stub retourne toujours None pour éviter les callsites de casser.
pub fn parse_music_state(_s: &str) -> Option<()> {
    None
}

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// sys_apply_stage_toggles désactivé : RequestMusicState n'existe plus dans
// forgia_audio_music_state (scaffold vide). Stub no-op.
pub fn sys_apply_stage_toggles(
    stage_result: Res<forgia_stage::StageLoadResult>,
    mut last_applied_id: Local<String>,
) {
    if stage_result.state != forgia_stage::StageState::Ready {
        return;
    }
    if stage_result.stage_id == *last_applied_id || stage_result.stage_id.is_empty() {
        return;
    }
    // Music toggle disabled — MusicState/RequestMusicState supprimés.
    // Weather override : log seulement (pas de consumer V1, future `forgia-weather`).
    if !stage_result.weather_override.is_empty() {
        info!(
            "[roguelite] Stage '{}' weather_override='{}' (consumer V2)",
            stage_result.stage_id, stage_result.weather_override
        );
    }
    *last_applied_id = stage_result.stage_id.clone();
}

/// Story-483 V7 P2 (2026-05-20) — `OnEnter(GameMode::Roguelite)` minimal.
///
/// Stations health/ammo (cross-stage gameplay, pas map-bound). Le stage
/// arena lui-même est piloté par `sys_stage_dispatch` (Update, watches
/// RunState transitions).
pub fn sys_spawn_roguelite_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    crate::stations::spawn_stations(&mut commands, &mut meshes, &mut materials);
}

/// V7 M3 step 4 (2026-05-20) — Observer DeathEvent ciblant le Player → Defeat.
///
/// Pipeline : bot tire (BotShootConfig) → `apply_damage` mute Health → trigger
/// DeathEvent quand HP=0 → cet observer match `target == Player entity` et émet
/// `EndRunEvent(Defeat)`. `sys_end_run` transitionne `RunState::Defeat`, ce qui
/// déclenche `draw_defeat_overlay` (gated). Pattern Hadès "die → return to lobby".
///
/// Idempotent : la victory_emitted latch dans RogueliteWave bloque double-emit
/// si trigger arrive après une Victory déjà émise.
pub fn obs_roguelite_player_death(
    event: On<DeathEvent>,
    q_player: Query<Entity, With<Player>>,
    run_state: Option<Res<State<RunState>>>,
    mut end_run: MessageWriter<EndRunEvent>,
    mut wave: ResMut<crate::waves::RogueliteWave>,
) {
    // Filter : death cible le Player (pas un bot).
    let Ok(player_entity) = q_player.single() else {
        return;
    };
    if event.target != player_entity {
        return;
    }
    // Gate sur run active (Lobby/Defeat/Victory : ignore).
    let active = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !active {
        return;
    }
    // Anti double-emit (si Victory déjà fired = run finie, n'override pas).
    if wave.victory_emitted {
        return;
    }
    wave.victory_emitted = true; // bloque transitions further en orchestrator
    end_run.write(EndRunEvent {
        result: RunResult::Defeat,
    });
    info!("[roguelite] Player died — DEFEAT");
}

/// Observer Bevy 0.18 — sur DeathEvent d'un ennemi Roguelite, spawn un Pickup
/// glowing à sa position. Value selon archetype (Tank > Sniper > Runner).
///
/// Pattern miroir : `forgia-ai-arena-bot::on_bot_death` (story-466).
#[allow(clippy::too_many_arguments)]
pub fn obs_roguelite_enemy_death(
    event: On<DeathEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemies_q: Query<(&Transform, &crate::EnemyArchetype)>,
    time: Res<Time>,
    q_player_hp: Query<&forgia_damage::Health, With<forgia_player::Player>>,
    equipped: Option<Res<EquippedWeapons>>,
    asset_server: Res<AssetServer>,
) {
    let target = event.target;
    let Ok((xf, archetype)) = enemies_q.get(target) else {
        return; // pas un ennemi Roguelite (probablement bot Arena → ignore)
    };

    // TODO(story-471..479): API removed, refactor abandonné — re-implémenter
    // BarkEvent/BarkKind supprimés de forgia_audio_voicelines (scaffold vide).
    // Le bloc bark est désactivé ci-dessous.
    let _ = &equipped; // suppress unused warning

    // V7 M3 step 3 — Heart drop scaled par player HP (low HP = chance plus haute).
    // Boss garantit toujours un Heart big. Sources : Hadès "Centaur Hearts" + RoR2.
    let player_hp_frac = q_player_hp
        .iter()
        .next()
        .map(|h| if h.max > 0.0 { h.current / h.max } else { 1.0 })
        .unwrap_or(1.0);

    // Seed pseudo-deterministic from time + entity bits (cheap xorshift).
    let entity_seed: u64 = target.to_bits();
    let mut seed =
        (time.elapsed_secs_f64() * 1000.0) as u64 ^ entity_seed.wrapping_mul(2_654_435_761);
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    let roll = (seed % 100) as u32; // 0..99

    // TODO(story-471..479): PickupKind supprimé de forgia_loot_tables — kind remplacé par bool is_heart
    let (is_heart, value, color, emissive) = match *archetype {
        crate::EnemyArchetype::Boss => (
            true,
            40_u32, // big heart : ~33% of max HP (player max 100).
            (1.0_f32, 0.30_f32, 0.30_f32),
            LinearRgba::new(1.8, 0.20, 0.20, 1.0),
        ),
        _ if player_hp_frac < 0.40 && roll < 35 => (
            // Low HP + 35% chance → heart drop (Hadès Centaur pattern).
            true,
            20,
            (1.0, 0.30, 0.30),
            LinearRgba::new(1.6, 0.20, 0.20, 1.0),
        ),
        crate::EnemyArchetype::Tank => (
            false,
            5,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
        crate::EnemyArchetype::Sniper => (
            false,
            3,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
        crate::EnemyArchetype::Runner => (
            false,
            2,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
    };

    let base_y = 0.85;
    let pos = xf.translation.with_y(base_y);
    // TODO(story-471..479): PickupKind supprimé — is_heart bool remplace kind == PickupKind::Heart
    let label = if is_heart {
        format!("RoguelitePickup_heart{value}")
    } else {
        format!("RoguelitePickup_{value}souls")
    };
    let parent_id = if is_heart {
        // Cœur : sphère emissive (inchangé).
        let core_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(color.0, color.1, color.2, 1.0),
            emissive,
            metallic: 0.0,
            perceptual_roughness: 0.4,
            ..default()
        });
        let core_mesh = meshes.add(Sphere::new(0.35));
        commands
            .spawn((
                Name::new(label),
                RogueliteRunMarker,
                DespawnOnExit(GameMode::Roguelite),
                Pickup {
                    value,
                    lifetime_secs: 30.0,
                    collect_radius: 2.5,
                },
                Mesh3d(core_mesh),
                MeshMaterial3d(core_mat),
                Transform::from_translation(pos),
            ))
            .id()
    } else {
        // Or : vraie PIÈCE GLB (Coin_001 pack Inferno) qui tourne. Scale ~ valeur.
        let coin = asset_server
            .load(GltfAssetLabel::Scene(0).from_asset("models/environment/inferno/Coin_001.glb"));
        let coin_scale = 1.3 + (value.min(60) as f32) * 0.01;
        commands
            .spawn((
                Name::new(label),
                RogueliteRunMarker,
                DespawnOnExit(GameMode::Roguelite),
                Pickup {
                    value,
                    lifetime_secs: 30.0,
                    collect_radius: 2.5,
                },
                SceneRoot(coin),
                Transform::from_translation(pos).with_scale(Vec3::splat(coin_scale)),
                crate::CoinSpin,
            ))
            .id()
    };
    // Halo glow uniquement pour le cœur — la pièce d'or n'a PAS de sphère autour.
    if is_heart {
        let halo_mat = materials.add(StandardMaterial {
            base_color: Color::srgba(color.0 * 0.6, color.1 * 0.6, color.2 * 0.6, 0.30),
            emissive: LinearRgba::new(
                emissive.red * 0.5,
                emissive.green * 0.5,
                emissive.blue * 0.5,
                1.0,
            ),
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            unlit: true,
            ..default()
        });
        let halo_mesh = meshes.add(Sphere::new(0.70));
        commands.spawn((
            ChildOf(parent_id),
            Name::new("PickupHalo"),
            Mesh3d(halo_mesh),
            MeshMaterial3d(halo_mat),
            Transform::default(),
        ));
    }

    // Wisps d'âme (Âmes méta) : le boss en lâche plusieurs, les ennemis normaux
    // parfois (~8%). Visuel bleu-teal fantomatique, distinct des pièces d'or.
    let wisp_count = if matches!(*archetype, crate::EnemyArchetype::Boss) {
        4
    } else if roll % 100 < 8 {
        1
    } else {
        0
    };
    for i in 0..wisp_count {
        let s = seed.wrapping_add((i as u64).wrapping_mul(97).wrapping_add(31));
        let ox = ((s % 13) as f32 - 6.0) * 0.35;
        let oz = (((s >> 4) % 13) as f32 - 6.0) * 0.35;
        spawn_soul_wisp(
            &mut commands,
            &mut meshes,
            &mut materials,
            pos + Vec3::new(ox, 0.0, oz),
            s,
        );
    }
}

/// Tag le Player avec PickupCollector pour que `forgia_loot_tables::sys_collect_pickups`
/// puisse trigger sur sa position.
///
/// V7 M2.5 (2026-05-20) — DOIT tourner en `Update + run_if(in_state(Roguelite))`,
/// PAS en `OnEnter`. Player est spawn par `forgia-player::OnEnter(AppMode::InGame)`
/// (autre plugin) et l'ordre OnEnter cross-plugin n'est PAS garanti par Bevy 0.18.
/// Pattern récurrent : voir memory `feedback_cross_plugin_onenter_race_pattern.md`.
/// Guard idempotent via `Without<PickupCollector>` → no-op après 1er tag.
pub fn sys_tag_player_as_collector(
    mut commands: Commands,
    q_player: Query<Entity, (With<Player>, Without<PickupCollector>)>,
) {
    for e in &q_player {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.insert(PickupCollector);
            info!("[roguelite] Player {e:?} tagged PickupCollector");
        }
    }
}

/// QoL — aimante les pickups (or + cœurs) vers le joueur quand l'écran de choix
/// (Coffre du Forgeron) est ouvert. Plus besoin de tourner la caméra pour
/// récupérer l'or restant : il vole vers le joueur (collecté par
/// `sys_collect_pickups` à l'arrivée, radius 2.5). Pattern Hadès/Vampire Survivors.
pub fn sys_magnetize_pickups_on_break(
    time: Res<Time>,
    coffre: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    q_player: Query<&Transform, With<PickupCollector>>,
    mut q_pickups: Query<&mut Transform, (With<Pickup>, Without<PickupCollector>, Without<SoulWisp>)>,
    mut q_wisps: Query<&mut Transform, (With<SoulWisp>, Without<PickupCollector>, Without<Pickup>)>,
) {
    let Some(coffre) = coffre else {
        return;
    };
    if !coffre.is_open {
        return;
    }
    let Some(player) = q_player.iter().next() else {
        return;
    };
    let ppos = player.translation;
    let step = 38.0 * time.delta_secs();
    for mut pt in &mut q_pickups {
        let to = ppos - pt.translation;
        let d = to.length();
        if d > 0.05 {
            pt.translation += to / d * step.min(d);
        }
    }
    for mut pt in &mut q_wisps {
        let to = ppos - pt.translation;
        let d = to.length();
        if d > 0.05 {
            pt.translation += to / d * step.min(d);
        }
    }
}

// ─── Wisps d'âme (Âmes méta) — pickup distinct des coins (Or) ─────────────────

/// Valeur en Âmes d'un wisp ramassé.
pub const SOUL_WISP_VALUE: u32 = 2;

/// Pickup wisp d'âme : flotte + bobbe + tourne, bleu-teal fantomatique.
/// Distinct des coins (Or) : donne des **Âmes** (`MetaSouls`), pas de l'Or.
#[derive(Component, Debug, Clone, Copy)]
pub struct SoulWisp {
    pub value: u32,
    pub collect_radius: f32,
    pub base_y: f32,
    pub phase: f32,
}

/// Spawn un wisp d'âme procédural (core sphère emissive + halo translucide +
/// pointe flamme) à `pos`. Flotte à ~1 m du sol.
pub fn spawn_soul_wisp(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    seed: u64,
) {
    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.85, 0.95),
        emissive: LinearRgba::new(0.3, 2.6, 3.2, 1.0),
        metallic: 0.0,
        perceptual_roughness: 0.3,
        ..default()
    });
    let halo_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.9, 1.0, 0.22),
        emissive: LinearRgba::new(0.2, 1.3, 1.6, 1.0),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        unlit: true,
        ..default()
    });
    let core_mesh = meshes.add(Sphere::new(0.26));
    let halo_mesh = meshes.add(Sphere::new(0.52));
    let tip_mesh = meshes.add(Cone {
        radius: 0.14,
        height: 0.46,
    });
    let base_y = 1.0;
    let phase = (seed % 628) as f32 * 0.01;
    let parent = commands
        .spawn((
            Name::new(format!("RogueliteSoulWisp_{SOUL_WISP_VALUE}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            SoulWisp {
                value: SOUL_WISP_VALUE,
                collect_radius: 2.5,
                base_y,
                phase,
            },
            Mesh3d(core_mesh),
            MeshMaterial3d(core_mat.clone()),
            Transform::from_translation(pos.with_y(base_y)),
        ))
        .id();
    commands.spawn((
        ChildOf(parent),
        Name::new("SoulWispHalo"),
        Mesh3d(halo_mesh),
        MeshMaterial3d(halo_mat),
        Transform::default(),
    ));
    commands.spawn((
        ChildOf(parent),
        Name::new("SoulWispTip"),
        Mesh3d(tip_mesh),
        MeshMaterial3d(core_mat),
        Transform::from_xyz(0.0, 0.42, 0.0),
    ));
}

/// Walk-over collect des wisps → `MetaSouls` (Âmes). Radius `collect_radius`.
pub fn sys_collect_soul_wisps(
    mut commands: Commands,
    mut meta: ResMut<MetaSouls>,
    q_player: Query<&Transform, With<PickupCollector>>,
    q_wisps: Query<(Entity, &Transform, &SoulWisp)>,
) {
    let Some(player) = q_player.iter().next() else {
        return;
    };
    let ppos = player.translation;
    for (e, xf, wisp) in &q_wisps {
        let r_sq = wisp.collect_radius * wisp.collect_radius;
        if (xf.translation - ppos).length_squared() <= r_sq {
            meta.current = meta.current.saturating_add(wisp.value);
            meta.earned_run = meta.earned_run.saturating_add(wisp.value);
            commands.entity(e).despawn();
            info!("[roguelite] Soul wisp collected : +{} Âmes", wisp.value);
        }
    }
}

/// Flottement (bob sinus) + rotation lente des wisps. Skip pendant le Coffre
/// ouvert (l'aimantation contrôle alors la position).
pub fn sys_animate_soul_wisps(
    time: Res<Time>,
    coffre: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    mut q: Query<(&mut Transform, &SoulWisp)>,
) {
    if coffre.map(|c| c.is_open).unwrap_or(false) {
        return;
    }
    let t = time.elapsed_secs();
    for (mut tf, wisp) in &mut q {
        tf.translation.y = wisp.base_y + (t * 2.2 + wisp.phase).sin() * 0.16;
        tf.rotate_y(time.delta_secs() * 1.3);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sys_start_run(
    mut events: MessageReader<StartRunEvent>,
    mut next: ResMut<NextState<RunState>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    stage_graph_config: Res<forgia_stage::graph::RunGraphConfig>,
    mut wave: ResMut<crate::waves::RogueliteWave>,
    q_existing_enemies: Query<Entity, With<forgia_ai_arena_bot::ArenaBot>>,
    // Story-571 — monnaies : reset Or (in-run), conserve Souls méta.
    mut gold: Option<ResMut<Gold>>,
    mut meta: ResMut<MetaSouls>,
    mut run_timer: ResMut<RunTimer>,
    // Story-591 — bonus permanents (L'Enclume des Âmes) appliqués au run-start.
    meta_save: Res<crate::meta_shop::MetaShopSave>,
    meta_cat: Res<crate::meta_shop::MetaShopCatalogue>,
    mut perm_mods: ResMut<crate::meta_shop::PermanentPlayerMods>,
) {
    // 2026-05-29 — anti double-spawn : drain TOUS les events mais ne spawn
    // que pour le PREMIER. Le log montrait 2 events StartRunEvent traités
    // dans la même frame (1 fired par auto_start_run_on_enter, 1 résiduel
    // d'une session précédente non drainé entre transitions GameMode).
    // Effet : 2× wave 1 spawn = 16 enemies au lieu de 8 + 2 seeds qui s'écrasent.
    let mut first = true;
    for ev in events.read() {
        if !first {
            warn!("[roguelite] sys_start_run — extra StartRunEvent ignored (anti double-spawn)");
            continue;
        }
        first = false;
        let seed = ev.seed.unwrap_or_else(default_seed_from_clock);
        let graph = forgia_stage::graph::generate_run_graph(&stage_graph_config, seed);
        let total_stages = graph.total_stages;
        let boss_depth = graph.boss_depth();

        // Reset RogueliteWave (run repeatable depuis Lobby).
        *wave = crate::waves::RogueliteWave::default();

        // Story-571 — nouvelle run : l'Or (monnaie in-run) repart à 0. Les Souls
        // méta NE sont PAS reset (persistantes) ; seul le compteur "gagné cette
        // run" est remis à zéro pour le sensor.
        if let Some(g) = gold.as_deref_mut() {
            // Story-591 — nouvelle run : Or remis à l'Or de DÉPART (upgrade Pactole ;
            // 0 sans rang acheté, donc équivalent au reset à 0 d'avant).
            g.current = meta_save.start_gold(&meta_cat);
            g.total_collected = 0;
        }
        meta.earned_run = 0;
        // Story-591 — bonus permanents combat (composés dans PlayerCombatMods
        // par boons_apply::sys_recompute_boon_mods, qui re-tourne car perm change).
        perm_mods.damage_mul = meta_save.damage_mul(&meta_cat);
        perm_mods.damage_reduction = meta_save.damage_reduction(&meta_cat);
        // Chrono de run remis à zéro.
        run_timer.secs = 0.0;

        // Bug-leak bots (diag 2026-05-29) — despawn TOUS les ennemis survivants
        // avant de spawner la nouvelle wave 1. Sans ça, chaque restart de run
        // (Lobby→InRun, Defeat→relance) ré-empile 8 ennemis SANS nettoyer les
        // précédents : `DespawnOnExit(GameMode::Roguelite)` ne tire pas car on reste
        // DANS le mode Roguelite. Observé : `bots_alive: 51` en wave 1 (attendu 8)
        // → 51 capsules IA + skeletons + raycasts LOS → stutters frame
        // (`forgia2_lag_events.json`) ressentis comme freeze. `despawn()` Bevy 0.18
        // cascade aux enfants (body collider + visual + head proxy).
        let mut purged = 0_u32;
        for e in &q_existing_enemies {
            commands.entity(e).despawn();
            purged += 1;
        }
        if purged > 0 {
            info!("[roguelite] sys_start_run — purged {purged} stale enemies before wave 1 (anti-leak)");
        }

        // V7 M3 step 4 — Reset Player HP au max au start (sinon HP=0 sticky après Defeat).
        // Story-591 — upgrade Vitalité : PV max = base + bonus permanent.
        let max_hp = crate::meta_shop::BASE_PLAYER_HP + meta_save.max_hp_bonus(&meta_cat);
        commands.queue(move |world: &mut World| {
            let mut q = world.query_filtered::<&mut forgia_damage::Health, With<Player>>();
            if let Ok(mut hp) = q.single_mut(world) {
                hp.max = max_hp;
                hp.current = max_hp;
            }
        });

        commands.insert_resource(RunSeed {
            seed,
            stage_count: total_stages,
        });
        commands.insert_resource(graph.clone());

        // TODO(story-471..479): API removed, refactor abandonné — re-implémenter
        // current_stage_node / composition_for_stage / spawn_stage_enemies supprimés de crate::waves.
        // wave.current_stage_depth / wave.current_stage_kind supprimés de RogueliteWave.
        // Fallback : spawn wave 1 directement via spawn_wave_enemies.
        {
            let _ = &graph; // graph utilisé pour total_stages + boss_depth ci-dessus
            let spawned = crate::waves::spawn_wave_enemies(
                &mut commands,
                &mut meshes,
                &mut materials,
                &asset_server,
                1, // wave 1
            );
            info!("[roguelite] sys_start_run fallback — wave 1 spawned {spawned} enemies");
            next.set(RunState::InRun { stage: 0 });
        }
        info!(
            "[roguelite] Run started — seed={seed} total_stages={total_stages} boss_depth={boss_depth}"
        );
    }
}

// ─── Économie double monnaie (story-571, 2026-06-02) ───────────────────────
// Modèle Gunfire Reborn (décision canon user) :
//  - **Or** = monnaie in-run = `Gold` (alias de `forgia_rpg_data::loot_tables::Souls`).
//    Ramassée via pickups, dépensée au Coffre, PERDUE à 100% à la mort,
//    reset à 0 au start de chaque run.
//  - **Souls** = monnaie MÉTA persistante (`MetaSouls`). Gagnée en fin de
//    wave/boss, JAMAIS dépensée pour l'instant (sink = hub story-569),
//    CONSERVÉE à 100% à la mort. Reset uniquement à la fermeture du jeu
//    (save/load = story-569, hors scope).

/// Souls méta gagnés par wave régulière nettoyée.
/// Const V1 — à externaliser en gene genome via story-566 (retargetée Or/Souls).
pub const SOULS_PER_WAVE: u32 = 5;
/// Bonus Souls méta pour la wave boss (finale).
/// Const V1 — à externaliser en gene genome via story-566.
pub const SOULS_PER_BOSS: u32 = 25;

/// Monnaie MÉTA persistante (story-571). Distincte de l'Or in-run (`Gold`).
/// Persiste entre les runs dans la session ; pas reset à la mort.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MetaSouls {
    /// Total persistant accumulé (jamais reset en session).
    pub current: u32,
    /// Gagné sur la run en cours (reset au start, exposé au sensor).
    pub earned_run: u32,
}

/// 2026-06-02 — chrono de la run en cours (style Gunfire « 6 min 43 s »).
/// Tické pendant InRun/Boss (pause-safe : pas en Lobby/Paused), reset au start.
/// Affiché sous la minimap.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct RunTimer {
    pub secs: f32,
}

/// Tick le chrono de run uniquement en combat (InRun/Boss) et hors pause.
pub fn sys_tick_run_timer(
    time: Res<Time>,
    app_state: Res<State<AppMode>>,
    run_state: Option<Res<State<RunState>>>,
    mut timer: ResMut<RunTimer>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let in_run = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if in_run {
        timer.secs += time.delta_secs();
    }
}

/// État dernière run pour l'overlay Defeat (Or perdu / Souls conservées).
/// Set par sys_end_run quand result == Defeat. Lu par hud::draw_defeat_overlay.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct LastDefeatSummary {
    /// Or in-run perdu à la mort (= total de l'Or au moment du décès).
    pub or_lost: u32,
    /// Souls méta conservées (intégralement) malgré la défaite.
    pub souls_persistent: u32,
}

pub fn sys_end_run(
    mut events: MessageReader<EndRunEvent>,
    mut next: ResMut<NextState<RunState>>,
    mut gold: Option<ResMut<Gold>>,
    meta: Res<MetaSouls>,
    mut last_defeat: ResMut<LastDefeatSummary>,
) {
    for ev in events.read() {
        let state = match ev.result {
            RunResult::Victory => RunState::Victory,
            RunResult::Defeat => RunState::Defeat,
            RunResult::Abort => RunState::Lobby,
        };
        // Story-571 — Or in-run PERDU à 100% sur Defeat ; Souls méta conservées
        // intégralement. Victory : l'Or sera reset au prochain run start (les
        // Souls méta restent acquises). Abort = pause, pas de pénalité.
        if matches!(ev.result, RunResult::Defeat) {
            if let Some(g) = gold.as_deref_mut() {
                last_defeat.or_lost = g.current;
                g.current = 0;
            }
            last_defeat.souls_persistent = meta.current;
            info!(
                "[roguelite] Defeat — Or perdu={} ; Souls méta conservées={}",
                last_defeat.or_lost, meta.current
            );
        }
        next.set(state);
        info!("[roguelite] Run ended — {:?}", ev.result);
    }
}

/// Seed depuis l'horloge système (nanos) — fallback si user ne fournit pas.
/// Pas crypto-secure, suffisant pour seed déterministe roguelite.
fn default_seed_from_clock() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() & u128::from(u64::MAX)) as u64)
        .unwrap_or(0xC0FF_EEDE_ADBE_EF00)
}

/// Story-481 — Map WeaponType courante → speaker_id du genome dialogue.
/// Clés alignées sur `assets/genomes/roguelite/roguelite_dialogue.toml`
/// (pools indexés `speaker = "pepin" | "bourrasque" | "lenoir" | "boucherie"`).
/// Armes hors Arena V1 ou sans persona → "any" (fallback pool générique).
pub fn weapon_to_speaker(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "pepin",
        WeaponType::AssaultRifle => "bourrasque",
        WeaponType::Shotgun => "lenoir",
        WeaponType::RocketLauncher => "boucherie",
        _ => "any",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runstate_default_is_lobby() {
        assert_eq!(RunState::default(), RunState::Lobby);
    }

    #[test]
    fn stage_id_for_depth_alternates() {
        assert_eq!(stage_id_for_depth(0, false), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(1, false), "forge_sanctum");
        assert_eq!(stage_id_for_depth(2, false), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(3, false), "forge_sanctum");
    }

    #[test]
    fn stage_id_for_depth_boss_forces_crypts() {
        // Boss state utilise toujours crypts_of_anvil (boss_pad_required=true).
        assert_eq!(stage_id_for_depth(4, true), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(5, true), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(0, true), "crypts_of_anvil");
    }

    // Story-571 — l'ancien carry-over 25% (Souls) est supprimé : l'Or in-run
    // est désormais perdu à 100% à la mort, les Souls méta conservées à 100%.
    // Plus de math de fraction à tester.

    #[test]
    fn souls_earn_constants_sane() {
        // Boss vaut plus qu'une wave régulière (incitation à finir la run).
        assert!(SOULS_PER_BOSS > SOULS_PER_WAVE);
        assert_eq!(SOULS_PER_WAVE, 5);
        assert_eq!(SOULS_PER_BOSS, 25);
    }

    // TODO(story-471..479) : tests parse_music_state_combat_variants +
    // parse_music_state_all_known_states supprimés car forgia_audio_music_state
    // ::MusicState n'existe plus (scaffold vide post-refactor). parse_music_state
    // est stub Option<()> retournant None (cf src/run.rs:165-169). À ré-instaurer
    // quand l'enum MusicState sera re-implémentée.

    #[test]
    fn parse_music_state_unknown_returns_none() {
        assert!(parse_music_state("").is_none());
        assert!(parse_music_state("nope").is_none());
        assert!(parse_music_state("xyz_combat").is_none()); // pas en prefix
    }

    #[test]
    fn run_seed_stage_seed_deterministic() {
        let a = RunSeed {
            seed: 42,
            stage_count: 0,
        };
        let b = RunSeed {
            seed: 42,
            stage_count: 5,
        }; // stage_count NOT used in derive
        assert_eq!(a.stage_seed(3), b.stage_seed(3));
        assert_eq!(a.stage_seed(0), b.stage_seed(0));
    }

    #[test]
    fn run_seed_different_seeds_diverge() {
        let a = RunSeed {
            seed: 42,
            stage_count: 0,
        };
        let b = RunSeed {
            seed: 43,
            stage_count: 0,
        };
        assert_ne!(a.stage_seed(0), b.stage_seed(0));
    }

    #[test]
    fn encounter_seed_deterministic() {
        let s = RunSeed {
            seed: 0xCAFE,
            stage_count: 0,
        };
        assert_eq!(s.encounter_seed(2, 7), s.encounter_seed(2, 7));
        assert_ne!(s.encounter_seed(2, 7), s.encounter_seed(2, 8));
    }

    #[test]
    fn run_result_variants_distinct() {
        assert_ne!(RunResult::Victory, RunResult::Defeat);
        assert_ne!(RunResult::Defeat, RunResult::Abort);
    }

    #[test]
    fn weapon_to_speaker_arena_v1_mapped() {
        assert_eq!(weapon_to_speaker(WeaponType::ModernAR), "pepin");
        assert_eq!(weapon_to_speaker(WeaponType::AssaultRifle), "bourrasque");
        assert_eq!(weapon_to_speaker(WeaponType::Shotgun), "lenoir");
        assert_eq!(weapon_to_speaker(WeaponType::RocketLauncher), "boucherie");
    }

    #[test]
    fn weapon_to_speaker_unknown_falls_back_to_any() {
        assert_eq!(weapon_to_speaker(WeaponType::AK47), "any");
        assert_eq!(weapon_to_speaker(WeaponType::PlasmaRifle), "any");
        assert_eq!(weapon_to_speaker(WeaponType::Chainsaw), "any");
    }
}

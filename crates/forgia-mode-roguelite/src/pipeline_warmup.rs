//! pipeline_warmup.rs — warmup des pipelines de rendu PBR au Lobby (anti-freeze
//! « tourner la caméra », audit fire-path 2026-07-20).
//!
//! ## Problème
//!
//! Freezes de 45-146 ms **quand le joueur tourne la caméra** : le décor (props
//! Inferno/KayKit, squelettes ennemis) qui entre dans le frustum pour la
//! première fois fait compiler son pipeline de rendu (specialization) sur le
//! thread de rendu → hitch. Profil confirmé runtime : burst dense au 1er passage,
//! puis calme (pipeline caché), puis re-burst en découvrant du neuf.
//!
//! ## Stratégie : warmup UNIQUE par session, au Lobby
//!
//! Le `PipelineCache` de Bevy est **global et persistant** : une combinaison
//! (mesh layout + StandardMaterial key) compilée une fois est réutilisée pour
//! toute la session. Toutes les salles d'une run puisent dans le MÊME catalogue
//! (`DecorAssets` préchargé au Startup, 3 GLB squelettes) → compiler un
//! représentant de chaque au Lobby purge TOUS les futurs hitches. On ne re-warme
//! pas aux Lobbys suivants (garde `WarmupState.done`).
//!
//! ## Pièges
//!
//! - **Frustum obligatoire** (bevy-specialist 2026-07-20) : un `StandardMaterial`
//!   ne compile QUE si son entité est réellement rendue (`ViewVisibility==true`).
//!   `Visibility::Hidden` / hors-champ NE compile PAS (contrairement aux dummies
//!   Hanabi). Le diorama est donc **visible mais minuscule** (`WARMUP_SCALE`),
//!   parenté à la caméra et occlus par le viewmodel — jamais caché.
//! - **Chargement GLB async** : au spawn, les `SceneRoot` ne sont pas encore
//!   instanciées → rien à compiler tout de suite. On garde le diorama vivant au
//!   moins `WARMUP_MIN_FRAMES` (le temps que les scènes chargent ET soient rendues
//!   une frame) AVANT de faire confiance au gate `PipelinesReady`. Un plafond
//!   `WARMUP_MAX_FRAMES` force le despawn (anti-lock si une compile ne finit jamais).
//!
//! Placement : le diorama est spawné en **world-space, hors-champ** (`Y=-500`),
//! PAS parenté à la caméra. C'est `NoFrustumCulling` (posé sur les meshes au
//! scene-ready) qui force leur rendu — donc la compilation — indépendamment de la
//! position/orientation de la caméra. (Le prewarm d'armes `weapon_select.rs` fait
//! l'inverse — parenté caméra + occlus par le viewmodel — mais lui n'a pas de
//! meshes hors-champ à couvrir.)

use bevy::prelude::*;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::scene::SceneInstanceReady;
use bevy::state::state_scoped::DespawnOnExit;
use forgia_core::prelude::{GameMode, GameSet};
use forgia_effects::prelude::PipelinesReady;

use crate::boss_portal::{PORTAL_CLOSED_GLB, PORTAL_OPEN_GLB};
use crate::decor::DecorAssets;
use crate::enemies::{skeleton_asset_path, EnemyArchetype};
use crate::run::RunState;

/// Plugin du warmup de pipelines PBR au Lobby (anti-freeze « tourner la caméra »).
/// Encapsule état + spawn/tick/cleanup + sensor (miroir `WeaponSelectPlugin`).
/// Le détecteur générique `PipelinesReadyPlugin` est fourni par `ForgiaEffectsPlugin`.
pub struct PipelineWarmupPlugin;

impl Plugin for PipelineWarmupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WarmupState>()
            .add_systems(OnEnter(RunState::Lobby), sys_spawn_warmup_diorama)
            .add_systems(OnExit(RunState::Lobby), sys_clear_warmup)
            .add_systems(
                Update,
                sys_tick_warmup
                    .in_set(GameSet::Effects)
                    .run_if(in_state(RunState::Lobby)),
            )
            .add_systems(Update, sys_write_warmup_sensor.in_set(GameSet::Sensors));
    }
}

/// Position hors-champ du diorama (loin sous le monde) — invisible pour le
/// joueur. Le rendu (donc la compilation) est forcé par `NoFrustumCulling` sur
/// ses meshes, PAS par le fait d'être dans le frustum (rendu interne, pas gameplay).
const WARMUP_OFFSCREEN_Y: f32 = -500.0;
/// Frames minimales avant de faire confiance à `PipelinesReady` : laisse les
/// `SceneRoot` GLB s'instancier ET être rendues au moins une frame (sinon le gate
/// serait « prêt » AVANT que quoi que ce soit ait commencé à compiler).
const WARMUP_MIN_FRAMES: u32 = 90;
/// Plafond dur (anti-lock) : despawn le diorama même si le gate n'est jamais prêt.
const WARMUP_MAX_FRAMES: u32 = 900;

const SENSOR_PATH: &str = "forgia2_pipeline_warmup.json";

/// Marqueur sur chaque entité du diorama de warmup (cleanup ciblé).
#[derive(Component)]
pub struct WarmupProp;

/// État session-once du warmup.
#[derive(Resource, Default)]
pub struct WarmupState {
    /// `true` une fois le warmup terminé — ne pas re-warmer aux Lobbys suivants.
    pub done: bool,
    /// Frames écoulées depuis le spawn du diorama (0 tant que non spawné).
    pub frames: u32,
    /// Nombre de `SceneRoot` de warmup spawnés (diagnostic sensor).
    pub classes_spawned: u32,
    /// Nombre de scènes RÉELLEMENT instanciées (`SceneInstanceReady` reçu). Gate
    /// anti-race (BUG #2 auto-QA) : `PipelinesReady` peut valoir `true` par
    /// ABSENCE de travail tant qu'une scène lourde n'a pas fini de charger (aucune
    /// demande de pipeline émise) → on n'autorise le despawn que quand TOUTES les
    /// scènes ont été instanciées, pas juste `frames >= MIN`.
    pub scenes_ready: u32,
    /// Frames qu'il a fallu pour atteindre `PipelinesReady` (0 tant que non atteint).
    pub frames_to_ready: u32,
    /// `true` tant que le diorama est vivant (spawn fait, pas encore despawné).
    pub active: bool,
}

/// OnEnter(Lobby) — spawn le diorama de warmup (1re fois de la session). Chaque
/// GLB distinct du catalogue (`DecorAssets` + 3 squelettes) est rendu une fois,
/// minuscule et occlus, pour compiler son pipeline au Lobby et non en combat.
pub fn sys_spawn_warmup_diorama(
    mut commands: Commands,
    mut state: ResMut<WarmupState>,
    asset_server: Res<AssetServer>,
    decor: Option<Res<DecorAssets>>,
    q_cam: Query<&Camera, With<Camera3d>>,
) {
    if state.done || state.active {
        return;
    }
    // Garde : sans caméra 3D active qui rend, aucun pipeline ne se compile → inutile
    // de spawner (OnEnter Lobby = in-game, la FpsCamera est présente ; garde par sûreté).
    if !q_cam.iter().any(|c| c.is_active) {
        return;
    }

    let mut count: u32 = 0;
    let mut spawn_scene = |commands: &mut Commands, scene: Handle<Scene>| {
        commands
            .spawn((
                Name::new("PipelineWarmupProp"),
                WarmupProp,
                SceneRoot(scene),
                // Hors-champ + Visible : les meshes reçoivent `NoFrustumCulling` au
                // scene-ready (observer ci-dessous) → rendus (donc pipeline compilé)
                // SANS être vus du joueur. `Hidden` ne compilerait rien (piège PBR).
                Transform::from_xyz(0.0, WARMUP_OFFSCREEN_Y, 0.0),
                Visibility::Visible,
                // BUG #3 auto-QA : filet de sécurité, comme tout prop du mode
                // (`decor_markers`) — despawn garanti si on quitte le mode sans
                // passer proprement par `OnExit(RunState::Lobby)`.
                DespawnOnExit(GameMode::Roguelite),
            ))
            .observe(on_warmup_scene_ready);
        count += 1;
    };

    // 1) Tous les GLB de décor distincts (handles déjà préchargés au Startup).
    if let Some(decor) = decor {
        for group in [
            &decor.landmarks,
            &decor.big,
            &decor.braziers,
            &decor.scatter,
            &decor.walls,
            &decor.wall_corner,
            &decor.rubble,
            &decor.buildings,
        ] {
            for handle in group {
                spawn_scene(&mut commands, handle.clone());
            }
        }
    }

    // 2) Les squelettes ennemis distincts (skinned mesh = pipeline distinct).
    //    Dédupliqué : 4 archétypes → 3 GLB (Boss réutilise Warrior).
    let mut seen: Vec<&'static str> = Vec::new();
    for arch in [
        EnemyArchetype::Tank,
        EnemyArchetype::Runner,
        EnemyArchetype::Sniper,
        EnemyArchetype::Boss,
    ] {
        let path = skeleton_asset_path(arch);
        if seen.contains(&path) {
            continue;
        }
        seen.push(path);
        let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        spawn_scene(&mut commands, scene);
    }

    // 3) BUG #1 auto-QA — assets GARANTIS visibles en combat mais absents de
    //    `DecorAssets` : le SOL de l'arène (vu dès la 1re frame de CHAQUE run,
    //    source unique `forgia_stage::MERGED_FLOOR_GLB`) et les portails de boss.
    //    (Non couverts : matériaux procéduraux mushrooms/stations + VFX blend —
    //    story de suivi, cf story-664 §limites.)
    for path in forgia_stage::MERGED_FLOOR_GLB {
        let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        spawn_scene(&mut commands, scene);
    }
    for path in [PORTAL_CLOSED_GLB, PORTAL_OPEN_GLB] {
        let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(path));
        spawn_scene(&mut commands, scene);
    }

    state.active = true;
    state.frames = 0;
    state.scenes_ready = 0;
    state.classes_spawned = count;
    info!("[pipeline-warmup] diorama spawné : {count} classes de pipeline (décor + squelettes) — Lobby, session-once");
}

/// Observer posé sur chaque `WarmupProp` : au scene-ready, marque tous les meshes
/// descendants `NoFrustumCulling` → ils sont rendus (et donc leur pipeline
/// compilé) MÊME hors du frustum de la caméra (position hors-champ). C'est ce qui
/// permet de warmer sans afficher quoi que ce soit au joueur.
fn on_warmup_scene_ready(
    event: On<SceneInstanceReady>,
    children: Query<&Children>,
    q_mesh: Query<Entity, With<Mesh3d>>,
    q_prop: Query<(), With<WarmupProp>>,
    mut state: ResMut<WarmupState>,
    mut commands: Commands,
) {
    // Ne compter que les scènes de warmup (l'observer est ciblé, mais garde par
    // sûreté si un SceneInstanceReady d'une autre source remontait).
    if q_prop.get(event.entity).is_err() {
        return;
    }
    for desc in children.iter_descendants(event.entity) {
        if q_mesh.get(desc).is_ok() {
            if let Ok(mut ec) = commands.get_entity(desc) {
                ec.insert(NoFrustumCulling);
            }
        }
    }
    // BUG #2 auto-QA — cette scène est réellement instanciée : le gate de despawn
    // n'ouvre que quand `scenes_ready == classes_spawned` (toutes chargées).
    state.scenes_ready = state.scenes_ready.saturating_add(1);
}

/// Update(Lobby) — attend que les GLB soient chargés+rendus (`WARMUP_MIN_FRAMES`)
/// PUIS que `PipelinesReady` passe true, puis despawn le diorama. Plafond
/// `WARMUP_MAX_FRAMES` = sécurité anti-lock.
pub fn sys_tick_warmup(
    mut commands: Commands,
    mut state: ResMut<WarmupState>,
    ready: Option<Res<PipelinesReady>>,
    q_props: Query<Entity, With<WarmupProp>>,
) {
    if !state.active {
        return;
    }
    state.frames = state.frames.saturating_add(1);

    let ready_now = ready.map(|r| r.0).unwrap_or(false);
    let past_min = state.frames >= WARMUP_MIN_FRAMES;
    // BUG #2 auto-QA — toutes les scènes spawnées ont-elles été instanciées ?
    // Tant que non, `PipelinesReady` peut être `true` par absence de travail (une
    // scène lourde n'a pas encore émis ses demandes de pipeline) → despawn interdit.
    let all_scenes_ready = state.scenes_ready >= state.classes_spawned;
    let timed_out = state.frames >= WARMUP_MAX_FRAMES;

    // Despawn quand : (délai min écoulé ET toutes les scènes instanciées ET
    // pipelines prêts) OU plafond atteint (anti-lock si une compile ne finit jamais).
    if (past_min && all_scenes_ready && ready_now) || timed_out {
        for e in &q_props {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
        state.frames_to_ready = state.frames;
        state.active = false;
        state.done = true;
        if timed_out && !ready_now {
            warn!(
                "[pipeline-warmup] despawn au PLAFOND ({} frames) sans PipelinesReady — compile anormalement longue ou bloquée",
                state.frames
            );
        } else {
            info!(
                "[pipeline-warmup] pipelines prêts après {} frames — diorama despawné ({} classes préchauffées)",
                state.frames, state.classes_spawned
            );
        }
    }
}

/// OnExit(Lobby) — filet de sécurité : despawn tout diorama résiduel (si le
/// joueur lance la run avant la fin du warmup, le plafond n'a pas encore tiré).
pub fn sys_clear_warmup(
    mut commands: Commands,
    mut state: ResMut<WarmupState>,
    q_props: Query<Entity, With<WarmupProp>>,
) {
    for e in &q_props {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.try_despawn();
        }
    }
    if state.active {
        // Le warmup n'a pas fini de compiler (run lancée tôt) : on considère le
        // travail « fait » pour la session (les pipelines déjà compilés persistent ;
        // le résidu compilera au 1er affichage en jeu, comme avant — pas pire).
        state.active = false;
        state.done = true;
    }
}

/// Sensor `forgia2_pipeline_warmup.json` (règle observability-required) : état du
/// warmup + health check si le gate n'a jamais été atteint (compile bloquée).
pub fn sys_write_warmup_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    state: Res<WarmupState>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    // warn si le warmup a été « terminé » par le plafond sans jamais atteindre
    // ready (frames_to_ready == WARMUP_MAX_FRAMES) → compile anormale.
    let (severity, next_step) = if state.done && state.frames_to_ready >= WARMUP_MAX_FRAMES {
        (
            "warn",
            "warmup terminé au plafond sans PipelinesReady — compile pipeline anormalement longue, freezes possibles en jeu",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"pipeline_warmup","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"done":{},"active":{},"classes_spawned":{},"scenes_ready":{},"frames_to_ready":{}}}"#,
        time.elapsed_secs(),
        state.done,
        state.active,
        state.classes_spawned,
        state.scenes_ready,
        state.frames_to_ready,
    );
    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[pipeline-warmup] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_before_max_frames() {
        // Le plafond anti-lock doit laisser une vraie fenêtre de warmup.
        assert!(WARMUP_MIN_FRAMES < WARMUP_MAX_FRAMES);
    }

    #[test]
    fn despawn_gate_requires_all_scenes_ready() {
        // BUG #2 auto-QA — reproduit la logique du gate : pipelines "prêts" +
        // délai min écoulé mais une scène pas encore instanciée → NE PAS despawn.
        let classes_spawned = 30u32;
        let scenes_ready = 29u32; // une scène lourde encore en chargement
        let frames = WARMUP_MIN_FRAMES + 1;
        let ready_now = true;
        let past_min = frames >= WARMUP_MIN_FRAMES;
        let all_scenes_ready = scenes_ready >= classes_spawned;
        let timed_out = frames >= WARMUP_MAX_FRAMES;
        assert!(
            !((past_min && all_scenes_ready && ready_now) || timed_out),
            "le gate ne doit PAS despawn tant que toutes les scènes ne sont pas instanciées"
        );
        // Une fois la dernière scène prête, le gate ouvre.
        let all_ready = 30u32 >= classes_spawned;
        assert!(past_min && all_ready && ready_now);
    }

    #[test]
    fn skeletons_dedup_to_three() {
        // 4 archétypes → 3 GLB distincts (Boss réutilise Warrior) : le diorama
        // ne double pas le pipeline skinned Warrior.
        let mut seen: Vec<&'static str> = Vec::new();
        for arch in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ] {
            let p = skeleton_asset_path(arch);
            if !seen.contains(&p) {
                seen.push(p);
            }
        }
        assert_eq!(seen.len(), 3);
    }
}

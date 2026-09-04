//! # castle_hub — Hall de Forgia
//!
//! Hub social 3D **walkable** (2026-07-22). Charge le château importé depuis le
//! pack Unity « FANTASTIC Highlands Castle » (`castle_highlands.glb`, ~48 MB,
//! 7453 pièces instanciées) et laisse le joueur **marcher dedans en 1ʳᵉ personne**,
//! sans combat. Zone NEUTRE — point de rassemblement (multijoueur à terme).
//!
//! Modèle : scène visuelle séparée de la collision. Le GLB reste un `SceneRoot`,
//! tandis que cinq colliders cuboïdes couvrent la Grande Salle. Cela évite de
//! construire un TriMesh Rapier pour chacun des milliers de meshes décoratifs.
//! - **1ʳᵉ personne** : on réutilise tel quel le déplacement FPS de `forgia-player`
//!   (`player_movement`, gaté seulement sur `AppMode::InGame`) → aucune dépendance
//!   à forgia-rpg/forgia-fps, aucune touche au crate roguelite.
//! - **Zéro pass rendu spécial** : lumière jour + ambiante caméra.
//!
//! - Entrée  : **F10** depuis le menu → `GameMode::CastleHub` + `AppMode::InGame`.
//!   (V1 debug ; un bouton menu « Château » viendra en V2.)
//! - Spawn   : la capsule joueur (forgia-player, `OnEnter(InGame)`) est replacée
//!   près d'un coin de l'emprise du château, au niveau du sol, face au centre.
//! - Sortie  : ESC → Paused, puis Q → Menu (cleanup auto via marker).
//!
//! V1 = « spawn + marcher ». La zone réellement jouable est actuellement la
//! Grande Salle ; le reste du château est visuel jusqu'à la création de proxies
//! de collision par zone dans le pipeline d'assets.

use bevy::asset::{LoadState, RenderAssetUsages};
use bevy::light::CascadeShadowConfigBuilder;
use bevy::math::Affine3A;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_rapier3d::prelude::{
    Collider, ComputedColliderShape, KinematicCharacterController, PhysicsSet, QueryFilter,
    ReadRapierContext, RigidBody,
};
use forgia_core::prelude::*;
use forgia_player::prelude::Player;
use serde::Deserialize;
use std::collections::HashSet;

// 2026-08-14 — le format de manifeste et le calcul de distance vivent desormais
// dans `forgia_streaming::cells`, parce que la carte d'expedition « Le Vallon »
// utilise EXACTEMENT le meme format (48 cellules, `schema_version = 1`,
// `cell_size_m = 40`). Les dupliquer aurait ete la classe de defaut n°1 du
// projet — une grandeur ecrite deux fois — appliquee au format d'un FICHIER,
// c'est-a-dire a l'endroit ou une divergence se voit le plus tard.
//
// Les alias gardent le code ci-dessous lisible sans le reecrire.
use forgia_streaming::cells::{
    StreamCell as CastleStreamCellManifest,
    horizontal_distance as horizontal_distance_to_stream_cell,
};

/// Découpage offline du château visuel. Chaque cellule est un glTF séparé : le
/// runtime n'instancie jamais plus le GLB monolithique de 7 453 pièces.
const CASTLE_STREAM_MANIFEST: &str = include_str!(
    "../../../assets/models/environment/castle/castle_stream_cells_grass/castle_stream_cells.toml"
);
/// Rayon de rendu autour du joueur. Le déschargement utilise une hystérésis
/// plus grande pour éviter le ping-pong au bord d'une cellule.
///
/// 240 m couvre la diagonale complète du château (~193 m d'emprise, cellule la
/// plus lointaine à 139,6 m du spawn, ~230 m depuis un bord opposé) : le Hall
/// est une vitrine, TOUT le château doit être visible d'où qu'on soit
/// (retour utilisateur 2026-07-23 « je ne vois pas tout le château » — l'ancien
/// 48 m ne chargeait que 24/46 cellules autour du spawn). Le découpage en
/// cellules et la cadence 1 spawn/3 frames restent actifs : si la perf exige de
/// réduire ce rayon un jour, la voie durable est un HLOD/impostor distant cuit
/// offline, pas un retour au château tronqué.
const CASTLE_STREAM_RENDER_RADIUS_M: f32 = 240.0;
const CASTLE_STREAM_UNLOAD_RADIUS_M: f32 = 280.0;
/// Une racine de scène au plus toutes les trois frames : l'AssetServer reste
/// asynchrone, et l'instanciation ECS est lissée plutôt que concentrée sur une
/// seule frame lors de l'entrée dans le Hall.
const CASTLE_STREAM_SPAWN_COOLDOWN_FRAMES: u8 = 2;
/// Collision structurelle cuite offline depuis le même GLB. Le proxy fusionne
/// sols, escaliers, murs, tours, piliers et falaises en un seul TriMesh décimé.
/// Voir `tools/blender/build_castle_collision.py`.
const CASTLE_COLLISION_MESH: &str =
    "models/environment/castle/castle_highlands_collision_runtime.glb#Mesh0/Primitive0";
/// Surfaces jouables (sols + escaliers) cuites séparément. Elles ne sont pas
/// soumises à la décimation agressive des murs/falaises.
const CASTLE_WALKABLE_COLLISION_MESH: &str =
    "models/environment/castle/castle_highlands_walkable_runtime.glb#Mesh0/Primitive0";
/// Échelle appliquée au GLB. Le château est déjà exporté en mètres (~193 m
/// d'emprise), donc 1.0. Tunable si l'échelle ressentie en jeu est off.
const CASTLE_SCALE: f32 = 1.0;
/// Point de spawn validé en jeu dans le Hall (capture capteur 2026-07-22).
/// Le sol walkable est à Y≈37,5 ici ; le spawn précédent à Y=218 plaçait le
/// joueur dans une zone haute du château, loin du point de rassemblement voulu.
const GREAT_HALL_SPAWN: Vec3 = Vec3::new(10.321, 37.524, 35.625);
/// Dalle visuelle source sous `GREAT_HALL_SPAWN`, mesurée par raycast Blender
/// dans `castle_highlands.glb` (2026-07-22). C'est aussi un proxy de collision
/// déterministe : un spawn critique ne dépend jamais d'un TriMesh décimé.
// +1 mm au-dessus de la dalle mesurée : le raycast de santé touche toujours ce
// proxy déterministe avant le TriMesh coplanaire, sans effet visible/jouable.
const GREAT_HALL_SPAWN_FLOOR_CENTER: Vec3 = Vec3::new(9.959, 36.330, 37.372);
const GREAT_HALL_SPAWN_FLOOR_HALF: Vec3 = Vec3::new(2.0, 0.145, 2.0);
/// Plaque physique de la Grande Salle, relevée depuis les dalles Unity source
/// `SM_MOD_floor_castle_LOD0` : X=[-4.041,17.959], Z=[31.372,43.372],
/// Y top=36.474. Le TriMesh offline reste un complément (escaliers et futures
/// zones), mais il ne doit pas être l'unique support d'une zone critique : il
/// a laissé tomber le KCC entre deux dalles adjacentes lors du test runtime.
const GREAT_HALL_FLOOR_PLATE_CENTER: Vec3 = Vec3::new(6.959, 36.330, 37.372);
const GREAT_HALL_FLOOR_PLATE_HALF: Vec3 = Vec3::new(11.0, 0.145, 6.0);
const GREAT_HALL_FALL_RECOVERY_Y: f32 = 32.0;
/// Cible du regard = le trône (le joueur apparaît face à lui).
const GREAT_HALL_LOOK: Vec3 = Vec3::new(-89.5, 218.0, 39.6);
/// Limite de sûreté du terrain Unity. Elle reste loin du château pour ne pas
/// fermer les portes de la Grande Salle : le joueur peut rejoindre les
/// extérieurs portés par le collider de terrain.
const CASTLE_BOUNDARY_CENTER: Vec3 = Vec3::new(-13.0, 10.0, 34.0);
const CASTLE_BOUNDARY_HALF_EXTENTS: Vec3 = Vec3::new(200.0, 40.0, 225.0);
const CASTLE_BOUNDARY_THICKNESS: f32 = 1.0;
/// Budget qui détecte une régression vers des colliders générés depuis le GLB.
const GREAT_HALL_COLLIDER_BUDGET: u32 = 32;
const SENSOR_PATH: &str = "forgia2_castle_hub.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

pub struct CastleHubPlugin;

impl Plugin for CastleHubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CastleHubTelemetry>()
            .add_systems(OnEnter(GameMode::CastleHub), spawn_castle_hub)
            .add_systems(OnExit(GameMode::CastleHub), cleanup_castle_hub)
            // Entrée debug F10 depuis le menu principal.
            .add_systems(
                Update,
                enter_castle_hub_hotkey.run_if(in_state(AppMode::Menu)),
            )
            .add_systems(
                Update,
                (
                    initialize_castle_collision,
                    build_streamed_cell_colliders,
                    place_player_in_castle,
                    ensure_castle_ambient,
                    stream_castle_visual_cells,
                )
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame)),
            )
            // Cross-mode : le dernier état persiste après sortie, ce qui rend le
            // cleanup vérifiable au lieu de perdre la preuve au retour Menu.
            .add_systems(
                Update,
                sys_write_castle_hub_sensor.in_set(GameSet::Sensors),
            )
            // Ne laisse jamais une session Hall poursuivre sous la carte. Ce
            // garde-fou ne remplace pas les colliders : il transforme une
            // régression de couverture en retour contrôlé au spawn, après que
            // Rapier a écrit le résultat du step.
            .add_systems(
                FixedUpdate,
                recover_fallen_castle_player
                    .after(PhysicsSet::Writeback)
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

/// Marker — entités décor du hub (scène + lumières), despawn `OnExit`.
#[derive(Component)]
struct CastleHubMarker;

/// Marker sur l'entité racine de la scène (télémétrie de chargement).
#[derive(Component)]
struct CastleHubSceneRoot;

/// Une cellule visuelle effectivement instanciée par le streaming du Hall.
#[derive(Component)]
struct CastleHubStreamCell {
    id: String,
}

/// Cellule dont le collider TriMesh (fusion de ses meshes) n'est pas encore bâti.
/// Retiré une fois le collider posé. La collision suit ainsi EXACTEMENT le visuel
/// (aligné par construction), sans dépendre d'un GLB de collision séparé — les
/// anciens étaient dans un repère incompatible : la « walkable » plafonnait 13 m
/// SOUS les sols, la couverture Z s'arrêtait à 45 m sur un château de 215 m
/// (mesuré 2026-07-23, cause de la chute hors dalle de spawn).
#[derive(Component)]
struct CellCollisionPending;

#[derive(Clone, Debug, Deserialize)]

/// Etat runtime des cellules : le plan est cuit hors-ligne, la décision de
/// présence est faite à partir de la position du joueur sans relire le disque.
#[derive(Resource)]
struct CastleVisualStreaming {
    cells: Vec<CastleStreamCellManifest>,
    cooldown_frames: u8,
}

/// Racine cachée du proxy de collision structurel, séparée de la scène visuelle.
#[derive(Component)]
struct CastleHubCollisionRoot;

/// Racine du proxy qui garantit les surfaces sous les spawns et les escaliers.
#[derive(Component)]
struct CastleHubWalkableCollision;

/// Dalle de sûreté exactement sous le spawn de la Grande Salle. Ce proxy simple
/// est l'oracle runtime du spawn : si un raycast Rapier ne le touche pas, la
/// scène ne doit pas annoncer un état physique sain.
#[derive(Component)]
struct CastleHubSpawnFloor;

/// Sol principal de la zone V1 réellement jouable du Hall. Il est séparé du
/// pad de certification du spawn pour instrumenter les deux invariants.
#[derive(Component)]
struct CastleHubGreatHallFloorPlate;

/// Le proxy est construit une seule fois lorsque son GLB est chargé. Nous ne
/// confions pas cette étape à `AsyncCollider`: quand un GLB sans index est
/// rencontré, Rapier réessayait à chaque frame et loguait le mesh entier.
#[derive(Component)]
struct CastleHubCollisionPending;

/// Marker sur la caméra une fois l'ambiante hub appliquée (idempotence + retrait).
#[derive(Component)]
struct CastleAmbientApplied;

/// Pose en attente : le joueur sera replacé dès qu'il existe. La collision est
/// synchrone et indépendante du chargement visuel du GLB.
#[derive(Resource, Default)]
struct CastleSpawnPending {
    frames_waited: u32,
}

/// État compact de cycle de vie, séparé de la scène afin de conserver le dernier
/// cleanup et le résultat du placement après le despawn du Hall.
#[derive(Resource)]
struct CastleHubTelemetry {
    entries: u32,
    last_entry_secs: f32,
    last_exit_secs: f32,
    last_cleanup_roots: u32,
    last_wait_frames: u32,
    fall_recoveries: u32,
    spawn_status: &'static str,
    last_active_secs: f32,
    last_active_scene_state: &'static str,
    last_active_descendants: u32,
    last_active_meshes: u32,
    last_active_colliders: u32,
    last_active_spawn_status: &'static str,
    peak_descendants: u32,
    peak_meshes: u32,
    peak_colliders: u32,
    first_colliders_ready_secs: f32,
    streamed_cells: u32,
    stream_plan_cells: u32,
}

impl Default for CastleHubTelemetry {
    fn default() -> Self {
        Self {
            entries: 0,
            last_entry_secs: 0.0,
            last_exit_secs: 0.0,
            last_cleanup_roots: 0,
            last_wait_frames: 0,
            fall_recoveries: 0,
            spawn_status: "never_entered",
            last_active_secs: 0.0,
            last_active_scene_state: "never_entered",
            last_active_descendants: 0,
            last_active_meshes: 0,
            last_active_colliders: 0,
            last_active_spawn_status: "never_entered",
            peak_descendants: 0,
            peak_meshes: 0,
            peak_colliders: 0,
            first_colliders_ready_secs: 0.0,
            streamed_cells: 0,
            stream_plan_cells: 0,
        }
    }
}

/// F10 depuis le menu → entre dans le Hall de Forgia.
fn enter_castle_hub_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
) {
    if keys.just_pressed(KeyCode::F10) {
        next_game.set(GameMode::CastleHub);
        next_app.set(AppMode::InGame);
        info!("[castle-hub] Entrée Hall de Forgia (F10)");
    }
}

/// Soleil principal du Hall (porte les ombres).
#[derive(Component)]
pub(crate) struct CastleKeyLight;

/// Remplissage ciel — SANS ombres, donc traversant. Doit rester discret.
#[derive(Component)]
pub(crate) struct CastleFillLight;

/// Portée des cascades d'ombre du soleil, en mètres. Le château fait ~193 m
/// d'emprise : en deçà, ses intérieurs lointains ne sont pas ombrés du tout.
const CASTLE_SHADOW_DISTANCE_M: f32 = 420.0;

fn spawn_castle_hub(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    lighting: Res<crate::castle_flames::CastleLighting>,
    mut telemetry: ResMut<CastleHubTelemetry>,
) {
    telemetry.entries = telemetry.entries.saturating_add(1);
    telemetry.last_entry_secs = time.elapsed_secs();
    telemetry.last_wait_frames = 0;
    telemetry.fall_recoveries = 0;
    telemetry.spawn_status = "stream_manifest_requested";
    telemetry.last_active_secs = 0.0;
    telemetry.last_active_scene_state = "stream_manifest_requested";
    telemetry.last_active_descendants = 0;
    telemetry.last_active_meshes = 0;
    telemetry.last_active_colliders = 0;
    telemetry.last_active_spawn_status = "stream_manifest_requested";
    telemetry.peak_descendants = 0;
    telemetry.peak_meshes = 0;
    telemetry.peak_colliders = 0;
    telemetry.first_colliders_ready_secs = 0.0;
    telemetry.streamed_cells = 0;
    telemetry.stream_plan_cells = 0;
    match forgia_streaming::cells::parse_manifest(CASTLE_STREAM_MANIFEST) {
        Ok(manifest) if manifest.schema_version == 1 && !manifest.cells.is_empty() => {
            let cells = manifest.cells;
            let cell_count = cells.len();
            let cell_size_m = manifest.cell_size_m;
            commands.insert_resource(CastleVisualStreaming {
                cells,
                cooldown_frames: 0,
            });
            telemetry.stream_plan_cells = cell_count as u32;
            telemetry.spawn_status = "streaming_cells_requested";
            info!(
                "[castle-hub] streaming visuel activé: {cell_count} cellules de {cell_size_m:.0}m"
            );
        }
        Ok(manifest) => {
            telemetry.spawn_status = "stream_manifest_invalid";
            error!(
                "[castle-hub] manifest de streaming invalide (schema={}, cells={}); rendu château non instancié",
                manifest.schema_version,
                manifest.cells.len()
            );
        }
        Err(error) => {
            telemetry.spawn_status = "stream_manifest_parse_failed";
            error!("[castle-hub] manifest de streaming illisible: {error}");
        }
    }
    spawn_castle_collision(&mut commands, &asset_server);

    // Lumière jour : key soleil chaud (avec ombres) + fill ciel froid.
    //
    // 🚨 Le fill n'a PAS d'ombres — il traverse donc murs et toit et éclaire
    // l'intérieur comme s'il n'y avait pas de château. À 7 000 lux (sa valeur
    // d'origine) c'était la cause principale des murs incohérents : identiques
    // côte à côte, l'un blanc éclatant parce qu'il lui fait face, l'autre noir.
    // Il reste utile comme soupçon de ciel, mais doit rester DISCRET.
    // Les deux intensités vivent maintenant dans `castle_hub_lighting.toml`
    // (hot-reload) : elles se jugent à l'œil, pas à la compilation.
    commands.spawn((
        CastleHubMarker,
        CastleKeyLight,
        DirectionalLight {
            color: Color::srgb(1.0, 0.97, 0.90),
            illuminance: lighting.key_lux,
            shadows_enabled: true,
            ..default()
        },
        // Les cascades par défaut ne portent pas jusqu'au bout du château
        // (~193 m d'emprise) : sans ça, les intérieurs lointains reçoivent le
        // soleil SANS ombre, donc à pleine puissance à travers le toit.
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            maximum_distance: CASTLE_SHADOW_DISTANCE_M,
            first_cascade_far_bound: 24.0,
            ..default()
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.5, 0.0)),
        Name::new("CastleHubKeyLight"),
    ));
    commands.spawn((
        CastleHubMarker,
        CastleFillLight,
        DirectionalLight {
            color: Color::srgb(0.68, 0.78, 1.0),
            illuminance: lighting.fill_lux,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.3, 2.4, 0.0)),
        Name::new("CastleHubFillLight"),
    ));

    commands.insert_resource(CastleSpawnPending::default());
    info!(
        "[castle-hub] cellules visuelles + mesh collision structurel={} (un TriMesh offline)",
        CASTLE_COLLISION_MESH,
    );
}

/// Rend le château par cellules, avec une cadence bornée. Les colliders restent
/// volontairement séparés et complets : aucun changement de cellule ne peut
/// retirer le sol sous les pieds du joueur.
fn stream_castle_visual_cells(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    q_player: Query<&Transform, With<Player>>,
    streaming: Option<ResMut<CastleVisualStreaming>>,
    q_loaded: Query<(Entity, &CastleHubStreamCell)>,
    mut telemetry: ResMut<CastleHubTelemetry>,
    mut diag_tick: Local<u32>,
) {
    let Some(mut streaming) = streaming else {
        return;
    };
    let player_pos = q_player
        .single()
        .map(|transform| transform.translation)
        .unwrap_or(GREAT_HALL_SPAWN);
    let loaded: HashSet<&str> = q_loaded.iter().map(|(_, cell)| cell.id.as_str()).collect();
    telemetry.streamed_cells = loaded.len() as u32;

    // Désactivation d'abord, avec hystérésis : les entités de la cellule et
    // leurs descendants sont libérés ensemble par le cleanup Bevy.
    for (entity, cell) in &q_loaded {
        let Some(spec) = streaming.cells.iter().find(|spec| spec.id == cell.id) else {
            warn!(
                "[castle-hub] cellule runtime absente du manifest: {}",
                cell.id
            );
            commands.entity(entity).despawn();
            continue;
        };
        let unload_distance = horizontal_distance_to_stream_cell(player_pos, spec);
        if unload_distance > CASTLE_STREAM_UNLOAD_RADIUS_M {
            info!(
                "[castle-hub] unload cellule {} (d={unload_distance:.1}m > {CASTLE_STREAM_UNLOAD_RADIUS_M:.0}m, player=({:.1},{:.1},{:.1}))",
                cell.id, player_pos.x, player_pos.y, player_pos.z
            );
            commands.entity(entity).despawn();
        }
    }

    // Sonde ~4 s : état de la boucle (diagnostic « streaming bloqué à N »).
    *diag_tick = diag_tick.wrapping_add(1);
    if diag_tick.is_multiple_of(1024) {
        let in_radius = streaming
            .cells
            .iter()
            .filter(|cell| {
                horizontal_distance_to_stream_cell(player_pos, cell)
                    <= CASTLE_STREAM_RENDER_RADIUS_M
            })
            .count();
        info!(
            "[castle-hub] stream tick: {} chargées, {in_radius}/{} dans le rayon, cooldown={}, player=({:.1},{:.1},{:.1})",
            loaded.len(),
            streaming.cells.len(),
            streaming.cooldown_frames,
            player_pos.x,
            player_pos.y,
            player_pos.z
        );
    }

    if streaming.cooldown_frames > 0 {
        streaming.cooldown_frames -= 1;
        return;
    }

    // Stable : à distance égale l'id trie toujours de la même manière. Une
    // seule scène est instanciée par fenêtre, ce qui borne le spike ECS.
    let next = streaming
        .cells
        .iter()
        .filter(|cell| !loaded.contains(cell.id.as_str()))
        .filter_map(|cell| {
            let distance = horizontal_distance_to_stream_cell(player_pos, cell);
            (distance <= CASTLE_STREAM_RENDER_RADIUS_M).then_some((distance, cell))
        })
        .min_by(|(left_distance, left), (right_distance, right)| {
            left_distance
                .total_cmp(right_distance)
                .then_with(|| left.id.cmp(&right.id))
        });
    let Some((_, cell)) = next else {
        return;
    };
    commands.spawn((
        CastleHubMarker,
        CastleHubSceneRoot,
        CastleHubStreamCell {
            id: cell.id.clone(),
        },
        CellCollisionPending,
        SceneRoot(asset_server.load(cell.render.clone())),
        Transform::from_scale(Vec3::splat(CASTLE_SCALE)),
        Name::new(format!("CastleHubStreamCell:{}", cell.id)),
    ));
    info!(
        "[castle-hub] spawn cellule {} ({} déjà chargées / plan {})",
        cell.id,
        loaded.len(),
        streaming.cells.len()
    );
    streaming.cooldown_frames = CASTLE_STREAM_SPAWN_COOLDOWN_FRAMES;
    telemetry.streamed_cells = telemetry.streamed_cells.saturating_add(1);
}

/// Construit UN TriMesh fusionné par cellule dès que sa scène est instanciée : la
/// collision suit alors EXACTEMENT le visuel (mêmes meshes, même repère),
/// contrairement aux GLB de collision offline désalignés. Un seul collider par
/// cellule (≤ 46 au total) — jamais un par mesh (crash 8052 documenté). Au plus
/// un build par frame pour lisser le coût (chaque fusion ≈ 100k sommets + BVH).
fn build_streamed_cell_colliders(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    q_pending: Query<Entity, With<CellCollisionPending>>,
    q_children: Query<&Children>,
    q_mesh: Query<&Mesh3d>,
    q_tf: Query<&Transform>,
) {
    for cell in &q_pending {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        // BFS en transform LOCAL accumulé (racine cellule = identité : sa propre
        // transform est appliquée par la physique). N'utilise PAS GlobalTransform,
        // non propagé la frame où la scène s'instancie.
        let mut stack: Vec<(Entity, Affine3A)> = vec![(cell, Affine3A::IDENTITY)];
        while let Some((entity, world_in_cell)) = stack.pop() {
            if let Ok(children) = q_children.get(entity) {
                for &child in children {
                    let local = q_tf
                        .get(child)
                        .map(Transform::compute_affine)
                        .unwrap_or(Affine3A::IDENTITY);
                    stack.push((child, world_in_cell * local));
                }
            }
            let Ok(mesh3d) = q_mesh.get(entity) else {
                continue;
            };
            let Some(mesh) = meshes.get(&mesh3d.0) else {
                continue;
            };
            let Some(VertexAttributeValues::Float32x3(pos)) =
                mesh.attribute(Mesh::ATTRIBUTE_POSITION)
            else {
                continue;
            };
            let base = positions.len() as u32;
            for p in pos {
                positions.push(
                    world_in_cell
                        .transform_point3(Vec3::from_array(*p))
                        .to_array(),
                );
            }
            match mesh.indices() {
                Some(Indices::U32(ix)) => indices.extend(ix.iter().map(|i| base + i)),
                Some(Indices::U16(ix)) => indices.extend(ix.iter().map(|i| base + u32::from(*i))),
                None => indices.extend((0..pos.len() as u32).map(|i| base + i)),
            }
        }
        if positions.is_empty() || indices.len() < 3 {
            continue; // scène pas encore instanciée — on réessaie la frame suivante
        }
        commands.entity(cell).remove::<CellCollisionPending>();
        let mut merged = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        merged.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        merged.insert_indices(Indices::U32(indices));
        match Collider::from_bevy_mesh(&merged, &ComputedColliderShape::TriMesh(default())) {
            Some(collider) => {
                commands.entity(cell).insert((
                    collider,
                    RigidBody::Fixed,
                    CastleHubWalkableCollision,
                ));
            }
            None => warn!("[castle-hub] collider cellule non bâti (mesh dégénéré)"),
        }
        break; // un build par frame pour lisser le hitch
    }
}

/// Charge directement un mesh de collision structurel, fabriqué offline. Le
/// chargement direct via `AsyncCollider` évite la dépendance au `SceneSpawner`
/// de `AsyncSceneCollider`, qui restait en attente au moment de l'entrée hub.
/// Il remplace le faux grand plan plat qui faisait léviter le joueur.
fn spawn_castle_collision(commands: &mut Commands, asset_server: &AssetServer) {
    spawn_collision_proxy(
        commands,
        asset_server,
        CASTLE_COLLISION_MESH,
        "CastleHubStructuralCollision",
        None,
    );
    spawn_collision_proxy(
        commands,
        asset_server,
        CASTLE_WALKABLE_COLLISION_MESH,
        "CastleHubWalkableCollision",
        Some(CastleHubWalkableCollision),
    );
    spawn_castle_navigation_bounds(commands);
}

fn spawn_collision_proxy(
    commands: &mut Commands,
    asset_server: &AssetServer,
    asset_path: &'static str,
    name: &'static str,
    walkable: Option<CastleHubWalkableCollision>,
) {
    let mut entity = commands.spawn((
        CastleHubMarker,
        CastleHubCollisionRoot,
        CastleHubCollisionPending,
        Mesh3d(asset_server.load(asset_path)),
        Transform::from_scale(Vec3::splat(CASTLE_SCALE)),
        RigidBody::Fixed,
        Visibility::Hidden,
        Name::new(name),
    ));
    if let Some(marker) = walkable {
        entity.insert(marker);
    }
}

/// Crée la collision structurelle après le chargement du mesh. Certains GLB
/// Blender valides sont exportés sans accessors d'indices ; Rapier exige un
/// index buffer alors que Bevy sait les rendre. Dans ce cas précis, les
/// sommets sont déjà déroulés par triangles : on fabrique donc l'indexation
/// séquentielle, sans changer le mesh visuel ni l'asset source.
fn initialize_castle_collision(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    pending: Query<(Entity, &Mesh3d), With<CastleHubCollisionPending>>,
) {
    for (entity, mesh_handle) in &pending {
        let Some(source_mesh) = meshes.get(mesh_handle) else {
            continue;
        };

        let mut collision_mesh = source_mesh.clone();
        let index_source = if collision_mesh.indices().is_some() {
            "source"
        } else {
            let vertex_count = collision_mesh.count_vertices();
            if vertex_count == 0 || vertex_count % 3 != 0 {
                error!(
                    "[castle-hub] collision mesh invalide: {vertex_count} sommets non indexés (attendu multiple de 3)"
                );
                commands
                    .entity(entity)
                    .remove::<CastleHubCollisionPending>();
                continue;
            }
            collision_mesh.insert_indices(Indices::U32((0..vertex_count as u32).collect()));
            "generated_sequential"
        };

        match Collider::from_bevy_mesh(&collision_mesh, &ComputedColliderShape::TriMesh(default()))
        {
            Some(collider) => {
                commands
                    .entity(entity)
                    .insert(collider)
                    .remove::<CastleHubCollisionPending>();
                info!(
                    "[castle-hub] collider structurel prêt ({} triangles, indices={index_source})",
                    collision_mesh.count_vertices() / 3,
                );
            }
            None => {
                error!(
                    "[castle-hub] impossible de créer le collider structurel; collision désactivée pour cette entrée"
                );
                commands
                    .entity(entity)
                    .remove::<CastleHubCollisionPending>();
            }
        }
    }
}

/// Empêche une sortie hors du terrain Unity, sans enfermer les portes du Hall.
fn spawn_castle_navigation_bounds(commands: &mut Commands) {
    let boundary = CASTLE_BOUNDARY_HALF_EXTENTS;
    let spawn_collider =
        |commands: &mut Commands, name: &'static str, position: Vec3, half: Vec3| {
            commands.spawn((
                CastleHubMarker,
                RigidBody::Fixed,
                Collider::cuboid(half.x, half.y, half.z),
                Transform::from_translation(position),
                Name::new(name),
            ));
        };

    commands.spawn((
        CastleHubMarker,
        CastleHubSpawnFloor,
        RigidBody::Fixed,
        Collider::cuboid(
            GREAT_HALL_SPAWN_FLOOR_HALF.x,
            GREAT_HALL_SPAWN_FLOOR_HALF.y,
            GREAT_HALL_SPAWN_FLOOR_HALF.z,
        ),
        Transform::from_translation(GREAT_HALL_SPAWN_FLOOR_CENTER),
        Name::new("CastleHubGreatHallSpawnFloor"),
    ));
    commands.spawn((
        CastleHubMarker,
        CastleHubGreatHallFloorPlate,
        RigidBody::Fixed,
        Collider::cuboid(
            GREAT_HALL_FLOOR_PLATE_HALF.x,
            GREAT_HALL_FLOOR_PLATE_HALF.y,
            GREAT_HALL_FLOOR_PLATE_HALF.z,
        ),
        Transform::from_translation(GREAT_HALL_FLOOR_PLATE_CENTER),
        Name::new("CastleHubGreatHallFloorPlate"),
    ));
    spawn_collider(
        commands,
        "CastleHubTerrainNorthBoundary",
        Vec3::new(
            CASTLE_BOUNDARY_CENTER.x,
            CASTLE_BOUNDARY_CENTER.y,
            CASTLE_BOUNDARY_CENTER.z - boundary.z,
        ),
        Vec3::new(boundary.x, boundary.y, CASTLE_BOUNDARY_THICKNESS),
    );
    spawn_collider(
        commands,
        "CastleHubTerrainSouthBoundary",
        Vec3::new(
            CASTLE_BOUNDARY_CENTER.x,
            CASTLE_BOUNDARY_CENTER.y,
            CASTLE_BOUNDARY_CENTER.z + boundary.z,
        ),
        Vec3::new(boundary.x, boundary.y, CASTLE_BOUNDARY_THICKNESS),
    );
    spawn_collider(
        commands,
        "CastleHubTerrainWestBoundary",
        Vec3::new(
            CASTLE_BOUNDARY_CENTER.x - boundary.x,
            CASTLE_BOUNDARY_CENTER.y,
            CASTLE_BOUNDARY_CENTER.z,
        ),
        Vec3::new(CASTLE_BOUNDARY_THICKNESS, boundary.y, boundary.z),
    );
    spawn_collider(
        commands,
        "CastleHubTerrainEastBoundary",
        Vec3::new(
            CASTLE_BOUNDARY_CENTER.x + boundary.x,
            CASTLE_BOUNDARY_CENTER.y,
            CASTLE_BOUNDARY_CENTER.z,
        ),
        Vec3::new(CASTLE_BOUNDARY_THICKNESS, boundary.y, boundary.z),
    );
}

/// Replace le joueur au point walkable validé en jeu, face au trône.
fn place_player_in_castle(
    mut commands: Commands,
    pending: Option<ResMut<CastleSpawnPending>>,
    mut q_player: Query<(
        &mut Transform,
        &mut Player,
        &mut KinematicCharacterController,
    )>,
    mut telemetry: ResMut<CastleHubTelemetry>,
    walkable_collision: Query<(), (With<CastleHubWalkableCollision>, With<Collider>)>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let Ok((mut tf, mut player, mut kcc)) = q_player.single_mut() else {
        return; // joueur pas encore spawné (OnEnter InGame) — retry.
    };
    pending.frames_waited += 1;
    telemetry.last_wait_frames = pending.frames_waited;
    if walkable_collision.is_empty() {
        telemetry.spawn_status = "waiting_walkable_collision";
        return;
    }
    tf.translation = GREAT_HALL_SPAWN;
    player.vertical_velocity = 0.0;
    // Évite de conserver la translation de gravité calculée avant que le
    // chargement du Hall ne pose le joueur au-dessus du sol.
    kcc.translation = None;
    let look_flat = Vec3::new(GREAT_HALL_LOOK.x, GREAT_HALL_SPAWN.y, GREAT_HALL_LOOK.z);
    let dir = (look_flat - GREAT_HALL_SPAWN).normalize_or_zero();
    player.yaw = (-dir.x).atan2(-dir.z);
    tf.rotation = Quat::from_rotation_y(player.yaw);
    telemetry.spawn_status = "placed_great_hall";
    // NE PAS retirer `CastleVisualStreaming` ici : c'est un one-shot de PLACEMENT
    // du joueur, mais le streaming doit vivre TOUTE la session hub. Le retrait de
    // cette ressource au placement (bug 2026-07-23) figeait le château aux ~2
    // cellules chargées pendant les 4 frames avant le placement → `stream_castle_
    // visual_cells` sortait en early-return à jamais. Le nettoyage appartient à
    // `cleanup_castle_hub` (OnExit). Seul `CastleSpawnPending` (garde de placement)
    // se retire ici.
    commands.remove_resource::<CastleSpawnPending>();
    info!(
        "[castle-hub] Joueur posé Grande Salle @ ({:.1},{:.1},{:.1}) après {} frame(s)",
        GREAT_HALL_SPAWN.x, GREAT_HALL_SPAWN.y, GREAT_HALL_SPAWN.z, pending.frames_waited
    );
}

/// Récupère le joueur si une future zone non couverte le fait passer sous le
/// plancher du Hall. C'est volontairement limité au `GameMode::CastleHub` :
/// aucun autre mode ne voit son comportement de chute modifié.
fn recover_fallen_castle_player(
    mut q_player: Query<(
        &mut Transform,
        &mut Player,
        &mut KinematicCharacterController,
    )>,
    mut telemetry: ResMut<CastleHubTelemetry>,
    fly: Option<Res<crate::castle_ground::FlyMode>>,
) {
    // En vol libre (outil de calage dev), on descend volontairement sous le sol du
    // Hall : ne pas ramener le joueur au spawn tant que le vol est actif.
    if fly.is_some_and(|f| f.0) {
        return;
    }
    // Pendant les quelques frames entre `OnEnter(InGame)` (spawn générique Y=2)
    // et le placement Hall, une récupération serait un faux positif. Le garde
    // ne s'arme qu'une fois le spawn Hall effectivement publié.
    if telemetry.spawn_status != "placed_great_hall" {
        return;
    }
    let Ok((mut transform, mut player, mut kcc)) = q_player.single_mut() else {
        return;
    };
    if transform.translation.y >= GREAT_HALL_FALL_RECOVERY_Y {
        return;
    }
    warn!(
        "[castle-hub] chute hors couverture Hall @ ({:.2},{:.2},{:.2}) — retour spawn sûr",
        transform.translation.x, transform.translation.y, transform.translation.z,
    );
    transform.translation = GREAT_HALL_SPAWN;
    player.vertical_velocity = 0.0;
    kcc.translation = None;
    telemetry.fall_recoveries = telemetry.fall_recoveries.saturating_add(1);
    telemetry.spawn_status = "recovered_after_fall";
}

#[cfg(test)]
fn is_inside_great_hall_floor_plate(position: Vec3) -> bool {
    let delta = position - GREAT_HALL_FLOOR_PLATE_CENTER;
    delta.x.abs() <= GREAT_HALL_FLOOR_PLATE_HALF.x && delta.z.abs() <= GREAT_HALL_FLOOR_PLATE_HALF.z
}

/// Compte les colliders attachés à une racine ou à ses enfants. Les proxies
/// courants sont des meshes directs (donc racines), mais la fonction conserve
/// la télémétrie correcte si un proxy redevient une scène GLB hiérarchique.
fn subtree_collider_count(
    root: Entity,
    children: &Query<&Children>,
    colliders: &Query<(), With<Collider>>,
) -> u32 {
    let mut count = 0u32;
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if colliders.get(entity).is_ok() {
            count = count.saturating_add(1);
        }
        if let Ok(list) = children.get(entity) {
            stack.extend(list.iter());
        }
    }
    count
}

/// Ajoute une ambiante douce à la caméra 3D (déboucher les intérieurs du château).
/// Idempotent (marker), retirée en sortie de mode.
/// Brouillard de distance du Hall — crépuscule doré chaud. Subtil : le château
/// reste net, seul le grand lointain se fond dans le ciel, avec un halo solaire.
fn castle_ambient_fog() -> DistanceFog {
    DistanceFog {
        color: Color::srgb(0.66, 0.58, 0.60),
        directional_light_color: Color::srgb(1.0, 0.82, 0.62),
        directional_light_exponent: 30.0,
        falloff: FogFalloff::Linear {
            start: 140.0,
            end: 850.0,
        },
    }
}

fn ensure_castle_ambient(
    mut commands: Commands,
    lighting: Res<crate::castle_flames::CastleLighting>,
    q_cam: Query<Entity, (With<Camera3d>, Without<CastleAmbientApplied>)>,
) {
    for cam in &q_cam {
        commands.entity(cam).insert((
            CastleAmbientApplied,
            // Valeur pilotée par `assets/genomes/castle_hub_lighting.toml`
            // (hot-reload). Historique de l'ancienne const : 700 « trop sombre »
            // → 1600 « cramait la roche » → 900. Ces allers-retours étaient le
            // symptôme de l'absence de lumières locales, pas un mauvais réglage :
            // une ambiante n'a pas de direction, trop forte elle SUPPRIME le
            // modelé. Les bougies allumées (castle_flames) prennent le relais,
            // donc l'ambiante peut redescendre.
            lighting.ambient(),
            // Brouillard de distance : donne de la profondeur atmosphérique
            // (le lointain se fond dans le ciel crépusculaire) + un halo doré
            // autour du soleil vu à travers la brume. Volontairement SUBTIL —
            // `start` très loin pour ne pas voiler le château (~215 m) : à 215 m
            // le voile ne fait que ~11 %, le fond/horizon (>850 m) se fond.
            castle_ambient_fog(),
        ));
        info!("[castle-hub] Ambiante + brouillard hub appliqués à la caméra");
    }
}

fn cleanup_castle_hub(
    mut commands: Commands,
    q: Query<Entity, With<CastleHubMarker>>,
    q_cam: Query<Entity, With<CastleAmbientApplied>>,
    time: Res<Time>,
    mut telemetry: ResMut<CastleHubTelemetry>,
) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    for cam in &q_cam {
        commands
            .entity(cam)
            .remove::<(CastleAmbientApplied, AmbientLight, DistanceFog)>();
    }
    commands.remove_resource::<CastleSpawnPending>();
    // Le streaming vit toute la session hub ; on le libère à la sortie (une
    // ré-entrée le ré-insère via spawn_castle_hub).
    commands.remove_resource::<CastleVisualStreaming>();
    telemetry.last_exit_secs = time.elapsed_secs();
    telemetry.last_cleanup_roots = count as u32;
    telemetry.spawn_status = "cleaned";
    info!("[castle-hub] Hub nettoyé : {count} entités despawn");
}

fn scene_load_status(asset_server: &AssetServer, scene: Option<&SceneRoot>) -> &'static str {
    let Some(scene) = scene else {
        return "not_present";
    };
    match asset_server.get_load_state(&scene.0) {
        Some(LoadState::Loaded) => "loaded",
        Some(LoadState::Loading) => "loading",
        Some(LoadState::Failed(_)) => "failed",
        Some(LoadState::NotLoaded) | None => "not_loaded",
    }
}

fn mesh_load_status(asset_server: &AssetServer, mesh: Option<&Mesh3d>) -> &'static str {
    let Some(mesh) = mesh else {
        return "not_present";
    };
    match asset_server.get_load_state(&mesh.0) {
        Some(LoadState::Loaded) => "loaded",
        Some(LoadState::Loading) => "loading",
        Some(LoadState::Failed(_)) => "failed",
        Some(LoadState::NotLoaded) | None => "not_loaded",
    }
}

fn severity_for_castle_hub(
    active: bool,
    scene_state: &str,
    waiting_frames: u32,
    mesh_count: u32,
    scene_collider_count: u32,
    collider_count: u32,
) -> (&'static str, &'static str) {
    if scene_state == "failed" {
        (
            "critical",
            "castle scene failed to load — inspect forgia2_assets.json and asset path",
        )
    } else if active && waiting_frames >= 120 {
        (
            "warn",
            "castle player was not spawned after 120 frames — inspect AppMode transition",
        )
    } else if active && scene_state == "loaded" && mesh_count == 0 {
        (
            "warn",
            "castle scene reports loaded but no Mesh3d descendants are instantiated",
        )
    } else if active && scene_collider_count > 0 {
        (
            "warn",
            "castle visual GLB has colliders — remove AsyncSceneCollider to prevent a physics hitch",
        )
    } else if active && collider_count > GREAT_HALL_COLLIDER_BUDGET {
        (
            "warn",
            "castle collider budget exceeded — inspect collision proxies before profiling",
        )
    } else {
        ("ok", "")
    }
}

/// Capteur du Hall de Forgia : scène, collider, placement et cleanup.
///
/// Le parcours du sous-arbre du château ne se produit qu'à 1 Hz. Il ne participe
/// donc pas au coût du chargement des ~7453 pièces instanciées.
#[allow(clippy::too_many_arguments)]
fn sys_write_castle_hub_sensor(
    time: Res<Time>,
    game_mode: Res<State<GameMode>>,
    asset_server: Res<AssetServer>,
    mut telemetry: ResMut<CastleHubTelemetry>,
    pending: Option<Res<CastleSpawnPending>>,
    roots: Query<(Entity, &SceneRoot), With<CastleHubSceneRoot>>,
    collision_roots: Query<(Entity, &Mesh3d), With<CastleHubCollisionRoot>>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    scene_colliders: Query<(), With<Collider>>,
    proxy_colliders: Query<(), (With<Collider>, With<CastleHubMarker>)>,
    spawn_floor: Query<Entity, (With<CastleHubSpawnFloor>, With<Collider>)>,
    floor_plate: Query<Entity, (With<CastleHubGreatHallFloorPlate>, With<Collider>)>,
    q_player: Query<(Entity, &Transform), With<Player>>,
    rapier: ReadRapierContext,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    // Position joueur en direct (pour capturer un point de spawn walkable choisi
    // en jeu). Vec3::ZERO si le joueur n'est pas spawné.
    let player = q_player
        .single()
        .map(|(entity, transform)| (entity, transform.translation))
        .ok();
    let ppos = player.map(|(_, position)| position).unwrap_or(Vec3::ZERO);

    let active = *game_mode.get() == GameMode::CastleHub;
    let streamed_cell_count = telemetry.streamed_cells;
    let stream_plan_cells = telemetry.stream_plan_cells;
    let root_entries: Vec<(Entity, &SceneRoot)> = roots.iter().collect();
    let collision_root = collision_roots.iter().next();
    let mut any_scene_loading = false;
    let mut any_scene_failed = false;
    let mut all_scenes_loaded = !root_entries.is_empty();
    for (_, scene) in &root_entries {
        match scene_load_status(&asset_server, Some(scene)) {
            "failed" => any_scene_failed = true,
            "loaded" => {}
            _ => {
                any_scene_loading = true;
                all_scenes_loaded = false;
            }
        }
    }
    let scene_state = if root_entries.is_empty() {
        "not_present"
    } else if any_scene_failed {
        "failed"
    } else if all_scenes_loaded {
        "loaded"
    } else if any_scene_loading {
        "loading"
    } else {
        "not_loaded"
    };
    let collision_scene_state =
        mesh_load_status(&asset_server, collision_root.map(|(_, mesh)| mesh));
    let mut descendants = 0u32;
    let mut mesh_count = 0u32;
    let mut scene_collider_count = 0u32;
    for (root_entity, _) in &root_entries {
        let mut stack = vec![*root_entity];
        while let Some(entity) = stack.pop() {
            if entity != *root_entity {
                descendants = descendants.saturating_add(1);
            }
            if meshes.get(entity).is_ok() {
                mesh_count = mesh_count.saturating_add(1);
            }
            if scene_colliders.get(entity).is_ok() {
                scene_collider_count = scene_collider_count.saturating_add(1);
            }
            if let Ok(list) = children.get(entity) {
                stack.extend(list.iter());
            }
        }
    }
    let proxy_collider_count = proxy_colliders.iter().count() as u32;
    let structural_collider_count = collision_roots
        .iter()
        .map(|(root_entity, _)| subtree_collider_count(root_entity, &children, &scene_colliders))
        .sum::<u32>();
    // `proxy_colliders` inclut déjà les deux TriMesh et les boîtes de navigation.
    // Ne pas rajouter `structural_collider_count`, sinon le capteur double-compte
    // un proxy et masque une régression de budget.
    let collider_count = scene_collider_count.saturating_add(proxy_collider_count);
    let expected_spawn_floor = spawn_floor.iter().next();
    let expected_floor_plate = floor_plate.iter().next();
    let floor_plate_present = expected_floor_plate.is_some();
    let spawn_floor_hit = player.and_then(|(player_entity, _)| {
        let context = rapier.single().ok()?;
        let origin = Vec3::new(
            GREAT_HALL_SPAWN.x,
            GREAT_HALL_SPAWN.y + 8.0,
            GREAT_HALL_SPAWN.z,
        );
        context
            .cast_ray(
                origin,
                Vec3::NEG_Y,
                16.0,
                true,
                QueryFilter::default().exclude_collider(player_entity),
            )
            .map(|(entity, toi)| (entity, origin.y - toi))
    });
    let spawn_floor_ready = matches!(
        (expected_spawn_floor, spawn_floor_hit),
        (Some(expected), Some((hit, height)))
            if (expected == hit || expected_floor_plate == Some(hit))
                && (height
                    - (GREAT_HALL_SPAWN_FLOOR_CENTER.y + GREAT_HALL_SPAWN_FLOOR_HALF.y))
                    .abs()
                    <= 0.05
    );
    // Sonde de couverture walkable : rayons descendants sur un anneau autour du
    // spawn (5→60 m). Avant les colliders par-cellule, seuls les ~2 m de la boîte
    // de spawn répondaient ; après, les sols du château répondent partout où il y
    // a un sol. Métrique de validation de la collision par-cellule sans jouer.
    let walkable_probe_total: u32 = 24;
    let walkable_probe_hits: u32 = rapier
        .single()
        .ok()
        .map(|context| {
            let mut hits = 0u32;
            for ring in [5.0_f32, 12.0, 20.0, 30.0, 45.0, 60.0] {
                for (dx, dz) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)] {
                    let origin = Vec3::new(
                        GREAT_HALL_SPAWN.x + dx * ring,
                        GREAT_HALL_SPAWN.y + 10.0,
                        GREAT_HALL_SPAWN.z + dz * ring,
                    );
                    if context
                        .cast_ray(origin, Vec3::NEG_Y, 60.0, true, QueryFilter::default())
                        .is_some()
                    {
                        hits += 1;
                    }
                }
            }
            hits
        })
        .unwrap_or(0);
    let waiting_frames = pending
        .as_ref()
        .map(|pending| pending.frames_waited)
        .unwrap_or(telemetry.last_wait_frames);
    let (mut severity, mut next_step) = severity_for_castle_hub(
        active,
        scene_state,
        waiting_frames,
        mesh_count,
        scene_collider_count,
        collider_count,
    );
    if active && expected_spawn_floor.is_none() {
        severity = "critical";
        next_step = "Great Hall spawn floor entity is missing — inspect CastleHub lifecycle";
    } else if active && !floor_plate_present {
        severity = "critical";
        next_step = "Great Hall floor plate is missing — playable Hall coverage is incomplete";
    } else if active && !spawn_floor_ready {
        severity = "critical";
        next_step = "spawn-floor raycast misses its certified Rapier collider — inspect physics sync or collision groups";
    } else if active && telemetry.fall_recoveries > 0 {
        severity = "critical";
        next_step = "player fell below the Hall floor and was recovered — add or calibrate the missing walkable zone";
    } else if active && stream_plan_cells == 0 {
        severity = "critical";
        next_step =
            "castle streaming manifest is unavailable — inspect cooked cell assets and TOML schema";
    }
    if active {
        telemetry.last_active_secs = time.elapsed_secs();
        telemetry.last_active_scene_state = scene_state;
        telemetry.last_active_descendants = descendants;
        telemetry.last_active_meshes = mesh_count;
        telemetry.last_active_colliders = collider_count;
        telemetry.last_active_spawn_status = telemetry.spawn_status;
        telemetry.peak_descendants = telemetry.peak_descendants.max(descendants);
        telemetry.peak_meshes = telemetry.peak_meshes.max(mesh_count);
        telemetry.peak_colliders = telemetry.peak_colliders.max(collider_count);
        if spawn_floor_ready && telemetry.first_colliders_ready_secs == 0.0 {
            telemetry.first_colliders_ready_secs = time.elapsed_secs();
        }
    }
    let spawn_floor_height = spawn_floor_hit
        .map(|(_, height)| format!("{height:.3}"))
        .unwrap_or_else(|| "null".to_owned());
    let json = format!(
        r#"{{"id":"castle_hub","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"active":{active},"scene_state":"{scene_state}","collision_scene_state":"{collision_scene_state}","scene_root_present":{},"streamed_cells":{streamed_cell_count},"stream_plan_cells":{stream_plan_cells},"descendants":{descendants},"meshes":{mesh_count},"colliders":{collider_count},"scene_colliders":{scene_collider_count},"structural_collision":{structural_collider_count},"collision_proxies":{proxy_collider_count},"walkable_probe_hits":{walkable_probe_hits},"walkable_probe_total":{walkable_probe_total},"collider_budget":{GREAT_HALL_COLLIDER_BUDGET},"colliders_ready":{},"spawn_floor_entity_present":{},"spawn_floor_raycast_hit":{},"spawn_floor_height":{},"great_hall_floor_plate_present":{floor_plate_present},"fall_recoveries":{},"waiting_frames":{waiting_frames},"spawn_status":"{}","entries":{},"last_entry_secs":{:.1},"last_exit_secs":{:.1},"last_cleanup_roots":{},"player_pos":[{:.2},{:.2},{:.2}],"last_active":{{"timestamp_secs":{:.1},"scene_state":"{}","descendants":{},"meshes":{},"colliders":{},"spawn_status":"{}"}},"peak_active":{{"descendants":{},"meshes":{},"colliders":{},"first_colliders_ready_secs":{:.1}}}}}"#,
        time.elapsed_secs(),
        !root_entries.is_empty(),
        spawn_floor_ready,
        expected_spawn_floor.is_some(),
        spawn_floor_ready,
        spawn_floor_height,
        telemetry.fall_recoveries,
        telemetry.spawn_status,
        telemetry.entries,
        telemetry.last_entry_secs,
        telemetry.last_exit_secs,
        telemetry.last_cleanup_roots,
        ppos.x,
        ppos.y,
        ppos.z,
        telemetry.last_active_secs,
        telemetry.last_active_scene_state,
        telemetry.last_active_descendants,
        telemetry.last_active_meshes,
        telemetry.last_active_colliders,
        telemetry.last_active_spawn_status,
        telemetry.peak_descendants,
        telemetry.peak_meshes,
        telemetry.peak_colliders,
        telemetry.first_colliders_ready_secs,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_scene_is_critical() {
        assert_eq!(
            severity_for_castle_hub(true, "failed", 0, 0, 0, 0).0,
            "critical"
        );
    }

    #[test]
    fn timeout_and_empty_loaded_scene_warn() {
        assert_eq!(
            severity_for_castle_hub(true, "loading", 120, 0, 0, 0).0,
            "warn"
        );
        assert_eq!(
            severity_for_castle_hub(true, "loaded", 0, 0, 0, 0).0,
            "warn"
        );
    }

    #[test]
    fn inactive_hub_is_healthy() {
        assert_eq!(
            severity_for_castle_hub(false, "not_present", 0, 0, 0, 0).0,
            "ok"
        );
    }

    #[test]
    fn generated_scene_colliders_and_budget_overruns_warn() {
        assert_eq!(
            severity_for_castle_hub(true, "loaded", 0, 1, 1, 6).0,
            "warn"
        );
        assert_eq!(
            severity_for_castle_hub(true, "loaded", 0, 1, 0, GREAT_HALL_COLLIDER_BUDGET + 1).0,
            "warn"
        );
    }

    #[test]
    fn certified_spawn_floor_matches_the_measured_visual_tile() {
        let floor_top = GREAT_HALL_SPAWN_FLOOR_CENTER.y + GREAT_HALL_SPAWN_FLOOR_HALF.y;
        assert!((floor_top - 36.474).abs() <= 0.005);
        // Une capsule joueur de ~1 m peut se poser sur la dalle sans commencer
        // à l'intérieur du collider.
        assert!(GREAT_HALL_SPAWN.y - floor_top >= 1.0);
    }

    #[test]
    fn hall_plate_covers_spawn_and_the_confirmed_fall_position() {
        assert!(is_inside_great_hall_floor_plate(GREAT_HALL_SPAWN));
        // Runtime capture du 2026-07-22 : le KCC a quitté la petite dalle de
        // spawn à cette position alors qu'une dalle visuelle existe bien ici.
        assert!(is_inside_great_hall_floor_plate(Vec3::new(
            11.204, 1.062, 34.731
        )));
    }

    #[test]
    fn streamed_castle_manifest_is_versioned_and_contains_the_hall_cell() {
        // Passe par le lecteur PARTAGE (`forgia_streaming::cells`) : ce test
        // verifie desormais que le manifeste du chateau reste lisible par le
        // meme code que celui de l'expedition. Une divergence de format se
        // verrait ici, pas en jeu avec un chateau vide.
        let manifest = forgia_streaming::cells::parse_manifest(CASTLE_STREAM_MANIFEST)
            .expect("checked-in castle streaming manifest must parse");
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.cell_size_m, 32.0);
        assert!(manifest.cells.iter().any(|cell| {
            cell.id == "cell_x0_z1"
                && horizontal_distance_to_stream_cell(GREAT_HALL_SPAWN, cell) == 0.0
        }));
    }

    #[test]
    fn stream_distance_is_zero_inside_and_grows_outside_the_xz_bounds() {
        let cell = CastleStreamCellManifest {
            id: "test".into(),
            render: "test.gltf#Scene0".into(),
            bounds_min_m: [0.0, -20.0, 10.0],
            bounds_max_m: [20.0, 80.0, 30.0],
        };
        assert_eq!(
            horizontal_distance_to_stream_cell(Vec3::new(10.0, 500.0, 20.0), &cell),
            0.0
        );
        assert!(
            (horizontal_distance_to_stream_cell(Vec3::new(23.0, 0.0, 34.0), &cell) - 5.0).abs()
                < f32::EPSILON
        );
    }
}

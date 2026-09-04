//! # castle_ground — sol gazon du Hall de Forgia
//!
//! Le pack Unity « FANTASTIC Highlands Castle » pose le château sur un **Unity
//! Terrain** (heightmap 513², peint gazon/terre/pavé). Ce terrain n'est PAS un
//! prefab mesh → il était absent du parse GameObject qui a produit le GLB du
//! château : d'où le « château qui flotte » sans sol.
//!
//! Ce plugin ajoute le sol manquant : un mesh gazon reconstruit depuis le
//! heightmap Unity réel (via UnityPy), **ancré sur l'emprise des cliffs** (même
//! frame que le château — le transform brut du terrain Unity est dans un frame
//! prefab décalé, inutilisable). Détail : cf `reference_unitypackage_scene_reconstruction`.
//!
//! - **Séparé de `castle_hub.rs`** (autre terminal : colliders/spawn/perf) — 1
//!   fichier = 1 terminal. Le sol est purement visuel ici ; les proxies de
//!   collision walkable restent les colliders boîte de la Grande Salle.
//! - **Calage Y au runtime** : la hauteur du mesh est bakée en relatif [0..58 m] ;
//!   `TERRAIN_ALIGN.y` glisse le tout et `TERRAIN_VSCALE` module le relief, pour
//!   caler « en jeu » (capture > calcul) sans re-bake du GLB.

use bevy::asset::LoadState;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy_rapier3d::prelude::{
    AsyncSceneCollider, Collider, ComputedColliderShape, KinematicCharacterController, RigidBody,
};
use forgia_core::prelude::*;
use forgia_player::prelude::Player;

/// Scène GLB du sol : **reconstruit depuis le Unity Terrain réel** (`Terrain_castle_01`,
/// 300×300 m, heightmap 513², relief 117 m) via `tools/blender/terrain_from_unity.py`.
/// Peint herbe/terre/**chemins pavés** depuis la splatmap Unity (SplatAlpha 0, canaux
/// R=herbe G=terre B=pavé). Le placement (centre sous le château, plateau = sol du
/// Hall) est **baké dans le mesh** d'après la scène Unity — plus de valeur devinée.
const TERRAIN_SCENE: &str = "models/environment/castle/castle_terrain_unity.glb#Scene0";
/// Version décimée du même terrain (7 790 triangles), même repère baké, réservée à la
/// physique : un unique TriMesh statique = le plateau marchable autour du château.
const TERRAIN_COLLISION_SCENE: &str =
    "models/environment/castle/castle_terrain_unity_collision.glb#Scene0";
/// Végétation du créateur : 21 365 instances (herbe/fleurs/buissons) reconstruites
/// depuis les `m_TreeInstances` du Unity Terrain, meshes partagés + atlas alpha
/// (`T_ENV_foliage_castle`). Bevy auto-instancie les entités au même mesh. Purement
/// visuel (pas de collision — on traverse l'herbe). Suit le tune du sol.
const VEGETATION_SCENE: &str = "models/environment/castle/castle_vegetation.glb#Scene0";
/// La position monde est bakée dans le mesh (dérivée de la scène Unity : Terrain à
/// (−150,0,−150), château à (0,194,0), plateau ~200 m = sol du Hall). L'align reste
/// à zéro ; le fichier de tune LIVE ci-dessous permet un calage fin sans rebuild.
const TERRAIN_ALIGN: Vec3 = Vec3::ZERO;
const TERRAIN_VSCALE: f32 = 1.0;
/// Fichier de calage LIVE (racine repo) relu 1×/s en mode hub. Permet d'ajuster
/// position/relief/rotation du sol sans recompiler ni relancer le jeu.
/// Format : {"align":[x,y,z],"vscale":f,"yaw_deg":f}. Absent → consts ci-dessus.
const TUNE_PATH: &str = "castle_ground_tune.json";
/// Bornes Y du terrain reconstruit (mesh baké, jeu). Plateau du château ≈ 36.5 m ;
/// le terrain descend en falaises jusqu'à −59.5 m et culmine sur une colline à 57.5 m.
const TERRAIN_BAKED_MIN_Y: f32 = -59.5;
const TERRAIN_BAKED_MAX_Y: f32 = 57.5;
const TERRAIN_COLLISION_TRIANGLES: u32 = 7_790;
const SENSOR_PATH: &str = "forgia2_castle_ground.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;

pub struct CastleGroundPlugin;

impl Plugin for CastleGroundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GroundTune>()
            .init_resource::<FlyMode>()
            .add_systems(OnEnter(GameMode::CastleHub), spawn_castle_ground)
            .add_systems(OnExit(GameMode::CastleHub), cleanup_castle_ground)
            // `sys_nudge_ground` est désarmé quand l'éditeur de scène est ouvert :
            // il occupe tout le pavé numérique (4/6/8/2/9/3/7/1, +/-, Numpad0) et
            // continuerait à déplacer le terrain sous les pieds du créateur pendant
            // qu'il édite. Le vol libre, lui, reste actif — c'est le moyen de
            // naviguer pour aller placer un objet.
            .add_systems(
                Update,
                sys_nudge_ground
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame))
                    .run_if(not(forgia_editor::editor_holds_keyboard)),
            )
            .add_systems(
                Update,
                sys_toggle_fly
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame)),
            )
            // Vol noclip : APRÈS le mouvement (surcharge kcc.translation comme dash),
            // en FixedUpdate pour écraser la gravité posée par player_movement.
            .add_systems(
                FixedUpdate,
                sys_fly_move
                    .after(GameSet::Movement)
                    .run_if(in_state(GameMode::CastleHub))
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(
                Update,
                sys_write_castle_ground_sensor.in_set(GameSet::Sensors),
            );
    }
}

/// Calage LIVE du sol, éditable en jeu au pavé numérique (outil de dev). Chargé
/// depuis `castle_ground_tune.json` au OnEnter, appliqué à toutes les entités
/// `CastleGroundMarker`, sauvegardable (Numpad0). C'est la vérité runtime du
/// placement ; la valeur persiste dans le fichier de tune (couche definition).
#[derive(Resource)]
struct GroundTune {
    align: Vec3,
    vscale: f32,
    yaw_deg: f32,
}

impl Default for GroundTune {
    fn default() -> Self {
        Self {
            align: TERRAIN_ALIGN,
            vscale: TERRAIN_VSCALE,
            yaw_deg: 0.0,
        }
    }
}

/// Vol libre (noclip) — OUTIL DE DEV pour naviguer et aligner le sol du Hall.
/// Toggle par « / ». `pub(crate)` : `castle_hub` désarme la récup de chute en vol.
#[derive(Resource, Default)]
pub(crate) struct FlyMode(pub(crate) bool);

/// Vitesse du vol de calage (dev, hors gameplay — non exposé au créateur, ne
/// touche aucune valeur de mouvement du jeu shippé).
const FLY_SPEED: f32 = 30.0;
/// Multiplicateur de vol en maintenant Maj (dev).
const FLY_BOOST: f32 = 4.0;

/// Marker — mesh du sol gazon, despawn `OnExit`.
#[derive(Component)]
struct CastleGroundMarker;

#[derive(Component)]
pub(crate) struct CastleGroundVisual;

#[derive(Component)]
struct CastleGroundCollision;

fn spawn_castle_ground(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut tune: ResMut<GroundTune>,
) {
    // Charge le calage persisté (fichier tune) dans la ressource live avant le spawn.
    if let Ok(txt) = std::fs::read_to_string(TUNE_PATH) {
        let (align, vscale, yaw) = parse_tune(&txt);
        tune.align = align;
        tune.vscale = vscale;
        tune.yaw_deg = yaw;
    }
    let transform = ground_transform(tune.align, tune.vscale, tune.yaw_deg);
    commands.spawn((
        CastleGroundMarker,
        CastleGroundVisual,
        SceneRoot(asset_server.load(TERRAIN_SCENE)),
        transform,
        Name::new("CastleGroundVisual"),
    ));
    // Le terrain a un seul mesh. AsyncSceneCollider crée donc un seul TriMesh
    // depuis la version décimée cachée, au lieu des milliers de colliders que
    // produirait le GLB du château.
    commands.spawn((
        CastleGroundMarker,
        CastleGroundCollision,
        SceneRoot(asset_server.load(TERRAIN_COLLISION_SCENE)),
        transform,
        RigidBody::Fixed,
        AsyncSceneCollider {
            shape: Some(ComputedColliderShape::TriMesh(default())),
            ..default()
        },
        Visibility::Hidden,
        Name::new("CastleGroundCollision"),
    ));
    // Végétation créateur (herbe/fleurs/buissons) — visuel seul, suit le sol.
    commands.spawn((
        CastleGroundMarker,
        SceneRoot(asset_server.load(VEGETATION_SCENE)),
        transform,
        Name::new("CastleVegetation"),
    ));
    info!(
        "[castle-ground] terrain Unity: visual={} collision={} veg={} align=[{:.1},{:.1},{:.1}] vscale×{:.2} ({} triangles)",
        TERRAIN_SCENE,
        TERRAIN_COLLISION_SCENE,
        VEGETATION_SCENE,
        tune.align.x,
        tune.align.y,
        tune.align.z,
        tune.vscale,
        TERRAIN_COLLISION_TRIANGLES,
    );
    info!(
        "[castle-ground] CALAGE LIVE (pavé num): 4/6=X 8/2=Z 9/3=Y 7/1=yaw +/-=relief | Maj=grossier | Numpad0=sauver"
    );
}

/// Centre XZ du mesh terrain dans SON repère local (bornes GLB X=[-163,137],
/// Z=[-116,184] → centre ≈ (-13, 34)). Sert de pivot au yaw pour tourner le sol
/// sur lui-même et non autour de l'origine du GLB (sinon la rotation le balaie en arc).
const TERRAIN_CENTER_LOCAL: Vec3 = Vec3::new(-13.2, 0.0, 34.0);

fn ground_transform(align: Vec3, vscale: f32, yaw_deg: f32) -> Transform {
    let rotation = Quat::from_rotation_y(yaw_deg.to_radians());
    let scale = Vec3::new(1.0, vscale, 1.0);
    // Pivote autour du CENTRE du terrain, pas de l'origine du GLB : on compense la
    // translation pour que le centre reste fixe sous la rotation/échelle. yaw=0 &
    // vscale=1 → translation == align (rétro-compatible).
    let c = TERRAIN_CENTER_LOCAL;
    let translation = align + c - rotation * (scale * c);
    Transform {
        translation,
        scale,
        rotation,
    }
}

/// Toggle du vol libre (dev) par « / » (Slash ou pavé numérique ÷).
fn sys_toggle_fly(keys: Res<ButtonInput<KeyCode>>, mut fly: ResMut<FlyMode>) {
    if keys.just_pressed(KeyCode::Slash) || keys.just_pressed(KeyCode::NumpadDivide) {
        fly.0 = !fly.0;
        info!(
            "[castle-ground] Vol libre {}",
            if fly.0 {
                "ACTIVÉ (ZQSD + Espace=haut / Ctrl=bas, Maj=rapide) — récup chute désarmée"
            } else {
                "désactivé"
            }
        );
    }
}

/// Vol noclip (dev) : déplace directement le joueur (traverse tout), sans gravité,
/// pour naviguer et aligner le sol. ZQSD = horizontal (relatif au regard), Espace =
/// haut, Ctrl = bas, Maj = boost. Surcharge `kcc.translation` APRÈS `player_movement`
/// (même pattern que dash) pour annuler le pas de gravité. Inerte hors vol.
fn sys_fly_move(
    keys: Res<ButtonInput<KeyCode>>,
    fly: Res<FlyMode>,
    time: Res<Time>,
    mut q: Query<
        (
            &mut Transform,
            &mut KinematicCharacterController,
            &mut Player,
        ),
        With<Player>,
    >,
) {
    if !fly.0 {
        return;
    }
    let Ok((mut tf, mut kcc, mut player)) = q.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let fwd = tf.forward().as_vec3();
    let right = tf.right().as_vec3();
    let flat_fwd = Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero();
    let flat_right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
    let mut dir = Vec3::ZERO;
    // Touches physiques : KeyW/A/S/D = position AZERTY ZQSD (KeyCode = layout physique).
    if keys.pressed(KeyCode::KeyW) {
        dir += flat_fwd;
    }
    if keys.pressed(KeyCode::KeyS) {
        dir -= flat_fwd;
    }
    if keys.pressed(KeyCode::KeyD) {
        dir += flat_right;
    }
    if keys.pressed(KeyCode::KeyA) {
        dir -= flat_right;
    }
    if keys.pressed(KeyCode::Space) {
        dir += Vec3::Y;
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        dir -= Vec3::Y;
    }
    let boost = if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        FLY_BOOST
    } else {
        1.0
    };
    tf.translation += dir.normalize_or_zero() * FLY_SPEED * boost * dt;
    // noclip : pas de déplacement physique + pas d'accumulation de gravité.
    kcc.translation = None;
    player.vertical_velocity = 0.0;
}

/// Outil de calage LIVE au pavé numérique (dev). Déplace / tourne / échelle le sol
/// en temps réel pour l'aligner à l'œil sur le château, puis **Numpad0 = sauvegarde**
/// dans `castle_ground_tune.json` (la valeur redevient donnée persistée). La ressource
/// `GroundTune` est la vérité, éditée au clavier, appliquée aux entités du sol.
///
/// Pavé num : 4/6 = X∓, 8/2 = Z∓, 9/3 = Y±, 7/1 = yaw∓, +/− = relief (vscale).
/// Maj = pas grossier (5 m / 15° / 0.05), sinon fin (0.5 m / 5° / 0.01).
fn sys_nudge_ground(
    keys: Res<ButtonInput<KeyCode>>,
    mut tune: ResMut<GroundTune>,
    mut q: Query<&mut Transform, With<CastleGroundMarker>>,
) {
    let coarse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let dp = if coarse { 5.0 } else { 0.5 }; // pas position (m)
    let ds = if coarse { 0.05 } else { 0.01 }; // pas relief (vscale)
    let dyaw = if coarse { 15.0 } else { 5.0 }; // pas rotation (deg)

    let mut changed = false;
    if keys.just_pressed(KeyCode::Numpad4) {
        tune.align.x -= dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad6) {
        tune.align.x += dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad8) {
        tune.align.z -= dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad2) {
        tune.align.z += dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad9) {
        tune.align.y += dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad3) {
        tune.align.y -= dp;
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad7) {
        tune.yaw_deg = (tune.yaw_deg - dyaw).clamp(-180.0, 180.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::Numpad1) {
        tune.yaw_deg = (tune.yaw_deg + dyaw).clamp(-180.0, 180.0);
        changed = true;
    }
    if keys.just_pressed(KeyCode::NumpadAdd) {
        tune.vscale = (tune.vscale + ds).clamp(0.50, 1.25);
        changed = true;
    }
    if keys.just_pressed(KeyCode::NumpadSubtract) {
        tune.vscale = (tune.vscale - ds).clamp(0.50, 1.25);
        changed = true;
    }

    if changed {
        let transform = ground_transform(tune.align, tune.vscale, tune.yaw_deg);
        for mut tf in &mut q {
            *tf = transform;
        }
        info!(
            "[castle-ground] align=[{:.1},{:.1},{:.1}] vscale={:.2} yaw={:.0}°  (Numpad0=sauver)",
            tune.align.x, tune.align.y, tune.align.z, tune.vscale, tune.yaw_deg
        );
    }

    if keys.just_pressed(KeyCode::Numpad0) {
        let json = format!(
            "{{\"align\":[{:.2},{:.2},{:.2}],\"vscale\":{:.3},\"yaw_deg\":{:.1}}}\n",
            tune.align.x, tune.align.y, tune.align.z, tune.vscale, tune.yaw_deg
        );
        match std::fs::write(TUNE_PATH, &json) {
            Ok(()) => info!("[castle-ground] SAUVÉ -> {TUNE_PATH} : {}", json.trim()),
            Err(e) => warn!("[castle-ground] échec sauvegarde {TUNE_PATH}: {e}"),
        }
    }
}

/// Parse minimal du fichier de calage (sans dep serde). Champs manquants →
/// valeurs des consts. Format : `{"align":[x,y,z],"vscale":f,"yaw_deg":f}`.
fn parse_tune(s: &str) -> (Vec3, f32, f32) {
    let mut align = TERRAIN_ALIGN;
    if let Some(i) = s
        .find("\"align\"")
        .and_then(|k| s[k..].find('[').map(|b| k + b + 1))
    {
        if let Some(end) = s[i..].find(']') {
            let vals: Vec<f32> = s[i..i + end]
                .split(',')
                .filter_map(|t| t.trim().parse().ok())
                .collect();
            if vals.len() == 3 {
                align = Vec3::new(vals[0], vals[1], vals[2]);
            }
        }
    }
    let vscale = num_after(s, "\"vscale\"")
        .unwrap_or(TERRAIN_VSCALE)
        .clamp(0.50, 1.25);
    let yaw = num_after(s, "\"yaw_deg\"")
        .unwrap_or(0.0)
        .clamp(-180.0, 180.0);
    (align, vscale, yaw)
}

/// Extrait le premier nombre suivant `key` (après le `:`).
fn num_after(s: &str, key: &str) -> Option<f32> {
    let k = s.find(key)? + key.len();
    let rest = s[k..].trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    let b = rest.as_bytes();
    let mut j = 0;
    while j < b.len() && (b[j].is_ascii_digit() || matches!(b[j], b'-' | b'+' | b'.' | b'e' | b'E'))
    {
        j += 1;
    }
    rest[..j].parse().ok()
}

fn cleanup_castle_ground(mut commands: Commands, q: Query<Entity, With<CastleGroundMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[castle-ground] Sol nettoyé : {count} entité(s)");
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

fn severity_for_ground(
    active: bool,
    scene_state: &str,
    mesh_count: u32,
    collision_state: &str,
    collision_count: u32,
) -> (&'static str, &'static str) {
    if !active {
        ("ok", "")
    } else if scene_state == "failed" {
        (
            "critical",
            "castle ground failed to load — check castle_terrain.glb path/asset",
        )
    } else if scene_state == "loaded" && mesh_count == 0 {
        (
            "warn",
            "castle ground loaded but no mesh instantiated — re-export terrain GLB",
        )
    } else if collision_state == "failed" {
        (
            "critical",
            "castle terrain collision failed to load — check collision GLB",
        )
    } else if scene_state == "loaded" && collision_count == 0 {
        (
            "warn",
            "castle terrain is visible but its navigation collider is not ready",
        )
    } else {
        ("ok", "")
    }
}

/// Capteur du sol gazon : présence, chargement, plage Y effective en jeu.
fn sys_write_castle_ground_sensor(
    time: Res<Time>,
    game_mode: Res<State<GameMode>>,
    asset_server: Res<AssetServer>,
    visual_roots: Query<(Entity, &SceneRoot, &Transform), With<CastleGroundVisual>>,
    collision_roots: Query<(Entity, &SceneRoot), With<CastleGroundCollision>>,
    children: Query<&Children>,
    meshes: Query<(), With<Mesh3d>>,
    colliders: Query<(), With<Collider>>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < SENSOR_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    let active = *game_mode.get() == GameMode::CastleHub;
    let root = visual_roots.iter().next();
    let collision_root = collision_roots.iter().next();
    let scene_state = scene_load_status(&asset_server, root.map(|(_, scene, _)| scene));
    let collision_state = scene_load_status(&asset_server, collision_root.map(|(_, scene)| scene));
    let mut mesh_count = 0u32;
    if let Some((root_entity, _, _)) = root {
        let mut stack = vec![root_entity];
        while let Some(entity) = stack.pop() {
            if meshes.get(entity).is_ok() {
                mesh_count = mesh_count.saturating_add(1);
            }
            if let Ok(list) = children.get(entity) {
                stack.extend(list.iter());
            }
        }
    }
    let mut collision_count = 0u32;
    if let Some((root_entity, _)) = collision_root {
        let mut stack = vec![root_entity];
        while let Some(entity) = stack.pop() {
            if colliders.get(entity).is_ok() {
                collision_count = collision_count.saturating_add(1);
            }
            if let Ok(list) = children.get(entity) {
                stack.extend(list.iter());
            }
        }
    }
    // Les bornes locales du GLB ne partent pas de zéro : applique l'échelle et
    // la translation réelles, sinon le capteur peut valider un terrain qui
    // coupe visuellement le Hall.
    let (align_y, vscale) = root
        .map(|(_, _, t)| (t.translation.y, t.scale.y))
        .unwrap_or((TERRAIN_ALIGN.y, TERRAIN_VSCALE));
    let y_min = align_y + TERRAIN_BAKED_MIN_Y * vscale;
    let y_max = align_y + TERRAIN_BAKED_MAX_Y * vscale;
    let (severity, next_step) = severity_for_ground(
        active,
        scene_state,
        mesh_count,
        collision_state,
        collision_count,
    );
    let json = format!(
        r#"{{"id":"castle_ground","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"active":{active},"scene_state":"{scene_state}","present":{},"meshes":{mesh_count},"collision_scene_state":"{collision_state}","collision_count":{collision_count},"collision_triangles":{TERRAIN_COLLISION_TRIANGLES},"align_y":{align_y:.2},"vscale":{vscale:.2},"surface_y_range":[{y_min:.2},{y_max:.2}]}}"#,
        time.elapsed_secs(),
        root.is_some(),
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_is_ok() {
        assert_eq!(
            severity_for_ground(false, "not_present", 0, "not_present", 0).0,
            "ok"
        );
    }

    #[test]
    fn failed_scene_is_critical_when_active() {
        assert_eq!(
            severity_for_ground(true, "failed", 0, "not_present", 0).0,
            "critical"
        );
    }

    #[test]
    fn loaded_but_empty_warns() {
        assert_eq!(
            severity_for_ground(true, "loaded", 0, "loaded", 1).0,
            "warn"
        );
        assert_eq!(severity_for_ground(true, "loaded", 1, "loaded", 1).0, "ok");
    }

    #[test]
    fn visible_terrain_without_navigation_collider_warns() {
        assert_eq!(
            severity_for_ground(true, "loaded", 1, "loaded", 0).0,
            "warn"
        );
    }

    #[test]
    fn terrain_bounds_bracket_hall_floor() {
        // Le terrain reconstruit encadre le sol du Hall (~36.5 m) : plateau au niveau
        // du château, falaises en dessous, colline au-dessus.
        let top = TERRAIN_ALIGN.y + TERRAIN_BAKED_MAX_Y * TERRAIN_VSCALE;
        let bottom = TERRAIN_ALIGN.y + TERRAIN_BAKED_MIN_Y * TERRAIN_VSCALE;
        assert!(bottom < 36.5 && 36.5 < top);
    }
}

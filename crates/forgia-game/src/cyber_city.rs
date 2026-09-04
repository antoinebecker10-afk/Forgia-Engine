//! # cyber_city
//!
//! Démo perf moteur (2026-06-15) — entrée menu "Cyber City démo".
//!
//! Charge un GLB lourd (`models/environment/cyberpunk_city.glb`, ~185 MB),
//! l'agrandit à une échelle marchable, lui génère des **colliders TriMesh**
//! (rues + murs walkable). Le déplacement se fait à la **3e personne avec Rex
//! animé** : on réutilise le pipeline locomotion + caméra orbitale du RPG, en
//! élargissant son gating à `GameMode::CyberCity` (cf `forgia-rpg::rex_third_person_active`).
//! Un pass de rendu "cyberpunk crépuscule" (HDR + bloom néon + brume bleu nuit +
//! ambiante froide) est appliqué à la **caméra orbitale** tant qu'on est dans ce
//! mode, puis retiré en sortie.
//!
//! - Entrée  : bouton menu "Cyber City démo" → `GameMode::CyberCity` + `AppMode::InGame`.
//! - Perso   : le Player (capsule physique) est spawné par forgia-player ; Rex +
//!   `LocomotionState` + `OrbitCamera` sont attachés par `forgia-rpg::spawn_rex_character`
//!   (désormais actif aussi en CyberCity). On marche en ZQSD, la souris oriente Rex.
//! - Spawn   : la capsule joueur est replacée via un raycast vertical au-dessus du
//!   centre → posée sur le toit/la rue trouvée (Rex la suit, layout-agnostic).
//! - Sortie  : ESC → Paused, ESC → resume, Q (en pause) → Menu (cleanup auto).

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy_rapier3d::prelude::{AsyncSceneCollider, ComputedColliderShape, RigidBody};
use forgia_camera_orbit::OrbitCamera;
use forgia_core::prelude::*;
use forgia_player::prelude::Player;

/// Chemin de la scène GLB, relatif au dossier `assets/`.
const CITY_SCENE: &str = "models/environment/cyberpunk_city.glb#Scene0";
/// Échelle appliquée au GLB (le pack natif est petit ; ×N pour le rendre
/// marchable). Tunable — me demander pour ajuster.
const CITY_SCALE: f32 = 10.0;
/// Fraction de l'emprise vers le coin où poser Rex (0 = centre, 1 = bord pile).
/// 0.85 = près du bord, hors de l'amas dense central → la caméra orbitale
/// derrière reste dégagée (vue non bouchée par les tours).
const CORNER_INSET: f32 = 0.85;
/// Marge verticale au-dessus du sol de l'emprise (gravité pose ensuite Rex).
const SPAWN_CLEARANCE_M: f32 = 2.0;
/// Position de repli si la géométrie de la ville n'apparaît jamais.
const FALLBACK_SPAWN: Vec3 = Vec3::new(0.0, 60.0, 0.0);
/// Frames d'attente avant fallback (instanciation d'un GLB lourd = quelques s).
const PLACE_TIMEOUT_FRAMES: u32 = 600;

pub struct CyberCityDemoPlugin;

impl Plugin for CyberCityDemoPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::CyberCity), spawn_cyber_city)
            .add_systems(
                OnExit(GameMode::CyberCity),
                (cleanup_cyber_city, remove_cyber_render),
            )
            .add_systems(
                Update,
                (place_player_in_city, ensure_cyber_render)
                    .run_if(in_state(GameMode::CyberCity))
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

/// Marker — entités décor de la démo (scène + lumières), despawn `OnExit`.
#[derive(Component)]
struct CyberCityMarker;

/// Marker sur l'entité racine de la scène ville (pour calculer son emprise AABB).
#[derive(Component)]
struct CyberCitySceneRoot;

/// Marker sur la caméra FPS une fois le pass cyberpunk appliqué (idempotence).
#[derive(Component)]
struct CyberRenderApplied;

/// Pose en attente : le joueur sera replacé dans la ville dès que ses colliders
/// existent (raycast). Consommée puis retirée.
#[derive(Resource, Default)]
struct CyberSpawnPending {
    frames_waited: u32,
}

fn spawn_cyber_city(mut commands: Commands, asset_server: Res<AssetServer>) {
    // Scène GLB agrandie + collider TriMesh statique (rues/murs walkable). Même
    // pattern que forgia-village-loader : scale + SceneRoot + RigidBody::Fixed +
    // AsyncSceneCollider sur la MÊME entité (rapier scale via GlobalTransform).
    commands.spawn((
        CyberCityMarker,
        CyberCitySceneRoot,
        SceneRoot(asset_server.load(CITY_SCENE)),
        Transform::from_scale(Vec3::splat(CITY_SCALE)),
        RigidBody::Fixed,
        AsyncSceneCollider {
            shape: Some(ComputedColliderShape::TriMesh(default())),
            ..default()
        },
        Name::new("CyberCityScene"),
    ));

    // Éclairage crépuscule cyberpunk : key froide (lune/néon) + fill magenta.
    // Émissif néon du pack + bloom HDR feront le reste de l'ambiance.
    // Key warm-white forte (visibilité) — 2026-06-16 : montée 9k→18k + teinte
    // neutre chaude après retour user « trop sombre ».
    commands.spawn((
        CyberCityMarker,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.90),
            illuminance: 18_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, -0.6, 0.0)),
        Name::new("CyberCityKeyLight"),
    ));
    // Fill cool (déboucher les ombres) — magenta saturé retiré (faisait virer
    // toute la scène au violet), remplacé par un bleu doux plus intense.
    commands.spawn((
        CyberCityMarker,
        DirectionalLight {
            color: Color::srgb(0.55, 0.66, 1.0),
            illuminance: 6_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.25, 2.3, 0.0)),
        Name::new("CyberCityFillLight"),
    ));

    commands.insert_resource(CyberSpawnPending::default());
    info!(
        "[cyber-city] Scène demandée ({CITY_SCENE}) scale ×{CITY_SCALE} + colliders TriMesh — placement joueur en attente des colliders"
    );
}

/// Replace le joueur **près d'un coin du périmètre** de la ville (pas au centre
/// dense), face au centre. La caméra orbitale 3P (7 m derrière) reste ainsi
/// dégagée vers l'extérieur au lieu de s'enfoncer dans les tours (écran noir).
/// Calcule l'emprise monde en unionnant les `Aabb` du sous-arbre de la scène
/// (instanciation async → retry jusqu'à ce que les meshes existent).
fn place_player_in_city(
    mut commands: Commands,
    pending: Option<ResMut<CyberSpawnPending>>,
    q_scene: Query<Entity, With<CyberCitySceneRoot>>,
    q_children: Query<&Children>,
    q_aabb: Query<(&GlobalTransform, &Aabb)>,
    mut q_player: Query<(&mut Transform, &mut Player)>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    let Ok((mut tf, mut player)) = q_player.single_mut() else {
        return; // joueur pas encore spawné (OnEnter InGame) — retry.
    };
    let Ok(scene) = q_scene.single() else {
        return;
    };
    pending.frames_waited += 1;

    // Union des Aabb du sous-arbre (BFS) → emprise monde de la ville (scale inclus
    // via GlobalTransform). On transforme les 8 coins de chaque Aabb local.
    let mut wmin = Vec3::splat(f32::MAX);
    let mut wmax = Vec3::splat(f32::MIN);
    let mut found = 0u32;
    let mut stack = vec![scene];
    while let Some(e) = stack.pop() {
        if let Ok((gt, aabb)) = q_aabb.get(e) {
            let c = Vec3::from(aabb.center);
            let h = Vec3::from(aabb.half_extents);
            for sx in [-1.0, 1.0] {
                for sy in [-1.0, 1.0] {
                    for sz in [-1.0, 1.0] {
                        let w = gt.transform_point(c + h * Vec3::new(sx, sy, sz));
                        wmin = wmin.min(w);
                        wmax = wmax.max(w);
                    }
                }
            }
            found += 1;
        }
        if let Ok(children) = q_children.get(e) {
            for child in children.iter() {
                stack.push(child);
            }
        }
    }

    if found > 0 {
        let center = (wmin + wmax) * 0.5;
        let half = (wmax - wmin) * 0.5;
        let pos = Vec3::new(
            center.x + half.x * CORNER_INSET,
            wmin.y + SPAWN_CLEARANCE_M,
            center.z + half.z * CORNER_INSET,
        );
        tf.translation = pos;
        // Oriente Rex vers le centre (forward = -Z tourné par yaw) → la caméra
        // orbitale derrière pointe vers l'extérieur de la ville.
        let dir = (center - pos).normalize_or_zero();
        player.yaw = (-dir.x).atan2(-dir.z);
        tf.rotation = Quat::from_rotation_y(player.yaw);
        info!(
            "[cyber-city] Joueur posé au coin @ ({:.1},{:.1},{:.1}) — emprise X[{:.0}..{:.0}] Z[{:.0}..{:.0}] ({found} meshes)",
            pos.x, pos.y, pos.z, wmin.x, wmax.x, wmin.z, wmax.z
        );
        commands.remove_resource::<CyberSpawnPending>();
    } else if pending.frames_waited >= PLACE_TIMEOUT_FRAMES {
        tf.translation = FALLBACK_SPAWN;
        warn!(
            "[cyber-city] Géométrie ville introuvable après {PLACE_TIMEOUT_FRAMES} frames \
             — repli @ {FALLBACK_SPAWN:?}"
        );
        commands.remove_resource::<CyberSpawnPending>();
    }
}

/// Applique le pass rendu "cyberpunk" sur la caméra FPS (HDR + bloom + brume +
/// ambiante froide). Idempotent (marker), robuste à un respawn caméra.
fn ensure_cyber_render(
    mut commands: Commands,
    q_cam: Query<Entity, (With<OrbitCamera>, Without<CyberRenderApplied>)>,
) {
    for cam in &q_cam {
        // 2026-06-16 : `Hdr` + `Bloom` retirés — posés après-coup sur la caméra
        // orbitale, ils cassaient la passe principale (écran ClearColor nu : ni
        // skybox ni géométrie). `DistanceFog` + `AmbientLight` restent (prouvés
        // sûrs par forgia-mode-roguelite::atmosphere sur FpsCamera).
        commands
            .entity(cam)
            .insert((CyberRenderApplied, cyber_fog(), cyber_ambient()));
        info!("[cyber-city] Pass rendu cyberpunk appliqué (brume + ambiante)");
    }
}

/// Retire le pass rendu de la caméra orbitale (retour rendu normal hors CyberCity).
fn remove_cyber_render(mut commands: Commands, q_cam: Query<Entity, With<OrbitCamera>>) {
    for cam in &q_cam {
        commands
            .entity(cam)
            .remove::<(CyberRenderApplied, DistanceFog, AmbientLight)>();
    }
}

/// Brume bleu nuit : le lointain de la ville se fond dans une nappe froide,
/// la directionnelle perçue à travers garde un halo cyan.
fn cyber_fog() -> DistanceFog {
    // 2026-06-16 : éclairci (couleur + densité ÷2) — l'ancien bleu nuit dense
    // assombrissait trop le lointain.
    DistanceFog {
        color: Color::srgb(0.14, 0.15, 0.22),
        directional_light_color: Color::srgb(0.7, 0.80, 1.0),
        directional_light_exponent: 30.0,
        falloff: FogFalloff::Exponential { density: 0.0018 },
    }
}

/// Ambiante (déboucher les ombres pour que les textures se lisent). 2026-06-16 :
/// brightness 180→850 + teinte quasi neutre (cool léger) après « trop sombre ».
fn cyber_ambient() -> AmbientLight {
    AmbientLight {
        color: Color::srgb(0.68, 0.72, 0.82),
        brightness: 850.0,
        ..default()
    }
}

fn cleanup_cyber_city(mut commands: Commands, q: Query<Entity, With<CyberCityMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<CyberSpawnPending>();
    info!("[cyber-city] Démo nettoyée : {count} entités despawn");
}

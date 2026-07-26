//! Porte boss-gated posée sur le dais melee_pit central (story-603).
//!
//! La porte est posée sur le DAIS EXISTANT au centre de l'arène : le module
//! `melee_pit` (forgia-stage, prefab CirclePlatformSmall, ancre AnchorKind::MeleePit).
//! Ce dais est placé par le seed (bouge chaque run) et n'a PAS de collider →
//! `sys_reconcile_boss_gate` lit son ancre, lui ajoute un collider et y pose la porte.
//!
//! Ce module est une extraction pure de `loot_room.rs` — 0 changement de comportement.

use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{
    Collider, ComputedColliderShape, QueryFilter, ReadRapierContext, RigidBody,
};
use forgia_anchor::{AnchorKind, AnchorPoint};
use forgia_core::prelude::*;

use crate::loot_room::{LootRoomMarker, Portal, PortalKind};
use crate::waves::RogueliteWave;

// ── Constantes boss-gate ──────────────────────────────────────────────────────

/// Repli central si aucune ancre MeleePit (stage sans melee_pit).
pub(crate) const DAIS_FALLBACK_POS: Vec3 = Vec3::new(0.0, 0.0, -16.0);
/// Préfixe du nom de l'entité du dais (module melee_pit posé par forgia-stage).
const DAIS_MODULE_NAME_PREFIX: &str = "Module_melee_pit";
/// Hauteur cible du portail (calibrée via AABB, GLB de taille native inconnue).
const PORTAL_TARGET_HEIGHT: f32 = 7.0;
/// Décalage Y du portail au-dessus du sommet MESURÉ du dais (tunable selon pivot GLB).
const PORTAL_Y_OFFSET: f32 = 0.0;
/// Décalage de yaw du portail (la porte regarde le spawn). Axe avant du GLB =
/// +Z (la porte tournait le dos) → `PI` (180°). `±FRAC_PI_2` si jamais de profil.
const PORTAL_YAW_OFFSET: f32 = std::f32::consts::PI;
// pub(crate) — consommés aussi par le warmup de pipelines (story-664).
pub(crate) const PORTAL_CLOSED_GLB: &str = "models/environment/portal/portal_closed.glb";
pub(crate) const PORTAL_OPEN_GLB: &str = "models/environment/portal/portal_open.glb";
/// Nb de frames d'échec du raycast de surface AVANT le fallback AABB durci. Le cas
/// normal résout en < 10 frames ; ce seuil (~2 s @ 60 fps) ne déclenche que si le
/// collider du dais ne s'enregistre jamais (charge authored 11 GLB = ordre variable,
/// cf disparition intermittente de la porte). Évite le `return` éternel = porte fantôme.
const RAYCAST_MAX_ATTEMPTS: u32 = 120;
/// Seuil health-check : dais trouvé mais porte non posée depuis > N s → severity warn
/// dans `forgia2_boss_gate.json` (rend la disparition intermittente diagnosticable).
const GATE_STUCK_WARN_SECS: f32 = 5.0;
/// Positions (locales au centre-base du portail = `portal_pos`) des 4 yeux des 2
/// crânes flanquant l'arche. Tunable : si les flammes ne sont pas pile dans les
/// orbites, ajuster X (écart crânes), Y (hauteur), Z (avancée face joueur, +Z).
const SKULL_EYE_OFFSETS: [Vec3; 4] = [
    Vec3::new(-2.30, 0.50, 1.30), // crâne gauche — œil externe
    Vec3::new(-1.95, 0.50, 1.30), // crâne gauche — œil interne
    Vec3::new(1.95, 0.50, 1.30),  // crâne droit — œil interne
    Vec3::new(2.30, 0.50, 1.30),  // crâne droit — œil externe
];

// ── Composants ────────────────────────────────────────────────────────────────

/// Story-603 — entité racine de la porte FERMÉE (GLB + collider bloquant).
#[derive(Component)]
pub(crate) struct ClosedPortal;

/// Story-603 — entité racine de la porte OUVERTE (GLB + `Portal{Enter}` walk-over).
#[derive(Component)]
pub(crate) struct OpenPortal;

/// Story-603 — posé sur un SceneRoot de portail : `sys_calibrate_portal` mesure
/// l'AABB une fois la scène chargée et applique `scale = target_h / max_dim`
/// (GLB de taille native inconnue), puis passe le relais à `NeedsPortalGround`.
#[derive(Component)]
pub(crate) struct NeedsPortalCalibrate {
    pub(crate) target_h: f32,
    /// Y monde où la BASE du portail doit reposer (= sommet du socle).
    pub(crate) base_world_y: f32,
}

/// Story-603 — après calibration (scale propagé), `sys_ground_portal` décale le
/// SceneRoot en Y pour que la base réelle de la géométrie repose sur `base_world_y`
/// (corrige « le portail est dans le socle » — pivot GLB non au pied).
#[derive(Component)]
pub(crate) struct NeedsPortalGround {
    pub(crate) base_world_y: f32,
}

/// Story-603 — flamme dans un œil de crâne du portail. Scintille (flicker) via
/// `sys_flicker_portal_flames` (intensité PointLight modulée + phase par œil).
#[derive(Component)]
pub(crate) struct PortalFlame {
    pub(crate) base_intensity: f32,
    pub(crate) phase: f32,
}

// ── Resource ──────────────────────────────────────────────────────────────────

/// Story-603 — état de la porte posée sur le dais melee_pit. `opened` suit
/// `RogueliteWave.boss_defeated` via `sys_reconcile_boss_gate` (fermée→ouverte au
/// boss-clear, ré-fermée au nouveau run). `placed_at` = centre du dais courant
/// (re-pose si le seed déplace le dais). `portal_pos` = sommet du dais (pose porte) ;
/// `parcours_entry` = pad zone 1.
#[derive(Resource, Default)]
pub struct BossGate {
    pub(crate) opened: bool,
    pub(crate) placed_at: Option<Vec3>,
    /// True quand les colliders TriMesh ont été ajoutés au mesh du dais (étape avant
    /// le raycast de surface — il faut 1 frame que rapier les enregistre).
    pub(crate) dais_ready: bool,
    /// Surface marchable au CENTRE du dais (raycast) — `None` tant que pas mesurée.
    /// La porte attend cette valeur → posée PILE sur le sol (pas l'AABB max).
    pub(crate) dais_top_y: Option<f32>,
    pub(crate) portal_pos: Vec3,
    pub parcours_entry: Vec3,
    /// Diag (exposé `forgia2_boss_gate.json`) — étape courante de pose de la porte.
    pub(crate) diag_stage: GateStage,
    /// Compteur de frames d'échec du raycast de surface depuis la dernière (re)pose.
    /// Déclenche le fallback AABB au-delà de [`RAYCAST_MAX_ATTEMPTS`]. Reset à la repose.
    pub(crate) raycast_attempts: u32,
}

/// Étape courante de pose de la porte du dais (diag — miroir `forgia2_boss_gate.json`).
/// Rend la disparition intermittente de la porte observable : on voit à QUELLE étape
/// la cascade `find_dais → solidify → raycast → pose` bloque sur un run malchanceux.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum GateStage {
    /// `find_dais_root` échoue — module melee_pit pas (encore) spawné, ou hors run.
    #[default]
    NoDais,
    /// Dais trouvé, colliders TriMesh en cours d'ajout (attend chargement meshes).
    Solidifying,
    /// Colliders prêts, raycast de surface en cours (attend enregistrement rapier).
    Raycasting,
    /// Porte FERMÉE posée sur le dais.
    Placed,
    /// Porte OUVERTE (boss vaincu → parcours débloqué).
    Open,
}

impl GateStage {
    /// Label stable (snake_case) pour le sensor JSON.
    pub(crate) fn label(self) -> &'static str {
        match self {
            GateStage::NoDais => "no_dais",
            GateStage::Solidifying => "solidifying",
            GateStage::Raycasting => "raycasting",
            GateStage::Placed => "placed",
            GateStage::Open => "open",
        }
    }
}

// ── Helpers privés ────────────────────────────────────────────────────────────

/// Entité racine du dais melee_pit (nom `Module_melee_pit*`, posé par forgia-stage).
fn find_dais_root(q_named: &Query<(Entity, &Name)>) -> Option<Entity> {
    q_named
        .iter()
        .find(|(_, n)| n.as_str().starts_with(DAIS_MODULE_NAME_PREFIX))
        .map(|(e, _)| e)
}

/// Walk récursif : collecte les entités `Mesh3d` sous `root`.
fn collect_mesh_descendants(
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    out: &mut Vec<Entity>,
) {
    if q_mesh.contains(root) {
        out.push(root);
    }
    if let Ok(children) = q_children.get(root) {
        for c in children.iter() {
            collect_mesh_descendants(c, q_children, q_mesh, out);
        }
    }
}

/// Ajoute un collider TriMesh à chaque mesh du dais (→ solide + marchable, épouse
/// le visuel exact, visible F10). Retourne `true` quand tous les meshes sont chargés
/// et collidérés (retry sinon). Idempotent (skip si déjà collideré).
fn solidify_dais(
    commands: &mut Commands,
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    q_has_col: &Query<(), With<Collider>>,
    meshes: &Assets<Mesh>,
) -> bool {
    let mut mesh_ents = Vec::new();
    collect_mesh_descendants(root, q_children, q_mesh, &mut mesh_ents);
    if mesh_ents.is_empty() {
        return false; // scène pas encore peuplée
    }
    let all_loaded = mesh_ents
        .iter()
        .all(|e| q_mesh.get(*e).ok().and_then(|m| meshes.get(&m.0)).is_some());
    if !all_loaded {
        return false;
    }
    for &e in &mesh_ents {
        if q_has_col.get(e).is_ok() {
            continue; // déjà collideré
        }
        if let Ok(m3d) = q_mesh.get(e) {
            if let Some(mesh) = meshes.get(&m3d.0) {
                if let Some(col) =
                    Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(default()))
                {
                    commands.entity(e).try_insert(col);
                }
            }
        }
    }
    true
}

/// Surface marchable au CENTRE du dais via raycast vertical (sur les colliders
/// qu'on vient d'ajouter). Donne le VRAI sol où poser la porte — contrairement à
/// l'AABB max qui captait une déco haute (totem/braséro) → top=3.67 m alors que le
/// sol est ~1 m. `None` si rien touché (colliders pas encore enregistrés → retry).
fn raycast_dais_surface(center: Vec3, rapier: &ReadRapierContext) -> Option<f32> {
    let ctx = rapier.single().ok()?;
    let origin = Vec3::new(center.x, center.y + 60.0, center.z);
    ctx.cast_ray(origin, Vec3::NEG_Y, 120.0, true, QueryFilter::default())
        .map(|(_, toi)| origin.y - toi)
}

/// Porte FERMÉE : GLB visuel (calibré AABB + posé sur le socle) + collider cuboïde
/// BLOQUANT (le joueur ne peut pas la franchir) + flammes dans les yeux des crânes.
/// PAS de composant `Portal` → ignorée par le walk-over.
fn spawn_closed_portal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
) {
    let glb = asset_server.load(GltfAssetLabel::Scene(0).from_asset(PORTAL_CLOSED_GLB));
    let parent = commands
        .spawn((
            LootRoomMarker,
            DespawnOnExit(GameMode::Roguelite),
            ClosedPortal,
            Name::new("ClosedPortal"),
            RigidBody::Fixed,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    // Collider bloquant dimensionné depuis la cible (fiable, indépendant du
    // chargement async du GLB — cf decor.rs : collider primitif au spawn).
    commands.spawn((
        ChildOf(parent),
        Name::new("ClosedPortalCollider"),
        Transform::from_xyz(0.0, PORTAL_TARGET_HEIGHT * 0.5, 0.0),
        Collider::cuboid(PORTAL_TARGET_HEIGHT * 0.30, PORTAL_TARGET_HEIGHT * 0.5, 0.6),
    ));
    // Visuel GLB (scale calibré + posé sur le socle par sys_calibrate/ground_portal).
    commands.spawn((
        ChildOf(parent),
        Name::new("ClosedPortalVisual"),
        SceneRoot(glb),
        Transform::IDENTITY,
        NeedsPortalCalibrate {
            target_h: PORTAL_TARGET_HEIGHT,
            base_world_y: pos.y,
        },
    ));
    spawn_eye_flames(commands, meshes, materials, parent);
}

/// Porte OUVERTE : parent (`Portal{Enter}` walk-over → parcours + glow teal +
/// flammes des yeux) + enfant visuel GLB (calibré + posé sur le socle). PAS de
/// collider bloquant (on entre dedans). Spawné au boss-clear.
#[allow(clippy::too_many_arguments)]
fn spawn_open_portal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    target: Vec3,
    yaw: f32,
) {
    let glb = asset_server.load(GltfAssetLabel::Scene(0).from_asset(PORTAL_OPEN_GLB));
    let parent = commands
        .spawn((
            LootRoomMarker,
            DespawnOnExit(GameMode::Roguelite),
            OpenPortal,
            Portal {
                kind: PortalKind::Enter,
                target,
            },
            Name::new("OpenPortal"),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            children![(
                PointLight {
                    color: Color::srgb(0.3, 1.0, 1.2),
                    intensity: 4_000.0,
                    range: 18.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, PORTAL_TARGET_HEIGHT * 0.5, 0.0),
            )],
        ))
        .id();
    commands.spawn((
        ChildOf(parent),
        Name::new("OpenPortalVisual"),
        SceneRoot(glb),
        Transform::IDENTITY,
        NeedsPortalCalibrate {
            target_h: PORTAL_TARGET_HEIGHT,
            base_world_y: pos.y,
        },
    ));
    spawn_eye_flames(commands, meshes, materials, parent);
}

/// Spawn les 4 flammes (1 par œil de crâne) en enfants du `parent` du portail,
/// aux offsets `SKULL_EYE_OFFSETS`. Chaque flamme = sphère émissive orange +
/// PointLight scintillant (`sys_flicker_portal_flames`). Mesh+matériau partagés.
fn spawn_eye_flames(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
) {
    let mesh = meshes.add(Sphere::new(0.12));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.5, 0.12),
        emissive: LinearRgba::new(7.0, 2.2, 0.3, 1.0),
        unlit: true,
        ..default()
    });
    for (i, off) in SKULL_EYE_OFFSETS.iter().enumerate() {
        commands.spawn((
            ChildOf(parent),
            Name::new("PortalEyeFlame"),
            PortalFlame {
                base_intensity: 2_200.0,
                phase: i as f32 * 1.7,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(*off),
            PointLight {
                color: Color::srgb(1.0, 0.45, 0.12),
                intensity: 2_200.0,
                range: 4.5,
                shadows_enabled: false,
                ..default()
            },
        ));
    }
}

/// Walk récursif : min Y monde sur tous les `Aabb` du sous-arbre (8 coins
/// transformés par leur `GlobalTransform`, robuste aux rotations).
fn collect_world_min_y(
    e: Entity,
    q_children: &Query<&Children>,
    q_gt_aabb: &Query<(&GlobalTransform, &Aabb)>,
    acc: &mut f32,
    found: &mut bool,
) {
    if let Ok((gt, aabb)) = q_gt_aabb.get(e) {
        let c = Vec3::from(aabb.center);
        let he = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = c + Vec3::new(sx * he.x, sy * he.y, sz * he.z);
                    *acc = acc.min(gt.transform_point(corner).y);
                }
            }
        }
        *found = true;
    }
    if let Ok(children) = q_children.get(e) {
        for child in children.iter() {
            collect_world_min_y(child, q_children, q_gt_aabb, acc, found);
        }
    }
}

/// Walk récursif : MAX Y monde sur tous les `Aabb` du sous-arbre (8 coins
/// transformés). Symétrique de [`collect_world_min_y`].
fn collect_world_max_y(
    e: Entity,
    q_children: &Query<&Children>,
    q_gt_aabb: &Query<(&GlobalTransform, &Aabb)>,
    acc: &mut f32,
    found: &mut bool,
) {
    if let Ok((gt, aabb)) = q_gt_aabb.get(e) {
        let c = Vec3::from(aabb.center);
        let he = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = c + Vec3::new(sx * he.x, sy * he.y, sz * he.z);
                    *acc = acc.max(gt.transform_point(corner).y);
                }
            }
        }
        *found = true;
    }
    if let Ok(children) = q_children.get(e) {
        for child in children.iter() {
            collect_world_max_y(child, q_children, q_gt_aabb, acc, found);
        }
    }
}

/// Sommet monde du SOUS-ARBRE du dais (sa propre géométrie seulement, pas les
/// décos voisines). Fallback durci du raycast quand le collider ne s'enregistre
/// jamais : un sommet approximatif vaut mieux qu'une porte fantôme absente.
fn dais_aabb_top(
    root: Entity,
    q_children: &Query<&Children>,
    q_gt_aabb: &Query<(&GlobalTransform, &Aabb)>,
) -> Option<f32> {
    let mut acc = f32::MIN;
    let mut found = false;
    collect_world_max_y(root, q_children, q_gt_aabb, &mut acc, &mut found);
    found.then_some(acc)
}

/// Walk récursif des Children pour le 1er `Aabb` ; `max(half_extents)*2`.
fn portal_aabb_max_dim(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<f32> {
    if let Ok(a) = q_aabb.get(root) {
        return Some(a.half_extents.max_element() * 2.0);
    }
    let children = q_children.get(root).ok()?;
    let mut max = 0.0_f32;
    let mut found = false;
    for child in children.iter() {
        if let Some(d) = portal_aabb_max_dim(child, q_aabb, q_children) {
            max = max.max(d);
            found = true;
        }
    }
    if found {
        Some(max)
    } else {
        None
    }
}

// ── Systèmes ──────────────────────────────────────────────────────────────────

/// Story-603 — ouvre/ferme la porte du socle en suivant `RogueliteWave.boss_defeated`.
/// Boss vaincu → fermée→ouverte (parcours débloqué). Nouveau run (reset
/// `boss_defeated`) → ouverte→fermée. Cheap : agit seulement sur transition.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sys_reconcile_boss_gate(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    wave: Res<RogueliteWave>,
    mut gate: ResMut<BossGate>,
    q_anchor: Query<(&AnchorPoint, &Transform)>,
    q_named: Query<(Entity, &Name)>,
    q_children: Query<&Children>,
    q_mesh3d: Query<&Mesh3d>,
    q_has_col: Query<(), With<Collider>>,
    q_closed: Query<Entity, With<ClosedPortal>>,
    q_open: Query<Entity, With<OpenPortal>>,
    rapier: ReadRapierContext,
    // Fallback durci : sommet AABB du dais si le raycast ne résout jamais.
    q_gt_aabb: Query<(&GlobalTransform, &Aabb)>,
) {
    // 1. Centre du dais = ancre MeleePit (suit le seed) ; sinon garde la place
    //    actuelle ; sinon repli central (1re frame, avant chargement du stage).
    let anchor_pos = q_anchor
        .iter()
        .find(|(a, _)| a.kind == AnchorKind::MeleePit)
        .map(|(_, t)| t.translation);
    let dais_center = anchor_pos.or(gate.placed_at).unwrap_or(DAIS_FALLBACK_POS);
    // Yaw : la porte regarde le spawn (origine), +PORTAL_YAW_OFFSET (PI) car l'avant
    // natif du GLB est +Z (sans ça la porte tournait le dos au joueur).
    let yaw = dais_center.x.atan2(dais_center.z) + PORTAL_YAW_OFFSET;

    // 2. (Re)pose si le dais a bougé (nouveau run/seed) ou jamais posé : on efface
    //    nos portes et on ré-arme la mesure. Les colliders TriMesh ajoutés au mesh du
    //    dais partent avec lui (cleanup forgia-stage au reload de stage).
    let moved = gate
        .placed_at
        .map(|p| p.distance(dais_center) > 0.5)
        .unwrap_or(true);
    if moved {
        for e in q_closed.iter().chain(&q_open) {
            commands.entity(e).despawn();
        }
        gate.placed_at = Some(dais_center);
        gate.dais_ready = false;
        gate.dais_top_y = None;
        gate.opened = false;
        gate.raycast_attempts = 0;
        gate.diag_stage = GateStage::NoDais;
        return;
    }

    // 3a. Solidifie le dais : collider TriMesh sur SON mesh (épouse le visuel,
    //     marchable, F10). Attend 1 frame ensuite que rapier l'enregistre.
    if !gate.dais_ready {
        let Some(root) = find_dais_root(&q_named) else {
            gate.diag_stage = GateStage::NoDais;
            return; // module melee_pit pas encore spawné
        };
        if !solidify_dais(
            &mut commands,
            root,
            &q_children,
            &q_mesh3d,
            &q_has_col,
            &meshes,
        ) {
            gate.diag_stage = GateStage::Solidifying;
            return; // meshes du dais pas tous chargés → retry
        }
        gate.dais_ready = true;
        gate.diag_stage = GateStage::Raycasting;
        return;
    }

    // 3b. Surface marchable au CENTRE via raycast (sur les colliders ajoutés) →
    //     porte posée PILE sur le sol. Fini l'AABB max qui captait une déco haute
    //     (totem/braséro) → top=3.67 m alors que le sol est ~1 m.
    if gate.dais_top_y.is_none() {
        gate.diag_stage = GateStage::Raycasting;
        gate.raycast_attempts = gate.raycast_attempts.saturating_add(1);
        // Voie normale : raycast de la surface (sol du stage à y≈0 → un hit ≤ 0.5 =
        // collider du dais pas encore enregistré, rayon traversé jusqu'au sol).
        let raycast_top = raycast_dais_surface(dais_center, &rapier).filter(|t| *t > 0.5);
        // Fallback DURCI : si après RAYCAST_MAX_ATTEMPTS le raycast n'a jamais résolu
        // (collider du dais authored jamais enregistré → cause de la disparition
        // intermittente), on retombe sur le sommet AABB du SOUS-ARBRE du dais. Un
        // sommet approximatif vaut mieux qu'une porte fantôme absente.
        let (top, via_fallback) = match raycast_top {
            Some(t) => (Some(t), false),
            None if gate.raycast_attempts >= RAYCAST_MAX_ATTEMPTS => {
                let aabb_top = find_dais_root(&q_named)
                    .and_then(|root| dais_aabb_top(root, &q_children, &q_gt_aabb))
                    .filter(|t| *t > 0.5);
                (aabb_top, aabb_top.is_some())
            }
            None => (None, false),
        };
        let Some(top) = top else {
            return; // pas encore résolu (sous le seuil d'attempts) → retry
        };
        let portal_pos = Vec3::new(dais_center.x, top + PORTAL_Y_OFFSET, dais_center.z);
        spawn_closed_portal(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            portal_pos,
            yaw,
        );
        gate.dais_top_y = Some(top);
        gate.portal_pos = portal_pos;
        gate.diag_stage = GateStage::Placed;
        info!(
            "[boss-gate] surface dais y={top:.2}m ({}) → porte posée @ ({:.1},{:.1},{:.1}), yaw={:.0}° (après {} tentatives)",
            if via_fallback { "FALLBACK AABB" } else { "raycast" },
            portal_pos.x, portal_pos.y, portal_pos.z, yaw.to_degrees(), gate.raycast_attempts
        );
        return;
    }

    // 4. Swap fermée↔ouverte selon boss_defeated.
    if wave.boss_defeated && !gate.opened {
        for e in &q_closed {
            commands.entity(e).despawn();
        }
        spawn_open_portal(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            gate.portal_pos,
            gate.parcours_entry,
            yaw,
        );
        gate.opened = true;
        gate.diag_stage = GateStage::Open;
        info!("[boss-gate] BOSS VAINCU → porte du dais OUVERTE (parcours débloqué)");
    } else if !wave.boss_defeated && gate.opened {
        for e in &q_open {
            commands.entity(e).despawn();
        }
        spawn_closed_portal(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            gate.portal_pos,
            yaw,
        );
        gate.opened = false;
        gate.diag_stage = GateStage::Placed;
        info!("[boss-gate] nouveau run → porte du dais REFERMÉE");
    }
}

/// Scale un SceneRoot de portail à `target_h` une fois l'AABB chargée (GLB de
/// taille native inconnue). Mirroir de `decor::sys_calibrate_decor`.
pub(crate) fn sys_calibrate_portal(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsPortalCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
) {
    for (e, needs) in &q_needs {
        let Some(max_dim) = portal_aabb_max_dim(e, &q_aabb, &q_children) else {
            continue; // scène pas encore chargée → retry next frame
        };
        if max_dim <= 0.0 || !max_dim.is_finite() {
            commands.entity(e).remove::<NeedsPortalCalibrate>();
            continue;
        }
        let scale = needs.target_h / max_dim;
        if let Ok(mut tf) = q_tf.get_mut(e) {
            tf.scale = Vec3::splat(scale);
        }
        // Passe le relais au grounding : une fois le scale propagé (frame
        // suivante), `sys_ground_portal` posera la base réelle sur le socle.
        commands
            .entity(e)
            .remove::<NeedsPortalCalibrate>()
            .insert(NeedsPortalGround {
                base_world_y: needs.base_world_y,
            });
    }
}

/// Story-603 — pose la BASE réelle du portail sur le socle. Après le scale (frame
/// précédente, GlobalTransform propagé), mesure le min Y monde de la géométrie et
/// décale le SceneRoot pour que ce min repose sur `base_world_y` (corrige pivot GLB
/// non centré au pied → « le portail est dans le socle »).
pub(crate) fn sys_ground_portal(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsPortalGround)>,
    q_children: Query<&Children>,
    q_gt_aabb: Query<(&GlobalTransform, &Aabb)>,
    mut q_tf: Query<&mut Transform>,
) {
    for (root, ground) in &q_needs {
        let mut min_y = f32::MAX;
        let mut found = false;
        collect_world_min_y(root, &q_children, &q_gt_aabb, &mut min_y, &mut found);
        if !found {
            continue; // GlobalTransform/Aabb pas encore propagés → retry
        }
        let delta = ground.base_world_y - min_y;
        if let Ok(mut tf) = q_tf.get_mut(root) {
            tf.translation.y += delta;
        }
        commands.entity(root).remove::<NeedsPortalGround>();
    }
}

/// Scintillement des flammes des yeux (intensité PointLight modulée, phase/œil).
pub(crate) fn sys_flicker_portal_flames(
    time: Res<Time>,
    mut q: Query<(&PortalFlame, &mut PointLight)>,
) {
    let t = time.elapsed_secs();
    for (flame, mut light) in &mut q {
        let k = 0.65 + 0.35 * (t * 11.0 + flame.phase).sin();
        light.intensity = flame.base_intensity * k;
    }
}

// ── Sensor (forgia2_boss_gate.json — observabilité de la pose de la porte) ──────

/// Pur (testable headless) : severity + next_step selon l'étape et le temps bloqué.
/// `stuck_secs` = temps écoulé avec le dais trouvé mais la porte pas encore posée.
pub(crate) fn severity_for_boss_gate(stage: GateStage, stuck_secs: f32) -> (&'static str, String) {
    match stage {
        GateStage::Placed => ("ok", "Porte fermée posée sur le dais.".to_string()),
        GateStage::Open => ("ok", "Porte ouverte — parcours débloqué.".to_string()),
        GateStage::NoDais => (
            "info",
            "Pas de dais melee_pit (hors run, ou stage sans fosse à mêlée).".to_string(),
        ),
        GateStage::Solidifying | GateStage::Raycasting if stuck_secs > GATE_STUCK_WARN_SECS => (
            "warn",
            format!(
                "Dais trouvé mais porte non posée depuis {stuck_secs:.0}s (étape {}) — raycast/collider du dais authored échoue ; le fallback AABB doit prendre le relais.",
                stage.label()
            ),
        ),
        _ => ("ok", "Pose de la porte en cours.".to_string()),
    }
}

/// Sensor 1Hz `forgia2_boss_gate.json` — rend la disparition intermittente de la
/// porte diagnosticable (étape de la cascade, tentatives de raycast, temps bloqué).
pub(crate) fn sys_write_boss_gate_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut stuck_secs: Local<f32>,
    gate: Res<BossGate>,
) {
    // Compteur de blocage temps-réel (chaque frame) : dais trouvé mais porte pas
    // encore posée. Reset dès que posée / ouverte / pas de dais.
    let waiting = matches!(
        gate.diag_stage,
        GateStage::Solidifying | GateStage::Raycasting
    );
    if waiting {
        *stuck_secs += time.delta_secs();
    } else {
        *stuck_secs = 0.0;
    }

    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (severity, next_step) = severity_for_boss_gate(gate.diag_stage, *stuck_secs);
    let placed = gate.dais_top_y.is_some();
    let top = gate.dais_top_y.unwrap_or(0.0);
    let json = format!(
        r#"{{"id":"boss_gate","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"stage":"{}","placed":{placed},"opened":{},"dais_top_y":{top:.2},"raycast_attempts":{},"stuck_secs":{:.1}}}"#,
        time.elapsed_secs(),
        gate.diag_stage.label(),
        gate.opened,
        gate.raycast_attempts,
        *stuck_secs,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_boss_gate.json", json) {
        warn!("[boss-gate] sensor write failed: {e}");
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct BossPortalPlugin;

impl Plugin for BossPortalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossGate>();
        app.add_systems(
            Update,
            (
                sys_reconcile_boss_gate,
                sys_calibrate_portal,
                sys_ground_portal,
            )
                .in_set(GameSet::Movement)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_flicker_portal_flames
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        // Observabilité (observability-required) : sensor 1Hz de la pose de la porte.
        app.add_systems(
            Update,
            sys_write_boss_gate_sensor.run_if(in_state(GameMode::Roguelite)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_default_is_no_dais() {
        assert_eq!(GateStage::default(), GateStage::NoDais);
        assert_eq!(GateStage::default().label(), "no_dais");
    }

    #[test]
    fn severity_ok_when_placed_or_open() {
        assert_eq!(severity_for_boss_gate(GateStage::Placed, 0.0).0, "ok");
        assert_eq!(severity_for_boss_gate(GateStage::Open, 999.0).0, "ok");
    }

    #[test]
    fn severity_info_when_no_dais_even_long() {
        // Hors run : pas de dais ≠ erreur, même après longtemps.
        assert_eq!(severity_for_boss_gate(GateStage::NoDais, 9999.0).0, "info");
    }

    #[test]
    fn severity_warn_when_stuck_past_threshold() {
        let (sev, next) = severity_for_boss_gate(GateStage::Raycasting, GATE_STUCK_WARN_SECS + 0.1);
        assert_eq!(sev, "warn");
        assert!(next.contains("raycast"));
    }

    #[test]
    fn severity_ok_while_waiting_under_threshold() {
        // En cours de pose (sous le seuil) = normal, pas de warn.
        assert_eq!(
            severity_for_boss_gate(GateStage::Solidifying, GATE_STUCK_WARN_SECS - 0.1).0,
            "ok"
        );
    }
}

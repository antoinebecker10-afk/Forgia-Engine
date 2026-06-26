//! arms.rs — Mains + avant-bras FPS **procéduraux**, placement **auto par-arme**
//! (story-617 inc.2 ; story-618 live ; story-619 per-weapon).
//!
//! Chaque bras = un poing (dos de main arrondi + 4 doigts repliés + pouce) au bout
//! d'un avant-bras peau à **manche relevée** (cuff sombre), enfant de la FpsCamera
//! (rendu par la caméra viewmodel à FOV séparé). Reçoit l'offset sway/bob partagé.
//!
//! **Placement auto par-arme** : les poignets sont dérivés de la position + taille
//! RÉELLES de l'arme équipée (genome : `offset_*` + `target_size`) → main droite sur
//! la crosse (arrière), main gauche sous le canon (avant), pour N'IMPORTE quelle arme.
//! Le tuning `[viewmodel_arms]` ne donne que des **fractions/décalages** (agnostiques
//! à l'arme), hot-reload.
//!
//! Plafond assumé : procédural = stylisé. Réalisme poussé = mesh de mains riggé (asset).

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::{ArmCosmetics, ArmStyle, GameMode};
use forgia_genome_core::Genome;
use forgia_player::prelude::FpsCamera;

use crate::calibration::{viewmodel_target_size, viewmodel_transform};
use crate::genome::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle};
use crate::pose::{apply_viewmodel_sway_bob, AdsState, ViewmodelMotionOffset};

/// Marker sur le root des bras (enfant de la FpsCamera).
#[derive(Component)]
pub struct ViewmodelArms;

/// Handles des matériaux peau+manche des bras (stockés sur le root) → permet de
/// ré-appliquer la cosmétique (couleur/style choisie dans l'onglet Forge) en mutant
/// l'asset partagé, sans re-spawn. Lu par `sync_arm_cosmetics`.
#[derive(Component)]
pub struct ArmMaterialHandles {
    pub skin: Handle<StandardMaterial>,
    pub cuff: Handle<StandardMaterial>,
}

/// Applique couleur + style à un matériau de bras (peau ou manche `is_cuff`).
/// Le style varie metallic/roughness/emissive ; la couleur teinte la base.
fn apply_arm_style(mat: &mut StandardMaterial, color: [f32; 3], style: ArmStyle, is_cuff: bool) {
    let [r, g, b] = color;
    // Manche = variante assombrie de la teinte (cohérence peau/manche).
    let (r, g, b) = if is_cuff {
        (r * 0.45, g * 0.47, b * 0.52)
    } else {
        (r, g, b)
    };
    match style {
        ArmStyle::Peau => {
            mat.base_color = Color::srgb(r, g, b);
            mat.metallic = 0.0;
            mat.perceptual_roughness = if is_cuff { 0.9 } else { 0.72 };
            mat.reflectance = 0.35;
            mat.emissive = if is_cuff {
                LinearRgba::rgb(0.03, 0.035, 0.05)
            } else {
                LinearRgba::rgb(0.10, 0.06, 0.04)
            };
        }
        ArmStyle::Gantelet => {
            mat.base_color = Color::srgb(r * 0.8, g * 0.8, b * 0.82);
            mat.metallic = 0.85;
            mat.perceptual_roughness = 0.38;
            mat.reflectance = 0.6;
            mat.emissive = LinearRgba::rgb(0.02, 0.02, 0.03);
        }
        ArmStyle::Cyber => {
            mat.base_color = Color::srgb(r * 0.25, g * 0.25, b * 0.30);
            mat.metallic = 0.4;
            mat.perceptual_roughness = 0.3;
            mat.reflectance = 0.5;
            // Glow de la teinte choisie (gant cyber lumineux).
            mat.emissive = LinearRgba::rgb(r * 1.4, g * 1.4, b * 1.4);
        }
    }
}

/// Ré-applique la cosmétique aux matériaux des bras quand `ArmCosmetics` change
/// (choix dans l'onglet Forge). Mute les assets partagés → paume/doigts/manche
/// suivent. Event-driven (`is_changed`).
pub fn sync_arm_cosmetics(
    cosmetics: Res<ArmCosmetics>,
    q_handles: Query<&ArmMaterialHandles>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !cosmetics.is_changed() {
        return;
    }
    for h in &q_handles {
        if let Some(m) = materials.get_mut(&h.skin) {
            apply_arm_style(m, cosmetics.color, cosmetics.style, false);
        }
        if let Some(m) = materials.get_mut(&h.cuff) {
            apply_arm_style(m, cosmetics.color, cosmetics.style, true);
        }
    }
}

/// Une main. `mirror` = +1 (droite, crosse) / -1 (gauche, sous le canon).
#[derive(Component, Clone, Copy)]
pub struct ViewmodelHand {
    pub mirror: f32,
}

/// Placement des bras — **fractions/décalages agnostiques à l'arme** (hot-reload
/// `fps_tuning.toml [viewmodel_arms]`). La position absolue est dérivée par-arme.
#[derive(Resource, Debug, Clone, Copy)]
pub struct ViewmodelArmsTuning {
    pub enabled: bool,
    pub scale: f32,
    /// Main crosse (droite) : décalage latéral, vertical, et recul (fraction de la
    /// longueur de l'arme, vers l'arrière/caméra).
    pub grip_x: f32,
    pub grip_drop: f32,
    pub grip_back: f32,
    /// Main soutien (gauche) : latéral, vertical, avancée (fraction longueur, vers le canon).
    pub barrel_x: f32,
    pub barrel_drop: f32,
    pub barrel_fwd: f32,
    /// Orientation des avant-bras (coude relatif au poignet).
    pub elbow_drop: f32,
    pub elbow_back: f32,
    /// Écartement latéral du coude — séparé par main (droite crosse / gauche soutien)
    /// pour un contrôle CoD-style : avant-bras gauche bien sorti vers le bas-gauche.
    pub grip_elbow_out: f32,
    pub barrel_elbow_out: f32,
}

impl Default for ViewmodelArmsTuning {
    fn default() -> Self {
        Self {
            enabled: true,
            scale: 2.0,
            grip_x: 0.0,
            grip_drop: -0.08,
            grip_back: 0.30,
            barrel_x: -0.04,
            barrel_drop: -0.06,
            barrel_fwd: 0.30,
            elbow_drop: 0.30,
            elbow_back: 0.45,
            grip_elbow_out: 0.12,
            barrel_elbow_out: 0.34,
        }
    }
}

// Couleurs/matériaux des bras = `apply_arm_style` (couleur + style cosmétiques choisis
// dans l'onglet Forge). Le viewmodel n'est PAS toon-shadé (caméra séparée) et n'est
// éclairé que par la scène → emissive léger gardé pour rester lisible au crépuscule.

// ── Proportions du poing (mètres, main-local : +Y = vers les doigts) ──
const FOREARM_RADIUS: f32 = 0.036;
// Allongé (0.16→0.26) : avant-bras visible qui remonte du coin bas vers l'arme (style CoD).
const FOREARM_LEN: f32 = 0.26;
// Profil de longueur des 4 doigts (index→auriculaire) : majeur le plus long.
const FINGER_PROFILE: [f32; 4] = [0.88, 1.0, 0.95, 0.78];

/// Hash → [0,1] déterministe (pour la texture procédurale).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(0x27d4_eb2d).wrapping_add(0x9e37_79b9);
    x ^= x >> 16;
    x = x.wrapping_mul(0x21f0_aaad);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32
}

/// Value-noise lissé (bilinéaire + smoothstep) sur une grille `cells×cells`.
fn smooth_noise(px: u32, py: u32, size: f32, cells: f32) -> f32 {
    let fx = px as f32 / size * cells;
    let fy = py as f32 / size * cells;
    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let corner = |cx: u32, cy: u32| {
        hash01(cx.wrapping_mul(374_761_393).wrapping_add(cy.wrapping_mul(668_265_263)))
    };
    let (v00, v10) = (corner(x0, y0), corner(x0 + 1, y0));
    let (v01, v11) = (corner(x0, y0 + 1), corner(x0 + 1, y0 + 1));
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/// Texture de détail peau procédurale (multipliée par la base_color) : casse
/// l'aspect plastique uni avec une variation organique — taches basse-fréquence
/// (sang/teint) + grain fin. Générée une fois au spawn (pas de hot path).
fn skin_detail_texture() -> Image {
    const N: u32 = 128;
    let mut data = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let blotch = smooth_noise(x, y, N as f32, 5.0); // grandes taches
            let grain = hash01(x.wrapping_mul(73).wrapping_add(y.wrapping_mul(151)));
            // Variation multiplicative douce, légèrement chaude.
            let v = ((0.86 + 0.14 * blotch) * (0.97 + 0.03 * grain)).clamp(0.0, 1.0);
            let idx = ((y * N + x) * 4) as usize;
            data[idx] = (v * 255.0) as u8;
            data[idx + 1] = (v * 0.985 * 255.0) as u8;
            data[idx + 2] = (v * 0.955 * 255.0) as u8;
            data[idx + 3] = 255;
        }
    }
    Image::new(
        Extent3d {
            width: N,
            height: N,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Doigt à 2 phalanges (courbure progressive) enfant de `hand`. `base` = jointure
/// (hand-local), `base_rot` = orientation de base (utile pour le pouce en travers),
/// `curl1/curl2` = flexion proximale puis distale → bout du doigt qui s'enroule.
#[allow(clippy::too_many_arguments)]
fn spawn_finger(
    commands: &mut Commands,
    hand: Entity,
    meshes: &mut Assets<Mesh>,
    skin: &Handle<StandardMaterial>,
    base: Vec3,
    base_rot: Quat,
    width: f32,
    l1: f32,
    l2: f32,
    curl1: f32,
    curl2: f32,
) {
    let root = commands
        .spawn((
            Transform::from_translation(base).with_rotation(base_rot * Quat::from_rotation_x(curl1)),
            Visibility::Inherited,
            Name::new("Finger"),
        ))
        .id();
    let prox = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(width, l1, width * 0.95))),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(0.0, l1 * 0.5, 0.0),
        ))
        .id();
    // Pivot distal au bout de la phalange proximale, flexion additionnelle.
    let pivot = commands
        .spawn((
            Transform::from_xyz(0.0, l1, 0.0).with_rotation(Quat::from_rotation_x(curl2)),
            Visibility::Inherited,
        ))
        .id();
    let dist = commands
        .spawn((
            Mesh3d(meshes.add(Cuboid::new(width * 0.9, l2, width * 0.85))),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(0.0, l2 * 0.5, 0.0),
        ))
        .id();
    commands.entity(pivot).add_child(dist);
    commands.entity(root).add_children(&[prox, pivot]);
    commands.entity(hand).add_child(root);
}

fn spawn_hand(
    commands: &mut Commands,
    root: Entity,
    meshes: &mut Assets<Mesh>,
    skin: &Handle<StandardMaterial>,
    cuff: &Handle<StandardMaterial>,
    mirror: f32,
) {
    let hand = commands
        .spawn((
            ViewmodelHand { mirror },
            Transform::IDENTITY,
            Visibility::Inherited,
            Name::new("ViewmodelHand"),
        ))
        .id();
    commands.entity(root).add_child(hand);

    // Pièces simples (avant-bras, manche, poignet, paume, dos de main).
    let kids = [
        commands
            .spawn((
                Mesh3d(meshes.add(Capsule3d::new(FOREARM_RADIUS, FOREARM_LEN))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, -FOREARM_LEN * 0.55, 0.0),
                Name::new("Forearm"),
            ))
            .id(),
        commands
            .spawn((
                Mesh3d(meshes.add(Cylinder::new(FOREARM_RADIUS * 1.25, 0.035))),
                MeshMaterial3d(cuff.clone()),
                Transform::from_xyz(0.0, -FOREARM_LEN * 0.92, 0.0),
                Name::new("Cuff"),
            ))
            .id(),
        // Poignet : raccord doux avant-bras → main.
        commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.034))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::new(1.0, 0.8, 0.85)),
                Name::new("Wrist"),
            ))
            .id(),
        // Paume (corps de la main).
        commands
            .spawn((
                Mesh3d(meshes.add(Cuboid::new(0.070, 0.058, 0.030))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 0.028, 0.004),
                Name::new("Palm"),
            ))
            .id(),
        // Dos de la main : galbe arrondi (côté -Z).
        commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.038))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, 0.030, -0.008).with_scale(Vec3::new(1.0, 0.6, 1.0)),
                Name::new("HandBack"),
            ))
            .id(),
    ];
    commands.entity(hand).add_children(&kids);

    // 4 doigts à 2 phalanges, longueurs variées (FINGER_PROFILE), enroulés (grip).
    let knuckle_y = 0.056;
    for (i, &p) in FINGER_PROFILE.iter().enumerate() {
        let x = (i as f32 - 1.5) * 0.0175;
        spawn_finger(
            commands,
            hand,
            meshes,
            skin,
            Vec3::new(x, knuckle_y, 0.010),
            Quat::IDENTITY,
            0.013,
            0.026 * p,
            0.022 * p,
            -0.55,
            -1.05,
        );
        // Bosse de jointure (relief du poing).
        let bump = commands
            .spawn((
                Mesh3d(meshes.add(Sphere::new(0.0105))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(x, knuckle_y, 0.004),
                Name::new("Knuckle"),
            ))
            .id();
        commands.entity(hand).add_child(bump);
    }

    // Pouce : 2 segments, orienté en travers (rotation Z miroir), peu enroulé.
    spawn_finger(
        commands,
        hand,
        meshes,
        skin,
        Vec3::new(mirror * 0.040, 0.020, 0.016),
        Quat::from_rotation_z(mirror * 0.95),
        0.016,
        0.024,
        0.020,
        -0.35,
        -0.70,
    );
}

/// Spawn root + 2 mains une fois la FpsCamera présente. Idempotent.
pub fn spawn_arms(
    mut commands: Commands,
    tuning: Res<ViewmodelArmsTuning>,
    cosmetics: Res<ArmCosmetics>,
    q_cam: Query<Entity, With<FpsCamera>>,
    q_arms: Query<(), With<ViewmodelArms>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !tuning.enabled || !q_arms.is_empty() {
        return;
    }
    let Ok(cam) = q_cam.single() else {
        return;
    };
    // Texture de détail procédurale (variation organique) — partagée skin + cuff.
    let detail = images.add(skin_detail_texture());
    // Couleur + style initiaux = cosmétique courante (choix Forge, persistée).
    // Les changements live sont appliqués par `sync_arm_cosmetics`.
    let mut skin_mat = StandardMaterial {
        base_color_texture: Some(detail.clone()),
        ..default()
    };
    apply_arm_style(&mut skin_mat, cosmetics.color, cosmetics.style, false);
    let skin = materials.add(skin_mat);
    let mut cuff_mat = StandardMaterial {
        base_color_texture: Some(detail),
        ..default()
    };
    apply_arm_style(&mut cuff_mat, cosmetics.color, cosmetics.style, true);
    let cuff = materials.add(cuff_mat);
    let root = commands
        .spawn((
            ViewmodelArms,
            ArmMaterialHandles {
                skin: skin.clone(),
                cuff: cuff.clone(),
            },
            Transform::IDENTITY,
            Visibility::Inherited,
            Name::new("ViewmodelArms"),
        ))
        .id();
    commands.entity(cam).add_child(root);
    spawn_hand(&mut commands, root, &mut meshes, &skin, &cuff, 1.0);
    spawn_hand(&mut commands, root, &mut meshes, &skin, &cuff, -1.0);
    info!("[forgia-viewmodel] mains procédurales spawnées (placement auto par-arme)");
}

/// Positionne chaque main depuis la position + taille RÉELLES de l'arme équipée
/// (genome) → s'adapte à chaque arme. Live (chaque frame) : hot-reload instantané.
pub fn position_hands(
    tuning: Res<ViewmodelArmsTuning>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<(&mut Transform, &ViewmodelHand)>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    // Centre + longueur de l'arme en camera-local (mêmes helpers que l'attach).
    let gun = viewmodel_transform(equipped.current, entry).translation;
    let len = viewmodel_target_size(equipped.current, entry);

    for (mut tf, hand) in &mut q {
        let wrist = if hand.mirror > 0.0 {
            // Main crosse : arrière (+Z vers caméra), sous l'arme.
            gun + Vec3::new(tuning.grip_x, tuning.grip_drop, tuning.grip_back * len)
        } else {
            // Main soutien : avant (-Z vers le canon), sous l'arme.
            gun + Vec3::new(tuning.barrel_x, tuning.barrel_drop, -tuning.barrel_fwd * len)
        };
        let elbow_out = if hand.mirror > 0.0 {
            tuning.grip_elbow_out
        } else {
            tuning.barrel_elbow_out
        };
        let elbow = wrist
            + Vec3::new(
                hand.mirror * elbow_out,
                -tuning.elbow_drop,
                tuning.elbow_back,
            );
        let fwd = (wrist - elbow).normalize_or_zero();
        let rot = Quat::from_rotation_arc(Vec3::Y, fwd);
        *tf = Transform::from_translation(wrist)
            .with_rotation(rot)
            .with_scale(Vec3::splat(tuning.scale.max(0.01)));
    }
}

/// Applique l'offset sway/bob partagé au root (les mains héritent). Root scale 1.0.
pub fn apply_arms_motion(
    offset: Res<ViewmodelMotionOffset>,
    mut q: Query<&mut Transform, With<ViewmodelArms>>,
) {
    for mut tf in &mut q {
        tf.translation = offset.translation;
        tf.rotation = offset.rotation;
    }
}

/// Cache les bras en visée sniper plein écran (même condition que le masquage de
/// l'arme dans `apply_ads_viewmodel`) → sinon les mains flottent devant le scope.
/// Visibility::Hidden sur le root se propage aux enfants (poings).
pub fn update_arms_visibility(
    ads: Res<AdsState>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<&mut Visibility, With<ViewmodelArms>>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let hide = entry
        .map(|e| e.sniper_scope_fullscreen && ads.progress > 0.5)
        .unwrap_or(false);
    let target = if hide {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut vis in &mut q {
        if *vis != target {
            *vis = target;
        }
    }
}

/// Plugin bras : spawn + placement auto par-arme + motion. Gated FPS + Roguelite.
pub struct ForgiaViewmodelArmsPlugin;

impl Plugin for ForgiaViewmodelArmsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewmodelArmsTuning>().add_systems(
            Update,
            (
                spawn_arms,
                position_hands,
                apply_arms_motion.after(apply_viewmodel_sway_bob),
                update_arms_visibility,
                sync_arm_cosmetics,
            )
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}

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
use bevy::scene::SceneInstanceReady;
use forgia_assets::GameAssets;
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::{ArmCosmetics, ArmStyle, GameMode};
use forgia_genome_core::Genome;
use forgia_player::prelude::FpsCamera;

use crate::attach::{NeedsAutoScale, ViewmodelBaseScale, WeaponViewmodel};
use crate::calibration::{viewmodel_target_size, viewmodel_transform};
use crate::genome::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle};
use crate::pose::{apply_ads_viewmodel, apply_viewmodel_sway_bob, AdsState, ViewmodelMotionOffset};

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

/// Variante GLB de `apply_arm_style` : la couleur devient une TEINTE qui
/// préserve la luminosité (normalisée par son canal max), multipliée par la
/// texture aplats du GLB — un multiply par la couleur brute assombrirait tout.
/// Mêmes familles de réglages matériau que `apply_arm_style` (cosmétique).
fn apply_arm_style_glb(mat: &mut StandardMaterial, color: [f32; 3], style: ArmStyle) {
    let [r, g, b] = color;
    let max = r.max(g).max(b).max(1e-3);
    let (tr, tg, tb) = (r / max, g / max, b / max);
    match style {
        ArmStyle::Peau => {
            mat.base_color = Color::srgb(tr, tg, tb);
            mat.metallic = 0.0;
            mat.perceptual_roughness = 0.9;
            mat.reflectance = 0.35;
            mat.emissive = LinearRgba::BLACK;
        }
        ArmStyle::Gantelet => {
            mat.base_color = Color::srgb(tr * 0.85, tg * 0.85, tb * 0.9);
            mat.metallic = 0.85;
            mat.perceptual_roughness = 0.38;
            mat.reflectance = 0.6;
            mat.emissive = LinearRgba::BLACK;
        }
        ArmStyle::Cyber => {
            mat.base_color = Color::srgb(tr * 0.35, tg * 0.35, tb * 0.4);
            mat.metallic = 0.4;
            mat.perceptual_roughness = 0.3;
            mat.reflectance = 0.5;
            // Glow de la teinte choisie (gant cyber lumineux).
            mat.emissive = LinearRgba::rgb(r * 1.4, g * 1.4, b * 1.4);
        }
    }
}

/// Handle du matériau CLONÉ d'une main GLB → teinte/style live via
/// `sync_arm_cosmetics` (onglet Forge, comme les bras procéduraux).
#[derive(Component)]
pub struct ArmGlbMaterial(pub Handle<StandardMaterial>);

/// Observer posé sur chaque main GLB au spawn : au scene-ready, clone le
/// matériau du GLB (l'asset est PARTAGÉ entre les 2 mains — muter l'original
/// teinterait tout usage du GLB) puis applique la cosmétique courante.
/// Event-driven : zéro coût par-frame (pas de scan global des matériaux).
/// Distingue les plaques d'armure de la combinaison, **par la texture** et non
/// par l'ordre des matériaux.
///
/// L'ordre des primitives d'un glTF n'est garanti par rien : s'y fier ferait
/// intervertir les deux teintes au prochain ré-export, en silence. Le nom du
/// fichier d'albédo, lui, porte le sens.
fn is_armor_material(m: &StandardMaterial) -> bool {
    m.base_color_texture
        .as_ref()
        .and_then(|h| h.path())
        .map(|p| p.path().to_string_lossy().contains("_armor_"))
        .unwrap_or(false)
}

pub fn on_arm_scene_ready(
    event: On<SceneInstanceReady>,
    children: Query<&Children>,
    mat_q: Query<&MeshMaterial3d<StandardMaterial>>,
    cosmetics: Res<ArmCosmetics>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let root = event.entity;
    for desc in children.iter_descendants(root) {
        let Ok(mat_handle) = mat_q.get(desc) else {
            continue;
        };
        let Some(mut m) = materials.get(&mat_handle.0).cloned() else {
            continue; // matériau pas encore chargé — laisser l'original
        };
        // Deux couches : les PLAQUES prennent la rareté, la combinaison prend la
        // couleur d'identité. Le bras du Trooper porte les deux matériaux.
        if is_armor_material(&m) {
            m.base_color = Color::srgb(
                cosmetics.armor_rgb[0],
                cosmetics.armor_rgb[1],
                cosmetics.armor_rgb[2],
            );
        } else {
            apply_arm_style_glb(&mut m, cosmetics.color, cosmetics.style);
        }
        let h = materials.add(m);
        commands.entity(desc).insert(MeshMaterial3d(h.clone()));
        commands.entity(desc).insert(ArmGlbMaterial(h));
    }
}

/// Ré-applique la cosmétique aux matériaux des bras quand `ArmCosmetics` change
/// (choix dans l'onglet Forge). Mute les assets (procéduraux partagés / clones
/// GLB par-main) → paume/doigts/manche suivent. Event-driven (`is_changed`).
pub fn sync_arm_cosmetics(
    cosmetics: Res<ArmCosmetics>,
    q_handles: Query<&ArmMaterialHandles>,
    q_glb: Query<&ArmGlbMaterial>,
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
    for h in &q_glb {
        if let Some(m) = materials.get_mut(&h.0) {
            // Même partage qu'au scene-ready : plaques ← rareté, combinaison ←
            // couleur d'identité. Les deux doivent suivre un changement live.
            if is_armor_material(m) {
                m.base_color = Color::srgb(
                    cosmetics.armor_rgb[0],
                    cosmetics.armor_rgb[1],
                    cosmetics.armor_rgb[2],
                );
            } else {
                apply_arm_style_glb(m, cosmetics.color, cosmetics.style);
            }
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
    /// Bras GLB cartoon (story-661) au lieu des poings procéduraux.
    /// Hot-reload : le toggle despawn/respawn les bras dans le bon mode.
    pub use_glb: bool,
    /// Échelle des bras GLB (mesh baked en mètres réels ; 1.0 = taille physique).
    pub glb_scale: f32,
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
            use_glb: true,
            glb_scale: 1.0,
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

// ── Bras GLB cartoon (story-661, pipeline `tools/blender/cartoonize_arms.py`) ──
// Convention baked identique aux poings procéduraux : poignet à l'origine,
// avant-bras vers -Y, doigts +Y, paume +Z, pouce ±X → `position_hands` inchangé.
// Chemins littéraux : même précédent que les armes (attach.rs). `pub(crate)` :
// lus aussi par le capteur `arms_sensor` (état de chargement).
pub(crate) const ARM_GLB_RIGHT: &str = "models/arms/fps_arm_R.glb#Scene0";
pub(crate) const ARM_GLB_LEFT: &str = "models/arms/fps_arm_L.glb#Scene0";

/// Mode des bras actuellement spawnés (true = GLB) — permet le respawn quand
/// `use_glb` change à chaud (fps_tuning hot-reload).
#[derive(Component)]
pub struct ArmsGlbMode(pub bool);

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
        hash01(
            cx.wrapping_mul(374_761_393)
                .wrapping_add(cy.wrapping_mul(668_265_263)),
        )
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
                                                            // 2e octave (fréquence ~2.5×, coordonnées décalées pour décorréler) :
                                                            // marbrures moyennes entre les grandes taches et le grain.
            let blotch2 = smooth_noise(x.wrapping_add(53), y.wrapping_add(97), N as f32, 13.0);
            let grain = hash01(x.wrapping_mul(73).wrapping_add(y.wrapping_mul(151)));
            // Variation multiplicative douce, légèrement chaude (même moyenne qu'avant).
            let v =
                ((0.85 + 0.10 * blotch + 0.05 * blotch2) * (0.97 + 0.03 * grain)).clamp(0.0, 1.0);
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
            Transform::from_translation(base)
                .with_rotation(base_rot * Quat::from_rotation_x(curl1)),
            Visibility::Inherited,
            Name::new("Finger"),
        ))
        .id();
    // Phalanges en capsules (bouts arrondis, section circulaire) — casse l'aspect
    // « briques » des cuboids. Longueur totale capsule = cylindre + 2×rayon = l.
    let prox_r = width * 0.5;
    let prox = commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(prox_r, (l1 - 2.0 * prox_r).max(l1 * 0.3)))),
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
    let dist_r = width * 0.45;
    let dist = commands
        .spawn((
            Mesh3d(meshes.add(Capsule3d::new(dist_r, (l2 - 2.0 * dist_r).max(l2 * 0.3)))),
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
        // Section elliptique (x>z) : un avant-bras réel n'est pas un tube rond.
        commands
            .spawn((
                Mesh3d(meshes.add(Capsule3d::new(FOREARM_RADIUS, FOREARM_LEN))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, -FOREARM_LEN * 0.55, 0.0)
                    .with_scale(Vec3::new(1.10, 1.0, 0.92)),
                Name::new("Forearm"),
            ))
            .id(),
        // Galbe du muscle (côté coude) : léger renflement, reste sous le rayon de
        // la manche (1.25×) pour ne pas la percer.
        commands
            .spawn((
                Mesh3d(meshes.add(Capsule3d::new(FOREARM_RADIUS * 1.12, FOREARM_LEN * 0.20))),
                MeshMaterial3d(skin.clone()),
                Transform::from_xyz(0.0, -FOREARM_LEN * 0.60, 0.0)
                    .with_scale(Vec3::new(1.10, 1.0, 0.92)),
                Name::new("ForearmBulge"),
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
        // Éventail léger (~±4.5°) : les doigts extérieurs s'écartent du majeur →
        // main organique au lieu de 4 doigts parallèles.
        let splay = Quat::from_rotation_z((1.5 - i as f32) * 0.052);
        spawn_finger(
            commands,
            hand,
            meshes,
            skin,
            Vec3::new(x, knuckle_y, 0.010),
            splay,
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

    // Éminence thénar : galbe charnu à la base du pouce (raccord paume→pouce).
    let thenar = commands
        .spawn((
            Mesh3d(meshes.add(Sphere::new(0.016))),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(mirror * 0.030, 0.012, 0.010)
                .with_scale(Vec3::new(1.0, 1.25, 0.85)),
            Name::new("Thenar"),
        ))
        .id();
    commands.entity(hand).add_child(thenar);
}

/// Spawn root + 2 mains une fois la FpsCamera présente. Idempotent.
/// `use_glb` (hot-reload) choisit bras GLB cartoon (story-661) vs poings
/// procéduraux ; un toggle à chaud despawn puis respawn dans le bon mode.
pub fn spawn_arms(
    mut commands: Commands,
    tuning: Res<ViewmodelArmsTuning>,
    cosmetics: Res<ArmCosmetics>,
    game_assets: Res<GameAssets>,
    q_cam: Query<Entity, With<FpsCamera>>,
    q_arms: Query<(Entity, &ArmsGlbMode), With<ViewmodelArms>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !tuning.enabled {
        return;
    }
    if let Ok((root, mode)) = q_arms.single() {
        if mode.0 != tuning.use_glb {
            commands.entity(root).despawn();
        }
        return;
    }
    let Ok(cam) = q_cam.single() else {
        return;
    };

    // ── Mode GLB (story-661) : 1 scène par main, placée par `position_hands`
    // exactement comme les poings procéduraux (convention baked identique). ──
    if tuning.use_glb {
        let root = commands
            .spawn((
                ViewmodelArms,
                ArmsGlbMode(true),
                Transform::IDENTITY,
                Visibility::Inherited,
                Name::new("ViewmodelArms"),
            ))
            .id();
        commands.entity(cam).add_child(root);
        for (mirror, scene) in [
            (1.0, game_assets.viewmodel_arm_right.clone()),
            (-1.0, game_assets.viewmodel_arm_left.clone()),
        ] {
            let hand = commands
                .spawn((
                    ViewmodelHand { mirror },
                    SceneRoot(scene),
                    Transform::IDENTITY,
                    Visibility::Inherited,
                    Name::new("ViewmodelHandGlb"),
                ))
                // Cosmétiques Forge : clone du matériau + teinte au scene-ready.
                .observe(on_arm_scene_ready)
                .id();
            commands.entity(root).add_child(hand);
        }
        info!("[forgia-viewmodel] bras GLB cartoon spawnés (story-661)");
        return;
    }

    // ── Mode procédural (fallback A/B, `use_glb = false`) ──
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
            ArmsGlbMode(false),
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

/// Positionne chaque main depuis la pose RÉELLE de l'arme équipée → suit le lerp
/// hipfire→ADS (translation, dé-tilt, shrink `ads_scale_factor`) écrit par
/// `apply_ads_viewmodel`. Ordonné APRÈS la pose ADS et AVANT le sway (le sway est
/// répliqué sur le root des bras par `apply_arms_motion`, sinon il compterait double).
/// Fallback genome hipfire tant que l'arme n'existe pas / est en auto-scale (boot).
pub fn position_hands(
    tuning: Res<ViewmodelArmsTuning>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    q_weapon: Query<
        (&Transform, Option<&ViewmodelBaseScale>),
        (
            With<WeaponViewmodel>,
            Without<NeedsAutoScale>,
            Without<ViewmodelHand>,
        ),
    >,
    mut q: Query<(&mut Transform, &ViewmodelHand), Without<WeaponViewmodel>>,
) {
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    let hipfire = viewmodel_transform(equipped.current, entry);
    let base_len = viewmodel_target_size(equipped.current, entry);

    // Ancre = Transform courant de l'arme. `delta_rot` = rotation hipfire→pose courante
    // (≈ retrait du tilt en ADS) appliquée aux offsets camera-local → mains collées.
    // `len` suit le shrink ADS (scale courant / scale de base calibré).
    let (gun, delta_rot, len) = match q_weapon.single() {
        Ok((wtf, base_scale)) => {
            let scale_mul = base_scale
                .filter(|b| b.0 > 1e-6)
                .map(|b| wtf.scale.x / b.0)
                .unwrap_or(1.0);
            (
                wtf.translation,
                wtf.rotation * hipfire.rotation.inverse(),
                base_len * scale_mul,
            )
        }
        Err(_) => (hipfire.translation, Quat::IDENTITY, base_len),
    };

    // Shrink ADS appliqué aux ancres par-arme (absolues à l'hipfire).
    let ads_mul = if base_len > 1e-6 { len / base_len } else { 1.0 };
    for (mut tf, hand) in &mut q {
        // Ancre PAR-ARME (genome, calibrée aux tubes) si présente ; sinon
        // fallback fractions globales — no-hardcode : la prise dépend de la
        // géométrie de CHAQUE arme (retour user Madame Lenoir 2026-07-20).
        let offset = if hand.mirror > 0.0 {
            match entry.and_then(|e| e.grip_anchor) {
                Some(a) => Vec3::from(a) * ads_mul,
                // Main crosse : arrière (+Z vers caméra), sous l'arme.
                None => Vec3::new(tuning.grip_x, tuning.grip_drop, tuning.grip_back * len),
            }
        } else {
            match entry.and_then(|e| e.barrel_anchor) {
                Some(a) => Vec3::from(a) * ads_mul,
                // Main soutien : avant (-Z vers le canon), sous l'arme.
                None => Vec3::new(
                    tuning.barrel_x,
                    tuning.barrel_drop,
                    -tuning.barrel_fwd * len,
                ),
            }
        };
        let wrist = gun + delta_rot * offset;
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
        // Roulis par-arme (deg) autour de l'axe avant-bras (= Y local de la main) :
        // oriente la paume par-arme (ex. sniper paume sous le canon) sans re-baker.
        let roll_deg = entry
            .map(|e| {
                if hand.mirror > 0.0 {
                    e.grip_roll_deg
                } else {
                    e.barrel_roll_deg
                }
            })
            .unwrap_or(0.0);
        let rot =
            Quat::from_rotation_arc(Vec3::Y, fwd) * Quat::from_rotation_y(roll_deg.to_radians());
        // GLB = mètres réels (glb_scale ~1) ; procédural = proportions internes ×scale.
        let scale = if tuning.use_glb {
            tuning.glb_scale
        } else {
            tuning.scale
        };
        *tf = Transform::from_translation(wrist)
            .with_rotation(rot)
            .with_scale(Vec3::splat(scale.max(0.01)));
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
    // Une arme pixel art porte sa main DANS le sprite : laisser les bras 3D
    // par-dessus donnerait deux mains sur la même crosse.
    let hide = crate::sprite::weapon_is_sprite(entry)
        || entry
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

/// Masque la main soutien (gauche, `mirror < 0`) pour les armes une-main
/// (genome `hide_support_hand`, ex. pistolet). Data-driven par-arme, hot-reload.
/// Visibility PAR-MAIN : compose avec le masquage sniper du root (root Hidden
/// cache tout ; sinon chaque main applique sa propre visibilité). `Without<
/// ViewmodelArms>` = disjoint de `update_arms_visibility` (pas de conflit query).
pub fn update_support_hand_visibility(
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<(&ViewmodelHand, &mut Visibility), Without<ViewmodelArms>>,
) {
    let hide_support = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current))
        .map(|e| e.hide_support_hand)
        .unwrap_or(false);
    for (hand, mut vis) in &mut q {
        let target = if hand.mirror < 0.0 && hide_support {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
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
                // Après la pose ADS (ancre = arme réelle), avant le sway (répliqué
                // séparément sur le root par apply_arms_motion — sinon double).
                position_hands
                    .after(apply_ads_viewmodel)
                    .before(apply_viewmodel_sway_bob),
                apply_arms_motion.after(apply_viewmodel_sway_bob),
                update_arms_visibility,
                update_support_hand_visibility,
                sync_arm_cosmetics,
                // Story-661 — capteur forgia2_viewmodel_arms.json (1 Hz).
                crate::arms_sensor::write_arms_sensor,
            )
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}

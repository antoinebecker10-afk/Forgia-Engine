//! weapon_preview.rs — aperçus **3D live** du hub-menu (render-to-texture).
//!
//! Deux aperçus RTT indépendants, chacun sur son propre layer isolé :
//! - **Arme** (onglet Armes) : le GLB d'arme sélectionné, tourne.
//! - **Bras** (onglet Forgeron) : les 2 bras GLB du forgeron, teintés à la couleur/
//!   style choisis (`ArmCosmetics`), tournent.
//!
//! Au menu-titre il n'y a pas de scène 3D (Camera2d + fond vidéo egui) : une
//! Camera3d classique passerait derrière le fond opaque ou devant les panneaux.
//! Solution = **RTT** — une caméra dédiée rend le sujet dans une `Image` offscreen
//! (layer isolé), affichée comme image egui DANS le panneau, bien intégrée à la UI.
//!
//! Recette Bevy 0.18-exacte (bevy-specialist, miroir de l'exemple officiel
//! `bevy_egui/examples/render_to_image_widget.rs`) :
//! - `RenderTarget` = **Component séparé** (pas `Camera.target`), requis par `Camera`.
//! - `RenderLayers` ne se propage PAS aux enfants d'un `SceneRoot` GLB → propagation
//!   BFS manuelle (cf `forgia-viewmodel::propagate_viewmodel_layer`).
//! - clear **opaque** sombre ; lumière DÉDIÉE sur le layer.
//!
//! Cycle : spawn OnEnter(Menu), despawn OnExit(Menu). Swap arme sur
//! `StartingWeaponChoice`, teinte bras sur `ArmCosmetics`.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use forgia_assets::GameAssets;
use forgia_core::prelude::{AppMode, ArmCosmetics, ArmStyle};
use forgia_mode_roguelite::weapon_select::StartingWeaponChoice;

/// Layer de rendu de l'aperçu ARME (0 = monde, 1 = viewmodel FPS déjà pris).
const WEAPON_LAYER: usize = 2;
/// Layer de rendu de l'aperçu BRAS (isolé de l'arme → les 2 caméras ne se voient pas).
const ARM_LAYER: usize = 3;
/// Côté de l'image RTT (px). Carré → viewport carré dans le panneau.
const RTT_SIZE: u32 = 512;
/// Taille cible (plus grande dimension, m) du sujet après calibrage AABB.
const PREVIEW_TARGET: f32 = 1.15;
/// Vitesse de rotation turntable (rad/s).
const PREVIEW_SPIN: f32 = 0.7;

/// Les slots du choix d'arme et les handles préchargés ont le même ordre.
/// Le modulo conserve le comportement précédent si un choix persistant est
/// issu d'une ancienne version du jeu.
fn weapon_preview_scene(assets: &GameAssets, choice_idx: usize) -> Handle<Scene> {
    assets.weapon_preview_scenes[choice_idx % assets.weapon_preview_scenes.len()].clone()
}

/// Ressource de l'aperçu ARME : `TextureId` (affiché par `sys_menu_armes`) + entité
/// `SceneRoot` swappable à la sélection.
#[derive(Resource)]
pub struct WeaponPreviewRtt {
    pub tex_id: egui::TextureId,
    image: Handle<Image>,
    scene_entity: Entity,
    shown_idx: usize,
}

/// Ressource de l'aperçu BRAS : `TextureId` (affiché par `sys_menu_forgeron`) +
/// conteneur des 2 bras (calibré ensemble).
#[derive(Resource)]
pub struct ArmPreviewRtt {
    pub tex_id: egui::TextureId,
    image: Handle<Image>,
    arm_holder: Entity,
}

/// Marqueur des entités racines d'un aperçu (caméra / lumière / pivot) — despawn
/// récursif au départ du menu (les scènes cascadent via le pivot).
#[derive(Component)]
struct PreviewEntity;

/// Pivot rotatif (tourne autour de Y). Un par aperçu (arme + bras).
#[derive(Component)]
struct PreviewPivot;

/// Calibrage AABB en attente (recentrage + mise à l'échelle) — ré-armé au swap.
#[derive(Component)]
struct NeedsPreviewCalibrate;

/// Matériau CLONÉ d'un mesh de bras (l'asset GLB est partagé — muter l'original
/// teinterait tout usage) → teinte/style live via `sys_retint_arm_materials`.
#[derive(Component)]
struct ArmPreviewMat(Handle<StandardMaterial>);

/// Plugin : câble le cycle de vie des deux aperçus RTT sur `AppMode::Menu`.
pub struct WeaponPreviewPlugin;

impl Plugin for WeaponPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppMode::Menu),
            (sys_spawn_weapon_preview, sys_spawn_arm_preview),
        )
        .add_systems(OnExit(AppMode::Menu), sys_despawn_previews)
        .add_systems(
            Update,
            (
                sys_swap_weapon_preview,
                sys_clone_arm_materials,
                sys_retint_arm_materials,
                sys_propagate_preview_layers,
                sys_calibrate_previews,
                sys_rotate_previews,
            )
                .chain()
                .run_if(in_state(AppMode::Menu)),
        );
    }
}

// ── Création de l'image RTT (partagée arme/bras) ────────────────────────────────

/// Crée une image render-target carrée + l'enregistre auprès d'egui (une fois).
fn create_rtt_image(
    images: &mut Assets<Image>,
    contexts: &mut EguiContexts,
    label: &'static str,
) -> (Handle<Image>, egui::TextureId) {
    let size = Extent3d {
        width: RTT_SIZE,
        height: RTT_SIZE,
        depth_or_array_layers: 1,
    };
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: Some(label),
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    // OBLIGATOIRE : alloue le buffer data (sinon image vide → noir/panic wgpu).
    image.resize(size);
    let handle = images.add(image);
    let tex_id = contexts.add_image(EguiTextureHandle::Strong(handle.clone()));
    (handle, tex_id)
}

/// Spawn caméra RTT (order négatif, cible = image, clear opaque sombre) + lumière
/// dédiée, sur le layer donné. Les entités portent `PreviewEntity`.
fn spawn_rtt_camera_light(
    commands: &mut Commands,
    image: &Handle<Image>,
    layer: &RenderLayers,
    order: isize,
    name: &'static str,
) {
    commands.spawn((
        Camera3d::default(),
        Camera {
            order,
            clear_color: ClearColorConfig::Custom(Color::srgba(0.06, 0.05, 0.09, 1.0)),
            ..default()
        },
        RenderTarget::Image(image.clone().into()),
        Transform::from_xyz(0.0, 0.15, 1.7).looking_at(Vec3::ZERO, Vec3::Y),
        layer.clone(),
        PreviewEntity,
        Name::new(name),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 6000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.6, 0.4, 0.0)),
        layer.clone(),
        PreviewEntity,
    ));
}

// ── Aperçu ARME ────────────────────────────────────────────────────────────────

/// OnEnter(Menu) — crée l'aperçu 3D de l'arme sélectionnée (layer WEAPON_LAYER).
fn sys_spawn_weapon_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
    assets: Res<GameAssets>,
    choice: Res<StartingWeaponChoice>,
    existing: Option<Res<WeaponPreviewRtt>>,
) {
    if existing.is_some() {
        return;
    }
    let (image, tex_id) = create_rtt_image(&mut images, &mut contexts, "weapon_preview_rtt");
    let layer = RenderLayers::layer(WEAPON_LAYER);
    spawn_rtt_camera_light(&mut commands, &image, &layer, -1, "WeaponPreviewCamera");

    let pivot = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            PreviewPivot,
            PreviewEntity,
            Name::new("WeaponPreviewPivot"),
        ))
        .id();
    let scene = weapon_preview_scene(&assets, choice.idx);
    let scene_entity = commands
        .spawn((
            SceneRoot(scene),
            Transform::from_scale(Vec3::splat(0.001)),
            Visibility::Inherited,
            layer,
            NeedsPreviewCalibrate,
            Name::new("WeaponPreviewScene"),
            ChildOf(pivot),
        ))
        .id();

    commands.insert_resource(WeaponPreviewRtt {
        tex_id,
        image,
        scene_entity,
        shown_idx: choice.idx,
    });
    info!("[weapon-preview] aperçu 3D arme spawné (layer {WEAPON_LAYER})");
}

/// Swap l'arme montrée quand `StartingWeaponChoice` change (clic ‹ › du panneau).
fn sys_swap_weapon_preview(
    rtt: Option<ResMut<WeaponPreviewRtt>>,
    choice: Res<StartingWeaponChoice>,
    assets: Res<GameAssets>,
    mut q_scene: Query<&mut SceneRoot>,
    mut commands: Commands,
) {
    let Some(mut rtt) = rtt else {
        return;
    };
    if choice.idx == rtt.shown_idx {
        return;
    }
    rtt.shown_idx = choice.idx;
    let scene = weapon_preview_scene(&assets, choice.idx);
    if let Ok(mut sr) = q_scene.get_mut(rtt.scene_entity) {
        sr.0 = scene;
        commands.entity(rtt.scene_entity).insert((
            NeedsPreviewCalibrate,
            Transform::from_scale(Vec3::splat(0.001)),
        ));
    }
}

// ── Aperçu BRAS ──────────────────────────────────────────────────────────────

/// OnEnter(Menu) — crée l'aperçu 3D des 2 bras du forgeron (layer ARM_LAYER),
/// depuis les handles préchargés `GameAssets` (pas de `load()` ad-hoc / hardcode).
fn sys_spawn_arm_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
    assets: Res<GameAssets>,
    existing: Option<Res<ArmPreviewRtt>>,
) {
    if existing.is_some() {
        return;
    }
    let (image, tex_id) = create_rtt_image(&mut images, &mut contexts, "arm_preview_rtt");
    let layer = RenderLayers::layer(ARM_LAYER);
    spawn_rtt_camera_light(&mut commands, &image, &layer, -2, "ArmPreviewCamera");

    let pivot = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            PreviewPivot,
            PreviewEntity,
            Name::new("ArmPreviewPivot"),
        ))
        .id();
    // Conteneur des 2 bras : calibré ENSEMBLE (AABB combinée) → tourne autour du
    // centre du duo.
    let arm_holder = commands
        .spawn((
            Transform::from_scale(Vec3::splat(0.001)),
            Visibility::Inherited,
            layer.clone(),
            NeedsPreviewCalibrate,
            Name::new("ArmPreviewHolder"),
            ChildOf(pivot),
        ))
        .id();
    for (handle, name) in [
        (assets.viewmodel_arm_right.clone(), "ArmPreviewR"),
        (assets.viewmodel_arm_left.clone(), "ArmPreviewL"),
    ] {
        commands.spawn((
            SceneRoot(handle),
            Transform::default(),
            Visibility::Inherited,
            layer.clone(),
            Name::new(name),
            ChildOf(arm_holder),
        ));
    }

    commands.insert_resource(ArmPreviewRtt {
        tex_id,
        image,
        arm_holder,
    });
    info!("[weapon-preview] aperçu 3D bras spawné (layer {ARM_LAYER})");
}

/// Clone (une fois par mesh) le matériau des bras + le teinte à la cosmétique
/// courante. Cloner car l'asset GLB est partagé (miroir `arms::on_arm_scene_ready`).
/// Réplique locale forcée : `arms.rs` est verrouillé (édition parallèle, bras Lenoir).
fn sys_clone_arm_materials(
    rtt: Option<Res<ArmPreviewRtt>>,
    cosmetics: Res<ArmCosmetics>,
    q_children: Query<&Children>,
    q_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    q_cloned: Query<(), With<ArmPreviewMat>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let Some(rtt) = rtt else {
        return;
    };
    let mut stack = vec![rtt.arm_holder];
    while let Some(e) = stack.pop() {
        if q_cloned.get(e).is_err() {
            if let Ok(mm) = q_mat.get(e) {
                if let Some(src) = materials.get(&mm.0) {
                    let mut clone = src.clone();
                    apply_arm_style_glb(&mut clone, cosmetics.color, cosmetics.style);
                    let handle = materials.add(clone);
                    commands
                        .entity(e)
                        .insert((MeshMaterial3d(handle.clone()), ArmPreviewMat(handle)));
                }
            }
        }
        if let Ok(children) = q_children.get(e) {
            stack.extend(children.iter());
        }
    }
}

/// Re-teinte les matériaux clonés des bras quand `ArmCosmetics` change (choix Forge).
fn sys_retint_arm_materials(
    cosmetics: Res<ArmCosmetics>,
    q: Query<&ArmPreviewMat>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !cosmetics.is_changed() {
        return;
    }
    for m in &q {
        if let Some(mat) = materials.get_mut(&m.0) {
            apply_arm_style_glb(mat, cosmetics.color, cosmetics.style);
        }
    }
}

/// Teinte GLB : la couleur devient une TEINTE normalisée (préserve la luminosité)
/// + variations metallic/roughness/emissive par style. Réplique locale de
///
/// `forgia_viewmodel::arms::apply_arm_style_glb` (crate verrouillée, cf ci-dessus).
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
            mat.emissive = LinearRgba::rgb(r * 1.4, g * 1.4, b * 1.4);
        }
    }
}

// ── Systèmes génériques (arme + bras) ───────────────────────────────────────────

/// Propage le `RenderLayers` de chaque racine d'aperçu à TOUS ses descendants (un
/// `SceneRoot` GLB ne le fait pas en 0.18). Lit le layer porté par la racine.
fn sys_propagate_preview_layers(
    weapon: Option<Res<WeaponPreviewRtt>>,
    arm: Option<Res<ArmPreviewRtt>>,
    q_children: Query<&Children>,
    q_layers: Query<&RenderLayers>,
    mut commands: Commands,
) {
    let roots = [
        weapon.as_ref().map(|w| w.scene_entity),
        arm.as_ref().map(|a| a.arm_holder),
    ];
    for root in roots.into_iter().flatten() {
        let Ok(target) = q_layers.get(root).cloned() else {
            continue;
        };
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            if q_layers.get(e).map(|l| *l != target).unwrap_or(true) {
                commands.entity(e).insert(target.clone());
            }
            if let Ok(children) = q_children.get(e) {
                stack.extend(children.iter());
            }
        }
    }
}

/// Calibrage AABB : recentre (-centre) + met à l'échelle (`PREVIEW_TARGET`) toute
/// entité `NeedsPreviewCalibrate` une fois son AABB (descendants) disponible.
fn sys_calibrate_previews(
    q_need: Query<Entity, With<NeedsPreviewCalibrate>>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
    mut commands: Commands,
) {
    for e in &q_need {
        let Some((min, max)) = preview_aabb_bounds(e, &q_aabb, &q_children) else {
            continue;
        };
        let center = (min + max) * 0.5;
        let extent = (max - min).max_element().max(1e-3);
        let scale = PREVIEW_TARGET / extent;
        if let Ok(mut tf) = q_tf.get_mut(e) {
            // p → S*p + T ; on veut S*center + T = 0 → T = -center*scale.
            tf.translation = -center * scale;
            tf.scale = Vec3::splat(scale);
        }
        commands.entity(e).remove::<NeedsPreviewCalibrate>();
    }
}

/// Rotation turntable de tous les pivots d'aperçu (arme + bras).
fn sys_rotate_previews(time: Res<Time>, mut q: Query<&mut Transform, With<PreviewPivot>>) {
    let dr = PREVIEW_SPIN * time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_y(dr);
    }
}

/// OnExit(Menu) — désenregistre les images d'egui + despawn les racines (récursif
/// en 0.18 → les pivots cascadent les scènes + enfants).
fn sys_despawn_previews(
    mut commands: Commands,
    weapon: Option<Res<WeaponPreviewRtt>>,
    arm: Option<Res<ArmPreviewRtt>>,
    q: Query<Entity, With<PreviewEntity>>,
    mut contexts: EguiContexts,
) {
    if let Some(w) = weapon.as_ref() {
        contexts.remove_image(w.image.id());
    }
    if let Some(a) = arm.as_ref() {
        contexts.remove_image(a.image.id());
    }
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<WeaponPreviewRtt>();
    commands.remove_resource::<ArmPreviewRtt>();
}

/// Walk les descendants → `(min, max)` de l'AABB combinée (espace local du root).
fn preview_aabb_bounds(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    let mut found = false;
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(a) = q_aabb.get(e) {
            let c = Vec3::from(a.center);
            let h = Vec3::from(a.half_extents);
            min = min.min(c - h);
            max = max.max(c + h);
            found = true;
        }
        if let Ok(children) = q_children.get(e) {
            stack.extend(children.iter());
        }
    }
    found.then_some((min, max))
}

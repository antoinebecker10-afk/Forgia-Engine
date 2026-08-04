//! weapon_preview.rs — aperçus **3D live** du hub-menu (render-to-texture).
//!
//! Deux aperçus RTT indépendants, chacun sur son propre layer isolé :
//! - **Arme** (onglet Armes) : le GLB d'arme sélectionné, tourne.
//! - **Personnage** (onglet Forgeron) : le corps + les pièces portées, teintées
//!   à leur rareté, tourne.
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
//! `StartingWeaponChoice`, reconstruction du personnage sur `EquipmentSave`.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy_egui::{egui, EguiContexts, EguiTextureHandle};
use forgia_assets::GameAssets;
use forgia_core::prelude::AppMode;
use forgia_mode_roguelite::avatar::{equipped_key, spawn_equipped_avatar};
use forgia_mode_roguelite::equipment::{EquipmentConfig, EquipmentSave};
use forgia_mode_roguelite::weapon_select::StartingWeaponChoice;

/// Layer de rendu de l'aperçu ARME (0 = monde, 1 = viewmodel FPS déjà pris).
const WEAPON_LAYER: usize = 2;
// Le layer 3 était celui de l'aperçu des BRAS, retiré : l'aperçu du personnage
// montre déjà les bras, et les deux se superposaient à l'origine.
/// Layer de rendu de l'aperçu PERSONNAGE (équipement porté).
const CHARACTER_LAYER: usize = 4;
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

/// Plugin : câble le cycle de vie des deux aperçus RTT sur `AppMode::Menu`.
pub struct WeaponPreviewPlugin;

impl Plugin for WeaponPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppMode::Menu),
            (
                sys_spawn_weapon_preview,
                sys_spawn_character_preview,
            ),
        )
        .add_systems(OnExit(AppMode::Menu), sys_despawn_previews)
        .add_systems(
            Update,
            (
                sys_swap_weapon_preview,
                // Reconstruire AVANT de propager les layers et de calibrer : les
                // pièces qui viennent d'apparaître doivent être vues par la
                // caméra dédiée et entrer dans le cadrage de la même passe.
                sys_sync_character_pieces,
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

// ── Aperçu PERSONNAGE (équipement) ──────────────────────────────────────────
//
// Le pendant visuel du panneau ÉQUIPEMENT : on voit ce qu'on porte, et la pièce
// change de couleur avec sa rareté. C'est la convention de couleur héritée de
// Diablo II / WoW (gris → vert → bleu → violet → or) : elle vaut précisément
// parce qu'elle se lit SANS texte, donc elle doit se voir sur le personnage, pas
// seulement sur une pastille d'interface.

/// Ressource de l'aperçu PERSONNAGE : `TextureId` + conteneur corps/pièces.
#[derive(Resource)]
pub struct CharacterPreviewRtt {
    pub tex_id: egui::TextureId,
    image: Handle<Image>,
    holder: Entity,
    /// Équipement actuellement montré — clé de reconstruction.
    shown: String,
}

/// OnEnter(Menu) — crée l'aperçu 3D du personnage équipé (layer CHARACTER_LAYER).
fn sys_spawn_character_preview(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
    existing: Option<Res<CharacterPreviewRtt>>,
) {
    if existing.is_some() {
        return;
    }
    let (image, tex_id) = create_rtt_image(&mut images, &mut contexts, "character_preview_rtt");
    let layer = RenderLayers::layer(CHARACTER_LAYER);
    spawn_rtt_camera_light(&mut commands, &image, &layer, -3, "CharacterPreviewCamera");

    let pivot = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            PreviewPivot,
            PreviewEntity,
            Name::new("CharacterPreviewPivot"),
        ))
        .id();
    // Conteneur calibré ENSEMBLE (corps + pièces) : le personnage garde son
    // cadrage quoi qu'on lui mette dessus.
    let holder = commands
        .spawn((
            Transform::from_scale(Vec3::splat(0.001)),
            Visibility::Inherited,
            layer,
            Name::new("CharacterPreviewHolder"),
            ChildOf(pivot),
        ))
        .id();

    commands.insert_resource(CharacterPreviewRtt {
        tex_id,
        image,
        holder,
        // Volontairement différent de toute clé réelle (même vide) pour forcer la
        // première construction.
        shown: "\u{0}jamais construit".to_string(),
    });
    info!("[weapon-preview] aperçu 3D personnage spawné (layer {CHARACTER_LAYER})");
}

/// Reconstruit le personnage quand l'équipement porté change (et une fois au
/// premier passage). Le corps est toujours là ; chaque pièce équipée s'ajoute
/// par-dessus, teintée à sa rareté.
fn sys_sync_character_pieces(
    mut commands: Commands,
    assets: Res<AssetServer>,
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    rtt: Option<ResMut<CharacterPreviewRtt>>,
    q_children: Query<&Children>,
) {
    let Some(mut rtt) = rtt else {
        return;
    };
    let key = equipped_key(&save);
    if rtt.shown == key {
        return;
    }
    rtt.shown = key;

    if let Ok(children) = q_children.get(rtt.holder) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }
    // Le montage est partagé avec l'avatar du Hall. Le layer de rendu n'est pas
    // posé ici : `sys_propagate_preview_layers` le pousse depuis le holder à
    // TOUS ses descendants, pièces neuves comprises.
    spawn_equipped_avatar(
        &mut commands,
        &assets,
        &cfg,
        &save,
        rtt.holder,
        Transform::default(),
    );
    // Re-cadrer : le personnage vient de changer d'emprise.
    commands.entity(rtt.holder).insert((
        NeedsPreviewCalibrate,
        Transform::from_scale(Vec3::splat(0.001)),
    ));
}

// ── Systèmes génériques (arme + bras + personnage) ──────────────────────────────

/// Propage le `RenderLayers` de chaque racine d'aperçu à TOUS ses descendants (un
/// `SceneRoot` GLB ne le fait pas en 0.18). Lit le layer porté par la racine.
fn sys_propagate_preview_layers(
    weapon: Option<Res<WeaponPreviewRtt>>,
    character: Option<Res<CharacterPreviewRtt>>,
    q_children: Query<&Children>,
    q_layers: Query<&RenderLayers>,
    mut commands: Commands,
) {
    let roots = [
        weapon.as_ref().map(|w| w.scene_entity),
        character.as_ref().map(|c| c.holder),
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
    character: Option<Res<CharacterPreviewRtt>>,
    q: Query<Entity, With<PreviewEntity>>,
    mut contexts: EguiContexts,
) {
    if let Some(w) = weapon.as_ref() {
        contexts.remove_image(w.image.id());
    }
    if let Some(c) = character.as_ref() {
        contexts.remove_image(c.image.id());
    }
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<WeaponPreviewRtt>();
    commands.remove_resource::<CharacterPreviewRtt>();
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

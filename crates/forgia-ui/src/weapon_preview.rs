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
///
/// Public dans la crate depuis story-678 Phase 5 : le fond d'arène du menu
/// rend CE layer en plus du sien, pour montrer **le même** personnage sans en
/// spawner un second. Dupliquer l'avatar rebrancherait les pièces sur un
/// squelette arbitraire (cf. `reference_shared_skeleton_avatar_pairing`).
pub(crate) const CHARACTER_LAYER: usize = 4;
/// Côté de l'image RTT (px). Carré → viewport carré dans le panneau.
const RTT_SIZE: u32 = 512;
/// Taille cible (plus grande dimension, m) du sujet après calibrage AABB.
///
/// Public dans la crate depuis story-678 Phase 5 : le diorama du fond doit
/// bâtir son décor À L'ÉCHELLE de ce personnage — c'est lui l'unité de mesure
/// de la scène, pas le mètre.
pub(crate) const PREVIEW_TARGET: f32 = 1.15;
/// Vitesse de rotation turntable (rad/s) — un tour en ~9 s.
const PREVIEW_SPIN: f32 = 0.7;
/// Le personnage tourne cinq fois plus lentement que l'arme (~45 s le tour).
///
/// L'aperçu d'arme est le sujet unique de son panneau : il gagne à tourner.
/// L'avatar, lui, occupe le coin d'un écran de préparation qu'on lit pendant
/// qu'on choisit son chapitre — au rythme de l'arme, il tirait l'œil en continu.
const CHARACTER_SPIN_FACTOR: f32 = 0.2;

/// Les slots du choix d'arme et les handles préchargés ont le même ordre.
/// Le modulo conserve le comportement précédent si un choix persistant est
/// issu d'une ancienne version du jeu.
fn weapon_preview_scene(assets: &GameAssets, choice_idx: usize) -> Handle<Scene> {
    assets.weapon_preview_scenes[choice_idx % assets.weapon_preview_scenes.len()].clone()
}

/// Passes consécutives SANS insertion avant de DÉSARMER la propagation des
/// `RenderLayers` (~5 s à 60 fps). Les scènes GLB se peuplent async sur les
/// frames qui suivent un spawn/swap/rebuild — passé ce délai, re-balayer
/// l'arbre chaque frame était un O(N) permanent pour rien (audit 2026-08-07,
/// P1bis). Tout geste qui ajoute des entités remet son compteur à zéro.
pub(crate) const LAYERS_SETTLE_PASSES: u16 = 300;

/// Ressource de l'aperçu ARME : `TextureId` (affiché par `sys_menu_armes`) + entité
/// `SceneRoot` swappable à la sélection.
#[derive(Resource)]
pub struct WeaponPreviewRtt {
    pub tex_id: egui::TextureId,
    image: Handle<Image>,
    scene_entity: Entity,
    shown_idx: usize,
    /// Compteur de désarmement de la propagation des layers (cf. [`LAYERS_SETTLE_PASSES`]).
    layers_settled: u16,
}

/// Marqueur des entités racines d'un aperçu (caméra / lumière / pivot) — despawn
/// récursif au départ du menu (les scènes cascadent via le pivot).
#[derive(Component)]
struct PreviewEntity;

/// Pivot rotatif (tourne autour de Y). Un par aperçu (arme + personnage).
///
/// La vitesse vit SUR le composant : les deux aperçus partagent ce marqueur et
/// tournaient donc au même rythme, alors qu'ils ne jouent pas le même rôle à
/// l'écran (cf. `CHARACTER_SPIN_FACTOR`).
#[derive(Component)]
struct PreviewPivot {
    /// rad/s autour de Y.
    spin: f32,
}

/// Calibrage AABB en attente (recentrage + mise à l'échelle) — ré-armé au swap.
///
/// 🚨 DÉFAUT CONNU, non corrigé (2026-08-05). L'aperçu du personnage est
/// INSTABLE d'un lancement à l'autre — quatre lancements, trois résultats :
/// personnage entier mais pièces désalignées · cadré sur les seules jambes ·
/// panneau vide (deux fois). Le sujet est fait de SIX scènes glTF (le corps et
/// ses cinq pièces) qui se peuplent sur des frames différentes, et ce calibrage
/// se fige sur la première venue — donc sur un sous-ensemble arbitraire.
///
/// Faire CONVERGER le calibrage (recalculer tant que l'étendue bouge, refermer
/// sur une mesure stable) n'a rien changé : le panneau restait vide AVANT comme
/// APRÈS le retrait de cette tentative. La course au chargement explique la
/// variabilité, mais elle n'explique pas à elle seule le panneau vide.
///
/// À reprendre par la MESURE (journaliser `extent`/`scale` et le compte de
/// descendants porteurs d'`Aabb` à chaque passe), pas par raisonnement : deux
/// hypothèses ont déjà été réfutées par l'observation.
#[derive(Component)]
struct NeedsPreviewCalibrate;

/// Plugin : câble le cycle de vie des deux aperçus RTT sur `AppMode::Menu`.
pub struct WeaponPreviewPlugin;

impl Plugin for WeaponPreviewPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppMode::Menu),
            (sys_spawn_weapon_preview, sys_spawn_character_preview),
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
                sys_gate_preview_cameras,
            )
                .chain()
                .run_if(in_state(AppMode::Menu)),
        );
    }
}

// ── Création de l'image RTT (partagée arme/bras) ────────────────────────────────

/// Crée une image render-target + l'enregistre auprès d'egui (une fois).
///
/// `width`/`height` séparés : les aperçus sont carrés (viewport carré dans leur
/// panneau), le fond d'arène est au format de l'écran.
pub(crate) fn create_rtt_image(
    images: &mut Assets<Image>,
    contexts: &mut EguiContexts,
    label: &'static str,
    width: u32,
    height: u32,
) -> (Handle<Image>, egui::TextureId) {
    let size = Extent3d {
        width,
        height,
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

/// Caméra de l'aperçu ARME — pour la couper hors de la page Armes (story-691).
#[derive(Component)]
struct WeaponPreviewCam;
/// Caméra de l'aperçu PERSONNAGE — coupée hors Forgeron/Sac (story-691).
#[derive(Component)]
struct CharacterPreviewCam;

/// Frames de grâce à l'entrée du menu pendant lesquelles les DEUX caméras
/// d'aperçu restent actives, quelle que soit la page.
///
/// Le premier rendu réel compile les pipelines PBR de ces vues — un warmup en
/// `Visibility::Hidden` ne le fait PAS (`reference_pbr_pipeline_warmup_frustum_trap`).
/// Sans cette grâce, la première ouverture de la page Armes/Forgeron paierait
/// la compilation en hitch visible ; ici elle se paie sous l'anim d'entrée du
/// menu, où personne ne la voit.
#[derive(Resource)]
struct PreviewCamWarmup(u8);
const PREVIEW_CAM_WARMUP_FRAMES: u8 = 5;

/// Spawn caméra RTT (order négatif, cible = image, clear opaque sombre) + lumière
/// dédiée, sur le layer donné. Les entités portent `PreviewEntity`. Rend
/// l'`Entity` de la caméra pour que l'appelant y pose son marqueur de gate.
fn spawn_rtt_camera_light(
    commands: &mut Commands,
    image: &Handle<Image>,
    layer: &RenderLayers,
    order: isize,
    name: &'static str,
) -> Entity {
    // Fond STUDIO opaque pour les deux aperçus (audit 2026-08-06). Le clear
    // transparent de l'aperçu personnage n'a jamais fonctionné — le tonemapping
    // force alpha=1 dans egui — et depuis la refonte Dicero le portrait est
    // CADRÉ, donc un fond sombre assorti aux panneaux est le comportement
    // voulu, pas un pis-aller.
    let clear = ClearColorConfig::Custom(Color::srgba(0.06, 0.05, 0.09, 1.0));
    let cam = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order,
                clear_color: clear,
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Transform::from_xyz(0.0, 0.15, 1.7).looking_at(Vec3::ZERO, Vec3::Y),
            layer.clone(),
            PreviewEntity,
            // Exclut ces caméras du grading d'univers — leur fond ressortait rose
            // ambiance au lieu du sombre studio (cf. `color_grading::sys_apply`).
            forgia_core::prelude::UiStudioCamera,
            Name::new(name),
        ))
        .id();
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
    cam
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
    let (image, tex_id) = create_rtt_image(
        &mut images,
        &mut contexts,
        "weapon_preview_rtt",
        RTT_SIZE,
        RTT_SIZE,
    );
    let layer = RenderLayers::layer(WEAPON_LAYER);
    let cam = spawn_rtt_camera_light(&mut commands, &image, &layer, -1, "WeaponPreviewCamera");
    commands.entity(cam).insert(WeaponPreviewCam);
    commands.insert_resource(PreviewCamWarmup(PREVIEW_CAM_WARMUP_FRAMES));

    let pivot = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            PreviewPivot { spin: PREVIEW_SPIN },
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
        layers_settled: 0,
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
    // La nouvelle scène va se peupler sur les frames qui viennent : la
    // propagation des layers doit se réarmer pour la voir.
    rtt.layers_settled = 0;
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
    /// Ce que la dernière construction a RÉELLEMENT créé (corps + pièces).
    ///
    /// 🚨 On ne nettoie pas via les `Children` du holder. Mesuré le 2026-08-06 :
    /// le balayage retirait tantôt 6 entités, tantôt 5, alors que l'avatar en
    /// compte toujours 6 (un corps, cinq pièces). La pièce oubliée survivait à
    /// la reconstruction, gardait ses os branchés sur le corps DÉTRUIT — donc
    /// sans transform — et restait figée dans le monde pendant que le nouveau
    /// corps tournait. C'est le « l'armure ne tourne plus » rapporté en jeu.
    ///
    /// `Children` est une relation dérivée : s'en servir pour détruire, c'est
    /// faire confiance à un miroir. On garde donc la liste de ce qu'on a spawné
    /// — `spawn_equipped_avatar` la rend déjà, elle était jetée.
    parts: Vec<Entity>,
    /// Équipement actuellement montré — clé de reconstruction.
    shown: String,
    /// Compteur de désarmement de la propagation des layers (cf. [`LAYERS_SETTLE_PASSES`]).
    layers_settled: u16,
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
    let (image, tex_id) = create_rtt_image(
        &mut images,
        &mut contexts,
        "character_preview_rtt",
        RTT_SIZE,
        RTT_SIZE,
    );
    let layer = RenderLayers::layer(CHARACTER_LAYER);
    let cam = spawn_rtt_camera_light(&mut commands, &image, &layer, -3, "CharacterPreviewCamera");
    commands.entity(cam).insert(CharacterPreviewCam);

    let pivot = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            PreviewPivot {
                spin: PREVIEW_SPIN * CHARACTER_SPIN_FACTOR,
            },
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
        parts: Vec::new(),
        // Volontairement différent de toute clé réelle (même vide) pour forcer la
        // première construction.
        shown: "\u{0}jamais construit".to_string(),
        layers_settled: 0,
    });
    info!("[weapon-preview] aperçu 3D personnage spawné (layer {CHARACTER_LAYER})");
}

/// Reconstruit le personnage quand l'équipement porté change (et une fois au
/// premier passage). Le corps est toujours là ; chaque pièce équipée s'ajoute
/// par-dessus, teintée à sa rareté.
fn sys_sync_character_pieces(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut body_handles: ResMut<forgia_mode_roguelite::avatar::AvatarBodyHandles>,
    cfg: Res<EquipmentConfig>,
    save: Res<EquipmentSave>,
    rtt: Option<ResMut<CharacterPreviewRtt>>,
) {
    let Some(mut rtt) = rtt else {
        return;
    };
    let key = equipped_key(&save);
    if rtt.shown == key {
        return;
    }
    rtt.shown = key;
    // Corps + pièces vont être reconstruits : réarme la propagation des layers.
    rtt.layers_settled = 0;

    // On détruit EXACTEMENT ce qu'on avait créé (cf. `CharacterPreviewRtt::parts`).
    // `try_despawn` et non `despawn` : une pièce peut déjà être partie par une
    // autre voie, et ce nettoyage ne doit jamais faire tomber le jeu.
    let holder = rtt.holder;
    let previous: Vec<Entity> = std::mem::take(&mut rtt.parts);
    let removed = previous.len();
    for part in previous {
        if let Ok(mut ec) = commands.get_entity(part) {
            ec.try_despawn();
        }
    }
    // Trace de RECONSTRUCTION (une ligne par changement d'équipement, jamais par
    // frame). Le compte DOIT être stable d'une reconstruction à l'autre : c'est
    // son alternance 6/5 qui a révélé les pièces fantômes.
    info!("[character-preview] reconstruction : {removed} entité(s) retirée(s)");
    // Le montage est partagé avec l'avatar du Hall. Le layer de rendu n'est pas
    // posé ici : `sys_propagate_preview_layers` le pousse depuis le holder à
    // TOUS ses descendants, pièces neuves comprises.
    rtt.parts = spawn_equipped_avatar(
        &mut commands,
        &assets,
        &mut body_handles,
        &cfg,
        &save,
        holder,
        Transform::default(),
    );
    // Re-cadrer : le personnage vient de changer d'emprise. Le calibrage repart
    // à zéro et convergera au fil de l'arrivée des six scènes.
    commands.entity(holder).insert((
        NeedsPreviewCalibrate,
        Transform::from_scale(Vec3::splat(0.001)),
    ));
}

// ── Systèmes génériques (arme + bras + personnage) ──────────────────────────────

/// Balaye l'arbre sous `root` et pose le `RenderLayers` de la racine sur tout
/// descendant qui ne le porte pas — puis se DÉSARME (audit 2026-08-07, P1bis).
///
/// L'implémentation UNIQUE de la classe « propagation de layers » : l'aperçu
/// arme, le personnage et le diorama du fond (`arena_backdrop`) la partagent.
/// Un `SceneRoot` GLB ne propage pas les layers en 0.18 ; les scènes se
/// peuplent async, donc on re-balaye — mais pas pour toujours : après
/// [`LAYERS_SETTLE_PASSES`] passes sans rien poser, ce O(N) s'arrête jusqu'au
/// prochain réarmement (`settled = 0` posé par les spawns/swaps/rebuilds).
/// Pile fournie par l'appelant (`Local` du système) : zéro alloc par frame.
pub(crate) fn propagate_layers_from(
    root: Entity,
    settled: &mut u16,
    q_children: &Query<&Children>,
    q_layers: &Query<&RenderLayers>,
    commands: &mut Commands,
    stack: &mut Vec<Entity>,
) {
    if *settled >= LAYERS_SETTLE_PASSES {
        return;
    }
    let Ok(target) = q_layers.get(root).cloned() else {
        return;
    };
    let mut inserted = false;
    stack.clear();
    stack.push(root);
    while let Some(e) = stack.pop() {
        if q_layers.get(e).map(|l| *l != target).unwrap_or(true) {
            commands.entity(e).insert(target.clone());
            inserted = true;
        }
        if let Ok(children) = q_children.get(e) {
            stack.extend(children.iter());
        }
    }
    *settled = if inserted { 0 } else { settled.saturating_add(1) };
}

/// Propage le `RenderLayers` de chaque racine d'aperçu à TOUS ses descendants,
/// via [`propagate_layers_from`] (désarmement + pile réutilisée).
fn sys_propagate_preview_layers(
    weapon: Option<ResMut<WeaponPreviewRtt>>,
    character: Option<ResMut<CharacterPreviewRtt>>,
    q_children: Query<&Children>,
    q_layers: Query<&RenderLayers>,
    mut commands: Commands,
    mut stack: Local<Vec<Entity>>,
) {
    if let Some(mut w) = weapon {
        let root = w.scene_entity;
        propagate_layers_from(
            root,
            &mut w.layers_settled,
            &q_children,
            &q_layers,
            &mut commands,
            &mut stack,
        );
    }
    if let Some(mut c) = character {
        let root = c.holder;
        propagate_layers_from(
            root,
            &mut c.layers_settled,
            &q_children,
            &q_layers,
            &mut commands,
            &mut stack,
        );
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

/// Rotation turntable des aperçus, chacun à SA vitesse (arme + personnage).
///
/// `Time<Real>` et pas `Time` : anti-trap CLAUDE.md §6 « UI/menu = Real » — le
/// fond d'arène fait déjà ce choix pour la même raison (le menu ne doit pas se
/// figer si le temps virtuel est en pause).
fn sys_rotate_previews(
    time: Res<Time<bevy::time::Real>>,
    mut q: Query<(&mut Transform, &PreviewPivot)>,
) {
    let dt = time.delta_secs();
    for (mut tf, pivot) in &mut q {
        tf.rotate_y(pivot.spin * dt);
    }
}

/// Coupe les caméras d'aperçu hors de LEUR page (story-691) : sur l'Accueil —
/// la page la plus fréquentée — le GPU payait chaque frame deux passes 3D 512²
/// pour deux textures que personne n'affichait. Le personnage du FOND, lui,
/// reste rendu par la caméra du diorama (`arena_backdrop`), pas par celles-ci.
fn sys_gate_preview_cameras(
    page: Res<crate::MenuPage>,
    warmup: Option<ResMut<PreviewCamWarmup>>,
    mut q_weapon: Query<&mut Camera, (With<WeaponPreviewCam>, Without<CharacterPreviewCam>)>,
    mut q_char: Query<&mut Camera, With<CharacterPreviewCam>>,
) {
    // Grâce de warmup : laisser les deux vues rendre quelques frames pour
    // compiler leurs pipelines pendant l'entrée du menu (cf. PreviewCamWarmup).
    if let Some(mut w) = warmup {
        if w.0 > 0 {
            w.0 -= 1;
            return;
        }
    }
    let weapon_on = matches!(*page, crate::MenuPage::Armes);
    let char_on = matches!(*page, crate::MenuPage::Forgeron | crate::MenuPage::Sac);
    for mut cam in &mut q_weapon {
        if cam.is_active != weapon_on {
            cam.is_active = weapon_on;
        }
    }
    for mut cam in &mut q_char {
        if cam.is_active != char_on {
            cam.is_active = char_on;
        }
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

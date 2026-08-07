//! arena_backdrop.rs — **le fond du menu est le DÉCOR que le joueur a équipé**.
//!
//! Story-678 Phase 5. Le fond du hub était une vidéo pré-extraite de branding
//! (`menu_video.rs`, 361 frames WebP) : jolie, mais elle ne disait rien du
//! joueur. Le menu montre maintenant un lieu du jeu, avec **les props de la
//! vraie arène**, et son personnage équipé debout dedans.
//!
//! **Révision du 2026-08-06** : ce fond suivait d'abord le chapitre le plus haut
//! atteint — il MIROITAIT la progression. Il est devenu une **cosmétique de
//! collection** : on en débloque (en battant un chapitre, ou en les achetant),
//! et le joueur choisit celui qu'il affiche. Le catalogue et la règle de
//! possession vivent dans `forgia_mode_roguelite::cosmetics` ; ce module ne
//! fait que **rendre** ce qu'on lui désigne.
//!
//! ## Ce qui rend ce fond honnête
//!
//! Il n'y a pas d'« asset d'arène » à charger : une arène Forgia est procédurale.
//! Un décor est donc **une palette de props + une ambiance** — ce qu'on voit et
//! la lumière dans laquelle on le voit. Le diorama est bâti avec **exactement**
//! les GLB que l'arène spawne, calibrés aux **mêmes tailles cibles**
//! (`RogueliteDecorConfig::target_*`, en mètres) et noyé dans **la même brume**
//! que l'univers correspondant. Deux décors peuvent partager leurs props et
//! différer par leur seule ambiance : c'est ce qui rend les variantes possibles
//! sans un seul asset neuf.
//!
//! ## Les deux pièges évités par construction
//!
//! 1. **Le squelette partagé.** On ne spawne PAS un second avatar : la caméra du
//!    fond rend `CHARACTER_LAYER` *en plus* du sien, donc elle filme le
//!    personnage qui existe déjà. Deux avatars vivants rebrancheraient les
//!    pièces d'équipement sur un corps arbitraire
//!    (`reference_shared_skeleton_avatar_pairing`).
//! 2. **Le tonemapping qui force alpha = 1.** La tentative précédente affichait
//!    l'avatar en RTT *transparent* par-dessus le fond, et récupérait un
//!    rectangle opaque en travers de l'écran. Ici le fond est **une seule image
//!    opaque** contenant décor ET personnage : il n'y a plus de composition
//!    alpha à réussir. C'est aussi pourquoi on n'ajoute **aucune passe
//!    fullscreen** — celles-ci font tomber DX12 (`reference_runtime_root_and_single_pass_outline`).
//!
//! ## Échelle
//!
//! Le personnage de l'aperçu est calibré à [`PREVIEW_TARGET`] unités pour un
//! humain de [`PLAYER_HEIGHT_M`] mètres. Tout le reste s'en déduit
//! ([`units_per_meter`]) : un landmark de 16 m fait 9,2 unités, et la densité de
//! brume (donnée *par mètre*) est convertie. Aucune taille n'est choisie.

use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::gltf::GltfAssetLabel;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::AppMode;
use forgia_mode_roguelite::ambiances::{Ambiance, AmbiancesConfig};
use forgia_mode_roguelite::cosmetics::{Cosmetic, CosmeticsConfig, OwnedCosmetics};
use forgia_mode_roguelite::decor::RogueliteDecorConfig;
use forgia_mode_roguelite::decor_palettes::{DecorPalette, DecorPalettesConfig};
use forgia_mode_roguelite::identity::IdentitySave;
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_mode_roguelite::render_quality::RogueliteRenderConfig;

use crate::weapon_preview::{create_rtt_image, CHARACTER_LAYER, PREVIEW_TARGET};

/// Layer de rendu du décor de fond. 0 = monde · 1 = viewmodel FPS · 2 = aperçu
/// arme · 3 = libre (ex-aperçu bras, retiré) · 4 = aperçu personnage.
const ARENA_LAYER: usize = 5;

/// Taille du joueur (m) — capsule 2,0 m, invariant de `map-design-patterns` §métriques.
/// Sert d'étalon : c'est ce qui relie les unités du diorama aux mètres du génome.
const PLAYER_HEIGHT_M: f32 = 2.0;

/// Format de REPLI du fond quand la fenêtre est illisible (16:9, l'écran
/// cible). Depuis story-692 le format réel vient de la fenêtre : figé à 16:9,
/// l'image était étirée de ~+31 % en 21:9 et le personnage — le sujet de
/// l'écran — devenait visiblement déformé.
const BACKDROP_FALLBACK_ASPECT: f32 = 16.0 / 9.0;

/// Frames de stabilité exigées avant de reconstruire le RTT après un resize :
/// un drag de bord de fenêtre change la taille CHAQUE frame, et recréer une
/// image render-target par frame est le genre de rafale qui a déjà produit des
/// races wgpu fatales côté fenêtre (cf. `apply_window_settings`).
const RESIZE_STABLE_FRAMES: u8 = 30;

/// L'aspect voulu du fond — celui de la fenêtre, source unique partagée par le
/// spawn et la reconstruction sur resize.
fn window_aspect(win: Option<&Window>) -> f32 {
    win.map(|w| w.width() / w.height().max(1.0))
        .unwrap_or(BACKDROP_FALLBACK_ASPECT)
}

/// Ordre de la caméra RTT du fond — avant les aperçus (-1 arme, -3 personnage)
/// et avant la caméra du menu, pour que l'image soit prête au pass egui.
const BACKDROP_CAMERA_ORDER: isize = -5;

/// Champ de vision vertical de la caméra du fond (rad). Plus serré que le défaut
/// Bevy : un diorama se lit mieux au téléobjectif, la perspective écrase moins
/// le décor lointain.
const BACKDROP_FOV_Y: f32 = 0.6;

/// Part de la hauteur d'image occupée par le personnage. En dessous il devient
/// une figurine, au-dessus il mange le décor qu'on vient montrer.
const CHARACTER_SCREEN_FRACTION: f32 = 0.46;

/// Position horizontale du personnage dans l'image (0 = bord gauche, 1 = droit).
/// « Le personnage à droite de l'écran » — le tiers droit, règle des tiers.
const CHARACTER_SCREEN_X: f32 = 0.70;

/// Amplitude de la dérive de caméra, en fraction de la largeur visible. Assez
/// pour que le fond respire, assez peu pour que le personnage garde son tiers.
const DRIFT_AMPLITUDE: f32 = 0.035;
/// Périodes de la dérive (s) — deux périodes premières entre elles pour que le
/// mouvement ne se referme pas sur un cycle court et lisible.
const DRIFT_PERIOD_X: f32 = 37.0;
const DRIFT_PERIOD_Y: f32 = 23.0;

/// Distances (en hauteurs de personnage) des trois plans du diorama.
const RING_LANDMARK: f32 = 9.0;
const RING_BIG: f32 = 5.5;
const RING_SCATTER: f32 = 2.6;

/// Combien de props par plan. Le diorama n'est pas une arène : il en montre une
/// tranche lisible, pas ses 200 props.
const COUNT_LANDMARK: usize = 3;
const COUNT_BIG: usize = 6;
const COUNT_BRAZIER: usize = 3;
const COUNT_SCATTER: usize = 5;

/// Le décor occupe l'arc DERRIÈRE le personnage, ouvert vers la caméra : un
/// anneau complet mettrait des props devant lui et boucherait la lecture.
/// Bornes en radians autour de l'axe caméra→scène.
const ARC_START: f32 = -2.5;
const ARC_END: f32 = 2.5;

/// Unités du diorama par mètre du génome.
///
/// Le personnage vaut [`PREVIEW_TARGET`] unités pour [`PLAYER_HEIGHT_M`] mètres :
/// tout le décor se déduit de ce rapport. C'est la seule conversion du module.
fn units_per_meter() -> f32 {
    PREVIEW_TARGET / PLAYER_HEIGHT_M
}

/// Distance caméra→sujet qui donne au personnage sa part d'image voulue.
///
/// Dérivée, pas choisie : à distance `d`, la hauteur visible d'une caméra de
/// champ `f` vaut `2·d·tan(f/2)`. On veut que le personnage
/// ([`PREVIEW_TARGET`]) en occupe [`CHARACTER_SCREEN_FRACTION`].
fn camera_distance() -> f32 {
    let visible_height = PREVIEW_TARGET / CHARACTER_SCREEN_FRACTION;
    visible_height / (2.0 * (BACKDROP_FOV_Y * 0.5).tan())
}

/// Largeur visible à la distance de la caméra, pour l'aspect DONNÉ (story-692 :
/// l'aspect vient de la fenêtre — figé à 16:9, le personnage glissait de son
/// tiers dès que l'écran n'était pas 16:9).
fn visible_width(aspect: f32) -> f32 {
    let visible_height = PREVIEW_TARGET / CHARACTER_SCREEN_FRACTION;
    visible_height * aspect
}

/// De combien décaler la visée pour que le personnage (à l'origine) tombe à
/// [`CHARACTER_SCREEN_X`] dans l'image. Positif = on vise à gauche de lui.
fn aim_offset_x(aspect: f32) -> f32 {
    (CHARACTER_SCREEN_X - 0.5) * visible_width(aspect)
}

/// Niveau du sol : le calibrage AABB centre le personnage sur l'origine, donc
/// ses pieds sont une demi-hauteur plus bas.
fn ground_y() -> f32 {
    -PREVIEW_TARGET * 0.5
}

/// Le décor à rendre : **celui que le joueur a équipé**.
///
/// Révision du 2026-08-06 : le fond suivait automatiquement le chapitre le plus
/// haut atteint — il MIROITAIT la progression. C'est devenu une cosmétique de
/// collection, donc c'est la sauvegarde d'identité qui décide, et le catalogue
/// qui valide (une sauvegarde bricolée ne doit pas afficher un décor non gagné :
/// `CosmeticsConfig::resolve_decor` refait le contrôle).
fn equipped_backdrop<'a>(
    cfg: &'a CosmeticsConfig,
    identity: &IdentitySave,
    meta: Option<&MetaShopSave>,
) -> Option<&'a Cosmetic> {
    let owned = OwnedCosmetics {
        chapters_cleared: meta.map(|m| m.chapters_cleared).unwrap_or(0),
        identity,
    };
    cfg.resolve_decor(&identity.equipped_backdrop, &owned)
}

/// PRNG déterministe (splitmix64) — même chapitre, même diorama, à chaque
/// lancement. Un décor qui change à chaque retour au menu se lit comme un bug.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1))
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Flottant dans [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }
    /// Flottant dans [lo, hi).
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() * (hi - lo)
    }
    /// Un élément de `xs`, ou `None` si la liste est vide (palette incomplète).
    fn pick<'a>(&mut self, xs: &'a [String]) -> Option<&'a str> {
        if xs.is_empty() {
            return None;
        }
        Some(xs[(self.next_u64() as usize) % xs.len()].as_str())
    }
}

/// Ressource du fond : l'image à peindre + de quoi le reconstruire.
#[derive(Resource)]
pub struct ArenaBackdropRtt {
    /// Texture à afficher plein écran par le menu.
    pub tex_id: egui::TextureId,
    image: Handle<Image>,
    /// Racine du diorama — tout le décor lui est enfant (despawn récursif).
    root: Entity,
    /// La caméra, pour la faire dériver.
    camera: Entity,
    /// Décor actuellement bâti (id du catalogue) + la palette et l'ambiance
    /// qu'il désignait alors.
    ///
    /// Les trois, et pas seulement l'id : le catalogue et les palettes sont
    /// hot-reloadables, donc « même décor » ne veut pas dire « même contenu ».
    /// Ne comparer que l'id ferait qu'éditer un TOML ne change rien à l'écran.
    shown: BuiltKey,
    /// Nombre de props RÉELLEMENT posés à la dernière construction.
    ///
    /// C'est la mesure que le capteur publie. `rtt.is_some()` ne prouvait rien :
    /// la ressource existe même si la palette est vide et que le fond est nu.
    pub props_spawned: usize,
}

impl ArenaBackdropRtt {
    /// L'id du décor actuellement bâti — publié au capteur.
    pub fn shown_backdrop(&self) -> &str {
        &self.shown.backdrop
    }

    /// Le diorama est-il réellement À L'ÉCRAN ? Source unique de la condition
    /// « fond d'arène visible » (le choix du fond dans `main_menu_ui` ET le gel
    /// de la vidéo la lisent) : écrite deux fois, elle divergerait — un fond
    /// choisi sur un critère et une vidéo gelée sur un autre.
    pub fn is_showing(&self) -> bool {
        self.props_spawned > 0
    }
}

/// Ce qui, s'il change, oblige à rebâtir le diorama.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BuiltKey {
    backdrop: String,
    palette: String,
    ambiance: String,
}

/// Marqueur des entités du fond (caméra, lumière, racine) — despawn OnExit.
#[derive(Component)]
struct BackdropEntity;

/// Prop en attente de calibrage AABB, avec sa taille cible en unités du diorama.
#[derive(Component)]
struct NeedsBackdropCalibrate {
    target: f32,
}

/// Plugin du fond d'arène.
pub struct ArenaBackdropPlugin;

impl Plugin for ArenaBackdropPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppMode::Menu), sys_spawn_arena_backdrop)
            .add_systems(OnExit(AppMode::Menu), sys_despawn_arena_backdrop)
            .add_systems(
                Update,
                (
                    // Rebâtir AVANT de propager/calibrer : les props qui viennent
                    // d'apparaître doivent être vus et cadrés dans la même passe.
                    sys_resize_backdrop_on_window,
                    sys_rebuild_backdrop_on_change,
                    sys_propagate_backdrop_layers,
                    sys_calibrate_backdrop_props,
                    sys_drift_backdrop_camera,
                )
                    .chain()
                    .run_if(in_state(AppMode::Menu)),
            );
    }
}

/// OnEnter(Menu) — crée l'image RTT, la caméra et la racine du diorama.
///
/// Le décor lui-même n'est pas posé ici : il dépend du génome de palette, qui
/// peut n'être pas encore chargé. `sys_rebuild_backdrop_on_change` s'en
/// charge dès qu'il l'est, et à chaque hot-reload.
fn sys_spawn_arena_backdrop(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
    render_cfg: Option<Res<RogueliteRenderConfig>>,
    existing: Option<Res<ArenaBackdropRtt>>,
    q_win: Query<&Window, With<bevy::window::PrimaryWindow>>,
) {
    if existing.is_some() {
        return;
    }
    let aspect = window_aspect(q_win.single().ok());
    let height = render_cfg
        .as_deref()
        .map(|c| c.ui_backdrop_height_px)
        .unwrap_or(RogueliteRenderConfig::default().ui_backdrop_height_px);
    let width = ((height as f32) * aspect).round() as u32;
    let (image, tex_id) = create_rtt_image(
        &mut images,
        &mut contexts,
        "arena_backdrop_rtt",
        width,
        height,
    );

    // La caméra voit le décor ET le personnage : c'est ce qui « fond » l'un dans
    // l'autre, sans dupliquer d'avatar.
    let layers = RenderLayers::from_layers(&[ARENA_LAYER, CHARACTER_LAYER]);
    let d = camera_distance();
    let aim = aim_offset_x(aspect);
    let eye_y = ground_y() + PREVIEW_TARGET * 0.62;
    let camera = commands
        .spawn((
            Camera3d::default(),
            Camera {
                order: BACKDROP_CAMERA_ORDER,
                // Opaque : ce fond n'est jamais composé en alpha (cf. §pièges).
                clear_color: ClearColorConfig::Custom(Color::srgb(0.03, 0.03, 0.05)),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: BACKDROP_FOV_Y,
                aspect_ratio: aspect,
                ..default()
            }),
            RenderTarget::Image(image.clone().into()),
            Transform::from_xyz(-aim, eye_y, d).looking_at(Vec3::new(-aim, eye_y * 0.6, 0.0), Vec3::Y),
            layers,
            BackdropEntity,
            Name::new("ArenaBackdropCamera"),
        ))
        .id();

    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            RenderLayers::layer(ARENA_LAYER),
            BackdropEntity,
            Name::new("ArenaBackdropRoot"),
        ))
        .id();

    commands.insert_resource(ArenaBackdropRtt {
        tex_id,
        image,
        root,
        camera,
        // Volontairement vide : force la première construction.
        shown: BuiltKey::default(),
        props_spawned: 0,
    });
    info!("[arena-backdrop] fond d'arène spawné ({width}×{height}, layer {ARENA_LAYER})");
}

/// Reconstruit l'IMAGE du fond quand l'aspect de la fenêtre change (story-692).
///
/// L'image RTT était créée une fois au spawn, au format 16:9 — un resize en
/// cours de session (fenêtre resizable, bascule borderless↔fenêtré) laissait
/// une image étirée pour toujours. Seul l'ASPECT déclenche : la hauteur du RTT
/// est un budget GPU du génome, pas une propriété de la fenêtre. L'aspect bâti
/// est DÉRIVÉ de l'image elle-même — un champ miroir aurait fini par mentir.
fn sys_resize_backdrop_on_window(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut contexts: EguiContexts,
    render_cfg: Option<Res<RogueliteRenderConfig>>,
    rtt: Option<ResMut<ArenaBackdropRtt>>,
    q_win: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_proj: Query<&mut Projection>,
    mut stable: Local<(f32, u8)>,
) {
    let Some(mut rtt) = rtt else { return };
    let Ok(win) = q_win.single() else { return };
    let aspect_now = window_aspect(Some(win));
    if (stable.0 - aspect_now).abs() > 1e-3 {
        *stable = (aspect_now, 0);
        return;
    }
    if stable.1 < RESIZE_STABLE_FRAMES {
        stable.1 += 1;
        return;
    }
    let built_aspect = images
        .get(&rtt.image)
        .map(|img| {
            let s = img.size();
            s.x as f32 / (s.y.max(1)) as f32
        })
        .unwrap_or(BACKDROP_FALLBACK_ASPECT);
    if (built_aspect - aspect_now).abs() < 0.02 {
        return;
    }
    contexts.remove_image(rtt.image.id());
    let height = render_cfg
        .as_deref()
        .map(|c| c.ui_backdrop_height_px)
        .unwrap_or(RogueliteRenderConfig::default().ui_backdrop_height_px);
    let width = (((height as f32) * aspect_now).round() as u32).max(16);
    let (image, tex_id) = create_rtt_image(
        &mut images,
        &mut contexts,
        "arena_backdrop_rtt",
        width,
        height,
    );
    if let Ok(mut ec) = commands.get_entity(rtt.camera) {
        ec.insert(RenderTarget::Image(image.clone().into()));
    }
    if let Ok(mut proj) = q_proj.get_mut(rtt.camera) {
        if let Projection::Perspective(p) = &mut *proj {
            p.aspect_ratio = aspect_now;
        }
    }
    rtt.image = image;
    rtt.tex_id = tex_id;
    info!("[arena-backdrop] RTT reconstruit {width}×{height} (aspect {aspect_now:.2})");
}

/// Rebâtit le diorama quand le joueur change de décor — ou quand un génome
/// (catalogue, palette) est rechargé à chaud sous le même décor.
#[allow(clippy::too_many_arguments)]
fn sys_rebuild_backdrop_on_change(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    save: Option<Res<MetaShopSave>>,
    identity: Option<Res<IdentitySave>>,
    catalogue: Option<Res<CosmeticsConfig>>,
    palettes: Option<Res<DecorPalettesConfig>>,
    ambiances: Option<Res<AmbiancesConfig>>,
    decor_cfg: Option<Res<RogueliteDecorConfig>>,
    render_cfg: Option<Res<RogueliteRenderConfig>>,
    rtt: Option<ResMut<ArenaBackdropRtt>>,
    mut q_camera: Query<&mut Camera>,
) {
    let Some(mut rtt) = rtt else {
        return;
    };
    // Le fond peut être coupé en data (coût GPU, ou panne) : on ne bâtit rien —
    // et on DÉMONTE ce qui l'était déjà (QA story-691). Ce chemin faisait un
    // simple `return` sans jamais remettre `props_spawned` à 0 : le décor
    // restait à l'écran, `is_showing()` restait vrai, la vidéo restait gelée —
    // le toggle documenté comme escape hatch était inerte jusqu'à la sortie du
    // menu. Décor à terre, caméra coupée, clé oubliée : la réactivation
    // reconstruit tout par le chemin normal ci-dessous.
    if !render_cfg
        .as_deref()
        .map(|c| c.ui_backdrop_enabled)
        .unwrap_or(true)
    {
        if rtt.props_spawned > 0 || rtt.shown != BuiltKey::default() {
            if let Ok(mut ec) = commands.get_entity(rtt.root) {
                ec.try_despawn();
            }
            rtt.props_spawned = 0;
            rtt.shown = BuiltKey::default();
            if let Ok(mut cam) = q_camera.get_mut(rtt.camera) {
                cam.is_active = false;
            }
            info!("[arena-backdrop] ui_backdrop_enabled=false → diorama démonté à chaud");
        }
        return;
    }
    let (Some(palettes), Some(catalogue)) = (palettes.as_deref(), catalogue.as_deref()) else {
        return;
    };
    // Le décor ÉQUIPÉ, validé contre ce que le joueur possède réellement.
    // Sans sauvegarde d'identité on ne sait pas quoi montrer : on attend
    // plutôt que de bâtir un décor qui sera remplacé la frame d'après.
    let Some(identity) = identity.as_deref() else {
        return;
    };
    let Some(decor_choisi) = equipped_backdrop(catalogue, identity, save.as_deref()).cloned()
    else {
        return;
    };
    let key = BuiltKey {
        backdrop: decor_choisi.id.clone(),
        palette: decor_choisi.palette.clone(),
        ambiance: decor_choisi.ambiance.clone(),
    };
    if rtt.shown == key {
        return;
    }
    let Some(palette) = palettes.palette(&decor_choisi.palette) else {
        // Le catalogue pointe une palette qui n'existe pas : on ne bâtit rien
        // plutôt qu'un décor vide, et le capteur le dira (props = 0).
        warn!(
            "[arena-backdrop] « {} » pointe la palette « {} », absente du génome",
            decor_choisi.id, decor_choisi.palette
        );
        return;
    };
    rtt.shown = key;

    // Table rase : on détruit la racine et on en refait une. Le décor est
    // ENTIÈREMENT enfant d'elle, donc rien ne survit à la reconstruction (le
    // piège des pièces fantômes de l'aperçu personnage vient précisément d'un
    // nettoyage partiel).
    if let Ok(mut ec) = commands.get_entity(rtt.root) {
        ec.try_despawn();
    }
    let root = commands
        .spawn((
            Transform::default(),
            Visibility::Inherited,
            RenderLayers::layer(ARENA_LAYER),
            BackdropEntity,
            Name::new("ArenaBackdropRoot"),
        ))
        .id();
    rtt.root = root;

    let decor = decor_cfg
        .as_deref()
        .copied()
        .unwrap_or_else(RogueliteDecorConfig::default);
    // L'ambiance vient du DÉCOR CHOISI, plus de la rotation de round : c'est
    // elle qui fait que deux décors partageant leurs props (« La Forge » et
    // « La Forge au crépuscule ») sont deux lieux différents. `ambiance()`
    // replie tout seul sur l'ambiance par défaut si l'id est vide ou inconnu.
    let ambiance = ambiances
        .as_deref()
        .map(|a| a.ambiance(&decor_choisi.ambiance).clone())
        .unwrap_or_else(Ambiance::forge_ardente);

    // La graine du semis est l'id du décor : deux décors sur la MÊME palette
    // doivent composer différemment, sinon la variante achetée n'est qu'un
    // filtre de couleur posé sur une image déjà vue.
    let posed = build_diorama(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        root,
        palette,
        &decor,
        &ambiance,
        seed_of(&decor_choisi.id),
    );
    rtt.props_spawned = posed;
    // Réactive la caméra si un démontage à chaud l'avait coupée.
    if let Ok(mut cam) = q_camera.get_mut(rtt.camera) {
        cam.is_active = true;
    }

    // La brume et l'ambiante de l'univers vont SUR LA CAMÉRA (Components en
    // 0.18) : c'est ce qui fait que le fond a la couleur de l'arène, pas une
    // teinte inventée ici.
    let upm = units_per_meter();
    let density = render_cfg
        .as_deref()
        .map(|c| c.fog_density)
        .unwrap_or(ambiance.fog_density)
        / upm.max(1e-3);
    if let Ok(mut ec) = commands.get_entity(rtt.camera) {
        ec.insert((
            DistanceFog {
                color: Color::srgb(ambiance.fog_rgb[0], ambiance.fog_rgb[1], ambiance.fog_rgb[2]),
                directional_light_color: Color::srgb(
                    ambiance.fog_sun_rgb[0],
                    ambiance.fog_sun_rgb[1],
                    ambiance.fog_sun_rgb[2],
                ),
                directional_light_exponent: 30.0,
                falloff: FogFalloff::Exponential { density },
            },
            AmbientLight {
                color: Color::srgb(
                    ambiance.ambient_rgb[0],
                    ambiance.ambient_rgb[1],
                    ambiance.ambient_rgb[2],
                ),
                brightness: ambiance.ambient_brightness,
                ..default()
            },
        ));
    }
    info!(
        "[arena-backdrop] décor « {} » · palette « {} » · ambiance « {} » · {posed} props posés",
        decor_choisi.id, palette.name, ambiance.label
    );
}

/// Graine de composition d'un décor, dérivée de son id.
///
/// FNV-1a : stable d'un lancement à l'autre et d'une machine à l'autre (le
/// `DefaultHasher` de la std ne l'est ni l'un ni l'autre — un décor changerait
/// de forme au redémarrage).
fn seed_of(id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Pose le décor sous `root` et rend le nombre de props réellement posés.
///
/// Séparé du système pour rester lisible et pour que le comptage soit la valeur
/// de retour — un capteur qui compterait « à peu près » ne servirait à rien.
#[allow(clippy::too_many_arguments)]
fn build_diorama(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    root: Entity,
    palette: &DecorPalette,
    decor: &RogueliteDecorConfig,
    ambiance: &Ambiance,
    seed: u64,
) -> usize {
    let upm = units_per_meter();
    let mut rng = Rng::new(seed);
    let mut posed = 0usize;

    // ── Le sol ──
    // Un disque assez large pour dépasser du cadre, teinté de l'ambiante de
    // l'univers assombrie : il doit porter les props sans voler la vedette.
    let ground = materials.add(StandardMaterial {
        base_color: Color::srgb(
            ambiance.ambient_rgb[0] * 0.16,
            ambiance.ambient_rgb[1] * 0.16,
            ambiance.ambient_rgb[2] * 0.16,
        ),
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(RING_LANDMARK * 2.0))),
        MeshMaterial3d(ground),
        Transform::from_xyz(0.0, ground_y(), 0.0)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        RenderLayers::layer(ARENA_LAYER),
        ChildOf(root),
        Name::new("ArenaBackdropGround"),
    ));

    // ── Le soleil de l'univers ──
    // Sur ARENA_LAYER seulement : le personnage garde l'éclairage de son propre
    // aperçu, donc le portrait du Forgeron ne bouge pas d'un pixel.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(
                ambiance.fog_sun_rgb[0],
                ambiance.fog_sun_rgb[1],
                ambiance.fog_sun_rgb[2],
            ),
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.7, 0.8, 0.0)),
        RenderLayers::layer(ARENA_LAYER),
        ChildOf(root),
        Name::new("ArenaBackdropSun"),
    ));

    // ── Les trois plans de props ──
    // Tailles cibles = celles de la VRAIE arène, converties en unités.
    let plans: [(&Vec<String>, usize, f32, f32); 3] = [
        (
            &palette.landmarks,
            COUNT_LANDMARK,
            RING_LANDMARK,
            decor.target_landmark * upm,
        ),
        (&palette.big, COUNT_BIG, RING_BIG, decor.target_big * upm),
        (
            &palette.scatter,
            COUNT_SCATTER,
            RING_SCATTER,
            decor.target_scatter * upm,
        ),
    ];
    for (pool, count, ring, target) in plans {
        for i in 0..count {
            let Some(path) = rng.pick(pool) else {
                // Palette incomplète : on ne pose rien plutôt que d'inventer un
                // prop d'une autre DA. Le compte le dira.
                break;
            };
            let t = (i as f32 + 0.5) / count as f32;
            let angle = ARC_START + t * (ARC_END - ARC_START) + rng.range(-0.15, 0.15);
            let r = ring * rng.range(0.85, 1.15);
            posed += 1;
            spawn_backdrop_prop(commands, assets, root, path, angle, r, target, &mut rng);
        }
    }

    // ── Les braseros ──
    // Ils portent la lumière de l'univers ; c'est eux qui font « vivre » le fond.
    for i in 0..COUNT_BRAZIER {
        let Some(path) = rng.pick(&palette.braziers) else {
            break;
        };
        let t = (i as f32 + 0.5) / COUNT_BRAZIER as f32;
        let angle = ARC_START + t * (ARC_END - ARC_START) + rng.range(-0.2, 0.2);
        let r = RING_BIG * 0.75 * rng.range(0.9, 1.1);
        posed += 1;
        let e = spawn_backdrop_prop(
            commands,
            assets,
            root,
            path,
            angle,
            r,
            decor.target_brazier * upm,
            &mut rng,
        );
        // Le halo, à hauteur de flamme (haut du brasero).
        commands.spawn((
            PointLight {
                color: Color::srgb(
                    ambiance.fog_sun_rgb[0],
                    ambiance.fog_sun_rgb[1],
                    ambiance.fog_sun_rgb[2],
                ),
                intensity: 120_000.0,
                range: RING_BIG,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, decor.target_brazier * upm * 0.9, 0.0),
            RenderLayers::layer(ARENA_LAYER),
            ChildOf(e),
            Name::new("ArenaBackdropBrazierLight"),
        ));
    }

    posed
}

/// Pose un prop GLB sur l'arc, en attente de calibrage AABB.
#[allow(clippy::too_many_arguments)]
fn spawn_backdrop_prop(
    commands: &mut Commands,
    assets: &AssetServer,
    root: Entity,
    path: &str,
    angle: f32,
    radius: f32,
    target: f32,
    rng: &mut Rng,
) -> Entity {
    let pos = Vec3::new(
        angle.sin() * radius,
        ground_y(),
        // Négatif = derrière le personnage, vers le fond de l'image.
        -angle.cos() * radius,
    );
    // Le pivot porte la position et l'orientation ; l'échelle est appliquée à la
    // scène par le calibrage. Séparer les deux évite qu'un re-calibrage écrase
    // le placement.
    let pivot = commands
        .spawn((
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(rng.range(0.0, std::f32::consts::TAU))),
            Visibility::Inherited,
            RenderLayers::layer(ARENA_LAYER),
            ChildOf(root),
            Name::new("ArenaBackdropProp"),
        ))
        .id();
    commands.spawn((
        SceneRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.to_string()))),
        // Minuscule au départ : un GLB à sa taille native peut être énorme et
        // remplir l'écran le temps que le calibrage arrive.
        Transform::from_scale(Vec3::splat(0.001)),
        Visibility::Inherited,
        RenderLayers::layer(ARENA_LAYER),
        NeedsBackdropCalibrate { target },
        ChildOf(pivot),
        Name::new("ArenaBackdropPropScene"),
    ));
    pivot
}

/// Propage `RenderLayers` à tous les descendants du diorama : un `SceneRoot` GLB
/// ne le fait pas en 0.18, et un prop non marqué serait rendu par la caméra du
/// monde (donc invisible ici, et visible ailleurs).
fn sys_propagate_backdrop_layers(
    rtt: Option<Res<ArenaBackdropRtt>>,
    q_children: Query<&Children>,
    q_layers: Query<&RenderLayers>,
    mut commands: Commands,
) {
    let Some(rtt) = rtt else {
        return;
    };
    let target = RenderLayers::layer(ARENA_LAYER);
    let mut stack = vec![rtt.root];
    while let Some(e) = stack.pop() {
        if q_layers.get(e).map(|l| *l != target).unwrap_or(true) {
            commands.entity(e).insert(target.clone());
        }
        if let Ok(children) = q_children.get(e) {
            stack.extend(children.iter());
        }
    }
}

/// Calibrage AABB : chaque prop est mis à SA taille cible, posé sur le sol.
///
/// Les GLB des packs ont des tailles natives incohérentes (c'est pourquoi
/// l'arène calibre aussi) — un rocher « gros » peut sortir plus petit qu'un vase.
fn sys_calibrate_backdrop_props(
    q_need: Query<(Entity, &NeedsBackdropCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
    mut commands: Commands,
) {
    for (e, need) in &q_need {
        let Some((min, max)) = backdrop_aabb_bounds(e, &q_aabb, &q_children) else {
            continue;
        };
        let extent = (max - min).max_element().max(1e-3);
        let scale = need.target / extent;
        if let Ok(mut tf) = q_tf.get_mut(e) {
            // Centré en X/Z, POSÉ sur le sol en Y : un prop centré verticalement
            // flotterait à mi-hauteur.
            let center = (min + max) * 0.5;
            tf.translation = Vec3::new(-center.x * scale, -min.y * scale, -center.z * scale);
            tf.scale = Vec3::splat(scale);
        }
        commands.entity(e).remove::<NeedsBackdropCalibrate>();
    }
}

/// Dérive lente de la caméra — le fond respire sans jamais reprendre la main sur
/// la lecture de l'interface. Deux sinus de périodes premières entre elles.
///
/// `Time<Real>` : le menu ne doit pas se figer si le temps virtuel est en pause.
fn sys_drift_backdrop_camera(
    time: Res<Time<bevy::time::Real>>,
    rtt: Option<Res<ArenaBackdropRtt>>,
    q_win: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut q_tf: Query<&mut Transform>,
) {
    let Some(rtt) = rtt else {
        return;
    };
    let Ok(mut tf) = q_tf.get_mut(rtt.camera) else {
        return;
    };
    let aspect = window_aspect(q_win.single().ok());
    let t = time.elapsed_secs();
    let amp = visible_width(aspect) * DRIFT_AMPLITUDE;
    let dx = (t * std::f32::consts::TAU / DRIFT_PERIOD_X).sin() * amp;
    let dy = (t * std::f32::consts::TAU / DRIFT_PERIOD_Y).sin() * amp * 0.5;
    let base_x = -aim_offset_x(aspect);
    let eye_y = ground_y() + PREVIEW_TARGET * 0.62;
    tf.translation = Vec3::new(base_x + dx, eye_y + dy, camera_distance());
    // La visée suit le même décalage : le personnage garde son tiers pendant
    // que le décor défile derrière lui.
    tf.look_at(Vec3::new(base_x + dx, eye_y * 0.6, 0.0), Vec3::Y);
}

/// OnExit(Menu) — désenregistre l'image d'egui et détruit tout le diorama.
fn sys_despawn_arena_backdrop(
    mut commands: Commands,
    rtt: Option<Res<ArenaBackdropRtt>>,
    q: Query<Entity, With<BackdropEntity>>,
    mut contexts: EguiContexts,
) {
    if let Some(r) = rtt.as_ref() {
        contexts.remove_image(r.image.id());
    }
    for e in &q {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<ArenaBackdropRtt>();
}

/// `(min, max)` de l'AABB combinée des descendants, en espace local de `root`.
fn backdrop_aabb_bounds(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_fond_montre_le_decor_equipe_et_pas_la_progression() {
        let Some(cfg) = forgia_mode_roguelite::cosmetics::shipped_catalogue() else {
            eprintln!("génome absent (checkout partiel), contrôle non exécuté");
            return;
        };
        let mut identity = IdentitySave::default();
        let mut meta = MetaShopSave::default();

        // Rien de battu : le décor de départ, quoi qu'il arrive.
        meta.chapters_cleared = 0;
        let d = equipped_backdrop(&cfg, &identity, Some(&meta)).unwrap();
        assert_eq!(d.id, forgia_mode_roguelite::cosmetics::FALLBACK_BACKDROP);

        // Battre des chapitres ne CHANGE PAS le fond tout seul — c'est
        // précisément ce qui a été retiré le 2026-08-06.
        meta.chapters_cleared = 7;
        let d = equipped_backdrop(&cfg, &identity, Some(&meta)).unwrap();
        assert_eq!(d.id, forgia_mode_roguelite::cosmetics::FALLBACK_BACKDROP, "le fond ne suit plus la progression");

        // Équiper un décor gagné, en revanche, le change.
        identity.equipped_backdrop = "les_falaises".into(); // chapitre 7
        let d = equipped_backdrop(&cfg, &identity, Some(&meta)).unwrap();
        assert_eq!(d.id, "les_falaises");
    }

    #[test]
    fn une_sauvegarde_bricolee_n_affiche_pas_un_decor_non_gagne() {
        let Some(cfg) = forgia_mode_roguelite::cosmetics::shipped_catalogue() else {
            eprintln!("génome absent (checkout partiel), contrôle non exécuté");
            return;
        };
        let mut identity = IdentitySave::default();
        let meta = MetaShopSave::default(); // 0 chapitre battu
        // Un décor de chapitre lointain, jamais gagné, écrit à la main.
        identity.equipped_backdrop = "les_champs".into();
        let d = equipped_backdrop(&cfg, &identity, Some(&meta)).unwrap();
        assert_ne!(d.id, "les_champs");
        // Et un décor payant qu'on n'a pas acheté.
        identity.equipped_backdrop = "falaises_sous_la_glace".into();
        let d = equipped_backdrop(&cfg, &identity, Some(&meta)).unwrap();
        assert_ne!(d.id, "falaises_sous_la_glace");
    }

    #[test]
    fn deux_decors_de_meme_palette_ne_composent_pas_pareil() {
        // « La Forge » et « La Forge au crépuscule » partagent leurs props :
        // sans graine distincte, la variante achetée ne serait qu'un filtre de
        // couleur sur une image déjà vue.
        assert_ne!(seed_of("crypte_enclume"), seed_of("forge_crepuscule"));
        // La graine est ÉPINGLÉE, pas seulement reproductible dans la même
        // exécution : c'est ce qui distingue FNV-1a du `DefaultHasher` de la
        // std, dont la valeur change à chaque lancement. Si cette constante
        // saute, tous les décors changent de forme chez les joueurs.
        assert_eq!(seed_of("donjon_oublie"), 0x0e85_4040_3a67_a536);
    }

    #[test]
    fn le_diorama_se_mesure_en_hauteurs_de_personnage() {
        // L'étalon : le personnage calibré vaut bien un humain de 2 m.
        let upm = units_per_meter();
        assert!((upm * PLAYER_HEIGHT_M - PREVIEW_TARGET).abs() < 1e-6);
        // Un landmark de 16 m doit dominer le personnage sans sortir du cadre.
        let landmark = 16.0 * upm;
        assert!(
            landmark > PREVIEW_TARGET * 4.0,
            "un landmark doit écraser le personnage (obtenu {landmark})"
        );
    }

    #[test]
    fn le_personnage_tombe_dans_le_tiers_droit() {
        // Vrai pour TOUTE fenêtre supportée (story-692 : l'aspect vient de la
        // fenêtre) — du 4:3 à l'ultrawide 21:9.
        for aspect in [4.0 / 3.0, 16.0 / 10.0, BACKDROP_FALLBACK_ASPECT, 21.0 / 9.0] {
            // La visée est décalée à GAUCHE, donc le sujet à l'origine part à droite.
            assert!(aim_offset_x(aspect) > 0.0);
            // Et il reste DANS le cadre : son décalage est sous la demi-largeur.
            assert!(aim_offset_x(aspect) < visible_width(aspect) * 0.5);
        }
    }

    #[test]
    fn la_derive_ne_sort_jamais_le_personnage_de_son_tiers() {
        // Pire cas des deux sinus : l'amplitude cumulée doit rester petite
        // devant la marge qui sépare le personnage du bord — à tout aspect.
        for aspect in [4.0 / 3.0, BACKDROP_FALLBACK_ASPECT, 21.0 / 9.0] {
            let marge = visible_width(aspect) * 0.5 - aim_offset_x(aspect);
            let derive = visible_width(aspect) * DRIFT_AMPLITUDE;
            assert!(
                derive < marge,
                "la dérive ({derive}) déborderait la marge ({marge}) à l'aspect {aspect}"
            );
        }
    }

    #[test]
    fn la_meme_graine_donne_le_meme_diorama() {
        let tirage = |graine: u64| {
            let mut r = Rng::new(graine);
            (0..8).map(|_| r.unit()).collect::<Vec<_>>()
        };
        assert_eq!(tirage(3), tirage(3), "un décor doit être stable");
        assert_ne!(tirage(3), tirage(4), "deux décors, deux compositions");
    }

    #[test]
    fn une_palette_vide_ne_pose_rien_plutot_que_d_inventer() {
        let mut r = Rng::new(1);
        let vide: Vec<String> = Vec::new();
        assert!(r.pick(&vide).is_none());
        let plein = vec!["a.glb".to_string()];
        assert_eq!(r.pick(&plein), Some("a.glb"));
    }
}

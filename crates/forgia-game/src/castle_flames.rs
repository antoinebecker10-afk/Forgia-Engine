//! # castle_flames — bougies allumées et éclairage du Hall de Forgia
//!
//! Le pack Unity d'origine éclaire son château avec des **lumières** et des
//! **particules** — deux familles d'objets qui ne portent pas de mesh, donc
//! écartées par la reconstruction de la scène. Résultat : nos ~300 bougeoirs sont
//! posés partout, mèche nue, et l'intérieur n'est éclairé que par une ambiante
//! constante (montée à 900 pour compenser, ce qui aplatit tout le modelé).
//!
//! Ce module rend les bougies vivantes : une flamme émissive sur chaque bougie, et
//! une vraie lumière sur les plus proches du joueur.
//!
//! ## Deux décisions qui structurent le module
//!
//! **Les flammes sont des entités racines, pas des enfants des bougies.** Une
//! flamme enfant hériterait de l'échelle de son parent — or les pièces du pack ont
//! des échelles variées, y compris **miroir** (échelle négative) : la flamme
//! prendrait une taille arbitraire. En racine, sa taille est celle qu'on demande,
//! et son cycle de vie est explicite (elle est retirée quand son ancre disparaît
//! avec le déchargement de sa cellule).
//!
//! **Seules les N flammes les plus proches portent une lumière.** ~300 lumières
//! ponctuelles dépasseraient le budget du rendu groupé de Bevy. Le composant
//! `PointLight` est posé et retiré **uniquement aux transitions**, pas à chaque
//! frame : le joueur se déplaçant continûment, ça fait quelques commandes par
//! seconde.
//!
//! Réglages : `assets/genomes/castle_hub_lighting.toml`, relu à chaud (1 Hz).
//! Contexte complet : `docs/audits/audit-2026-07-26-comparaison-interieur-createur.md`.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use forgia_core::prelude::*;
use forgia_player::prelude::Player;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

const TUNING_PATH: &str = "assets/genomes/castle_hub_lighting.toml";
/// Lumières du créateur du pack, portées 1:1 depuis sa scène Unity (56 : 43
/// ponctuelles + 13 spots). Généré par `tools/unity/extract_scene_lights.py`.
const CREATOR_LIGHTS_PATH: &str = "assets/genomes/castle_hub_creator_lights.toml";
const SENSOR_PATH: &str = "forgia2_castle_flames.json";
const POLL_PERIOD_SECS: f32 = 1.0;
const SENSOR_PERIOD_SECS: f32 = 1.0;
/// Fréquence de re-sélection des flammes allumées. Inutile de reclasser 300
/// distances chaque frame : le joueur marche, il ne téléporte pas.
const CAP_PERIOD_SECS: f32 = 0.2;
/// Table des mèches par type, générée par `tools/gltf/extract_candle_mounts.py`.
///
/// 🚨 C'est **elle** qui décide où va une flamme, pas une règle sur les noms.
/// Deux tentatives précédentes ont échoué : poser la flamme au sommet de la boîte
/// englobante mettait une seule flamme au centre du métal d'un chandelier à trois
/// branches ; ne garder que les nœuds `candle_castle_*` supprimait toute flamme
/// autour de la Grande Salle, où il n'y a **que** des bougeoirs (la bougie la plus
/// proche du spawn est à 38,7 m, le bougeoir le plus proche à 8,1 m). Beaucoup de
/// bougeoirs ont leurs bougies **modelées dans le mesh** : les mèches se trouvent
/// donc dans la géométrie, pas dans la hiérarchie.
const CANDLE_MOUNTS_PATH: &str = "assets/genomes/castle_hub_candle_mounts.toml";
/// Frames d'attente accordées à l'instanciation de la scène glTF avant d'abandonner
/// une flamme : au moment où le nœud apparaît, sa géométrie n'existe pas encore,
/// donc sa boîte englobante — et donc le sommet de la bougie — est inconnue.
const MAX_PLACEMENT_ATTEMPTS: u8 = 120;
/// Élongation verticale de la flamme (une flamme est plus haute que large).
const FLAME_HEIGHT_RATIO: f32 = 1.7;
/// Garde-fou de parcours de hiérarchie.
const MAX_SUBTREE_NODES: usize = 512;

// ─── Réglages ────────────────────────────────────────────────────────────────

#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct CastleLighting {
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
    pub flames_enabled: bool,
    pub max_active_lights: usize,
    pub light_intensity_lm: f32,
    pub light_range_m: f32,
    pub light_color: [f32; 3],
    pub emissive_luminance: f32,
    pub size_m: f32,
    pub lift_m: f32,
    pub flicker_amplitude: f32,
    pub flicker_speed_hz: f32,

    /// Convertit l'intensité Unity (0,4 à 59 dans sa scène) en lumens Bevy.
    /// Un seul curseur pour caler les 56 lumières d'un coup, sans réencoder.
    pub creator_light_scale: f32,
    pub creator_lights_enabled: bool,
    /// Soleil principal, en lux (porte les ombres).
    pub key_lux: f32,
    /// Remplissage ciel, en lux. **Sans ombres** : il traverse murs et toit, donc
    /// il doit rester discret. À 7 000 (valeur d'origine) il éclairait l'intérieur
    /// comme s'il n'y avait pas de château.
    pub fill_lux: f32,

    /// Éclairage par image du Hall — cf `castle_envmap`. Sans lui, une surface PBR
    /// n'a rien à réfléchir et tombe sur un reflet plat (« pierre mouillée »).
    pub env_enabled: bool,
    /// Intensité de la cubemap d'ambiance, en cd/m². Les sondes du pack sont très
    /// sombres (luminance moyenne 0,008) : ce facteur les remonte au niveau du
    /// reste de l'éclairage.
    pub env_intensity: f32,

    /// Lumière cuite du créateur — cf `castle_lightmaps`. C'est elle qui apporte
    /// le rebond, absent de tout éclairage temps réel.
    pub lightmaps_enabled: bool,
    /// Sens vertical des UV de lightmap. Unity place l'origine d'une texture en
    /// bas à gauche, glTF en haut à gauche : impossible de trancher sans lancer
    /// le jeu. Si les lightmaps sortent retournées, c'est **cette** valeur.
    pub lightmaps_flip_v: bool,
    /// Gain appliqué à la lumière cuite (`StandardMaterial::lightmap_exposure`).
    ///
    /// 🚨 Ses valeurs cuites ont une **médiane de 0,05** ; l'ambiante Bevy qu'elles
    /// remplacent valait **400**. Les deux moteurs ne comptent pas dans la même
    /// unité, et à gain 1 (le défaut de Bevy) la lumière cuite ne pèse rien : le
    /// Hall devient noir. Ce facteur fait le pont.
    pub lightmaps_exposure: f32,
}

impl Default for CastleLighting {
    fn default() -> Self {
        Self {
            ambient_brightness: 250.0,
            ambient_color: [0.72, 0.80, 0.92],
            flames_enabled: true,
            max_active_lights: 24,
            light_intensity_lm: 2200.0,
            light_range_m: 7.0,
            light_color: [1.0, 0.63, 0.28],
            emissive_luminance: 9000.0,
            size_m: 0.022,
            lift_m: 0.012,
            flicker_amplitude: 0.22,
            flicker_speed_hz: 6.5,
            creator_light_scale: 900.0,
            creator_lights_enabled: true,
            key_lux: 12_000.0,
            fill_lux: 600.0,
            env_enabled: true,
            env_intensity: 900.0,
            lightmaps_enabled: true,
            lightmaps_flip_v: false,
            // 400 (l'ambiante remplacée) / 0,05 (la médiane cuite) = 8000.
            lightmaps_exposure: 8000.0,
        }
    }
}

impl CastleLighting {
    pub fn ambient(&self) -> AmbientLight {
        AmbientLight {
            color: Color::srgb(
                self.ambient_color[0],
                self.ambient_color[1],
                self.ambient_color[2],
            ),
            brightness: self.ambient_brightness.max(0.0),
            // 🚨 Le rebond est déjà dans les lightmaps. Laisser l'ambiante
            // s'ajouter par-dessus le compterait deux fois — et une ambiante
            // plate est justement ce que la lumière cuite remplace.
            affects_lightmapped_meshes: !self.lightmaps_enabled,
        }
    }

    fn emissive(&self) -> LinearRgba {
        let [r, g, b] = self.light_color;
        LinearRgba::rgb(r, g, b) * self.emissive_luminance.max(0.0)
    }

    /// Pur — testable sans app Bevy. Les champs absents gardent leur défaut.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: LightingToml = toml::from_str(content).unwrap_or_default();
        let base = Self::default();
        let ambient = parsed.ambient.unwrap_or_default();
        let flames = parsed.flames.unwrap_or_default();
        let creator = parsed.creator_lights.unwrap_or_default();
        let environment = parsed.environment.unwrap_or_default();
        let lightmaps = parsed.lightmaps.unwrap_or_default();
        Self {
            ambient_brightness: ambient.brightness.unwrap_or(base.ambient_brightness),
            ambient_color: ambient.color.unwrap_or(base.ambient_color),
            flames_enabled: flames.enabled.unwrap_or(base.flames_enabled),
            max_active_lights: flames.max_active_lights.unwrap_or(base.max_active_lights),
            light_intensity_lm: flames.light_intensity_lm.unwrap_or(base.light_intensity_lm),
            light_range_m: flames.light_range_m.unwrap_or(base.light_range_m),
            light_color: flames.light_color.unwrap_or(base.light_color),
            emissive_luminance: flames.emissive_luminance.unwrap_or(base.emissive_luminance),
            size_m: flames.size_m.unwrap_or(base.size_m),
            lift_m: flames.lift_m.unwrap_or(base.lift_m),
            flicker_amplitude: flames.flicker_amplitude.unwrap_or(base.flicker_amplitude),
            flicker_speed_hz: flames.flicker_speed_hz.unwrap_or(base.flicker_speed_hz),
            creator_light_scale: creator.scale.unwrap_or(base.creator_light_scale),
            creator_lights_enabled: creator.enabled.unwrap_or(base.creator_lights_enabled),
            key_lux: ambient.key_lux.unwrap_or(base.key_lux),
            fill_lux: ambient.fill_lux.unwrap_or(base.fill_lux),
            env_enabled: environment.enabled.unwrap_or(base.env_enabled),
            env_intensity: environment.intensity.unwrap_or(base.env_intensity),
            lightmaps_enabled: lightmaps.enabled.unwrap_or(base.lightmaps_enabled),
            lightmaps_flip_v: lightmaps.flip_v.unwrap_or(base.lightmaps_flip_v),
            lightmaps_exposure: lightmaps.exposure.unwrap_or(base.lightmaps_exposure),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(TUNING_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => {
                info!("[castle-flames] {TUNING_PATH} absent — réglages par défaut");
                Self::default()
            }
        }
    }
}

#[derive(Deserialize, Default)]
struct LightingToml {
    ambient: Option<AmbientToml>,
    flames: Option<FlamesToml>,
    creator_lights: Option<CreatorLightsToml>,
    environment: Option<EnvironmentToml>,
    lightmaps: Option<LightmapsToml>,
}

#[derive(Deserialize, Default)]
struct LightmapsToml {
    enabled: Option<bool>,
    flip_v: Option<bool>,
    exposure: Option<f32>,
}

#[derive(Deserialize, Default)]
struct EnvironmentToml {
    enabled: Option<bool>,
    intensity: Option<f32>,
}

#[derive(Deserialize, Default)]
struct CreatorLightsToml {
    enabled: Option<bool>,
    scale: Option<f32>,
}

#[derive(Deserialize, Default)]
struct AmbientToml {
    brightness: Option<f32>,
    color: Option<[f32; 3]>,
    key_lux: Option<f32>,
    fill_lux: Option<f32>,
}

#[derive(Deserialize, Default)]
struct FlamesToml {
    enabled: Option<bool>,
    max_active_lights: Option<usize>,
    light_intensity_lm: Option<f32>,
    light_range_m: Option<f32>,
    light_color: Option<[f32; 3]>,
    emissive_luminance: Option<f32>,
    size_m: Option<f32>,
    lift_m: Option<f32>,
    flicker_amplitude: Option<f32>,
    flicker_speed_hz: Option<f32>,
}

/// Suivi du fichier de réglages pour le rechargement à chaud.
#[derive(Resource)]
struct TuningWatch {
    next_poll: f32,
    last_modified: Option<SystemTime>,
    reloads: u32,
}

impl Default for TuningWatch {
    fn default() -> Self {
        Self {
            next_poll: 0.0,
            last_modified: fs::metadata(TUNING_PATH).and_then(|m| m.modified()).ok(),
            reloads: 0,
        }
    }
}

// ─── Composants ──────────────────────────────────────────────────────────────

/// Points de flamme d'un type d'objet, en coordonnées **locales** au nœud.
#[derive(Deserialize, Clone, Debug)]
struct CandleMount {
    #[serde(rename = "type")]
    node_type: String,
    points: Vec<[f32; 3]>,
}

#[derive(Deserialize, Default)]
struct CandleMountsFile {
    #[serde(default)]
    mount: Vec<CandleMount>,
}

/// Table chargée : quel type porte combien de mèches, et où.
#[derive(Resource, Default)]
struct CandleMounts(Vec<CandleMount>);

impl CandleMounts {
    /// Mèches d'un nœud, d'après son nom. `None` = pas de flamme sur cet objet.
    fn points_for(&self, node_name: &str) -> Option<&[[f32; 3]]> {
        self.0
            .iter()
            .find(|mount| node_name.contains(mount.node_type.as_str()))
            .map(|mount| mount.points.as_slice())
    }
}

fn load_candle_mounts(mut mounts: ResMut<CandleMounts>) {
    mounts.0.clear();
    let Ok(content) = fs::read_to_string(CANDLE_MOUNTS_PATH) else {
        warn!("[castle-flames] {CANDLE_MOUNTS_PATH} introuvable — aucune flamme");
        return;
    };
    match toml::from_str::<CandleMountsFile>(&content) {
        Ok(file) => {
            let wicks: usize = file.mount.iter().map(|m| m.points.len()).sum();
            info!(
                "[castle-flames] {} types de bougie, {wicks} mèches",
                file.mount.len()
            );
            mounts.0 = file.mount;
        }
        Err(error) => error!("[castle-flames] {CANDLE_MOUNTS_PATH} invalide : {error}"),
    }
}

/// Bougie repérée, en attente que sa géométrie existe pour placer la flamme.
#[derive(Component, Default)]
struct NeedsFlame {
    attempts: u8,
    /// Mèches locales de ce type, résolues au repérage.
    points: Vec<[f32; 3]>,
}

/// Une flamme. `anchor` est la bougie qui l'a fait naître : quand elle disparaît
/// (déchargement de cellule), la flamme suit.
#[derive(Component)]
struct CastleFlame {
    anchor: Entity,
    /// Décalage de phase du vacillement, pour que les flammes ne battent pas
    /// toutes ensemble. Dérivé de l'index de l'ancre — déterministe, pas aléatoire.
    phase: f32,
    /// La flamme porte-t-elle actuellement une lumière ?
    lit: bool,
}

/// Ressources partagées par toutes les flammes : un mesh, un matériau.
#[derive(Resource)]
struct FlameAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct FlameStats {
    total: u32,
    lit: u32,
}

// ─── Lumières du créateur ────────────────────────────────────────────────────

/// Une lumière portée depuis la scène Unity du créateur.
#[derive(Deserialize, Clone, Copy, Debug)]
struct CreatorLightToml {
    kind: CreatorLightKind,
    pos: [f32; 3],
    #[serde(default)]
    dir: Option<[f32; 3]>,
    #[serde(default)]
    outer_deg: Option<f32>,
    #[serde(default)]
    inner_deg: Option<f32>,
    intensity: f32,
    range: f32,
    color: [f32; 3],
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CreatorLightKind {
    Point,
    Spot,
}

#[derive(Deserialize, Default)]
struct CreatorLightsFile {
    #[serde(default)]
    light: Vec<CreatorLightToml>,
}

/// Marqueur des lumières portées (despawn en quittant le Hall).
#[derive(Component)]
struct CreatorLight;

#[derive(Resource, Default)]
struct CreatorLightCount(u32);

/// Instancie les 56 lumières placées à la main par le créateur du pack.
///
/// Leurs positions viennent de sa scène Unity, converties par une transformation
/// **déduite puis vérifiée** (écart médian 0,000 m sur 1 195 pièces appariées) —
/// pas devinée. Détail : `tools/unity/extract_scene_lights.py`.
///
/// Ombres portées désactivées : 56 lumières à ombres coûteraient bien plus que
/// ce qu'elles apporteraient, et sa scène s'appuyait de toute façon sur des
/// ombres **cuites** (shadowmasks) que nous n'avons pas.
fn sys_spawn_creator_lights(
    mut commands: Commands,
    tuning: Res<CastleLighting>,
    mut count: ResMut<CreatorLightCount>,
) {
    count.0 = 0;
    if !tuning.creator_lights_enabled {
        return;
    }
    let content = match fs::read_to_string(CREATOR_LIGHTS_PATH) {
        Ok(content) => content,
        Err(error) => {
            warn!("[castle-flames] {CREATOR_LIGHTS_PATH} illisible : {error}");
            return;
        }
    };
    let file: CreatorLightsFile = match toml::from_str(&content) {
        Ok(file) => file,
        Err(error) => {
            error!("[castle-flames] {CREATOR_LIGHTS_PATH} invalide : {error}");
            return;
        }
    };
    for light in &file.light {
        let color = Color::srgb(light.color[0], light.color[1], light.color[2]);
        let position = Vec3::from_array(light.pos);
        // L'intensité Unity n'est pas en lumens : un facteur unique la convertit,
        // réglable à chaud pour caler les 56 d'un coup.
        let intensity = light.intensity * tuning.creator_light_scale;
        match light.kind {
            CreatorLightKind::Point => {
                commands.spawn((
                    CreatorLight,
                    PointLight {
                        color,
                        intensity,
                        range: light.range,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(position),
                    Name::new("CastleCreatorLight"),
                ));
            }
            CreatorLightKind::Spot => {
                let direction = light
                    .dir
                    .map(Vec3::from_array)
                    .and_then(|d| Dir3::new(d).ok())
                    .unwrap_or(Dir3::NEG_Y);
                let outer = light.outer_deg.unwrap_or(60.0).to_radians() * 0.5;
                // Unity donne parfois un angle interne nul : on garde un cône
                // intérieur légèrement plus étroit, sinon le bord est dur.
                let inner = light
                    .inner_deg
                    .filter(|a| *a > 0.0)
                    .map(|a| a.to_radians() * 0.5)
                    .unwrap_or(outer * 0.8)
                    .min(outer);
                commands.spawn((
                    CreatorLight,
                    SpotLight {
                        color,
                        intensity,
                        range: light.range,
                        outer_angle: outer,
                        inner_angle: inner,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(position).looking_to(direction, Vec3::Y),
                    Name::new("CastleCreatorSpot"),
                ));
            }
        }
        count.0 += 1;
    }
    info!("[castle-flames] {} lumières du créateur instanciées", count.0);
}

fn sys_cleanup_creator_lights(
    mut commands: Commands,
    q: Query<Entity, With<CreatorLight>>,
    mut count: ResMut<CreatorLightCount>,
) {
    for entity in &q {
        commands.entity(entity).despawn();
    }
    count.0 = 0;
}

/// Applique les intensités de soleil réglées à chaud.
///
/// Le remplissage est le point sensible : il n'a **pas d'ombres**, donc il
/// traverse murs et toit. À 7 000 lux il éclairait l'intérieur comme si le
/// château n'existait pas — deux murs identiques côte à côte pouvaient être l'un
/// blanc éclatant et l'autre noir selon leur orientation. Il doit rester discret.
fn sys_apply_sun(
    tuning: Res<CastleLighting>,
    mut q_key: Query<
        &mut DirectionalLight,
        (With<crate::castle_hub::CastleKeyLight>, Without<crate::castle_hub::CastleFillLight>),
    >,
    mut q_fill: Query<
        &mut DirectionalLight,
        (With<crate::castle_hub::CastleFillLight>, Without<crate::castle_hub::CastleKeyLight>),
    >,
) {
    if !tuning.is_changed() {
        return;
    }
    // 🚨 Son soleil est **cuit** : chez lui, la lumière directionnelle n'existe
    // pas au runtime, elle est déjà dans les lightmaps. Quand les nôtres sont
    // posées, nos directionnelles ne doivent donc plus éclairer la géométrie
    // cuite — sinon le soleil compte deux fois et l'intérieur se délave. Elles
    // restent utiles pour ce qui n'est pas cuit : le joueur, les ennemis, les
    // pièces réimportées.
    let affects_baked = !tuning.lightmaps_enabled;
    for mut light in &mut q_key {
        light.illuminance = tuning.key_lux.max(0.0);
        light.affects_lightmapped_mesh_diffuse = affects_baked;
    }
    for mut light in &mut q_fill {
        light.illuminance = tuning.fill_lux.max(0.0);
        light.affects_lightmapped_mesh_diffuse = affects_baked;
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct CastleFlamesPlugin;

impl Plugin for CastleFlamesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CastleLighting::load_or_default())
            .init_resource::<TuningWatch>()
            .init_resource::<FlameStats>()
            .init_resource::<CreatorLightCount>()
            .init_resource::<CandleMounts>()
            .add_systems(
                OnEnter(GameMode::CastleHub),
                (
                    load_candle_mounts,
                    sys_prepare_flame_assets,
                    sys_spawn_creator_lights,
                ),
            )
            .add_systems(
                OnExit(GameMode::CastleHub),
                (sys_cleanup_flames, sys_cleanup_creator_lights),
            )
            .add_systems(
                Update,
                (
                    sys_poll_tuning,
                    sys_apply_ambient,
                    sys_apply_sun,
                    sys_mark_candles,
                    sys_place_flames,
                    sys_drop_orphan_flames,
                    sys_cap_active_lights,
                    sys_flicker,
                )
                    .chain()
                    .run_if(in_state(GameMode::CastleHub)),
            )
            .add_systems(Update, sys_write_sensor.in_set(GameSet::Sensors));
    }
}

/// Crée le mesh et le matériau partagés (une flamme = une goutte émissive).
fn sys_prepare_flame_assets(
    mut commands: Commands,
    tuning: Res<CastleLighting>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Sphère basse résolution : une flamme fait 2 cm à l'écran, sa silhouette
    // exacte n'a aucune importance — son émission, si.
    let mesh = meshes.add(Sphere::new(1.0).mesh().uv(8, 6));
    let material = materials.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: tuning.emissive(),
        // Une flamme n'est pas une surface réfléchissante : pas de spéculaire
        // parasite, et elle reste visible quel que soit l'éclairage autour.
        perceptual_roughness: 1.0,
        ..default()
    });
    commands.insert_resource(FlameAssets { mesh, material });
}

/// Relit le fichier de réglages quand il change sur le disque.
fn sys_poll_tuning(
    time: Res<Time<Real>>,
    mut watch: ResMut<TuningWatch>,
    mut tuning: ResMut<CastleLighting>,
    assets: Option<Res<FlameAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    if now < watch.next_poll {
        return;
    }
    watch.next_poll = now + POLL_PERIOD_SECS;

    let modified = fs::metadata(TUNING_PATH).and_then(|m| m.modified()).ok();
    if modified == watch.last_modified {
        return;
    }
    watch.last_modified = modified;
    let fresh = CastleLighting::load_or_default();
    if fresh == *tuning {
        return;
    }
    *tuning = fresh;
    watch.reloads += 1;
    // Le matériau est partagé : une seule écriture met à jour toutes les flammes.
    if let Some(assets) = assets {
        if let Some(material) = materials.get_mut(&assets.material) {
            material.emissive = tuning.emissive();
        }
    }
    info!("[castle-flames] réglages rechargés ({} fois)", watch.reloads);
}

/// Applique l'ambiante des réglages aux caméras, et la met à jour à chaud.
fn sys_apply_ambient(tuning: Res<CastleLighting>, mut q_ambient: Query<&mut AmbientLight>) {
    if !tuning.is_changed() {
        return;
    }
    let wanted = tuning.ambient();
    for mut ambient in &mut q_ambient {
        ambient.color = wanted.color;
        ambient.brightness = wanted.brightness;
    }
}

/// Repère les bougies dès que leur nœud apparaît (le streaming instancie les
/// cellules en continu, il n'y a pas de moment « tout est chargé »).
fn sys_mark_candles(
    mut commands: Commands,
    tuning: Res<CastleLighting>,
    mounts: Res<CandleMounts>,
    q_new: Query<(Entity, &Name), Added<Name>>,
) {
    if !tuning.flames_enabled {
        return;
    }
    for (entity, name) in &q_new {
        if let Some(points) = mounts.points_for(name.as_str()) {
            commands.entity(entity).insert(NeedsFlame {
                attempts: 0,
                points: points.to_vec(),
            });
        }
    }
}

/// Place les flammes en attente, au sommet de la bougie.
#[allow(clippy::too_many_arguments)]
fn sys_place_flames(
    mut commands: Commands,
    tuning: Res<CastleLighting>,
    assets: Option<Res<FlameAssets>>,
    mut q_pending: Query<(Entity, &mut NeedsFlame)>,
    q_children: Query<&Children>,
    q_shape: Query<(&GlobalTransform, &Aabb)>,
    q_global: Query<&GlobalTransform>,
    mut scratch: Local<Vec<Entity>>,
    mut stats: ResMut<FlameStats>,
) {
    let Some(assets) = assets else {
        return;
    };
    if q_pending.is_empty() {
        return;
    }
    for (entity, mut pending) in &mut q_pending {
        // La boîte englobante ne sert plus qu'à savoir si la scène glTF est
        // instanciée : sans géométrie, la transformation du nœud n'est pas encore
        // propagée et la flamme atterrirait à l'origine du monde.
        let Some(_) = world_bounds(entity, &q_children, &q_shape, &mut scratch) else {
            // Géométrie pas encore instanciée : on retente.
            pending.attempts = pending.attempts.saturating_add(1);
            if pending.attempts >= MAX_PLACEMENT_ATTEMPTS {
                commands.entity(entity).remove::<NeedsFlame>();
            }
            continue;
        };
        commands.entity(entity).remove::<NeedsFlame>();

        // Une flamme par mèche détectée dans la géométrie. Un chandelier à trois
        // branches en reçoit trois, chacune au-dessus de SA bougie — la boîte
        // englobante, elle, n'aurait donné qu'un point au centre du métal.
        let Ok(global) = q_global.get(entity) else {
            continue;
        };
        let scale = Vec3::new(tuning.size_m, tuning.size_m * FLAME_HEIGHT_RATIO, tuning.size_m);
        for (index, point) in pending.points.iter().enumerate() {
            let position = global.transform_point(Vec3::from_array(*point));
            commands.spawn((
                CastleFlame {
                    anchor: entity,
                    // Phase dérivée de l'ancre ET du rang de la mèche : les trois
                    // bougies d'un même chandelier ne vacillent pas ensemble.
                    phase: ((entity.to_bits() + index as u64) % 64) as f32 * 0.098,
                    lit: false,
                },
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::from_translation(position).with_scale(scale),
                Name::new("CastleFlame"),
            ));
            stats.total = stats.total.saturating_add(1);
        }
    }
}

/// Retire les flammes dont la bougie a disparu (cellule déchargée).
fn sys_drop_orphan_flames(
    mut commands: Commands,
    q_flames: Query<(Entity, &CastleFlame)>,
    q_alive: Query<Entity>,
    mut stats: ResMut<FlameStats>,
) {
    for (entity, flame) in &q_flames {
        if !q_alive.contains(flame.anchor) {
            commands.entity(entity).despawn();
            stats.total = stats.total.saturating_sub(1);
        }
    }
}

/// N'allume que les flammes les plus proches du joueur.
fn sys_cap_active_lights(
    time: Res<Time<Real>>,
    mut next_at: Local<f32>,
    tuning: Res<CastleLighting>,
    q_player: Query<&GlobalTransform, With<Player>>,
    mut q_flames: Query<(Entity, &GlobalTransform, &mut CastleFlame)>,
    mut distances: Local<Vec<f32>>,
    mut commands: Commands,
    mut stats: ResMut<FlameStats>,
) {
    let now = time.elapsed_secs();
    if now < *next_at {
        return;
    }
    *next_at = now + CAP_PERIOD_SECS;

    let Ok(player) = q_player.single() else {
        return;
    };
    let origin = player.translation();

    // Seuil de distance de la N-ième flamme la plus proche. Passer par un seuil
    // évite d'allouer un ensemble d'entités : on reclasse des scalaires.
    distances.clear();
    for (_, transform, _) in &q_flames {
        distances.push(origin.distance_squared(transform.translation()));
    }
    let budget = tuning.max_active_lights.max(1);
    let threshold = if distances.len() <= budget {
        f32::INFINITY
    } else {
        distances.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        distances[budget - 1]
    };

    let light = PointLight {
        color: Color::srgb(
            tuning.light_color[0],
            tuning.light_color[1],
            tuning.light_color[2],
        ),
        intensity: tuning.light_intensity_lm,
        range: tuning.light_range_m,
        // Une bougie ne projette pas d'ombre : ~24 lumières à ombres portées
        // coûteraient bien plus que ce que l'effet apporte.
        shadows_enabled: false,
        ..default()
    };

    let mut lit = 0u32;
    for (entity, transform, mut flame) in &mut q_flames {
        let wanted = origin.distance_squared(transform.translation()) <= threshold;
        if wanted {
            lit += 1;
        }
        // Commande seulement aux transitions : le joueur marche, il ne téléporte pas.
        if wanted != flame.lit {
            if wanted {
                commands.entity(entity).insert(light);
            } else {
                commands.entity(entity).remove::<PointLight>();
            }
            flame.lit = wanted;
        }
    }
    stats.lit = lit;
}

/// Fait vaciller les flammes allumées.
fn sys_flicker(
    time: Res<Time<Real>>,
    tuning: Res<CastleLighting>,
    mut q_lit: Query<(&CastleFlame, &mut PointLight)>,
) {
    if tuning.flicker_amplitude <= 0.0 {
        return;
    }
    let t = time.elapsed_secs() * tuning.flicker_speed_hz;
    for (flame, mut light) in &mut q_lit {
        let wave = (t + flame.phase * std::f32::consts::TAU).sin();
        light.intensity = tuning.light_intensity_lm * (1.0 + tuning.flicker_amplitude * wave);
    }
}

/// Retire toutes les flammes en quittant le Hall.
fn sys_cleanup_flames(
    mut commands: Commands,
    q_flames: Query<Entity, With<CastleFlame>>,
    q_pending: Query<Entity, With<NeedsFlame>>,
    mut stats: ResMut<FlameStats>,
) {
    for entity in &q_flames {
        commands.entity(entity).despawn();
    }
    for entity in &q_pending {
        commands.entity(entity).remove::<NeedsFlame>();
    }
    *stats = FlameStats::default();
}

/// Boîte englobante monde d'un sous-arbre : centre + demi-dimensions.
fn world_bounds(
    root: Entity,
    q_children: &Query<&Children>,
    q_shape: &Query<(&GlobalTransform, &Aabb)>,
    scratch: &mut Vec<Entity>,
) -> Option<(Vec3, Vec3)> {
    scratch.clear();
    scratch.push(root);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;
    let mut cursor = 0;

    while cursor < scratch.len() && scratch.len() < MAX_SUBTREE_NODES {
        let entity = scratch[cursor];
        cursor += 1;
        if let Ok((transform, aabb)) = q_shape.get(entity) {
            let affine = transform.affine();
            let local_min = Vec3::from(aabb.min());
            let local_max = Vec3::from(aabb.max());
            for corner in 0..8 {
                let point = affine.transform_point3(Vec3::new(
                    if corner & 1 == 0 { local_min.x } else { local_max.x },
                    if corner & 2 == 0 { local_min.y } else { local_max.y },
                    if corner & 4 == 0 { local_min.z } else { local_max.z },
                ));
                min = min.min(point);
                max = max.max(point);
            }
            found = true;
        }
        if let Ok(children) = q_children.get(entity) {
            scratch.extend(children.iter());
        }
    }
    if !found {
        return None;
    }
    Some(((min + max) * 0.5, (max - min) * 0.5))
}

fn sys_write_sensor(
    time: Res<Time<Real>>,
    mut next_write: Local<f32>,
    game_mode: Res<State<GameMode>>,
    tuning: Res<CastleLighting>,
    watch: Res<TuningWatch>,
    stats: Res<FlameStats>,
    creator: Res<CreatorLightCount>,
    q_pending: Query<(), With<NeedsFlame>>,
) {
    let now = time.elapsed_secs();
    if now < *next_write {
        return;
    }
    *next_write = now + SENSOR_PERIOD_SECS;

    let in_hub = matches!(game_mode.get(), GameMode::CastleHub);
    let pending = q_pending.iter().count() as u32;
    let (severity, next_step) =
        severity_for_flames(in_hub, tuning.flames_enabled, stats.total, pending);
    let json = format!(
        r#"{{"id":"castle_flames","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.1},"in_hub":{in_hub},"enabled":{},"flames":{},"lit":{},"pending":{pending},"max_active_lights":{},"ambient_brightness":{:.0},"creator_lights":{},"reloads":{}}}"#,
        tuning.flames_enabled,
        stats.total,
        stats.lit,
        tuning.max_active_lights,
        tuning.ambient_brightness,
        creator.0,
        watch.reloads,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Sévérité + action de remédiation. Extraite pour être testable sans app Bevy.
fn severity_for_flames(
    in_hub: bool,
    enabled: bool,
    flames: u32,
    pending: u32,
) -> (&'static str, &'static str) {
    if !in_hub || !enabled {
        return ("ok", "-");
    }
    if flames == 0 && pending == 0 {
        return (
            "critical",
            "aucune flamme posee dans le Hall : verifier que les noeuds bougies portent bien '_candle' dans les cellules",
        );
    }
    if pending > 0 && flames == 0 {
        return (
            "warn",
            "flammes en attente de geometrie : normal quelques frames apres l'entree, anormal si persistant",
        );
    }
    ("ok", "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_survive_an_empty_file() {
        let parsed = CastleLighting::parse_toml("");
        assert_eq!(parsed, CastleLighting::default());
    }

    #[test]
    fn partial_toml_keeps_other_defaults() {
        let parsed = CastleLighting::parse_toml("[ambient]\nbrightness = 42.0\n");
        assert_eq!(parsed.ambient_brightness, 42.0);
        assert_eq!(parsed.max_active_lights, CastleLighting::default().max_active_lights);
    }

    #[test]
    fn broken_toml_falls_back_instead_of_panicking() {
        let parsed = CastleLighting::parse_toml("ceci n'est pas du toml {{{");
        assert_eq!(parsed, CastleLighting::default());
    }

    #[test]
    fn shipped_tuning_file_parses() {
        let content = include_str!("../../../assets/genomes/castle_hub_lighting.toml");
        let parsed = CastleLighting::parse_toml(content);
        // Le fichier livré baisse volontairement l'ambiante par rapport aux 900
        // historiques : c'est le sens du correctif.
        assert!(parsed.ambient_brightness < 900.0);
        assert!(parsed.flames_enabled);
        assert!(parsed.max_active_lights > 0);
    }

    #[test]
    fn ambient_brightness_never_goes_negative() {
        let parsed = CastleLighting::parse_toml("[ambient]\nbrightness = -50.0\n");
        assert_eq!(parsed.ambient().brightness, 0.0);
    }

    /// Charge la table livrée telle que le jeu la lit.
    fn shipped_mounts() -> CandleMounts {
        let file: CandleMountsFile = toml::from_str(include_str!(
            "../../../assets/genomes/castle_hub_candle_mounts.toml"
        ))
        .expect("table de mèches valide");
        CandleMounts(file.mount)
    }

    /// 🚨 Le point qui a cassé deux fois. Autour de la Grande Salle il n'y a QUE
    /// des bougeoirs (bougie la plus proche du spawn : 38,7 m ; bougeoir : 8,1 m).
    /// Si les bougeoirs ne portent pas de flamme, le joueur n'en voit aucune.
    #[test]
    fn holders_carry_flames_or_the_great_hall_stays_dark() {
        let mounts = shipped_mounts();
        let sconce = mounts
            .points_for("SM_PROP_candleholder_castle_01_LOD0")
            .expect("une applique murale porte une flamme");
        assert_eq!(sconce.len(), 1);
    }

    /// Un chandelier à branches reçoit une flamme PAR bougie, pas une au centre.
    #[test]
    fn candelabra_gets_one_flame_per_arm() {
        let mounts = shipped_mounts();
        let three = mounts
            .points_for("SM_PROP_candleholder_castle_03_LOD0")
            .expect("connu");
        let five = mounts
            .points_for("SM_PROP_candleholder_castle_04_LOD0")
            .expect("connu");
        assert_eq!(three.len(), 3, "le chandelier à trois branches a trois mèches");
        assert_eq!(five.len(), 5);
        // Les mèches sont bien réparties, pas empilées sur un même axe.
        let spread = three
            .iter()
            .flat_map(|a| three.iter().map(move |b| (a[0] - b[0]).hypot(a[2] - b[2])))
            .fold(0.0f32, f32::max);
        assert!(spread > 0.1, "mèches trop rapprochées : {spread} m");
    }

    #[test]
    fn a_simple_candle_has_exactly_one_wick_and_a_vase_none() {
        let mounts = shipped_mounts();
        assert_eq!(
            mounts.points_for("SM_PROP_candle_castle_05_LOD0").map(<[_]>::len),
            Some(1)
        );
        assert!(mounts.points_for("SM_PROP_vase_castle_01_LOD0").is_none());
    }

    /// La flamme se pose au-dessus de la cire, jamais dedans ni sous l'objet.
    #[test]
    fn wicks_sit_above_the_candle_base() {
        for mount in shipped_mounts().0 {
            for point in mount.points {
                assert!(point[1] > 0.0, "{} : mèche à Y={}", mount.node_type, point[1]);
            }
        }
    }

    #[test]
    fn missing_flames_in_hub_is_critical() {
        assert_eq!(severity_for_flames(true, true, 0, 0).0, "critical");
        assert_eq!(severity_for_flames(true, true, 0, 12).0, "warn");
        assert_eq!(severity_for_flames(true, true, 300, 0).0, "ok");
        assert_eq!(severity_for_flames(false, true, 0, 0).0, "ok");
        assert_eq!(severity_for_flames(true, false, 0, 0).0, "ok");
    }
}

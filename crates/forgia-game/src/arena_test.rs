//! arena_test.rs — Banc de blockout d'arène (story-667, 2026-07-27).
//!
//! **L'étape « greybox » du process de level design**, isolée du Roguelite pour
//! ne rien casser de ce qui tourne. On construit la FORME de l'arène en
//! primitives grises, on la joue, on l'itère — et on ne l'habille qu'une fois
//! qu'elle est bonne. C'est l'ordre suivi par tous les studios : Valve, id,
//! Bungie, Blizzard. Inverser coûte des mois.
//!
//! - Entrée : `FORGIA_BOOT_MODE=arena_test` → `GameMode::ArenaTest`. L'onglet de
//!   menu a été retiré le 2026-07-30 (temporaire) ; le mode, lui, est intact.
//! - Source : `assets/genomes/arena_test_crypte_vertical.toml` — **toute** la
//!   géométrie y vit.
//!   Le Rust ne contient pas une seule coordonnée d'arène.
//! - Hot-reload : sauvegarder le TOML reconstruit l'arène à la volée (poll mtime
//!   1 Hz). C'est ce qui rend l'itération viable — sans ça, 3 essais par heure.
//! - F2 : grille au sol · F4 : encart metrics · tir autorisé (cf `forgia-fps`).
//!
//! ## Le contrat de vérité des metrics
//!
//! Le TOML déclare les metrics joueur (hauteur de saut, portée…). Ces valeurs
//! sont **recalculées au chargement** depuis `player_movement.toml` et comparées.
//! Un écart lève une alerte `warn` dans `forgia2_arena_test.json` : une arène
//! construite sur des metrics périmées est une arène fausse, et le capteur le
//! dit au lieu de laisser le level design dériver en silence.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use bevy_rapier3d::prelude::{Collider, RigidBody};
use forgia_core::prelude::*;
use forgia_player::prelude::Player;
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// Scénario de travail courant et unique de l'Arena Test.
const GENOME_PATH: &str = "assets/genomes/arena_test_crypte_vertical.toml";
/// Metrics de locomotion — la source de vérité contre laquelle on recalcule.
const MOVEMENT_GENOME_PATH: &str = "assets/genomes/player_movement.toml";
const SENSOR_PATH: &str = "forgia2_arena_test.json";
const POLL_PERIOD_SEC: f32 = 1.0;
const SENSOR_PERIOD_SEC: f32 = 1.0;
/// Épaisseur des dalles de rampe (m). Purement constructif : la rampe est un
/// pavé incliné dont la FACE SUPÉRIEURE passe par `from`/`to`.
const RAMP_SLAB_THICKNESS_M: f32 = 0.5;
/// Fraction de la demi-largeur de route qui doit rester dégagée.
///
/// Reprend la règle sourcée « une porte fait 62 à 88 % de la largeur du couloir » :
/// on vérifie un canal utile, pas la largeur pleine. À 1,0, ce sont les murs qui
/// DÉFINISSENT le couloir qui sonneraient, puisqu'ils se trouvent exactement à
/// une demi-largeur de l'axe.
const USABLE_CHANNEL_RATIO: f32 = 0.8;
/// Épaisseur des dalles de plafond (m). Constructif, comme les rampes.
const CEILING_SLAB_THICKNESS_M: f32 = 0.5;
/// Plafond d'intensité d'une ponctuelle (lm). Au-delà, l'atmosphère et le bloom
/// sont écrasés — même garde-fou que `forgia-stage` (dette 2026-05-18).
const MAX_POINT_LIGHT_LUMENS: f32 = 8_000.0;
/// Les transitions de route primaire restent confortables à la manette comme
/// au clavier ; à 25° la pente est lisible sans devenir un obstacle de contrôle.
const MAX_COMFORTABLE_RAMP_SLOPE_DEG: f32 = 25.0;
/// Tolérance relative avant de signaler une dérive metrics déclarées/calculées.
const METRICS_DRIFT_TOLERANCE: f32 = 0.02;

// ─── Genome (Deserialize) ────────────────────────────────────────────────────

/// Metrics joueur. Déclarées ici, vérifiées contre `player_movement.toml`.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct Metrics {
    pub player_height_m: f32,
    pub player_radius_m: f32,
    /// Hauteur de l'œil au-dessus des pieds. C'est LE seuil de couverture : sans
    /// accroupissement, un obstacle ne casse une ligne de vue qu'au-dessus.
    pub eye_height_m: f32,
    pub walk_speed_ms: f32,
    pub sprint_speed_ms: f32,
    pub jump_velocity_ms: f32,
    pub gravity_ms2: f32,
    pub jump_height_m: f32,
    pub air_time_s: f32,
    pub jump_reach_walk_m: f32,
    pub jump_reach_sprint_m: f32,
    pub engagement_close_m: f32,
    pub engagement_core_m: f32,
    pub engagement_sniper_m: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            player_height_m: 2.0,
            player_radius_m: 0.3,
            eye_height_m: 1.7,
            walk_speed_ms: 6.5,
            sprint_speed_ms: 9.75,
            jump_velocity_ms: 6.5,
            gravity_ms2: 18.0,
            jump_height_m: 1.174,
            air_time_s: 0.722,
            jump_reach_walk_m: 4.69,
            jump_reach_sprint_m: 7.04,
            engagement_close_m: 12.0,
            engagement_core_m: 30.0,
            engagement_sniper_m: 50.0,
        }
    }
}

/// Metrics recalculées depuis la locomotion réelle — l'arbitre.
#[derive(Clone, Copy, Debug, Default)]
pub struct DerivedMetrics {
    pub jump_height_m: f32,
    pub air_time_s: f32,
    pub jump_reach_walk_m: f32,
    pub jump_reach_sprint_m: f32,
}

/// Recalcule les dérivées d'un tir balistique simple : montée jusqu'à v=0 sous
/// accélération constante. Pur, donc testable sans moteur.
pub fn derive_metrics(
    walk_speed: f32,
    sprint_mul: f32,
    jump_velocity: f32,
    gravity: f32,
) -> DerivedMetrics {
    if gravity <= 0.0 {
        return DerivedMetrics::default();
    }
    let jump_height_m = jump_velocity * jump_velocity / (2.0 * gravity);
    let air_time_s = 2.0 * jump_velocity / gravity;
    DerivedMetrics {
        jump_height_m,
        air_time_s,
        jump_reach_walk_m: walk_speed * air_time_s,
        jump_reach_sprint_m: walk_speed * sprint_mul * air_time_s,
    }
}

// ─── Géométrie de domination (la math du point haut) ────────────────────────
//
// Le résultat le plus utile de la littérature de level design n'est pas une
// liste de bonnes pratiques mais une inégalité. Un tireur en hauteur voit
// par-dessus les couvertures, et la hauteur d'obstacle nécessaire pour lui
// couper la vue croît LINÉAIREMENT avec sa propre altitude.
//
// Tireur : pieds à `shooter_h`, œil à `shooter_h + eye`.
// Cible   : pieds à 0, tête à `eye`, à distance horizontale `span`.
// Le rayon de visée vaut, à la distance horizontale `x` du tireur :
//     y(x) = shooter_h + eye − (shooter_h / span)·x
// Un obstacle en `x` coupe la vue si sa hauteur atteint ce rayon, d'où :
//
//     c ≥ shooter_h · (1 − x/span) + eye
//
// Conséquence directe, et c'est elle qui condamne un perchoir mal placé : à
// mi-portée (x = span/2) il faut c ≥ shooter_h/2 + eye. Une couverture de 2 m
// (œil à 1,7) ne contre donc qu'un tireur perché à 0,6 m. Le point haut ne se
// conteste pas avec des caisses — il se conteste avec de l'architecture, ou en
// abaissant le point haut.

/// Hauteur minimale d'un obstacle pour couper la ligne de vue d'un tireur perché.
///
/// `span` = distance horizontale tireur→cible, `dist` = distance tireur→obstacle.
/// Retourne `f32::INFINITY` si la portée est nulle (pas de ligne de vue à couper).
pub fn cover_height_to_break_sight(shooter_h: f32, span: f32, dist: f32, eye: f32) -> f32 {
    if span <= 0.0 {
        return f32::INFINITY;
    }
    shooter_h * (1.0 - dist / span) + eye
}

/// Altitude maximale d'un point haut encore contestable par une couverture de
/// hauteur `cover_h` placée à mi-portée. Inverse de la relation ci-dessus :
/// `h ≤ 2·(cover_h − eye)`.
///
/// Sert à dimensionner un perchoir À PARTIR de la couverture disponible, plutôt
/// que de poser un perchoir puis de constater qu'il domine tout.
pub fn max_contestable_height(cover_h: f32, eye: f32) -> f32 {
    (2.0 * (cover_h - eye)).max(0.0)
}

/// Un occultant vu d'en haut : centre au sol, hauteur, demi-largeur.
///
/// **Toute** géométrie solide en est un — pas seulement les pièces marquées
/// « couverture ». Une plateforme de 3 m coupe une ligne de vue exactement comme
/// un muret ; ne compter que les couvertures surestimerait la domination.
pub type CoverDisc = (Vec3, f32, f32);

impl ArenaCfg {
    /// Demi-emprise jouable selon la forme. Source unique pour l'enceinte, le
    /// contrôle de couverture du sol et l'échantillonnage de l'analyse.
    pub fn half_extents(&self) -> Vec2 {
        if self.shape == "hexagonal" {
            Vec2::splat(self.extent_m)
        } else {
            Vec2::new(self.size_x * 0.5, self.size_z * 0.5)
        }
    }
}

/// Fraction [0,1] du sol praticable qu'un tireur posé en `shooter` domine.
/// 1.0 = poste de tir absolu, rien n'est à l'abri.
///
/// Échantillonnage régulier du rectangle `±half`, occultants traités en cylindres
/// verticaux (approximation assumée : en blockout on mesure un ordre de grandeur,
/// pas une silhouette). Pur, donc testable sans moteur.
///
/// `z_range` borne l'échantillonnage à une bande — c'est ainsi qu'on mesure la
/// domination d'une position de force **sur sa propre voie**, la seule question
/// qui ait un sens sur une carte à trois voies.
pub fn overlook_fraction(
    shooter: Vec3,
    eye: f32,
    covers: &[CoverDisc],
    region: (Vec2, Vec2),
    samples: u32,
) -> f32 {
    let (lo, hi) = region;
    if hi.x <= lo.x || hi.y <= lo.y || samples == 0 {
        return 0.0;
    }
    let eye_pos = Vec3::new(shooter.x, shooter.y + eye, shooter.z);
    let mut seen = 0u32;
    let mut total = 0u32;
    for ix in 0..samples {
        let x = lo.x + (hi.x - lo.x) * (ix as f32 + 0.5) / samples as f32;
        for iz in 0..samples {
            let z = lo.y + (hi.y - lo.y) * (iz as f32 + 0.5) / samples as f32;
            let ground = Vec3::new(x, 0.0, z);
            // Un point situé dans un volume solide n'est pas du sol : il ne doit
            // pas compter comme « à l'abri ». Sinon un bâtiment gonflerait
            // artificiellement la protection offerte par la carte.
            if inside_footprint(ground, covers) {
                continue;
            }
            total += 1;
            if !sight_blocked(eye_pos, Vec3::new(x, eye, z), covers) {
                seen += 1;
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    seen as f32 / total as f32
}

/// Vrai si le point au sol tombe dans l'emprise d'un volume qui dépasse la
/// cheville — donc pas du sol praticable.
fn inside_footprint(ground: Vec3, covers: &[CoverDisc]) -> bool {
    covers.iter().any(|&(center, height, half_width)| {
        center.y + height > 0.5
            && Vec2::new(ground.x - center.x, ground.z - center.z).length() < half_width
    })
}

/// Vrai si un cylindre de `covers` coupe le segment œil→cible.
fn sight_blocked(from: Vec3, to: Vec3, covers: &[CoverDisc]) -> bool {
    let d = to - from;
    let horiz = Vec3::new(d.x, 0.0, d.z);
    let span = horiz.length();
    if span <= f32::EPSILON {
        return false;
    }
    let dir = horiz / span;
    for &(center, height, half_width) in covers {
        // Projection du centre sur la ligne de visée horizontale.
        let to_center = Vec3::new(center.x - from.x, 0.0, center.z - from.z);
        let t = to_center.dot(dir);
        if t <= 0.0 || t >= span {
            continue; // derrière le tireur, ou au-delà de la cible
        }
        if (to_center - dir * t).length() > half_width {
            continue; // la visée passe à côté
        }
        // Altitude du rayon à cette distance : coupe si l'obstacle la dépasse.
        if center.y + height >= from.y + d.y * (t / span) {
            return true;
        }
    }
    false
}

/// Plus grande marche à franchir dans une chaîne de traversée, en mètres.
///
/// ⚠️ Une caisse de 2 m posée au sol n'est **pas** une marche de 2 m : on y monte
/// depuis la caisse de 1 m. Ce qui compte est l'écart entre deux surfaces
/// SUCCESSIVES, pas la hauteur d'un volume. Confondre les deux fait crier une
/// alerte sur un escalier parfaitement praticable.
///
/// On prend donc les sommets des pièces de traversée, on y ajoute le sol (la
/// première marche part de là), on trie, et on retient le plus grand écart entre
/// deux niveaux consécutifs.
///
/// Limite assumée : le saut FINAL de la chaîne vers sa plateforme n'est pas
/// couvert (rien n'associe une caisse à sa destination). Cette vérification-là
/// reste au playtest.
pub fn max_traversal_step(tops: &[f32]) -> f32 {
    let mut levels: Vec<f32> = tops.to_vec();
    levels.push(0.0);
    levels.sort_by(f32::total_cmp);
    levels.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
    levels
        .windows(2)
        .map(|w| w[1] - w[0])
        .fold(0.0_f32, f32::max)
}

/// Vrai si les deux valeurs s'écartent de plus de `tolerance` en relatif.
/// Répartition des volumes par fonction RÉELLE, mesurée sur leur hauteur.
///
/// Sans accroupissement et avec l'œil à 1,70 m, la taxonomie haute/basse/nulle des
/// jeux à couverture ne transpose pas : un abri « bas » à 1 m ne cache **rien**.
/// Il n'existe que trois fonctions — on monte dessus, ça casse la vue, ou ça ferme
/// l'espace — et la hauteur seule décide de laquelle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CoverBands {
    /// Franchissable au saut → c'est une traversée, pas un abri.
    pub traversable: u32,
    /// Entre le saut et le seuil de couverture : masque le corps, **pas la vue**.
    /// Zone morte — ces volumes ne servent à rien, ni à monter ni à se cacher.
    pub dead_band: u32,
    /// Vraie couverture : casse la ligne de vue sans fermer l'espace.
    pub cover: u32,
    /// Au-delà de la bande : occultation totale. C'est un mur, qu'on le dise.
    pub wall: u32,
}

pub fn cover_bands(heights: &[f32], jump_m: f32, low_m: f32, high_m: f32) -> CoverBands {
    let mut b = CoverBands::default();
    for &h in heights {
        if h <= jump_m {
            b.traversable += 1;
        } else if h < low_m {
            b.dead_band += 1;
        } else if h <= high_m {
            b.cover += 1;
        } else {
            b.wall += 1;
        }
    }
    b
}

// Story-674 — `covers_expected` a déménagé dans `forgia-core::layout` : le banc
// la MESURAIT, le générateur de décor roguelite en a besoin pour DÉRIVER ses
// comptes. Deux consommateurs = une seule source. Chemin public conservé.
pub use forgia_core::layout::covers_expected;

pub fn drifted(declared: f32, actual: f32, tolerance: f32) -> bool {
    if actual.abs() < f32::EPSILON {
        return declared.abs() > f32::EPSILON;
    }
    ((declared - actual) / actual).abs() > tolerance
}

/// Grille de construction — le vocabulaire dimensionnel de l'arène.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct GridCfg {
    pub module_m: f32,
    pub step_max_m: f32,
    pub platform_step_m: f32,
    pub cover_low_m: f32,
    pub cover_high_m: f32,
    /// Espacement visé entre deux abris (m) — d'où se DÉRIVE leur nombre.
    ///
    /// Bande sourcée **3–10 m, 10 m au maximum** (Watch Dogs, Gears of War). Le
    /// compte d'abris d'une salle n'est donc pas une opinion sur la densité :
    /// c'est `aire / espacement²`. Valeur par défaut au milieu de la bande, pour
    /// que les cartes antérieures restent lisibles.
    #[serde(default = "default_cover_spacing")]
    pub cover_spacing_m: f32,
    pub wall_min_m: f32,
    pub gap_max_m: f32,
    pub show: bool,
    pub cell_count: u32,
    /// Débord de la dalle de plafond au-delà de l'emprise de la salle (m).
    ///
    /// Le creusement ouvre la salle AVEC une marge autour ; une dalle taillée
    /// pile sur l'emprise laisse donc une fente ouverte sur tout le périmètre —
    /// on voit une bande de vide au raccord mur/plafond, sur 200 m de développé
    /// par salle. Le débord la ferme. À garder ≥ à la marge de creusement.
    #[serde(default = "default_ceiling_overhang")]
    pub ceiling_overhang_m: f32,
}

fn default_cover_spacing() -> f32 {
    6.0
}

fn default_ceiling_overhang() -> f32 {
    1.5
}

impl Default for GridCfg {
    fn default() -> Self {
        Self {
            module_m: 2.0,
            step_max_m: 0.4,
            platform_step_m: 1.0,
            // Le défaut portait 1,0 → 2,0 m : une bande qui commence SOUS l'œil
            // (1,70 m). Un abri de 1,0 m ne cache rien — le défaut classait donc
            // en « couverture » des volumes qui ne couvrent pas. Aligné sur la
            // bande réelle, au-dessus de l'œil.
            cover_low_m: 1.8,
            cover_high_m: 2.8,
            cover_spacing_m: default_cover_spacing(),
            wall_min_m: 1.5,
            gap_max_m: 4.0,
            show: true,
            cell_count: 40,
            ceiling_overhang_m: default_ceiling_overhang(),
        }
    }
}

/// Code couleur fonctionnel du greybox.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct Palette {
    pub floor: [f32; 3],
    pub platform: [f32; 3],
    pub cover: [f32; 3],
    pub wall: [f32; 3],
    pub traversal: [f32; 3],
    pub perch: [f32; 3],
    /// Plafonds — plus sombres que les murs. En greybox le plafond doit se lire
    /// comme une limite, pas se confondre avec une paroi qu'on pourrait longer.
    #[serde(default = "default_ceiling_tint")]
    pub ceiling: [f32; 3],
}

fn default_ceiling_tint() -> [f32; 3] {
    [0.24, 0.24, 0.27]
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            floor: [0.52, 0.52, 0.54],
            platform: [0.62, 0.60, 0.56],
            cover: [0.44, 0.44, 0.47],
            wall: [0.34, 0.34, 0.37],
            traversal: [0.85, 0.68, 0.22],
            perch: [0.35, 0.62, 0.78],
            ceiling: default_ceiling_tint(),
        }
    }
}

impl Palette {
    /// Rôle → teinte. Rôle inconnu = gris neutre (jamais d'échec silencieux
    /// visuel : la pièce apparaît, elle est juste non catégorisée).
    fn color_for(&self, role: &str) -> Color {
        let c = match role {
            "floor" => self.floor,
            "platform" => self.platform,
            "cover" => self.cover,
            "wall" => self.wall,
            "traversal" => self.traversal,
            "perch" => self.perch,
            "ceiling" => self.ceiling,
            _ => self.floor,
        };
        Color::srgb(c[0], c[1], c[2])
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ArenaCfg {
    pub name: String,
    /// `"rectangular"` (deux camps, trois voies — modèle Call of Duty) ou
    /// `"hexagonal"` (arène centrée). Détermine l'enceinte ET la zone
    /// échantillonnée par l'analyse de domination.
    pub shape: String,
    /// Emprise jouable en rectangulaire (m). Ignoré en hexagonal.
    pub size_x: f32,
    pub size_z: f32,
    /// Rayon circonscrit en hexagonal (m). Ignoré en rectangulaire.
    pub extent_m: f32,
    pub wall_height_m: f32,
    pub wall_thickness_m: f32,
    /// Y = centre de la capsule joueur (demi-hauteur 1 m) : sous 1.0 le joueur
    /// naît encastré dans le sol.
    pub spawn_pos: [f32; 3],
    pub spawn_yaw_deg: f32,
}

impl Default for ArenaCfg {
    fn default() -> Self {
        Self {
            name: "Le Creuset".to_string(),
            shape: "rectangular".to_string(),
            size_x: 80.0,
            size_z: 56.0,
            extent_m: 30.0,
            wall_height_m: 8.0,
            wall_thickness_m: 1.0,
            spawn_pos: [-36.0, 1.5, 0.0],
            spawn_yaw_deg: -90.0,
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(default)]
pub struct LightingCfg {
    pub sun_illuminance: f32,
    pub sun_color: [f32; 3],
    pub fill_illuminance: f32,
    pub fill_color: [f32; 3],
    pub ambient_brightness: f32,
    pub ambient_color: [f32; 3],
}

impl Default for LightingCfg {
    fn default() -> Self {
        Self {
            sun_illuminance: 11_000.0,
            sun_color: [1.0, 0.99, 0.96],
            fill_illuminance: 4_000.0,
            fill_color: [0.78, 0.82, 0.90],
            ambient_brightness: 700.0,
            ambient_color: [0.80, 0.82, 0.88],
        }
    }
}

/// Pavé. `pos` = centre de l'empreinte au sol ; la boîte monte de `pos.y` à
/// `pos.y + size[1]`. Convention level designer, pas demi-extents.
#[derive(Deserialize, Clone, Debug)]
pub struct Block {
    pub pos: [f32; 3],
    pub size: [f32; 3],
    #[serde(default)]
    pub yaw_deg: f32,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub section: String,
}

/// Disque (sol, plateforme ronde). `pos` = centre de la base.
#[derive(Deserialize, Clone, Debug)]
pub struct Disc {
    pub pos: [f32; 3],
    pub radius_m: f32,
    pub height_m: f32,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub section: String,
}

/// Rampe décrite par ses DEUX EXTRÉMITÉS. Le code en déduit longueur, lacet et
/// pente : un designer pose deux points, il ne compose pas un quaternion.
#[derive(Deserialize, Clone, Debug)]
pub struct Ramp {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub width_m: f32,
    #[serde(default)]
    pub section: String,
}

/// Pose calculée d'une rampe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampPose {
    /// Centre du pavé (face supérieure sur le segment from→to).
    pub center: Vec3,
    pub rotation: Quat,
    /// Longueur suivant la pente (m).
    pub length_m: f32,
    /// Pente en degrés (positif = ça monte de `from` vers `to`).
    pub slope_deg: f32,
}

/// Pur : deux points + une épaisseur → la pose du pavé incliné.
///
/// `Quat::from_rotation_y(yaw)` envoie +Z sur `(sin yaw, 0, cos yaw)` : on aligne
/// donc l'axe long (Z local) sur la projection horizontale. Le tangage autour de
/// X local envoie +Z sur `(0, -sin p, cos p)` — d'où le signe négatif pour que
/// `to` soit bien le point HAUT.
pub fn ramp_pose(from: Vec3, to: Vec3, thickness: f32) -> RampPose {
    let delta = to - from;
    let horizontal = Vec3::new(delta.x, 0.0, delta.z);
    let run = horizontal.length();
    let length_m = delta.length().max(f32::EPSILON);
    let yaw = horizontal.x.atan2(horizontal.z);
    let pitch = -(delta.y / length_m).asin();
    let rotation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch);
    // La face supérieure doit passer par le segment → on descend d'une demi-dalle
    // le long de la normale de la rampe (Y local), pas de l'axe monde.
    let center = (from + to) * 0.5 + rotation * Vec3::new(0.0, -thickness * 0.5, 0.0);
    RampPose {
        center,
        rotation,
        length_m,
        slope_deg: delta.y.atan2(run.max(f32::EPSILON)).to_degrees(),
    }
}

/// Une voie de la carte, bornée sur Z. Modèle Call of Duty : deux voies
/// extérieures et une centrale relient les deux camps, et **chaque voie a une
/// portée dédiée**. Déclarer les voies dans la donnée permet de mesurer la
/// domination d'une position de force *sur sa propre voie* — la seule question
/// qui ait un sens ici : une position qui tient 60 % de SA voie est forte, la
/// même rapportée à la carte entière ne voudrait rien dire.
#[derive(Deserialize, Clone, Debug)]
pub struct LaneDef {
    pub name: String,
    pub z_min: f32,
    pub z_max: f32,
    /// Portée visée : "courte" | "moyenne" | "longue". Documentaire — l'analyse
    /// mesure le profil réel, elle ne le suppose pas.
    #[serde(default)]
    pub range: String,
}

/// Salle logique du greybox. Elle précède l'art : les volumes physiques sont
/// libres d'évoluer, mais les objectifs et connexions restent stables.
#[derive(Deserialize, Clone, Debug)]
pub struct RoomDef {
    pub id: String,
    /// `spawn`, `combat`, `flank`, `reward`, `bridge` ou `boss`.
    pub role: String,
    /// `center[1]` = niveau du SOL de la salle (la terrasse est à +4, la cour à +2).
    pub center: [f32; 3],
    /// Emprise. Seuls X et Z bornent la salle (cf. `room_contains_xz`) : la
    /// hauteur libre se déclare par `ceiling_m`, pour ne pas confondre volume
    /// d'appartenance et gabarit construit.
    pub size: [f32; 3],
    /// Hauteur libre sous plafond, au-dessus du sol de la salle (m).
    /// **0 = à ciel ouvert**, et c'est le défaut. Couvrir une salle sans y poser
    /// de lumière la rend NOIRE — le soleil directionnel ne passe plus. Le
    /// contrôle de santé refuse ce cas au lieu de laisser découvrir en jeu.
    #[serde(default)]
    pub ceiling_m: f32,
}

/// Source lumineuse ponctuelle. Indispensable dès qu'une salle est couverte :
/// un blockout d'intérieur éclairé au seul soleil directionnel est illisible.
///
/// ⚠️ Intensités FAIBLES. Une ponctuelle « réaliste » à 100 000 lm écrase
/// l'atmosphère et le bloom — même garde-fou que `forgia-stage` (dette 2026-05-18),
/// d'où le plafond appliqué au spawn.
#[derive(Deserialize, Clone, Debug)]
pub struct LightDef {
    pub pos: [f32; 3],
    #[serde(default = "default_light_intensity")]
    pub intensity: f32,
    #[serde(default = "default_light_range")]
    pub range_m: f32,
    #[serde(default = "default_light_color")]
    pub color: [f32; 3],
    /// Salle éclairée — sert au contrôle « salle couverte sans lumière ».
    #[serde(default)]
    pub room: String,
}

fn default_light_intensity() -> f32 {
    6_000.0
}
fn default_light_range() -> f32 {
    18.0
}
fn default_light_color() -> [f32; 3] {
    [1.0, 0.96, 0.90]
}

/// Connexion voulue entre deux salles. Quand une route change d'altitude, elle
/// référence la rampe qui la rend physiquement possible.
#[derive(Deserialize, Clone, Debug)]
pub struct RouteDef {
    pub id: String,
    pub from: String,
    pub to: String,
    pub width_m: f32,
    #[serde(default)]
    pub ramp_section: String,
    /// Centre du passage au niveau des pieds. Ce trajet est vérifié contre les
    /// murs/couvertures du greybox, indépendamment du rendu.
    #[serde(default)]
    pub path: Vec<[f32; 3]>,
}

/// Genome complet.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct ArenaTestGenome {
    pub metrics: Metrics,
    pub grid: GridCfg,
    pub palette: Palette,
    pub arena: ArenaCfg,
    pub lighting: LightingCfg,
    pub lanes: Vec<LaneDef>,
    pub rooms: Vec<RoomDef>,
    pub routes: Vec<RouteDef>,
    pub lights: Vec<LightDef>,
    pub blocks: Vec<Block>,
    pub discs: Vec<Disc>,
    pub ramps: Vec<Ramp>,
}

/// Contrat hard du plan : le spawn doit rejoindre le boss par les connexions
/// déclarées ; chaque sortie doit être assez large ; toute rampe référencée doit
/// exister et rester sous la pente de confort. C'est volontairement distinct du
/// futur navmesh : cette validation attrape les erreurs de plan dès le TOML.
pub fn route_contract_errors(g: &ArenaTestGenome) -> Vec<String> {
    if g.rooms.is_empty() && g.routes.is_empty() {
        return Vec::new(); // rétrocompatibilité du banc historique.
    }
    let mut errors = Vec::new();
    let mut rooms: HashMap<&str, &RoomDef> = HashMap::new();
    for room in &g.rooms {
        if room.id.trim().is_empty() || rooms.insert(&room.id, room).is_some() {
            errors.push(format!("room id invalide ou dupliqué: `{}`", room.id));
        }
        if room.size.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || room.center.iter().any(|v| !v.is_finite())
        {
            errors.push(format!("room `{}` a une emprise non positive", room.id));
        }
    }
    let spawn = g.rooms.iter().find(|room| room.role == "spawn");
    let boss = g.rooms.iter().find(|room| room.role == "boss");
    if spawn.is_none() {
        errors.push("aucune room role=spawn".to_string());
    }
    if boss.is_none() {
        errors.push("aucune room role=boss".to_string());
    }

    let min_width = g.metrics.player_radius_m * 2.0 + 0.8;
    let mut links: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut route_ids: HashSet<&str> = HashSet::new();
    for route in &g.routes {
        if route.id.trim().is_empty() || !route_ids.insert(&route.id) {
            errors.push(format!("route id invalide ou dupliqué: `{}`", route.id));
        }
        if !rooms.contains_key(route.from.as_str()) || !rooms.contains_key(route.to.as_str()) {
            errors.push(format!(
                "route `{}` référence une room absente ({} → {})",
                route.id, route.from, route.to
            ));
            continue;
        }
        if !route.width_m.is_finite() || route.width_m < min_width {
            errors.push(format!(
                "route `{}` fait {:.1} m (< {:.1} m requis)",
                route.id, route.width_m, min_width
            ));
        }
        if !route.ramp_section.is_empty() {
            match g.ramps.iter().find(|r| r.section == route.ramp_section) {
                Some(ramp)
                    if ramp_pose(
                        Vec3::from(ramp.from),
                        Vec3::from(ramp.to),
                        RAMP_SLAB_THICKNESS_M,
                    )
                    .slope_deg
                    .abs()
                        <= MAX_COMFORTABLE_RAMP_SLOPE_DEG =>
                {
                    let from_room = rooms[route.from.as_str()];
                    let to_room = rooms[route.to.as_str()];
                    let start = Vec3::from(ramp.from);
                    let end = Vec3::from(ramp.to);
                    let aligned = (room_contains_xz(from_room, start)
                        && room_contains_xz(to_room, end))
                        || (room_contains_xz(from_room, end) && room_contains_xz(to_room, start));
                    if !aligned {
                        errors.push(format!(
                            "route `{}` : la rampe `{}` ne relie pas les emprises déclarées `{}` et `{}`",
                            route.id, route.ramp_section, route.from, route.to
                        ));
                    }
                }
                Some(ramp) => errors.push(format!(
                    "route `{}` utilise la rampe `{}` à {:.1}° (> {:.0}° confortable)",
                    route.id,
                    route.ramp_section,
                    ramp_pose(
                        Vec3::from(ramp.from),
                        Vec3::from(ramp.to),
                        RAMP_SLAB_THICKNESS_M
                    )
                    .slope_deg
                    .abs(),
                    MAX_COMFORTABLE_RAMP_SLOPE_DEG
                )),
                None => errors.push(format!(
                    "route `{}` référence la rampe absente `{}`",
                    route.id, route.ramp_section
                )),
            }
        }
        validate_route_path(g, route, &rooms, &mut errors);
        links
            .entry(route.from.as_str())
            .or_default()
            .push(route.to.as_str());
        links
            .entry(route.to.as_str())
            .or_default()
            .push(route.from.as_str());
    }
    if let (Some(spawn), Some(boss)) = (spawn, boss) {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut pending = VecDeque::from([spawn.id.as_str()]);
        while let Some(room) = pending.pop_front() {
            if !seen.insert(room) {
                continue;
            }
            if let Some(next) = links.get(room) {
                pending.extend(next.iter().copied());
            }
        }
        if !seen.contains(boss.id.as_str()) {
            errors.push(format!(
                "boss `{}` inaccessible depuis le spawn `{}` dans le graphe de routes",
                boss.id, spawn.id
            ));
        }
    }
    errors
}

/// Une connexion doit toucher les volumes logiques, même lorsqu'elle arrive
/// exactement sur leur bord. La hauteur est laissée à la rampe : le plan de
/// salle reste en 2D pour ne pas confondre étage jouable et volume du mesh.
fn room_contains_xz(room: &RoomDef, point: Vec3) -> bool {
    const EDGE_EPSILON_M: f32 = 0.25;
    (point.x - room.center[0]).abs() <= room.size[0] * 0.5 + EDGE_EPSILON_M
        && (point.z - room.center[2]).abs() <= room.size[2] * 0.5 + EDGE_EPSILON_M
}

/// Validation géométrique légère et déterministe. Ce n'est pas encore le
/// NavMesh/Rapier probe final, mais elle arrête le cas fréquent où le graphe
/// est valide alors qu'un cover a été posé au milieu du passage promis.
fn validate_route_path(
    g: &ArenaTestGenome,
    route: &RouteDef,
    rooms: &HashMap<&str, &RoomDef>,
    errors: &mut Vec<String>,
) {
    if route.path.len() < 2 {
        errors.push(format!(
            "route `{}` n'a pas de trajectoire de référence (au moins 2 points requis)",
            route.id
        ));
        return;
    }
    let points: Vec<Vec3> = route.path.iter().copied().map(Vec3::from).collect();
    if !room_contains_xz(rooms[route.from.as_str()], points[0])
        || !room_contains_xz(rooms[route.to.as_str()], *points.last().expect("len >= 2"))
    {
        errors.push(format!(
            "route `{}` : les extrémités du trajet ne sont pas dans `{}` → `{}`",
            route.id, route.from, route.to
        ));
    }

    let half = g.arena.half_extents();
    let route_ramps: Vec<&Ramp> = g
        .ramps
        .iter()
        .filter(|r| route.ramp_section.is_empty() || r.section == route.ramp_section)
        .collect();
    for pair in points.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start.x.abs() > half.x
            || start.z.abs() > half.y
            || end.x.abs() > half.x
            || end.z.abs() > half.y
        {
            errors.push(format!("route `{}` sort de l'emprise de l'arène", route.id));
            break;
        }
        if (end.y - start.y).abs() > 0.25
            && !route_ramps
                .iter()
                .any(|ramp| segment_matches_ramp(start, end, ramp))
        {
            errors.push(format!(
                "route `{}` change de hauteur sans rampe déclarée ({:.1} → {:.1} m)",
                route.id, start.y, end.y
            ));
        }
        // Tranche verticale réellement occupée par le joueur le long du segment.
        // Sans elle, le contrôle est purement 2D : les piles SOUS un pont étaient
        // comptées comme bloquant la route QUI PASSE DESSUS.
        let path_low = start.y.min(end.y) - 0.1;
        let path_high = start.y.max(end.y) + g.metrics.player_height_m;
        for block in &g.blocks {
            if !matches!(block.role.as_str(), "wall" | "cover" | "perch") {
                continue;
            }
            let block_low = block.pos[1];
            let block_high = block.pos[1] + block.size[1];
            if block_high <= path_low || block_low >= path_high {
                continue; // entièrement sous ou au-dessus du trajet
            }
            // ⚠️ Le dégagement se mesure sur la LARGEUR DÉCLARÉE de la route, pas
            // sur le rayon du joueur. Avec `player_radius_m` (0,3 m) le contrôle
            // ne testait qu'un fil de 0,6 m : un bloc pouvait manger 88 % d'un
            // couloir annoncé à 5 m sans que rien ne le signale — le contrat
            // validait une donnée qu'il n'utilisait pas.
            //
            // On teste le CANAL UTILE, pas la largeur pleine : à `width/2` exact,
            // ce sont les murs qui DÉFINISSENT le couloir qui sonnent, puisqu'ils
            // se trouvent précisément à cette distance. La fraction reprend la
            // règle « une porte fait 62-88 % de la largeur du couloir ».
            if segment_hits_block_clearance(
                start,
                end,
                block,
                route.width_m * 0.5 * USABLE_CHANNEL_RATIO,
            ) {
                errors.push(format!(
                    "route `{}` traverse `{}` ({}) : déplacer le bloc ou le trajet",
                    route.id, block.section, block.role
                ));
                break;
            }
        }
    }
}

fn segment_matches_ramp(start: Vec3, end: Vec3, ramp: &Ramp) -> bool {
    const ENDPOINT_EPSILON_M: f32 = 0.35;
    let from = Vec3::from(ramp.from);
    let to = Vec3::from(ramp.to);
    (start.distance(from) <= ENDPOINT_EPSILON_M && end.distance(to) <= ENDPOINT_EPSILON_M)
        || (start.distance(to) <= ENDPOINT_EPSILON_M && end.distance(from) <= ENDPOINT_EPSILON_M)
}

/// Collision XZ du centre de capsule. Le sol et les plateformes portent le
/// chemin et sont donc exclus par l'appelant.
///
/// **La capsule est un disque, pas un carré.** Gonfler l'AABB du pavé de
/// `radius` sur-déclare jusqu'à 41 % dans les diagonales — le coin du pavé
/// gonflé est à `radius·√2` du pavé réel. On mesure donc la distance vraie,
/// comme `forgia_stage::layout::is_in_player_boss_corridor`.
fn segment_hits_block_clearance(start: Vec3, end: Vec3, block: &Block, radius: f32) -> bool {
    let min_y = block.pos[1] - 0.05;
    let max_y = block.pos[1] + block.size[1] + 0.05;
    if start.y.max(end.y) < min_y || start.y.min(end.y) > max_y {
        return false;
    }
    // Passage dans le repère du pavé : les obstacles peuvent être orientés et
    // une AABB monde les rendrait artificiellement trop large ou trop petite.
    let a = world_to_block_xz(start, block);
    let b = world_to_block_xz(end, block);
    let half = Vec2::new(block.size[0] * 0.5, block.size[2] * 0.5);
    segment_aabb_distance_2d(a, b, half) <= radius
}

/// Distance exacte segment ↔ pavé centré, en 2D. Deux convexes : le minimum est
/// atteint sur un sommet de l'un et le point le plus proche de l'autre — donc
/// les extrémités du segment contre le pavé, et les 4 coins contre le segment.
fn segment_aabb_distance_2d(a: Vec2, b: Vec2, half: Vec2) -> f32 {
    if segment_intersects_aabb_2d(a, b, -half, half) {
        return 0.0;
    }
    let mut best = point_aabb_distance_2d(a, half).min(point_aabb_distance_2d(b, half));
    for corner in [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(half.x, half.y),
        Vec2::new(-half.x, half.y),
    ] {
        best = best.min(closest_point_on_segment_2d(corner, a, b).distance(corner));
    }
    best
}

fn point_aabb_distance_2d(p: Vec2, half: Vec2) -> f32 {
    Vec2::new(p.x.abs() - half.x, p.y.abs() - half.y)
        .max(Vec2::ZERO)
        .length()
}

fn closest_point_on_segment_2d(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < f32::EPSILON {
        return a;
    }
    a + ab * ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0)
}

fn world_to_block_xz(point: Vec3, block: &Block) -> Vec2 {
    let delta = Vec2::new(point.x - block.pos[0], point.z - block.pos[2]);
    let (sin, cos) = block.yaw_deg.to_radians().sin_cos();
    // Inverse de la rotation Bevy autour de Y.
    Vec2::new(cos * delta.x - sin * delta.y, sin * delta.x + cos * delta.y)
}

/// Slab test 2D : le segment touche-t-il le pavé ? Sert de cas « distance nulle »
/// à [`segment_aabb_distance_2d`], donc sur le pavé RÉEL (jamais gonflé).
fn segment_intersects_aabb_2d(start: Vec2, end: Vec2, min: Vec2, max: Vec2) -> bool {
    let delta = end - start;
    let mut t_min: f32 = 0.0;
    let mut t_max: f32 = 1.0;
    for (origin, direction, lo, hi) in [
        (start.x, delta.x, min.x, max.x),
        (start.y, delta.y, min.y, max.y),
    ] {
        if direction.abs() < f32::EPSILON {
            if origin < lo || origin > hi {
                return false;
            }
            continue;
        }
        let mut a = (lo - origin) / direction;
        let mut b = (hi - origin) / direction;
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        t_min = t_min.max(a);
        t_max = t_max.min(b);
        if t_min > t_max {
            return false;
        }
    }
    true
}

impl ArenaTestGenome {
    /// Bande Z de la voie qui contient `z`. Toute la carte si aucune voie n'est
    /// déclarée (arène centrée d'avant la refonte trois voies).
    pub fn lane_of(&self, z: f32, half: Vec2) -> (f32, f32) {
        self.lanes
            .iter()
            .find(|l| z >= l.z_min && z <= l.z_max)
            .map(|l| (l.z_min, l.z_max))
            .unwrap_or((-half.y, half.y))
    }

    /// Emprise à laquelle rapporter la domination d'une position de force.
    ///
    /// **La SALLE d'abord** : sur une carte à salles, une position tient sa
    /// salle, pas la carte. Rapporter à la carte entière noierait le signal —
    /// une position parfaitement dominante afficherait 4 %.
    /// À défaut, la voie (cartes trois voies), puis la carte entière.
    pub fn region_of(&self, pos: Vec3, half: Vec2) -> (Vec2, Vec2) {
        if let Some(room) = self.rooms.iter().find(|r| {
            (pos.x - r.center[0]).abs() <= r.size[0] * 0.5
                && (pos.z - r.center[2]).abs() <= r.size[2] * 0.5
        }) {
            let c = Vec2::new(room.center[0], room.center[2]);
            let h = Vec2::new(room.size[0] * 0.5, room.size[2] * 0.5);
            return (c - h, c + h);
        }
        let (z0, z1) = self.lane_of(pos.z, half);
        (Vec2::new(-half.x, z0), Vec2::new(half.x, z1))
    }
}

// ─── Ressources runtime ──────────────────────────────────────────────────────

/// Genome chargé + état de la surveillance disque (hot-reload).
#[derive(Resource, Debug)]
pub struct ArenaTestConfig {
    pub genome: ArenaTestGenome,
    /// Metrics recalculées depuis `player_movement.toml`.
    pub derived: DerivedMetrics,
    /// Champs de `[metrics]` en désaccord avec le calcul.
    pub metrics_drift: Vec<String>,
    /// Violations hard du graphe de salles / routes.
    pub route_errors: Vec<String>,
    /// Message de parse si le TOML est illisible (arène alors vide → alerte).
    pub load_error: String,
    /// Scène chargée et surveillée par le hot-reload.
    source_path: &'static str,
    mtime: Option<SystemTime>,
    poll_timer: f32,
    /// Incrémenté à chaque rechargement — déclenche la reconstruction.
    pub revision: u32,
}

impl Default for ArenaTestConfig {
    fn default() -> Self {
        Self {
            genome: ArenaTestGenome::default(),
            derived: DerivedMetrics::default(),
            metrics_drift: Vec::new(),
            route_errors: Vec::new(),
            load_error: String::new(),
            source_path: GENOME_PATH,
            mtime: None,
            poll_timer: 0.0,
            revision: 0,
        }
    }
}

/// Bascules d'affichage du banc (F2 / F4).
#[derive(Resource, Debug)]
pub struct ArenaTestView {
    pub show_grid: bool,
    pub show_metrics: bool,
}

impl Default for ArenaTestView {
    fn default() -> Self {
        Self {
            show_grid: true,
            show_metrics: true,
        }
    }
}

/// Ce qui a réellement été posé — la base du capteur.
#[derive(Resource, Debug, Default)]
pub struct ArenaTestTelemetry {
    pub blocks_spawned: u32,
    pub discs_spawned: u32,
    pub ramps_spawned: u32,
    pub walls_spawned: u32,
    /// Rampe la plus raide posée (°). Au-delà de ~35° ça devient pénible à monter.
    pub steepest_ramp_deg: f32,
    /// Marche la plus haute d'une chaîne de traversée (m). Doit rester sous la
    /// hauteur de saut mesurée, sinon la route est infranchissable.
    pub tallest_traversal_step_m: f32,
    /// Nombre de pièces `role = "traversal"` réellement posées.
    ///
    /// ⚠️ SANS CE COMPTE, `tallest_traversal_step_m` MENT. Il vaut 0,0 aussi bien
    /// quand les marches sont franchissables que quand il n'y a **aucune** chaîne
    /// de saut à mesurer — et 0,0 se lit comme « vérifié, tout va bien ». Un
    /// contrôle qui n'a rien à contrôler doit le DIRE, pas passer au vert.
    pub traversal_blocks: u32,
    /// Marge entre le bord du sol et l'enceinte, sur l'axe le plus juste (m).
    /// **Négative = il existe une bordure sans sol**, où l'on tombe hors de la
    /// carte : panne silencieuse, on croit à un bug de collision.
    pub floor_margin_m: f32,
    /// Fraction du sol de la fosse dominée par le point haut le plus fort (%).
    /// 100 = poste de tir absolu : le point haut n'est pas contesté, il est acquis.
    pub worst_overlook_pct: f32,
    /// Hauteur qu'un obstacle placé à mi-chemin devrait atteindre pour couper la
    /// vue du point haut le plus élevé. Comparer à la couverture réellement posée
    /// dit en un coup d'œil si le point haut est contestable — ou décoratif.
    pub cover_needed_at_midspan_m: f32,
    /// Volumes qui cassent VRAIMENT la ligne de vue sans fermer l'espace.
    ///
    /// ⚠️ Mesuré sur la HAUTEUR, pas sur l'étiquette. 16 blocs portaient
    /// `role = "cover"` en faisant 3 à 6 m : des murs. La carte annonçait
    /// 16 couvertures et n'en offrait **aucune** — donc aucune mécanique de peek.
    pub covers_in_band: u32,
    /// Volumes entre le saut et le seuil de couverture : ils masquent le corps
    /// mais **pas la vue**. Ni marche, ni abri — du décor qui coûte un collider.
    pub covers_dead_band: u32,
    /// Compte d'abris attendu, `Σ aire / espacement²` sur les salles de combat.
    /// À comparer à `covers_in_band` : l'écart est le déficit de couverture.
    pub covers_expected: f32,
    /// Structure solide la plus haute réellement posée. À comparer directement à
    /// `cover_needed_at_midspan_m` : si elle est en dessous, aucune architecture
    /// de l'arène ne peut contester le point haut.
    pub tallest_occluder_m: f32,
    /// Dalles de plafond posées (salles couvertes).
    pub ceilings_spawned: u32,
    /// Lumières ponctuelles posées.
    pub lights_spawned: u32,
    /// Salles couvertes sans aucune lumière — donc NOIRES en jeu.
    pub dark_rooms: u32,
    /// Nom de la première salle noire, pour que l'alerte soit actionnable.
    pub first_dark_room: String,
    /// Plus faible hauteur libre sous plafond (m). 0 = aucune salle couverte.
    pub min_headroom_m: f32,
    pub player_altitude_m: f32,
    /// Position au sol du joueur. Sans elle, « regarde où je suis » est
    /// insoluble : l'altitude seule ne localise personne.
    pub player_x_m: f32,
    pub player_z_m: f32,
    /// Salle qui contient le joueur, ou vide s'il est dans un couloir.
    pub player_room: String,
    pub sensor_timer: f32,
    /// Révision du genome effectivement construite. Évite de reconstruire à
    /// chaque frame (et de reconstruire une 2ᵉ fois juste après `OnEnter`).
    pub built_revision: u32,
}

/// Marqueur — tout ce que le banc pose, despawn à la sortie.
#[derive(Component)]
struct ArenaTestMarker;

/// Marqueur d'idempotence de l'ambiante posée sur une caméra.
#[derive(Component)]
struct ArenaTestAmbientApplied;

/// Vrai quand le joueur doit être (re)posé au spawn de l'arène.
#[derive(Resource, Default)]
struct ArenaTestSpawnPending;

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct ArenaTestPlugin;

impl Plugin for ArenaTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ArenaTestConfig>()
            .init_resource::<ArenaTestView>()
            .init_resource::<ArenaTestTelemetry>()
            .add_systems(
                OnEnter(GameMode::ArenaTest),
                (sys_load_genome, sys_build_arena).chain(),
            )
            .add_systems(OnExit(GameMode::ArenaTest), sys_cleanup)
            .add_systems(
                Update,
                (
                    sys_poll_genome,
                    sys_rebuild_on_reload,
                    sys_place_player,
                    sys_ensure_ambient,
                    sys_toggle_view,
                    sys_draw_grid,
                )
                    .chain()
                    .run_if(in_state(GameMode::ArenaTest))
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(
                Update,
                sys_write_sensor
                    .in_set(GameSet::Sensors)
                    .run_if(in_state(GameMode::ArenaTest)),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                sys_metrics_overlay
                    .run_if(in_state(GameMode::ArenaTest))
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

// ─── Chargement du genome ────────────────────────────────────────────────────

/// Lit `player_movement.toml` pour recalculer les dérivées. Un champ absent
/// retombe sur le défaut Rust de la locomotion (miroir documenté du TOML).
fn read_locomotion() -> (f32, f32, f32, f32) {
    #[derive(Deserialize)]
    #[serde(default)]
    struct Movement {
        speed: f32,
        sprint_multiplier: f32,
        jump_velocity: f32,
        gravity: f32,
    }
    impl Default for Movement {
        fn default() -> Self {
            Self {
                speed: 6.5,
                sprint_multiplier: 1.5,
                jump_velocity: 6.5,
                gravity: 18.0,
            }
        }
    }
    let m: Movement = fs::read_to_string(MOVEMENT_GENOME_PATH)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    (m.speed, m.sprint_multiplier, m.jump_velocity, m.gravity)
}

fn load_genome_from_path(cfg: &mut ArenaTestConfig, path: &'static str) {
    cfg.source_path = path;
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            cfg.load_error = format!("{path} illisible : {e}");
            error!("[arena-test] {}", cfg.load_error);
            return;
        }
    };
    match toml::from_str::<ArenaTestGenome>(&source) {
        Ok(genome) => {
            cfg.genome = genome;
            cfg.load_error.clear();
        }
        Err(e) => {
            cfg.load_error = format!("{path} invalide : {e}");
            error!("[arena-test] {}", cfg.load_error);
            return;
        }
    }
    cfg.mtime = fs::metadata(path).and_then(|m| m.modified()).ok();

    // Contrat de vérité : les metrics déclarées sont confrontées au calcul.
    let (speed, sprint_mul, jump_v, gravity) = read_locomotion();
    let derived = derive_metrics(speed, sprint_mul, jump_v, gravity);
    let declared = cfg.genome.metrics;
    let mut drift = Vec::new();
    for (label, d, a) in [
        (
            "jump_height_m",
            declared.jump_height_m,
            derived.jump_height_m,
        ),
        ("air_time_s", declared.air_time_s, derived.air_time_s),
        (
            "jump_reach_walk_m",
            declared.jump_reach_walk_m,
            derived.jump_reach_walk_m,
        ),
        (
            "jump_reach_sprint_m",
            declared.jump_reach_sprint_m,
            derived.jump_reach_sprint_m,
        ),
        ("walk_speed_ms", declared.walk_speed_ms, speed),
        ("jump_velocity_ms", declared.jump_velocity_ms, jump_v),
        ("gravity_ms2", declared.gravity_ms2, gravity),
    ] {
        if drifted(d, a, METRICS_DRIFT_TOLERANCE) {
            drift.push(format!("{label} déclaré {d:.3} ≠ mesuré {a:.3}"));
        }
    }
    if !drift.is_empty() {
        warn!(
            "[arena-test] metrics périmées dans {path} : {}",
            drift.join(" · ")
        );
    }
    cfg.derived = derived;
    cfg.metrics_drift = drift;
    cfg.route_errors = route_contract_errors(&cfg.genome);
    if !cfg.route_errors.is_empty() {
        error!(
            "[arena-test] contrat de routes invalide dans {path} : {}",
            cfg.route_errors.join(" · ")
        );
    }
    cfg.revision = cfg.revision.wrapping_add(1);
    info!(
        "[arena-test] genome chargé : « {} » ({path}) — {} pavés, {} disques, {} rampes",
        cfg.genome.arena.name,
        cfg.genome.blocks.len(),
        cfg.genome.discs.len(),
        cfg.genome.ramps.len()
    );
}

fn sys_load_genome(mut cfg: ResMut<ArenaTestConfig>) {
    let path = cfg.source_path;
    load_genome_from_path(&mut cfg, path);
}

/// Surveille le mtime du genome — sauvegarder le TOML reconstruit l'arène.
fn sys_poll_genome(time: Res<Time>, mut cfg: ResMut<ArenaTestConfig>) {
    cfg.poll_timer += time.delta_secs();
    if cfg.poll_timer < POLL_PERIOD_SEC {
        return;
    }
    cfg.poll_timer = 0.0;
    let current = fs::metadata(cfg.source_path)
        .and_then(|m| m.modified())
        .ok();
    if current.is_some() && current != cfg.mtime {
        info!(
            "[arena-test] genome modifié ({}) — reconstruction",
            cfg.source_path
        );
        let path = cfg.source_path;
        load_genome_from_path(&mut cfg, path);
    }
}

// ─── Construction ────────────────────────────────────────────────────────────

/// Matériau mat : en greybox on lit la FORME, donc pas de spéculaire qui parasite.
fn greybox_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.92,
        metallic: 0.0,
        ..default()
    }
}

/// Milieux + orientations + longueurs des segments d'enceinte.
///
/// Rectangulaire (modèle Call of Duty, deux camps face à face) : 4 murs.
/// Hexagonal (arène centrée) : 6 côtés inscrits dans le cercle `extent_m`.
/// `Quat::from_rotation_y(yaw)` envoie +X sur `(cos yaw, 0, -sin yaw)` : on aligne
/// donc l'axe long du mur sur le segment.
fn perimeter_poses(cfg: &ArenaCfg) -> Vec<(Vec3, f32, f32)> {
    let half = cfg.half_extents();
    if cfg.shape == "hexagonal" {
        let e = cfg.extent_m;
        if e <= 0.0 {
            return Vec::new();
        }
        let verts: Vec<Vec3> = (0..6)
            .map(|i| {
                let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
                Vec3::new(e * a.cos(), 0.0, e * a.sin())
            })
            .collect();
        return (0..6)
            .map(|i| {
                let a = verts[i];
                let b = verts[(i + 1) % 6];
                let dir = b - a;
                ((a + b) * 0.5, (-dir.z).atan2(dir.x), dir.length())
            })
            .collect();
    }
    if half.x <= 0.0 || half.y <= 0.0 {
        return Vec::new();
    }
    let t = cfg.wall_thickness_m;
    // Les murs Z portent la largeur totale + épaisseur pour fermer les angles.
    vec![
        (
            Vec3::new(0.0, 0.0, half.y + t * 0.5),
            0.0,
            half.x * 2.0 + t * 2.0,
        ),
        (
            Vec3::new(0.0, 0.0, -(half.y + t * 0.5)),
            0.0,
            half.x * 2.0 + t * 2.0,
        ),
        (
            Vec3::new(half.x + t * 0.5, 0.0, 0.0),
            std::f32::consts::FRAC_PI_2,
            half.y * 2.0,
        ),
        (
            Vec3::new(-(half.x + t * 0.5), 0.0, 0.0),
            std::f32::consts::FRAC_PI_2,
            half.y * 2.0,
        ),
    ]
}

fn sys_build_arena(
    mut commands: Commands,
    cfg: Res<ArenaTestConfig>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut telemetry: ResMut<ArenaTestTelemetry>,
    existing: Query<Entity, With<ArenaTestMarker>>,
) {
    // Reconstruction : on efface d'abord (hot-reload → repartir d'une page vierge).
    for e in &existing {
        commands.entity(e).despawn();
    }
    *telemetry = ArenaTestTelemetry::default();

    let g = &cfg.genome;
    let palette = g.palette;

    // Un matériau par rôle, partagé — pas un matériau par pièce (chaque matériau
    // distinct est une spécialisation de pipeline à compiler, cf leçon warmup PBR).
    let mat_of = |role: &str, materials: &mut Assets<StandardMaterial>| {
        materials.add(greybox_material(palette.color_for(role)))
    };
    let mat_floor = mat_of("floor", &mut materials);
    let mat_platform = mat_of("platform", &mut materials);
    let mat_cover = mat_of("cover", &mut materials);
    let mat_wall = mat_of("wall", &mut materials);
    let mat_traversal = mat_of("traversal", &mut materials);
    let mat_perch = mat_of("perch", &mut materials);
    let mat_ceiling = mat_of("ceiling", &mut materials);
    let pick = |role: &str| match role {
        "platform" => mat_platform.clone(),
        "cover" => mat_cover.clone(),
        "wall" => mat_wall.clone(),
        "traversal" => mat_traversal.clone(),
        "perch" => mat_perch.clone(),
        "ceiling" => mat_ceiling.clone(),
        _ => mat_floor.clone(),
    };

    // Matière de l'analyse de domination (cf. `overlook_fraction`). Les occultants
    // recensent TOUTE la géométrie solide, pas seulement les couvertures.
    let mut occluders: Vec<CoverDisc> = Vec::new();
    // Jusqu'où le sol porte réellement, par axe. Comparé ensuite à l'emprise :
    // un sol plus petit que l'enceinte laisse une bordure où l'on tombe.
    let mut floor_reach = Vec2::ZERO;

    // Sommets des pièces de traversée et positions de force. Les perchoirs
    // peuvent être des pavés *ou* des disques : les deux doivent alimenter la
    // même mesure de domination.
    let mut traversal_tops: Vec<f32> = Vec::new();
    let mut perch_tops: Vec<Vec3> = Vec::new();

    // ── Disques (sol, massif central) ──
    for (i, d) in g.discs.iter().enumerate() {
        let pos = Vec3::from(d.pos);
        let mesh = meshes.add(Cylinder::new(d.radius_m, d.height_m));
        commands.spawn((
            Name::new(format!("ArenaTestDisc_{}_{i}", d.section)),
            ArenaTestMarker,
            Mesh3d(mesh),
            MeshMaterial3d(pick(&d.role)),
            Transform::from_translation(pos + Vec3::Y * d.height_m * 0.5),
            RigidBody::Fixed,
            Collider::cylinder(d.height_m * 0.5, d.radius_m),
        ));
        telemetry.discs_spawned += 1;
        // Un massif central occulte autant qu'un mur (le sol, lui, a son sommet
        // à 0 : il ne coupera jamais une visée qui part d'une tête à 1,7 m).
        occluders.push((pos, d.height_m, d.radius_m));
        if d.role == "floor" {
            let reach = Vec3::new(pos.x, 0.0, pos.z).length() + d.radius_m;
            floor_reach = floor_reach.max(Vec2::splat(reach));
        }
        if d.role == "perch" {
            perch_tops.push(Vec3::new(pos.x, pos.y + d.height_m, pos.z));
        }
    }

    // ── Pavés (plateformes, couvertures, chaînes de saut) ──
    // Sommets des pièces de traversée : la hauteur des MARCHES s'en déduit après
    // la boucle (un volume isolé ne dit rien de l'effort demandé au joueur).
    for (i, b) in g.blocks.iter().enumerate() {
        let pos = Vec3::from(b.pos);
        let size = Vec3::from(b.size);
        let mesh = meshes.add(Cuboid::new(size.x, size.y, size.z));
        commands.spawn((
            Name::new(format!("ArenaTestBlock_{}_{i}", b.section)),
            ArenaTestMarker,
            Mesh3d(mesh),
            MeshMaterial3d(pick(&b.role)),
            Transform {
                translation: pos + Vec3::Y * size.y * 0.5,
                rotation: Quat::from_rotation_y(b.yaw_deg.to_radians()),
                scale: Vec3::ONE,
            },
            RigidBody::Fixed,
            Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
        ));
        telemetry.blocks_spawned += 1;
        // Rayon circonscrit : on est GÉNÉREUX avec les occultants. Si la
        // domination reste écrasante malgré ça, le constat est sans appel.
        occluders.push((pos, size.y, Vec2::new(size.x, size.z).length() * 0.5));
        match b.role.as_str() {
            "traversal" => {
                traversal_tops.push(pos.y + size.y);
                telemetry.traversal_blocks += 1;
            }
            "perch" => perch_tops.push(Vec3::new(pos.x, pos.y + size.y, pos.z)),
            "floor" => {
                floor_reach = floor_reach.max(Vec2::new(
                    pos.x.abs() + size.x * 0.5,
                    pos.z.abs() + size.z * 0.5,
                ));
            }
            _ => {}
        }
    }
    // Une marche plus haute que le saut = route morte. On la mesure pour que le
    // capteur puisse le dire, plutôt que de laisser le joueur découvrir qu'il ne
    // monte pas et croire à un bug de collision.
    telemetry.tallest_traversal_step_m = max_traversal_step(&traversal_tops);

    // Combien de volumes cassent VRAIMENT la ligne de vue sans fermer l'espace.
    //
    // Compter les blocs `role = "cover"` ne suffisait pas : 16 d'entre eux
    // faisaient 3 à 6 m, donc des murs. La carte annonçait 16 couvertures et n'en
    // offrait aucune — aucune mécanique de peek nulle part. On mesure donc la
    // HAUTEUR, pas l'étiquette, et on la confronte au compte dérivé de la surface.
    // Uniquement les volumes qui PRÉTENDENT abriter ou porter un pas. Mesurer la
    // hauteur de TOUS les blocs comptait les plateformes (la cour est à +2, la
    // chapelle à +2,8) comme des couvertures : 20 annoncées pour 14 réelles.
    // Mesurer la bonne grandeur sur la mauvaise population reste un capteur faux.
    let occluder_heights: Vec<f32> = g
        .blocks
        .iter()
        .filter(|b| matches!(b.role.as_str(), "cover" | "traversal"))
        .map(|b| b.size[1])
        .collect();
    let bands = cover_bands(
        &occluder_heights,
        g.metrics.jump_height_m,
        g.grid.cover_low_m,
        g.grid.cover_high_m,
    );
    telemetry.covers_in_band = bands.cover;
    telemetry.covers_dead_band = bands.dead_band;
    let spacing = g.grid.cover_spacing_m;
    telemetry.covers_expected = g
        .rooms
        .iter()
        .filter(|r| matches!(r.role.as_str(), "combat" | "boss"))
        .map(|r| covers_expected(r.size[0] * r.size[2], spacing))
        .sum();

    // Domination des positions de force. Une position qui voit tout n'est pas un
    // objectif à conquérir : c'est un acquis, et la voie perd sa raison d'être.
    let eye = g.metrics.eye_height_m;
    let half = g.arena.half_extents();
    telemetry.tallest_occluder_m = occluders.iter().map(|&(_, h, _)| h).fold(0.0_f32, f32::max);
    telemetry.floor_margin_m = (floor_reach - half).min_element();
    // Domination de chaque position de force SUR SA PROPRE VOIE.
    telemetry.worst_overlook_pct = perch_tops
        .iter()
        .map(|&p| overlook_fraction(p, eye, &occluders, g.region_of(p, half), 44))
        .fold(0.0_f32, f32::max)
        * 100.0;
    // Le chiffre qui rend le constat concret : ce qu'il FAUDRAIT construire pour
    // contester la position la plus haute, à comparer à ce qui est réellement posé.
    if let Some(highest) = perch_tops
        .iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .copied()
    {
        let span = half.x - Vec2::new(highest.x, highest.z).length().min(half.x);
        let span = span.max(half.x * 0.5);
        telemetry.cover_needed_at_midspan_m =
            cover_height_to_break_sight(highest.y, span, span * 0.5, eye);
    }

    // ── Rampes ──
    for (i, r) in g.ramps.iter().enumerate() {
        let pose = ramp_pose(Vec3::from(r.from), Vec3::from(r.to), RAMP_SLAB_THICKNESS_M);
        let mesh = meshes.add(Cuboid::new(r.width_m, RAMP_SLAB_THICKNESS_M, pose.length_m));
        commands.spawn((
            Name::new(format!("ArenaTestRamp_{}_{i}", r.section)),
            ArenaTestMarker,
            Mesh3d(mesh),
            MeshMaterial3d(mat_traversal.clone()),
            Transform {
                translation: pose.center,
                rotation: pose.rotation,
                scale: Vec3::ONE,
            },
            RigidBody::Fixed,
            Collider::cuboid(
                r.width_m * 0.5,
                RAMP_SLAB_THICKNESS_M * 0.5,
                pose.length_m * 0.5,
            ),
        ));
        telemetry.ramps_spawned += 1;
        let slope = pose.slope_deg.abs();
        if slope > telemetry.steepest_ramp_deg {
            telemetry.steepest_ramp_deg = slope;
        }
    }

    // ── Enceinte (rectangulaire ou hexagonale, selon `[arena] shape`) ──
    let a = &g.arena;
    for (i, (mid, yaw, side_len)) in perimeter_poses(a).into_iter().enumerate() {
        let mesh = meshes.add(Cuboid::new(side_len, a.wall_height_m, a.wall_thickness_m));
        commands.spawn((
            Name::new(format!("ArenaTestWall_{i}")),
            ArenaTestMarker,
            Mesh3d(mesh),
            MeshMaterial3d(mat_wall.clone()),
            Transform {
                translation: mid + Vec3::Y * a.wall_height_m * 0.5,
                rotation: Quat::from_rotation_y(yaw),
                scale: Vec3::ONE,
            },
            RigidBody::Fixed,
            Collider::cuboid(
                side_len * 0.5,
                a.wall_height_m * 0.5,
                a.wall_thickness_m * 0.5,
            ),
        ));
        telemetry.walls_spawned += 1;
    }

    // ── Plafonds des salles couvertes ──
    // Générés depuis `[[rooms]] ceiling_m`, comme l'enceinte est générée depuis
    // `shape`/`size` : la dalle ne peut pas dériver de l'emprise qu'elle couvre.
    // Un plafond n'est PAS un occultant de tir : la visée reste à hauteur d'œil,
    // il ne doit donc pas entrer dans l'analyse de domination.
    let over = g.grid.ceiling_overhang_m.max(0.0);
    for room in g.rooms.iter().filter(|r| r.ceiling_m > 0.0) {
        let top = room.center[1] + room.ceiling_m;
        commands.spawn((
            Name::new(format!("ArenaTestCeiling_{}", room.id)),
            ArenaTestMarker,
            Mesh3d(meshes.add(Cuboid::new(
                room.size[0] + 2.0 * over,
                CEILING_SLAB_THICKNESS_M,
                room.size[2] + 2.0 * over,
            ))),
            MeshMaterial3d(mat_ceiling.clone()),
            Transform::from_xyz(
                room.center[0],
                top + CEILING_SLAB_THICKNESS_M * 0.5,
                room.center[2],
            ),
            RigidBody::Fixed,
            Collider::cuboid(
                room.size[0] * 0.5 + over,
                CEILING_SLAB_THICKNESS_M * 0.5,
                room.size[2] * 0.5 + over,
            ),
        ));
        telemetry.ceilings_spawned += 1;
        telemetry.min_headroom_m = if telemetry.min_headroom_m <= 0.0 {
            room.ceiling_m
        } else {
            telemetry.min_headroom_m.min(room.ceiling_m)
        };
    }

    // ── Lumières ponctuelles ──
    // Sans elles, une salle couverte est noire : le soleil directionnel ne
    // traverse pas une dalle. Intensité plafonnée — une ponctuelle trop forte
    // écrase l'atmosphère et le bloom (même garde-fou que forgia-stage).
    for (i, light) in g.lights.iter().enumerate() {
        commands.spawn((
            Name::new(format!("ArenaTestLight_{}_{i}", light.room)),
            ArenaTestMarker,
            PointLight {
                color: Color::srgb(light.color[0], light.color[1], light.color[2]),
                intensity: light.intensity.clamp(0.0, MAX_POINT_LIGHT_LUMENS),
                range: light.range_m,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(light.pos[0], light.pos[1], light.pos[2]),
        ));
        telemetry.lights_spawned += 1;
    }

    // Salles couvertes que rien n'éclaire : la panne la plus bête et la plus
    // certaine dès qu'on pose des plafonds. On la nomme au lieu de la subir.
    for room in g.rooms.iter().filter(|r| r.ceiling_m > 0.0) {
        let lit = g.lights.iter().any(|l| {
            l.room == room.id
                || ((l.pos[0] - room.center[0]).abs() <= room.size[0] * 0.5
                    && (l.pos[2] - room.center[2]).abs() <= room.size[2] * 0.5)
        });
        if !lit {
            telemetry.dark_rooms += 1;
            if telemetry.first_dark_room.is_empty() {
                telemetry.first_dark_room.clone_from(&room.id);
            }
        }
    }

    // ── Éclairage neutre : on juge la forme, pas l'ambiance ──
    let l = g.lighting;
    commands.spawn((
        Name::new("ArenaTestKeyLight"),
        ArenaTestMarker,
        DirectionalLight {
            color: Color::srgb(l.sun_color[0], l.sun_color[1], l.sun_color[2]),
            illuminance: l.sun_illuminance,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.6, 0.0)),
    ));
    commands.spawn((
        Name::new("ArenaTestFillLight"),
        ArenaTestMarker,
        DirectionalLight {
            color: Color::srgb(l.fill_color[0], l.fill_color[1], l.fill_color[2]),
            illuminance: l.fill_illuminance,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, -2.1, 0.0)),
    ));

    telemetry.built_revision = cfg.revision;
    commands.insert_resource(ArenaTestSpawnPending);
    info!(
        "[arena-test] « {} » construit : {} pavés · {} disques · {} rampes · {} murs",
        a.name,
        telemetry.blocks_spawned,
        telemetry.discs_spawned,
        telemetry.ramps_spawned,
        telemetry.walls_spawned
    );
}

/// Reconstruit quand le genome a été rechargé.
///
/// La révision construite est portée par la télémétrie, pas par un `Local` : un
/// `Local` partirait à 0 alors que `OnEnter` a déjà construit la révision 1, et
/// le banc se reconstruirait une seconde fois dès la première frame.
fn sys_rebuild_on_reload(
    cfg: Res<ArenaTestConfig>,
    telemetry: Res<ArenaTestTelemetry>,
    mut commands: Commands,
) {
    if cfg.revision == telemetry.built_revision {
        return;
    }
    commands.run_system_cached(sys_build_arena);
}

// ─── Joueur, ambiante, affichage ─────────────────────────────────────────────

/// Pose le joueur au spawn déclaré. Une seule fois par construction : ensuite il
/// est libre (sinon on le téléporterait en boucle).
fn sys_place_player(
    mut commands: Commands,
    pending: Option<Res<ArenaTestSpawnPending>>,
    cfg: Res<ArenaTestConfig>,
    mut q_player: Query<(&mut Transform, &mut Player)>,
) {
    if pending.is_none() {
        return;
    }
    let Ok((mut tf, mut player)) = q_player.single_mut() else {
        return; // joueur pas encore spawné (OnEnter AppMode::InGame) → on retente
    };
    let a = &cfg.genome.arena;
    tf.translation = Vec3::from(a.spawn_pos);
    player.yaw = a.spawn_yaw_deg.to_radians();
    tf.rotation = Quat::from_rotation_y(player.yaw);
    commands.remove_resource::<ArenaTestSpawnPending>();
    info!(
        "[arena-test] joueur posé au spawn ({:.1}, {:.1}, {:.1})",
        a.spawn_pos[0], a.spawn_pos[1], a.spawn_pos[2]
    );
}

/// Ambiante posée sur les caméras 3D (même approche que la démo Cyber City :
/// robuste à un respawn de caméra, idempotente par marqueur).
fn sys_ensure_ambient(
    mut commands: Commands,
    cfg: Res<ArenaTestConfig>,
    q_cam: Query<Entity, (With<Camera3d>, Without<ArenaTestAmbientApplied>)>,
) {
    let l = cfg.genome.lighting;
    for cam in &q_cam {
        commands.entity(cam).insert((
            ArenaTestAmbientApplied,
            AmbientLight {
                color: Color::srgb(l.ambient_color[0], l.ambient_color[1], l.ambient_color[2]),
                brightness: l.ambient_brightness,
                ..default()
            },
        ));
    }
}

fn sys_toggle_view(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<ArenaTestView>) {
    if keys.just_pressed(KeyCode::F2) {
        view.show_grid = !view.show_grid;
    }
    if keys.just_pressed(KeyCode::F4) {
        view.show_metrics = !view.show_metrics;
    }
}

/// Grille au sol à l'échelle du module. Sans échelle visible, on construit au
/// jugé — et au jugé, tout paraît trop grand une fois en jeu.
fn sys_draw_grid(mut gizmos: Gizmos, cfg: Res<ArenaTestConfig>, view: Res<ArenaTestView>) {
    if !view.show_grid || !cfg.genome.grid.show {
        return;
    }
    let grid = cfg.genome.grid;
    let cells = grid.cell_count.max(1);
    gizmos.grid(
        Isometry3d::new(
            // Légèrement au-dessus du sol : sinon la grille z-fight avec la dalle.
            Vec3::new(0.0, 0.02, 0.0),
            Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        ),
        UVec2::splat(cells),
        Vec2::splat(grid.module_m),
        Color::srgba(0.95, 0.95, 1.0, 0.16),
    );
}

// ─── Encart metrics ──────────────────────────────────────────────────────────

fn sys_metrics_overlay(
    mut contexts: EguiContexts,
    cfg: Res<ArenaTestConfig>,
    view: Res<ArenaTestView>,
    telemetry: Res<ArenaTestTelemetry>,
) {
    if !view.show_metrics {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let m = cfg.genome.metrics;
    let grid = cfg.genome.grid;
    let arena = &cfg.genome.arena;
    egui::Window::new(format!("Arena Test — {}", arena.name))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(16.0, 16.0))
        .resizable(false)
        .collapsible(true)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new("GREYBOX — on juge la forme, pas l'art")
                    .italics()
                    .weak(),
            );
            ui.separator();

            egui::Grid::new("arena_test_metrics")
                .num_columns(2)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    let mut row = |k: &str, v: String| {
                        ui.label(k);
                        ui.label(egui::RichText::new(v).strong());
                        ui.end_row();
                    };
                    row("Hauteur de saut", format!("{:.2} m", m.jump_height_m));
                    row(
                        "Portée saut (marche)",
                        format!("{:.2} m", m.jump_reach_walk_m),
                    );
                    row(
                        "Portée saut (sprint)",
                        format!("{:.2} m", m.jump_reach_sprint_m),
                    );
                    row(
                        "Vitesse marche / sprint",
                        format!("{:.1} / {:.1} m/s", m.walk_speed_ms, m.sprint_speed_ms),
                    );
                    row("Gabarit joueur", format!("{:.1} m", m.player_height_m));
                    row("Module de grille", format!("{:.1} m", grid.module_m));
                    row(
                        "Couverture basse / haute",
                        format!("{:.1} / {:.1} m", grid.cover_low_m, grid.cover_high_m),
                    );
                    row(
                        "Bande de combat",
                        format!("{:.0} – {:.0} m", m.engagement_close_m, m.engagement_core_m),
                    );
                    row(
                        "Altitude joueur",
                        format!("{:+.2} m", telemetry.player_altitude_m),
                    );
                    row(
                        "Rampe la plus raide",
                        format!("{:.1}°", telemetry.steepest_ramp_deg),
                    );
                    row(
                        "Fosse vue du point haut",
                        format!("{:.0} %", telemetry.worst_overlook_pct),
                    );
                    row(
                        "Point haut contestable ≤",
                        format!(
                            "{:.1} m",
                            max_contestable_height(grid.cover_high_m, m.eye_height_m)
                        ),
                    );
                    row(
                        "Plan logique",
                        format!(
                            "{} salles · {} routes",
                            cfg.genome.rooms.len(),
                            cfg.genome.routes.len()
                        ),
                    );
                    row(
                        "Requis / posé à mi-portée",
                        format!(
                            "{:.1} m / {:.1} m",
                            telemetry.cover_needed_at_midspan_m, telemetry.tallest_occluder_m
                        ),
                    );
                });

            // Les voies et leur portée visée. C'est la lecture que le joueur doit
            // avoir depuis le spawn (« au plus trois décisions ») — l'afficher ici
            // permet de vérifier qu'on construit bien la voie qu'on croit.
            if !cfg.genome.lanes.is_empty() {
                ui.separator();
                for l in &cfg.genome.lanes {
                    ui.label(format!(
                        "{}  —  portée {}  (Z {:.0} → {:.0})",
                        l.name, l.range, l.z_min, l.z_max
                    ));
                }
            }

            ui.separator();
            ui.label(format!(
                "{} pavés · {} disques · {} rampes · {} murs",
                telemetry.blocks_spawned,
                telemetry.discs_spawned,
                telemetry.ramps_spawned,
                telemetry.walls_spawned
            ));

            if !cfg.load_error.is_empty() {
                ui.colored_label(egui::Color32::from_rgb(230, 90, 90), &cfg.load_error);
            }
            for d in &cfg.metrics_drift {
                ui.colored_label(egui::Color32::from_rgb(235, 175, 70), d);
            }
            for error in &cfg.route_errors {
                ui.colored_label(
                    egui::Color32::from_rgb(230, 90, 90),
                    format!("Contrat de route : {error}"),
                );
            }

            ui.separator();
            ui.label(
                egui::RichText::new("F2 grille · F4 cet encart · sauvegarder le TOML reconstruit")
                    .weak()
                    .small(),
            );
        });
}

// ─── Capteur ─────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct ArenaTestSensor<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    arena_name: &'a str,
    shape: &'a str,
    bounds_x_m: f32,
    bounds_z_m: f32,
    blocks_spawned: u32,
    discs_spawned: u32,
    ramps_spawned: u32,
    walls_spawned: u32,
    steepest_ramp_deg: f32,
    tallest_traversal_step_m: f32,
    traversal_blocks: u32,
    floor_margin_m: f32,
    worst_overlook_pct: f32,
    cover_needed_at_midspan_m: f32,
    covers_in_band: u32,
    covers_dead_band: u32,
    covers_expected: f32,
    tallest_occluder_m: f32,
    ceilings_spawned: u32,
    lights_spawned: u32,
    dark_rooms: u32,
    min_headroom_m: f32,
    jump_height_m: f32,
    player_altitude_m: f32,
    player_x_m: f32,
    player_z_m: f32,
    player_room: &'a str,
    rooms_declared: usize,
    routes_declared: usize,
    metrics_drift: &'a [String],
    route_errors: &'a [String],
    load_error: &'a str,
    next_step: String,
}

/// Sévérité + remédiation. Pur pour être testable sans moteur.
///
/// Les pannes visées rendent le banc faux **sans jamais crasher** : TOML
/// illisible (arène vide), sol qui n'atteint pas les sommets de l'hexagone
/// (chute hors-map dans les coins), marche plus haute que le saut (route
/// infranchissable). Les trois se lisent en jeu comme « un bug de collision »
/// alors que ce sont des erreurs de donnée — d'où l'alerte nommée.
pub fn arena_test_health(
    load_error: &str,
    blocks_spawned: u32,
    floor_margin_m: f32,
    traversal_blocks: u32,
    tallest_step_m: f32,
    jump_height_m: f32,
    overlook_pct: f32,
    dark_rooms: u32,
    first_dark_room: &str,
    min_headroom_m: f32,
    player_height_m: f32,
    drift_count: usize,
) -> (&'static str, String) {
    if !load_error.is_empty() {
        return (
            "critical",
            format!(
                "Genome illisible : {load_error}. Corriger la syntaxe de \
                 {GENOME_PATH} puis sauvegarder — le banc reconstruit tout seul."
            ),
        );
    }
    if blocks_spawned == 0 {
        return (
            "critical",
            format!(
                "Aucune géométrie posée. Vérifier que {GENOME_PATH} contient des \
                 entrées [[blocks]] / [[discs]] / [[ramps]]."
            ),
        );
    }
    // Une salle couverte sans lumière est NOIRE : le soleil ne traverse pas la
    // dalle. C'est la panne la plus certaine dès qu'on pose des plafonds, et
    // elle se lit en jeu comme « le niveau n'a pas chargé ».
    if dark_rooms > 0 {
        return (
            "critical",
            format!(
                "{dark_rooms} salle(s) couverte(s) sans aucune lumière — dont \
                 `{first_dark_room}`. Elles seront NOIRES : une dalle de plafond \
                 arrête le soleil directionnel. Ajouter des entrées [[lights]] \
                 dans ces salles, ou repasser leur `ceiling_m` à 0 (ciel ouvert)."
            ),
        );
    }
    // Une salle où l'on se cogne n'est pas jouable : il faut la taille du joueur
    // plus de quoi sauter, sinon le saut devient inutilisable sous plafond.
    let needed_headroom = player_height_m + jump_height_m;
    if min_headroom_m > 0.0 && min_headroom_m < needed_headroom {
        return (
            "warn",
            format!(
                "Hauteur libre minimale {min_headroom_m:.1} m < {needed_headroom:.1} m \
                 requis (joueur {player_height_m:.1} + saut {jump_height_m:.2}) : \
                 le joueur se cogne au plafond en sautant. Monter `ceiling_m` de \
                 la salle concernée."
            ),
        );
    }
    // Le sol doit dépasser l'enceinte, pas l'effleurer : sinon il subsiste une
    // bordure sans plancher le long des murs, et l'on y tombe hors de la carte.
    if floor_margin_m < 0.0 {
        return (
            "warn",
            format!(
                "Le sol s'arrête {:.1} m AVANT l'enceinte : il existe une bordure \
                 sans plancher où le joueur tombe hors de la carte. Élargir la \
                 pièce role=\"floor\" au-delà de l'emprise déclarée.",
                -floor_margin_m
            ),
        );
    }
    // Un contrôle sans matière doit se déclarer AVEUGLE, pas vert. Sinon
    // `tallest_traversal_step_m = 0.0` se lit « marches vérifiées » alors qu'il
    // n'y a aucune marche — c'est exactement ainsi qu'un capteur ment.
    if traversal_blocks == 0 {
        return (
            "info",
            "Aucune pièce role=\"traversal\" : le contrôle des marches n'a RIEN              à mesurer (il n'est pas vert, il est aveugle). Si la carte est censée              demander des sauts, aucune chaîne n'est posée — le saut du joueur ne              sert alors nulle part."
                .to_string(),
        );
    }
    if tallest_step_m > jump_height_m && jump_height_m > 0.0 {
        return (
            "warn",
            format!(
                "Une marche de traversée fait {tallest_step_m:.2} m alors que le \
                 saut plafonne à {jump_height_m:.2} m : la route est \
                 infranchissable. Abaisser la marche dans [[blocks]] role=\"traversal\"."
            ),
        );
    }
    // Un point haut qui voit tout n'est plus un objectif : il est acquis dès
    // qu'on y monte, et les autres niveaux perdent leur raison d'être. Seuil à
    // 85 % — au-delà, aucune approche n'est possible à couvert.
    if overlook_pct > 85.0 {
        return (
            "warn",
            format!(
                "Un point haut domine {overlook_pct:.0} % de la fosse : il n'est \
                 pas contesté, il est acquis. Une couverture ne coupe la vue d'un \
                 tireur perché à h que si elle atteint h·(1−x/portée)+œil — à \
                 mi-portée, h/2+1,7. Abaisser le perchoir, l'aveugler côté fosse, \
                 ou monter les couvertures."
            ),
        );
    }
    if drift_count > 0 {
        return (
            "warn",
            format!(
                "{drift_count} metric(s) de {GENOME_PATH} en désaccord avec \
                 player_movement.toml — l'arène est dimensionnée sur des valeurs \
                 périmées. Recopier les valeurs mesurées listées dans metrics_drift."
            ),
        );
    }
    (
        "ok",
        "Banc opérationnel. Jouer l'arène, puis itérer le TOML — il se recharge à chaud."
            .to_string(),
    )
}

fn sys_write_sensor(
    time: Res<Time>,
    cfg: Res<ArenaTestConfig>,
    mut telemetry: ResMut<ArenaTestTelemetry>,
    q_player: Query<&Transform, With<Player>>,
) {
    if let Ok(tf) = q_player.single() {
        telemetry.player_altitude_m = tf.translation.y;
        telemetry.player_x_m = tf.translation.x;
        telemetry.player_z_m = tf.translation.z;
        telemetry.player_room = cfg
            .genome
            .rooms
            .iter()
            .find(|r| {
                (tf.translation.x - r.center[0]).abs() <= r.size[0] * 0.5
                    && (tf.translation.z - r.center[2]).abs() <= r.size[2] * 0.5
            })
            .map(|r| r.id.clone())
            .unwrap_or_default();
    }
    telemetry.sensor_timer += time.delta_secs();
    if telemetry.sensor_timer < SENSOR_PERIOD_SEC {
        return;
    }
    telemetry.sensor_timer = 0.0;

    let (mut severity, mut next_step) = arena_test_health(
        &cfg.load_error,
        telemetry.blocks_spawned,
        telemetry.floor_margin_m,
        telemetry.traversal_blocks,
        telemetry.tallest_traversal_step_m,
        cfg.derived.jump_height_m,
        telemetry.worst_overlook_pct,
        telemetry.dark_rooms,
        &telemetry.first_dark_room,
        telemetry.min_headroom_m,
        cfg.genome.metrics.player_height_m,
        cfg.metrics_drift.len(),
    );
    if !cfg.route_errors.is_empty() {
        severity = "critical";
        next_step = format!(
            "Le plan logique est invalide ({} erreur(s)) : {}",
            cfg.route_errors.len(),
            cfg.route_errors.join(" · ")
        );
    }
    let payload = ArenaTestSensor {
        id: "arena_test",
        severity,
        timestamp_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
        arena_name: &cfg.genome.arena.name,
        shape: &cfg.genome.arena.shape,
        bounds_x_m: cfg.genome.arena.half_extents().x * 2.0,
        bounds_z_m: cfg.genome.arena.half_extents().y * 2.0,
        blocks_spawned: telemetry.blocks_spawned,
        discs_spawned: telemetry.discs_spawned,
        ramps_spawned: telemetry.ramps_spawned,
        walls_spawned: telemetry.walls_spawned,
        steepest_ramp_deg: telemetry.steepest_ramp_deg,
        tallest_traversal_step_m: telemetry.tallest_traversal_step_m,
        traversal_blocks: telemetry.traversal_blocks,
        floor_margin_m: telemetry.floor_margin_m,
        worst_overlook_pct: telemetry.worst_overlook_pct,
        cover_needed_at_midspan_m: telemetry.cover_needed_at_midspan_m,
        covers_in_band: telemetry.covers_in_band,
        covers_dead_band: telemetry.covers_dead_band,
        covers_expected: telemetry.covers_expected,
        tallest_occluder_m: telemetry.tallest_occluder_m,
        ceilings_spawned: telemetry.ceilings_spawned,
        lights_spawned: telemetry.lights_spawned,
        dark_rooms: telemetry.dark_rooms,
        min_headroom_m: telemetry.min_headroom_m,
        jump_height_m: cfg.derived.jump_height_m,
        player_altitude_m: telemetry.player_altitude_m,
        player_x_m: telemetry.player_x_m,
        player_z_m: telemetry.player_z_m,
        player_room: &telemetry.player_room,
        rooms_declared: cfg.genome.rooms.len(),
        routes_declared: cfg.genome.routes.len(),
        metrics_drift: &cfg.metrics_drift,
        route_errors: &cfg.route_errors,
        load_error: &cfg.load_error,
        next_step,
    };
    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
    }
}

// ─── Nettoyage ───────────────────────────────────────────────────────────────

fn sys_cleanup(
    mut commands: Commands,
    q: Query<Entity, With<ArenaTestMarker>>,
    q_cam: Query<Entity, With<ArenaTestAmbientApplied>>,
    mut telemetry: ResMut<ArenaTestTelemetry>,
) {
    let n = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    // L'ambiante du banc ne doit pas fuiter dans le mode suivant.
    for cam in &q_cam {
        commands
            .entity(cam)
            .remove::<(ArenaTestAmbientApplied, AmbientLight)>();
    }
    commands.remove_resource::<ArenaTestSpawnPending>();
    *telemetry = ArenaTestTelemetry::default();
    info!("[arena-test] banc nettoyé : {n} entités despawn");
}

// ─── Tests purs ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_metrics_match_measured_locomotion() {
        // player_movement.toml au 2026-07-27 : 6.5 m/s, saut 6.5 m/s, g 18 m/s².
        let d = derive_metrics(6.5, 1.5, 6.5, 18.0);
        assert!(
            (d.jump_height_m - 1.1736).abs() < 1e-3,
            "{}",
            d.jump_height_m
        );
        assert!((d.air_time_s - 0.7222).abs() < 1e-3, "{}", d.air_time_s);
        assert!((d.jump_reach_walk_m - 4.694).abs() < 1e-2);
        assert!((d.jump_reach_sprint_m - 7.042).abs() < 1e-2);
    }

    #[test]
    fn zero_gravity_yields_no_metrics_instead_of_infinity() {
        let d = derive_metrics(6.5, 1.5, 6.5, 0.0);
        assert_eq!(d.jump_height_m, 0.0);
        assert_eq!(d.air_time_s, 0.0);
    }

    /// Le piège qui a fait échouer la première passe : une caisse de 2 m au sol
    /// n'est pas une marche de 2 m si une caisse de 1 m la précède.
    #[test]
    fn traversal_step_measures_gaps_not_block_heights() {
        // Escalier 1 m → 2 m depuis le sol : trois marches de 1 m.
        assert!((max_traversal_step(&[1.0, 2.0]) - 1.0).abs() < 1e-3);
        // La même caisse de 2 m, seule : la marche vaut bien 2 m.
        assert!((max_traversal_step(&[2.0]) - 2.0).abs() < 1e-3);
        // Doublons (une chaîne répétée en 3 exemplaires) : sans effet.
        assert!((max_traversal_step(&[1.0, 2.0, 1.0, 2.0, 1.0, 2.0]) - 1.0).abs() < 1e-3);
        // Trou au milieu de l'escalier : c'est le trou qui compte.
        assert!((max_traversal_step(&[1.0, 3.5]) - 2.5).abs() < 1e-3);
        // Aucune pièce de traversée : aucune marche.
        assert_eq!(max_traversal_step(&[]), 0.0);
    }

    /// L'inégalité de crête, vérifiée sur des cas où le résultat est évident.
    #[test]
    fn cover_height_follows_the_sightline_inequality() {
        // Obstacle collé à la cible : il suffit qu'il atteigne la tête.
        assert!((cover_height_to_break_sight(6.0, 20.0, 20.0, 1.7) - 1.7).abs() < 1e-3);
        // À mi-portée : h/2 + œil.
        assert!((cover_height_to_break_sight(6.0, 20.0, 10.0, 1.7) - 4.7).abs() < 1e-3);
        // Tireur au sol : l'obstacle n'a qu'à dépasser l'œil, où qu'il soit.
        assert!((cover_height_to_break_sight(0.0, 20.0, 5.0, 1.7) - 1.7).abs() < 1e-3);
        // Portée nulle : aucune ligne de vue à couper.
        assert!(cover_height_to_break_sight(6.0, 0.0, 0.0, 1.7).is_infinite());
    }

    #[test]
    fn contestable_height_is_the_inverse_relation() {
        // Une couverture de 2 m ne conteste qu'un perchoir de 0,6 m. C'est le
        // chiffre qui condamne l'idée de contrer un point haut avec des caisses.
        assert!((max_contestable_height(2.0, 1.7) - 0.6).abs() < 1e-3);
        assert!((max_contestable_height(3.0, 1.7) - 2.6).abs() < 1e-3);
        // Une couverture sous la hauteur d'œil ne conteste rien du tout.
        assert_eq!(max_contestable_height(1.0, 1.7), 0.0);
    }

    #[test]
    fn perimeter_is_four_walls_when_rectangular_six_when_hexagonal() {
        let mut cfg = ArenaCfg::default();
        cfg.shape = "rectangular".into();
        cfg.size_x = 80.0;
        cfg.size_z = 56.0;
        let p = perimeter_poses(&cfg);
        assert_eq!(p.len(), 4);
        // Les deux murs longs portent la largeur totale, angles inclus.
        assert!(p.iter().any(|(_, _, len)| (*len - 82.0).abs() < 1e-3));
        assert!(p.iter().any(|(_, _, len)| (*len - 56.0).abs() < 1e-3));

        cfg.shape = "hexagonal".into();
        cfg.extent_m = 30.0;
        let h = perimeter_poses(&cfg);
        assert_eq!(h.len(), 6);
        for (_, _, len) in &h {
            assert!((len - 30.0).abs() < 1e-3, "{len}");
        }
    }

    #[test]
    fn lane_of_selects_the_band_containing_z() {
        let g = ArenaTestGenome {
            lanes: vec![
                LaneDef {
                    name: "A".into(),
                    z_min: 9.0,
                    z_max: 28.0,
                    range: "longue".into(),
                },
                LaneDef {
                    name: "B".into(),
                    z_min: -9.0,
                    z_max: 9.0,
                    range: "courte".into(),
                },
            ],
            ..Default::default()
        };
        let half = Vec2::new(40.0, 28.0);
        assert_eq!(g.lane_of(18.0, half), (9.0, 28.0));
        assert_eq!(g.lane_of(0.0, half), (-9.0, 9.0));
        // Hors de toute voie déclarée : on retombe sur la carte entière.
        assert_eq!(g.lane_of(-20.0, half), (-28.0, 28.0));
        // Genome sans voies : idem, pas de panique.
        assert_eq!(ArenaTestGenome::default().lane_of(0.0, half), (-28.0, 28.0));
    }

    #[test]
    fn overlook_is_total_without_cover_and_drops_behind_a_wall() {
        let perch = Vec3::new(20.0, 6.0, 0.0);
        let region = (Vec2::splat(-16.0), Vec2::splat(16.0));
        // Sans rien pour couper la vue : la position voit tout.
        assert!((overlook_fraction(perch, 1.7, &[], region, 16) - 1.0).abs() < 1e-6);
        // Un mur de 12 m juste devant elle en masque une bonne part.
        let wall = vec![(Vec3::new(14.0, 0.0, 0.0), 12.0, 8.0)];
        let f = overlook_fraction(perch, 1.7, &wall, region, 16);
        assert!(f < 0.8, "un mur de 12 m devrait masquer largement, vu {f}");
        // Emprise nulle : rien à dominer, pas de division par zéro.
        assert_eq!(
            overlook_fraction(perch, 1.7, &[], (Vec2::ZERO, Vec2::ZERO), 16),
            0.0
        );
    }

    #[test]
    fn drift_detects_stale_declaration() {
        assert!(!drifted(1.174, 1.1736, 0.02));
        assert!(drifted(1.174, 2.5, 0.02));
        // Une valeur déclarée sur un actuel nul est une dérive, pas une division.
        assert!(drifted(1.0, 0.0, 0.02));
        assert!(!drifted(0.0, 0.0, 0.02));
    }

    #[test]
    fn ramp_pose_puts_far_end_up_and_measures_slope() {
        // 8 m d'avancée pour 3 m de montée → ~20,6°.
        let p = ramp_pose(Vec3::new(8.5, 0.0, 0.0), Vec3::new(16.5, 3.0, 0.0), 0.5);
        assert!((p.slope_deg - 20.556).abs() < 0.1, "{}", p.slope_deg);
        assert!((p.length_m - 8.544).abs() < 1e-2, "{}", p.length_m);
        // L'axe long (Z local) doit pointer vers le point haut.
        let axis = p.rotation * Vec3::Z;
        assert!(
            axis.y > 0.0,
            "la rampe descend au lieu de monter : {axis:?}"
        );
        assert!(
            axis.x > 0.9,
            "la rampe ne suit pas la direction from→to : {axis:?}"
        );
    }

    #[test]
    fn flat_ramp_has_no_slope() {
        let p = ramp_pose(Vec3::ZERO, Vec3::new(0.0, 0.0, 10.0), 0.5);
        assert!(p.slope_deg.abs() < 1e-3);
        assert!((p.length_m - 10.0).abs() < 1e-3);
    }

    #[test]
    fn perimeter_is_empty_when_the_arena_has_no_extent() {
        let mut cfg = ArenaCfg::default();
        cfg.shape = "hexagonal".into();
        cfg.extent_m = 0.0;
        assert!(perimeter_poses(&cfg).is_empty());
        cfg.shape = "rectangular".into();
        cfg.size_x = 0.0;
        assert!(perimeter_poses(&cfg).is_empty());
    }

    #[test]
    fn health_flags_unreachable_traversal_step() {
        let (sev, next) = arena_test_health("", 12, 1.0, 12, 1.6, 1.174, 0.0, 0, "", 0.0, 2.0, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("infranchissable"));
    }

    /// Le piège géométrique : un sol taillé sur l'APOTHÈME (26 pour extent 30)
    /// laisse six coins vides. Personne ne le voit avant d'y tomber.
    #[test]
    fn health_flags_floor_stopping_short_of_the_perimeter() {
        // Sol 4 m trop court : il reste une bordure sans plancher le long des murs.
        let (sev, next) = arena_test_health("", 12, -4.0, 12, 1.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("bordure"), "{next}");
        assert!(
            next.contains("4.0"),
            "la marge manquante doit être chiffrée: {next}"
        );
        // Un sol qui déborde l'enceinte ne déclenche rien.
        assert_eq!(arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0).0, "ok");
    }

    /// Poser un plafond sans lumière est LA panne certaine du passage aux salles
    /// fermées : en jeu ça se lit « le niveau n'a pas chargé », pas « il manque
    /// une lumière ». L'alerte doit donc nommer la salle.
    #[test]
    fn health_flags_covered_room_without_light() {
        let (sev, next) = arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 2, "chapelle", 5.0, 2.0, 0);
        assert_eq!(sev, "critical");
        assert!(next.contains("chapelle"), "la salle doit être nommée: {next}");
        assert!(next.contains("NOIRES") || next.contains("noire"), "{next}");
        // Salle couverte ET éclairée : rien à signaler.
        assert_eq!(
            arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 5.0, 2.0, 0).0,
            "ok"
        );
    }

    /// Sous plafond, le saut doit rester utilisable : il faut la taille du joueur
    /// PLUS la hauteur de saut, sinon on se cogne dès qu'on appuie sur espace.
    #[test]
    fn health_flags_ceiling_too_low_to_jump_under() {
        // 2,0 (joueur) + 1,174 (saut) = 3,174 m requis.
        let (sev, next) = arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 2.6, 2.0, 0);
        assert_eq!(sev, "warn");
        assert!(next.contains("cogne"), "{next}");
        // Juste au-dessus du seuil : accepté.
        assert_eq!(
            arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 3.2, 2.0, 0).0,
            "ok"
        );
        // Aucune salle couverte (0) : le contrôle ne doit pas se déclencher.
        assert_eq!(
            arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0).0,
            "ok"
        );
    }

    #[test]
    fn health_flags_empty_and_broken_genome() {
        assert_eq!(
            arena_test_health("boom", 0, 1.0, 12, 0.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0).0,
            "critical"
        );
        assert_eq!(
            arena_test_health("", 0, 1.0, 12, 0.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0).0,
            "critical"
        );
        assert_eq!(arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 0.0, 2.0, 0).0, "ok");
        assert_eq!(arena_test_health("", 12, 1.0, 12, 1.0, 1.174, 0.0, 0, "", 0.0, 2.0, 3).0, "warn");
    }

    #[test]
    fn shipped_genome_parses_and_matches_measured_metrics() {
        // Le blockout livré doit rester lisible ET cohérent avec la locomotion.
        let src = std::fs::read_to_string("../../assets/genomes/arena_test.toml")
            .expect("arena_test.toml présent");
        let g: ArenaTestGenome = toml::from_str(&src).expect("arena_test.toml valide");
        assert!(!g.blocks.is_empty(), "blockout vide");
        assert!(!g.ramps.is_empty(), "aucune rampe");

        let d = derive_metrics(6.5, 1.5, 6.5, 18.0);
        assert!(!drifted(g.metrics.jump_height_m, d.jump_height_m, 0.02));
        assert!(!drifted(
            g.metrics.jump_reach_walk_m,
            d.jump_reach_walk_m,
            0.02
        ));

        // Toute marche de traversée doit rester franchissable — la MARCHE, c'est
        // l'écart entre deux surfaces successives, pas la hauteur d'une caisse.
        let tops: Vec<f32> = g
            .blocks
            .iter()
            .filter(|b| b.role == "traversal")
            .map(|b| b.pos[1] + b.size[1])
            .collect();
        let step = max_traversal_step(&tops);
        assert!(
            step <= d.jump_height_m,
            "marche de {step} m > saut de {:.2} m",
            d.jump_height_m
        );
        // Toute rampe doit rester montable confortablement.
        for r in &g.ramps {
            let p = ramp_pose(Vec3::from(r.from), Vec3::from(r.to), RAMP_SLAB_THICKNESS_M);
            assert!(
                p.slope_deg.abs() < 35.0,
                "rampe à {:.1}° — trop raide",
                p.slope_deg
            );
        }

        // Le sol doit dépasser l'enceinte : sinon il reste une bordure sans
        // plancher le long des murs.
        let half = g.arena.half_extents();
        let mut reach = Vec2::ZERO;
        for b in g.blocks.iter().filter(|b| b.role == "floor") {
            reach = reach.max(Vec2::new(
                b.pos[0].abs() + b.size[0] * 0.5,
                b.pos[2].abs() + b.size[2] * 0.5,
            ));
        }
        for d in g.discs.iter().filter(|d| d.role == "floor") {
            let r = Vec2::new(d.pos[0], d.pos[2]).length() + d.radius_m;
            reach = reach.max(Vec2::splat(r));
        }
        assert!(
            (reach - half).min_element() >= 0.0,
            "sol {reach:?} pour une emprise {half:?} — bordure sans plancher"
        );

        // Trois voies déclarées, chacune avec sa portée : c'est la structure
        // Call of Duty, et l'analyse s'appuie dessus pour mesurer par voie.
        assert_eq!(g.lanes.len(), 3, "carte trois voies attendue");
        let mut ranges: Vec<&str> = g.lanes.iter().map(|l| l.range.as_str()).collect();
        ranges.sort_unstable();
        assert_eq!(
            ranges,
            vec!["courte", "longue", "moyenne"],
            "les trois voies doivent couvrir les trois portées"
        );

        // Le spawn doit être au-dessus du sol : la capsule joueur mesure 2 m,
        // donc son centre ne peut pas être sous 1 m sans naître encastré.
        assert!(
            g.arena.spawn_pos[1] >= g.metrics.player_height_m * 0.5,
            "spawn à Y={} : le joueur naît dans le sol",
            g.arena.spawn_pos[1]
        );
    }

    #[test]
    fn crypt_vertical_scenario_is_playable_from_declared_metrics() {
        let src = std::fs::read_to_string("../../assets/genomes/arena_test_crypte_vertical.toml")
            .expect("arena_test_crypte_vertical.toml présent");
        let g: ArenaTestGenome = toml::from_str(&src).expect("crypte verticale TOML valide");
        assert_eq!(g.arena.name, "Les Cryptes verticales");
        // L'ENCEINTE SUIT LE CONTENU, elle ne le fige pas.
        //
        // Cette assertion figeait `size_x == 120` — une photo, pas un invariant.
        // Elle a sauté le jour où les emprises de salles ont été posées bout à
        // bout : deux salles adjacentes à des hauteurs différentes ne peuvent pas
        // se chevaucher en plan, et supprimer ce chevauchement allonge la chaîne.
        // Figer la taille du monde revenait à interdire la correction.
        //
        // Ce qui se vérifie vraiment : l'enceinte contient le contenu, et le serre
        // — assez large pour tout englober, pas si large qu'elle cache une dérive.
        let half_x = g.arena.size_x * 0.5;
        let half_z = g.arena.size_z * 0.5;
        let debord_x = g
            .rooms
            .iter()
            .map(|r| r.center[0].abs() + r.size[0] * 0.5)
            .fold(0.0_f32, f32::max);
        let debord_z = g
            .rooms
            .iter()
            .map(|r| r.center[2].abs() + r.size[2] * 0.5)
            .fold(0.0_f32, f32::max);
        let module = g.grid.module_m.max(0.5);
        for (axe, half, debord) in [('x', half_x, debord_x), ('z', half_z, debord_z)] {
            assert!(
                half >= debord,
                "enceinte trop petite en {axe} : demi-largeur {half} < débord des salles {debord}"
            );
            // 3 modules : l'épaisseur de paroi admissible (le creusement ouvre
            // déjà salle + 1 m, le reste est de la roche porteuse).
            assert!(
                half - debord <= 3.0 * module,
                "enceinte trop lâche en {axe} : {} m de roche morte au-delà des salles \
                 (max {} m). L'enceinte se DÉRIVE du contenu, elle ne se choisit pas.",
                half - debord,
                3.0 * module
            );
        }
        for room in &g.rooms {
            let diag = room.size[0].hypot(room.size[2]);
            assert!(
                diag <= 34.5,
                "salle `{}` diag {:.1} m > barème combat ~34 m",
                room.id,
                diag
            );
        }
        assert!(
            g.blocks.iter().any(|b| b.role == "perch") || g.discs.iter().any(|d| d.role == "perch"),
            "au moins une position de force attendue"
        );
        // Le tunnel de contournement doit exister ET être construit. Depuis la
        // passe « donjon » il ne s'exprime plus en blocs taggés `tunnel` : c'est
        // une salle bornée par la roche, avec une chicane qui casse sa ligne de
        // vue (62 m → 30 m). On vérifie donc la salle et sa chicane.
        let tunnel = g
            .rooms
            .iter()
            .find(|r| r.id == "tunnel")
            .expect("salle `tunnel` (contournement) attendue");
        assert!(
            tunnel.ceiling_m > 0.0,
            "le tunnel est un boyau : il doit être couvert"
        );
        assert!(
            g.lights.iter().any(|l| l.room == "tunnel"),
            "tunnel couvert sans lumière : il serait noir"
        );
        assert!(
            g.blocks
                .iter()
                .filter(|b| b.section == "chicane_tunnel")
                .count()
                >= 3,
            "chicane attendue : sans elle le tunnel offre 62 m de ligne droite, \
             ce qui en fait un stand de tir et non un contournement"
        );
        assert!(g.ramps.len() >= 6, "routes sûres attendues (≥6 rampes de liaison)");
        // Toute route doit offrir le couloir qu'elle déclare. Ce contrat a été
        // un cliquet de dette borné à 15 obstructions ; il est désormais tenu, et
        // l'exigence est franche. Historique, pour ne pas refaire le chemin :
        //
        // - Le contrôle testait un fil de 0,6 m (`player_radius_m`) au lieu du
        //   canal utile : il ne voyait rien et passait au vert sur n'importe quoi.
        // - Réparé, il gonflait l'AABB du pavé du rayon joueur — or la capsule
        //   est un DISQUE : 3 des obstructions annoncées étaient de faux positifs
        //   de coin (jusqu'à `r·√2`, soit +41 %). D'où la distance exacte.
        // - Générateur : `snap()` réalignait les pavés de roche déjà exacts et les
        //   déplaçait de 1 m une bande sur deux — la roche bouchait un couloir ici
        //   et laissait un trou là. C'était la cause dominante des 8 `roche`.
        // - Générateur : le creusement dilatait de 1,0 m sur une trame de 2 m,
        //   donc les coins de cellule mordaient de 0,41 m dans les couloirs.
        // - Carte : passage de chicane à 4,0 m pour 5 déclarés, un 4ᵉ mur de
        //   chicane en îlot à 1,0 m de l'axe, 1 couverture et 1 pilier tangents.
        //
        // Ne JAMAIS regagner le vert en abaissant le seuil du contrôle : ce serait
        // recréer le capteur menteur d'origine.
        let errs = route_contract_errors(&g);
        assert!(
            errs.is_empty(),
            "{} route(s) ne tiennent pas la largeur déclarée : {errs:?}",
            errs.len()
        );
        // Ces erreurs-là ne sont pas de la dette tolérable : elles cassent le plan.
        for e in &errs {
            assert!(
                !e.contains("inaccessible") && !e.contains("sort de l'emprise"),
                "erreur de plan bloquante : {e}"
            );
        }

        let d = derive_metrics(6.5, 1.5, 6.5, 18.0);
        let steps: Vec<f32> = g
            .blocks
            .iter()
            .filter(|b| b.role == "traversal")
            .map(|b| b.pos[1] + b.size[1])
            .collect();
        assert!(
            max_traversal_step(&steps) <= d.jump_height_m,
            "escaliers de récompense impraticables"
        );
        for ramp in &g.ramps {
            let pose = ramp_pose(
                Vec3::from(ramp.from),
                Vec3::from(ramp.to),
                RAMP_SLAB_THICKNESS_M,
            );
            assert!(
                pose.slope_deg.abs() <= MAX_COMFORTABLE_RAMP_SLOPE_DEG,
                "{} à {:.1}° : pente trop raide",
                ramp.section,
                pose.slope_deg
            );
        }
        assert!(
            g.arena.spawn_pos[1] >= g.metrics.player_height_m * 0.5,
            "spawn sous le sol"
        );
    }

    #[test]
    fn route_contract_rejects_a_disconnected_or_narrow_plan() {
        let src = r#"
            [metrics]
            player_radius_m = 0.3
            [[rooms]]
            id = "spawn"
            role = "spawn"
            center = [0.0, 0.0, 0.0]
            size = [10.0, 2.0, 10.0]
            [[rooms]]
            id = "boss"
            role = "boss"
            center = [30.0, 0.0, 0.0]
            size = [10.0, 2.0, 10.0]
            [[routes]]
            id = "couloir"
            from = "spawn"
            to = "absente"
            width_m = 0.5
        "#;
        let g: ArenaTestGenome = toml::from_str(src).expect("fixture TOML valide");
        let errors = route_contract_errors(&g);
        assert!(errors.iter().any(|e| e.contains("room absente")));
        assert!(errors.iter().any(|e| e.contains("inaccessible")));
    }

    #[test]
    fn route_clearance_rejects_a_cover_on_the_promised_path() {
        let cover = Block {
            pos: [5.0, 0.0, 0.0],
            size: [2.0, 2.0, 2.0],
            role: "cover".to_string(),
            section: "fixture".to_string(),
            yaw_deg: 0.0,
        };
        assert!(segment_hits_block_clearance(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            &cover,
            0.3,
        ));
        assert!(!segment_hits_block_clearance(
            Vec3::new(0.0, 0.0, 3.0),
            Vec3::new(10.0, 0.0, 3.0),
            &cover,
            0.3,
        ));

        let diagonal_cover = Block {
            yaw_deg: 45.0,
            ..cover
        };
        assert!(segment_hits_block_clearance(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            &diagonal_cover,
            0.3,
        ));
    }

    /// Le compte d'abris se DÉRIVE de la surface, il ne s'estime pas.
    ///
    /// Espacement sourcé 3–10 m, 10 m maximum (Watch Dogs, Gears of War). À 6 m,
    /// la cour (22 × 22 = 484 m²) en réclame ~13 — elle en a 4 aujourd'hui, soit
    /// un facteur 3. Le chiffre n'est donc pas une opinion sur la densité.
    #[test]
    fn cover_count_derives_from_area_not_taste() {
        assert!((covers_expected(484.0, 6.0) - 13.44).abs() < 0.01);
        // Espacement au plafond de la bande sourcée → moins d'abris, jamais zéro.
        assert!(covers_expected(484.0, 10.0) < covers_expected(484.0, 6.0));
        // Pas de division par zéro ni de compte négatif sur une salle vide.
        assert_eq!(covers_expected(484.0, 0.0), 0.0);
        assert_eq!(covers_expected(0.0, 6.0), 0.0);
    }

    /// Un bloc ne doit pas mentir sur ce qu'il est.
    ///
    /// Origine : 16 blocs portaient `role = "cover"` en faisant 3, 5 ou 6 m —
    /// **zéro** dans la bande déclarée 1,8–2,8 m. La carte annonçait 16
    /// couvertures et n'en offrait aucune : tout abri était une occultation
    /// totale, donc **aucune mécanique de peek nulle part**. Le rôle se dérive
    /// désormais de la hauteur dans le générateur ; ce test verrouille l'accord.
    #[test]
    fn cover_role_matches_the_declared_band() {
        let src = std::fs::read_to_string("../../assets/genomes/arena_test_crypte_vertical.toml")
            .expect("arena_test_crypte_vertical.toml présent");
        let g: ArenaTestGenome = toml::from_str(&src).expect("crypte verticale TOML valide");
        let (jump, low, high) = (
            g.metrics.jump_height_m,
            g.grid.cover_low_m,
            g.grid.cover_high_m,
        );
        assert!(
            jump < low && low < high,
            "bande incohérente : saut {jump}, couverture {low}..{high}"
        );

        for b in g.blocks.iter().filter(|b| b.role == "cover") {
            let h = b.size[1];
            assert!(
                (low..=high).contains(&h),
                "`{}` porte role=\"cover\" en faisant {h} m — hors bande [{low}, {high}]. \
                 Sous {low} m il ne cache rien (œil à {}), au-dessus de {high} m c'est un mur.",
                b.section,
                g.metrics.eye_height_m
            );
        }

        // La zone morte est le pire cas : ni marche, ni abri. Rien ne doit y tomber.
        let hauteurs: Vec<f32> = g
            .blocks
            .iter()
            .filter(|b| matches!(b.role.as_str(), "cover" | "traversal"))
            .map(|b| b.size[1])
            .collect();
        let bandes = cover_bands(&hauteurs, jump, low, high);
        assert_eq!(
            bandes.dead_band, 0,
            "{} volume(s) entre {jump} et {low} m : ils masquent le corps sans casser \
             la vue, donc ils ne servent ni à monter ni à se couvrir",
            bandes.dead_band
        );
    }

    /// La capsule est un DISQUE. Gonfler l'AABB du rayon place son coin à
    /// `r·√2` du pavé, donc le contrôle déclarait obstruées des routes qui
    /// passent large dans la diagonale — la classe même de capteur menteur que
    /// cette story traque, mais côté faux positif.
    #[test]
    fn route_clearance_does_not_over_report_on_a_diagonal_corner() {
        let pillar = Block {
            pos: [0.0, 0.0, 0.0],
            size: [2.0, 2.0, 2.0],
            role: "cover".to_string(),
            section: "fixture".to_string(),
            yaw_deg: 0.0,
        };
        // Segment porté par `x + z = 4` : sa distance au coin (1, 1) vaut
        // √2 ≈ 1.414 m, et le point le plus proche (2, 2) est bien dessus.
        let start = Vec3::new(1.0, 0.0, 3.0);
        let end = Vec3::new(3.0, 0.0, 1.0);

        // 1.2 m exigé < 1.414 m disponible → la route passe. L'AABB gonflée,
        // elle, mordait dès 1.0 m (coin en (2.2, 2.2), somme au-delà de 4).
        assert!(!segment_hits_block_clearance(start, end, &pillar, 1.2));
        // 1.5 m exigé > 1.414 m disponible → vraie obstruction, toujours vue.
        assert!(segment_hits_block_clearance(start, end, &pillar, 1.5));
    }

    /// Le point haut doit être une position à CONQUÉRIR : ni un acquis (il voit
    /// tout), ni un piège (il ne voit rien et personne n'y monte). Les deux
    /// échecs sont silencieux en jeu — d'où l'encadrement mécanique.
    #[test]
    fn shipped_arena_high_ground_is_contested() {
        let src = std::fs::read_to_string("../../assets/genomes/arena_test.toml")
            .expect("arena_test.toml présent");
        let g: ArenaTestGenome = toml::from_str(&src).expect("arena_test.toml valide");

        // Occultants = TOUTE la géométrie solide, blocs et disques confondus.
        let mut occluders: Vec<CoverDisc> = g
            .blocks
            .iter()
            .map(|b| {
                (
                    Vec3::from(b.pos),
                    b.size[1],
                    Vec2::new(b.size[0], b.size[2]).length() * 0.5,
                )
            })
            .collect();
        occluders.extend(
            g.discs
                .iter()
                .map(|d| (Vec3::from(d.pos), d.height_m, d.radius_m)),
        );
        let half = g.arena.half_extents();
        let perches: Vec<Vec3> = g
            .blocks
            .iter()
            .filter(|b| b.role == "perch")
            .map(|b| Vec3::new(b.pos[0], b.pos[1] + b.size[1], b.pos[2]))
            .collect();
        assert!(!perches.is_empty(), "aucune position de force");

        for p in &perches {
            // Mesurée SUR SA VOIE : une position de force tient sa voie, pas la
            // carte. Rapporter à la carte entière noierait le signal.
            let f = overlook_fraction(
                *p,
                g.metrics.eye_height_m,
                &occluders,
                g.region_of(*p, half),
                44,
            );
            assert!(
                f < 0.85,
                "position de force à +{} m : elle voit {:.0} % de sa voie — c'est \
                 un acquis, pas un objectif",
                p.y,
                f * 100.0
            );
            assert!(
                f > 0.15,
                "position de force à +{} m : elle ne voit que {:.0} % de sa voie — \
                 personne n'ira jamais y monter",
                p.y,
                f * 100.0
            );
        }

        // La contre-épreuve géométrique : il doit exister, dans l'arène, une
        // structure assez haute pour contester le point haut le plus élevé.
        // Sans elle, la domination ne serait basse que par accident de placement.
        let highest = perches
            .iter()
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .copied()
            .expect("au moins un point haut");
        // Portée retenue : la longueur de sa voie, soit la moitié de la carte —
        // ce qu'un tireur posé là couvre réellement devant lui.
        let span = half.x;
        let needed =
            cover_height_to_break_sight(highest.y, span, span * 0.5, g.metrics.eye_height_m);
        let tallest = occluders.iter().map(|&(_, h, _)| h).fold(0.0_f32, f32::max);
        assert!(
            tallest >= needed,
            "structure la plus haute {tallest:.1} m < {needed:.1} m requis pour \
             contester une position à +{:.1} m à mi-portée",
            highest.y
        );
    }

    /// Chaque voie doit avoir un profil de portée DISTINCT — c'est la doctrine
    /// Call of Duty (« every lane needs a purpose ») et la demande explicite.
    /// Trois voies au même profil, c'est une seule voie répétée trois fois.
    #[test]
    fn shipped_lanes_have_distinct_range_profiles() {
        let src = std::fs::read_to_string("../../assets/genomes/arena_test.toml")
            .expect("arena_test.toml présent");
        let g: ArenaTestGenome = toml::from_str(&src).expect("arena_test.toml valide");
        let half = g.arena.half_extents();

        // Densité au sol par voie : elle doit croître de la voie longue vers la
        // courte. Une voie de tireur d'élite encombrée n'est plus une voie longue.
        let mut by_range: Vec<(String, f32)> = Vec::new();
        for lane in &g.lanes {
            let occupied: f32 = g
                .blocks
                .iter()
                .filter(|b| {
                    b.role != "floor"
                        && b.section != "enceinte"
                        && b.pos[2] > lane.z_min
                        && b.pos[2] < lane.z_max
                })
                .map(|b| b.size[0] * b.size[2])
                .sum();
            let area = (lane.z_max - lane.z_min) * half.x * 2.0;
            by_range.push((lane.range.clone(), occupied / area.max(1.0)));
        }
        let get = |r: &str| {
            by_range
                .iter()
                .find(|(k, _)| k == r)
                .map(|(_, v)| *v)
                .unwrap_or(0.0)
        };
        assert!(
            get("longue") < get("moyenne") && get("moyenne") < get("courte") + 0.10,
            "densités longue {:.2} / moyenne {:.2} / courte {:.2} — la voie longue \
             doit rester la plus dégagée",
            get("longue"),
            get("moyenne"),
            get("courte")
        );

        // Positions de force en miroir : à chaque bâtiment son opposé (doctrine
        // Treyarch). Sans ça un camp part avec un avantage structurel.
        let perches: Vec<[f32; 3]> = g
            .blocks
            .iter()
            .filter(|b| b.role == "perch")
            .map(|b| b.pos)
            .collect();
        assert!(perches.len() >= 2 && perches.len().is_multiple_of(2));
        for p in &perches {
            assert!(
                perches
                    .iter()
                    .any(|q| (q[0] + p[0]).abs() < 0.1 && (q[2] - p[2]).abs() < 0.1),
                "position de force en {p:?} sans miroir : un camp est avantagé"
            );
        }
    }
}

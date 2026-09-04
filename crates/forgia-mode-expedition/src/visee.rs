//! visee.rs — Viser à la 3ᵉ personne : la tenue change, la vue se resserre.
//!
//! Un seul état — `Visee.facteur`, de 0 (décontracté) à 1 (tactique) — commande
//! trois choses qui doivent rester d'accord :
//!
//! 1. **La tenue du personnage** : bras levés, buste tourné vers la cible.
//! 2. **Le champ de vision** de la caméra d'épaule, resserré selon l'arme.
//! 3. **Le réticule**, qui passe de la croix au point rouge.
//!
//! Les trois lisent le MÊME facteur. C'est ce qui évite le défaut classique :
//! une vue qui zoome pendant que le personnage garde son arme le long du corps.
//!
//! # Pourquoi la pose est superposée, et pas jouée
//!
//! Le corps d'Expédition porte 9 clips, et **aucun n'est une visée**. Attendre
//! un clip « aim » reviendrait à ne rien livrer. On ajoute donc une rotation par
//! os PAR-DESSUS ce que l'animation vient d'écrire, entre `AnimationSystems` et
//! la propagation des transforms. C'est l'« aim offset » des moteurs du marché,
//! et c'est ce qui permet de viser **en courant** : le clip de course continue
//! de jouer sous la pose.
//!
//! # Ce que ce module ne fait PAS
//!
//! Il ne touche ni à la distance de la caméra (le zoom est optique, pas un
//! rapprochement — et écrire `OrbitCamera::distance` se battrait avec la molette
//! de l'utilisateur), ni à la précision du tir : viser change la lecture et la
//! tenue, pas la dispersion. Lier les deux se fera au génome d'arme, là où les
//! chiffres de combat vivent déjà.

use std::collections::HashMap;

// `bevy_animation` importe ce set depuis `bevy_app` sans le ré-exporter : c'est
// donc là qu'il faut le prendre. Il enferme `advance_animations` et
// `animate_targets`, qui écrivent les transforms des os.
use bevy::app::AnimationSystems;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use serde::Deserialize;

use forgia_camera_orbit::OrbitCamera;
use forgia_combat::weapons::EquippedWeapons;
// `forgia-ik` a été retiré du chemin de visée le 2026-08-17. Son `two_bone_ik`
// rend `mid_rotation` comme une flexion ABSOLUE ; la composer sur la rotation
// courante du coude, frame après frame, faisait visser le bras. Le placement du
// coude se calcule ici (`coude_vise`) : une fonction pure de l'épaule, de la
// cible et des longueurs d'os, qui ne dépend d'aucune sortie précédente.
//
// La crate reste juste — c'est son contrat que j'avais mal lu. Le foot IK
// continue de s'en servir.
use forgia_core::prelude::{AppMode, GameMode, GameSet};
use forgia_genome_core::Genome;

use crate::arme_main::{ArmeMainGenome, ArmeMainGenomeHandle};

/// Le bouton qui vise. Le droit, comme partout ailleurs dans le jeu.
const BOUTON_VISEE: MouseButton = MouseButton::Right;

/// L'os dont la rotation suit le tangage de la caméra.
///
/// Le buste, et non la tête : tourner la tête ferait regarder ailleurs sans
/// bouger l'arme, qui pend au bout du bras. C'est la chaîne épaule → bras →
/// main qui porte le canon, donc c'est le buste qu'il faut incliner.
const OS_DU_BUSTE: &str = "Spine2";

/// Bornes du suivi de buste (radians).
///
/// # 🚨 Ce que ces bornes portent vraiment — et pourquoi elles ont baissé
///
/// Elles valaient ±0,7 rad, soit **±40°**. Deux choses pendent de `Spine2` et ne
/// peuvent pas absorber ça :
///
/// - la **cape** (`cloak_01 < root.001 < Spine2`, vérifié dans le glTF) : elle a
///   ses six os à elle, articulés pour se balancer, pas pour encaisser un
///   basculement de quarante degrés de leur point d'accroche. Rapporté en jeu le
///   2026-08-17 — « la cape est mal raccordée » ;
/// - les **cheveux** du corps actif (`stylized_male_cheveux.glb`), même montage.
///
/// ±20° est ce qu'un torse humain donne en visant sans que les épaules partent.
/// Le reste du tangage appartient à la caméra, pas au personnage : le réticule
/// monte déjà, le canon suit par le bras, et le buste n'a pas à faire le travail
/// des deux.
const BUSTE_MIN_RAD: f32 = -0.35;
const BUSTE_MAX_RAD: f32 = 0.35;

// ---------------------------------------------------------------------------
// Couche definition
// ---------------------------------------------------------------------------

/// Réglages de visée, lus dans `expedition_arme_main.toml`, section `[visee]`.
#[derive(Deserialize, Debug, Clone)]
pub struct ViseeGenome {
    #[serde(default = "montee_par_defaut")]
    pub montee_s: f32,
    #[serde(default = "descente_par_defaut")]
    pub descente_s: f32,
    #[serde(default = "suivi_par_defaut")]
    pub suivi_du_buste: f32,
    #[serde(default)]
    pub bras: BrasGenome,
    #[serde(default)]
    pub pose_decontractee: HashMap<String, [f32; 3]>,
    #[serde(default)]
    pub pose_tactique: HashMap<String, [f32; 3]>,
    /// Au-delà de cet écart canon↔regard, l'arme est tenue à l'envers.
    ///
    /// Ces deux seuils vivent au génome parce qu'ils décident d'une ALERTE, et
    /// qu'un seuil d'alerte se règle sans recompiler — sinon on le laisse faux.
    /// Ils étaient écrits en dur dans le capteur ; le génome se recharge à
    /// chaud, donc on peut les resserrer pendant qu'on regarde le symptôme.
    #[serde(default = "ecart_alerte_par_defaut")]
    pub ecart_alerte_deg: f32,
    /// Au-delà de celui-ci, elle se tient de travers sans être retournée.
    #[serde(default = "ecart_avertit_par_defaut")]
    pub ecart_avertit_deg: f32,
}

/// Un quart de tour : au-delà, le canon a franchi le plan de l'épaule et ne
/// peut plus désigner quoi que ce soit de ce que le personnage regarde.
fn ecart_alerte_par_defaut() -> f32 {
    90.0
}

/// L'écart à partir duquel la lueur de bouche sort visiblement à côté du
/// réticule — mesuré le 2026-08-16 : 71,7° donnait « les bras dans le dos ».
fn ecart_avertit_par_defaut() -> f32 {
    35.0
}

/// Où va la MAIN quand on vise — en fractions de la longueur du bras.
///
/// # Pourquoi une cible et pas des angles
///
/// La première version déclarait douze angles d'Euler, os par os. Ça ne peut pas
/// marcher : l'orientation locale d'un os Mixamo ne se lit pas dans un fichier,
/// et l'axe qui lève un bras sur un rig le fait pivoter sur un autre. Résultat
/// mesuré le 2026-08-16 après trois réglages à l'œil — **les bras dans le dos**,
/// et 71,7° d'écart entre le canon et le regard.
///
/// On déclare donc **où la main doit être**, et un solveur d'IK à deux os en
/// déduit les angles. Il travaille sur les positions RÉELLES des os : il est
/// juste sur n'importe quel squelette, sans rien supposer de ses conventions.
///
/// Les fractions rendent le réglage indépendant de la taille du personnage.
#[derive(Deserialize, Debug, Clone, Copy)]
pub struct BrasGenome {
    /// 🚨 L'IK des bras est-elle active ?
    ///
    /// # Pourquoi elle est ÉTEINTE par défaut depuis le 2026-08-17
    ///
    /// Le corps porte désormais `rifle_aim` — une pose de visée **capturée**.
    /// Deux systèmes voudraient alors écrire les mêmes quatre os : le clip les
    /// met où le mocap dit, l'IK les tire vers sa cible calculée. Le plus tardif
    /// gagne, et le résultat n'est ni l'un ni l'autre.
    ///
    /// Une mocap bat une approximation géométrique sur la POSE. Ce que le clip ne
    /// sait pas faire, c'est **suivre le tangage de la caméra** — viser vers le
    /// haut. Ça reste le travail du suivi de buste (`suivi_du_buste`), qui touche
    /// `Spine2` et non les bras : les deux ne se disputent plus rien.
    ///
    /// La rallumer n'a de sens que pour un corps SANS clip de visée. Le laisser
    /// réglable plutôt que de supprimer le code : c'est exactement le cas du
    /// trooper, et de tout corps à venir.
    #[serde(default)]
    pub actif: bool,
    /// 0 = main contre l'épaule, 1 = bras tendu.
    #[serde(default = "portee_par_defaut")]
    pub portee_avant: f32,
    /// Au-dessus (+) ou en dessous (−) de l'épaule.
    #[serde(default = "hauteur_par_defaut")]
    pub hauteur: f32,
    /// Vers l'axe du corps (−) : l'arme se centre devant le buste.
    #[serde(default = "ecart_par_defaut")]
    pub ecart_lateral: f32,
    /// La main gauche soutient, un peu à gauche de la droite.
    #[serde(default = "soutien_par_defaut")]
    pub soutien_lateral: f32,
    /// De combien le coude de tir s'ÉCARTE du corps. 0 = coude collé, pendant
    /// tout droit sous l'épaule ; 1 = coude à l'horizontale.
    ///
    /// # Ce que ce réglage corrige
    ///
    /// Les deux coudes tombaient tout droit vers le bas — le seul pôle était
    /// `-Y`. C'est la posture de quelqu'un qui **tient** une arme, pas de
    /// quelqu'un qui **vise** : un tireur lève le coude de tir sur le côté pour
    /// caler la crosse, et garde l'autre bas sous l'arme pour la porter. Deux
    /// coudes identiques donnent la silhouette symétrique et molle rapportée le
    /// 2026-08-17 — « pas une pose très naturelle ».
    #[serde(default = "ouverture_par_defaut")]
    pub coude_ouverture: f32,
}

impl Default for BrasGenome {
    fn default() -> Self {
        Self {
            // Éteinte : le clip capturé fait mieux, et deux systèmes sur les
            // mêmes os ne produisent ni l'un ni l'autre.
            actif: false,
            portee_avant: portee_par_defaut(),
            hauteur: hauteur_par_defaut(),
            ecart_lateral: ecart_par_defaut(),
            soutien_lateral: soutien_par_defaut(),
            coude_ouverture: ouverture_par_defaut(),
        }
    }
}

impl BrasGenome {
    /// Le pôle du coude, côté par côté.
    ///
    /// Le coude de tir s'ouvre sur le côté ; celui qui soutient reste bas, et
    /// rentre légèrement sous l'arme. C'est **l'asymétrie** qui fait la pose :
    /// deux coudes traités pareil donnent une silhouette de mannequin.
    ///
    /// Pure : le seul endroit où cette asymétrie se décide, donc le seul à
    /// tester.
    #[must_use]
    pub fn pole_coude(&self, bas: Vec3, droite: Vec3, soutien: bool) -> Vec3 {
        let o = self.coude_ouverture.clamp(0.0, 1.0);
        let lateral = if soutien {
            // La main gauche passe SOUS l'arme : son coude rentre vers l'axe du
            // corps, et beaucoup moins que celui de tir.
            -o * 0.35
        } else {
            o
        };
        (bas + droite * lateral).normalize_or(bas)
    }
}

/// Coude PLIÉ, arme près de la poitrine. 0,72 tendait presque le bras — une
/// posture de statue qui présente une arme, pas de quelqu'un qui vise.
fn portee_par_defaut() -> f32 {
    0.62
}
/// À hauteur de poitrine, pas au-dessus de l'épaule : c'est là que la crosse se
/// cale.
fn hauteur_par_defaut() -> f32 {
    0.02
}
/// L'arme se centre franchement sur l'axe du corps. Une arme restée sur le côté
/// donne la silhouette de quelqu'un qui la porte, pas qui la pointe.
fn ecart_par_defaut() -> f32 {
    -0.18
}
fn soutien_par_defaut() -> f32 {
    0.30
}

/// Le coude de tir s'ouvre franchement : c'est ce qui distingue « viser » de
/// « porter ». 0,55 est la moitié du chemin vers l'horizontale — au-delà, la
/// silhouette devient celle d'un archer, pas d'un tireur.
fn ouverture_par_defaut() -> f32 {
    0.55
}

impl BrasGenome {
    /// Où la main doit se trouver, dans le monde. **Pure** — c'est le cœur du
    /// nouveau système, et il se vérifie sans moteur ni squelette.
    ///
    /// `longueur` est la longueur RÉELLE de la chaîne (épaule→coude→main),
    /// mesurée sur le rig : c'est elle qui rend les fractions transposables.
    /// Le repère est celui du regard, pas celui du monde — viser vers le haut
    /// lève donc la cible, sans qu'aucun terme ne le dise explicitement.
    #[must_use]
    pub fn cible(
        &self,
        epaule: Vec3,
        avant: Vec3,
        haut: Vec3,
        droite: Vec3,
        longueur: f32,
        soutien: bool,
    ) -> Vec3 {
        let lateral = if soutien {
            self.ecart_lateral + self.soutien_lateral
        } else {
            self.ecart_lateral
        };
        epaule
            + avant * (longueur * self.portee_avant)
            + haut * (longueur * self.hauteur)
            + droite * (longueur * lateral)
    }
}

impl Default for ViseeGenome {
    fn default() -> Self {
        Self {
            montee_s: montee_par_defaut(),
            descente_s: descente_par_defaut(),
            suivi_du_buste: suivi_par_defaut(),
            bras: BrasGenome::default(),
            pose_decontractee: HashMap::new(),
            pose_tactique: HashMap::new(),
            ecart_alerte_deg: ecart_alerte_par_defaut(),
            ecart_avertit_deg: ecart_avertit_par_defaut(),
        }
    }
}

fn montee_par_defaut() -> f32 {
    0.16
}
fn descente_par_defaut() -> f32 {
    0.24
}
fn suivi_par_defaut() -> f32 {
    0.75
}

impl ViseeGenome {
    /// Rotation à ajouter à un os, pour un facteur de visée donné.
    ///
    /// Pure, et c'est délibéré : c'est le cœur du module, et il se vérifie sans
    /// monter de moteur ni charger de glTF.
    #[must_use]
    pub fn rotation_os(&self, os: &str, facteur: f32) -> Quat {
        let lire = |m: &HashMap<String, [f32; 3]>| m.get(os).copied().unwrap_or([0.0; 3]);
        let a = euler(lire(&self.pose_decontractee));
        let b = euler(lire(&self.pose_tactique));
        // `slerp` et non une interpolation des angles : trois angles d'Euler
        // interpolés séparément ne décrivent pas le chemin le plus court, et le
        // bras part en vrille au milieu du fondu.
        a.slerp(b, facteur.clamp(0.0, 1.0))
    }

    /// Tous les os cités par l'une ou l'autre tenue.
    pub fn os_cites(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .pose_decontractee
            .keys()
            .chain(self.pose_tactique.keys())
            .cloned()
            .collect();
        v.sort();
        v.dedup();
        v
    }
}

fn euler(d: [f32; 3]) -> Quat {
    Quat::from_rotation_x(d[0].to_radians())
        * Quat::from_rotation_y(d[1].to_radians())
        * Quat::from_rotation_z(d[2].to_radians())
}

// ---------------------------------------------------------------------------
// État
// ---------------------------------------------------------------------------

/// 0 = décontracté, 1 = tactique. Une seule vérité pour la tenue, la vue et le
/// réticule.
#[derive(Resource, Default, Debug)]
pub struct Visee {
    pub facteur: f32,
    /// Le zoom de l'arme actuellement tenue (1 = aucun). Retenu ici pour que le
    /// capteur puisse le montrer sans relire le génome.
    pub zoom_arme: f32,
    /// **Mesures d'INTENTION** — la distance qui reste entre chaque main et
    /// l'endroit où on lui demande d'être.
    ///
    /// C'est la différence entre « quelle est la rotation de l'épaule » et
    /// « est-ce que la main est au bon endroit ». La première vieillit avec le
    /// code, la seconde avec le jeu — et c'est la seconde qui trouve les bugs
    /// qu'on n'avait pas prévus. Le 2026-08-16, aucune mesure de ce genre
    /// n'existait sur l'avatar : il a fallu une capture d'écran pour voir que
    /// les bras partaient en arrière.
    pub ecart_main_droite_m: f32,
    pub ecart_main_gauche_m: f32,
    /// Angle entre l'axe du buste et le regard, en degrés. Un buste qui ne suit
    /// pas la caméra fait tirer le personnage à côté de ce qu'il regarde.
    pub ecart_buste_deg: f32,
}

/// Le champ de vision de la caméra AVANT que ce module y touche.
///
/// Capturé, pas déclaré : la caméra d'épaule est construite ailleurs
/// (`castle_avatar`), et recopier sa valeur ici en ferait une grandeur écrite
/// deux fois — qui divergerait au premier réglage de l'autre côté.
#[derive(Component)]
struct ChampDeVisionAuRepos(f32);

/// Les os de la tenue, retrouvés une fois puis mémorisés.
///
/// L'avatar est reconstruit à chaque changement d'équipement : la table est donc
/// invalidée dès qu'une de ses entités ne répond plus, et non « au premier
/// chargement » — ce qui laisserait des entités mortes dedans.
/// Ce qui reste entre chaque main et sa cible. `-1` = on ne vise pas, donc rien
/// à mesurer — et non « zéro », qui se lirait comme « parfaitement en place ».
#[derive(Resource)]
struct MesuresBras {
    droite_m: f32,
    gauche_m: f32,
}

impl Default for MesuresBras {
    fn default() -> Self {
        Self {
            droite_m: -1.0,
            gauche_m: -1.0,
        }
    }
}

/// Les trois os de chaque bras, mémorisés. Clé = les noms de la chaîne, donc
/// stable même si le corps change d'entités.
#[derive(Resource, Default)]
struct OsDesBras {
    table: HashMap<(&'static str, &'static str, &'static str), [Entity; 3]>,
}

#[derive(Resource, Default)]
struct OsDeLaTenue {
    table: HashMap<String, Entity>,
    /// Par os : la rotation **animée** (la base), et la rotation qu'on a écrite
    /// à la frame précédente. Voir `poser_la_tenue` — c'est ce couple qui
    /// empêche la pose de s'ajouter à elle-même indéfiniment.
    memo: HashMap<String, (Quat, Quat)>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ExpeditionViseePlugin;

impl Plugin for ExpeditionViseePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visee>()
            .init_resource::<OsDeLaTenue>()
            .init_resource::<OsDesBras>()
            .init_resource::<MesuresBras>()
            // La cible dessinée appartient au gizmo de debug, mais on l'initie
            // ici aussi : `init_resource` est idempotent, et dépendre d'un autre
            // plugin pour une ressource qu'on ÉCRIT rend ce plugin
            // inconstruisible seul — donc intestable seul.
            .init_resource::<forgia_anim_debug::prelude::CiblesDebug>()
            .add_systems(OnExit(GameMode::Expedition), rendre_la_vue)
            .add_systems(
                Update,
                (suivre_le_bouton, appliquer_le_zoom, teindre_le_reticule)
                    .chain()
                    .run_if(in_state(GameMode::Expedition))
                    .run_if(in_state(AppMode::InGame)),
            )
            // ENTRE l'animation et la propagation. Avant `AnimationSystems`, le
            // clip écraserait la pose à chaque frame — le personnage ne lèverait
            // jamais les bras. Après la propagation, les matrices monde seraient
            // déjà calculées : la pose ne se verrait qu'à la frame suivante, et
            // l'arme (enfant d'un os) traînerait d'une frame derrière la main.
            .add_systems(
                PostUpdate,
                // L'IK des bras AVANT les retouches d'os : elle pose la
                // structure, la pose déclarée ne fait qu'ajuster ce qu'elle ne
                // pilote pas (le buste). L'inverse ferait corriger le buste sur
                // une posture qui n'est pas encore celle qu'on rendra.
                (poser_les_bras, poser_la_tenue)
                    .chain()
                    .after(AnimationSystems)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameMode::Expedition)),
            )
            .add_systems(Update, capteur_visee.in_set(GameSet::Sensors));
    }
}

// ---------------------------------------------------------------------------
// Systèmes
// ---------------------------------------------------------------------------

/// Fait monter et descendre le facteur de visée.
fn suivre_le_bouton(
    time: Res<Time>,
    boutons: Res<ButtonInput<MouseButton>>,
    equipped: Res<EquippedWeapons>,
    handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    mut visee: ResMut<Visee>,
) {
    let genome = handle.as_deref().and_then(|h| genomes.get(&h.0));
    let cfg = genome.map(|g| &g.data.visee);
    let vise = boutons.pressed(BOUTON_VISEE);
    let duree = match (vise, cfg) {
        (true, Some(c)) => c.montee_s,
        (false, Some(c)) => c.descente_s,
        (true, None) => montee_par_defaut(),
        (false, None) => descente_par_defaut(),
    };
    // Une durée nulle vaut « instantané », pas une division par zéro.
    let pas = if duree > 1e-4 {
        time.delta_secs() / duree
    } else {
        1.0
    };
    let cible = if vise { 1.0 } else { 0.0 };
    visee.facteur = if visee.facteur < cible {
        (visee.facteur + pas).min(cible)
    } else {
        (visee.facteur - pas).max(cible)
    };
    visee.zoom_arme = genome
        .map(|g| g.data.reglage(equipped.current).zoom)
        .unwrap_or(1.0);
}

/// Resserre le champ de vision de la caméra d'épaule.
///
/// Le champ au repos est **capturé** sur la caméra la première fois qu'on la
/// voit, puis restitué exactement. Écrire une valeur « de repos » déclarée ici
/// écraserait tout réglage venu d'ailleurs, et le zoom deviendrait un effet de
/// bord permanent au lieu d'un état transitoire.
fn appliquer_le_zoom(
    mut commands: Commands,
    visee: Res<Visee>,
    mut q: Query<
        (
            Entity,
            &mut Projection,
            Option<&ChampDeVisionAuRepos>,
            &Camera,
        ),
        With<OrbitCamera>,
    >,
) {
    for (entite, mut projection, repos, camera) in &mut q {
        if !camera.is_active {
            continue;
        }
        let Projection::Perspective(persp) = projection.as_mut() else {
            continue; // orthographique : aucun champ de vision à resserrer
        };
        let base = match repos {
            Some(r) => r.0,
            None => {
                commands
                    .entity(entite)
                    .insert(ChampDeVisionAuRepos(persp.fov));
                persp.fov
            }
        };
        // Un zoom de 1 doit rendre EXACTEMENT le champ d'origine : sans ce
        // repli, une arme sans zoom réécrirait la valeur capturée à chaque
        // frame et interdirait tout réglage extérieur.
        let facteur = 1.0 + (visee.zoom_arme.max(0.01) - 1.0) * visee.facteur.clamp(0.0, 1.0);
        let voulu = base / facteur;
        if (persp.fov - voulu).abs() > 1e-5 {
            persp.fov = voulu;
        }
    }
}

/// Fait passer le réticule de la croix au point rouge.
fn teindre_le_reticule(visee: Res<Visee>, mode: Option<ResMut<forgia_crosshair::CrosshairMode>>) {
    if let Some(mut m) = mode {
        if (m.ads_progress - visee.facteur).abs() > 1e-4 {
            m.ads_progress = visee.facteur;
        }
    }
}

/// Superpose la tenue aux os, par-dessus le clip qui joue.
fn poser_la_tenue(
    visee: Res<Visee>,
    handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_noms: Query<(Entity, &Name)>,
    mut q_tf: Query<&mut Transform>,
    q_cam: Query<(&OrbitCamera, &Camera)>,
    mut os: ResMut<OsDeLaTenue>,
) {
    let Some(genome) = handle.as_deref().and_then(|h| genomes.get(&h.0)) else {
        return;
    };
    let cfg = &genome.data.visee;
    let cites = cfg.os_cites();
    if cites.is_empty() {
        return;
    }

    // La table est-elle encore valable ? Une seule entité morte suffit à la
    // périmer : l'avatar est reconstruit d'un bloc, jamais os par os.
    let valide = cites.len() == os.table.len()
        && os
            .table
            .iter()
            .all(|(nom, e)| q_tf.get(*e).is_ok() && cites.contains(nom));
    if !valide {
        os.table.clear();
        // La mémoire des bases part avec la table : elle décrit des os qui
        // n'existent plus. La garder ferait repartir la pose d'une base morte.
        os.memo.clear();
        for (entite, nom) in &q_noms {
            let n = nom.as_str();
            if cites.iter().any(|c| c == n) {
                os.table.insert(n.to_string(), entite);
            }
        }
        if os.table.len() < cites.len() {
            // Le corps n'est pas encore là — on retentera. Ce n'est un défaut
            // que si ça dure, et c'est le capteur qui le dira.
            return;
        }
    }

    // Le tangage de la caméra ACTIVE : c'est lui qui dit où on regarde.
    let tangage = q_cam
        .iter()
        .find(|(_, c)| c.is_active)
        .map(|(o, _)| o.pitch)
        .unwrap_or(0.0);
    let suivi = (-tangage * cfg.suivi_du_buste * visee.facteur.clamp(0.0, 1.0))
        .clamp(BUSTE_MIN_RAD, BUSTE_MAX_RAD);

    // 🚨 On REMPLACE la rotation, on ne la multiplie pas. Ces deux lignes ont
    // l'air équivalentes et ne le sont pas.
    //
    // `tf.rotation *= ajout` suppose que l'animation vient de réécrire l'os,
    // donc que la valeur lue est la pose animée nue. C'est faux dès que
    // l'animation ne l'a PAS réécrite — lecteur arrêté, clip qui ne cible pas
    // cet os, avatar en cours de reconstruction. Alors la multiplication
    // s'applique à MA sortie de la frame précédente, et la pose s'ajoute à
    // elle-même : le bras part en hélice. Mesuré le 2026-08-16 —
    // `anim_playing: 0` dans le capteur, et « y'a un souci quand je vise » en
    // jeu. Uniquement en visée, parce qu'au repos l'ajout vaut l'identité et
    // qu'une identité accumulée reste l'identité : le défaut était invisible
    // tant qu'on ne visait pas.
    //
    // On garde donc la base animée. Si l'os porte encore EXACTEMENT ce qu'on y
    // a écrit, personne ne l'a touché depuis : la base est celle qu'on avait
    // mémorisée. Sinon, l'animation vient d'écrire, et c'est la nouvelle base.
    // Emprunts disjoints des deux champs : lire la table pendant qu'on écrit la
    // mémoire, sans le tampon intermédiaire qu'un emprunt global imposerait —
    // un `Vec` par frame sur un chemin par-frame est exactement ce que
    // `scalability.md` interdit.
    let OsDeLaTenue { table, memo } = &mut *os;
    for (nom, entite) in table.iter() {
        let Ok(mut tf) = q_tf.get_mut(*entite) else {
            continue;
        };
        let courant = tf.rotation;
        let base = match memo.get(nom) {
            Some((base, sortie)) if courant.abs_diff_eq(*sortie, 1e-5) => *base,
            _ => courant,
        };
        let mut ajout = cfg.rotation_os(nom, visee.facteur);
        if nom == OS_DU_BUSTE {
            // Le buste porte DEUX termes : la pose déclarée (l'ouverture vers la
            // cible) et le suivi dérivé du tangage. Sans le second, viser vers
            // le haut lèverait le réticule sans lever le canon.
            ajout *= Quat::from_rotation_x(suivi);
        }
        let sortie = base * ajout;
        tf.rotation = sortie;
        match memo.get_mut(nom) {
            Some(e) => *e = (base, sortie),
            // Alloue une seule fois par os, au premier passage — pas à chaque
            // frame comme le ferait un `insert` avec `nom.clone()` systématique.
            None => {
                memo.insert(nom.clone(), (base, sortie));
            }
        }
    }
}

/// Les deux chaînes de bras : épaule → coude → main.
///
/// Noms Mixamo, présents dans tous les corps livrés (vérifié sur les 83 nœuds du
/// chien d'expédition comme sur les 81 du corps humain). Ce sont des OS, pas des
/// sockets : ils survivent aux changements d'outil, contrairement à ce qui vient
/// de coûter une session.
const CHAINES_BRAS: [(&str, &str, &str, bool); 2] = [
    ("RightArm", "RightForeArm", "RightHand", false),
    ("LeftArm", "LeftForeArm", "LeftHand", true),
];

/// Où doit tomber le COUDE pour que la main atteigne la cible.
///
/// # Pourquoi on calcule une position et pas des rotations
///
/// `two_bone_ik` rend `mid_rotation` comme une **flexion absolue**
/// (`from_axis_angle(axe, angle_de_pliure)`), pas comme un écart à appliquer.
/// La version d'avant la composait pourtant sur la rotation courante du coude,
/// à chaque frame : un angle absolu appliqué en relatif, cent vingt fois par
/// deux secondes. C'est **le** mécanisme derrière « les bras tournent sur
/// eux-mêmes » du 2026-08-17 — et l'arme, qui suit la main, penchait avec.
///
/// Ici on ne compose plus rien. On calcule où le coude DOIT être — une fonction
/// pure de l'épaule, de la cible, des deux longueurs d'os et du pôle — et on
/// oriente ensuite chaque os vers un point connu. Aucune de ces entrées ne
/// dépend de ce qu'on a écrit à la frame précédente : il n'y a donc **plus de
/// boucle**, et plus rien à faire converger.
///
/// C'est l'intersection de deux sphères (rayons `l1` autour de l'épaule, `l2`
/// autour de la cible) : un cercle. Le pôle choisit le point sur ce cercle —
/// c'est lui qui décide que le coude tombe vers le bas plutôt que vers le haut.
#[must_use]
pub fn coude_vise(epaule: Vec3, cible: Vec3, l1: f32, l2: f32, pole: Vec3) -> Vec3 {
    let vers = cible - epaule;
    let chaine = l1 + l2;
    let brut = vers.length();
    // Cible hors d'atteinte : on tend le bras au lieu de rendre NaN. Une cible
    // sur l'épaule ferait de même dans l'autre sens.
    let d = brut.clamp(1e-4, (chaine - 1e-4).max(1e-4));
    let dir = vers.try_normalize().unwrap_or(Vec3::NEG_Z);
    // Projection du coude sur l'axe épaule→cible, par la loi des cosinus.
    let a = ((l1 * l1 - l2 * l2 + d * d) / (2.0 * d)).clamp(-l1, l1);
    let h = (l1 * l1 - a * a).max(0.0).sqrt();
    let perp = (pole - dir * pole.dot(dir))
        .try_normalize()
        .unwrap_or_else(|| dir.any_orthonormal_vector());
    epaule + dir * a + perp * h
}

/// Oriente un os vers une direction, **roulis compris**.
///
/// # Le degré de liberté qu'une rotation d'arc laisse traîner
///
/// `Quat::from_rotation_arc(a, b)` rend la rotation la plus courte de `a` vers
/// `b`. Elle est unique, mais elle ne décrit qu'**une contrainte sur trois** :
/// l'os pointe où il faut, et rien ne dit comment il est tourné autour de son
/// propre axe. Ce degré libre ne dérange pas une fois — il dérange quand on
/// recompose l'arc sur le résultat de la frame précédente, parce qu'il n'est
/// jamais remis à zéro. Le bras se met alors à visser lentement.
///
/// Ici on le ferme : après avoir pointé l'os, on le fait tourner autour de sa
/// propre direction jusqu'à ce que son côté regarde le pôle. Deux contraintes
/// (direction + pôle) déterminent entièrement une orientation.
///
/// Pure : testable sans moteur, et c'est là que le défaut se prouve.
#[must_use]
pub fn orienter_os(rot_monde: Quat, axe_local: Vec3, cible: Vec3, pole: Vec3) -> Quat {
    let actuel = rot_monde * axe_local;
    let Some(cible) = cible.try_normalize() else {
        return rot_monde;
    };
    let apres_arc = Quat::from_rotation_arc(actuel.normalize_or_zero(), cible) * rot_monde;

    // La référence de roulis : un axe local perpendiculaire à l'os, stable
    // parce qu'il se dérive du rig et non de la frame courante.
    let cote_local = axe_local.any_orthonormal_vector();
    let cote = apres_arc * cote_local;
    // Projeter côté et pôle dans le plan perpendiculaire à l'os : c'est le seul
    // plan où le roulis se mesure.
    let plat = |v: Vec3| v - cible * v.dot(cible);
    let (Some(c), Some(p)) = (plat(cote).try_normalize(), plat(pole).try_normalize()) else {
        // Pôle aligné sur l'os : le roulis n'a pas de bonne réponse. On garde
        // celui qu'on a plutôt que d'en inventer un — mais on ne dérive pas,
        // puisqu'on repart de la même base à chaque frame.
        return apres_arc;
    };
    let angle = c.angle_between(p) * c.cross(p).dot(cible).signum();
    Quat::from_axis_angle(cible, angle) * apres_arc
}

/// Lève les bras vers la cible, par IK — au lieu de leur imposer des angles.
///
/// # Ce que ce système remplace, et pourquoi il converge
///
/// Il tourne dans le même créneau que `poser_la_tenue` (après les clips, avant
/// la propagation) et lit les transforms MONDE de la frame précédente. C'est un
/// retour en boucle — mais un retour **convergent** : la cible est fixe dans le
/// monde, donc dès que la main l'atteint, la correction devient l'identité et
/// plus rien ne bouge. C'est la différence avec la couche additive d'hier, qui
/// s'ajoutait à elle-même sans jamais se stabiliser.
///
/// On mélange par `slerp` vers la rotation LOCALE voulue, jamais en appliquant
/// une fraction de correction : au repos on n'écrit rien du tout, et l'animation
/// garde la main.
#[allow(clippy::too_many_arguments)]
fn poser_les_bras(
    visee: Res<Visee>,
    handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    q_noms: Query<(Entity, &Name)>,
    q_gt: Query<&GlobalTransform>,
    q_parent: Query<&ChildOf>,
    mut q_tf: Query<&mut Transform>,
    q_cam: Query<(&GlobalTransform, &Camera), With<OrbitCamera>>,
    mut cache: ResMut<OsDesBras>,
    mut mesures: ResMut<MesuresBras>,
    mut cibles: ResMut<forgia_anim_debug::prelude::CiblesDebug>,
) {
    // Au repos, on ne touche à RIEN : l'animation est la tenue décontractée.
    // Un seuil, et non `== 0`, parce que le fondu approche zéro sans l'atteindre.
    cibles.0.clear();
    if visee.facteur < 1e-3 {
        mesures.droite_m = -1.0;
        mesures.gauche_m = -1.0;
        return;
    }
    // Éteinte quand le corps a une pose de visée capturée : deux systèmes qui
    // écrivent les mêmes os ne donnent ni l'un ni l'autre. Le suivi de buste,
    // lui, continue — c'est la seule chose qu'un clip figé ne sait pas faire.
    if !handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| g.data.visee.bras.actif)
        .unwrap_or(false)
    {
        mesures.droite_m = -1.0;
        mesures.gauche_m = -1.0;
        return;
    }
    let Some(cfg) = handle
        .as_deref()
        .and_then(|h| genomes.get(&h.0))
        .map(|g| g.data.visee.bras)
    else {
        return;
    };
    let Some(cam) = q_cam.iter().find(|(_, c)| c.is_active).map(|(gt, _)| gt) else {
        return;
    };
    let avant = cam.forward().as_vec3();
    let droite = cam.right().as_vec3();
    let haut = Vec3::Y;

    for (epaule_n, coude_n, main_n, soutien) in CHAINES_BRAS {
        // Résolution des trois os, mémorisée : une entité morte périme la
        // chaîne entière — l'avatar est reconstruit d'un bloc.
        let cle = (epaule_n, coude_n, main_n);
        let trio = match cache.table.get(&cle) {
            Some(t) if t.iter().all(|e| q_gt.get(*e).is_ok()) => *t,
            _ => {
                let mut trouve = [None; 3];
                for (e, n) in &q_noms {
                    match n.as_str() {
                        x if x == epaule_n => trouve[0] = Some(e),
                        x if x == coude_n => trouve[1] = Some(e),
                        x if x == main_n => trouve[2] = Some(e),
                        _ => {}
                    }
                }
                let (Some(a), Some(b), Some(c)) = (trouve[0], trouve[1], trouve[2]) else {
                    continue; // corps pas encore là, ou os absents
                };
                let t = [a, b, c];
                cache.table.insert(cle, t);
                t
            }
        };

        let (Ok(g_ep), Ok(g_co), Ok(g_ma)) =
            (q_gt.get(trio[0]), q_gt.get(trio[1]), q_gt.get(trio[2]))
        else {
            continue;
        };
        let (p_ep, p_co, p_ma) = (
            g_ep.translation(),
            g_co.translation(),
            g_ma.translation(),
        );
        let longueur = (p_co - p_ep).length() + (p_ma - p_co).length();
        if longueur < 1e-3 {
            continue; // rig dégénéré : on ne prétend rien
        }

        let l1 = (p_co - p_ep).length();
        let l2 = (p_ma - p_co).length();
        let cible = cfg.cible(p_ep, avant, haut, droite, longueur, soutien);
        // LA mesure d'intention : ce qui reste entre la main et l'endroit où on
        // lui demande d'être. Elle se lit sans rien savoir du rig — c'est tout
        // son intérêt. Publiée AVANT le solveur : elle décrit donc l'état de la
        // frame précédente, ce qui est exactement ce qu'on veut savoir (« est-ce
        // que ça converge ? »).
        let ecart = p_ma.distance(cible);
        if soutien {
            mesures.gauche_m = ecart;
        } else {
            mesures.droite_m = ecart;
        }
        // Et la même chose, en image : la cible dessinée à côté de la main
        // transforme la question en évidence (F3).
        cibles.0.push((
            cible,
            if soutien {
                Color::srgb(0.4, 0.7, 1.0)
            } else {
                Color::srgb(1.0, 0.8, 0.2)
            },
        ));
        // Le pôle décide où tombe le coude — sans lui, rien ne distingue un
        // coude sous le bras d'un coude au-dessus : les deux mettent la main au
        // même endroit. Et il diffère d'un bras à l'autre : c'est cette
        // asymétrie qui fait la différence entre viser et porter.
        let pole = cfg.pole_coude(-haut, droite, soutien);
        let coude = coude_vise(p_ep, cible, l1, l2, pole);

        // Deux os, deux points connus : l'épaule regarde le coude, le coude
        // regarde la cible. Aucune rotation n'est composée sur la précédente —
        // c'est ce qui supprime la boucle, donc la dérive.
        for (os, enfant, dir) in [
            (trio[0], trio[1], coude - p_ep),
            (trio[1], trio[2], cible - coude),
        ] {
            let (Ok(g_os), Ok(g_enf)) = (q_gt.get(os), q_gt.get(enfant)) else {
                continue;
            };
            let Some(cible_dir) = dir.try_normalize() else {
                continue;
            };
            // L'axe local de l'os — celui qui pointe vers son enfant. On le
            // MESURE au lieu de le supposer : Mixamo met +Y le long de l'os,
            // mais un autre rig ne le fera pas, et supposer une convention est
            // précisément ce qui a coûté les trois derniers allers-retours.
            let Some(axe_local) = (g_os
                .rotation()
                .inverse()
                * (g_enf.translation() - g_os.translation()))
            .try_normalize() else {
                continue;
            };
            let voulu_monde = orienter_os(g_os.rotation(), axe_local, cible_dir, pole);
            let parent_monde = q_parent
                .get(os)
                .ok()
                .and_then(|p| q_gt.get(p.parent()).ok())
                .map(|g| g.rotation())
                .unwrap_or(Quat::IDENTITY);
            let voulu_local = parent_monde.inverse() * voulu_monde;
            if let Ok(mut tf) = q_tf.get_mut(os) {
                tf.rotation = tf.rotation.slerp(voulu_local, visee.facteur.clamp(0.0, 1.0));
            }
        }
    }
}

/// Sortie du mode : le champ de vision est rendu tel qu'on l'a trouvé.
///
/// Sans ça, quitter l'Expédition l'œil dans la lunette laisserait une caméra
/// zoomée dans le Hall — et rien là-bas n'écrit ce champ, donc personne ne le
/// remettrait jamais.
fn rendre_la_vue(
    mut commands: Commands,
    mut q: Query<(Entity, &mut Projection, &ChampDeVisionAuRepos)>,
    mut visee: ResMut<Visee>,
    mut os: ResMut<OsDeLaTenue>,
) {
    for (entite, mut projection, repos) in &mut q {
        if let Projection::Perspective(persp) = projection.as_mut() {
            persp.fov = repos.0;
        }
        commands.entity(entite).remove::<ChampDeVisionAuRepos>();
    }
    visee.facteur = 0.0;
    os.table.clear();
}

/// `forgia2_expedition_visee.json`, 1 Hz.
fn capteur_visee(
    time: Res<Time>,
    mut accum: Local<f32>,
    mode: Res<State<GameMode>>,
    // 🚨 UN SEUL accès à `Visee`, en écriture. La version d'ajout des mesures
    // d'intention avait `Res<Visee>` ET `ResMut<Visee>` dans la même signature :
    // Bevy refuse (B0002) et le jeu **panique au démarrage**. Le compilateur ne
    // l'attrape pas — le conflit se résout à la construction du schedule, donc
    // au lancement. C'est le genre de faute qu'aucun `cargo check` ne voit.
    mut visee: ResMut<Visee>,
    handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    os: Res<OsDeLaTenue>,
    mesures: Res<MesuresBras>,
    q_cam: Query<(&Projection, &Camera), With<OrbitCamera>>,
    q_cam_gt: Query<(&GlobalTransform, &Camera), With<OrbitCamera>>,
    q_gt: Query<&GlobalTransform>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let en_expedition = *mode.get() == GameMode::Expedition;
    let cfg = handle.as_deref().and_then(|h| genomes.get(&h.0));
    let os_attendus = cfg.map(|g| g.data.visee.os_cites().len()).unwrap_or(0);
    let os_trouves = os.table.len();
    let fov_deg = q_cam
        .iter()
        .find(|(_, c)| c.is_active)
        .and_then(|(p, _)| match p {
            Projection::Perspective(pp) => Some(pp.fov.to_degrees()),
            _ => None,
        })
        .unwrap_or(0.0);

    // L'écart buste↔regard : un buste qui ne suit pas la caméra fait tirer le
    // personnage à côté de ce qu'il regarde. Mesuré sur le +Y de l'os (l'axe
    // long d'un os Mixamo) projeté à l'horizontale — c'est l'orientation du
    // tronc, pas son inclinaison.
    let ecart_buste = match (
        os.table.get(OS_DU_BUSTE).and_then(|e| q_gt.get(*e).ok()),
        q_cam_gt.iter().find(|(_, c)| c.is_active),
    ) {
        (Some(buste), Some((cam, _))) => {
            let a = (buste.rotation() * Vec3::Y).with_y(0.0);
            let b = cam.forward().as_vec3().with_y(0.0);
            match (a.try_normalize(), b.try_normalize()) {
                (Some(a), Some(b)) => a.angle_between(b).to_degrees(),
                _ => -1.0,
            }
        }
        _ => -1.0,
    };
    visee.ecart_main_droite_m = mesures.droite_m;
    visee.ecart_main_gauche_m = mesures.gauche_m;
    visee.ecart_buste_deg = ecart_buste;

    // Un écart de main supérieur à ça, c'est une main qui n'arrive pas où on la
    // demande : l'IK ne converge pas, ou la cible est hors d'atteinte du bras.
    const ECART_MAIN_TOLERE_M: f32 = 0.12;

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — aucune visee attendue")
    } else if visee.facteur > 0.9 && mesures.droite_m > ECART_MAIN_TOLERE_M {
        (
            "warn",
            "MAIN_HORS_CIBLE : en pleine visee, la main droite reste loin du point demande. Soit la cible est hors d'atteinte du bras (baisser portee_avant au genome), soit l'IK ne converge pas (un autre systeme ecrit les memes os). Lire ecart_main_droite_m.",
        )
    } else if os_attendus == 0 {
        (
            "warn",
            "TENUE_NON_DECLAREE : aucune pose dans [visee.pose_tactique] — viser ne changera que la vue, le personnage gardera les bras le long du corps",
        )
    } else if os_trouves == 0 {
        (
            "warn",
            "OS_INTROUVABLES : aucun des os cites par la tenue n'existe dans la scene — verifier les noms (rig Mixamo : RightArm, RightForeArm, Spine2), le corps charge n'est peut-etre pas celui de l'Expedition",
        )
    } else if os_trouves < os_attendus {
        (
            "warn",
            "TENUE_PARTIELLE : des os cites au genome sont absents du squelette — la pose sera incomplete et asymetrique",
        )
    } else if visee.facteur > 0.5 && visee.zoom_arme > 1.01 && fov_deg > 1.0 {
        // Cas positif mesuré : on vise, l'arme a un zoom, le champ DOIT avoir
        // bougé. S'il n'a pas bougé, la caméra visée n'est pas celle qui rend.
        ("ok", "")
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"expedition_visee","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"facteur":{:.2},"zoom_arme":{:.2},"fov_deg":{:.1},"os_attendus":{os_attendus},"os_trouves":{os_trouves},"ecart_main_droite_m":{:.3},"ecart_main_gauche_m":{:.3},"ecart_buste_deg":{:.1}}}"#,
        time.elapsed_secs(),
        visee.facteur,
        visee.zoom_arme,
        fov_deg,
        mesures.droite_m,
        mesures.gauche_m,
        ecart_buste,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_visee.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome_deux_poses() -> ViseeGenome {
        let mut g = ViseeGenome::default();
        g.pose_decontractee
            .insert("RightArm".into(), [0.0, 0.0, 0.0]);
        g.pose_tactique.insert("RightArm".into(), [-60.0, 0.0, 0.0]);
        g
    }

    /// Le garde qui manquait : **construire le schedule et le faire tourner**.
    ///
    /// Le 2026-08-17, `capteur_visee` prenait `Res<Visee>` ET `ResMut<Visee>`.
    /// Bevy refuse (`error[B0002]`) — mais il le refuse à la **construction du
    /// schedule**, c'est-à-dire au lancement du jeu. `cargo check`, clippy et
    /// tous les tests purs de ce fichier passaient au vert sur un binaire qui
    /// paniquait avant d'afficher une image.
    ///
    /// Un `app.update()` suffit à forcer l'initialisation des systèmes, donc la
    /// vérification des accès. C'est le seul test de ce fichier qui monte un
    /// moteur, et c'est le seul qui pouvait attraper cette classe de faute.
    ///
    /// 🚨 IL N'EN COUVRAIT QUE DEUX SUR SIX.
    ///
    /// Constaté à l'audit du 2026-08-18 : `ExpeditionPosturePlugin`,
    /// `ExpeditionAvatarPlugin`, `ExpeditionMatierePlugin` et
    /// `ExpeditionVentPlugin` n'étaient montés par AUCUN `App::new()` du dépôt.
    /// Le garde passait au vert en ne regardant qu'un tiers de ce qu'il
    /// prétendait protéger — et rien ne le disait, ce qui est exactement la
    /// faute qu'il existe pour empêcher.
    ///
    /// `ExpeditionVentPlugin` reste dehors, seul, et pour une raison NOMMÉE :
    /// son `MaterialPlugin` exige l'app de rendu, qu'un test ne monte pas. Une
    /// exception qui se cite vaut mieux qu'un périmètre implicite ; le cliquet
    /// `xtask plugin-gate` la connaît et la compte.
    #[test]
    fn les_plugins_se_construisent_et_tournent_sans_conflit_d_acces() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(forgia_core::ForgiaCorePlugin)
            .add_plugins(crate::arme_main::ExpeditionArmeMainPlugin)
            .add_plugins(crate::posture::ExpeditionPosturePlugin)
            .add_plugins(crate::cape::ExpeditionCapePlugin)
            .add_plugins(crate::matiere::ExpeditionMatierePlugin)
            .add_plugins(crate::avatar_vfx::ExpeditionAvatarPlugin)
            .add_plugins(ExpeditionViseePlugin);
        // Deux passes : la première construit et initialise, la seconde
        // vérifie qu'on peut y revenir (une ressource consommée au premier tour
        // ferait échouer celle-ci).
        app.update();
        app.update();
    }

    /// La main doit ARRIVER sur la cible, et le coude tomber vers le pôle.
    ///
    /// C'est le contrat de `coude_vise`, et il se vérifie par construction
    /// géométrique — sans moteur, sans squelette, sans lancer le jeu.
    #[test]
    fn le_coude_place_la_main_sur_la_cible_et_tombe_vers_le_bas() {
        let epaule = Vec3::new(0.2, 1.5, 0.0);
        let (l1, l2) = (0.30, 0.28);
        let pole = -Vec3::Y;
        for cible in [
            epaule + Vec3::new(0.0, 0.0, -0.45),
            epaule + Vec3::new(0.1, 0.1, -0.40),
            epaule + Vec3::new(-0.15, -0.05, -0.35),
        ] {
            let coude = coude_vise(epaule, cible, l1, l2, pole);
            // Les deux longueurs d'os doivent être RESPECTÉES : un coude qui
            // étire l'humérus déforme le maillage au lieu de plier le bras.
            assert!(
                ((coude - epaule).length() - l1).abs() < 1e-3,
                "l'humérus mesure {:.3} au lieu de {l1}",
                (coude - epaule).length()
            );
            assert!(
                ((cible - coude).length() - l2).abs() < 1e-3,
                "l'avant-bras mesure {:.3} au lieu de {l2}",
                (cible - coude).length()
            );
            // Et le coude tombe SOUS la ligne épaule→cible, jamais au-dessus :
            // un coude en l'air est la signature d'un pôle ignoré.
            let dir = (cible - epaule).normalize();
            let ecart = coude - epaule - dir * (coude - epaule).dot(dir);
            assert!(
                ecart.dot(pole) > 0.0,
                "le coude part vers le haut (écart {ecart:?})"
            );
        }
    }

    #[test]
    fn une_cible_hors_d_atteinte_tend_le_bras_sans_rendre_nan() {
        // Un réglage trop généreux au génome, ou un bras replié pendant une
        // reconstruction d'avatar : le solveur doit tendre le bras, pas rendre
        // NaN — un NaN sur un os fait disparaître le personnage sans un mot.
        let c = coude_vise(Vec3::ZERO, Vec3::new(0.0, 0.0, -10.0), 0.3, 0.28, -Vec3::Y);
        assert!(c.is_finite(), "coude non fini pour une cible hors d'atteinte");
        let c2 = coude_vise(Vec3::ZERO, Vec3::ZERO, 0.3, 0.28, -Vec3::Y);
        assert!(c2.is_finite(), "coude non fini pour une cible sur l'épaule");
    }

    /// Le test qui manquait : **un os ne doit pas visser** quand on réapplique
    /// l'orientation frame après frame.
    ///
    /// C'est le défaut du 2026-08-17 — « les bras tournent sur eux-mêmes » —
    /// reproduit en pur calcul. Il n'apparaît QUE dans l'itération : une passe
    /// isolée pointe parfaitement, et c'est ce qui l'a rendu invisible à tous
    /// les tests précédents.
    #[test]
    fn un_os_ne_visse_pas_quand_on_le_reoriente_frame_apres_frame() {
        let axe_local = Vec3::Y; // convention Mixamo : +Y le long de l'os
        let cible = Vec3::new(0.0, 0.0, -1.0);
        let pole = -Vec3::Y;

        let mut rot = Quat::from_rotation_x(0.3) * Quat::from_rotation_z(0.2);
        let premiere = orienter_os(rot, axe_local, cible, pole);
        rot = premiere;
        for _ in 0..120 {
            rot = orienter_os(rot, axe_local, cible, pole);
        }
        // Après deux secondes de jeu à 60 images/s, l'orientation doit être
        // EXACTEMENT celle de la première passe. Toute dérive, même d'un
        // dixième de degré par frame, se voit à l'écran au bout de dix.
        assert!(
            rot.angle_between(premiere).to_degrees() < 0.5,
            "l'os a vissé de {:.1}° en 120 frames",
            rot.angle_between(premiere).to_degrees()
        );
    }

    #[test]
    fn orienter_un_os_le_fait_bien_pointer_vers_la_cible() {
        // Le contrat de base : la contrainte de roulis ne doit pas se payer en
        // ratant la direction. Corriger le vissage en cessant de viser serait
        // un échange perdant, et silencieux.
        let axe_local = Vec3::Y;
        for cible in [
            Vec3::NEG_Z,
            Vec3::new(1.0, 0.5, -0.3),
            Vec3::new(-0.2, -1.0, 0.4),
        ] {
            let rot = orienter_os(Quat::from_rotation_x(0.7), axe_local, cible, -Vec3::Y);
            let obtenu = rot * axe_local;
            assert!(
                obtenu.dot(cible.normalize()) > 0.999,
                "l'os pointe à {:.1}° de la cible",
                obtenu.angle_between(cible.normalize()).to_degrees()
            );
        }
    }

    #[test]
    fn le_buste_ne_bascule_pas_assez_pour_decrocher_la_cape() {
        // La cape et les cheveux pendent de `Spine2` (chaîne vérifiée dans le
        // glTF). Leurs chaînes d'os se balancent, elles n'encaissent pas un
        // basculement de leur point d'accroche. ±20° est la limite humaine.
        assert!(BUSTE_MAX_RAD.to_degrees() <= 25.0, "le buste bascule trop");
        assert!((BUSTE_MIN_RAD + BUSTE_MAX_RAD).abs() < 1e-6, "bornes asymétriques");
    }

    #[test]
    fn au_repos_la_tenue_ne_touche_a_rien() {
        // Facteur 0 = le clip d'animation tel quel. Une rotation résiduelle au
        // repos déformerait le personnage en permanence, y compris à l'arrêt —
        // et se lirait comme un rig cassé, pas comme un réglage de visée.
        let g = genome_deux_poses();
        let q = g.rotation_os("RightArm", 0.0);
        assert!(
            q.angle_between(Quat::IDENTITY) < 1e-5,
            "au repos, l'ajout doit être l'identité"
        );
    }

    #[test]
    fn a_fond_la_tenue_vaut_la_pose_declaree() {
        let g = genome_deux_poses();
        let q = g.rotation_os("RightArm", 1.0);
        let attendu = Quat::from_rotation_x((-60.0_f32).to_radians());
        assert!(
            (1.0 - q.dot(attendu).abs()).abs() < 1e-4,
            "la tenue pleine doit valoir exactement la pose déclarée"
        );
    }

    #[test]
    fn le_fondu_est_monotone_et_borne() {
        // À mi-chemin, on doit être STRICTEMENT entre les deux — c'est ce qui
        // distingue un fondu d'une bascule sèche. Et un facteur hors [0,1] ne
        // doit pas extrapoler : maintenir le clic ne doit pas continuer à
        // tordre le bras au-delà de la pose.
        let g = genome_deux_poses();
        let moitie = g.rotation_os("RightArm", 0.5);
        let plein = g.rotation_os("RightArm", 1.0);
        let a = moitie.angle_between(Quat::IDENTITY);
        let b = plein.angle_between(Quat::IDENTITY);
        assert!(a > 1e-3 && a < b, "mi-fondu {a:.3} rad, plein {b:.3} rad");
        assert!(
            g.rotation_os("RightArm", 5.0)
                .angle_between(plein)
                .abs()
                < 1e-5,
            "un facteur > 1 doit être borné, pas extrapolé"
        );
    }

    /// Reproduit le défaut du 2026-08-16 en pur calcul : une pose superposée à
    /// un os que l'animation NE réécrit PAS.
    ///
    /// C'est le test qui manquait. Les précédents vérifiaient une frame isolée ;
    /// aucun ne vérifiait ce que N frames font à la suite. Or le défaut n'existe
    /// QUE dans l'itération — et il ne se voyait qu'en visée, parce qu'au repos
    /// l'ajout vaut l'identité et qu'une identité accumulée reste l'identité.
    #[test]
    fn la_pose_ne_s_ajoute_pas_a_elle_meme_quand_l_animation_est_arretee() {
        let g = genome_deux_poses();
        let ajout = g.rotation_os("RightArm", 1.0);
        let base_animee = Quat::IDENTITY;

        // ❌ Ce que faisait la première version : multiplier ce qu'on lit.
        let mut naif = base_animee;
        for _ in 0..60 {
            naif *= ajout; // l'animation ne réécrit rien : on relit sa propre sortie
        }
        assert!(
            naif.angle_between(ajout) > 1.0,
            "le test ne reproduit pas le défaut : après 60 frames l'écart devrait être énorme"
        );

        // ✅ Ce que fait la version corrigée : garder la base, écrire base*ajout.
        let mut memo: Option<(Quat, Quat)> = None;
        let mut os = base_animee;
        for _ in 0..60 {
            let base = match memo {
                Some((base, sortie)) if os.abs_diff_eq(sortie, 1e-5) => base,
                _ => os,
            };
            os = base * ajout;
            memo = Some((base, os));
        }
        assert!(
            os.angle_between(ajout) < 1e-4,
            "après 60 frames sans animation, la pose doit valoir EXACTEMENT la pose voulue"
        );
    }

    /// L'autre moitié du contrat : quand l'animation réécrit l'os, la base doit
    /// suivre. Sinon on corrigerait l'hélice en figeant le personnage sur la
    /// première pose animée — un défaut plus discret, et pire.
    #[test]
    fn la_base_suit_l_animation_quand_elle_ecrit() {
        let g = genome_deux_poses();
        let ajout = g.rotation_os("RightArm", 1.0);
        let mut memo: Option<(Quat, Quat)> = None;
        let mut derniere = Quat::IDENTITY;

        for i in 0..30 {
            // L'animation écrit une base qui CHANGE à chaque frame (le clip joue).
            let base_animee = Quat::from_rotation_y((i as f32 * 0.05).to_radians());
            let lu = base_animee;
            let base = match memo {
                Some((base, sortie)) if lu.abs_diff_eq(sortie, 1e-5) => base,
                _ => lu,
            };
            derniere = base * ajout;
            memo = Some((base, derniere));
            assert!(
                base.abs_diff_eq(base_animee, 1e-5),
                "frame {i} : la base doit être celle que l'animation vient d'écrire"
            );
        }
        let attendu = Quat::from_rotation_y((29.0_f32 * 0.05).to_radians()) * ajout;
        assert!(derniere.abs_diff_eq(attendu, 1e-4));
    }

    /// La cible des mains doit être DEVANT le personnage, jamais derrière.
    ///
    /// C'est l'énoncé exact du défaut du 2026-08-16 — « les bras et l'arme sont
    /// dans le dos » — testé tel quel plutôt que par un angle abstrait. Il ne
    /// pouvait pas exister avant : avec des angles d'Euler par os, il n'y avait
    /// aucune grandeur à comparer au regard.
    #[test]
    fn la_cible_des_mains_est_devant_jamais_derriere() {
        let b = BrasGenome::default();
        let epaule = Vec3::new(0.2, 1.5, 0.0);
        // Le personnage regarde vers −Z ; sa droite est +X.
        let (avant, haut, droite) = (Vec3::NEG_Z, Vec3::Y, Vec3::X);
        for soutien in [false, true] {
            let c = b.cible(epaule, avant, haut, droite, 0.6, soutien);
            let vers_la_cible = c - epaule;
            assert!(
                vers_la_cible.dot(avant) > 0.0,
                "cible {c:?} : la main part DERRIÈRE l'épaule"
            );
            // Et pas au-delà de ce que le bras peut atteindre, sinon l'IK ne
            // peut que tendre le bras à fond : la tenue devient une pancarte.
            assert!(
                vers_la_cible.length() < 0.6,
                "cible à {:.2} m pour un bras de 0,60 m — le bras sera tendu",
                vers_la_cible.length()
            );
        }
    }

    /// Les deux coudes ne doivent PAS tomber au même endroit.
    ///
    /// C'est ce qui manquait à la pose du 2026-08-17 : un seul pôle `-Y` pour
    /// les deux bras, donc une silhouette symétrique — quelqu'un qui tient une
    /// arme, pas qui vise. Le coude de tir s'ouvre, celui qui soutient reste bas.
    #[test]
    fn les_deux_coudes_ne_tombent_pas_au_meme_endroit() {
        let b = BrasGenome::default();
        let (bas, droite) = (-Vec3::Y, Vec3::X);
        let tir = b.pole_coude(bas, droite, false);
        let soutien = b.pole_coude(bas, droite, true);
        assert!(
            tir.angle_between(soutien).to_degrees() > 20.0,
            "les deux coudes ne diffèrent que de {:.0}° — la pose reste symétrique",
            tir.angle_between(soutien).to_degrees()
        );
        // Le coude de TIR s'écarte du corps, celui de SOUTIEN rentre.
        assert!(tir.dot(droite) > 0.0, "le coude de tir ne s'ouvre pas");
        assert!(soutien.dot(droite) < 0.0, "le coude de soutien ne rentre pas");
        // Et les deux restent orientés vers le BAS : un coude en l'air est une
        // pose de danseur, pas de tireur.
        assert!(tir.dot(bas) > 0.4 && soutien.dot(bas) > 0.8);
    }

    /// Deux systèmes ne doivent pas écrire les mêmes os.
    ///
    /// Le corps déclare `aim_clip = "rifle_aim"` (pose capturée) ; l'IK des bras
    /// doit donc être éteinte. Ce test échoue si quelqu'un rallume l'une sans
    /// retirer l'autre — un conflit dont le symptôme serait « la pose de visée est
    /// bizarre », sans indice sur la cause.
    #[test]
    fn l_ik_des_bras_et_la_pose_capturee_ne_coexistent_pas() {
        let src = std::fs::read_to_string("../../assets/genomes/expedition_arme_main.toml")
            .expect("génome introuvable");
        let g: ArmeMainGenome = toml::from_str(&src).expect("génome mal formé");
        let equip =
            std::fs::read_to_string("../../assets/genomes/roguelite/roguelite_equipment.toml")
                .expect("génome d'équipement introuvable");
        let v: toml::Value = toml::from_str(&equip).expect("mal formé");
        let clip_de_visee = v["expedition_body"]["animation"]
            .get("aim_clip")
            .and_then(|c| c.as_str())
            .unwrap_or("");
        if !clip_de_visee.is_empty() {
            assert!(
                !g.visee.bras.actif,
                "le corps déclare la pose capturée « {clip_de_visee} » ET l'IK des bras \
                 est active : les deux écrivent les mêmes os, le plus tardif gagne, et le \
                 résultat n'est ni l'un ni l'autre"
            );
        }
        // Le suivi de buste, lui, DOIT rester : c'est la seule chose qu'un clip
        // figé ne sait pas faire — viser vers le haut.
        assert!(
            g.visee.suivi_du_buste > 0.0,
            "sans suivi de buste, viser vers le haut ne lève plus le canon"
        );
    }

    #[test]
    fn une_ouverture_nulle_rend_la_pose_symetrique() {
        // Le réglage doit rester capable de revenir au comportement d'avant :
        // s'il ne peut plus, ce n'est pas un réglage, c'est une opinion figée.
        let mut b = BrasGenome::default();
        b.coude_ouverture = 0.0;
        let (bas, droite) = (-Vec3::Y, Vec3::X);
        assert!(
            b.pole_coude(bas, droite, false)
                .abs_diff_eq(b.pole_coude(bas, droite, true), 1e-5)
        );
    }

    #[test]
    fn la_main_gauche_soutient_du_bon_cote() {
        // Elle doit venir vers l'axe du corps par rapport à la main droite,
        // sinon les deux mains s'écartent au lieu de se rejoindre sur l'arme.
        let b = BrasGenome::default();
        let (avant, haut, droite) = (Vec3::NEG_Z, Vec3::Y, Vec3::X);
        let d = b.cible(Vec3::ZERO, avant, haut, droite, 0.6, false);
        let g = b.cible(Vec3::ZERO, avant, haut, droite, 0.6, true);
        assert!(
            g.x > d.x,
            "la main de soutien doit venir du côté droit vers l'arme ({:.2} vs {:.2})",
            g.x,
            d.x
        );
    }

    #[test]
    fn viser_vers_le_haut_leve_la_cible() {
        // La cible est construite dans le repère du REGARD : lever la caméra
        // doit lever les mains sans qu'aucun terme ne le dise. C'est ce qui
        // évite d'avoir un jour à déclarer une pose « visée haute ».
        let b = BrasGenome::default();
        let bas = b.cible(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y, Vec3::X, 0.6, false);
        let haut_dir = (Vec3::NEG_Z + Vec3::Y).normalize();
        let haut = b.cible(Vec3::ZERO, haut_dir, Vec3::Y, Vec3::X, 0.6, false);
        assert!(haut.y > bas.y + 0.1, "viser haut ne lève pas les mains");
    }

    #[test]
    fn un_os_non_declare_ne_bouge_pas() {
        // Le génome ne cite que quelques os ; tous les autres appartiennent à
        // l'animation. Les toucher, même de l'identité, serait une écriture
        // inutile sur un composant à détection de changement.
        let g = genome_deux_poses();
        assert_eq!(g.rotation_os("Hips", 1.0), Quat::IDENTITY);
    }

    #[test]
    fn les_os_cites_couvrent_les_deux_tenues_sans_doublon() {
        let mut g = genome_deux_poses();
        g.pose_decontractee.insert("Spine2".into(), [0.0; 3]);
        let cites = g.os_cites();
        assert_eq!(cites, vec!["RightArm".to_string(), "Spine2".to_string()]);
    }

    #[test]
    fn le_genome_livre_donne_une_vraie_tenue_et_les_zooms_demandes() {
        // Lu depuis le disque : c'est la seule façon de prouver que le fichier
        // livré décrit VRAIMENT deux tenues distinctes. Deux poses identiques
        // compileraient, passeraient les tests de structure, et ne changeraient
        // rien à l'écran.
        let src = std::fs::read_to_string("../../assets/genomes/expedition_arme_main.toml")
            .expect("génome introuvable");
        let g: ArmeMainGenome = toml::from_str(&src).expect("génome mal formé");
        let v = &g.visee;
        // Le buste : la seule pose encore déclarée en angles. Les bras sont
        // passés sous IK le 2026-08-16 — les citer ici les ferait se battre
        // avec elle.
        assert!(
            v.rotation_os("Spine2", 1.0).angle_between(Quat::IDENTITY) > 1e-3,
            "le buste ne s'ouvre pas — viser ne se verrait presque pas"
        );
        for os in ["RightArm", "RightForeArm", "LeftArm", "LeftForeArm"] {
            assert!(
                !v.pose_tactique.contains_key(os) && !v.pose_decontractee.contains_key(os),
                "{os} est sous IK : le déclarer en angles ferait deux systèmes \
                 qui écrivent le même os, et c'est le plus fort qui gagnerait"
            );
        }
        // Et la cible des mains doit être atteignable : au-delà de 1, on demande
        // au bras d'être plus long qu'il n'est, et l'IK ne peut que le tendre.
        let b = v.bras;
        assert!(
            (0.2..0.95).contains(&b.portee_avant),
            "portée {} : hors de ce qu'un bras plié peut tenir",
            b.portee_avant
        );
        // Les zooms demandés le 2026-08-15 : aucun sur le pistolet et le
        // lance-roquettes, un vrai sur la mitraillette et le fusil de précision.
        use forgia_combat::weapons::WeaponType;
        assert_eq!(g.reglage(WeaponType::ModernAR).zoom, 1.0, "Pépin : aucun zoom");
        assert_eq!(
            g.reglage(WeaponType::RocketLauncher).zoom,
            1.0,
            "Boucherie : aucun zoom"
        );
        assert!(
            g.reglage(WeaponType::AssaultRifle).zoom > 1.0,
            "Bourrasque : zoom léger attendu"
        );
        assert!(
            g.reglage(WeaponType::Shotgun).zoom > g.reglage(WeaponType::AssaultRifle).zoom,
            "Madame Lenoir (le fusil de précision) doit zoomer plus que la mitraillette"
        );
    }

    #[test]
    fn le_fondu_met_le_temps_declare() {
        // Contrat de `suivre_le_bouton`, sans moteur : à 60 images/s et 0,16 s
        // de montée, il faut ~10 frames pour lever l'arme. Un fondu instantané
        // se lit comme une téléportation du bras.
        let montee = montee_par_defaut();
        let frames = (montee / (1.0 / 60.0)).ceil() as i32;
        assert!(
            (6..=20).contains(&frames),
            "{frames} frames pour lever l'arme — hors de la bande jouable"
        );
    }
}

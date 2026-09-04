//! cape — confier les six os de cape au solveur de mouvement secondaire.
//!
//! # Ce que ce module répare
//!
//! Le corps d'Expédition porte une cape (`Blaha2.001`) attachée à sa propre
//! peau, elle-même pilotée par six os `cloak_01`..`cloak_06` en chaîne sous
//! `root.001`, lui-même sous `Spine2`.
//!
//! **Aucun des 34 clips du corps n'anime ces six os.** La cape suivait donc le
//! buste comme une planche : elle tournait avec le torse, sans jamais retomber,
//! flotter, ni réagir à la course. Mesuré le 2026-08-18 par le contrôle du corps
//! livré, qui liste les os qu'aucun clip ne touche.
//!
//! # Pourquoi un solveur et pas un clip
//!
//! Une cape ne s'anime pas à la main : son mouvement dépend de ce que le
//! personnage vient de faire, pas d'une chorégraphie. Un clip « cape qui vole »
//! serait faux dès qu'on s'arrête. `forgia-secondary-motion` intègre déjà des
//! os-ressorts en Verlet — c'est exactement l'outil, et il n'avait aucun
//! consommateur actif.
//!
//! # Le piège déjà payé, deux fois
//!
//! Son solveur supposait l'axe long des os à `+Y`. L'audit d'animation du
//! 2026-06-04 a débranché la queue de Rex pour cette raison (« whip Verlet,
//! queue de travers ») en laissant la consigne de corriger. La chaîne de cape
//! court selon **−X** : la brancher sans corriger l'aurait tordue pareil. L'axe
//! se lit maintenant sur la pose de liaison, côté solveur.

use bevy::prelude::*;
use forgia_anim_debug::anim_sensor::ExternalAnimationTarget;
use forgia_core::prelude::{AppMode, GameMode, GameSet};
use forgia_secondary_motion::{SpringBone, SpringBoneChain};
use serde::Deserialize;

/// Couche **definition** : la souplesse d'une cape se juge à l'œil, en jeu.
const GENOME: &str = "assets/genomes/expedition_cape.toml";

/// L'os parent de la chaîne. Il porte l'échelle 0,01 du rig de cape — ce qui ne
/// gêne pas le solveur, qui n'écrit que des rotations.
const OS_RACINE: &str = "root.001";
/// Le préfixe des os de la chaîne.
const PREFIXE: &str = "cloak_";
const CHEVEUX_RACINE: &str = "cheveux_01";
const CHEVEUX_POINTE: &str = "cheveux_02";
/// Les deux os qui donnent le COTE du personnage sans rien supposer du repere
/// du rig : ils sont symetriques par construction (mesure sur le GLB, ±0,0671).
const BRAS_GAUCHE: &str = "LeftArm";
const BRAS_DROIT: &str = "RightArm";
/// En deca, le travers est du bruit de mesure et non un defaut a corriger.
const DECALAGE_NEGLIGEABLE_M: f32 = 0.01;
/// Sous cette longueur, la chaine n'est pas encore propagee : ses six os sont
/// empiles au meme endroit et toute direction lue serait du hasard. Mesuree en
/// jeu : 0,85 m.
const LONGUEUR_MINIMALE_MESURABLE_M: f32 = 0.10;

#[derive(Resource, Debug, Clone, Copy, Deserialize)]
pub struct CapeConfig {
    /// 0 = molle (tombe), 1 = rigide (colle au buste).
    #[serde(default = "raideur_par_defaut")]
    pub raideur: f32,
    /// 0 = oscille longtemps, 1 = figée.
    #[serde(default = "amorti_par_defaut")]
    pub amorti: f32,
    /// La pesanteur vue par le tissu. Plus faible que 9,81 : une cape est portée
    /// par l'air autant qu'elle est tirée vers le bas, et la vraie pesanteur
    /// donne un rideau, pas un vêtement.
    #[serde(default = "pesanteur_par_defaut")]
    pub pesanteur: f32,
    /// Ouverture maximale depuis la pose de repos, en degrés.
    #[serde(default = "angle_max_par_defaut")]
    pub angle_max_deg: f32,
    /// Rayon du corps que la cape ne traverse pas, en mètres.
    #[serde(default = "rayon_corps_par_defaut")]
    pub rayon_corps_m: f32,
    /// Le SOLVEUR pilote-t-il la cape ? `false` = pas de chaîne d'os-ressorts.
    /// Le balancement ci-dessous, lui, n'en a pas besoin.
    #[serde(default = "actif_par_defaut")]
    pub actif: bool,
    /// Le balancement conduit — la voie retenue le 2026-08-21.
    #[serde(default)]
    pub balancement: crate::balancement::BalancementConfig,
    #[serde(default)]
    pub cheveux: HairConfig,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct HairConfig {
    #[serde(default = "hair_stiffness_default")]
    pub raideur: f32,
    #[serde(default = "hair_damping_default")]
    pub amorti: f32,
    #[serde(default = "hair_gravity_default")]
    pub pesanteur: f32,
    #[serde(default = "hair_angle_max_default")]
    pub angle_max_deg: f32,
    #[serde(default = "hair_rayon_default")]
    pub rayon_corps_m: f32,
}

fn hair_stiffness_default() -> f32 {
    0.55
}
fn hair_damping_default() -> f32 {
    0.78
}
fn hair_gravity_default() -> f32 {
    -1.5
}
fn hair_angle_max_default() -> f32 {
    25.0
}
fn hair_rayon_default() -> f32 {
    0.10
}
/// 45° : au-delà, une cape cousue aux épaules ne s'ouvre plus, elle vole.
fn angle_max_par_defaut() -> f32 {
    45.0
}
/// 0,20 m autour de l'axe du corps. Repère mesuré sur le rig : les épaules sont
/// à ±0,067 m de l'axe, le buste habillé est plus large — à juger à l'œil, d'où
/// sa place au génome.
fn rayon_corps_par_defaut() -> f32 {
    0.20
}
fn actif_par_defaut() -> bool {
    true
}

impl Default for HairConfig {
    fn default() -> Self {
        Self {
            raideur: hair_stiffness_default(),
            amorti: hair_damping_default(),
            pesanteur: hair_gravity_default(),
            angle_max_deg: hair_angle_max_default(),
            rayon_corps_m: hair_rayon_default(),
        }
    }
}

fn raideur_par_defaut() -> f32 {
    0.35
}
fn amorti_par_defaut() -> f32 {
    0.7
}
fn pesanteur_par_defaut() -> f32 {
    -4.0
}

impl Default for CapeConfig {
    fn default() -> Self {
        Self {
            raideur: raideur_par_defaut(),
            amorti: amorti_par_defaut(),
            pesanteur: pesanteur_par_defaut(),
            angle_max_deg: angle_max_par_defaut(),
            rayon_corps_m: rayon_corps_par_defaut(),
            actif: actif_par_defaut(),
            balancement: crate::balancement::BalancementConfig::default(),
            cheveux: HairConfig::default(),
        }
    }
}

impl CapeConfig {
    #[must_use]
    pub fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                warn!("[cape] {GENOME} mal formé ({e}) — réglages par défaut");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Ce que l'accrochage a trouvé. Publié par le capteur.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct EtatCape {
    pub racine_trouvee: bool,
    pub os_de_chaine: usize,
    pub accrochee: bool,
    pub cheveux_trouves: usize,
    pub cheveux_accroches: bool,
    /// De combien la chaine sortait sur le cote a l'accrochage, en metres.
    /// Publie au capteur : c'est le defaut qu'on a vu a l'ecran avant de le
    /// mesurer, il doit rester lisible sans relancer Blender.
    pub decalage_lateral_m: f32,
    /// Qui porte la chaine, pour pouvoir VERIFIER qu'elle existe encore.
    ///
    /// 🚨 Sans ca, `accrochee` etait un loquet a sens unique : le corps est
    /// reconstruit a chaque changement de mode, la chaine meurt avec lui, et
    /// le loquet reste ferme. Mesure du 2026-08-21 dans le Hall :
    /// `accrochee: true` avec ZERO chaine vivante — l'avatar du Hall n'a donc
    /// jamais eu sa cape, et rien ne le disait.
    pub porteuse: Option<Entity>,
    pub porteuse_cheveux: Option<Entity>,
    /// Le recentrage a-t-il pu MESURER quelque chose ? Il echoue tant que les
    /// `GlobalTransform` de la scene fraiche ne sont pas propages : on retente
    /// alors, au lieu de latcher une correction fondee sur du hasard.
    pub recentree: bool,
}

pub struct ExpeditionCapePlugin;

impl Plugin for ExpeditionCapePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CapeConfig::charger())
            .init_resource::<EtatCape>()
            .init_resource::<crate::balancement::ChaineCape>()
            .init_resource::<crate::balancement::EtatBalancement>()
            .add_systems(
                Update,
                accrocher_le_balancement
                    .in_set(GameSet::Effects)
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(
                Update,
                crate::balancement::balancer_la_cape
                    .in_set(GameSet::Effects)
                    .after(accrocher_le_balancement)
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(
                Update,
                // 🚨 PAS de garde sur le mode. La cape appartient au CORPS, pas
                // a la carte : le Hall montre le meme avatar, et le filtre
                // `GameMode::Expedition` y laissait une etoffe raide et de
                // travers (constate a l'ecran le 2026-08-21). Le systeme sort
                // en une comparaison quand il n'y a rien a accrocher, et un
                // corps sans os `cloak_*` (le trooper du Roguelite) le fait
                // sortir des la recherche du nom.
                //
                // Dette assumee : ce module vit encore dans la crate d'un MODE
                // alors qu'il sert tous les modes (`fine-grained-crates`). Le
                // deplacer est un chantier a part.
                accrocher_la_cape
                    .in_set(GameSet::Effects)
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(Update, capteur_cape.in_set(GameSet::Sensors));
    }
}

/// Trouve `root.001` puis sa chaîne d'os `cloak_*`.
///
/// **Une seule source** : le solveur et le balancement lisent le même parcours.
/// La chaîne se suit par la HIÉRARCHIE, pas par le numéro du nom — un rig peut
/// renuméroter ses os, il ne peut pas mentir sur qui est l'enfant de qui.
fn trouver_la_chaine(
    q_noms: &Query<(Entity, &Name)>,
    q_enfants: &Query<&Children>,
) -> Option<(Entity, Vec<Entity>)> {
    let racine = q_noms
        .iter()
        .find(|(_, n)| n.as_str() == OS_RACINE)
        .map(|(e, _)| e)?;
    let mut chaine = Vec::new();
    let mut courant = racine;
    while let Ok(enfants) = q_enfants.get(courant) {
        let Some(suivant) = enfants.iter().find(|&e| {
            q_noms
                .get(e)
                .map(|(_, n)| n.as_str().starts_with(PREFIXE))
                .unwrap_or(false)
        }) else {
            break;
        };
        chaine.push(suivant);
        courant = suivant;
    }
    Some((racine, chaine))
}

/// Enregistre la chaîne pour le BALANCEMENT (pas de solveur, pas de ressort).
///
/// Capture la pose de liaison de chaque os : c'est la référence absolue que le
/// balancement réécrit chaque frame. Se revérifie comme l'accrochage du
/// solveur — un corps reconstruit emporte ses entités, et un `Vec<Entity>`
/// périmé pointerait dans le vide.
fn accrocher_le_balancement(
    cfg: Res<CapeConfig>,
    mut chaine_cape: ResMut<crate::balancement::ChaineCape>,
    q_noms: Query<(Entity, &Name)>,
    q_enfants: Query<&Children>,
    q_tf: Query<&Transform>,
) {
    // Le solveur a la main : on ne pilote pas les mêmes os à deux endroits.
    if cfg.actif {
        if !chaine_cape.vide() {
            chaine_cape.oublier();
        }
        return;
    }
    if chaine_cape
        .os
        .first()
        .is_some_and(|&premier| q_tf.get(premier).is_err())
    {
        chaine_cape.oublier();
    }
    if !chaine_cape.vide() {
        return;
    }
    let Some((racine, chaine)) = trouver_la_chaine(&q_noms, &q_enfants) else {
        return;
    };
    if chaine.len() < 2 {
        return;
    }
    let mut parents = vec![racine];
    parents.extend(chaine.iter().copied().take(chaine.len() - 1));
    let poses: Vec<Quat> = chaine
        .iter()
        .map(|&e| q_tf.get(e).map(|t| t.rotation).unwrap_or(Quat::IDENTITY))
        .collect();
    info!(
        "[cape] balancement conduit : {} os, poses de liaison capturées",
        chaine.len()
    );
    chaine_cape.os = chaine;
    chaine_cape.parents = parents;
    chaine_cape.poses = poses;
}

/// Trouve `root.001` et ses `cloak_*`, et pose la chaîne d'os-ressorts.
///
/// Tourne à chaque frame mais sort en une comparaison de booléen une fois la
/// cape accrochée : le corps arrive en tâche de fond, on ne sait pas à quelle
/// frame, et un `OnEnter` le manquerait.
fn accrocher_la_cape(
    mut commands: Commands,
    cfg: Res<CapeConfig>,
    mut etat: ResMut<EtatCape>,
    q_noms: Query<(Entity, &Name)>,
    q_enfants: Query<&Children>,
    q_parent: Query<&ChildOf>,
    q_gt: Query<&GlobalTransform>,
    q_chaines: Query<&SpringBoneChain>,
) {
    // On VERIFIE que ce qu'on a accroche existe encore, au lieu de le croire.
    // Un corps reconstruit (changement de mode, changement d'equipement) emporte
    // sa chaine ; le loquet, lui, survivait.
    if etat.porteuse.is_some_and(|e| q_chaines.get(e).is_err()) {
        etat.accrochee = false;
        etat.porteuse = None;
        etat.racine_trouvee = false;
        etat.os_de_chaine = 0;
    }
    if etat.porteuse_cheveux.is_some_and(|e| q_chaines.get(e).is_err()) {
        etat.cheveux_accroches = false;
        etat.porteuse_cheveux = None;
        etat.cheveux_trouves = 0;
    }
    // Le recentrage se retente tant qu'il n'a rien pu mesurer : la pose des os
    // n'est propagee qu'en fin de frame, et l'accrochage, lui, a lieu des que
    // les os EXISTENT.
    if etat.accrochee && !etat.recentree {
        let mesure = etat
            .porteuse
            .and_then(|porteuse| q_chaines.get(porteuse).ok().map(|c| (porteuse, c)))
            .and_then(|(porteuse, chaine)| {
                mesurer_le_travers(porteuse, &chaine.bones, &q_noms, &q_gt)
            });
        if let Some(mesure) = mesure {
            etat.decalage_lateral_m = mesure;
            etat.recentree = true;
        }
    }
    // Interrupteur du genome : sans lui, la seule facon de rendre sa pose de
    // liaison a la cape serait de retirer du code — donc de ne plus pouvoir
    // comparer les deux en jeu.
    if !cfg.actif {
        return;
    }
    if etat.accrochee && etat.cheveux_accroches {
        return;
    }
    if !etat.cheveux_accroches {
        let hair_root = q_noms
            .iter()
            .find(|(_, name)| name.as_str() == CHEVEUX_RACINE)
            .map(|(entity, _)| entity);
        let hair_tip = q_noms
            .iter()
            .find(|(_, name)| name.as_str() == CHEVEUX_POINTE)
            .map(|(entity, _)| entity);
        etat.cheveux_trouves = usize::from(hair_root.is_some()) + usize::from(hair_tip.is_some());
        if let (Some(root), Some(tip)) = (hair_root, hair_tip) {
            let parent = q_parent.get(root).map(ChildOf::parent).unwrap_or(root);
            let hair = cfg.cheveux;
            for bone in [root, tip] {
                commands.entity(bone).insert((
                    SpringBone {
                        stiffness: hair.raideur,
                        damping: hair.amorti,
                        gravity: Vec3::new(0.0, hair.pesanteur, 0.0),
                        angle_max_rad: hair.angle_max_deg.to_radians(),
                        rayon_corps_m: hair.rayon_corps_m,
                    },
                    ExternalAnimationTarget,
                ));
            }
            commands.entity(parent).insert(SpringBoneChain {
                bones: vec![root, tip],
                ..default()
            });
            etat.cheveux_accroches = true;
            etat.porteuse_cheveux = Some(parent);
            info!("[cape] 2 os de cheveux accrochés au mouvement secondaire");
        }
    }
    if etat.accrochee {
        return;
    }
    let Some((_racine, chaine)) = trouver_la_chaine(&q_noms, &q_enfants) else {
        return;
    };
    etat.racine_trouvee = true;
    etat.os_de_chaine = chaine.len();
    // Deux os au minimum : une chaîne d'un seul n'a aucune direction à suivre.
    if chaine.len() < 2 {
        return;
    }

    // 🚨 LA CHAINE DEMARRE A LA COUTURE, PAS SOUS ELLE.
    //
    // `root.001` est l'origine de l'armature de cape : mesuree en jeu, elle est
    // **1,33 m SOUS** `cloak_01` (y 5,298 contre 6,626 — au niveau des genoux,
    // pendant que la couture est aux epaules). L'accrocher comme racine faisait
    // simuler, en premier segment, un « os » de 1,395 m montant des genoux aux
    // epaules : la gravite le faisait basculer, et il EMPORTAIT la couture.
    // Symptome a l'ecran : « la cape ne tient pas sur toute la couture ».
    //
    // Une chaine de ressorts doit partir de l'os ANCRE. Ici c'est `cloak_01`,
    // soude aux epaules et pilote par le buste : il reste hors simulation, et
    // seuls les maillons suivants pendent.
    let Some((&couture, pendants)) = chaine.split_first() else {
        return;
    };
    if pendants.is_empty() {
        return;
    }
    for &os in pendants {
        commands.entity(os).insert((
            SpringBone {
                stiffness: cfg.raideur,
                damping: cfg.amorti,
                gravity: Vec3::new(0.0, cfg.pesanteur, 0.0),
                angle_max_rad: cfg.angle_max_deg.to_radians(),
                rayon_corps_m: cfg.rayon_corps_m,
            },
            ExternalAnimationTarget,
        ));
    }
    commands.entity(couture).insert(SpringBoneChain {
        bones: pendants.to_vec(),
        ..default()
    });
    if let Some(mesure) = mesurer_le_travers(couture, pendants, &q_noms, &q_gt) {
        etat.decalage_lateral_m = mesure;
        etat.recentree = true;
    }
    etat.accrochee = true;
    etat.porteuse = Some(couture);
    info!(
        "[cape] {} os accrochés au solveur de mouvement secondaire",
        chaine.len()
    );
}

/// Mesure de combien la chaîne sort sur le côté du personnage (mètres, au bout).
///
/// 🚨 Elle CORRIGEAIT, en posant une rotation sur `root.001`. Elle ne corrige
/// plus : depuis que la chaîne est ancrée à la couture, la direction de l'ourlet
/// est décidée par la gravité, et tourner `root.001` ferait pivoter la couture
/// elle-même — l'ancre. Une mesure publiée vaut mieux qu'une correction qui
/// déplace le problème (essai du 2026-08-21 : cape « dans le mauvais sens »).
///
/// # Le défaut que cette fonction corrige
///
/// L'armature de cape est SÉPARÉE (`root.001`, échelle 0,01) et sa chaîne court
/// selon −X dans son propre repère : lue dans le rig, elle est horizontale.
/// Sa rotation la ramène vers le bas, mais **de travers** — mesuré en jeu le
/// 2026-08-21 : 10 cm de côté à la racine, 23 cm au bout, sur 85 cm de chaîne.
/// À l'écran, une étoffe raide qui sort sur le côté au lieu de tomber dans le
/// dos.
///
/// `root.001` n'est animé par aucun clip (il figure dans les `cibles_inertes`
/// du capteur d'animation), donc la correction posée ici TIENT : rien ne la
/// réécrit à la frame suivante.
///
/// # Pourquoi une rotation et pas une translation
///
/// La racine, elle, est bien centrée dans le rig (`x = 0`, mesuré sur le GLB,
/// et le buste est symétrique à ±0,0671 sur les bras). Ce n'est donc pas la
/// chaîne qui est posée à côté : c'est son armature qui est TOURNÉE. Translater
/// aurait déplacé le défaut sans le corriger.
///
/// # Pourquoi l'axe latéral se mesure au lieu de se supposer
///
/// `LeftArm − RightArm` donne le côté du personnage sans rien supposer du
/// repère du rig — deux os symétriques, mesurés sur la pose du moment. Prendre
/// « X du buste » aurait marché sur ce rig-ci et faux sur le suivant.
fn mesurer_le_travers(
    racine: Entity,
    chaine: &[Entity],
    q_noms: &Query<(Entity, &Name)>,
    q_gt: &Query<&GlobalTransform>,
) -> Option<f32> {
    let &bout = chaine.last()?;
    let os_par_nom = |cible: &str| {
        q_noms
            .iter()
            .find(|(_, n)| n.as_str() == cible)
            .and_then(|(e, _)| q_gt.get(e).ok())
            .map(GlobalTransform::translation)
    };
    let (Some(gauche), Some(droit)) = (os_par_nom(BRAS_GAUCHE), os_par_nom(BRAS_DROIT)) else {
        warn!(
            "[cape] {BRAS_GAUCHE}/{BRAS_DROIT} introuvables — recentrage impossible, \
             la chaîne garde le travers de son armature"
        );
        return None;
    };
    let lateral = (gauche - droit).try_normalize()?;
    let (Ok(gt_racine), Ok(gt_bout)) = (q_gt.get(racine), q_gt.get(bout)) else {
        return None;
    };
    let bras = gt_bout.translation() - gt_racine.translation();
    // 🚨 NE PAS MESURER UNE POSE QUI N'EST PAS ENCORE PROPAGEE.
    //
    // L'accrochage a lieu dès que les os existent, et les `GlobalTransform`
    // d'une scène fraîchement instanciée ne sont propagés qu'en fin de frame :
    // les six os peuvent encore être empilés au même endroit. Mesurer là-dessus
    // rendrait une direction quelconque, et la rotation corrective l'écrirait
    // pour de bon. La chaîne mesure 0,85 m en jeu — sous 0,10 m, on ne corrige
    // rien et on laissera la frame suivante le faire.
    if bras.length() < LONGUEUR_MINIMALE_MESURABLE_M {
        return None;
    }
    let decalage = bras.dot(lateral);
    // Sous le millimètre, il n'y a rien à corriger : ne pas écrire une rotation
    // pour du bruit de mesure.
    if decalage.abs() < DECALAGE_NEGLIGEABLE_M {
        return Some(decalage);
    }
    info!(
        "[cape] couture ancrée, ourlet libre — travers mesuré {:.0} cm (non corrigé :          c'est la pose de liaison de l'asset)",
        decalage.abs() * 100.0
    );
    Some(decalage)
}

/// `forgia2_expedition_cape.json`, 1 Hz.
fn capteur_cape(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut dernier_nombre_chaines: Local<usize>,
    mode: Res<State<GameMode>>,
    cfg: Res<CapeConfig>,
    etat: Res<EtatCape>,
    q_chaines: Query<&SpringBoneChain>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let en_expedition = *mode.get() == GameMode::Expedition;
    // `initialized` est posé par le solveur à son premier passage : c'est la
    // preuve qu'il a VU la chaîne, pas seulement qu'on la lui a donnée.
    let chaines_initialisees = q_chaines
        .iter()
        .filter(|c| c.initialized && !c.bones.is_empty())
        .count();

    // 🚨 NE PAS EFFACER LA DERNIERE MESURE EN SORTANT DU MODE.
    //
    // Le 2026-08-18, TROIS parties de suite ont été jouées, observées, et leur
    // mesure perdue : à la sortie d'Expédition le capteur se réécrivait en
    // « hors Expedition » avec des sentinelles, et la seule trace de ce qui
    // s'était passé disparaissait. L'utilisateur décrivait un symptôme que le
    // dispositif venait d'effacer.
    //
    // Un capteur mesure un état FUGACE : le mode dure quelques minutes, la
    // lecture se fait après. Retenir la dernière valeur réelle ne coûte rien et
    // c'est la seule façon qu'un capteur de mode serve à quelque chose.
    if en_expedition {
        *dernier_nombre_chaines = chaines_initialisees;
    }
    let chaines_initialisees = if en_expedition {
        chaines_initialisees
    } else {
        *dernier_nombre_chaines
    };
    let chaines_attendues = usize::from(etat.accrochee) + usize::from(etat.cheveux_accroches);
    let solveur_a_pris = chaines_initialisees >= chaines_attendues && chaines_attendues > 0;

    let (severity, next_step) = if !en_expedition {
        (
            "info",
            "hors Expedition — valeurs retenues de la derniere session dans le mode",
        )
    } else if !etat.racine_trouvee {
        (
            "warn",
            "CAPE_SANS_RACINE : aucun os « root.001 » dans la scene — le corps charge n'a pas de cape, ou son rig a ete renomme. Verifier expedition_body.model au genome d'equipement",
        )
    } else if etat.os_de_chaine < 2 {
        (
            "warn",
            "CHAINE_TROP_COURTE : « root.001 » trouve mais moins de deux os « cloak_* » dessous — la cape restera rigide. Le rig d'origine en compte six",
        )
    } else if etat.cheveux_trouves < 2 || !etat.cheveux_accroches {
        (
            "warn",
            "CHEVEUX_INERTES : cheveux_01/02 ne sont pas tous branches au mouvement secondaire",
        )
    } else if !solveur_a_pris {
        (
            "warn",
            "SOLVEUR_INACTIF : la chaine est posee mais forgia-secondary-motion ne l'a pas initialisee — verifier que ForgiaSecondaryMotionPlugin est bien ajoute (forgia-game/src/lib.rs)",
        )
    } else {
        ("ok", "")
    };

    // 🚨 Publier les trois réglages ne disait RIEN de ce qu'ils produisent :
    // « raideur 0,35 · amorti 0,7 · pesanteur -4,0 » se lisait comme une cape
    // souple, alors que le solveur la tenait plaquée et l'empêchait de tomber
    // (0,095 m/s, invisible). On publie donc les deux grandeurs que l'œil
    // constate — la vitesse de chute atteignable et la raideur réellement
    // appliquée — pour que « la cape ne tombe pas » se chiffre au lieu de se
    // discuter. Repère : une étoffe se lit à partir de ~0,5 m/s.
    let dt_60 = 1.0 / 60.0;
    let chute_max_ms = forgia_secondary_motion::solver::chute_terminale_ms(
        cfg.pesanteur,
        cfg.amorti,
        dt_60,
    );
    let json = format!(
        r#"{{"id":"expedition_cape","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"racine_trouvee":{},"os_de_chaine":{},"accrochee":{},"cheveux_trouves":{},"cheveux_accroches":{},"chaines_initialisees":{chaines_initialisees},"chaines_attendues":{chaines_attendues},"solveur_a_pris":{solveur_a_pris},"raideur":{:.2},"amorti":{:.2},"pesanteur":{:.2},"chute_max_ms":{:.2},"chute_visible":{},"decalage_lateral_corrige_m":{:.3}}}"#,
        time.elapsed_secs(),
        etat.racine_trouvee,
        etat.os_de_chaine,
        etat.accrochee,
        etat.cheveux_trouves,
        etat.cheveux_accroches,
        cfg.raideur,
        cfg.amorti,
        cfg.pesanteur,
        chute_max_ms,
        chute_max_ms >= 0.5,
        etat.decalage_lateral_m,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_cape.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les trois réglages restent dans des bornes qui produisent une cape, pas
    /// un drapeau rigide ni un chiffon qui traverse le sol.
    #[test]
    fn les_reglages_par_defaut_font_une_cape() {
        let c = CapeConfig::default();
        assert!((0.1..0.6).contains(&c.raideur), "raideur {}", c.raideur);
        assert!((0.5..0.9).contains(&c.amorti), "amorti {}", c.amorti);
        // Plus faible que la pesanteur réelle : une cape est portée par l'air.
        assert!(c.pesanteur < 0.0 && c.pesanteur > -9.81);
        assert!((0.1..0.8).contains(&c.cheveux.raideur));
        assert!((0.5..0.95).contains(&c.cheveux.amorti));
        assert!(c.cheveux.pesanteur < 0.0 && c.cheveux.pesanteur > -9.81);
    }

    /// Le corps LIVRÉ porte bien la chaîne que ce module cherche. Sans ce test,
    /// un renommage du rig rendrait le module muet sans que rien n'échoue — et
    /// c'est exactement ainsi que la cape est restée rigide sans qu'on le voie.
    #[test]
    fn le_corps_livre_porte_la_chaine_de_cape() {
        let glb =
            std::path::Path::new("../../assets/models/characters/stylized/stylized_male_fusil.glb");
        if !glb.exists() {
            return; // corps absent (CI sans assets) — le capteur couvre ce cas
        }
        let d = std::fs::read(glb).expect("corps lisible");
        let lg = u32::from_le_bytes([d[12], d[13], d[14], d[15]]) as usize;
        let json = String::from_utf8_lossy(&d[20..20 + lg]);
        assert!(
            json.contains(OS_RACINE),
            "« {OS_RACINE} » absent du corps livré"
        );
        let os = (1..=6)
            .filter(|i| json.contains(&format!("{PREFIXE}0{i}")))
            .count();
        assert!(
            os >= 2,
            "seulement {os} os « {PREFIXE}* » dans le corps livré"
        );
    }
}

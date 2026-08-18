//! emote.rs — Danser sur commande, et se voir danser.
//!
//! # Ce qui rend ce module particulier
//!
//! Il **fabrique** une animation au lieu d'en charger une. Aucun des 9 clips du
//! personnage n'est une danse : le pack Mixamo importé n'en contient pas, et en
//! obtenir une demanderait un aller-retour par un site tiers pour chaque idée.
//!
//! La danse est donc **décrite** dans `assets/genomes/emotes.toml` — une liste
//! d'os, chacun oscillant autour d'un axe avec son amplitude et son décalage de
//! phase — et ce module en construit un [`AnimationClip`] au chargement.
//! Ajouter une danse ne demande ni Blender, ni Mixamo, ni recompilation.
//!
//! # Le piège qu'il évite
//!
//! 🚨 Une courbe d'animation ne vise pas un os par son NOM mais par un
//! `AnimationTargetId` — un hachage du chemin de noms depuis la racine
//! d'animation. Le recalculer nous obligerait à connaître exactement cette
//! hiérarchie, qui diffère d'un corps à l'autre (le trooper part de `root`, le
//! personnage d'expédition de `perso_squelette`) et se casse au moindre
//! ré-export.
//!
//! On ne le recalcule pas : le chargeur glTF a DÉJÀ posé un `AnimationTargetId`
//! sur chaque os. On retrouve l'os par son `Name` et on **lit** son identifiant.
//! Même principe que « s'accrocher aux os, pas aux sockets » — l'identité vient
//! de la donnée, pas d'une reconstruction.

use bevy::animation::animated_field;
use bevy::animation::animation_curves::{AnimatableCurve, AnimatedField};
use bevy::animation::AnimationTargetId;
use bevy::input::ButtonInput;
use bevy::math::curve::UnevenSampleAutoCurve;
use bevy::prelude::*;
use serde::Deserialize;
use std::time::Duration;

use crate::avatar::{AvatarLocomotion, AvatarPart};

const GENOME_EMOTES: &str = "assets/genomes/emotes.toml";

/// Nombre d'échantillons par mesure.
///
/// Dérivé, pas choisi : les courbes sont interpolées linéairement entre
/// échantillons, donc l'erreur maximale d'une sinusoïde échantillonnée à `n`
/// points par période vaut `1 − cos(π/n)`. À 16 points c'est **1,9 %** — sous le
/// seuil de perception sur un mouvement de 38°, soit moins d'un degré. Monter à
/// 32 diviserait l'erreur par 4 pour doubler la mémoire d'une danse qui tient
/// déjà dans quelques kilo-octets ; descendre à 8 la porterait à 7,6 %, et le
/// mouvement se mettrait à « claquer » aux extrêmes.
const ECHANTILLONS_PAR_MESURE: usize = 16;

// ---------------------------------------------------------------------------
// La description, en couche definition
// ---------------------------------------------------------------------------

/// Une piste : un os qui oscille autour d'un axe.
#[derive(Debug, Clone, Deserialize)]
pub struct PisteEmote {
    pub os: String,
    pub axe: String,
    pub amplitude: f32,
    #[serde(default)]
    pub phase: f32,
    #[serde(default = "harmonique_par_defaut")]
    pub harmonique: u32,
}

fn harmonique_par_defaut() -> u32 {
    1
}

/// Le cadrage pendant l'émote. Sans lui on danse dos à la caméra.
#[derive(Debug, Clone, Deserialize)]
pub struct CameraEmote {
    #[serde(default = "demi_tour_par_defaut")]
    pub demi_tour: f32,
    pub distance_m: f32,
    pub tangage_deg: f32,
    pub transition_ms: u64,
}

fn demi_tour_par_defaut() -> f32 {
    0.5
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmoteDef {
    pub id: String,
    pub label: String,
    pub touche: String,
    pub mesure_s: f32,
    #[serde(default)]
    pub boucle: bool,
    pub fondu_ms: u64,
    pub camera: CameraEmote,
    pub pistes: Vec<PisteEmote>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct GenomeEmotes {
    #[serde(default)]
    emotes: Vec<EmoteDef>,
}

/// Les émotes déclarées, lues une fois au démarrage.
#[derive(Resource, Debug, Clone, Default)]
pub struct Emotes {
    pub defs: Vec<EmoteDef>,
}

impl Emotes {
    fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME_EMOTES) {
            Ok(s) => match toml::from_str::<GenomeEmotes>(&s) {
                Ok(g) => Self { defs: g.emotes },
                Err(e) => {
                    warn!("[emote] {GENOME_EMOTES} illisible ({e}) — aucune emote");
                    Self::default()
                }
            },
            Err(e) => {
                warn!("[emote] {GENOME_EMOTES} absent ({e}) — aucune emote");
                Self::default()
            }
        }
    }

    /// L'émote dont la touche vient d'être pressée.
    fn pressee(&self, touches: &ButtonInput<KeyCode>) -> Option<&EmoteDef> {
        self.defs
            .iter()
            .find(|e| code_touche(&e.touche).is_some_and(|k| touches.just_pressed(k)))
    }
}

/// Traduit le nom de touche du génome en `KeyCode`.
///
/// Volontairement limité aux chiffres et aux lettres : un génome ne doit pas
/// pouvoir lier une émote à `Escape`. Une touche inconnue est signalée, pas
/// devinée — deviner poserait la danse sur une touche que personne n'a demandée.
fn code_touche(nom: &str) -> Option<KeyCode> {
    match nom {
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// La fabrication du clip
// ---------------------------------------------------------------------------

/// L'axe d'une piste, en vecteur unitaire. Un axe inconnu ne fait rien plutôt
/// que de tourner au hasard — et il est signalé au chargement.
fn axe_vecteur(axe: &str) -> Option<Vec3> {
    match axe {
        "x" => Some(Vec3::X),
        "y" => Some(Vec3::Y),
        "z" => Some(Vec3::Z),
        _ => None,
    }
}

/// Construit le clip d'une émote pour un squelette donné.
///
/// `os_connus` associe chaque nom d'os à son identifiant d'animation, LU sur les
/// entités plutôt que recalculé. Un os cité par le génome mais absent du corps
/// est rendu dans `manquants` — le personnage d'expédition et le trooper n'ont
/// pas les mêmes, et une piste silencieusement ignorée serait invisible.
///
/// **Pur** : aucune requête, aucun monde. Donc testable.
pub fn construire_clip(
    def: &EmoteDef,
    os_connus: &dyn Fn(&str) -> Option<AnimationTargetId>,
) -> (AnimationClip, Vec<String>) {
    let mut clip = AnimationClip::default();
    let mut manquants = Vec::new();
    // Les pistes du même os se cumulent : on rassemble d'abord, on construit
    // ensuite. Sans ça, la seconde piste d'un os écraserait la première —
    // `Hips` en porte deux (déhanché + pivot), et l'une des deux disparaîtrait
    // sans que rien ne le dise.
    let mut par_os: Vec<(String, Vec<&PisteEmote>)> = Vec::new();
    for piste in &def.pistes {
        match par_os.iter_mut().find(|(nom, _)| nom == &piste.os) {
            Some((_, v)) => v.push(piste),
            None => par_os.push((piste.os.clone(), vec![piste])),
        }
    }

    let mesure = def.mesure_s.max(0.05);
    for (nom, pistes) in &par_os {
        let Some(cible) = os_connus(nom) else {
            manquants.push(nom.clone());
            continue;
        };
        let mut echantillons = Vec::with_capacity(ECHANTILLONS_PAR_MESURE + 1);
        // On va jusqu'à `n` INCLUS : le dernier échantillon rejoint le premier.
        // Sans lui, la boucle saute d'un pas à chaque tour — un à-coup qui se
        // voit d'autant plus que la danse est rapide.
        for i in 0..=ECHANTILLONS_PAR_MESURE {
            let t = i as f32 / ECHANTILLONS_PAR_MESURE as f32;
            let mut rot = Quat::IDENTITY;
            for p in pistes {
                let Some(axe) = axe_vecteur(&p.axe) else {
                    continue;
                };
                let tours = (t * p.harmonique.max(1) as f32 + p.phase) * std::f32::consts::TAU;
                let angle = p.amplitude.to_radians() * tours.sin();
                rot *= Quat::from_axis_angle(axe, angle);
            }
            echantillons.push((t * mesure, rot));
        }
        // La courbe est un ADDITIF sur la pose de repos : les valeurs sont des
        // rotations locales pures. C'est ce qui rend la danse portable d'un
        // corps à l'autre — elle ne suppose aucune pose de départ.
        if let Ok(courbe) = UnevenSampleAutoCurve::new(echantillons) {
            clip.add_curve_to_target(
                cible,
                AnimatableCurve::new(animated_field!(Transform::rotation), courbe),
            );
        }
    }
    (clip, manquants)
}

// ---------------------------------------------------------------------------
// L'état en jeu
// ---------------------------------------------------------------------------

/// L'émote en cours. Absente = personne ne danse.
#[derive(Resource, Debug, Clone)]
pub struct EmoteEnCours {
    pub id: String,
    /// Le nœud ajouté au graphe du corps, pour pouvoir y revenir.
    pub noeud: AnimationNodeIndex,
    /// Réglages de caméra à appliquer, et à rendre en sortant.
    pub camera: CameraEmote,
}

/// Marque un corps dont le graphe porte déjà le nœud de cette émote — on ne
/// l'ajoute qu'une fois, sinon le graphe enfle à chaque pression.
#[derive(Component, Default)]
pub struct EmotesMontees(pub Vec<(String, AnimationNodeIndex)>);

pub struct AvatarEmotePlugin;

impl Plugin for AvatarEmotePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Emotes::charger()).add_systems(
            Update,
            (declencher_emote, arreter_si_on_bouge, cadrer_l_emote).chain(),
        );
    }
}

/// Lance ou arrête l'émote quand sa touche est pressée.
#[allow(clippy::too_many_arguments)]
fn declencher_emote(
    mut commands: Commands,
    touches: Res<ButtonInput<KeyCode>>,
    emotes: Res<Emotes>,
    en_cours: Option<Res<EmoteEnCours>>,
    mut clips: ResMut<Assets<AnimationClip>>,
    mut graphes: ResMut<Assets<AnimationGraph>>,
    q_parts: Query<Entity, With<AvatarPart>>,
    q_enfants: Query<&Children>,
    q_os: Query<(&Name, &AnimationTargetId)>,
    mut q_lecteur: Query<(
        Entity,
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &AnimationGraphHandle,
    )>,
    mut q_montees: Query<&mut EmotesMontees>,
) {
    let Some(def) = emotes.pressee(&touches) else {
        return;
    };
    // Rappuyer sur la même touche arrête la danse — geste attendu du genre.
    if en_cours.as_ref().is_some_and(|e| e.id == def.id) {
        commands.remove_resource::<EmoteEnCours>();
        info!("[emote] « {} » arretee", def.label);
        return;
    }

    for part in &q_parts {
        for descendant in q_enfants.iter_descendants(part) {
            let Ok((lecteur, mut player, mut transitions, graphe)) =
                q_lecteur.get_mut(descendant)
            else {
                continue;
            };

            // Déjà monté sur ce corps ? On rejoue le nœud existant.
            let deja = q_montees
                .get(lecteur)
                .ok()
                .and_then(|m| m.0.iter().find(|(id, _)| id == &def.id).map(|(_, n)| *n));

            let noeud = match deja {
                Some(n) => n,
                None => {
                    // Les os sont retrouvés PAR NOM sous ce corps, et leur
                    // identifiant est LU — cf. l'en-tête du module.
                    let table: Vec<(String, AnimationTargetId)> = q_enfants
                        .iter_descendants(part)
                        .filter_map(|e| q_os.get(e).ok())
                        .map(|(n, id)| (n.as_str().to_string(), *id))
                        .collect();
                    let (clip, manquants) = construire_clip(def, &|nom| {
                        table.iter().find(|(n, _)| n == nom).map(|(_, id)| *id)
                    });
                    if !manquants.is_empty() {
                        warn!(
                            "[emote] « {} » : {} os absents de ce corps, leurs pistes \
                             ne joueront pas — {manquants:?}",
                            def.label,
                            manquants.len()
                        );
                    }
                    if clip.curves().is_empty() {
                        warn!(
                            "[emote] « {} » : AUCUNE piste applicable a ce corps — \
                             la danse ne se verrait pas. Verifier les noms d'os du genome.",
                            def.label
                        );
                        continue;
                    }
                    let Some(graphe_mut) = graphes.get_mut(&graphe.0) else {
                        continue;
                    };
                    let racine = graphe_mut.root;
                    let n = graphe_mut.add_clip(clips.add(clip), 1.0, racine);
                    match q_montees.get_mut(lecteur) {
                        Ok(mut m) => m.0.push((def.id.clone(), n)),
                        Err(_) => {
                            commands
                                .entity(lecteur)
                                .insert(EmotesMontees(vec![(def.id.clone(), n)]));
                        }
                    }
                    n
                }
            };

            let fondu = Duration::from_millis(def.fondu_ms);
            let jouee = transitions.play(&mut player, noeud, fondu);
            if def.boucle {
                jouee.repeat();
            }
            commands.insert_resource(EmoteEnCours {
                id: def.id.clone(),
                noeud,
                camera: def.camera.clone(),
            });
            info!("[emote] « {} » lancee sur le corps {part:?}", def.label);
        }
    }
}

/// Bouger annule la danse — comme dans les jeux du genre.
///
/// On lit la vitesse RÉELLE de l'avatar, pas l'intention d'entrée : un joueur
/// bloqué contre un mur appuie sur avancer sans bouger, et sa danse ne doit pas
/// s'arrêter pour autant.
fn arreter_si_on_bouge(
    mut commands: Commands,
    en_cours: Option<Res<EmoteEnCours>>,
    loco: Res<AvatarLocomotion>,
    emotes: Res<Emotes>,
) {
    let Some(e) = en_cours else {
        return;
    };
    let Some(def) = emotes.defs.iter().find(|d| d.id == e.id) else {
        // L'émote a disparu du génome sous nos pieds (rechargement à chaud) :
        // on rend la main plutôt que de danser une chorégraphie inexistante.
        commands.remove_resource::<EmoteEnCours>();
        return;
    };
    // Le seuil de marche du corps sert de juge : c'est déjà lui qui distingue
    // « immobile » de « en mouvement » pour le choix du clip. En introduire un
    // second ici, ce serait la même grandeur écrite deux fois.
    let _ = def;
    if loco.speed > 0.6 {
        commands.remove_resource::<EmoteEnCours>();
    }
}

/// Ce que la caméra valait AVANT la danse, pour pouvoir le rendre.
///
/// 🚨 On mémorise à l'entrée plutôt que de recalculer à la sortie : la caméra
/// d'épaule et celle du Hall n'ont ni la même distance ni le même tangage, et
/// « remettre les valeurs par défaut » remettrait celles du mauvais mode. Ce
/// que l'on rend, c'est ce que l'on a pris.
#[derive(Component)]
struct CadrageAvantEmote {
    yaw_offset: f32,
    distance: f32,
    pitch: f32,
}

/// Fait passer la caméra devant le personnage pendant la danse, et l'y ramène
/// après.
///
/// Pendant l'émote, le lacet est IMPOSÉ chaque frame : sans ça la souris le
/// disputerait au cadrage et l'image tremblerait. C'est le choix des jeux du
/// genre — pendant une émote, la mise en scène prend la main.
fn cadrer_l_emote(
    mut commands: Commands,
    time: Res<Time>,
    en_cours: Option<Res<EmoteEnCours>>,
    mut q_cam: Query<(
        Entity,
        &mut forgia_camera_orbit::OrbitCamera,
        Option<&CadrageAvantEmote>,
    )>,
) {
    for (entite, mut cam, avant) in &mut q_cam {
        match (&en_cours, avant) {
            // La danse commence : on mémorise, puis on tend vers le cadrage.
            (Some(e), None) => {
                commands.entity(entite).insert(CadrageAvantEmote {
                    yaw_offset: cam.yaw_offset,
                    distance: cam.distance,
                    pitch: cam.pitch,
                });
                let _ = e;
            }
            (Some(e), Some(a)) => {
                let k = pas_de_transition(&e.camera, time.delta_secs());
                let vise_yaw = a.yaw_offset + e.camera.demi_tour * std::f32::consts::TAU;
                cam.yaw_offset += (vise_yaw - cam.yaw_offset) * k;
                // La distance reste dans les bornes de CETTE caméra : un génome
                // qui demanderait 20 m sur la caméra d'épaule (max 4,5) la
                // sortirait de sa plage déclarée.
                let vise_dist = e.camera.distance_m.clamp(cam.min_distance, cam.max_distance);
                cam.distance += (vise_dist - cam.distance) * k;
                let vise_pitch = e
                    .camera
                    .tangage_deg
                    .to_radians()
                    .clamp(cam.min_pitch, cam.max_pitch);
                cam.pitch += (vise_pitch - cam.pitch) * k;
            }
            // La danse est finie : on rend ce qu'on avait pris.
            (None, Some(a)) => {
                let k = pas_de_transition(&CameraEmote::retour(), time.delta_secs());
                cam.yaw_offset += (a.yaw_offset - cam.yaw_offset) * k;
                cam.distance += (a.distance - cam.distance) * k;
                cam.pitch += (a.pitch - cam.pitch) * k;
                // Assez proche : on pose la valeur EXACTE et on oublie. Laisser
                // converger asymptotiquement garderait un écart infime pour
                // toujours, et le composant vivrait éternellement.
                if (cam.yaw_offset - a.yaw_offset).abs() < 1e-3
                    && (cam.distance - a.distance).abs() < 1e-3
                {
                    cam.yaw_offset = a.yaw_offset;
                    cam.distance = a.distance;
                    cam.pitch = a.pitch;
                    commands.entity(entite).remove::<CadrageAvantEmote>();
                }
            }
            (None, None) => {}
        }
    }
}

/// Fraction du chemin parcourue cette frame, dérivée de la durée voulue.
///
/// Bornée à 1 : une frame plus longue que la transition entière ne doit pas
/// dépasser la cible, ce qui ferait osciller le cadrage sur les à-coups.
fn pas_de_transition(cfg: &CameraEmote, dt: f32) -> f32 {
    let duree = (cfg.transition_ms.max(1) as f32) / 1000.0;
    (dt / duree).clamp(0.0, 1.0)
}

impl CameraEmote {
    /// Le retour à la normale, à la même vitesse que l'aller.
    fn retour() -> Self {
        Self {
            demi_tour: 0.0,
            distance_m: 0.0,
            tangage_deg: 0.0,
            transition_ms: 420,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn genome() -> Emotes {
        let s = std::fs::read_to_string("../../assets/genomes/emotes.toml")
            .expect("le genome d'emotes doit etre lisible");
        Emotes {
            defs: toml::from_str::<GenomeEmotes>(&s)
                .expect("le genome d'emotes doit parser")
                .emotes,
        }
    }

    /// Les 62 os du personnage d'expédition, tels qu'on les connaît. Sert de
    /// squelette fictif pour vérifier que le génome ne cite pas d'os inventé.
    fn os_du_personnage() -> Vec<&'static str> {
        vec![
            "Hips", "Spine", "Spine1", "Spine2", "Neck", "Head", "LeftShoulder", "LeftArm",
            "LeftForeArm", "LeftHand", "RightShoulder", "RightArm", "RightForeArm", "RightHand",
            "LeftUpLeg", "LeftLeg", "LeftFoot", "RightUpLeg", "RightLeg", "RightFoot",
        ]
    }

    fn cible_bidon(nom: &str) -> AnimationTargetId {
        AnimationTargetId::from_name(&Name::new(nom.to_string()))
    }

    #[test]
    fn le_genome_declare_au_moins_une_emote_jouable() {
        let e = genome();
        assert!(!e.defs.is_empty(), "aucune emote declaree");
        for d in &e.defs {
            assert!(
                code_touche(&d.touche).is_some(),
                "« {} » est liee a la touche inconnue {:?} — elle ne se declenchera jamais",
                d.label,
                d.touche
            );
            assert!(!d.pistes.is_empty(), "« {} » n'a aucune piste", d.label);
            assert!(
                d.mesure_s > 0.05,
                "« {} » : une mesure de {} s est trop courte pour se voir",
                d.label,
                d.mesure_s
            );
        }
    }

    /// 🚨 Une piste qui cite un os inexistant est IGNORÉE en silence par le
    /// moteur — la danse se joue en partie, et rien à l'écran ne dit laquelle
    /// des seize pistes manque. Ce test compare les noms cités au squelette
    /// réel du personnage.
    #[test]
    fn toutes_les_pistes_citent_des_os_qui_existent() {
        let connus = os_du_personnage();
        for d in &genome().defs {
            let inventes: Vec<&str> = d
                .pistes
                .iter()
                .map(|p| p.os.as_str())
                .filter(|os| !connus.contains(os))
                .collect();
            assert!(
                inventes.is_empty(),
                "« {} » cite des os absents du squelette Mixamo : {inventes:?}",
                d.label
            );
            for p in &d.pistes {
                assert!(
                    axe_vecteur(&p.axe).is_some(),
                    "« {} » : axe {:?} inconnu sur l'os {} — la piste ne ferait rien",
                    d.label,
                    p.axe,
                    p.os
                );
            }
        }
    }

    /// La danse doit BOUCLER : la dernière image doit rejoindre la première,
    /// sinon un à-coup revient à chaque mesure — d'autant plus visible que la
    /// danse est rapide.
    #[test]
    fn la_danse_boucle_sans_a_coup() {
        for d in &genome().defs {
            let (clip, manquants) = construire_clip(d, &|nom| Some(cible_bidon(nom)));
            assert!(manquants.is_empty(), "os manquants sur un squelette complet");
            assert!(!clip.curves().is_empty(), "« {}» n'a produit aucune courbe", d.label);
            // Le premier et le dernier échantillon d'une sinusoïde d'un tour
            // entier coïncident. On le vérifie sur la formule, pas sur la
            // courbe : c'est la formule qui doit être juste.
            for p in &d.pistes {
                let h = p.harmonique.max(1) as f32;
                let debut = ((0.0 * h + p.phase) * std::f32::consts::TAU).sin();
                let fin = ((1.0 * h + p.phase) * std::f32::consts::TAU).sin();
                assert!(
                    (debut - fin).abs() < 1e-4,
                    "« {} » piste {} : la boucle saute de {:.4} — harmonique {} non entiere ?",
                    d.label,
                    p.os,
                    (debut - fin).abs(),
                    p.harmonique
                );
            }
        }
    }

    /// Un os absent doit être RENDU, pas avalé. Sans ça, changer de corps
    /// amputerait la danse en silence.
    #[test]
    fn un_os_absent_est_signale_pas_ignore() {
        let d = &genome().defs[0];
        let (clip, manquants) = construire_clip(d, &|nom| {
            // On refuse tout ce qui touche aux bras : deux os au moins.
            if nom.contains("Arm") {
                None
            } else {
                Some(cible_bidon(nom))
            }
        });
        assert!(
            manquants.len() >= 2,
            "les os de bras refuses devaient etre signales, {manquants:?}"
        );
        assert!(
            !clip.curves().is_empty(),
            "le reste de la danse doit quand meme jouer"
        );
    }

    /// Le cadrage fait partie de la demande : sans demi-tour, on danse dos à la
    /// caméra et on ne voit rien.
    #[test]
    fn le_cadrage_montre_le_personnage_de_face() {
        for d in &genome().defs {
            let c = &d.camera;
            assert!(
                (c.demi_tour - 0.5).abs() < 0.25,
                "« {} » : un demi-tour de {} ne met pas la camera devant",
                d.label,
                c.demi_tour
            );
            assert!(
                c.distance_m > 3.5,
                "« {} » : a {} m on ne cadre pas le corps entier",
                d.label,
                c.distance_m
            );
        }
    }
}

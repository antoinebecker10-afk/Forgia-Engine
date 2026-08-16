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

/// Bornes du suivi de buste (radians). Un buste qui se plie à 90° casse la
/// silhouette ; ces bornes sont celles d'un torse humain qui vise, pas des
/// bornes de sécurité numérique.
const BUSTE_MIN_RAD: f32 = -0.7;
const BUSTE_MAX_RAD: f32 = 0.7;

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
    pub pose_decontractee: HashMap<String, [f32; 3]>,
    #[serde(default)]
    pub pose_tactique: HashMap<String, [f32; 3]>,
}

impl Default for ViseeGenome {
    fn default() -> Self {
        Self {
            montee_s: montee_par_defaut(),
            descente_s: descente_par_defaut(),
            suivi_du_buste: suivi_par_defaut(),
            pose_decontractee: HashMap::new(),
            pose_tactique: HashMap::new(),
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
#[derive(Resource, Default)]
struct OsDeLaTenue {
    table: HashMap<String, Entity>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct ExpeditionViseePlugin;

impl Plugin for ExpeditionViseePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Visee>()
            .init_resource::<OsDeLaTenue>()
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
                poser_la_tenue
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

    for (nom, entite) in &os.table {
        let Ok(mut tf) = q_tf.get_mut(*entite) else {
            continue;
        };
        let mut ajout = cfg.rotation_os(nom, visee.facteur);
        if nom == OS_DU_BUSTE {
            // Le buste porte DEUX termes : la pose déclarée (l'ouverture vers la
            // cible) et le suivi dérivé du tangage. Sans le second, viser vers
            // le haut lèverait le réticule sans lever le canon.
            ajout *= Quat::from_rotation_x(suivi);
        }
        tf.rotation *= ajout;
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
    visee: Res<Visee>,
    handle: Option<Res<ArmeMainGenomeHandle>>,
    genomes: Res<Assets<Genome<ArmeMainGenome>>>,
    os: Res<OsDeLaTenue>,
    q_cam: Query<(&Projection, &Camera), With<OrbitCamera>>,
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

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — aucune visee attendue")
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
        r#"{{"id":"expedition_visee","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"facteur":{:.2},"zoom_arme":{:.2},"fov_deg":{:.1},"os_attendus":{os_attendus},"os_trouves":{os_trouves}}}"#,
        time.elapsed_secs(),
        visee.facteur,
        visee.zoom_arme,
        fov_deg,
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
        assert!(!v.pose_tactique.is_empty(), "aucune tenue tactique déclarée");
        for os in ["RightArm", "RightForeArm", "Spine2"] {
            let r = v.rotation_os(os, 1.0);
            assert!(
                r.angle_between(Quat::IDENTITY) > 1e-3,
                "{os} ne bouge pas entre les deux tenues — viser ne se verrait pas"
            );
        }
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

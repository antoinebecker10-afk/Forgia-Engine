//! Les campements du Vallon : ils s'éveillent à l'approche, et ils barrent la route.
//!
//! # La donnée existait, personne ne la lisait
//!
//! `expedition_vallon.json` porte **trois campements entièrement spécifiés**
//! depuis le 2026-08-14 — centre, rayon, sept points d'apparition chacun, les
//! abris, les effectifs, et un `verrou_xyz` qui dit où la route se ferme. Aucune
//! ligne de Rust ne les touchait. C'est le motif exact des seize braseros, resté
//! trois jours dans le même fichier avant d'être branché : **une carte autorée
//! ne devient du jeu qu'au moment où quelqu'un lit ce qu'elle déclare.**
//!
//! # Trois décisions, et pourquoi elles sont là plutôt qu'ailleurs
//!
//! **L'éveil à l'approche, pas au chargement.** Peupler les trois camps à
//! l'entrée de la carte, c'est vingt-sept ennemis qui pensent en permanence le
//! long d'un chemin de 359 m — pour un joueur qui n'en voit jamais plus d'un
//! groupe. `scalability.md` l'écrit comme une obligation, pas comme une
//! optimisation.
//!
//! **Les archétypes passent par une correspondance.** Le manifeste nomme
//! « grunt » et « archer » ; le jeu ne connaît que tank/runner/sniper/boss. La
//! table de conversion vit dans `assets/genomes/expedition_campements.toml`,
//! **jamais dans ce fichier** : le manifeste est recuit à chaque export Blender,
//! et l'y corriger à la main serait perdu au passage suivant.
//!
//! **L'assemblage vient de `forgia-enemy-archetypes`.** Un ennemi de Forgia est
//! quatre entités et cinq pièges déjà payés (rotation de PI, pieds sous la
//! capsule, tête qui dépasse, corps en `Sensor`, décalage de pieds dérivé). Les
//! recopier ici aurait donné deux ennemis à corriger le jour où l'un casse.
//!
//! # Ce que ce module ne fait pas encore
//!
//! Le **verrou** est publié dans le capteur mais ne bloque physiquement rien :
//! `verrou_xyz` et `verrou_cap_rad` attendent leur barrière. Et les ennemis
//! marchent en ligne droite vers le joueur (`forgia-ai-arena-bot` : poursuite,
//! strafe, évitement — ni repli, ni couverture) : sur un vallon en relief, ça se
//! verra. Le navmesh est le chantier d'à côté, pas celui-ci.

use bevy::prelude::*;
use std::collections::HashMap;
use forgia_core::prelude::*;
use forgia_enemy_archetypes::assemblage::{assembler, Ennemi};
use forgia_enemy_archetypes::{EnemyArchetype, EnemyStatsConfig};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_player::Player;
use serde::Deserialize;

use crate::manifest::blender_to_bevy;
use crate::plugin::ActiveExpedition;

const GENOME: &str = "genomes/expedition_campements.toml";
const CAPTEUR: &str = "forgia2_expedition_campements.json";
/// Le capteur se publie à 1 Hz — assez pour lire un état, trop peu pour peser.
const PERIODE_CAPTEUR_S: f32 = 1.0;

// ─── Couche definition ───────────────────────────────────────────────────────

#[derive(TypePath, Deserialize, Debug, Clone, Default)]
pub struct CampementsGenome {
    #[serde(default)]
    pub correspondance: HashMap<String, String>,
    #[serde(default)]
    pub apparition: ApparitionGenome,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApparitionGenome {
    pub rayon_declenchement_m: f32,
    pub restants_pour_ouvrir: u32,
    pub repeupler_apres_nettoyage: bool,
}

impl Default for ApparitionGenome {
    fn default() -> Self {
        // Miroir exact du TOML : si le fichier disparaît ou ne parse pas, le jeu
        // garde le comportement livré au lieu de spawner à zéro mètre.
        Self {
            rayon_declenchement_m: 33.0,
            restants_pour_ouvrir: 0,
            repeupler_apres_nettoyage: false,
        }
    }
}

impl CampementsGenome {
    /// Le nom autoré → l'archétype qui existe vraiment.
    ///
    /// Rend `None` sur un nom inconnu **au lieu de deviner**. Un camp qui
    /// n'apparaît pas est un défaut visible ; un camp peuplé du mauvais ennemi
    /// est un défaut qu'on met une session à voir.
    #[must_use]
    pub fn archetype_pour(&self, nom_autore: &str) -> Option<EnemyArchetype> {
        let vivant = self.correspondance.get(nom_autore)?;
        match vivant.as_str() {
            "tank" => Some(EnemyArchetype::Tank),
            "runner" => Some(EnemyArchetype::Runner),
            "sniper" => Some(EnemyArchetype::Sniper),
            "boss" => Some(EnemyArchetype::Boss),
            _ => None,
        }
    }
}

#[derive(Resource)]
pub struct CampementsGenomeHandle(pub Handle<Genome<CampementsGenome>>);

#[derive(Resource, Default)]
pub struct ReglesCampements(pub CampementsGenome);

// ─── État de course ──────────────────────────────────────────────────────────

/// Marque un ennemi comme appartenant à un campement — c'est ce qui permet de
/// compter les vivants sans supposer qu'aucun autre ennemi n'existe ailleurs.
#[derive(Component, Debug, Clone)]
pub struct MembreDeCampement(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtatCamp {
    /// Le joueur n'est pas encore venu : personne n'est né.
    Dormant,
    /// Peuplé, combat en cours.
    Eveille,
    /// Nettoyé — le verrou s'ouvre, et on n'y revient pas.
    Nettoye,
}

#[derive(Resource, Default)]
pub struct EtatsCampements {
    pub par_id: HashMap<String, EtatCamp>,
    /// Cumulés de SESSION : ils survivent au démontage de la carte. Une lecture
    /// faite depuis le menu doit dire ce qui s'est passé pendant la partie —
    /// leçon du capteur navmesh (2026-08-12), reprise de `ActiveExpedition`.
    pub eveilles_session: u32,
    pub nettoyes_session: u32,
    pub ennemis_nes_session: u32,
    /// Noms autorés que la correspondance ne sait pas traduire. Publié : un camp
    /// qui reste vide doit DIRE pourquoi.
    pub archetypes_inconnus: Vec<String>,
}

// ─── Systèmes ────────────────────────────────────────────────────────────────

fn charger_genome(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(CampementsGenomeHandle(assets.load(GENOME)));
}

/// Recopie le génome chargé (ou rechargé à chaud) dans la ressource lue en jeu.
///
/// Hors garde de mode, délibérément : les messages d'asset sont abandonnés au
/// bout de deux frames, et un système gaté raterait tout rechargement survenu
/// pendant qu'on est au menu. Même motif que `arme_main`.
fn suivre_genome(
    handle: Option<Res<CampementsGenomeHandle>>,
    genomes: Res<Assets<Genome<CampementsGenome>>>,
    mut regles: ResMut<ReglesCampements>,
) {
    let Some(handle) = handle else { return };
    let Some(g) = Assets::get(&genomes, &handle.0) else {
        return;
    };
    if g.data.apparition.rayon_declenchement_m != regles.0.apparition.rayon_declenchement_m
        || g.data.correspondance.len() != regles.0.correspondance.len()
    {
        info!(
            "[campements] règles chargées — déclenchement {:.0} m, {} correspondance(s)",
            g.data.apparition.rayon_declenchement_m,
            g.data.correspondance.len()
        );
    }
    regles.0 = g.data.clone();
}

/// Le joueur approche : le camp se peuple.
#[allow(clippy::too_many_arguments)]
pub fn sys_eveiller_campements(
    mut commands: Commands,
    assets: Res<AssetServer>,
    carte: Option<Res<ActiveExpedition>>,
    stats: Option<Res<EnemyStatsConfig>>,
    regles: Res<ReglesCampements>,
    mut etats: ResMut<EtatsCampements>,
    q_joueur: Query<&Transform, With<Player>>,
) {
    let (Some(carte), Some(stats)) = (carte, stats) else {
        return;
    };
    let Ok(joueur) = q_joueur.single() else { return };
    let p = joueur.translation;
    let rayon = regles.0.apparition.rayon_declenchement_m;

    for camp in &carte.gameplay.campements {
        let etat = *etats.par_id.get(&camp.id).unwrap_or(&EtatCamp::Dormant);
        if etat != EtatCamp::Dormant {
            continue;
        }
        // Le manifeste est en `blender_z_up`. La conversion est une FONCTION
        // NOMMEE avec ses tests — jamais un `Vec3::new(v[0], v[2], -v[1])`
        // recopie, ce que l'en-tete de `manifest.rs` interdit explicitement.
        //
        // Je l'ai recopiee quand meme, et de travers : les 11 ennemis du run du
        // 2026-08-18 sont apparus EN L'AIR et DEVANT LA FORTERESSE. Le capteur
        // disait « 11 ennemis nes » et il avait raison — il ne mesurait pas leur
        // position. Un controle ne voit que ce qu'il regarde.
        let centre = blender_to_bevy(camp.centre_xyz);
        if p.distance(centre) > rayon {
            continue;
        }

        // Les effectifs sont déclarés par archétype ; les points d'apparition
        // sont une liste plate. On distribue en tournant sur les points plutôt
        // que d'en tirer au hasard : deux ennemis au même endroit se poussent
        // l'un l'autre à la naissance, et le résultat n'est pas reproductible
        // d'une session à l'autre.
        let Some(effectifs) = camp.effectifs.as_ref() else {
            warn!("[campements] {} sans effectifs déclarés — ignoré", camp.id);
            etats.par_id.insert(camp.id.clone(), EtatCamp::Nettoye);
            continue;
        };
        if camp.apparitions_xyz.is_empty() {
            warn!("[campements] {} sans point d'apparition — ignoré", camp.id);
            etats.par_id.insert(camp.id.clone(), EtatCamp::Nettoye);
            continue;
        }

        let mut i = 0usize;
        let mut nes = 0u32;
        for (nom_autore, nombre) in effectifs {
            let Some(archetype) = regles.0.archetype_pour(nom_autore) else {
                // On le PUBLIE au lieu de le taire : un camp à moitié peuplé
                // sans explication se lit comme un bug de spawn.
                if !etats.archetypes_inconnus.contains(nom_autore) {
                    etats.archetypes_inconnus.push(nom_autore.clone());
                }
                warn!(
                    "[campements] {} : « {nom_autore} » n'a pas de correspondance \
                     dans expedition_campements.toml — {nombre} ennemi(s) non né(s)",
                    camp.id
                );
                continue;
            };
            for _ in 0..*nombre {
                let pos = blender_to_bevy(camp.apparitions_xyz[i % camp.apparitions_xyz.len()]);
                i += 1;
                let e = assembler(
                    &mut commands,
                    &assets,
                    &stats,
                    Ennemi {
                        archetype,
                        sol_xz: pos.xz(),
                        sol_y: pos.y,
                        nom: &format!("Expedition_{}_{}_{i}", camp.id, archetype.label()),
                        // Pas de couche défensive en Expédition : le bouclier et
                        // l'armure sont une mécanique de l'Abîme (méta-progression).
                        // Les y ajouter ici serait un choix de design, pas un branchement.
                        defense: None,
                    },
                );
                commands.entity(e).insert((
                    MembreDeCampement(camp.id.clone()),
                    crate::plugin::ExpeditionMarker,
                    bevy::state::state_scoped::DespawnOnExit(GameMode::Expedition),
                ));
                nes += 1;
            }
        }

        etats.par_id.insert(camp.id.clone(), EtatCamp::Eveille);
        etats.eveilles_session = etats.eveilles_session.saturating_add(1);
        etats.ennemis_nes_session = etats.ennemis_nes_session.saturating_add(nes);
        info!(
            "[campements] {} éveillé à {:.0} m — {nes} ennemi(s)",
            camp.id,
            p.distance(centre)
        );
    }
}

/// Plus personne debout : le camp est nettoyé.
pub fn sys_nettoyer_campements(
    regles: Res<ReglesCampements>,
    mut etats: ResMut<EtatsCampements>,
    q_membres: Query<&MembreDeCampement>,
) {
    let seuil = regles.0.apparition.restants_pour_ouvrir;
    let mut vivants: HashMap<&str, u32> = HashMap::default();
    for m in &q_membres {
        *vivants.entry(m.0.as_str()).or_insert(0) += 1;
    }
    let a_nettoyer: Vec<String> = etats
        .par_id
        .iter()
        .filter(|(id, e)| {
            **e == EtatCamp::Eveille && *vivants.get(id.as_str()).unwrap_or(&0) <= seuil
        })
        .map(|(id, _)| id.clone())
        .collect();
    for id in a_nettoyer {
        etats.par_id.insert(id.clone(), EtatCamp::Nettoye);
        etats.nettoyes_session = etats.nettoyes_session.saturating_add(1);
        info!("[campements] {id} nettoyé — la route s'ouvre");
    }
}

/// Remet les camps au repos en quittant la carte.
pub fn sys_reinitialiser_campements(mut etats: ResMut<EtatsCampements>) {
    etats.par_id.clear();
    // Les cumulés de session NE sont PAS remis à zéro : c'est ce qui permet de
    // lire, depuis le menu, ce qui s'est passé pendant la partie.
}

/// `forgia2_expedition_campements.json` — l'état des camps, et POURQUOI il est
/// ce qu'il est.
///
/// Il tourne **hors** de la garde de mode : un capteur qui se tait quand la
/// feature dort se lit comme « rien à signaler », et c'est le défaut que ce
/// dépôt traque. Depuis le menu il dit « carte non chargée », ce qui est une
/// information, pas un silence.
pub fn sys_capteur_campements(
    time: Res<Time>,
    mut accum: Local<f32>,
    carte: Option<Res<ActiveExpedition>>,
    regles: Res<ReglesCampements>,
    etats: Res<EtatsCampements>,
    q_membres: Query<&MembreDeCampement>,
) {
    *accum += time.delta_secs();
    if *accum < PERIODE_CAPTEUR_S {
        return;
    }
    *accum = 0.0;

    let Some(carte) = carte else {
        let json = format!(
            r#"{{"id":"expedition_campements","severity":"info","next_step":"","carte_chargee":false,"campements":0,"eveilles_session":{},"nettoyes_session":{},"ennemis_nes_session":{}}}"#,
            etats.eveilles_session, etats.nettoyes_session, etats.ennemis_nes_session
        );
        let _ = forgia_core::sensor_io::enqueue(CAPTEUR, json);
        return;
    };

    let total = carte.gameplay.campements.len();
    let vivants = q_membres.iter().count();
    let dormants = carte
        .gameplay
        .campements
        .iter()
        .filter(|c| {
            matches!(
                etats.par_id.get(&c.id).unwrap_or(&EtatCamp::Dormant),
                EtatCamp::Dormant
            )
        })
        .count();

    // La sévérité nomme la cause, et donne le geste. Trois cas se distinguent —
    // et le troisième est le seul qui ait coûté du temps par le passé : un camp
    // éveillé, vide, et qui ne s'ouvre pas parce qu'un nom autoré n'a pas de
    // correspondance.
    let (severity, next_step) = if !etats.archetypes_inconnus.is_empty() {
        (
            "warn",
            format!(
                "archetype(s) autore(s) sans correspondance : {} — ajouter la ligne dans assets/genomes/expedition_campements.toml [correspondance]",
                etats.archetypes_inconnus.join(", ")
            ),
        )
    } else if total == 0 {
        (
            "warn",
            "CAMPEMENTS ZERO : la carte est chargee mais ne declare aucun campement — verifier expedition_vallon.json"
                .to_owned(),
        )
    } else if regles.0.correspondance.is_empty() {
        (
            "warn",
            "aucune correspondance chargee : expedition_campements.toml absent ou illisible — aucun ennemi ne peut naitre"
                .to_owned(),
        )
    } else {
        ("ok", String::new())
    };

    let json = format!(
        r#"{{"id":"expedition_campements","severity":"{severity}","next_step":"{}","carte_chargee":true,"campements":{total},"dormants":{dormants},"ennemis_vivants":{vivants},"rayon_declenchement_m":{:.1},"correspondances":{},"eveilles_session":{},"nettoyes_session":{},"ennemis_nes_session":{}}}"#,
        next_step.replace('"', "'"),
        regles.0.apparition.rayon_declenchement_m,
        regles.0.correspondance.len(),
        etats.eveilles_session,
        etats.nettoyes_session,
        etats.ennemis_nes_session,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(CAPTEUR, json) {
        warn!("[campements] capteur non ecrit : {e}");
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct ExpeditionCampementsPlugin;

impl Plugin for ExpeditionCampementsPlugin {
    fn build(&self, app: &mut App) {
        // Les archétypes et leur assemblage viennent de la brique partagée —
        // l'Abîme la monte aussi, et le garde la rend idempotente.
        if !app.is_plugin_added::<forgia_enemy_archetypes::ForgiaEnemyArchetypesPlugin>() {
            app.add_plugins(forgia_enemy_archetypes::ForgiaEnemyArchetypesPlugin);
        }
        app.init_asset::<Genome<CampementsGenome>>()
            .register_asset_loader(GenomeLoader::<CampementsGenome>::default())
            .init_resource::<ReglesCampements>()
            .init_resource::<EtatsCampements>()
            .add_systems(Startup, charger_genome)
            .add_systems(Update, suivre_genome)
            .add_systems(
                Update,
                (sys_eveiller_campements, sys_nettoyer_campements)
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Expedition)),
            )
            .add_systems(
                OnExit(GameMode::Expedition),
                sys_reinitialiser_campements,
            )
            .add_systems(
                Update,
                sys_capteur_campements.in_set(GameSet::Sensors),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regles() -> CampementsGenome {
        let src = std::fs::read_to_string("../../assets/genomes/expedition_campements.toml")
            .expect("le genome des campements doit exister");
        toml::from_str(&src).expect("il doit parser")
    }

    /// Le manifeste du Vallon nomme « grunt » et « archer ». Si la table ne les
    /// traduit pas, les trois camps restent vides — sans erreur, sans crash, et
    /// c'est précisément le genre de silence qui coûte une session.
    #[test]
    fn les_noms_du_manifeste_ont_tous_une_correspondance() {
        let r = regles();
        let m = crate::manifest::ExpeditionManifest::parse(include_str!(
            "../../../assets/models/environment/expedition/expedition_vallon.json"
        ))
        .expect("le manifeste du Vallon doit parser");
        assert!(!m.campements.is_empty(), "le Vallon declare des campements");
        for c in &m.campements {
            for nom in c.effectifs.iter().flat_map(|e| e.keys()) {
                assert!(
                    r.archetype_pour(nom).is_some(),
                    "campement {} : « {nom} » n'a pas de correspondance — ces ennemis ne naitraient jamais",
                    c.id
                );
            }
        }
    }

    /// Les points d'apparition doivent tomber DANS la carte, au niveau du sol.
    ///
    /// Ce test manquait, et son absence a coûté un run : la première version
    /// convertissait les coordonnées à la main et plaçait les ennemis en l'air,
    /// à des dizaines de mètres du campement. Tous les autres tests passaient —
    /// ils vérifiaient la table de correspondance, jamais la géométrie.
    #[test]
    fn les_apparitions_tombent_au_sol_et_pres_de_leur_camp() {
        let m = crate::manifest::ExpeditionManifest::parse(include_str!(
            "../../../assets/models/environment/expedition/expedition_vallon.json"
        ))
        .expect("le manifeste du Vallon doit parser");
        for c in &m.campements {
            let centre = crate::manifest::blender_to_bevy(c.centre_xyz);
            for p in &c.apparitions_xyz {
                let v = crate::manifest::blender_to_bevy(*p);
                // Le Vallon fait 280 x 200 m et son relief tient dans quelques
                // metres : une hauteur a deux chiffres signifie un axe echange.
                assert!(
                    v.y.abs() < 10.0,
                    "{} : apparition a {:.1} m de haut — axe vertical faux",
                    c.id,
                    v.y
                );
                // Une apparition appartient a SON campement : au-dela du rayon
                // declare (+ une marge), elle est ailleurs sur la carte.
                let d = (v.xz() - centre.xz()).length();
                assert!(
                    d <= c.rayon_m + 5.0,
                    "{} : apparition a {d:.1} m du centre (rayon declare {:.1} m)",
                    c.id,
                    c.rayon_m
                );
            }
        }
    }

    /// Un nom absent rend `None` au lieu de tomber sur un archétype par défaut.
    #[test]
    fn un_nom_inconnu_ne_devine_pas() {
        assert!(regles().archetype_pour("chevalier_de_la_lune").is_none());
    }

    /// Le déclenchement doit couvrir la plus longue portée de tir du roster,
    /// sinon le joueur prend un tir venu d'un ennemi pas encore né.
    #[test]
    fn le_rayon_de_declenchement_couvre_la_portee_du_sniper() {
        let cfg = EnemyStatsConfig::default();
        let portee_max = cfg
            .for_archetype(EnemyArchetype::Sniper)
            .shoot_range
            .max(cfg.for_archetype(EnemyArchetype::Boss).shoot_range);
        assert!(
            regles().apparition.rayon_declenchement_m >= portee_max,
            "declenchement {:.1} m < portee {portee_max:.1} m : on se ferait tirer par un ennemi inexistant",
            regles().apparition.rayon_declenchement_m
        );
    }
}

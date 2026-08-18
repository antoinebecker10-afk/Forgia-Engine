//! anim_sensor.rs — Savoir CE QUI JOUE, et pourquoi ça ne se voit pas.
//!
//! # Le trou que ce fichier comble
//!
//! Audit du 2026-08-16 : le projet avait six capteurs qui touchaient à
//! l'animation, et **aucun ne disait quel clip jouait**. Ni son nom, ni son
//! temps, ni son poids. On savait *qu'il jouait* (`anim_playing: 1`), jamais
//! *quoi*. Quatre diagnostics de la même journée y sont revenus.
//!
//! # Les cinq causes de « ça ne bouge pas », et comment les séparer
//!
//! Un capteur qui ne les distingue pas ne vaut rien — c'est la règle « zéro
//! mesuré n'est pas vert, c'est aveugle » appliquée à l'animation :
//!
//! | cause | signature mesurable |
//! |---|---|
//! | **a** — pas de lecteur, ou sur la mauvaise entité | aucune entité avec `AnimationPlayer` sous le corps |
//! | **b** — lecteur présent, rien d'actif | `actifs == 0` |
//! | **c** — actif, mais **aucun os touché** | `poids_total > 0 && cibles_touchees == 0` |
//! | **d** — os touchés, **poids nul** | `cibles_touchees > 0 && poids_total ≈ 0` |
//! | **e** — tout bon, mais le temps ne coule pas | `seek_time` figé entre deux relevés |
//!
//! 🚨 **`cibles_touchees` est le champ le plus important du fichier.**
//! `cibles_declarees = 62` avec `cibles_touchees = 0` EST le diagnostic complet
//! d'un clip venu d'un autre squelette : Bevy relie une courbe à un os par un
//! `AnimationTargetId` = blake3 du **chemin de noms d'os**. Un os renommé, un
//! ré-export, un corps échangé — et la piste est ignorée **sans un mot**
//! (bevy#15612). Sans ce champ, on cherche une session ; avec, une lecture.
//!
//! # Ce que ce module ne fait pas
//!
//! Il ne juge pas la beauté d'une pose et ne remplace pas l'œil. Il répond à
//! « qu'est-ce qui tourne, sur quoi, et avec quel poids ». Le reste — squelette
//! dessiné, détection automatique de pose de repos, anneau de rejeu — viendra
//! par-dessus, et lira les mêmes données.

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeType};
use bevy::animation::transition::AnimationTransitions;
use bevy::animation::AnimationTargetId;
use bevy::gltf::Gltf;
use forgia_core::constat;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;

/// Fréquence d'écriture. 2 Hz : assez pour suivre un changement d'état, assez
/// peu pour ne pas peser. Les transitoires plus courts relèvent de l'anneau de
/// rejeu, pas d'un échantillonnage plus rapide — augmenter la fréquence pour
/// attraper un défaut d'une frame serait courir après un lièvre.
const PERIODE_S: f32 = 0.5;

/// Plafond de lecteurs détaillés dans le fichier.
///
/// 🚨 C'est un PLAFOND, et le capteur publie toujours le total à côté. Le
/// traceur d'os précédent en avait un (`max_characters: 8`) sans jamais dire
/// qu'il tronquait : on lisait « 8 personnages » en croyant les avoir tous,
/// alors que la scène en portait 16.
const MAX_LECTEURS: usize = 12;

/// Plafond de cibles inertes NOMMÉES par lecteur. Le total est publié à côté :
/// un plafond qui ne dit pas qu'il tronque fait croire qu'on a tout vu.
const MAX_INERTES: usize = 16;

/// L'état d'un lecteur, pour une frame.
struct FicheLecteur {
    nom: String,
    actifs: usize,
    poids_total: f32,
    cibles_declarees: usize,
    cibles_touchees: usize,
    a_graphe: bool,
    a_transitions: bool,
    principal: Option<String>,
    /// Les cibles qu'aucun clip actif ne pilote, nommées (plafonnées).
    inertes: Vec<String>,
    clips: Vec<FicheClip>,
}

struct FicheClip {
    nom: String,
    source: Option<String>,
    poids: f32,
    vitesse: f32,
    temps: f32,
    boucle: bool,
    completions: u32,
}

/// Table inverse `AssetId<AnimationClip>` → **nom déclaré dans le glTF**.
///
/// 🚨 C'est LE trajet qui manquait. `AssetServer::get_path()` rend
/// `…/corps.glb#Animation24` — un **label indexé**, pas le nom. Ce module
/// affichait donc exactement le `NodeIndex(3)` qu'il s'interdit deux lignes
/// plus bas, sous un autre habillage : le 2026-08-17, comprendre que
/// « `#Animation24` » voulait dire `rifle_idle` a exigé d'ouvrir le GLB et de
/// décoder son en-tête JSON à la main.
///
/// `Gltf::named_animations` porte la correspondance. On la retourne à la
/// lecture du capteur (2 Hz) : quelques dizaines de glTF chargés, jamais dans
/// une boucle de jeu.
fn noms_gltf(gltfs: &Assets<Gltf>) -> HashMap<AssetId<AnimationClip>, String> {
    let mut table = HashMap::default();
    for (_, g) in gltfs.iter() {
        for (nom, h) in &g.named_animations {
            table.insert(h.id(), nom.to_string());
        }
    }
    table
}

/// Ce qu'on affiche, selon ce dont on dispose. Fonction pure, donc testable —
/// c'est la seule façon que la règle « jamais un index quand un nom existe »
/// soit autre chose qu'un commentaire.
///
/// Ordre : nom déclaré > chemin de l'asset > nature du clip. Un clip FABRIQUÉ —
/// nos émotes — n'a ni nom glTF ni chemin : on le dit, au lieu d'inventer.
fn choisir_nom(declare: Option<&str>, source: Option<&str>, charge: bool, index: usize) -> String {
    match (declare, source) {
        (Some(n), _) => n.to_string(),
        (None, Some(p)) => format!("<sans nom declare> {p}"),
        (None, None) if charge => format!("<clip fabrique #{index}>"),
        (None, None) => format!("<clip NON CHARGE #{index}>"),
    }
}

/// Le nom lisible d'un clip, son fichier d'origine, et son identifiant d'asset.
///
/// 🚨 Un capteur qui affiche `NodeIndex(3)` rend le défaut « mauvais clip joué »
/// indébuggable — c'est un numéro qui ne dit rien et change au moindre
/// ré-export. On rend donc le nom **déclaré** (`rifle_idle`), et le fichier à
/// côté : deux corps peuvent porter un `idle` chacun, et savoir lequel joue est
/// la moitié du diagnostic.
fn nom_du_clip(
    graphe: &AnimationGraph,
    noeud: bevy::animation::graph::AnimationNodeIndex,
    assets: &AssetServer,
    clips: &Assets<AnimationClip>,
    noms: &HashMap<AssetId<AnimationClip>, String>,
) -> (String, Option<String>, Option<AssetId<AnimationClip>>) {
    let Some(n) = graphe.get(noeud) else {
        return (
            format!("noeud#{} ABSENT DU GRAPHE", noeud.index()),
            None,
            None,
        );
    };
    match &n.node_type {
        AnimationNodeType::Clip(h) => {
            let id = h.id();
            let source = assets.get_path(id).map(|p| p.to_string());
            let nom = choisir_nom(
                noms.get(&id).map(|s| s.as_str()),
                source.as_deref(),
                clips.get(id).is_some(),
                noeud.index(),
            );
            (nom, source, Some(id))
        }
        AnimationNodeType::Blend => (format!("<melange #{}>", noeud.index()), None, None),
        AnimationNodeType::Add => (format!("<somme #{}>", noeud.index()), None, None),
    }
}

/// Les os que ce lecteur peut réellement piloter : les descendants qui portent
/// un `AnimationTargetId`, posé par le chargeur glTF — **avec leur nom**.
///
/// 🚨 Le nom n'est pas un confort. Le 2026-08-17, ce capteur annonçait
/// « cibles 62/83 » : 21 cibles inertes, chiffre juste et inexploitable. Savoir
/// LESQUELLES a exigé d'ouvrir le GLB et de décoder son en-tête JSON — et la
/// réponse (`cloak_01…06`, `cheveux_01…02`, le reste étant des nœuds qui ne
/// sont pas des os) transformait un écart inquiétant en constat connu : aucun
/// clip Mixamo ne pilote la cape ni les cheveux.
///
/// Un écart chiffré sans les noms envoie chercher ; les noms concluent.
fn cibles_sous(
    racine: Entity,
    q_enfants: &Query<&Children>,
    q_cibles: &Query<&AnimationTargetId>,
    q_noms: &Query<&Name>,
) -> HashMap<AnimationTargetId, String> {
    let mut ids = HashMap::default();
    let ajoute = |e: Entity, ids: &mut HashMap<AnimationTargetId, String>| {
        if let Ok(id) = q_cibles.get(e) {
            let nom = q_noms
                .get(e)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|_| format!("{e}"));
            ids.insert(*id, nom);
        }
    };
    ajoute(racine, &mut ids);
    for e in q_enfants.iter_descendants(racine) {
        ajoute(e, &mut ids);
    }
    ids
}

/// Les cibles qu'aucun clip actif ne pilote, par nom, plafonnées.
///
/// Triées pour que deux lectures se comparent, et le total est publié à côté :
/// un plafond muet ferait croire qu'on les a toutes.
fn inertes(
    declarees: &HashMap<AnimationTargetId, String>,
    touchees: &HashSet<AnimationTargetId>,
) -> Vec<String> {
    let mut v: Vec<String> = declarees
        .iter()
        .filter(|(id, _)| !touchees.contains(*id))
        .map(|(_, nom)| nom.clone())
        .collect();
    v.sort();
    v.truncate(MAX_INERTES);
    v
}

#[allow(clippy::too_many_arguments)]
pub fn sys_write_anim_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    assets: Res<AssetServer>,
    clips: Res<Assets<AnimationClip>>,
    graphes: Res<Assets<AnimationGraph>>,
    gltfs: Res<Assets<Gltf>>,
    q_lecteurs: Query<(
        Entity,
        Option<&Name>,
        &AnimationPlayer,
        Option<&AnimationGraphHandle>,
        Option<&AnimationTransitions>,
    )>,
    q_enfants: Query<&Children>,
    q_cibles: Query<&AnimationTargetId>,
    q_noms: Query<&Name>,
) {
    *accum += time.delta_secs();
    if *accum < PERIODE_S {
        return;
    }
    *accum = 0.0;

    let total_lecteurs = q_lecteurs.iter().count();
    let noms = noms_gltf(&gltfs);
    let mut fiches = Vec::new();

    for (entite, nom, player, graphe_h, transitions) in q_lecteurs.iter().take(MAX_LECTEURS) {
        let declarees = cibles_sous(entite, &q_enfants, &q_cibles, &q_noms);
        let graphe = graphe_h.and_then(|h| graphes.get(&h.0));
        let mut clips_actifs = Vec::new();
        let mut poids_total = 0.0;
        // Les os qu'au moins un clip actif prétend piloter ET qui existent
        // vraiment sous ce lecteur. C'est l'intersection qui compte : un clip
        // peut citer 62 os dont aucun n'est ici.
        let mut touchees: HashSet<AnimationTargetId> = HashSet::default();

        for (noeud, actif) in player.playing_animations() {
            let (nom_clip, source, id) = match graphe {
                Some(g) => nom_du_clip(g, *noeud, &assets, &clips, &noms),
                None => (format!("<sans graphe #{}>", noeud.index()), None, None),
            };
            poids_total += actif.weight();
            if let Some(clip) = id.and_then(|i| clips.get(i)) {
                for cible in clip.curves().keys() {
                    if declarees.contains_key(cible) {
                        touchees.insert(*cible);
                    }
                }
            }
            clips_actifs.push(FicheClip {
                nom: nom_clip,
                source,
                poids: actif.weight(),
                vitesse: actif.speed(),
                temps: actif.seek_time(),
                boucle: !matches!(actif.repeat_mode(), bevy::animation::RepeatAnimation::Never),
                completions: actif.completions(),
            });
        }

        let principal = transitions
            .and_then(|t| t.get_main_animation())
            .map(|n| match graphe {
                Some(g) => nom_du_clip(g, n, &assets, &clips, &noms).0,
                None => format!("#{}", n.index()),
            });

        fiches.push(FicheLecteur {
            nom: nom
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("{entite}")),
            actifs: clips_actifs.len(),
            poids_total,
            cibles_declarees: declarees.len(),
            cibles_touchees: touchees.len(),
            a_graphe: graphe.is_some(),
            a_transitions: transitions.is_some(),
            principal,
            inertes: inertes(&declarees, &touchees),
            clips: clips_actifs,
        });
    }

    // L'echantillon EST le nombre de lecteurs : zero rend `info`, jamais `ok`.
    juger(&fiches, total_lecteurs)
        .echantillon(total_lecteurs)
        .publier(
            "animation",
            time.elapsed_secs(),
            &format!(
                r#""lecteurs_total":{},"lecteurs_detailles":{},"lecteurs":[{}]"#,
                total_lecteurs,
                fiches.len(),
                fiches.iter().map(fiche_json).collect::<Vec<_>>().join(",")
            ),
        );
}

fn echappe(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn fiche_json(f: &FicheLecteur) -> String {
    format!(
        r#"{{"nom":"{}","actifs":{},"poids_total":{:.3},"cibles_declarees":{},"cibles_touchees":{},"a_graphe":{},"a_transitions":{},"principal":{},"cibles_inertes":[{}],"clips":[{}]}}"#,
        echappe(&f.nom),
        f.actifs,
        f.poids_total,
        f.cibles_declarees,
        f.cibles_touchees,
        f.a_graphe,
        f.a_transitions,
        f.principal
            .as_ref()
            .map(|p| format!("\"{}\"", echappe(p)))
            .unwrap_or_else(|| "null".into()),
        f.inertes
            .iter()
            .map(|n| format!("\"{}\"", echappe(n)))
            .collect::<Vec<_>>()
            .join(","),
        f.clips
            .iter()
            .map(|c| format!(
                r#"{{"clip":"{}","source":{},"poids":{:.3},"vitesse":{:.2},"temps_s":{:.2},"boucle":{},"tours":{}}}"#,
                echappe(&c.nom),
                c.source
                    .as_ref()
                    .map(|s| format!("\"{}\"", echappe(s)))
                    .unwrap_or_else(|| "null".into()),
                c.poids,
                c.vitesse,
                c.temps,
                c.boucle,
                c.completions
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// Le verdict, dans l'ordre des cinq causes.
///
/// 🚨 Aucun lecteur du tout rend `info`, PAS `ok` : il n'y a rien à mesurer —
/// on est au menu, ou aucun personnage animé n'est monté. Le déclarer `ok`
/// ferait passer l'absence pour une bonne santé, et c'est précisément ce que
/// `forgia_anim_layer.json` fait aujourd'hui en rapportant zéro partout.
fn juger(fiches: &[FicheLecteur], total: usize) -> constat::Constat<constat::Pret> {
    if total == 0 {
        return constat::info(
            "aucun lecteur d'animation dans la scene — rien a mesurer (menu, ou aucun \
             personnage anime monte). Ce n'est pas un feu vert.",
        );
    }
    // (c) — la cause la plus coûteuse à diagnostiquer sans ce capteur.
    if let Some(f) = fiches
        .iter()
        .find(|f| f.poids_total > 0.01 && f.cibles_declarees > 0 && f.cibles_touchees == 0)
    {
        return constat::critique(format!(
                "ANIM_HORS_CIBLE : « {} » joue {} clip(s) a poids {:.2} mais touche 0 os \
                 sur {} declares. Le clip vient d'un AUTRE squelette : Bevy relie une \
                 courbe a un os par le HACHAGE de son chemin de noms, donc un os renomme \
                 ou un corps echange rend la piste muette SANS erreur. Comparer les noms \
                 d'os du clip et du corps.",
                echappe(&f.nom),
                f.actifs,
                f.poids_total,
                f.cibles_declarees
            ))
            .remede(
                "comparer les noms d'os du clip et du corps — un os renomme ou un corps                  echange rend la piste muette SANS erreur",
            );
    }
    // (b) — un lecteur qui ne joue rien. C'est la T-pose.
    if let Some(f) = fiches.iter().find(|f| f.actifs == 0 && f.cibles_declarees > 0) {
        return constat::critique(format!(
                "ANIM_MUET : « {} » pilote {} os et ne joue AUCUN clip — le corps reste \
                 sur sa pose de repos, bras ecartes. Verifier qui a appele stop() : une \
                 transition dont le poids atteint zero fait `player.stop()` et rien ne \
                 relance.",
                echappe(&f.nom),
                f.cibles_declarees
            ))
            .remede(
                "chercher qui a appele stop() : une transition dont le poids atteint zero                  fait `player.stop()` et rien ne relance",
            );
    }
    // (d) — ça joue, ça cible, mais à poids nul.
    if let Some(f) = fiches
        .iter()
        .find(|f| f.actifs > 0 && f.cibles_touchees > 0 && f.poids_total < 0.01)
    {
        return constat::alerte(format!(
                "ANIM_POIDS_NUL : « {} » joue et cible correctement, mais la somme des \
                 poids vaut {:.3}. Le clip tourne sans rien deplacer — regarder les \
                 transitions en cours.",
                echappe(&f.nom),
                f.poids_total
            ))
            .remede("regarder les transitions en cours — le clip tourne sans rien deplacer");
    }
    // Un lecteur sans graphe ne peut RIEN jouer en Bevy 0.18.
    if let Some(f) = fiches.iter().find(|f| !f.a_graphe) {
        return constat::alerte(format!(
                "ANIM_SANS_GRAPHE : « {} » a un lecteur mais aucun AnimationGraphHandle — \
                 en Bevy 0.18 un clip ne se joue QUE via un graphe.",
                echappe(&f.nom)
            ))
            .remede("poser un AnimationGraphHandle : en Bevy 0.18 un clip ne se joue QUE via un graphe");
    }
    constat::ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 🚨 Le défaut réel du 2026-08-17 : le capteur publiait
    /// `stylized_male_fusil.glb#Animation24`. Un **index**, exactement ce que
    /// l'en-tête du module s'interdit — il a fallu ouvrir le GLB et décoder son
    /// JSON à la main pour apprendre que 24 valait `rifle_idle`.
    #[test]
    fn un_nom_declare_bat_toujours_le_chemin_indexe() {
        let n = choisir_nom(
            Some("rifle_idle"),
            Some("models/characters/stylized/stylized_male_fusil.glb#Animation24"),
            true,
            24,
        );
        assert_eq!(n, "rifle_idle");
        assert!(
            !n.contains("Animation24") && !n.contains("#24"),
            "aucun index ne doit survivre quand un nom est declare : {n}"
        );
    }

    /// Sans nom déclaré, on rend le chemin — pas un numéro nu. Et on DIT qu'il
    /// manque un nom, au lieu de laisser croire que le chemin en est un.
    #[test]
    fn sans_nom_declare_on_rend_le_chemin_et_on_le_signale() {
        let n = choisir_nom(None, Some("corps.glb#Animation3"), true, 3);
        assert!(n.contains("corps.glb#Animation3"));
        assert!(n.contains("sans nom declare"), "l'absence doit se voir : {n}");
    }

    /// Un clip FABRIQUÉ — nos émotes — n'a ni nom glTF ni chemin. Le capteur
    /// doit le dire, pas inventer un nom ni prétendre qu'il manque.
    #[test]
    fn un_clip_fabrique_se_declare_comme_tel() {
        assert!(choisir_nom(None, None, true, 7).contains("fabrique"));
        assert!(choisir_nom(None, None, false, 7).contains("NON CHARGE"));
    }

    fn fiche(nom: &str, actifs: usize, poids: f32, dec: usize, touch: usize) -> FicheLecteur {
        FicheLecteur {
            nom: nom.into(),
            actifs,
            poids_total: poids,
            cibles_declarees: dec,
            cibles_touchees: touch,
            a_graphe: true,
            a_transitions: true,
            principal: None,
            inertes: Vec::new(),
            clips: Vec::new(),
        }
    }

    /// Le cas mesuré le 2026-08-17 : 62 cibles touchées sur 70 os, les 8
    /// absentes étant la cape et les cheveux. Le capteur doit les NOMMER —
    /// c'est ce qui sépare « un écart inquiétant » de « constat connu ».
    #[test]
    fn les_cibles_inertes_sont_nommees_et_triees() {
        let mut declarees = HashMap::default();
        let ids: Vec<AnimationTargetId> = ["mixamorig:Hips", "cloak_03", "cloak_01", "cheveux_01"]
            .iter()
            .map(|n| {
                let id = AnimationTargetId::from_name(&Name::new(*n));
                declarees.insert(id, (*n).to_string());
                id
            })
            .collect();
        let touchees: HashSet<AnimationTargetId> = [ids[0]].into_iter().collect();

        assert_eq!(
            inertes(&declarees, &touchees),
            vec!["cheveux_01", "cloak_01", "cloak_03"],
            "les inertes se rendent nommees et triees, pas comptees"
        );
    }

    /// 🚨 Le test qui garde la raison d'être du capteur : aucun lecteur ne doit
    /// JAMAIS rendre `ok`. C'est le défaut exact de `forgia_anim_layer.json`,
    /// qui rapporte 0 chaîne, 0 IK, 0 µs et se déclare au vert — donc une
    /// absence totale de mesure lue comme une bonne santé.
    #[test]
    fn zero_lecteur_n_est_pas_un_feu_vert() {
        let (s, msg) = juger(&[], 0).verdict();
        assert_eq!(s, constat::Severite::Info, "zero mesure doit etre info, jamais ok");
        assert!(
            msg.contains("pas un feu vert"),
            "le message doit dire qu'il n'y a rien a mesurer : {msg}"
        );
    }

    /// La cause qui a coûté trois tours le 2026-08-16 : des clips actifs, un
    /// poids réel, et zéro os touché.
    #[test]
    fn un_clip_qui_ne_touche_aucun_os_est_critique() {
        let (s, msg) = juger(&[fiche("corps", 1, 1.0, 62, 0)], 1).verdict();
        assert_eq!(s, constat::Severite::Critical);
        assert!(msg.contains("ANIM_HORS_CIBLE"), "{msg}");
        assert!(msg.contains("62"), "le message doit citer les os declares : {msg}");
    }

    /// La T-pose : un lecteur qui pilote des os et ne joue rien.
    #[test]
    fn un_lecteur_muet_est_critique() {
        let (s, msg) = juger(&[fiche("corps", 0, 0.0, 62, 0)], 1).verdict();
        assert_eq!(s, constat::Severite::Critical);
        assert!(msg.contains("ANIM_MUET"), "{msg}");
    }

    /// Les causes sont ORDONNÉES : hors-cible passe avant muet, parce qu'elle
    /// envoie chercher ailleurs. Les confondre ferait perdre la session qu'on
    /// vient de perdre.
    #[test]
    fn les_causes_ne_se_confondent_pas() {
        let hors_cible = juger(&[fiche("a", 1, 1.0, 62, 0)], 1).verdict();
        let muet = juger(&[fiche("b", 0, 0.0, 62, 0)], 1).verdict();
        let poids_nul = juger(&[fiche("c", 1, 0.0, 62, 30)], 1).verdict();
        assert!(hors_cible.1.contains("HORS_CIBLE"));
        assert!(muet.1.contains("MUET"));
        assert!(poids_nul.1.contains("POIDS_NUL"));
        assert_ne!(hors_cible.1, muet.1);
        assert_ne!(muet.1, poids_nul.1);
    }

    /// Un cas sain doit passer au vert — sinon le capteur crierait en
    /// permanence et on cesserait de le lire.
    #[test]
    fn un_avatar_sain_passe_au_vert() {
        let (s, _) = juger(&[fiche("corps", 1, 1.0, 62, 58)], 1).verdict();
        assert_eq!(s, constat::Severite::Ok);
    }
}

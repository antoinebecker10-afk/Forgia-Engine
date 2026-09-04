//! capteur.rs — `forgia2_expedition_carte.json`, 1 Hz.
//!
//! # Pourquoi ce fichier existe
//!
//! Audit du 2026-08-17 : **131 capteurs lus, aucun sur la carte**. Les deux
//! seuls fichiers d'Expédition concernaient l'arme tenue en main. Rien ne
//! publiait les cellules résidentes, les solides posés, l'état des salles de
//! combat, ni ce que la cuisson avait raté.
//!
//! `observability-required.md` est explicite : « quand l'user dit *regarde*,
//! l'IA doit pouvoir diagnostiquer la feature en lisant sa sortie. Si l'IA ne
//! voit rien, la feature est incomplète. »
//!
//! # Ce qu'il mesure que rien d'autre ne dit
//!
//! **La part de carte résidente.** C'est LE nombre qui décide de la taille des
//! cellules, et il ne se déduit d'aucun réglage : simulé hors ligne sur le
//! tracé, le rayon de 140 m garde ~78 % des 48 cellules en mémoire — le
//! streaming n'économise qu'un cinquième. Le test qui valide ce rayon le compare
//! à la demi-diagonale (172 m), mauvais juge sur une carte de 200 m de large.
//!
//! Tant que ce capteur n'existait pas, affiner les cellules se serait fait à
//! l'aveugle. **On mesure d'abord, on règle ensuite** — jamais l'inverse.

use bevy::prelude::*;
use forgia_core::prelude::GameMode;
use forgia_player::Player;

use crate::plugin::{ActiveExpedition, ExpeditionCell};

/// Demi-diagonale de la carte (m), au-delà de laquelle plus rien ne peut être
/// déchargé : `sqrt(280² + 200²) / 2`.
///
/// Sert à distinguer les deux lectures possibles d'une résidence élevée.
const DEMI_DIAGONALE_M: f32 = 172.0;

/// Verdict de santé de la carte : sévérité + action de remédiation.
///
/// Fonction **pure** — c'est ce qui la rend testable sans moteur, et c'est là
/// que vit toute la logique qui peut être fausse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub severity: &'static str,
    pub next_step: &'static str,
}

/// Ce qu'on sait de la carte à cet instant, sous une forme sans Bevy.
#[derive(Debug, Clone, Copy, Default)]
pub struct EtatCarte {
    pub cellules_total: usize,
    pub cellules_chargees: usize,
    pub colliders: u32,
    pub camps_en_tir_gratuit: usize,
    pub abris_menteurs: usize,
    pub defauts_de_cuisson: usize,
    pub joueur_place: bool,
    /// Rayon de chargement effectif (m) — pour juger le pop-in, pas la résidence.
    pub rayon_chargement_m: f32,
    /// Combien de matériaux ont reçu l'extension de vent.
    ///
    /// # Pourquoi ce compte existe
    ///
    /// Le vent tient sur **deux listes de matières qui doivent coïncider** :
    /// `14_ancrage.py` décide quels SOMMETS sont souples (il cuit leur raideur
    /// dans l'alpha de `COLOR_0`), et `vent.rs` décide quels MATÉRIAUX portent
    /// le shader. Si les deux divergent, l'effet est silencieux **dans les deux
    /// sens** : un matériau animé dont tous les sommets sont rigides ne bouge
    /// pas, et un masque cuit sur un matériau non animé est ignoré.
    ///
    /// Aucune erreur n'est levée dans un cas comme dans l'autre. On publie donc
    /// le compte — c'est la seule façon de voir la divergence sans profiler des
    /// shaders. `vent.rs` annonçait ce champ dans un commentaire alors qu'il
    /// n'existait pas : un commentaire qui décrit une donnée absente est
    /// exactement ce qui fait chercher le défaut ailleurs.
    pub materiaux_vent: usize,
}

impl EtatCarte {
    /// Part de la carte gardée en mémoire, en pour cent.
    #[must_use]
    pub fn part_residente_pct(&self) -> f32 {
        if self.cellules_total == 0 {
            return 0.0;
        }
        100.0 * self.cellules_chargees as f32 / self.cellules_total as f32
    }

    /// Le verdict, **par ordre de gravité décroissante**.
    ///
    /// L'ordre EST la décision : un défaut de cuisson se lit avant un réglage de
    /// streaming, parce qu'il veut dire que la carte n'est pas celle qu'on
    /// croit. Rendre le moins grave en premier ferait passer le plus grave pour
    /// absent.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        if self.cellules_total == 0 {
            return Verdict {
                severity: "critical",
                next_step: "CARTE_VIDE : zero cellule au manifeste — verifier \
                    vallon_stream_cells.toml et que 92_cellules.py a bien cuit",
            };
        }
        if self.colliders == 0 {
            return Verdict {
                severity: "critical",
                next_step: "AUCUN SOLIDE : le decor entier se traverse — le manifeste \
                    n'a pas de colliders_prop_xyzhr, ou toutes ses familles sont vides. \
                    Recuire avec vallon.py",
            };
        }
        if !self.joueur_place {
            return Verdict {
                severity: "warn",
                next_step: "JOUEUR NON PLACE : il est reste a l'origine, donc a 124 m \
                    de la carte, et il tombe. Voir place_player_when_ready",
            };
        }
        if self.defauts_de_cuisson > 0 {
            return Verdict {
                severity: "warn",
                next_step: "DEFAUTS DE CUISSON : la carte n'a pas produit tout ce \
                    qu'elle declare (zones de faune, abris). Lire le bloc defauts du \
                    manifeste — il nomme la cause et compte les rejets",
            };
        }
        if self.camps_en_tir_gratuit > 0 {
            return Verdict {
                severity: "warn",
                next_step: "TIR GRATUIT : un campement est plus large que la vision de \
                    ses ennemis, ils se font tirer sans pouvoir repondre. Le rayon \
                    derive de min(vision)/2 sous Blender — la derivation a saute",
            };
        }
        if self.abris_menteurs > 0 {
            return Verdict {
                severity: "warn",
                next_step: "ABRI QUI N'ABRITE PAS : un bloc sous l'oeil du joueur \
                    (1,70 m) masque le corps mais pas la vue. Il compte comme \
                    couverture sans en etre une — remonter son echelle sous Blender",
            };
        }
        // 🚨 CE CONTROLE A ETE INVERSE, ET VOICI POURQUOI
        //
        // Il alertait « STREAMING INUTILE » au-dela de 85 % de residence. Il a
        // donc alerte des sa premiere partie (87,2 % mesures) — sur un etat qui
        // est CORRECT.
        //
        // La mesure hors ligne le montre : le brouillard porte a 420 m au depart
        // (`cycle.rs`) pour une demi-diagonale de 172 m. La carte entiere est
        // dans le champ de vision, donc elle DOIT etre chargee. 95 % de
        // residence n'est pas un reglage rate, c'est la consequence exacte de
        // ces deux nombres — et affiner les cellules aurait multiplie les
        // appels de rendu en croyant les reduire (633 -> ~1800 primitives).
        //
        // Le vrai defaut, lui, est l'inverse : une cellule VISIBLE et NON
        // CHARGEE, c'est-a-dire du pop-in. Il n'est possible que si le rayon de
        // chargement est plus court que ce qu'on voit.
        //
        // Un capteur qui alerte a tort finit ignore, et c'est alors la vraie
        // alerte qu'on rate. Celui-ci le disait dans son propre test.
        if self.rayon_chargement_m < DEMI_DIAGONALE_M
            && self.cellules_chargees < self.cellules_total
        {
            return Verdict {
                severity: "info",
                next_step: "POP-IN POSSIBLE : des cellules sont hors du rayon de \
                    chargement alors que le brouillard porte plus loin que la carte. \
                    Se juge a l'oeil depuis le depart — la ceinture de 26 m les \
                    occulte peut-etre entierement. Si du decor apparait devant le \
                    joueur, relever RAYON_CHARGEMENT_M au-dela de la demi-diagonale",
            };
        }
        // LE VENT NE S'EST PAS ACCROCHE. La carte est chargee, donc du
        // feuillage est arrive — mais aucun materiau n'a recu l'extension.
        // Deux causes, dans cet ordre : les noms de matiere du glTF ne
        // commencent plus par les prefixes attendus (`leafs`, `grass`...), ou
        // le composant `GltfMaterialName` n'est plus pose par le chargeur.
        // Dans les deux cas le decor est parfaitement immobile ET parfaitement
        // silencieux — c'est precisement ce qu'un capteur doit attraper.
        if self.materiaux_vent == 0 {
            return Verdict {
                severity: "info",
                next_step: "VENT INACTIF : aucun materiau n'a recu l'extension.                     Verifier que les noms de matiere glTF commencent par leafs/                     grass/plant/flower (meme liste que 14_ancrage.py), et que le                     joueur a bien atteint une cellule feuillue",
            };
        }
        Verdict {
            severity: "ok",
            next_step: "",
        }
    }
}

/// Écrit le capteur, une fois par seconde.
pub fn capteur_carte(
    time: Res<Time>,
    mut accum: Local<f32>,
    mode: Res<State<GameMode>>,
    active: Option<Res<ActiveExpedition>>,
    vent: Option<Res<crate::vent::VentState>>,
    q_cells: Query<&ExpeditionCell>,
    q_player: Query<&GlobalTransform, With<Player>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let en_expedition = *mode.get() == GameMode::Expedition;
    let Some(active) = active else {
        // On écrit QUAND MÊME. Un capteur muet se lit comme « rien à signaler »
        // alors qu'il veut dire « je ne tourne pas » — c'est la classe de
        // défaut que `sensor_honesty` traque dans ce projet.
        let json = format!(
            r#"{{"id":"expedition_carte","severity":"info","next_step":"","timestamp_secs":{:.1},"en_expedition":{en_expedition},"carte_chargee":false}}"#,
            time.elapsed_secs()
        );
        let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_carte.json", json);
        return;
    };

    let m = &active.gameplay;
    let etat = EtatCarte {
        cellules_total: active.cells.cells.len(),
        cellules_chargees: q_cells.iter().count(),
        colliders: active.colliders_spawned,
        camps_en_tir_gratuit: m
            .campements
            .iter()
            .filter(|c| !c.vision_couvre_la_ligne())
            .count(),
        abris_menteurs: m.abris_qui_n_abritent_pas().count(),
        defauts_de_cuisson: m.defauts.len(),
        joueur_place: !active.spawn_pending,
        rayon_chargement_m: active.radii.load_m,
        materiaux_vent: vent.as_deref().map_or(0, |v| v.materiaux.len()),
    };
    let v = etat.verdict();

    // Progression du joueur le long du chemin — c'est elle qui situe tout le
    // reste. Un capteur qui dit « 40 cellules chargées » sans dire OÙ est le
    // joueur ne permet pas de savoir si c'est normal.
    let chemin: Vec<Vec3> = m.chemin_bevy().collect();
    let avancee = q_player
        .single()
        .ok()
        .map(|t| crate::cycle::progression_sur_chemin(t.translation(), &chemin))
        .unwrap_or(0.0);

    // Le détail par famille : un total qui tient pendant qu'une famille se vide
    // ne se verrait pas. C'est exactement ainsi que 207 arbres isolés, 110
    // rochers et 15 abris ont passé des semaines sans collision.
    //
    // Elle compte ce qui est POSE (`active.colliders_par_famille`), pas ce
    // que le manifeste declare : depuis les enveloppes convexes, les deux
    // different de 26 solides — des pieces placees dans la scene et jamais
    // inscrites comme solides. Lire la declaration ferait mentir la
    // ventilation pendant que le total, lui, dirait vrai.
    let mut familles: Vec<String> = active
        .colliders_par_famille
        .iter()
        .map(|(f, n)| format!(r#""{f}":{n}"#))
        .collect();
    familles.sort();

    let json = format!(
        r#"{{"id":"expedition_carte","severity":"{}","next_step":"{}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"carte_chargee":true,"carte":"{}","cellules_total":{},"cellules_chargees":{},"part_residente_pct":{:.1},"cellules_chargees_session":{},"solides":{},"solides_par_famille":{{{}}},"campements":{},"camps_en_tir_gratuit":{},"abris_menteurs":{},"defauts_de_cuisson":{},"zones_faune":{},"braseros":{},"chemin_m":{:.1},"avancee":{avancee:.3},"joueur_place":{},"rayon_chargement_m":{:.0},"materiaux_vent":{}}}"#,
        v.severity,
        v.next_step,
        time.elapsed_secs(),
        m.carte,
        etat.cellules_total,
        etat.cellules_chargees,
        etat.part_residente_pct(),
        active.cells_loaded_session,
        etat.colliders,
        familles.join(","),
        m.campements.len(),
        etat.camps_en_tir_gratuit,
        etat.abris_menteurs,
        etat.defauts_de_cuisson,
        m.faune.len(),
        m.lampes.len(),
        m.longueur_chemin_m(),
        etat.joueur_place,
        active.radii.load_m,
        etat.materiaux_vent,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_carte.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saine() -> EtatCarte {
        EtatCarte {
            cellules_total: 47,
            cellules_chargees: 20,
            colliders: 1616,
            camps_en_tir_gratuit: 0,
            abris_menteurs: 0,
            defauts_de_cuisson: 0,
            joueur_place: true,
            rayon_chargement_m: 140.0,
            materiaux_vent: 3,
        }
    }

    #[test]
    fn une_carte_saine_n_alerte_jamais() {
        // Un capteur qui alerte en permanence finit ignore, et c'est alors la
        // vraie alerte qu'on rate.
        //
        // `saine()` reproduit l'etat REEL mesure en partie : 20 cellules sur 47
        // chargees, rayon 140 m. Le verdict attendu est donc `info` — il reste
        // du decor hors du rayon alors que le brouillard porte plus loin, et ca
        // se verifie a l'oeil. Ce qui est INTERDIT, c'est d'en faire un `warn` :
        // c'est precisement ce que ce controle faisait, et il a alerte des la
        // premiere partie sur un etat correct.
        let v = saine().verdict();
        assert!(
            v.severity == "ok" || v.severity == "info",
            "une carte saine a rendu « {} » : {}",
            v.severity,
            v.next_step
        );
    }

    #[test]
    fn zero_solide_est_critique_et_pas_un_silence() {
        // C'est l'etat qu'avait la carte pour ses rochers, eboulis et abris : le
        // decor se traversait et RIEN ne le disait. Un compte a zero n'est pas
        // une absence de probleme, c'est le probleme.
        let e = EtatCarte { colliders: 0, ..saine() };
        assert_eq!(e.verdict().severity, "critical");
    }

    #[test]
    fn une_residence_elevee_n_est_pas_une_alerte() {
        // # Le test qui grave la correction
        //
        // Ce controle alertait au-dela de 85 % de residence, et il a donc
        // alerte des la premiere partie reelle (87,2 % mesures) sur un etat
        // CORRECT : le brouillard porte a 420 m pour une demi-diagonale de
        // 172 m, donc toute la carte est visible et doit etre chargee.
        //
        // Le seul verdict admis ici est `info` — un constat a verifier a
        // l'oeil, jamais un `warn` qui ferait corriger ce qui va bien.
        let e = EtatCarte { cellules_chargees: 44, ..saine() };
        assert!(e.part_residente_pct() > 90.0);
        assert_ne!(e.verdict().severity, "warn", "une carte saine ne doit pas alerter");
    }

    #[test]
    fn une_carte_entierement_residente_ne_dit_rien_du_tout() {
        // Rayon superieur a la demi-diagonale : plus rien ne peut etre
        // decharge, donc aucun pop-in n'est possible. C'est le cas le plus
        // sain qui soit, et il doit sortir muet.
        let e = EtatCarte { cellules_chargees: 47, rayon_chargement_m: 200.0, ..saine() };
        assert_eq!(e.verdict().severity, "ok");
    }

    #[test]
    fn une_carte_vide_ne_se_lit_pas_comme_un_streaming_efficace() {
        // Le piege : 0 cellule chargee sur 0 donne 0 %, donc « streaming
        // parfait ». Sans le garde, la carte la plus cassee sortirait au vert.
        let e = EtatCarte { cellules_total: 0, cellules_chargees: 0, ..saine() };
        assert_eq!(e.part_residente_pct(), 0.0);
        assert_eq!(e.verdict().severity, "critical");
    }

    #[test]
    fn le_plus_grave_passe_devant() {
        // Une carte sans solide ET avec du tir gratuit doit rendre le premier :
        // trier a l'envers ferait passer le pire pour absent.
        let e = EtatCarte { colliders: 0, camps_en_tir_gratuit: 3, ..saine() };
        assert!(e.verdict().next_step.contains("AUCUN SOLIDE"));
    }

    #[test]
    fn les_defauts_de_cuisson_remontent() {
        let e = EtatCarte { defauts_de_cuisson: 2, ..saine() };
        assert_eq!(e.verdict().severity, "warn");
        assert!(e.verdict().next_step.contains("DEFAUTS DE CUISSON"));
    }

    #[test]
    fn un_abri_menteur_est_signale_meme_quand_tout_le_reste_va_bien() {
        // Il ne casse rien, il MENT : il se compte comme couverture. C'est
        // precisement ce qui ne se voit jamais sans capteur.
        let e = EtatCarte { abris_menteurs: 2, ..saine() };
        assert_eq!(e.verdict().severity, "warn");
        assert!(e.verdict().next_step.contains("ABRI"));
    }
}

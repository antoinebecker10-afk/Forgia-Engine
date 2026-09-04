//! navpath.rs — le bot suit un chemin du navmesh au lieu de foncer en ligne droite.
//!
//! # Ce que cet incrément change, et surtout ce qu'il NE change PAS
//!
//! `bot_tactical_movement` calcule une direction de base (`fwd_dir` = vers la cible), puis
//! empile dessus le strafe, l'évitement local, le glissement contre les murs et le suivi
//! de sol. **Une seule ligne portait « fonce vers la cible ».** C'est elle, et elle seule,
//! qui est remplacée par « va vers le prochain point du chemin ».
//!
//! Tout le comportement tactique existant — qui fonctionne — reste intact. Le repli est la
//! ligne droite : sans maillage (hors arène, pendant le chargement), le bot se comporte
//! exactement comme avant.
//!
//! # Le désenlisement existant devient le filet, il n'est pas remplacé
//!
//! `unstick_step` détecte déjà la non-progression et fait longer l'obstacle. Il reste en
//! place **sous** le suivi de chemin, et son compteur `unstick_triggered_session` devient
//! la mesure de succès de cet incrément : il doit **chuter**. S'il ne bouge pas, le
//! chemin n'est pas suivi.

use crate::tactical::TacticalTuning;
use crate::{ArenaBot, BotState, BotTarget};
use bevy::prelude::*;
use forgia_navmesh::{ActiveNavMesh, PathOutcome};

/// Le chemin courant d'un bot vers sa cible.
#[derive(Component, Default)]
pub struct BotPath {
    /// Points en plan XZ. Vide = pas de chemin (le bot retombe en ligne droite).
    pub waypoints: Vec<Vec2>,
    /// Index du point visé. `>= waypoints.len()` ⇒ chemin épuisé.
    pub cursor: usize,
    /// Temps restant avant le prochain recalcul autorisé (s).
    pub repath_left: f32,
    /// Position de la cible au moment du calcul — sert à détecter qu'elle a bougé.
    pub planned_for: Vec2,
}

impl BotPath {
    /// Le point visé, ou `None` si le chemin est vide ou épuisé.
    #[must_use]
    pub fn current(&self) -> Option<Vec2> {
        self.waypoints.get(self.cursor).copied()
    }

    /// Sommes-nous sur le DERNIER tronçon du chemin ?
    ///
    /// C'est la frontière entre **traverser** et **engager**, et elle décide de tout le
    /// comportement (cf. `bot_tactical_movement`) : en traversée on suit le chemin au
    /// pied de la lettre, à l'engagement on rend la main au strafe et à l'évitement.
    ///
    /// Un chemin vide compte comme final : sans chemin, il n'y a rien à traverser.
    #[must_use]
    pub fn is_final_leg(&self) -> bool {
        self.cursor + 1 >= self.waypoints.len()
    }

    /// Avance le curseur au-delà des points déjà atteints.
    ///
    /// Boucle plutôt que test simple : un bot rapide peut franchir deux points serrés
    /// dans la même frame, et s'arrêter au premier le ferait revenir en arrière.
    pub fn advance(&mut self, pos: Vec2, arrive_m: f32) {
        while let Some(w) = self.current() {
            if pos.distance(w) > arrive_m {
                break;
            }
            self.cursor += 1;
        }
    }

    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.cursor = 0;
    }

    /// Longueur qu'il reste à parcourir sur CE chemin, depuis `pos`.
    ///
    /// Sert à décider s'il vaut la peine d'en changer ([`vaut_la_peine_de_changer`]).
    /// Un chemin vide ou épuisé rend `+∞` : « pas de route » doit toujours perdre
    /// face à une route réelle, jamais gagner par un zéro trompeur.
    #[must_use]
    pub fn remaining_length(&self, pos: Vec2) -> f32 {
        let Some(premier) = self.current() else {
            return f32::INFINITY;
        };
        let mut total = pos.distance(premier);
        for paire in self.waypoints[self.cursor..].windows(2) {
            total += paire[0].distance(paire[1]);
        }
        total
    }
}

/// Faut-il abandonner la route courante pour la nouvelle ? **PUR**, donc testable.
///
/// # Le défaut que cette fonction corrige — mesuré en jeu le 2026-08-13
///
/// `sys_bot_navpath` remettait `cursor = 0` à CHAQUE recalcul, toutes les 0,5 s.
/// Quand deux routes ont un coût quasi égal — contourner un pilier par la gauche
/// ou par la droite — le moindre déplacement du bot fait basculer polyanya de
/// l'une à l'autre. Le bot repart alors en sens inverse avant d'avoir atteint son
/// premier point, et recommence.
///
/// Relevé sur quatre bots à la fois : **8 à 20 m parcourus pour 1 à 17 cm de
/// déplacement net**, curseur bloqué à 0 sur des chemins de 3 et 4 points, sans
/// le moindre mur ni refus de pas. Le chien de garde ne voyait rien : ils
/// avançaient, à pleine vitesse.
///
/// # La marge se DÉRIVE, elle ne se choisit pas
///
/// La source du bruit est connue : entre deux recalculs, le bot s'est déplacé de
/// `vitesse × période`. C'est exactement ce qui fait basculer deux routes
/// équivalentes. On n'accepte donc une nouvelle route que si elle économise
/// **plus que ce déplacement** — en deçà, le gain est indiscernable du bruit qui
/// l'a produit.
///
/// À 3,5 m/s et 0,5 s de période, la marge vaut 1,75 m. Aucun gène nouveau : elle
/// sort de deux valeurs qui existent déjà.
#[must_use]
pub fn vaut_la_peine_de_changer(
    restant_actuel_m: f32,
    nouveau_m: f32,
    vitesse_m_s: f32,
    periode_recalcul_s: f32,
) -> bool {
    // Pas de route actuelle : toute route réelle vaut mieux.
    if !restant_actuel_m.is_finite() {
        return true;
    }
    let marge = vitesse_m_s * periode_recalcul_s;
    nouveau_m + marge < restant_actuel_m
}

/// Faut-il recalculer ? Pur, testable headless.
///
/// Trois raisons, et pas une de plus : plus de chemin, la cible a bougé au-delà du seuil,
/// ou le délai est écoulé. Recalculer chaque frame pour N bots trianguleraient l'arène en
/// boucle — le chemin est du travail de planification, pas du travail de frame.
/// # 2026-08-13 — le délai PRIME, sans exception
///
/// La version d'origine renvoyait `true` dès que le chemin était vide, **avant** de
/// regarder le délai. Or un chemin refusé par le maillage est justement vidé : un bot
/// dans cette situation re-demandait donc un chemin **à chaque frame**.
///
/// Mesuré en jeu : 30 700 requêtes en 166 s pour 7 bots — **26 par bot et par seconde**,
/// contre les 2 Hz voulus. Treize fois trop, sur le chemin le plus coûteux du système,
/// et le capteur `perf_diag` signalait un micro-stutter dans la même run.
///
/// Le délai est désormais le garde maître : un bot sans chemin attend au plus une
/// période avant de retenter. C'est exactement ce que le throttle est censé garantir.
#[must_use]
pub fn needs_repath(
    path: &BotPath,
    target: Vec2,
    moved_threshold_m: f32,
    cooldown_elapsed: bool,
) -> bool {
    // Délai écoulé : rafraîchissement périodique, même si rien n'a bougé. C'est lui
    // qui redonne sa chance à un bot dont le chemin a été refusé — sous 0,5 s, pas
    // sous 16 ms.
    if cooldown_elapsed {
        return true;
    }
    // Avant le délai, une seule urgence : la cible a franchi le seuil. Et comme
    // `planned_for` est réécrit à CHAQUE tentative — réussie **ou refusée** — elle doit
    // re-parcourir tout le seuil pour redéclencher. À 9,75 m/s, franchir 2 m prend
    // 0,2 s : le pire cas est donc borné à 5 Hz, jamais 60.
    path.planned_for.distance(target) > moved_threshold_m
}

/// Attache un [`BotPath`] à tout bot qui n'en a pas.
///
/// Fait ici plutôt qu'au spawn : les bots sont créés par plusieurs crates de mode, et
/// aller modifier chacune pour ajouter un composant serait un couplage inutile. Un bot
/// sans `BotPath` reste d'ailleurs parfaitement fonctionnel — il fonce en ligne droite,
/// comme avant.
pub fn sys_attach_bot_path(
    mut commands: Commands,
    sans_chemin: Query<Entity, (With<ArenaBot>, Without<BotPath>)>,
) {
    for e in &sans_chemin {
        commands.entity(e).insert(BotPath::default());
    }
}

/// Entretient le chemin de chaque bot : avance le curseur, recalcule quand il faut.
///
/// Tourne **avant** `bot_tactical_movement`, qui ne fait que lire le point courant.
pub fn sys_bot_navpath(
    mut bots: Query<(&ArenaBot, &Transform, &mut BotPath), Without<BotTarget>>,
    targets: Query<&Transform, With<BotTarget>>,
    nav: Res<ActiveNavMesh>,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
    mut sensor: ResMut<crate::tactical::BotAiSensor>,
) {
    let Some(target_tf) = targets.iter().next() else {
        return;
    };
    // Sans maillage, on n'entretient rien : les chemins sont vidés pour que le
    // mouvement retombe explicitement en ligne droite, au lieu de suivre des points
    // calculés sur une géométrie qui n'est plus là.
    if !nav.is_built() {
        for (_, _, mut path) in &mut bots {
            if !path.waypoints.is_empty() {
                path.clear();
            }
        }
        return;
    }

    let target = target_tf.translation.xz();
    let dt = time.delta_secs();

    for (bot, xf, mut path) in &mut bots {
        if bot.state == BotState::Dead {
            path.clear();
            continue;
        }
        let pos = xf.translation.xz();
        path.advance(pos, tuning.waypoint_arrive_m);
        path.repath_left = (path.repath_left - dt).max(0.0);

        if !needs_repath(
            &path,
            target,
            tuning.target_moved_repath_m,
            path.repath_left <= 0.0,
        ) {
            continue;
        }
        path.repath_left = tuning.repath_period_secs;
        // La cible a-t-elle bougé ? À lire AVANT d'écraser `planned_for` — c'est
        // ce qui distingue « rafraîchissement périodique » de « la cible a fui ».
        // Sur une fuite, la route actuelle mène au mauvais endroit : on l'adopte
        // sans discuter. L'hystérésis ne doit jamais retenir un chemin périmé.
        let cible_a_bouge = path.planned_for.distance(target) > tuning.target_moved_repath_m;
        path.planned_for = target;
        let restant_avant = path.remaining_length(pos);

        let poser = |found: &forgia_navmesh::Path, path: &mut BotPath| -> bool {
            // Hystérésis — cf. `vaut_la_peine_de_changer`. Sans elle, deux routes
            // de coût équivalent s'alternent toutes les 0,5 s et le bot fait du
            // sur-place : 20 m parcourus pour 7 cm nets, mesurés en jeu.
            if !cible_a_bouge
                && !vaut_la_peine_de_changer(
                    restant_avant,
                    found.length,
                    bot.speed,
                    tuning.repath_period_secs,
                )
            {
                return false;
            }
            path.waypoints.clear();
            path.waypoints.extend(found.path.iter().copied());
            path.cursor = 0;
            // Le premier point peut être déjà atteint (le bot est dessus).
            path.advance(pos, tuning.waypoint_arrive_m);
            true
        };

        match nav.route(pos, target) {
            PathOutcome::Direct(found) => {
                sensor.paths_ok_session = sensor.paths_ok_session.saturating_add(1);
                if !poser(&found, &mut path) {
                    sensor.paths_kept_session = sensor.paths_kept_session.saturating_add(1);
                }
            }
            // Le joueur était dans la bande que le maillage abandonne le long des murs
            // et autour des obstacles — le cas le plus courant, et celui qui coûtait
            // 13,5 % de refus le 2026-08-13. Le chemin s'arrête à un rayon d'agent de
            // lui ; le dernier mètre revient à la ligne droite, comme tout dernier
            // tronçon. Compté à part : si ce compteur explose, ce n'est plus un bord
            // qu'on rattrape, c'est une géométrie qui ne correspond plus au maillage.
            PathOutcome::Snapped {
                path: found,
                from_off_mesh,
                ..
            } => {
                sensor.paths_ok_session = sensor.paths_ok_session.saturating_add(1);
                sensor.paths_snapped_session = sensor.paths_snapped_session.saturating_add(1);
                // Le BOT hors du maillage est un tout autre diagnostic que le joueur
                // hors du maillage : cela veut dire qu'il se tient là où le maillage
                // le lui interdit, donc que l'emprise déclarée d'un obstacle ne
                // correspond pas à son collider. Sans ce partage, « 34,6 % de
                // recalages » ne se lit pas.
                if from_off_mesh {
                    sensor.paths_snapped_bot_session =
                        sensor.paths_snapped_bot_session.saturating_add(1);
                }
                if !poser(&found, &mut path) {
                    sensor.paths_kept_session = sensor.paths_kept_session.saturating_add(1);
                }
            }
            // Aucun trajet, même après recalage. Le repli est la ligne droite, c'est-à-
            // dire le comportement d'AVANT le navmesh — un bot qui échoue toujours au
            // même endroit y est bloqué comme avant.
            PathOutcome::NoRoute {
                from_off_mesh,
                to_off_mesh,
            } => {
                sensor.paths_failed_session = sensor.paths_failed_session.saturating_add(1);
                // OÙ, et surtout LEQUEL DES DEUX BOUTS. Le compteur seul a dit
                // « 105 refus » sans permettre de trancher entre « le bot est hors du
                // maillage », « la cible l'est » et « les deux sont navigables mais
                // rien ne les relie » — trois causes, trois remèdes opposés.
                sensor.last_fail_from = (pos.x, pos.y);
                sensor.last_fail_to = (target.x, target.y);
                sensor.last_fail_bot_off_mesh = from_off_mesh;
                sensor.last_fail_target_off_mesh = to_off_mesh;
                path.clear();
            }
            // Pas de maillage : déjà traité en amont par `is_built()`, mais on ne
            // compte surtout pas un chargement comme un échec de navigation.
            PathOutcome::NoMesh => path.clear(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> TacticalTuning {
        TacticalTuning::default()
    }

    // ── La grandeur écrite deux fois ────────────────────────────────────

    #[test]
    fn le_ressaut_du_navmesh_et_celui_du_bot_sont_la_meme_grandeur() {
        // LE test qui compte. Le maillage decide de ce qui est franchissable, le bot
        // decide de ce qu'il franchit reellement. Si les deux divergent, le navmesh
        // trace des chemins que le bot ne peut pas emprunter — et le defaut ne leve
        // AUCUNE erreur : le bot suit un chemin valide et se bloque contre une marche.
        //
        // Les deux vivent dans des crates separees, donc le miroir est inevitable :
        // `spawn-clearance.md` §4bis exige alors un test qui compare les deux.
        let nav = forgia_navmesh::NavmeshBuild::default();
        let bot = tuning();
        assert_eq!(
            nav.step_height_m, bot.max_step_up_m,
            "assets/genomes/navmesh.toml [agent].step_height_m doit valoir \
             TacticalTuning::max_step_up_m — sinon le maillage promet des chemins \
             que le bot ne peut pas suivre"
        );
    }

    // ── L'hystérésis de route ───────────────────────────────────────────

    const V: f32 = 3.5; // vitesse d'un bot d'arène (m/s)
    const PERIODE: f32 = 0.5; // repath_period_secs

    #[test]
    fn deux_routes_equivalentes_ne_font_pas_changer_d_avis() {
        // LE test du defaut mesure le 2026-08-13. Contourner un pilier par la
        // gauche ou par la droite coute presque pareil ; le moindre deplacement du
        // bot fait basculer polyanya de l'une a l'autre, et il repart en sens
        // inverse. Quatre bots ont ainsi parcouru 8 a 20 m pour 1 a 17 cm nets.
        //
        // 20,0 m contre 19,8 m : un gain de 20 cm ne justifie pas un demi-tour.
        assert!(
            !vaut_la_peine_de_changer(20.0, 19.8, V, PERIODE),
            "un gain de 20 cm ne doit pas declencher un changement de route"
        );
    }

    #[test]
    fn une_route_franchement_meilleure_est_adoptee() {
        // L'hysteresis ne doit pas rendre le bot bete : une porte qui s'ouvre, un
        // raccourci qui apparait, ca se prend.
        assert!(vaut_la_peine_de_changer(20.0, 5.0, V, PERIODE));
    }

    #[test]
    fn sans_route_actuelle_toute_route_est_bonne() {
        // `remaining_length` rend +inf quand il n'y a pas de chemin. « Pas de
        // route » doit PERDRE contre une route reelle, jamais gagner par un zero
        // trompeur — d'ou l'infini plutot que zero.
        assert!(vaut_la_peine_de_changer(f32::INFINITY, 999.0, V, PERIODE));
    }

    #[test]
    fn la_marge_vaut_exactement_ce_que_le_bot_parcourt_entre_deux_recalculs() {
        // La marge se DERIVE de la source du bruit, elle ne se choisit pas : entre
        // deux recalculs le bot s'est deplace de `vitesse x periode`, et c'est
        // precisement ce deplacement qui fait basculer deux routes equivalentes.
        // Un gain inferieur est indiscernable du bruit qui l'a produit.
        let marge = V * PERIODE; // 1,75 m
        assert!(
            !vaut_la_peine_de_changer(20.0, 20.0 - marge + 0.01, V, PERIODE),
            "juste sous la marge : on garde"
        );
        assert!(
            vaut_la_peine_de_changer(20.0, 20.0 - marge - 0.01, V, PERIODE),
            "juste au-dessus : on change"
        );
    }

    #[test]
    fn un_bot_rapide_est_plus_stable_qu_un_bot_lent() {
        // Consequence directe et voulue de la derivation : un grunt a 9 m/s bouge
        // plus entre deux recalculs, donc son bruit est plus grand, donc il exige
        // un gain plus grand. Ce n'est pas un reglage, c'est la meme regle.
        let gain_moyen = 20.0 - 3.0;
        assert!(
            vaut_la_peine_de_changer(20.0, gain_moyen, 3.5, PERIODE),
            "bot lent : 3 m de gain suffisent"
        );
        assert!(
            !vaut_la_peine_de_changer(20.0, gain_moyen, 9.0, PERIODE),
            "bot rapide : 3 m de gain sont dans son bruit"
        );
    }

    #[test]
    fn la_longueur_restante_ignore_les_points_deja_franchis() {
        // Comparer la nouvelle route a la route COMPLETE au lieu du RESTANT ferait
        // paraitre toute nouvelle route meilleure a mesure qu'on avance — et
        // relancerait exactement l'oscillation qu'on corrige.
        let path = BotPath {
            waypoints: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(10.0, 0.0),
                Vec2::new(20.0, 0.0),
            ],
            cursor: 2,
            ..Default::default()
        };
        let restant = path.remaining_length(Vec2::new(19.0, 0.0));
        assert!(
            (restant - 1.0).abs() < 1.0e-4,
            "restant {restant} : seul le dernier troncon compte"
        );
    }

    #[test]
    fn un_chemin_epuise_rend_l_infini_et_pas_zero() {
        let mut path = BotPath {
            waypoints: vec![Vec2::new(1.0, 0.0)],
            ..Default::default()
        };
        path.advance(Vec2::new(1.0, 0.0), 0.5);
        assert!(path.remaining_length(Vec2::ZERO).is_infinite());
    }

    // ── Le curseur ──────────────────────────────────────────────────────

    #[test]
    fn un_chemin_vide_ne_donne_aucun_point() {
        let path = BotPath::default();
        assert!(path.current().is_none());
    }

    #[test]
    fn le_curseur_saute_plusieurs_points_atteints_dans_la_meme_frame() {
        // Un bot rapide (9 m/s pour un grunt) peut franchir deux points serres en une
        // frame. S'arreter au premier le ferait revenir en arriere.
        let mut path = BotPath {
            waypoints: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(0.1, 0.0),
                Vec2::new(10.0, 0.0),
            ],
            ..Default::default()
        };
        path.advance(Vec2::new(0.05, 0.0), 0.5);
        assert_eq!(path.current(), Some(Vec2::new(10.0, 0.0)));
    }

    #[test]
    fn un_chemin_epuise_rend_none_au_lieu_de_paniquer() {
        let mut path = BotPath {
            waypoints: vec![Vec2::new(1.0, 0.0)],
            ..Default::default()
        };
        path.advance(Vec2::new(1.0, 0.0), 0.5);
        assert!(path.current().is_none(), "curseur au-dela du dernier point");
    }

    // ── La décision de recalcul ─────────────────────────────────────────

    #[test]
    fn un_bot_sans_chemin_ne_martele_pas_le_maillage() {
        // LE test qui aurait evite la tempete du 2026-08-13. Un chemin refuse est
        // vide ; si « vide » suffisait a redemander, le bot re-interrogerait polyanya
        // A CHAQUE FRAME. Mesure en jeu avant correctif : 26 requetes par bot et par
        // seconde pour 2 Hz voulus.
        let path = BotPath::default();
        assert!(
            !needs_repath(&path, Vec2::ZERO, 5.0, false),
            "delai non ecoule : on attend, meme sans chemin"
        );
        assert!(
            needs_repath(&path, Vec2::ZERO, 5.0, true),
            "delai ecoule : on retente"
        );
    }

    #[test]
    fn une_cible_qui_a_bouge_declenche_le_recalcul_sans_attendre() {
        // La cible qui franchit le seuil reste une urgence : c'est ce qui garde la
        // poursuite reactive malgre le throttle. Elle ne peut pas provoquer de tempete
        // parce que `planned_for` est reecrit a chaque tentative, meme refusee.
        let path = BotPath {
            waypoints: vec![Vec2::new(1.0, 0.0)],
            planned_for: Vec2::ZERO,
            ..Default::default()
        };
        assert!(needs_repath(&path, Vec2::new(6.0, 0.0), 5.0, false));
    }

    #[test]
    fn une_cible_immobile_et_un_delai_non_ecoule_ne_declenchent_rien() {
        // Le cas qui protege le budget : N bots ne doivent pas retrianguler chaque frame.
        let path = BotPath {
            waypoints: vec![Vec2::new(1.0, 0.0)],
            planned_for: Vec2::ZERO,
            ..Default::default()
        };
        assert!(!needs_repath(&path, Vec2::new(0.5, 0.0), 5.0, false));
    }

    #[test]
    fn le_delai_ecoule_declenche_meme_si_la_cible_n_a_pas_bouge() {
        let path = BotPath {
            waypoints: vec![Vec2::new(1.0, 0.0)],
            planned_for: Vec2::ZERO,
            ..Default::default()
        };
        assert!(needs_repath(&path, Vec2::ZERO, 5.0, true));
    }
}

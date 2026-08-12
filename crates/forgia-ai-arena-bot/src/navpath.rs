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
use forgia_navmesh::ActiveNavMesh;

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
}

/// Faut-il recalculer ? Pur, testable headless.
///
/// Trois raisons, et pas une de plus : plus de chemin, la cible a bougé au-delà du seuil,
/// ou le délai est écoulé. Recalculer chaque frame pour N bots trianguleraient l'arène en
/// boucle — le chemin est du travail de planification, pas du travail de frame.
#[must_use]
pub fn needs_repath(
    path: &BotPath,
    target: Vec2,
    moved_threshold_m: f32,
    cooldown_elapsed: bool,
) -> bool {
    if path.current().is_none() {
        return true;
    }
    if path.planned_for.distance(target) > moved_threshold_m {
        return true;
    }
    cooldown_elapsed
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
        path.planned_for = target;

        match nav.path(pos, target) {
            Some(found) => {
                sensor.paths_ok_session = sensor.paths_ok_session.saturating_add(1);
                path.waypoints.clear();
                path.waypoints.extend(found.path.iter().copied());
                path.cursor = 0;
                // Le premier point peut être déjà atteint (le bot est dessus).
                path.advance(pos, tuning.waypoint_arrive_m);
            }
            // Aucun trajet : le bot ou la cible est HORS du maillage — au-delà du bord,
            // ou à l'intérieur d'un obstacle dilaté. On vide plutôt que de garder un
            // chemin périmé, mais il faut voir ce cas : le repli est la ligne droite,
            // c'est-à-dire le comportement d'AVANT le navmesh. Un bot qui échoue
            // toujours au même endroit y est donc bloqué comme avant, et c'est
            // exactement le « ils se bloquent à certains endroits » du 2026-08-13.
            None => {
                sensor.paths_failed_session = sensor.paths_failed_session.saturating_add(1);
                path.clear();
            }
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
    fn sans_chemin_on_recalcule_toujours() {
        let path = BotPath::default();
        assert!(needs_repath(&path, Vec2::ZERO, 5.0, false));
    }

    #[test]
    fn une_cible_qui_a_bouge_declenche_le_recalcul() {
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

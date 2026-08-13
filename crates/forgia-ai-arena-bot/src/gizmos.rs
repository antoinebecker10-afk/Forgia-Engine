//! gizmos.rs — voir en jeu ce que le capteur compte.
//!
//! # Pourquoi ce fichier
//!
//! `forgia2_bot_traces.json` sait dire qu'un bot piétine et pourquoi. Il ne sait
//! pas dire **lequel, à l'écran**. Demandé en jeu le 2026-08-13 : *« avec F10 on
//! peut ajouter les gizmos des bots ? Tout ce qui gravite autour d'eux, que je
//! puisse contrôler visuellement, des couleurs différentes pour que je m'y
//! retrouve. »*
//!
//! Les deux se complètent et aucun ne remplace l'autre : le capteur donne les
//! chiffres exacts et l'historique cumulé, les gizmos donnent le **où** et le
//! **quoi** instantanés. Le premier diagnostic de la journée — un bot bloqué sur
//! une paroi de 0,60 m — aurait pris dix secondes avec ça.
//!
//! # La légende, et pourquoi ces couleurs
//!
//! Le code couleur suit une règle simple : **ce que le bot VEUT est chaud, ce
//! qu'il PERÇOIT est froid, ce qui ne VA PAS est saturé.**
//!
//! | élément | couleur | ce qu'il dit |
//! |---|---|---|
//! | disque au sol | **cyan** | son emprise RÉELLE (`body_radius_m`) |
//! | anneau d'état | gris / **orange** / **rouge** | Idle / Chase / Attack |
//! | chemin | **vert** | ce que le maillage lui a tracé |
//! | point visé | **vert vif** | le waypoint courant |
//! | ligne vers la cible | **blanc** | la ligne droite, pour comparer au chemin |
//! | sonde de mur | **magenta** | la hauteur à laquelle il teste les murs |
//! | sonde de sol | **jaune** | ce qui le porte |
//! | croix rouge | **rouge vif** | FIGÉ — son pas est refusé |
//! | anneau jaune | **jaune vif** | PIÉTINE — il marche sans avancer |
//! | disque violet | **violet** | traversée d'exception en cours |
//!
//! Un bot sain n'affiche donc que du cyan, de l'orange et du vert. **Toute
//! couleur saturée est une anomalie** — c'est le seul critère à retenir.

use crate::navpath::BotPath;
use crate::tactical::{BotTrace, StepRefusal, TacticalTuning};
use crate::{ArenaBot, BotState, BotTarget};
use bevy::color::palettes::css;
use bevy::prelude::*;

/// Ce que F10 affiche. Cycle plutôt que bascule : le fil-de-fer des colliders
/// Rapier est visuellement bruyant, et le vouloir en même temps que les gizmos de
/// bots n'est vrai qu'une fois sur trois.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotGizmoMode {
    #[default]
    Off,
    /// Les bots seuls — le mode utile 90 % du temps.
    Bots,
    /// Les bots ET le fil-de-fer des colliders : pour confronter l'emprise
    /// déclarée au collider réel, qui est LA classe de défaut de ce projet.
    BotsEtColliders,
}

impl BotGizmoMode {
    /// L'ordre du cycle. `Off` en premier pour qu'une pression de trop revienne
    /// à un écran propre plutôt que d'empiler du bruit.
    #[must_use]
    pub fn suivant(self) -> Self {
        match self {
            Self::Off => Self::Bots,
            Self::Bots => Self::BotsEtColliders,
            Self::BotsEtColliders => Self::Off,
        }
    }

    #[must_use]
    pub fn dessine_les_bots(self) -> bool {
        !matches!(self, Self::Off)
    }

    #[must_use]
    pub fn dessine_les_colliders(self) -> bool {
        matches!(self, Self::BotsEtColliders)
    }

    /// Libellé pour le log — l'user doit savoir dans quel mode il vient d'entrer
    /// sans avoir à le déduire de ce qui s'affiche.
    #[must_use]
    pub fn libelle(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Bots => "BOTS (emprise, etat, chemin, sondes, verdicts)",
            Self::BotsEtColliders => "BOTS + COLLIDERS Rapier",
        }
    }
}

// ── Palette ──────────────────────────────────────────────────────────────────
//
// Couleurs de debug : cosmétiques et non exposées au créateur, donc en dur —
// c'est l'exception explicite de `no-hardcode.md`.

const EMPRISE: Srgba = css::AQUA;
const ETAT_IDLE: Srgba = css::GRAY;
const ETAT_CHASE: Srgba = css::ORANGE;
const ETAT_ATTACK: Srgba = css::RED;
const CHEMIN: Srgba = css::LIME;
const POINT_VISE: Srgba = css::GREEN_YELLOW;
const LIGNE_CIBLE: Srgba = css::WHITE;
const SONDE_MUR: Srgba = css::MAGENTA;
const SONDE_SOL: Srgba = css::YELLOW;
const ALERTE_FIGE: Srgba = css::ORANGE_RED;
const ALERTE_PIETINE: Srgba = css::GOLD;
const TRAVERSEE: Srgba = css::BLUE_VIOLET;

/// Décale les marqueurs d'anomalie au-dessus de la tête : au sol ils seraient
/// masqués par le corps du bot, précisément quand on les cherche.
const HAUTEUR_MARQUEUR_M: f32 = 2.6;

/// Dessine tout ce qui gravite autour de chaque bot.
///
/// Ne tourne que si le mode le demande (`run_if`), donc coût nul quand F10 est
/// sur `Off` — un gizmo par bot et par frame n'est pas gratuit.
pub fn sys_bot_gizmos(
    mut gizmos: Gizmos,
    tuning: Res<TacticalTuning>,
    time: Res<Time>,
    bots: Query<
        (
            &ArenaBot,
            &Transform,
            Option<&BotPath>,
            Option<&BotTrace>,
        ),
        Without<BotTarget>,
    >,
    cible: Query<&Transform, With<BotTarget>>,
) {
    let cible_pos = cible.iter().next().map(|t| t.translation);
    // Pulsation 2 Hz : un marqueur d'anomalie doit attirer l'œil sans clignoter
    // au point d'être illisible.
    let pulse = 0.75 + 0.25 * (time.elapsed_secs() * std::f32::consts::TAU * 2.0).sin();

    for (bot, xf, chemin, trace) in &bots {
        if bot.state == BotState::Dead {
            continue;
        }
        let pieds = xf.translation - Vec3::Y * bot.foot_offset_m;
        let a_plat = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);

        // ── L'emprise RÉELLE ─────────────────────────────────────────────────
        // Le disque que le code de collision utilise vraiment. C'est LUI qui a
        // révélé qu'un tank de 0,55 m était traité comme un bot de 0,40.
        gizmos.circle(
            Isometry3d::new(pieds + Vec3::Y * 0.03, a_plat),
            bot.body_radius_m,
            EMPRISE,
        );

        // ── L'état ───────────────────────────────────────────────────────────
        let couleur_etat = match bot.state {
            BotState::Idle => ETAT_IDLE,
            BotState::Chase => ETAT_CHASE,
            BotState::Attack => ETAT_ATTACK,
            BotState::Dead => continue,
        };
        gizmos.circle(
            Isometry3d::new(pieds + Vec3::Y * (bot.foot_offset_m * 2.0), a_plat),
            bot.body_radius_m * 1.15,
            couleur_etat,
        );

        // ── Les deux sondes ──────────────────────────────────────────────────
        //
        // Les dessiner CÔTE À CÔTE est tout l'intérêt : elles partagent le même
        // seuil (`max_step_up_m`) et leur désaccord était le défaut du jour.
        // La sonde de mur est un anneau à la hauteur où le bot teste les murs ;
        // ce qui passe dessous est une marche, ce qui la touche est un mur.
        let y_sonde_mur = pieds.y + tuning.max_step_up_m;
        gizmos.circle(
            Isometry3d::new(Vec3::new(pieds.x, y_sonde_mur, pieds.z), a_plat),
            bot.body_radius_m * 1.02,
            SONDE_MUR,
        );
        // Sonde de sol : le segment vertical réellement balayé.
        let haut = pieds.y + tuning.ground_probe_height_m;
        gizmos.line(
            Vec3::new(pieds.x, haut, pieds.z),
            Vec3::new(pieds.x, haut - tuning.ground_probe_height_m - tuning.max_step_down_m, pieds.z),
            SONDE_SOL,
        );

        // ── Le chemin ────────────────────────────────────────────────────────
        if let Some(p) = chemin {
            let mut precedent = pieds;
            for (i, w) in p.waypoints.iter().enumerate() {
                let point = Vec3::new(w.x, pieds.y + 0.15, w.y);
                gizmos.line(precedent, point, CHEMIN);
                if i == p.cursor {
                    // Le point RÉELLEMENT visé, plus gros. Un curseur qui ne bouge
                    // pas alors que le bot marche, c'est le piétinement.
                    gizmos.sphere(point, 0.25, POINT_VISE);
                }
                precedent = point;
            }
        }

        // ── La ligne droite vers la cible ────────────────────────────────────
        // Superposée au chemin exprès : l'écart entre les deux EST le détour.
        if let Some(c) = cible_pos {
            gizmos.line(
                pieds + Vec3::Y * 0.1,
                Vec3::new(c.x, pieds.y + 0.1, c.z),
                LIGNE_CIBLE.with_alpha(0.25),
            );
        }

        // ── Les anomalies ────────────────────────────────────────────────────
        let sommet = pieds + Vec3::Y * HAUTEUR_MARQUEUR_M;

        // Traversée en cours : disque plein au sol, impossible à rater.
        if bot.phase_left > 0.0 {
            gizmos.circle(
                Isometry3d::new(pieds + Vec3::Y * 0.06, a_plat),
                bot.body_radius_m * 1.6 * pulse,
                TRAVERSEE,
            );
        }

        let Some(t) = trace else { continue };

        // FIGÉ — croix rouge. Le pas lui est refusé : rien ne le fera avancer.
        if t.fige(bot.speed) == Some(true) {
            let b = 0.35 * pulse;
            gizmos.line(sommet + Vec3::new(-b, -b, 0.0), sommet + Vec3::new(b, b, 0.0), ALERTE_FIGE);
            gizmos.line(sommet + Vec3::new(-b, b, 0.0), sommet + Vec3::new(b, -b, 0.0), ALERTE_FIGE);
        }
        // PIÉTINE — anneau doré. Il marche, mais tourne en rond.
        if t.pietine(bot.speed) == Some(true) {
            gizmos.circle(
                Isometry3d::new(sommet, a_plat),
                0.45 * pulse,
                ALERTE_PIETINE,
            );
        }

        // La CAUSE du dernier refus, dessinée là où elle agit : un segment
        // horizontal à la hauteur de la paroi rencontrée. Voir « 0,60 m » à
        // l'écran vaut mieux que le lire dans un JSON.
        if let Some(cause) = t.dernier_refus {
            let (hauteur, couleur) = match cause {
                StepRefusal::SolAbsent => (0.0, SONDE_SOL),
                StepRefusal::ParoiTropHaute { montee_m } => (montee_m, ALERTE_FIGE),
                StepRefusal::VideTropProfond { descente_m } => (-descente_m, SONDE_SOL),
            };
            let devant = xf.forward().as_vec3().with_y(0.0).normalize_or_zero()
                * (bot.body_radius_m + 0.3);
            let p = pieds + devant + Vec3::Y * hauteur;
            gizmos.line(p - Vec3::X * 0.4, p + Vec3::X * 0.4, couleur);
            gizmos.line(p - Vec3::Z * 0.4, p + Vec3::Z * 0.4, couleur);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_cycle_revient_toujours_a_off() {
        // Une pression de trop doit rendre l'ecran propre, jamais empiler du
        // bruit. Trois pressions = retour au point de depart.
        let mut m = BotGizmoMode::Off;
        for _ in 0..3 {
            m = m.suivant();
        }
        assert_eq!(m, BotGizmoMode::Off);
    }

    #[test]
    fn le_mode_bots_seul_ne_dessine_pas_les_colliders() {
        // Le fil-de-fer Rapier est bruyant : le mode utile au quotidien doit
        // pouvoir s'en passer.
        let m = BotGizmoMode::Off.suivant();
        assert!(m.dessine_les_bots());
        assert!(!m.dessine_les_colliders());
    }

    #[test]
    fn off_ne_dessine_rien_du_tout() {
        assert!(!BotGizmoMode::Off.dessine_les_bots());
        assert!(!BotGizmoMode::Off.dessine_les_colliders());
    }

    #[test]
    fn chaque_mode_a_un_libelle_distinct() {
        // L'user doit savoir dans quel mode il vient d'entrer sans le deduire de
        // ce qui s'affiche — sinon le cycle est un jeu de devinettes.
        let modes = [
            BotGizmoMode::Off,
            BotGizmoMode::Bots,
            BotGizmoMode::BotsEtColliders,
        ];
        let libelles: Vec<&str> = modes.iter().map(|m| m.libelle()).collect();
        for i in 0..libelles.len() {
            for j in (i + 1)..libelles.len() {
                assert_ne!(libelles[i], libelles[j], "libelles ambigus");
            }
        }
    }
}

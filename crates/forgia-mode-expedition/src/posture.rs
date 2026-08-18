//! posture — s'accroupir, et glisser au lieu de foncer.
//!
//! # Ce que ce module remplace
//!
//! L'arène a un **dash** : double-tap sur Espace, une impulsion instantanée dans
//! la direction du regard. C'est un mouvement d'arène — nerveux, abstrait, sans
//! poids. L'Expédition est un parcours : on y veut une **glissade**, qui part de
//! la course, se lit au sol, et coûte de la vitesse à la fin.
//!
//! Le dash n'est pas supprimé : il reste pour l'arène. Ici il est simplement
//! remplacé, et cette cohabitation est la raison pour laquelle ce module vit
//! dans le mode plutôt que dans le contrôleur partagé.
//!
//! # La glissade est une DURÉE, pas un état qu'on tient
//!
//! Si tenir la touche prolongeait la glissade, elle deviendrait un moyen de
//! déplacement gratuit — plus rapide que la course, sans coût. Elle a donc une
//! durée fixe, un délai avant de pouvoir recommencer, et elle exige de **courir**
//! pour se déclencher. Trois bornes qui la rendent un geste plutôt qu'une touche.

use bevy::prelude::*;
use forgia_core::prelude::{AppMode, GameMode, GameSet};
use forgia_input::PlayerAction;
use forgia_player::{Player, PlayerLocomotion, Posture};
use leafwing_input_manager::prelude::ActionState;
use serde::Deserialize;

/// Couche **definition** : ce qui se juge manette en main vit au génome.
const GENOME: &str = "assets/genomes/expedition_posture.toml";

#[derive(Resource, Debug, Clone, Copy, Deserialize)]
pub struct PostureConfig {
    /// Durée d'une glissade (s).
    #[serde(default = "duree_par_defaut")]
    pub glissade_duree_s: f32,
    /// Délai avant de pouvoir re-glisser (s). Sans lui, on enchaîne les
    /// glissades et on traverse la carte plus vite qu'en courant.
    #[serde(default = "repos_par_defaut")]
    pub glissade_repos_s: f32,
    /// Vitesse minimale pour déclencher. Glisser à l'arrêt n'a aucun sens : la
    /// glissade convertit de la vitesse, elle n'en crée pas.
    #[serde(default = "seuil_par_defaut")]
    pub glissade_vitesse_min_ms: f32,
    /// Ce que la vitesse devient pendant l'accroupissement (facteur).
    #[serde(default = "lenteur_par_defaut")]
    pub accroupi_facteur_vitesse: f32,
}

impl Default for PostureConfig {
    fn default() -> Self {
        Self {
            glissade_duree_s: duree_par_defaut(),
            glissade_repos_s: repos_par_defaut(),
            glissade_vitesse_min_ms: seuil_par_defaut(),
            accroupi_facteur_vitesse: lenteur_par_defaut(),
        }
    }
}

/// 0,7 s : la durée du clip `slide` de Mixamo, à peu de chose près. Une glissade
/// plus longue que son animation la ferait boucler — un personnage qui glisse en
/// répétant le même geste.
fn duree_par_defaut() -> f32 {
    0.7
}
/// Assez pour qu'enchaîner soit plus lent que courir. C'est ce qui empêche la
/// glissade de devenir le déplacement par défaut.
fn repos_par_defaut() -> f32 {
    1.2
}
/// Le seuil de course du personnage : on glisse depuis une course, pas depuis une
/// marche. Sinon le geste perd son sens.
fn seuil_par_defaut() -> f32 {
    5.0
}
/// Accroupi, on va à 45 % — assez lent pour que ce soit un choix, assez rapide
/// pour qu'on s'en serve.
fn lenteur_par_defaut() -> f32 {
    0.45
}

impl PostureConfig {
    #[must_use]
    pub fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                warn!("[posture] {GENOME} mal formé ({e}) — réglages par défaut");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    /// Peut-on déclencher une glissade ? **Pure**, donc testable — et c'est là
    /// que vivent les trois bornes qui empêchent la glissade de devenir gratuite.
    #[must_use]
    pub fn peut_glisser(&self, vitesse_ms: f32, repos_restant_s: f32, deja: bool) -> bool {
        !deja && repos_restant_s <= 0.0 && vitesse_ms >= self.glissade_vitesse_min_ms
    }
}

/// Le temps qui reste avant de pouvoir re-glisser. Séparé de `Posture` : celle-ci
/// décrit ce que le joueur EST, pas la comptabilité interne du mode.
#[derive(Resource, Default)]
struct ReposGlissade(f32);

pub struct ExpeditionPosturePlugin;

impl Plugin for ExpeditionPosturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PostureConfig::charger())
            .init_resource::<ReposGlissade>()
            .add_systems(OnExit(GameMode::Expedition), relever)
            .add_systems(
                Update,
                suivre_la_posture
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Expedition))
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(Update, capteur_posture.in_set(GameSet::Sensors));
    }
}

/// `forgia2_expedition_posture.json`, 1 Hz.
///
/// 🚨 Ce module a vécu une journée entière SANS aucune sortie, et ça a coûté
/// exactement ce que la règle annonce.
///
/// Le 2026-08-17 j'ai affirmé à l'utilisateur que s'accroupir faisait tomber la
/// vitesse à 45 %. C'était faux : `accroupi_facteur_vitesse` n'a **aucun
/// consommateur** — trois occurrences dans tout le dépôt, la déclaration, le
/// défaut, et un test qui valide la plage d'un gène que personne n'applique.
/// Le plugin était branché, la config data-driven, les tests verts, et rien de
/// tout ça ne pouvait montrer que le gène ne servait à rien.
///
/// D'où le choix des champs : ce capteur publie ce qui est **appliqué**, pas ce
/// qui est déclaré. `facteur_vitesse_demande` vient du génome ;
/// `facteur_vitesse_constate` se mesure sur le mouvement réel. Deux mesures
/// indépendantes de la même grandeur — c'est leur DÉSACCORD qui nomme le défaut,
/// et c'est le seul mécanisme qui ait attrapé quoi que ce soit cette session.
fn capteur_posture(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut vitesse_debout: Local<f32>,
    mode: Res<State<GameMode>>,
    cfg: Res<PostureConfig>,
    // 🚨 `Option` sur les DEUX ressources qui ne nous appartiennent pas.
    //
    // `Posture` et `PlayerLocomotion` sont fournies par `forgia-player`. Prises
    // en `Res<T>` nu, elles font PANIQUER le système quand la crate n'est pas
    // montée — et ça n'est visible ni à la compilation, ni au lint, ni dans 114
    // tests purs. C'est précisément ce qui vient d'arriver : la première version
    // de ce capteur, écrite le 2026-08-18, a fait tomber le harnais dans la
    // minute qui a suivi son ajout.
    //
    // La leçon est plus générale qu'un `Option` : **un capteur ne doit jamais
    // pouvoir tuer ce qu'il observe.** Il rapporte l'absence, il ne la subit pas.
    posture: Option<Res<Posture>>,
    loco: Option<Res<PlayerLocomotion>>,
    repos: Res<ReposGlissade>,
) {
    let en_expedition = *mode.get() == GameMode::Expedition;
    let (Some(posture), Some(loco)) = (posture, loco) else {
        *accum += time.delta_secs();
        if *accum >= 1.0 {
            *accum = 0.0;
            // Et on le DIT. Se taire ferait lire l'absence du fichier comme
            // « rien à signaler », alors qu'on ne sait simplement rien.
            let json = format!(
                r#"{{"id":"expedition_posture","severity":"{}","next_step":"POSTURE_INDISPONIBLE : forgia-player n'a pas publie Posture/PlayerLocomotion — aucune mesure possible. Ne pas lire ce capteur comme un feu vert","timestamp_secs":{:.1},"en_expedition":{en_expedition},"posture":"inconnue","facteur_vitesse_demande":{:.3},"facteur_vitesse_constate":-1.000,"facteur_mesure":false,"ecart_facteur":-1.000,"vitesse_ms":-1.00,"vitesse_debout_max_ms":-1.00,"glissade_repos_restant_s":-1.00,"glissade_duree_s":{:.2}}}"#,
                if en_expedition { "warn" } else { "info" },
                time.elapsed_secs(),
                cfg.accroupi_facteur_vitesse,
                cfg.glissade_duree_s,
            );
            let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_posture.json", json);
        }
        return;
    };
    // La référence : la plus grande vitesse vue DEBOUT. Sans elle, un facteur
    // constaté n'a aucun dénominateur et le capteur ne pourrait rien conclure.
    if en_expedition && !posture.accroupi && !posture.glisse {
        *vitesse_debout = vitesse_debout.max(loco.horizontal_speed);
    }
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let etat = if posture.glisse {
        "glissade"
    } else if posture.accroupi {
        "accroupi"
    } else {
        "debout"
    };
    // -1.0 = pas mesurable, JAMAIS 0.0 : un zéro se lirait comme « le
    // personnage est immobile », ce qui est une information, alors qu'ici on
    // n'en a aucune.
    let constate = if posture.accroupi && *vitesse_debout > 0.1 {
        loco.horizontal_speed / *vitesse_debout
    } else {
        -1.0
    };
    let mesure = constate >= 0.0;
    // Au-delà de cet écart, le gène déclaré et le mouvement observé ne parlent
    // plus de la même chose. Large : la vitesse instantanée oscille, et on ne
    // veut pas d'une alerte à chaque pas.
    const ECART_TOLERE: f32 = 0.15;
    let ecart = if mesure {
        (constate - cfg.accroupi_facteur_vitesse).abs()
    } else {
        -1.0
    };

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — aucune posture attendue")
    } else if mesure && ecart > ECART_TOLERE {
        (
            "warn",
            "GENE_NON_APPLIQUE : accroupi_facteur_vitesse est declare au genome mais le mouvement reel ne le suit pas. Verifier qui lit Posture.accroupi cote contrôleur (forgia-player) — un gene sans consommateur passe tous les tests et ne fait RIEN",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"expedition_posture","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"posture":"{etat}","facteur_vitesse_demande":{:.3},"facteur_vitesse_constate":{:.3},"facteur_mesure":{mesure},"ecart_facteur":{:.3},"vitesse_ms":{:.2},"vitesse_debout_max_ms":{:.2},"glissade_repos_restant_s":{:.2},"glissade_duree_s":{:.2}}}"#,
        time.elapsed_secs(),
        cfg.accroupi_facteur_vitesse,
        constate,
        ecart,
        loco.horizontal_speed,
        *vitesse_debout,
        repos.0,
        cfg.glissade_duree_s,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_posture.json", json);
}

/// Lit l'entrée et tient la posture à jour.
fn suivre_la_posture(
    time: Res<Time>,
    cfg: Res<PostureConfig>,
    loco: Res<PlayerLocomotion>,
    q: Query<&ActionState<PlayerAction>, With<Player>>,
    mut posture: ResMut<Posture>,
    mut repos: ResMut<ReposGlissade>,
) {
    let dt = time.delta_secs();
    repos.0 = (repos.0 - dt).max(0.0);

    let Ok(actions) = q.single() else {
        return;
    };
    let accroupi_demande = actions.pressed(&PlayerAction::Crouch);
    let court = actions.pressed(&PlayerAction::Sprint);
    let vitesse = loco.horizontal_speed;

    // ── La glissade en cours : elle s'écoule, personne ne l'interrompt ───────
    if posture.glisse {
        posture.glisse_restant_s -= dt;
        if posture.glisse_restant_s <= 0.0 {
            posture.glisse = false;
            posture.glisse_restant_s = 0.0;
            repos.0 = cfg.glissade_repos_s;
            // On finit ACCROUPI si la touche est encore tenue : c'est
            // l'enchaînement naturel (glisser derrière un couvert et y rester),
            // et le seul qui ne demande pas de relâcher puis re-appuyer.
            posture.accroupi = accroupi_demande;
        }
        return;
    }

    // ── Le déclenchement : courir + s'accroupir ─────────────────────────────
    if accroupi_demande
        && court
        && cfg.peut_glisser(vitesse, repos.0, posture.glisse)
    {
        posture.glisse = true;
        posture.glisse_restant_s = cfg.glissade_duree_s;
        posture.accroupi = false;
        return;
    }

    posture.accroupi = accroupi_demande;
}

/// Sortie du mode : on se relève.
///
/// Sans ça, quitter l'Expédition accroupi laisserait le Hall avec un personnage
/// baissé — et rien là-bas n'écrit cette ressource, donc personne ne le
/// relèverait jamais. Même motif que la bouche du canon et le champ de vision.
fn relever(mut posture: ResMut<Posture>, mut repos: ResMut<ReposGlissade>) {
    *posture = Posture::default();
    repos.0 = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_ne_glisse_pas_a_l_arret() {
        // La glissade CONVERTIT de la vitesse, elle n'en crée pas. Sans ce
        // seuil, s'accroupir à l'arrêt en tenant Shift déclencherait un geste
        // qui n'a aucun sens à l'écran.
        let c = PostureConfig::default();
        assert!(!c.peut_glisser(0.0, 0.0, false));
        assert!(!c.peut_glisser(2.0, 0.0, false), "marcher ne suffit pas");
        assert!(c.peut_glisser(8.0, 0.0, false), "courir suffit");
    }

    #[test]
    fn on_ne_peut_pas_enchainer_les_glissades() {
        // Sans délai, glisser en boucle traverse la carte plus vite que courir —
        // la glissade deviendrait le déplacement par défaut, et le sprint
        // décoratif.
        let c = PostureConfig::default();
        assert!(!c.peut_glisser(9.0, 0.5, false), "le repos doit bloquer");
        assert!(c.peut_glisser(9.0, 0.0, false));
    }

    #[test]
    fn une_glissade_en_cours_ne_se_redeclenche_pas() {
        let c = PostureConfig::default();
        assert!(!c.peut_glisser(9.0, 0.0, true));
    }

    #[test]
    fn le_repos_coute_plus_que_la_glissade_ne_rapporte() {
        // L'invariant qui empêche la glissade d'être gratuite : recommencer doit
        // prendre plus longtemps que le geste lui-même.
        let c = PostureConfig::default();
        assert!(
            c.glissade_repos_s > c.glissade_duree_s,
            "repos {} <= durée {} : enchaîner devient rentable",
            c.glissade_repos_s,
            c.glissade_duree_s
        );
    }

    #[test]
    fn accroupi_ralentit_sans_immobiliser() {
        // Trop lent, personne ne s'accroupit ; trop rapide, tout le monde reste
        // accroupi. Entre les deux, c'est un choix.
        let c = PostureConfig::default();
        assert!((0.2..0.7).contains(&c.accroupi_facteur_vitesse));
    }

    #[test]
    fn la_duree_de_glissade_tient_dans_son_clip() {
        // Le clip `slide` de Mixamo fait ~0,7 s. Une glissade plus longue le
        // ferait boucler : un personnage qui répète le même geste en glissant.
        let c = PostureConfig::default();
        assert!(c.glissade_duree_s > 0.3 && c.glissade_duree_s <= 1.0);
    }
}

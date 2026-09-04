//! contrat_mode.rs — Un mode rend-il ce qu'il a pris ?
//!
//! # Le défaut que ce fichier attrape, et il en a coûté quatre en une session
//!
//! Un mode de jeu pose des composants sur des entités **qu'il ne possède pas** :
//! la caméra, la fenêtre, le joueur. Ces entités SURVIVENT au mode. S'il oublie
//! d'en retirer un en sortant, il le lègue au mode suivant — qui n'a rien
//! demandé et n'a aucune raison de le chercher.
//!
//! Les quatre pannes du 2026-08-14→16, toutes de cette forme :
//!
//! | Ce qui a fuité | Symptôme | Où on l'a cherché |
//! |---|---|---|
//! | `Atmosphere` sur DEUX caméras | **panique wgpu, le jeu ne se lance plus** | le rendu, l'asset, le shader |
//! | `Hdr` (imposé par `Atmosphere`) | **écran noir** | le HDR, le tonemapping |
//! | Curseur verrouillé « en permanence » | **ESC ouvre la pause, souris morte** | l'input, la fenêtre |
//! | Brume et ambiante laissées sur les caméras | l'arène héritait de la nuit du Vallon | l'éclairage de l'arène |
//!
//! Aucune n'a été trouvée par un instrument : toutes ont été rapportées en jeu,
//! et deux ont coûté un tour de plus parce que le symptôme n'évoquait pas la
//! cause. **C'est un mode de panne, pas quatre bugs.**
//!
//! # Ce que ce module fait, et ce qu'il ne fait pas
//!
//! Il ne démonte rien — il **constate**. Chaque mode reste responsable de son
//! propre `OnExit` ; ce fichier vérifie qu'il l'a fait, et le dit fort quand
//! non. Un vérificateur qui corrigerait à la place du mode masquerait la fuite
//! au lieu de la nommer, et le mode continuerait de fuir.
//!
//! # Pourquoi la vérification est PÉRIODIQUE et pas sur `OnExit`
//!
//! 🚨 Un retrait passe par `Commands` : il est différé à la fin du stage. Un
//! contrôle posé sur `OnExit` verrait donc l'état d'AVANT le nettoyage et
//! crierait à chaque sortie de mode — le pire sort d'une alerte, parce qu'on
//! cesse de la lire. On vérifie donc **pendant** le mode suivant, quand plus
//! rien ne peut être en vol.

use bevy::camera::RenderTarget;
use forgia_core::constat;
use bevy::pbr::{Atmosphere, DistanceFog};
use bevy::prelude::*;
use bevy::render::view::Hdr;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use forgia_core::prelude::*;

/// Période de vérification. 2 s : assez lent pour ne rien coûter, assez rapide
/// pour que la fuite se voie dans la même session que la sortie de mode.
const PERIODE_S: f32 = 2.0;

/// Ce qu'un mode a le droit de laisser derrière lui : rien.
///
/// La liste est **explicite et compilée**, pas réflective. Un composant qu'on
/// oublie d'inscrire ici n'est pas surveillé — c'est le prix d'un contrôle
/// sans réflexion, et il est assumé : mieux vaut surveiller cinq composants
/// pour de vrai que quarante par introspection fragile.
///
/// Pour en ajouter un : une ligne dans la requête, une ligne dans `constater`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Fuites {
    pub atmosphere: usize,
    pub hdr: usize,
    pub brume: usize,
    pub ambiante: usize,
    /// Le curseur est-il resté pris alors qu'on n'est plus en jeu ?
    pub curseur_pris: bool,
}

impl Fuites {
    #[must_use]
    pub fn total(&self) -> usize {
        self.atmosphere + self.hdr + self.brume + self.ambiante + usize::from(self.curseur_pris)
    }

    #[must_use]
    pub fn est_propre(&self) -> bool {
        self.total() == 0
    }

    /// Le message, avec la cause en tête et le remède derrière.
    ///
    /// 🚨 Chaque ligne nomme le SYMPTÔME qu'on verra en jeu, pas seulement le
    /// composant. Un capteur qui dit « Atmosphere: 2 » laisse chercher ; un
    /// capteur qui dit « le jeu paniquera au premier rendu » envoie au bon
    /// endroit du premier coup.
    #[must_use]
    pub fn diagnostic(&self) -> String {
        let mut m = Vec::new();
        if self.atmosphere > 0 {
            m.push(format!(
                "{} camera(s) gardent Atmosphere — bevy exige `.single()`, donc a deux \
                 le tampon n'est jamais ecrit et wgpu PANIQUE au premier rendu",
                self.atmosphere
            ));
        }
        if self.hdr > 0 {
            m.push(format!(
                "{} camera(s) gardent Hdr — ecran noir sur les modes qui ne sont pas \
                 regles pour",
                self.hdr
            ));
        }
        if self.brume > 0 {
            m.push(format!(
                "{} camera(s) gardent DistanceFog — le mode suivant herite d'une brume \
                 qu'il n'a pas demandee",
                self.brume
            ));
        }
        if self.ambiante > 0 {
            m.push(format!(
                "{} camera(s) gardent AmbientLight — le mode suivant herite de la nuit \
                 du precedent",
                self.ambiante
            ));
        }
        if self.curseur_pris {
            m.push(
                "le curseur est PRIS hors du jeu — ESC ouvre la pause et la souris ne \
                 peut rien cliquer"
                    .to_string(),
            );
        }
        m.join(" · ")
    }
}

/// Ce qu'une caméra porte, et où elle rend. Structure plate pour que le
/// comptage soit une fonction pure — donc testable sans moteur.
#[derive(Clone, Copy, Debug, Default)]
pub struct EtatCamera {
    /// Rend-elle dans la FENÊTRE, ou dans une texture hors-écran ?
    pub vers_fenetre: bool,
    pub atmosphere: bool,
    pub hdr: bool,
    pub brume: bool,
    pub ambiante: bool,
}

/// Compte les fuites, en distinguant ce qui est **global** de ce qui est
/// **local à une cible de rendu**.
///
/// 🚨 C'est la correction du 2026-08-17, et elle est venue d'un faux positif
/// que CE capteur a produit à sa première sortie : il signalait « 1 caméra
/// garde DistanceFog · 1 caméra garde AmbientLight » au menu. Or le fond 3D du
/// menu (`arena_backdrop`) pose délibérément brume et ambiante sur **sa**
/// caméra, qui rend dans une TEXTURE. Elle ne lègue rien à personne.
///
/// La ligne de partage n'est donc pas « quel mode tourne » mais **où la caméra
/// rend** :
///
/// - `brume`, `ambiante`, `hdr` n'affectent que la cible de leur caméra → une
///   caméra hors-écran ne peut pas fuir.
/// - `Atmosphere` est GLOBALE : `write_atmosphere_buffer` fait `.single()`, donc
///   une deuxième caméra à atmosphère fait paniquer wgpu **même hors-écran**.
///   Elle se compte partout, sans exception.
#[must_use]
pub fn compter(cams: &[EtatCamera]) -> (usize, usize, usize, usize) {
    let mut atmosphere = 0;
    let (mut hdr, mut brume, mut ambiante) = (0, 0, 0);
    for c in cams {
        atmosphere += usize::from(c.atmosphere);
        if !c.vers_fenetre {
            continue;
        }
        hdr += usize::from(c.hdr);
        brume += usize::from(c.brume);
        ambiante += usize::from(c.ambiante);
    }
    (atmosphere, hdr, brume, ambiante)
}

/// La décision, **pure** : à partir de comptages, y a-t-il fuite ?
///
/// Séparée du monde pour être testable sans moteur — c'est ce qui permet de
/// vérifier que le contrôle MORD, au lieu d'espérer qu'il morde.
#[must_use]
pub fn constater(
    atmosphere: usize,
    hdr: usize,
    brume: usize,
    ambiante: usize,
    curseur_pris: bool,
    en_jeu: bool,
) -> Fuites {
    Fuites {
        atmosphere,
        hdr,
        brume,
        ambiante,
        // Le curseur pris EN JEU est normal — c'est la visée. Ce n'est une
        // fuite qu'une fois sorti.
        curseur_pris: curseur_pris && !en_jeu,
    }
}

#[allow(clippy::too_many_arguments)]
fn sys_verifier_contrat_mode(
    time: Res<Time>,
    mut accum: Local<f32>,
    app_mode: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    q_cam: Query<
        (
            // 🚨 En Bevy 0.18 `RenderTarget` est un COMPOSANT a part, pas un
            // champ de `Camera` — et son ABSENCE vaut « la fenetre ». Une
            // camera hors-ecran le porte explicitement (`RenderTarget::Image`).
            Option<&RenderTarget>,
            Has<Atmosphere>,
            Has<Hdr>,
            Has<DistanceFog>,
            Has<AmbientLight>,
        ),
        With<Camera3d>,
    >,
    q_fenetre: Query<&CursorOptions, With<PrimaryWindow>>,
) {
    *accum += time.delta_secs();
    if *accum < PERIODE_S {
        return;
    }
    *accum = 0.0;

    // Les composants d'ambiance sont LÉGITIMES pendant qu'un mode 3D tourne.
    // On ne vérifie donc que hors de ces modes — c'est là qu'ils deviennent
    // des restes.
    let mode_proprietaire = matches!(
        game_mode.get(),
        GameMode::Expedition | GameMode::CastleHub | GameMode::Rpg | GameMode::Roguelite
    );
    let en_jeu = matches!(app_mode.get(), AppMode::InGame);
    if mode_proprietaire && en_jeu {
        return;
    }

    let etats: Vec<EtatCamera> = q_cam
        .iter()
        .map(|(cible, a, h, b, am)| EtatCamera {
            vers_fenetre: matches!(cible, None | Some(RenderTarget::Window(_))),
            atmosphere: a,
            hdr: h,
            brume: b,
            ambiante: am,
        })
        .collect();
    let (atmosphere, hdr, brume, ambiante) = compter(&etats);
    let curseur_pris = q_fenetre
        .iter()
        .any(|c| !matches!(c.grab_mode, CursorGrabMode::None));

    let f = constater(atmosphere, hdr, brume, ambiante, curseur_pris, en_jeu);
    // 🚨 L'ÉCHANTILLON de ce capteur, ce sont les caméras rendant vers la
    // FENÊTRE — pas toutes les Camera3d. Brume, ambiante et Hdr n'affectent que
    // la cible de leur caméra ; une caméra hors-écran ne lègue rien. Au menu il
    // n'y en a aucune, donc ce contrôle n'a RIEN mesuré, et `Constat` le
    // dégrade tout seul en `info` au lieu de laisser passer un vert de vide.
    let a_l_ecran = etats.iter().filter(|c| c.vers_fenetre).count();
    let verdict = if f.atmosphere > 1 {
        // Le cas qui fait PANIQUER : deux caméras à atmosphère.
        constat::critique(f.diagnostic())
            .remede("elire UNE camera porteuse — `write_atmosphere_buffer` fait `.single()`")
    } else if !f.est_propre() {
        constat::alerte(f.diagnostic()).remede(
            "retirer le composant a la SORTIE du mode qui l'a pose (OnExit + systeme \
             periodique : les retraits par Commands sont differes)",
        )
    } else {
        constat::ok()
    };

    verdict.echantillon(a_l_ecran).publier(
        "contrat_mode",
        time.elapsed_secs(),
        &format!(
            r#""app_mode":"{:?}","game_mode":"{:?}","cameras":{},"cameras_hors_ecran":{},"atmosphere":{},"hdr":{},"brume":{},"ambiante":{},"curseur_pris":{}"#,
            app_mode.get(),
            game_mode.get(),
            etats.len(),
            // Publié EXPRÈS : ces caméras sont exemptées de brume/ambiante/hdr.
            // Un capteur qui restreint son échantillon sans le dire fait passer
            // une exemption pour une absence de défaut.
            etats.len() - a_l_ecran,
            f.atmosphere,
            f.hdr,
            f.brume,
            f.ambiante,
            f.curseur_pris,
        ),
    );
}

pub struct ContratDeModePlugin;

impl Plugin for ContratDeModePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            sys_verifier_contrat_mode.in_set(GameSet::Sensors),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hors_ecran() -> EtatCamera {
        EtatCamera {
            vers_fenetre: false,
            brume: true,
            ambiante: true,
            ..default()
        }
    }

    /// 🚨 LE faux positif que ce capteur a produit à sa toute première sortie,
    /// le 2026-08-17 : « 1 camera garde DistanceFog · 1 camera garde
    /// AmbientLight » au menu. C'était le fond 3D du menu, qui pose brume et
    /// ambiante sur SA caméra — laquelle rend dans une texture.
    ///
    /// Un instrument qui vise à côté ne rate pas un défaut : il en **fabrique**
    /// un, et on part le corriger.
    #[test]
    fn une_camera_hors_ecran_ne_fuit_ni_brume_ni_ambiante() {
        let (_, _, brume, ambiante) = compter(&[hors_ecran()]);
        assert_eq!(
            (brume, ambiante),
            (0, 0),
            "le fond 3D du menu rend dans une texture : il ne lègue rien"
        );
        assert!(constater(0, 0, brume, ambiante, false, false).est_propre());
    }

    /// 🚨 Le pendant du faux positif, constaté le lendemain : une fois
    /// l'exemption en place, le capteur rendait `ok` au menu alors que son
    /// contrôle brume/ambiante n'avait RIEN mesuré — les 3 Camera3d presentes
    /// sont toutes hors-ecran. Un vert d'echantillon vide se lit comme un vert
    /// de mesure. Le capteur dit desormais `info` + « aveugle ».
    #[test]
    fn aucune_camera_a_l_ecran_n_est_pas_un_feu_vert() {
        let cams = [hors_ecran(), hors_ecran(), hors_ecran()];
        let a_l_ecran = cams.iter().filter(|c| c.vers_fenetre).count();
        let (_, _, brume, ambiante) = compter(&cams);
        assert_eq!(a_l_ecran, 0);
        assert!(
            constater(0, 0, brume, ambiante, false, false).est_propre(),
            "rien ne fuit, mais rien n'a ete mesure non plus"
        );
        // C'est cette conjonction — propre ET echantillon vide — que le systeme
        // doit rendre en `info`, jamais en `ok`.
    }

    /// L'exemption ne doit pas devenir une porte ouverte : la MÊME ambiance sur
    /// une caméra de fenêtre reste une fuite.
    #[test]
    fn la_meme_ambiance_a_l_ecran_reste_une_fuite() {
        let a_l_ecran = EtatCamera {
            vers_fenetre: true,
            ..hors_ecran()
        };
        let (_, _, brume, ambiante) = compter(&[a_l_ecran]);
        assert_eq!((brume, ambiante), (1, 1));
    }

    /// 🚨 `Atmosphere` ne s'exempte JAMAIS : `write_atmosphere_buffer` fait
    /// `.single()`, donc une deuxième caméra à atmosphère fait paniquer wgpu
    /// même quand elle rend hors-écran. Le partage global/local est le cœur de
    /// la correction — s'il s'inverse, on ré-ouvre le crash du 14/08.
    #[test]
    fn l_atmosphere_se_compte_meme_hors_ecran() {
        let cams = [
            EtatCamera {
                vers_fenetre: true,
                atmosphere: true,
                ..default()
            },
            EtatCamera {
                vers_fenetre: false,
                atmosphere: true,
                ..default()
            },
        ];
        let (atmosphere, ..) = compter(&cams);
        assert_eq!(atmosphere, 2, "hors-ecran ou non, elle casse le `.single()`");
    }

    /// Le cas qui a empêché le jeu de se lancer le 2026-08-14.
    #[test]
    fn deux_atmospheres_est_le_cas_qui_fait_paniquer() {
        let f = constater(2, 2, 0, 0, false, false);
        assert!(!f.est_propre());
        assert!(
            f.diagnostic().contains("PANIQUE"),
            "le message doit nommer la panique, pas juste compter : {}",
            f.diagnostic()
        );
    }

    /// 🚨 Le curseur pris EN JEU est la visée, pas une fuite. Un contrôle qui
    /// ne ferait pas cette distinction crierait pendant toute la partie, et on
    /// cesserait de le lire — ce qui le rendrait pire qu'absent.
    #[test]
    fn le_curseur_pris_en_jeu_n_est_pas_une_fuite() {
        assert!(!constater(0, 0, 0, 0, true, true).curseur_pris);
        assert!(constater(0, 0, 0, 0, true, false).curseur_pris);
    }

    /// La sortie propre doit être propre — sinon l'alerte devient du bruit.
    #[test]
    fn un_demontage_complet_ne_declenche_rien() {
        let f = constater(0, 0, 0, 0, false, false);
        assert!(f.est_propre());
        assert_eq!(f.total(), 0);
        assert!(f.diagnostic().is_empty());
    }

    /// Chaque fuite doit être NOMMÉE séparément : les confondre ferait chercher
    /// au mauvais endroit, ce qui est exactement ce qui a coûté les quatre
    /// pannes d'origine.
    #[test]
    fn chaque_fuite_a_son_propre_message() {
        let msgs = [
            constater(1, 0, 0, 0, false, false).diagnostic(),
            constater(0, 1, 0, 0, false, false).diagnostic(),
            constater(0, 0, 1, 0, false, false).diagnostic(),
            constater(0, 0, 0, 1, false, false).diagnostic(),
            constater(0, 0, 0, 0, true, false).diagnostic(),
        ];
        for (i, a) in msgs.iter().enumerate() {
            assert!(!a.is_empty(), "fuite {i} sans message");
            for (j, b) in msgs.iter().enumerate() {
                assert!(i == j || a != b, "fuites {i} et {j} rendent le meme message");
            }
        }
    }

    /// Le total additionne tout, y compris le curseur — sinon une fuite unique
    /// de curseur passerait pour un état propre.
    #[test]
    fn le_curseur_compte_dans_le_total() {
        assert_eq!(constater(1, 1, 1, 1, true, false).total(), 5);
    }
}

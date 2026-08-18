//! cape — confier les six os de cape au solveur de mouvement secondaire.
//!
//! # Ce que ce module répare
//!
//! Le corps d'Expédition porte une cape (`Blaha2.001`) attachée à sa propre
//! peau, elle-même pilotée par six os `cloak_01`..`cloak_06` en chaîne sous
//! `root.001`, lui-même sous `Spine2`.
//!
//! **Aucun des 34 clips du corps n'anime ces six os.** La cape suivait donc le
//! buste comme une planche : elle tournait avec le torse, sans jamais retomber,
//! flotter, ni réagir à la course. Mesuré le 2026-08-18 par le contrôle du corps
//! livré, qui liste les os qu'aucun clip ne touche.
//!
//! # Pourquoi un solveur et pas un clip
//!
//! Une cape ne s'anime pas à la main : son mouvement dépend de ce que le
//! personnage vient de faire, pas d'une chorégraphie. Un clip « cape qui vole »
//! serait faux dès qu'on s'arrête. `forgia-secondary-motion` intègre déjà des
//! os-ressorts en Verlet — c'est exactement l'outil, et il n'avait aucun
//! consommateur actif.
//!
//! # Le piège déjà payé, deux fois
//!
//! Son solveur supposait l'axe long des os à `+Y`. L'audit d'animation du
//! 2026-06-04 a débranché la queue de Rex pour cette raison (« whip Verlet,
//! queue de travers ») en laissant la consigne de corriger. La chaîne de cape
//! court selon **−X** : la brancher sans corriger l'aurait tordue pareil. L'axe
//! se lit maintenant sur la pose de liaison, côté solveur.

use bevy::prelude::*;
use forgia_core::prelude::{AppMode, GameMode, GameSet};
use forgia_secondary_motion::{SpringBone, SpringBoneChain};
use serde::Deserialize;

/// Couche **definition** : la souplesse d'une cape se juge à l'œil, en jeu.
const GENOME: &str = "assets/genomes/expedition_cape.toml";

/// L'os parent de la chaîne. Il porte l'échelle 0,01 du rig de cape — ce qui ne
/// gêne pas le solveur, qui n'écrit que des rotations.
const OS_RACINE: &str = "root.001";
/// Le préfixe des os de la chaîne.
const PREFIXE: &str = "cloak_";

#[derive(Resource, Debug, Clone, Copy, Deserialize)]
pub struct CapeConfig {
    /// 0 = molle (tombe), 1 = rigide (colle au buste).
    #[serde(default = "raideur_par_defaut")]
    pub raideur: f32,
    /// 0 = oscille longtemps, 1 = figée.
    #[serde(default = "amorti_par_defaut")]
    pub amorti: f32,
    /// La pesanteur vue par le tissu. Plus faible que 9,81 : une cape est portée
    /// par l'air autant qu'elle est tirée vers le bas, et la vraie pesanteur
    /// donne un rideau, pas un vêtement.
    #[serde(default = "pesanteur_par_defaut")]
    pub pesanteur: f32,
}

fn raideur_par_defaut() -> f32 {
    0.35
}
fn amorti_par_defaut() -> f32 {
    0.7
}
fn pesanteur_par_defaut() -> f32 {
    -4.0
}

impl Default for CapeConfig {
    fn default() -> Self {
        Self {
            raideur: raideur_par_defaut(),
            amorti: amorti_par_defaut(),
            pesanteur: pesanteur_par_defaut(),
        }
    }
}

impl CapeConfig {
    #[must_use]
    pub fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                warn!("[cape] {GENOME} mal formé ({e}) — réglages par défaut");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// Ce que l'accrochage a trouvé. Publié par le capteur.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct EtatCape {
    pub racine_trouvee: bool,
    pub os_de_chaine: usize,
    pub accrochee: bool,
}

pub struct ExpeditionCapePlugin;

impl Plugin for ExpeditionCapePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CapeConfig::charger())
            .init_resource::<EtatCape>()
            .add_systems(
                Update,
                accrocher_la_cape
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Expedition))
                    .run_if(in_state(AppMode::InGame)),
            )
            .add_systems(Update, capteur_cape.in_set(GameSet::Sensors));
    }
}

/// Trouve `root.001` et ses `cloak_*`, et pose la chaîne d'os-ressorts.
///
/// Tourne à chaque frame mais sort en une comparaison de booléen une fois la
/// cape accrochée : le corps arrive en tâche de fond, on ne sait pas à quelle
/// frame, et un `OnEnter` le manquerait.
fn accrocher_la_cape(
    mut commands: Commands,
    cfg: Res<CapeConfig>,
    mut etat: ResMut<EtatCape>,
    q_noms: Query<(Entity, &Name)>,
    q_enfants: Query<&Children>,
) {
    if etat.accrochee {
        return;
    }
    let Some(racine) = q_noms
        .iter()
        .find(|(_, n)| n.as_str() == OS_RACINE)
        .map(|(e, _)| e)
    else {
        return;
    };
    etat.racine_trouvee = true;

    // La chaîne se suit par la HIÉRARCHIE, pas par le numéro du nom. Un rig peut
    // renuméroter ses os ; il ne peut pas mentir sur qui est l'enfant de qui.
    let mut chaine = Vec::new();
    let mut courant = racine;
    while let Ok(enfants) = q_enfants.get(courant) {
        let Some(suivant) = enfants.iter().find(|&e| {
            q_noms
                .get(e)
                .map(|(_, n)| n.as_str().starts_with(PREFIXE))
                .unwrap_or(false)
        }) else {
            break;
        };
        chaine.push(suivant);
        courant = suivant;
    }
    etat.os_de_chaine = chaine.len();
    // Deux os au minimum : une chaîne d'un seul n'a aucune direction à suivre.
    if chaine.len() < 2 {
        return;
    }

    for &os in &chaine {
        commands.entity(os).insert(SpringBone {
            stiffness: cfg.raideur,
            damping: cfg.amorti,
            gravity: Vec3::new(0.0, cfg.pesanteur, 0.0),
        });
    }
    commands.entity(racine).insert(SpringBoneChain {
        bones: chaine.clone(),
        ..default()
    });
    etat.accrochee = true;
    info!(
        "[cape] {} os accrochés au solveur de mouvement secondaire",
        chaine.len()
    );
}

/// `forgia2_expedition_cape.json`, 1 Hz.
fn capteur_cape(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut dernier_vu: Local<bool>,
    mode: Res<State<GameMode>>,
    cfg: Res<CapeConfig>,
    etat: Res<EtatCape>,
    q_chaines: Query<&SpringBoneChain>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let en_expedition = *mode.get() == GameMode::Expedition;
    // `initialized` est posé par le solveur à son premier passage : c'est la
    // preuve qu'il a VU la chaîne, pas seulement qu'on la lui a donnée.
    let vu_maintenant = q_chaines
        .iter()
        .any(|c| c.initialized && !c.bones.is_empty());

    // 🚨 NE PAS EFFACER LA DERNIERE MESURE EN SORTANT DU MODE.
    //
    // Le 2026-08-18, TROIS parties de suite ont été jouées, observées, et leur
    // mesure perdue : à la sortie d'Expédition le capteur se réécrivait en
    // « hors Expedition » avec des sentinelles, et la seule trace de ce qui
    // s'était passé disparaissait. L'utilisateur décrivait un symptôme que le
    // dispositif venait d'effacer.
    //
    // Un capteur mesure un état FUGACE : le mode dure quelques minutes, la
    // lecture se fait après. Retenir la dernière valeur réelle ne coûte rien et
    // c'est la seule façon qu'un capteur de mode serve à quelque chose.
    if en_expedition {
        *dernier_vu = vu_maintenant;
    }
    let solveur_a_pris = if en_expedition { vu_maintenant } else { *dernier_vu };

    let (severity, next_step) = if !en_expedition {
        ("info", "hors Expedition — valeurs retenues de la derniere session dans le mode")
    } else if !etat.racine_trouvee {
        (
            "warn",
            "CAPE_SANS_RACINE : aucun os « root.001 » dans la scene — le corps charge n'a pas de cape, ou son rig a ete renomme. Verifier expedition_body.model au genome d'equipement",
        )
    } else if etat.os_de_chaine < 2 {
        (
            "warn",
            "CHAINE_TROP_COURTE : « root.001 » trouve mais moins de deux os « cloak_* » dessous — la cape restera rigide. Le rig d'origine en compte six",
        )
    } else if !solveur_a_pris {
        (
            "warn",
            "SOLVEUR_INACTIF : la chaine est posee mais forgia-secondary-motion ne l'a pas initialisee — verifier que ForgiaSecondaryMotionPlugin est bien ajoute (forgia-game/src/lib.rs)",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"expedition_cape","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"en_expedition":{en_expedition},"racine_trouvee":{},"os_de_chaine":{},"accrochee":{},"solveur_a_pris":{solveur_a_pris},"raideur":{:.2},"amorti":{:.2},"pesanteur":{:.2}}}"#,
        time.elapsed_secs(),
        etat.racine_trouvee,
        etat.os_de_chaine,
        etat.accrochee,
        cfg.raideur,
        cfg.amorti,
        cfg.pesanteur,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_expedition_cape.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les trois réglages restent dans des bornes qui produisent une cape, pas
    /// un drapeau rigide ni un chiffon qui traverse le sol.
    #[test]
    fn les_reglages_par_defaut_font_une_cape() {
        let c = CapeConfig::default();
        assert!((0.1..0.6).contains(&c.raideur), "raideur {}", c.raideur);
        assert!((0.5..0.9).contains(&c.amorti), "amorti {}", c.amorti);
        // Plus faible que la pesanteur réelle : une cape est portée par l'air.
        assert!(c.pesanteur < 0.0 && c.pesanteur > -9.81);
    }

    /// Le corps LIVRÉ porte bien la chaîne que ce module cherche. Sans ce test,
    /// un renommage du rig rendrait le module muet sans que rien n'échoue — et
    /// c'est exactement ainsi que la cape est restée rigide sans qu'on le voie.
    #[test]
    fn le_corps_livre_porte_la_chaine_de_cape() {
        let glb = std::path::Path::new(
            "../../assets/models/characters/stylized/stylized_male_fusil.glb",
        );
        if !glb.exists() {
            return; // corps absent (CI sans assets) — le capteur couvre ce cas
        }
        let d = std::fs::read(glb).expect("corps lisible");
        let lg = u32::from_le_bytes([d[12], d[13], d[14], d[15]]) as usize;
        let json = String::from_utf8_lossy(&d[20..20 + lg]);
        assert!(
            json.contains(OS_RACINE),
            "« {OS_RACINE} » absent du corps livré"
        );
        let os = (1..=6)
            .filter(|i| json.contains(&format!("{PREFIXE}0{i}")))
            .count();
        assert!(
            os >= 2,
            "seulement {os} os « {PREFIXE}* » dans le corps livré"
        );
    }
}

//! Les braseros du chemin, qui s'allument à mesure que la nuit tombe.
//!
//! # Pourquoi ce module existait en creux
//!
//! `LampeDef` était déclarée dans le manifeste et 16 braseros étaient bâtis
//! dans le GLB — mais **aucun consommateur** ne les lisait. Un décor éclairé
//! par rien : les pièces se voyaient, la nuit restait noire. Un artefact ne se
//! prouve que par son consommateur, jamais par sa déclaration.
//!
//! # Les deux entrées, et pourquoi il en faut DEUX
//!
//! | entrée | source | ce qu'elle décide |
//! |---|---|---|
//! | `avancee` | manifeste Blender, par brasero | **lequel** s'allume |
//! | `soleil_elevation_deg` | [`crate::cycle`], dérivé de la progression | **s'il fait assez sombre** |
//!
//! Avec la seule avancée, les braseros s'allumeraient en plein jour dès qu'on
//! les dépasse. Avec la seule obscurité, les 16 s'allumeraient d'un coup à la
//! tombée du soir, y compris ceux qu'on n'a pas encore atteints. Le produit des
//! deux donne ce qui était demandé : **une file de feux qui s'allume devant
//! soi, à mesure que le jour baisse**.
//!
//! # Ordre de câblage — BLOQUANT
//!
//! [`update_lampes`] lit `CycleState`, que `lighting::update_expedition_cycle`
//! écrit. Il doit donc passer **après** lui dans la chaîne d'`Update`. S'il
//! passe avant, il travaille sur l'état de la frame précédente : invisible à
//! l'œil, mais c'est une dépendance qu'on ne veut pas laisser implicite.

use bevy::prelude::*;
use serde::Deserialize;

use crate::lighting::CycleState;
use crate::manifest::blender_to_bevy;
use crate::plugin::ActiveExpedition;

/// Même génome que les feux du personnage : les deux se règlent ensemble, de
/// nuit, et séparer leurs fichiers obligerait à ouvrir deux onglets pour juger
/// une seule ambiance.
const GENOME_VFX: &str = "assets/genomes/expedition_vfx.toml";

/// Un brasero allumable, et son avancée sur le chemin.
#[derive(Component)]
pub struct Brasero {
    /// Position `[0, 1]` de ce brasero le long du chemin.
    avancee: f32,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct LampesConfig {
    pub intensite: f32,
    pub fondu_par_s: f32,
    pub marge_allumage: f32,
    pub elevation_extinction_deg: f32,
}

impl Default for LampesConfig {
    fn default() -> Self {
        Self {
            intensite: 170_000.0,
            fondu_par_s: 2.0,
            marge_allumage: 0.08,
            elevation_extinction_deg: 14.0,
        }
    }
}

#[derive(Deserialize)]
struct VfxToml {
    #[serde(default)]
    lampes: Option<LampesToml>,
}

#[derive(Deserialize)]
struct LampesToml {
    intensite: f32,
    fondu_par_s: f32,
    marge_allumage: f32,
    elevation_extinction_deg: f32,
}

impl LampesConfig {
    #[must_use]
    pub fn charger() -> Self {
        match forgia_core::def_io::read_def_str(GENOME_VFX) {
            Ok(s) => Self::depuis_toml(&s),
            Err(e) => {
                warn!("[expedition-lampes] {GENOME_VFX} illisible ({e}) — défauts");
                Self::default()
            }
        }
    }

    fn depuis_toml(contenu: &str) -> Self {
        let Ok(t) = toml::from_str::<VfxToml>(contenu) else {
            warn!("[expedition-lampes] {GENOME_VFX} mal formé — défauts");
            return Self::default();
        };
        match t.lampes {
            Some(l) => Self {
                intensite: l.intensite,
                fondu_par_s: l.fondu_par_s,
                marge_allumage: l.marge_allumage.max(1e-3),
                elevation_extinction_deg: l.elevation_extinction_deg.max(1e-3),
            },
            None => Self::default(),
        }
    }

    /// Montée d'un brasero `[0, 1]` : il s'allume quand le joueur entre dans sa
    /// marge d'approche, et reste allumé une fois dépassé. **Pure.**
    #[must_use]
    pub fn approche(&self, progression: f32, avancee: f32) -> f32 {
        ((progression - (avancee - self.marge_allumage)) / self.marge_allumage).clamp(0.0, 1.0)
    }

    /// Part d'obscurité `[0, 1]` déduite de la hauteur du soleil. **Pure.**
    #[must_use]
    pub fn obscurite(&self, soleil_elevation_deg: f32) -> f32 {
        ((self.elevation_extinction_deg - soleil_elevation_deg) / self.elevation_extinction_deg)
            .clamp(0.0, 1.0)
    }
}

/// Pose une lumière par brasero, à la hauteur de sa FLAMME.
///
/// La flamme est à ~2,7 m au-dessus du pied de la pièce : une lumière posée à
/// l'origine de l'objet éclairerait le sol sous le brasero, pas le chemin.
/// C'est pour ça que le manifeste porte `flamme_xyz` en plus de `xyz`.
pub fn setup_lampes(
    mut commands: Commands,
    active: Option<Res<ActiveExpedition>>,
    config: Option<Res<LampesConfig>>,
) {
    let Some(active) = active else {
        warn!("[expedition-lampes] carte absente — aucun brasero");
        return;
    };
    let cfg = config.map(|c| *c).unwrap_or_else(LampesConfig::charger);
    commands.insert_resource(cfg);

    let mut poses = 0;
    for lampe in &active.gameplay.lampes {
        // `blender_to_bevy` est LA conversion de repère du projet. Le manifeste
        // est en `blender_z_up` ; l'appliquer ici et nulle part ailleurs est ce
        // qui évite qu'un brasero parte de côté sans que rien ne le signale.
        let position = blender_to_bevy(lampe.flamme_xyz);
        commands.spawn((
            PointLight {
                // Teinte de flamme, plus chaude que la lueur du personnage :
                // un brasero de chemin est un feu de bois, pas une braise.
                color: Color::srgb(1.0, 0.62, 0.28),
                intensity: 0.0, // montée par `update_lampes`
                range: lampe.portee_m,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(position),
            Brasero {
                avancee: lampe.avancee,
            },
            Name::new(format!("brasero_{poses}")),
        ));
        poses += 1;
    }
    if poses == 0 {
        // Zéro mesuré n'est pas vert : un chemin sans brasero est une carte
        // qu'on traversera dans le noir, et ça doit se voir dans le log.
        warn!("[expedition-lampes] 0 brasero dans le manifeste — chemin non éclairé");
    } else {
        info!("[expedition-lampes] {poses} brasero(s) posé(s)");
    }
}

/// Fait monter chaque brasero à mesure que la nuit le rattrape.
///
/// À câbler **après** `lighting::update_expedition_cycle` — cf. l'en-tête.
pub fn update_lampes(
    temps: Res<Time>,
    state: Option<Res<CycleState>>,
    config: Option<Res<LampesConfig>>,
    mut q: Query<(&mut PointLight, &Brasero)>,
) {
    let (Some(state), Some(cfg)) = (state, config) else {
        return;
    };
    let dt = temps.delta_secs().max(1e-4);
    let obscurite = cfg.obscurite(state.etat.soleil_elevation_deg);
    for (mut lumiere, brasero) in &mut q {
        let cible =
            cfg.intensite * cfg.approche(state.etat.progression, brasero.avancee) * obscurite;
        // Fondu : un brasero qui s'allume d'un coup en passant un seuil se lit
        // comme un défaut d'affichage, pas comme un feu qu'on approche.
        lumiere.intensity += (cible - lumiere.intensity) * (dt * cfg.fondu_par_s).min(1.0);
    }
}

/// Retire les braseros à la sortie. Sans ça, l'Expédition laisse seize points
/// lumineux dans l'arène et le Hall — même piège que l'ambiante et le
/// brouillard, qui sont des composants de caméra.
pub fn teardown_lampes(mut commands: Commands, q: Query<Entity, With<Brasero>>) {
    let mut n = 0;
    for e in &q {
        commands.entity(e).despawn();
        n += 1;
    }
    commands.remove_resource::<LampesConfig>();
    info!("[expedition-lampes] {n} brasero(s) retiré(s)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_genome_livre_se_lit() {
        let toml = include_str!("../../../assets/genomes/expedition_vfx.toml");
        let cfg = LampesConfig::depuis_toml(toml);
        assert!(cfg.intensite > 0.0, "des braseros à 0 lumen n'éclairent rien");
        assert!(cfg.marge_allumage > 0.0, "une marge nulle divise par zéro");
    }

    /// L'invariant demandé : un brasero s'allume **quand on l'approche**, pas
    /// avant, et il reste allumé une fois dépassé.
    #[test]
    fn un_brasero_s_allume_a_l_approche() {
        let cfg = LampesConfig::default();
        let avancee = 0.5;
        assert_eq!(cfg.approche(0.0, avancee), 0.0, "au départ, éteint");
        assert_eq!(
            cfg.approche(avancee - cfg.marge_allumage, avancee),
            0.0,
            "au bord de la marge, pas encore"
        );
        assert!(
            cfg.approche(avancee - cfg.marge_allumage * 0.5, avancee) > 0.0,
            "dans la marge, il monte"
        );
        // Comparaison approchée : `(0.5 - 0.42) / 0.08` rend 0,9999998 en f32.
        // Exiger l'égalité stricte ferait échouer un calcul pourtant juste.
        assert!(
            (cfg.approche(avancee, avancee) - 1.0).abs() < 1e-5,
            "à hauteur, plein"
        );
        assert_eq!(cfg.approche(1.0, avancee), 1.0, "dépassé, reste allumé");
    }

    /// L'autre moitié de la règle : en plein jour, un brasero dépassé reste
    /// éteint. Sans ce facteur, les feux s'allumeraient sous le soleil.
    #[test]
    fn en_plein_jour_les_braseros_restent_eteints() {
        let cfg = LampesConfig::default();
        assert_eq!(cfg.obscurite(45.0), 0.0, "soleil haut => rien");
        assert_eq!(
            cfg.obscurite(cfg.elevation_extinction_deg),
            0.0,
            "au seuil, rien encore"
        );
        assert!(cfg.obscurite(0.0) > 0.0, "soleil à l'horizon => ça monte");
        assert_eq!(cfg.obscurite(-10.0), 1.0, "sous l'horizon => plein");
    }

    /// Le produit des deux : c'est lui qui donne « une file de feux qui
    /// s'allume devant soi à mesure que le jour baisse ».
    #[test]
    fn le_produit_des_deux_entrees_fait_la_file_de_feux() {
        let cfg = LampesConfig::default();
        // Un brasero lointain, de nuit : pas encore atteint.
        assert_eq!(cfg.approche(0.2, 0.9) * cfg.obscurite(-5.0), 0.0);
        // Le même, une fois rejoint : allumé.
        assert!(cfg.approche(0.9, 0.9) * cfg.obscurite(-5.0) > 0.0);
        // Un brasero rejoint, mais en plein jour : éteint.
        assert_eq!(cfg.approche(0.9, 0.9) * cfg.obscurite(30.0), 0.0);
    }
}

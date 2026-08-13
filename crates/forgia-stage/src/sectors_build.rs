//! sectors_build.rs — l'arène en parts, bâtie (story-703 incrément 2).
//!
//! Prend la géométrie pure de [`forgia_core::sectors`] et en fait des colliders,
//! des solides déclarés et un capteur. **Rien n'est calculé ici** : ce fichier
//! traduit, il ne décide pas. C'est ce qui garde la géométrie testable sans
//! moteur, et c'est délibéré — les sept défauts du chantier navmesh du
//! 2026-08-13 vivaient tous dans du code qu'aucun test ne pouvait interroger.
//!
//! # La hauteur déclarée EST la hauteur physique
//!
//! Chaque mur publie sa `SolidSeg` et son collider **depuis la même variable**.
//! C'est le prérequis P2 de la story, pris à la source plutôt que contourné : le
//! défaut mesuré en jeu — une paroi de 0,60 m que le maillage croyait
//! franchissable parce qu'elle se déclarait sous 0,45 — vient précisément d'une
//! hauteur déclarée ailleurs que là où le collider est créé.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::sectors::{SectorLayout, WallSeg};
use serde::Deserialize;

const GENOME_PATH: &str = "assets/genomes/arena_sectors.toml";

/// Réglages lus depuis [`GENOME_PATH`]. Aucune valeur numérique ne vit dans ce
/// fichier — les défauts ci-dessous sont le reflet du TOML, pas une source
/// concurrente : quand les deux divergent, c'est le TOML qui gagne.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct SectorsConfig {
    pub enabled: bool,
    pub count: u32,
    pub atrium_radius_m: f32,
    pub door_agent_radius_m: f32,
    pub chord_deg: f32,
    pub wall_height_m: f32,
    pub wall_thickness_m: f32,
    pub aggro_sector_spill_frac: f32,
}

impl Default for SectorsConfig {
    fn default() -> Self {
        Self {
            // Coupé le 2026-08-13 : les murs ont été livrés avant ce qui les rend
            // jouables. Voir le génome pour le détail mesuré — 18 s de survie.
            enabled: false,
            count: 3,
            atrium_radius_m: 20.0,
            door_agent_radius_m: 1.40,
            chord_deg: 5.0,
            wall_height_m: 4.0,
            wall_thickness_m: 0.4,
            aggro_sector_spill_frac: 0.25,
        }
    }
}

#[derive(Deserialize)]
struct SectorsToml {
    sectors: SectorsBlock,
}

#[derive(Deserialize)]
struct SectorsBlock {
    enabled: Option<bool>,
    count: Option<u32>,
    atrium_radius_m: Option<f32>,
    door_agent_radius_m: Option<f32>,
    chord_deg: Option<f32>,
    wall_height_m: Option<f32>,
    wall_thickness_m: Option<f32>,
    aggro_sector_spill_frac: Option<f32>,
}

impl SectorsConfig {
    /// Lit le génome. **`def_io`, pas `std::fs`** : sur wasm un `std::fs` échoue
    /// en silence et le jeu tourne avec les défauts sans qu'aucune erreur ne le
    /// dise — c'est le mur n°9 du portage web, déjà payé une fois.
    #[must_use]
    pub fn load_or_default() -> Self {
        match forgia_core::def_io::read_def_str(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(_) => Self::default(),
        }
    }

    #[must_use]
    pub fn parse_toml(content: &str) -> Self {
        let Ok(t) = toml::from_str::<SectorsToml>(content) else {
            return Self::default();
        };
        let d = Self::default();
        let s = t.sectors;
        Self {
            enabled: s.enabled.unwrap_or(d.enabled),
            // ≥ 2 : une seule part n'a ni frontière ni voisine, donc ni cloison
            // ni poche sûre. Le concept n'existe pas sous 2.
            count: s.count.unwrap_or(d.count).max(2),
            atrium_radius_m: s.atrium_radius_m.unwrap_or(d.atrium_radius_m).max(1.0),
            door_agent_radius_m: s
                .door_agent_radius_m
                .unwrap_or(d.door_agent_radius_m)
                .clamp(0.1, 5.0),
            // Sous 1° l'anneau coûterait 360 colliders pour un gain invisible ;
            // au-delà de 30° ce n'est plus un anneau, c'est un polygone grossier
            // dont les portes ne veulent plus rien dire.
            chord_deg: s.chord_deg.unwrap_or(d.chord_deg).clamp(1.0, 30.0),
            // Doit dépasser l'œil du joueur (1,70 m), sinon la cloison ne casse
            // aucune ligne de vue et toute la mécanique d'évitement tombe.
            wall_height_m: s.wall_height_m.unwrap_or(d.wall_height_m).max(1.8),
            wall_thickness_m: s.wall_thickness_m.unwrap_or(d.wall_thickness_m).max(0.1),
            aggro_sector_spill_frac: s
                .aggro_sector_spill_frac
                .unwrap_or(d.aggro_sector_spill_frac)
                .clamp(0.0, 0.5),
        }
    }

    /// La disposition géométrique correspondante.
    #[must_use]
    pub fn layout(&self, outer_radius_m: f32) -> SectorLayout {
        SectorLayout {
            count: self.count,
            atrium_radius_m: self.atrium_radius_m,
            outer_radius_m,
            door_width_m: SectorLayout::door_width_for(self.door_agent_radius_m),
        }
    }

    #[must_use]
    pub fn chord_rad(&self) -> f32 {
        self.chord_deg.to_radians()
    }
}

/// Ce qui a été bâti — publié pour le capteur et pour les consommateurs à venir
/// (postes des packs, ouverture des portes, aggro angulaire).
#[derive(Resource, Debug, Clone, Default)]
pub struct BuiltSectors {
    pub layout: Option<SectorLayout>,
    pub partitions: u32,
    pub atrium_segments: u32,
    /// Passage utile RÉELLEMENT mesuré à chaque porte (m). **Mesuré, pas
    /// nominal** : l'anneau est approximé par des cordes, et c'est cette
    /// ouverture-là que l'agent franchit.
    pub measured_doors_m: Vec<f32>,
}

/// Marqueur sur les murs de parts, pour les distinguer du reste au démontage.
#[derive(Component)]
pub struct SectorWall;

/// Bâtit cloisons et anneau. Rend le nombre de tronçons posés.
///
/// `pousser_solide` reçoit chaque tronçon pour le déclarer à `ArenaGeometry` —
/// passé en fermeture plutôt qu'en `&mut ArenaGeometry` pour que cette fonction
/// reste indépendante du type qui la porte, et donc testable.
pub fn build_sector_walls(
    commands: &mut Commands,
    cfg: &SectorsConfig,
    outer_radius_m: f32,
    mut pousser_solide: impl FnMut(&WallSeg, f32, f32),
) -> BuiltSectors {
    let layout = cfg.layout(outer_radius_m);
    let chord = cfg.chord_rad();
    let mut bati = BuiltSectors {
        layout: Some(layout),
        ..Default::default()
    };

    let mut poser = |seg: &WallSeg, nom: &'static str| {
        let c = seg.center();
        let demi_long = seg.length_m() * 0.5;
        // MÊME hauteur pour le collider et pour le solide déclaré — P2 pris à la
        // source. Les deux sortent de la même variable, ils ne peuvent pas
        // diverger.
        let h = cfg.wall_height_m;
        let demi_ep = cfg.wall_thickness_m * 0.5;
        commands.spawn((
            Name::new(nom),
            SectorWall,
            crate::StageArenaMarker,
            Transform::from_xyz(c.x, h * 0.5, c.y)
                .with_rotation(Quat::from_rotation_y(-seg.yaw_rad())),
            GlobalTransform::default(),
            RigidBody::Fixed,
            Collider::cuboid(demi_long, h * 0.5, demi_ep),
        ));
        pousser_solide(seg, h, demi_ep);
    };

    for seg in &layout.partition_walls() {
        poser(seg, "SectorPartition");
        bati.partitions += 1;
    }
    for seg in &layout.atrium_walls(chord) {
        poser(seg, "SectorAtriumWall");
        bati.atrium_segments += 1;
    }
    bati.measured_doors_m = (0..layout.count)
        .map(|s| layout.measured_door_width_m(s, chord))
        .collect();
    bati
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_repli_reflete_exactement_le_toml() {
        // Le piege paye le 2026-08-13 sur `los_lost_grace_secs` : le genome
        // ecrase le defaut Rust, donc changer le Rust seul est INERTE. Ici c'est
        // l'inverse qui menace — un defaut Rust qui s'ecarte du TOML rendrait le
        // repli (fichier absent, wasm sans pack) different du jeu normal, et le
        // defaut ne se verrait qu'en production.
        let toml = include_str!("../../../assets/genomes/arena_sectors.toml");
        let lu = SectorsConfig::parse_toml(toml);
        assert_eq!(lu, SectorsConfig::default(), "TOML et defaut Rust divergent");
    }

    #[test]
    fn un_toml_illisible_retombe_sur_le_defaut_au_lieu_de_paniquer() {
        assert_eq!(
            SectorsConfig::parse_toml("ceci n'est pas du toml"),
            SectorsConfig::default()
        );
    }

    #[test]
    fn les_bornes_ne_sont_pas_decoratives() {
        // Une part unique n'a ni frontiere ni voisine : ni cloison, ni poche
        // sure. Un mur sous l'oeil du joueur (1,70 m) ne casse aucune ligne de
        // vue, donc toute la mecanique d'evitement tombe — en silence.
        let c = SectorsConfig::parse_toml(
            "[sectors]\ncount = 1\nwall_height_m = 0.5\nchord_deg = 90.0\n",
        );
        assert!(c.count >= 2, "une seule part n'a pas de frontiere");
        assert!(c.wall_height_m > 1.70, "un mur sous l'oeil ne cache rien");
        assert!(c.chord_deg <= 30.0, "au-dela ce n'est plus un anneau");
    }

    #[test]
    fn la_porte_derivee_admet_le_boss_et_refuse_de_valoir_deux_rayons() {
        let c = SectorsConfig::default();
        let l = c.layout(69.28);
        assert!(l.door_admits(1.40), "le boss doit passer");
        assert!(
            l.door_width_m > 2.0 * c.door_agent_radius_m,
            "une porte de 2r a un couloir navigable NUL"
        );
    }

    /// Anneaux de spawn de `roguelite_waves.toml [ring]` — miroir inévitable
    /// (autre crate), donc testé, comme `spawn-clearance.md` §4bis l'exige.
    const ANNEAUX_SPAWN: &[(&str, f32)] = &[
        ("tank", 12.0),
        ("runner", 25.0),
        ("sniper", 42.0),
        ("boss", 12.0),
    ];

    #[test]
    fn l_atrium_ne_doit_pas_couper_les_anneaux_de_spawn() {
        // LE test qui manquait, et qui a coûté une run le 2026-08-13.
        //
        // L'atrium de 20 m a été posé sans regarder OÙ les ennemis apparaissent.
        // Le tank et le boss sortent à 12 m — donc DANS l'atrium, enfermés avec
        // le joueur — pendant que le runner (25 m) et le sniper (42 m) restaient
        // dehors. Mesuré en jeu : « Player died — DEFEAT » 18 s après le spawn,
        // deux fois de suite.
        //
        // La géométrie était juste. C'est l'ORDRE qui était faux : bâtir les murs
        // n'est pas jouable tant que les packs ne sont pas postés dans leurs
        // parts (incrément 3). Ce test refuse désormais la combinaison, au lieu
        // de laisser la découverte à une partie perdue.
        let c = SectorsConfig::default();
        if !c.enabled {
            // Coupé : rien à vérifier, et surtout pas un faux vert.
            println!("SECTEURS COUPES — ce test se rearmera avec `enabled = true`");
            return;
        }
        let dedans: Vec<&str> = ANNEAUX_SPAWN
            .iter()
            .filter(|(_, r)| *r < c.atrium_radius_m)
            .map(|(n, _)| *n)
            .collect();
        let dehors: Vec<&str> = ANNEAUX_SPAWN
            .iter()
            .filter(|(_, r)| *r >= c.atrium_radius_m)
            .map(|(n, _)| *n)
            .collect();
        assert!(
            dedans.is_empty() || dehors.is_empty(),
            "l'atrium de {:.0} m COUPE les anneaux de spawn : {dedans:?} naissent \
             dedans, {dehors:?} dehors. Soit tous les packs sont postés dans les \
             parts (increment 3), soit l'atrium doit passer sous {:.0} m.",
            c.atrium_radius_m,
            ANNEAUX_SPAWN.iter().map(|(_, r)| *r).fold(f32::MAX, f32::min)
        );
    }

    #[test]
    fn le_passage_mesure_tient_la_promesse_du_genome() {
        // Bout en bout : du genome jusqu'a l'ouverture reellement laissee entre
        // deux montants. C'est le seul chiffre que l'agent rencontre.
        let c = SectorsConfig::default();
        let l = c.layout(69.28);
        for s in 0..l.count {
            let m = l.measured_door_width_m(s, c.chord_rad());
            assert!(
                m >= l.door_width_m,
                "porte {s} : {m:.2} m mesures pour {:.2} annonces",
                l.door_width_m
            );
        }
    }
}

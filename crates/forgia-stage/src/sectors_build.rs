//! sectors_build.rs — les portes de l'enceinte (story-703 incrément 2).
//!
//! # Ce que cet incrément fait, après s'être trompé une fois
//!
//! Une première version bâtissait un atrium central de 20 m avec des cloisons
//! radiales. Retirée le 2026-08-13 après une run : le tank et le boss
//! apparaissent à 12 m (`roguelite_waves.toml [ring]`), donc **dans** l'anneau,
//! enfermés avec le joueur — « Player died » 18 s après le spawn, deux fois.
//!
//! La géométrie était juste et mesurée ; c'est l'intention qui était fausse.
//! **La seule cage, pour le joueur comme pour les ennemis, est l'enceinte
//! extérieure.** On la perce, on n'ajoute rien.
//!
//! # La géométrie du kit donnait la réponse
//!
//! L'enceinte est un hexagone : les milieux de ses 6 faces tombent à 0°, 60°,
//! 120°, 180°, 240°, 300°. « Trois portes dans trois directions opposées » est
//! donc exactement **une face sur deux** — et elles s'alignent d'elles-mêmes sur
//! les axes de parts. Aucun décalage à retenir, aucune constante à choisir.
//!
//! # Rien n'est calculé ici
//!
//! Ce fichier traduit [`forgia_core::sectors`] en décisions de spawn. Le calcul
//! reste testable sans moteur — les sept défauts du chantier navmesh du
//! 2026-08-13 vivaient tous dans du code qu'aucun test ne pouvait interroger.

use bevy::prelude::*;
use forgia_core::sectors::SectorLayout;
use serde::Deserialize;

const GENOME_PATH: &str = "assets/genomes/arena_sectors.toml";

/// Réglages lus depuis [`GENOME_PATH`]. Les défauts ci-dessous sont le reflet du
/// TOML, pas une source concurrente : quand les deux divergent, c'est le TOML qui
/// gagne, et un test vérifie qu'ils ne divergent pas.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct SectorsConfig {
    pub enabled: bool,
    pub count: u32,
    pub door_agent_radius_m: f32,
    pub aggro_sector_spill_frac: f32,
}

impl Default for SectorsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            count: 3,
            door_agent_radius_m: 1.40,
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
    door_agent_radius_m: Option<f32>,
    aggro_sector_spill_frac: Option<f32>,
}

impl SectorsConfig {
    /// Lit le génome. **`def_io`, pas `std::fs`** : sur wasm un `std::fs` échoue
    /// en silence et le jeu tourne avec les défauts sans qu'aucune erreur ne le
    /// dise — mur n°9 du portage web, déjà payé une fois.
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
            // Au-delà de 6 parts, deux d'entre elles se disputeraient la même
            // face d'hexagone : il n'y a que six faces à percer.
            count: s.count.unwrap_or(d.count).clamp(2, 6),
            door_agent_radius_m: s
                .door_agent_radius_m
                .unwrap_or(d.door_agent_radius_m)
                .clamp(0.1, 5.0),
            aggro_sector_spill_frac: s
                .aggro_sector_spill_frac
                .unwrap_or(d.aggro_sector_spill_frac)
                .clamp(0.0, 0.5),
        }
    }

    /// La disposition correspondante. `extent_m` est le rayon **circonscrit** de
    /// l'enceinte hexagonale — celui que `spawn_stage_arena` appelle `extent`.
    #[must_use]
    pub fn layout(&self, extent_m: f32) -> SectorLayout {
        SectorLayout {
            count: self.count,
            outer_radius_m: extent_m,
            door_width_m: SectorLayout::door_width_for(self.door_agent_radius_m),
        }
    }
}

/// Ce qui a été percé — publié pour le capteur et pour les consommateurs à venir
/// (postes des packs, ouverture des portes, aggro angulaire).
#[derive(Resource, Debug, Clone, Default)]
pub struct BuiltSectors {
    pub layout: Option<SectorLayout>,
    /// Indices des faces d'hexagone percées.
    pub doored_faces: Vec<usize>,
    /// Passage utile RÉELLEMENT laissé (m). **Mesuré, pas nominal** : on retire
    /// des modules entiers, donc le trou est plus large que l'ouverture voulue.
    /// C'est ce nombre-là que l'agent franchit.
    pub measured_door_m: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anneaux de spawn de `roguelite_waves.toml [ring]` — miroir inévitable
    /// (autre crate), donc testé, comme `spawn-clearance.md` §4bis l'exige.
    const ANNEAUX_SPAWN: &[(&str, f32)] = &[
        ("tank", 12.0),
        ("runner", 25.0),
        ("sniper", 42.0),
        ("boss", 12.0),
    ];
    /// `kaykit_dungeon/wall.glb` mesuré : 4,00 m de large, échelle 1.
    const MODULE_LEN_M: f32 = 4.0;
    const EXTENT_M: f32 = 80.0;

    #[test]
    fn le_repli_reflete_exactement_le_toml() {
        // Le piège payé le 2026-08-13 sur `los_lost_grace_secs` : quand le TOML
        // et le défaut Rust divergent, le repli (fichier absent, wasm sans pack)
        // devient différent du jeu normal — et le défaut ne se voit qu'en prod.
        let toml = include_str!("../../../assets/genomes/arena_sectors.toml");
        assert_eq!(
            SectorsConfig::parse_toml(toml),
            SectorsConfig::default(),
            "TOML et defaut Rust divergent"
        );
    }

    #[test]
    fn un_toml_illisible_retombe_sur_le_defaut_au_lieu_de_paniquer() {
        assert_eq!(
            SectorsConfig::parse_toml("ceci n'est pas du toml"),
            SectorsConfig::default()
        );
    }

    #[test]
    fn on_ne_peut_pas_demander_plus_de_parts_que_l_hexagone_n_a_de_faces() {
        // Au-dela de 6, deux parts se disputeraient la meme face : l'une n'aurait
        // pas de porte et son pack resterait enferme dehors, EN SILENCE.
        let c = SectorsConfig::parse_toml("[sectors]\ncount = 12\n");
        assert!(c.count <= 6);
        let c = SectorsConfig::parse_toml("[sectors]\ncount = 1\n");
        assert!(c.count >= 2, "une seule part n'a ni voisine ni frontiere");
    }

    #[test]
    fn les_portes_percees_n_enferment_personne() {
        // LE test qui a manque, et qui a coute une run. La version precedente
        // posait un atrium de 20 m sans regarder OU les ennemis apparaissent :
        // tank et boss a 12 m se retrouvaient enfermes DEDANS avec le joueur.
        //
        // Percer l'enceinte ne peut plus produire ca — par construction, il n'y a
        // qu'une enceinte et tout le monde est du meme cote. Ce test le grave :
        // aucun anneau de spawn ne doit tomber au-dela de l'apotheme, sinon on
        // ferait naitre un ennemi DANS le mur ou dehors.
        let c = SectorsConfig::default();
        let l = c.layout(EXTENT_M);
        for (nom, r) in ANNEAUX_SPAWN {
            assert!(
                *r < l.apothem_m(),
                "{nom} nait a {r} m, l'enceinte est a {:.1} m — hors de l'arene",
                l.apothem_m()
            );
        }
    }

    #[test]
    fn le_maillage_et_l_enceinte_sont_le_meme_hexagone() {
        // LE test miroir. `forgia-navmesh` ne peut pas dependre de `forgia-stage`
        // (la dependance va dans l'autre sens), donc les deux hexagones sont
        // decrits deux fois — et `spawn-clearance.md` §4bis exige alors un test
        // qui les compare.
        //
        // Mesure du 2026-08-14, AVANT correctif : `hexagon_edge` posait ses
        // sommets a 0+60i, `ramparts_hex_positions` a 30+60i. Deux hexagones de
        // memes dimensions, tournes de 30° l'un par rapport a l'autre, donc un
        // ecart ALTERNE de ±10,72 m sur un rayon de 69,28 :
        //
        //   · dans chaque COIN, 10,7 m de sol reel hors maillage — le joueur y
        //     etait injoignable ;
        //   · au MILIEU de chaque face, le maillage depassait le mur d'autant —
        //     il promettait des chemins dans la pierre.
        //
        // Aucune erreur n'etait levee : deux hexagones justes, mal orientes.
        let extent = 80.0_f32;
        let apotheme = crate::layout::HEX_INSCRIBED_RATIO * extent;
        // Rayon d'agent nul : on compare les GEOMETRIES, pas le retrecissement.
        let maillage = forgia_navmesh::hexagon_edge(apotheme, 0.0);
        let enceinte = crate::ramparts_hex_segment_midpoints(extent);
        assert_eq!(maillage.len(), 6);
        assert_eq!(enceinte.len(), 6);

        // Les MILIEUX de face de l'enceinte doivent tomber sur les milieux de
        // face du maillage : meme orientation, meme apotheme.
        for (mid, _) in &enceinte {
            let angle_mur = mid.z.atan2(mid.x);
            let plus_proche = maillage
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let b = maillage[(i + 1) % 6];
                    let m = (*a + b) * 0.5;
                    let d = (m.y.atan2(m.x) - angle_mur).rem_euclid(std::f32::consts::TAU);
                    let d = if d > std::f32::consts::PI {
                        std::f32::consts::TAU - d
                    } else {
                        d
                    };
                    (d, m)
                })
                .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                .unwrap();
            assert!(
                plus_proche.0.to_degrees() < 1.0,
                "milieu de face du mur a {:.1}deg sans milieu de maillage en face \
                 (le plus proche est a {:.1}deg) : les deux hexagones sont desalignes",
                angle_mur.to_degrees(),
                plus_proche.0.to_degrees()
            );
            let ecart_radial = (plus_proche.1.length() - Vec2::new(mid.x, mid.z).length()).abs();
            assert!(
                ecart_radial < 0.05,
                "meme direction mais {ecart_radial:.2} m d'ecart radial"
            );
        }
    }

    #[test]
    fn les_trois_portes_sont_dans_trois_directions_opposees() {
        let c = SectorsConfig::default();
        let l = c.layout(EXTENT_M);
        let faces = l.doored_hex_faces();
        assert_eq!(faces.len(), 3);
        let mut vues = faces.clone();
        vues.sort_unstable();
        vues.dedup();
        assert_eq!(vues.len(), 3, "deux parts partagent une porte : {faces:?}");
    }

    #[test]
    fn le_passage_mesure_admet_les_quatre_archetypes() {
        // Bout en bout : du genome jusqu'a l'ouverture reellement laissee entre
        // deux modules. C'est le seul chiffre que l'agent rencontre.
        let c = SectorsConfig::default();
        let l = c.layout(EXTENT_M);
        let n = ((l.hex_side_len_m() / MODULE_LEN_M).ceil() as u32).max(1);
        let mesure = l.measured_door_width_m(n, MODULE_LEN_M);
        for (nom, r) in [("sniper", 0.30), ("runner", 0.32), ("tank", 0.55), ("boss", 1.40)] {
            assert!(
                mesure >= SectorLayout::door_width_for(r),
                "{nom} (rayon {r} m) ne passe pas les {mesure:.2} m mesures"
            );
        }
    }
}

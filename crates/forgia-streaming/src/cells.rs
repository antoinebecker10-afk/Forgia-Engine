//! cells.rs — streaming de cellules **autorées**, cuites hors ligne.
//!
//! # Ce que ce module n'est pas
//!
//! Ce n'est pas le streaming de terrain procédural du reste de cette crate. Ici
//! le découpage est **cuit hors ligne** par Blender : un manifeste TOML liste des
//! cellules glTF avec leur AABB, et le runtime décide seulement lesquelles sont
//! présentes. Il n'y a rien à générer, seulement à charger et à décharger.
//!
//! # Pourquoi il est partagé
//!
//! Deux cartes autorées utilisent **exactement le même format** :
//!
//! | carte | manifeste | cellules |
//! |---|---|---|
//! | Château de Forgia | `castle_stream_cells.toml` | 40+ |
//! | Expédition « Le Vallon » | `vallon_stream_cells.toml` | 48 |
//!
//! Le lecteur vivait enfermé dans `castle_hub.rs`, avec un `include_str!` en dur
//! sur le chemin du château. Le dupliquer pour l'expédition aurait été la classe
//! de défaut n°1 du projet — une grandeur écrite deux fois — sur le format d'un
//! fichier, c'est-à-dire à l'endroit où la divergence se voit le plus tard.
//!
//! # Ce qui reste chez l'appelant
//!
//! Le spawn et le despawn : chaque carte a son marqueur, sa télémétrie, et son
//! rayon. Ce module ne décide que du **quoi**, jamais du **comment**.

use bevy::prelude::*;
use serde::Deserialize;

/// Une cellule glTF cuite hors ligne, avec son emprise en mètres.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct StreamCell {
    pub id: String,
    /// Chemin d'asset, suffixé `#Scene0`.
    pub render: String,
    pub bounds_min_m: [f32; 3],
    pub bounds_max_m: [f32; 3],
}

/// Le manifeste complet.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CellManifest {
    pub schema_version: u32,
    pub cell_size_m: f32,
    pub cells: Vec<StreamCell>,
}

/// Ce qui peut clocher dans un manifeste, **nommé**.
///
/// Un `Option` aurait suffi à compiler, mais pas à diagnostiquer : « le château
/// ne s'affiche pas » et « le manifeste a une version inconnue » demandent deux
/// remèdes opposés, et sans cette distinction on cherche dans le rendu.
#[derive(Debug, thiserror::Error)]
pub enum CellManifestError {
    #[error("TOML illisible : {0}")]
    Toml(#[from] toml::de::Error),
    #[error("version de schema {trouvee} inconnue (attendu {attendue})")]
    Version { trouvee: u32, attendue: u32 },
    #[error("manifeste sans aucune cellule — la carte serait invisible")]
    Vide,
}

/// La seule version de schéma que ce lecteur comprend.
pub const SCHEMA_VERSION: u32 = 1;

/// Lit un manifeste et **refuse** ce qu'il ne sait pas lire.
///
/// Un manifeste vide est une erreur, pas un cas limite : il produirait une carte
/// invisible et un jeu qui « marche » en n'affichant rien. C'est le capteur
/// aveugle que `map-design-patterns.md` §13 interdit, appliqué au chargement.
pub fn parse_manifest(contenu: &str) -> Result<CellManifest, CellManifestError> {
    let m: CellManifest = toml::from_str(contenu)?;
    if m.schema_version != SCHEMA_VERSION {
        return Err(CellManifestError::Version {
            trouvee: m.schema_version,
            attendue: SCHEMA_VERSION,
        });
    }
    if m.cells.is_empty() {
        return Err(CellManifestError::Vide);
    }
    Ok(m)
}

/// Distance XZ du joueur à l'AABB d'une cellule — **nulle** quand il est dedans.
///
/// Y est ignoré volontairement : une tour ou une falaise peut être haute sans
/// qu'il faille charger plus de décor au sol. Mesurer en 3D chargerait un étage
/// entier parce qu'on est passé sous lui.
#[must_use]
pub fn horizontal_distance(position: Vec3, cell: &StreamCell) -> f32 {
    let dx = if position.x < cell.bounds_min_m[0] {
        cell.bounds_min_m[0] - position.x
    } else if position.x > cell.bounds_max_m[0] {
        position.x - cell.bounds_max_m[0]
    } else {
        0.0
    };
    let dz = if position.z < cell.bounds_min_m[2] {
        cell.bounds_min_m[2] - position.z
    } else if position.z > cell.bounds_max_m[2] {
        position.z - cell.bounds_max_m[2]
    } else {
        0.0
    };
    dx.hypot(dz)
}

/// Les deux rayons du streaming, **et l'invariant qui les lie**.
///
/// Le rayon de déchargement DOIT dépasser celui de chargement : sans cet écart,
/// un joueur qui marche sur la frontière d'une cellule la charge et la décharge
/// à chaque pas — le ping-pong classique, qui se voit comme un clignotement du
/// décor et coûte un chargement d'asset par frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamRadii {
    pub load_m: f32,
    pub unload_m: f32,
}

impl StreamRadii {
    /// Construit des rayons **cohérents par construction** : le déchargement est
    /// dérivé du chargement, il ne se choisit pas séparément.
    ///
    /// L'hystérésis vaut au moins une cellule — en deçà, un joueur qui longe une
    /// frontière repasse le seuil avant d'avoir traversé quoi que ce soit.
    #[must_use]
    pub fn from_load(load_m: f32, cell_size_m: f32) -> Self {
        Self {
            load_m,
            unload_m: load_m + cell_size_m.max(1.0),
        }
    }

    /// L'invariant, exposé pour être testable côté appelant.
    #[must_use]
    pub fn is_coherent(&self) -> bool {
        self.unload_m > self.load_m
    }
}

/// Ce qu'il faut faire d'une cellule, à cet instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellAction {
    /// Absente et dans le rayon : à charger.
    Charger,
    /// Présente et au-delà du rayon de déchargement : à retirer.
    Decharger,
    /// Rien à faire — le cas de loin le plus fréquent.
    Garder,
}

/// La décision, **pure**. C'est elle qui porte l'hystérésis, et c'est pour ça
/// qu'elle est testable sans moteur.
#[must_use]
pub fn decide(distance_m: f32, chargee: bool, radii: StreamRadii) -> CellAction {
    if chargee {
        if distance_m > radii.unload_m {
            CellAction::Decharger
        } else {
            CellAction::Garder
        }
    } else if distance_m <= radii.load_m {
        CellAction::Charger
    } else {
        CellAction::Garder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cellule(min: [f32; 3], max: [f32; 3]) -> StreamCell {
        StreamCell {
            id: "c".into(),
            render: "x#Scene0".into(),
            bounds_min_m: min,
            bounds_max_m: max,
        }
    }

    const TOML_MINIMAL: &str = r#"
schema_version = 1
cell_size_m = 40.0
[[cells]]
id = "cell_x0_z0"
render = "models/x/cell_x0_z0_render.gltf#Scene0"
bounds_min_m = [0.0, 0.0, 0.0]
bounds_max_m = [40.0, 10.0, 40.0]
"#;

    // ── La lecture ──────────────────────────────────────────────────────

    #[test]
    fn un_manifeste_valide_se_lit() {
        let m = parse_manifest(TOML_MINIMAL).expect("doit se lire");
        assert_eq!(m.cell_size_m, 40.0);
        assert_eq!(m.cells.len(), 1);
    }

    #[test]
    fn un_manifeste_vide_est_une_erreur_pas_un_cas_limite() {
        // Il produirait une carte INVISIBLE et un jeu qui « marche » en
        // n'affichant rien. Le capteur aveugle de `map-design-patterns.md` §13,
        // applique au chargement.
        let vide = "schema_version = 1\ncell_size_m = 40.0\ncells = []\n";
        assert!(matches!(
            parse_manifest(vide),
            Err(CellManifestError::Vide)
        ));
    }

    #[test]
    fn une_version_inconnue_se_refuse_en_le_disant() {
        // « La carte ne s'affiche pas » et « le manifeste a une version
        // inconnue » demandent deux remedes opposes. Sans ce message on cherche
        // dans le rendu.
        let futur = TOML_MINIMAL.replace("schema_version = 1", "schema_version = 7");
        match parse_manifest(&futur) {
            Err(CellManifestError::Version { trouvee, attendue }) => {
                assert_eq!((trouvee, attendue), (7, 1));
            }
            autre => panic!("attendu une erreur de version, obtenu {autre:?}"),
        }
    }

    // ── LES DEUX MANIFESTES REELS DU JEU ────────────────────────────────

    #[test]
    fn le_manifeste_du_chateau_se_lit() {
        // Test miroir n°1 : ce lecteur remplace celui de `castle_hub.rs`. S'il ne
        // sait pas lire le fichier que l'ancien lisait, l'extraction a echoue —
        // et ca ne se verrait qu'en jeu, chateau vide.
        let brut = include_str!(
            "../../../assets/models/environment/castle/castle_stream_cells_grass/castle_stream_cells.toml"
        );
        let m = parse_manifest(brut).expect("le manifeste du chateau doit se lire");
        assert!(m.cells.len() >= 10, "{} cellules seulement", m.cells.len());
    }

    #[test]
    fn le_manifeste_du_vallon_se_lit() {
        // Test miroir n°2 : la carte d'expedition, exportee par
        // `tools/blender/expedition/92_cellules.py`. Si Blender change son
        // format, ce test tombe AVANT qu'on lance le jeu.
        let brut = include_str!(
            "../../../assets/models/environment/expedition/vallon_stream_cells/vallon_stream_cells.toml"
        );
        let m = parse_manifest(brut).expect("le manifeste du Vallon doit se lire");
        assert_eq!(m.cells.len(), 48, "48 cellules attendues");
        assert_eq!(m.cell_size_m, 40.0);
    }

    #[test]
    fn les_deux_cartes_partagent_le_meme_format() {
        // Ce qui justifie ce module partage. Si les deux divergeaient, il
        // faudrait deux lecteurs — et ce test le dirait au lieu de le laisser
        // decouvrir en jeu.
        let chateau = parse_manifest(include_str!(
            "../../../assets/models/environment/castle/castle_stream_cells_grass/castle_stream_cells.toml"
        ))
        .unwrap();
        let vallon = parse_manifest(include_str!(
            "../../../assets/models/environment/expedition/vallon_stream_cells/vallon_stream_cells.toml"
        ))
        .unwrap();
        assert_eq!(chateau.schema_version, vallon.schema_version);
    }

    // ── La distance ─────────────────────────────────────────────────────

    #[test]
    fn dans_la_cellule_la_distance_est_nulle() {
        let c = cellule([0.0, 0.0, 0.0], [40.0, 10.0, 40.0]);
        assert_eq!(horizontal_distance(Vec3::new(20.0, 5.0, 20.0), &c), 0.0);
    }

    #[test]
    fn l_altitude_n_influe_pas_sur_la_distance() {
        // Volontaire : une tour peut etre haute sans qu'il faille charger plus de
        // decor au sol. Mesurer en 3D chargerait un etage entier parce qu'on est
        // passe dessous.
        let c = cellule([0.0, 0.0, 0.0], [40.0, 10.0, 40.0]);
        let bas = horizontal_distance(Vec3::new(60.0, 0.0, 20.0), &c);
        let haut = horizontal_distance(Vec3::new(60.0, 500.0, 20.0), &c);
        assert!((bas - haut).abs() < 1.0e-6);
        assert!((bas - 20.0).abs() < 1.0e-4);
    }

    #[test]
    fn en_diagonale_la_distance_est_la_vraie_distance_pas_la_somme() {
        // Prendre `dx + dz` surestimerait de 41 % dans les coins et dechargerait
        // trop tot — le meme piege que « le joueur est un disque »
        // (`map-design-patterns.md` §1).
        let c = cellule([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let d = horizontal_distance(Vec3::new(13.0, 0.0, 14.0), &c);
        assert!((d - 5.0).abs() < 1.0e-4, "distance {d}, attendu 5 (3-4-5)");
    }

    // ── L'hysteresis ────────────────────────────────────────────────────

    #[test]
    fn le_rayon_de_dechargement_depasse_toujours_celui_de_chargement() {
        // Sans cet ecart, un joueur qui marche sur une frontiere charge et
        // decharge la cellule a chaque pas : le decor clignote et on paie un
        // chargement d'asset par frame.
        for load in [10.0_f32, 120.0, 240.0] {
            for taille in [8.0_f32, 32.0, 40.0] {
                let r = StreamRadii::from_load(load, taille);
                assert!(r.is_coherent(), "load {load}, cellule {taille}");
                assert!(
                    r.unload_m - r.load_m >= taille,
                    "hysteresis plus petite qu'une cellule"
                );
            }
        }
    }

    #[test]
    fn une_cellule_dans_le_rayon_se_charge_une_seule_fois() {
        let r = StreamRadii::from_load(100.0, 40.0);
        assert_eq!(decide(50.0, false, r), CellAction::Charger);
        assert_eq!(decide(50.0, true, r), CellAction::Garder);
    }

    #[test]
    fn la_bande_d_hysteresis_ne_declenche_rien() {
        // LE test de l'hysteresis : entre les deux rayons, une cellule chargee
        // reste chargee et une cellule absente reste absente. C'est cette zone
        // morte qui supprime le ping-pong.
        let r = StreamRadii::from_load(100.0, 40.0);
        let milieu = (r.load_m + r.unload_m) * 0.5;
        assert_eq!(decide(milieu, true, r), CellAction::Garder);
        assert_eq!(decide(milieu, false, r), CellAction::Garder);
    }

    #[test]
    fn au_dela_du_rayon_de_dechargement_on_retire() {
        let r = StreamRadii::from_load(100.0, 40.0);
        assert_eq!(decide(r.unload_m + 0.1, true, r), CellAction::Decharger);
        assert_eq!(decide(r.unload_m + 0.1, false, r), CellAction::Garder);
    }

    #[test]
    fn un_aller_retour_sur_la_frontiere_ne_produit_aucun_battement() {
        // Simulation du cas reel : le joueur oscille autour du rayon de
        // chargement. Sans hysteresis ce serait N chargements ; avec, c'est UN.
        let r = StreamRadii::from_load(100.0, 40.0);
        let mut chargee = false;
        let mut chargements = 0;
        for i in 0..200 {
            let d = 100.0 + if i % 2 == 0 { -0.5 } else { 0.5 };
            match decide(d, chargee, r) {
                CellAction::Charger => {
                    chargee = true;
                    chargements += 1;
                }
                CellAction::Decharger => chargee = false,
                CellAction::Garder => {}
            }
        }
        assert_eq!(chargements, 1, "{chargements} chargements au lieu d'un seul");
    }
}

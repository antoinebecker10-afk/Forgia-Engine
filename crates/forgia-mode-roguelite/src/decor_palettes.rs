//! decor_palettes.rs — les DIRECTIONS ARTISTIQUES en couche definition (story-671).
//!
//! Avant : les catalogues de props étaient des `const &[&str]` dans `decor.rs`.
//! UNE seule palette pour tout le jeu (Inferno + murs KayKit dungeon), donc toutes
//! les salles de toutes les runs portaient le même habillage. Le semis bougeait —
//! il est tiré à la graine — mais c'étaient toujours les mêmes objets.
//!
//! Maintenant : `assets/genomes/roguelite/roguelite_palettes.toml` déclare N
//! palettes et la table `stage_palette` dit laquelle porte quel stage. Ré-habiller
//! une salle = changer une chaîne dans le TOML, sans toucher au code.
//!
//! **Le miroir Rust ne contient que `inferno`**, la palette historique. Si le
//! génome est absent (build distribué, chemin relatif au CWD), le jeu retombe
//! donc exactement sur le comportement d'avant story-671 — pas sur une salle vide.
//! Les trois autres DA vivent uniquement en couche definition, ce qui est leur
//! place : ce sont des listes d'assets, pas de la logique.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_palettes.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Palette par défaut quand un stage n'est pas dans `[stage_palette]`.
pub const FALLBACK_PALETTE: &str = "inferno";

/// Un jeu de props cohérent = une DA.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DecorPalette {
    /// Nom lisible (logs, futur affichage HUD « tu entres dans … »).
    pub name: String,
    /// Points focaux hauts (statue, tour, grand arbre).
    pub landmarks: Vec<String>,
    /// Masses de remplissage du périmètre.
    pub big: Vec<String>,
    /// Éléments porteurs de lumière (brasero) — ou leur équivalent de la DA.
    pub braziers: Vec<String>,
    /// Petits props dispersés au sol, sans collider.
    pub scatter: Vec<String>,
    /// Segments de mur (salles en L).
    pub walls: Vec<String>,
    /// Angle de mur.
    pub wall_corner: String,
    /// Gravats au sol (masque de répétition des dalles).
    pub rubble: Vec<String>,
    /// Bâtiments (silhouettes urbaines).
    pub buildings: Vec<String>,
}

impl DecorPalette {
    /// La palette historique (pack Inferno + murs KayKit dungeon) — miroir exact
    /// des ex-`const` de `decor.rs`.
    fn inferno() -> Self {
        let v = |xs: &[&str]| xs.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
        Self {
            name: "La Crypte de l'Enclume".into(),
            landmarks: v(&[
                "models/environment/inferno/StatueKnight_002.glb",
                "models/environment/inferno/TowerBig_001.glb",
            ]),
            big: v(&[
                "models/environment/inferno/RockBig_001.glb",
                "models/environment/inferno/RockBig_003.glb",
                "models/environment/inferno/RockBig_004.glb",
                "models/environment/inferno/Crag_001.glb",
                "models/environment/inferno/Crag_003.glb",
                "models/environment/inferno/Mound_005.glb",
                "models/environment/inferno/Mound_008.glb",
                "models/environment/inferno/ColumnBig_001.glb",
                "models/environment/inferno/ColumnBigBroken_001.glb",
                "models/environment/inferno/ColumnBigBroken_002.glb",
            ]),
            braziers: v(&[
                "models/environment/inferno/Brazier_002.glb",
                "models/environment/inferno/Brazier_004.glb",
            ]),
            scatter: v(&[
                "models/environment/inferno/RockMid_001.glb",
                "models/environment/inferno/RockMid_002.glb",
                "models/environment/inferno/RockMid_003.glb",
                "models/environment/inferno/Box_001.glb",
                "models/environment/inferno/Vase_001.glb",
                "models/environment/inferno/Vase_002.glb",
                "models/environment/inferno/Gear_001.glb",
                "models/environment/inferno/Gear_002.glb",
            ]),
            walls: v(&[
                "models/kaykit/dungeon/wall.glb",
                "models/kaykit/dungeon/wall.glb",
                "models/kaykit/dungeon/wall_broken.glb",
                "models/kaykit/dungeon/wall_window.glb",
            ]),
            wall_corner: "models/kaykit/dungeon/wall_corner.glb".into(),
            rubble: v(&["models/kaykit/dungeon/rubble.glb"]),
            buildings: v(&[
                "models/kaykit/hexagon/red/building_blacksmith_red.gltf",
                "models/kaykit/hexagon/red/building_mine_red.gltf",
                "models/kaykit/hexagon/red/building_tower_A_red.gltf",
                "models/kaykit/hexagon/red/building_tower_catapult_red.gltf",
                "models/kaykit/hexagon/neutral/building_scaffolding.gltf",
                "models/kaykit/hexagon/neutral/building_destroyed.gltf",
            ]),
        }
    }
}

/// Toutes les DA + la table stage → DA.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DecorPalettesConfig {
    pub palettes: HashMap<String, DecorPalette>,
    /// `stage_id` → id de palette. Absent = `FALLBACK_PALETTE`.
    pub stage_palette: HashMap<String, String>,
}

impl Default for DecorPalettesConfig {
    fn default() -> Self {
        let mut palettes = HashMap::new();
        palettes.insert(FALLBACK_PALETTE.to_string(), DecorPalette::inferno());
        Self {
            palettes,
            // Sans génome, tous les stages portent la DA historique.
            stage_palette: HashMap::new(),
        }
    }
}

impl DecorPalettesConfig {
    /// PUR — testable. Fallback complet si le TOML est illisible, et le DIT.
    pub fn parse_toml(content: &str) -> Self {
        match toml::from_str::<Self>(content) {
            Ok(mut c) => {
                if c.palettes.is_empty() {
                    warn!("[decor-palettes] genome sans palette — miroir Rust utilisé");
                    return Self::default();
                }
                // Garantie : la palette de repli existe TOUJOURS, sinon un stage non
                // listé se retrouverait sans aucun prop.
                c.palettes
                    .entry(FALLBACK_PALETTE.to_string())
                    .or_insert_with(DecorPalette::inferno);
                c
            }
            Err(e) => {
                warn!("[decor-palettes] genome illisible ({e}) — MIROIR RUST utilisé");
                Self::default()
            }
        }
    }

    /// Chargement direct depuis le disque — utilisé par le préchargement des
    /// assets, qui ne peut pas dépendre de l'ordre d'application des Commands
    /// au Startup.
    pub fn load_or_default_public() -> Self {
        Self::load_or_default()
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[decor-palettes] {GENOME_PATH} illisible ({e}) — DA historique seule");
                Self::default()
            }
        }
    }

    /// Id de palette d'un stage. Repli explicite (jamais de salle sans props).
    pub fn palette_id_for_stage(&self, stage_id: &str) -> &str {
        self.stage_palette
            .get(stage_id)
            .map(String::as_str)
            .filter(|id| self.palettes.contains_key(*id))
            .unwrap_or(FALLBACK_PALETTE)
    }

    pub fn palette(&self, id: &str) -> Option<&DecorPalette> {
        self.palettes.get(id)
    }

    /// Tous les chemins d'assets déclarés, toutes palettes confondues — pour le
    /// préchargement.
    pub fn all_paths(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for p in self.palettes.values() {
            for list in [
                &p.landmarks,
                &p.big,
                &p.braziers,
                &p.scatter,
                &p.walls,
                &p.rubble,
                &p.buildings,
            ] {
                out.extend(list.iter().map(String::as_str));
            }
            if !p.wall_corner.is_empty() {
                out.push(&p.wall_corner);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

// ─── Systems : load + hot-reload ─────────────────────────────────────────────

#[derive(Resource, Default, Debug)]
pub struct DecorPalettesWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

pub fn sys_init_decor_palettes(mut commands: Commands) {
    let cfg = DecorPalettesConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    let mut ids: Vec<&str> = cfg.palettes.keys().map(String::as_str).collect();
    ids.sort_unstable();
    info!(
        "[decor-palettes] {} DA chargées ({}) — {} assets distincts",
        ids.len(),
        ids.join(", "),
        cfg.all_paths().len()
    );
    commands.insert_resource(cfg);
    commands.insert_resource(DecorPalettesWatch {
        last_mtime: mtime,
        reload_count: 0,
    });
}

/// Poll mtime 1 Hz (`genome-code.md` : tout gène doit marcher en hot-reload).
/// Le rechargement ne re-précharge PAS les scènes : les handles vivent dans
/// `DecorAssets`, chargés au boot pour toutes les palettes. Éditer une liste
/// prend donc effet sur les props DÉJÀ préchargés ; ajouter un chemin inédit
/// demande un redémarrage.
pub fn sys_hot_reload_decor_palettes(
    time: Res<Time<Real>>,
    mut cfg: ResMut<DecorPalettesConfig>,
    mut watch: ResMut<DecorPalettesWatch>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = POLL_PERIOD_SEC;
    let Ok(mtime) = fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let next = DecorPalettesConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count = watch.reload_count.saturating_add(1);
    info!(
        "[decor-palettes] genome HOT-RELOADED (#{}) — effet à la PROCHAINE salle",
        watch.reload_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rust_mirror_is_the_historical_da_only() {
        let d = DecorPalettesConfig::default();
        assert_eq!(d.palettes.len(), 1, "sans génome : la DA historique seule");
        assert!(d.palettes.contains_key(FALLBACK_PALETTE));
        // Comportement identique à avant story-671 pour n'importe quel stage.
        assert_eq!(d.palette_id_for_stage("crypts_of_anvil"), FALLBACK_PALETTE);
        assert_eq!(d.palette_id_for_stage("n_importe_quoi"), FALLBACK_PALETTE);
    }

    #[test]
    fn an_unknown_stage_or_a_dangling_palette_falls_back_never_empty() {
        let c = DecorPalettesConfig::parse_toml(
            r#"
[palettes.inferno]
name = "hist"
big = ["a.glb"]

[stage_palette]
connu = "inferno"
casse = "palette_qui_n_existe_pas"
"#,
        );
        assert_eq!(c.palette_id_for_stage("connu"), "inferno");
        assert_eq!(
            c.palette_id_for_stage("casse"),
            FALLBACK_PALETTE,
            "une palette référencée mais absente ne doit pas vider la salle"
        );
        assert_eq!(c.palette_id_for_stage("jamais_declare"), FALLBACK_PALETTE);
    }

    /// Un génome sans `inferno` ne doit pas priver le repli de sa palette.
    #[test]
    fn the_fallback_palette_is_always_present() {
        let c = DecorPalettesConfig::parse_toml("[palettes.autre]\nname = \"x\"\nbig = [\"a.glb\"]");
        assert!(c.palettes.contains_key(FALLBACK_PALETTE));
        assert!(c.palettes.contains_key("autre"));
    }

    #[test]
    fn a_broken_genome_falls_back_loudly_not_silently_empty() {
        let c = DecorPalettesConfig::parse_toml("ceci n'est pas du TOML {{{");
        assert_eq!(c, DecorPalettesConfig::default());
    }

    /// Le TOML livré doit déclarer les 4 DA et router les 4 stages.
    #[test]
    fn the_real_genome_declares_the_four_art_directions() {
        let content = fs::read_to_string(GENOME_PATH)
            .or_else(|_| fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_palettes.toml introuvable depuis la crate ET depuis la racine");
        let c = DecorPalettesConfig::parse_toml(&content);
        for id in ["inferno", "donjon", "paturages", "bourg"] {
            let p = c.palette(id).unwrap_or_else(|| panic!("palette {id} absente"));
            assert!(!p.name.is_empty(), "{id} doit avoir un nom lisible");
            assert!(!p.big.is_empty(), "{id} doit avoir des masses de remplissage");
            assert!(!p.scatter.is_empty(), "{id} doit avoir du semis au sol");
        }
        for stage in [
            "crypts_of_anvil",
            "forge_sanctum",
            "donjon_oublie",
            "hauts_paturages",
        ] {
            let id = c.palette_id_for_stage(stage);
            assert!(c.palette(id).is_some(), "{stage} → palette {id} inconnue");
        }
        // Deux stages ne doivent pas porter la MÊME DA, sinon la variété est fictive.
        let ids: std::collections::HashSet<&str> = [
            "crypts_of_anvil",
            "forge_sanctum",
            "donjon_oublie",
            "hauts_paturages",
        ]
        .iter()
        .map(|s| c.palette_id_for_stage(s))
        .collect();
        assert_eq!(ids.len(), 4, "les 4 stages doivent porter 4 DA distinctes");
    }
}

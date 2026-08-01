//! ambiances.rs — les UNIVERS d'arène en couche definition (story-676).
//!
//! ## Ce qui était cassé
//!
//! Trois choses, vérifiées avant d'écrire une ligne :
//!
//! - `biome_sky.toml` déclare **12 palettes de ciel**, les stages en utilisaient
//!   **2** — trois arènes sur quatre déclarent le même biome `Plains`.
//! - `atmosphere.rs` posait `volcanic_fog()` sur `in_state(GameMode::Roguelite)`,
//!   **sans aucun filtre par biome** : brume rouge-orangée sur les Hauts Pâturages.
//!   Les couleurs étaient des `const` Rust.
//! - `forgia_stage::MERGED_FLOOR_GLB` était une `const [&str; 3]` : **un seul sol
//!   pour les 4 arènes**, impossible à changer sans recompiler.
//!
//! ## Ce que ce module fait
//!
//! Une **ambiance** regroupe ce qui doit varier ensemble : sol + ciel + brouillard
//! + ambiante. Et elle est indexée sur la **profondeur du round**, pas sur
//! l'identité de la salle — le ciel devient l'horloge de la run.
//!
//! Le miroir Rust ne contient que `forge_ardente`, l'ambiance historique : génome
//! absent → on retombe exactement sur le comportement d'avant, jamais sur une
//! arène sans sol.

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::SystemTime;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_ambiances.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Ambiance de repli — celle d'avant story-676.
pub const FALLBACK_AMBIANCE: &str = "forge_ardente";
/// Palette de sol de repli — les 3 GLB de l'ex-`MERGED_FLOOR_GLB`.
pub const FALLBACK_FLOOR: &str = "forge_pierre";

/// Le mélangeur du sol produit un `kind` 0/1/2 : une palette a exactement 3 tuiles.
pub const FLOOR_TILE_SLOTS: usize = 3;

// ─── Sol ────────────────────────────────────────────────────────────────────

/// Une palette de sol : le pas de trame et les 3 tuiles semées dessus.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct FloorPalette {
    /// Côté RÉEL de la tuile (m). L'arène pave à ce pas ; s'en écarter fait des
    /// trous ou des recouvrements.
    pub tile_size_m: f32,
    /// Exactement 3 chemins. tiles\[0\] domine, \[1\] et \[2\] sont les variantes.
    pub tiles: Vec<String>,
}

impl Default for FloorPalette {
    fn default() -> Self {
        Self::forge_pierre()
    }
}

impl FloorPalette {
    /// Le sol historique — ex-`forgia_stage::MERGED_FLOOR_GLB`.
    pub fn forge_pierre() -> Self {
        Self {
            tile_size_m: 4.0,
            tiles: vec![
                "models/kaykit/dungeon/floor.glb".into(),
                "models/kaykit/dungeon/floor_dirt.glb".into(),
                "models/kaykit/dungeon/floor_rocks.glb".into(),
            ],
        }
    }

    /// Une palette est utilisable si elle a ses 3 fentes et un pas plausible.
    ///
    /// Le pas est borné à \[1, 16\] m : en-dessous d'1 m une arène de 160 m
    /// demanderait 25 600 tuiles à fusionner, ce qui n'est pas mesuré.
    pub fn is_usable(&self) -> bool {
        self.tiles.len() == FLOOR_TILE_SLOTS
            && self.tiles.iter().all(|t| !t.is_empty())
            && (1.0..=16.0).contains(&self.tile_size_m)
    }
}

// ─── Ambiance ───────────────────────────────────────────────────────────────

/// Un univers d'arène : ce qui doit varier ensemble.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Ambiance {
    /// Nom lisible (logs, futur « tu entres dans … »).
    pub label: String,
    /// Clé dans `[floors]`.
    pub floor: String,
    /// Clé dans `assets/genomes/biome_sky.toml`.
    pub sky: String,
    pub fog_rgb: [f32; 3],
    pub fog_sun_rgb: [f32; 3],
    pub fog_density: f32,
    pub ambient_rgb: [f32; 3],
    pub ambient_brightness: f32,
}

impl Default for Ambiance {
    fn default() -> Self {
        Self::forge_ardente()
    }
}

impl Ambiance {
    /// L'ambiance historique — les ex-consts de `atmosphere.rs`.
    pub fn forge_ardente() -> Self {
        Self {
            label: "Forge ardente".into(),
            floor: FALLBACK_FLOOR.into(),
            sky: "volcanic".into(),
            fog_rgb: [0.30, 0.10, 0.07],
            fog_sun_rgb: [1.00, 0.55, 0.25],
            fog_density: 0.008,
            ambient_rgb: [1.00, 0.45, 0.22],
            ambient_brightness: 300.0,
        }
    }

    /// Bornes appliquées à la lecture : un génome édité à la main ne doit pas
    /// pouvoir produire un écran noir ou une brume opaque.
    fn clamped(mut self) -> Self {
        self.fog_density = self.fog_density.clamp(0.0, 0.2);
        self.ambient_brightness = self.ambient_brightness.clamp(0.0, 5_000.0);
        for c in self
            .fog_rgb
            .iter_mut()
            .chain(self.fog_sun_rgb.iter_mut())
            .chain(self.ambient_rgb.iter_mut())
        {
            *c = c.clamp(0.0, 1.0);
        }
        self
    }
}

/// Ordre de traversée des univers + option de décalage par graine.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Rotation {
    pub order: Vec<String>,
    pub shuffle_start_by_seed: bool,
}

impl Default for Rotation {
    fn default() -> Self {
        Self {
            order: vec![FALLBACK_AMBIANCE.into()],
            shuffle_start_by_seed: false,
        }
    }
}

// ─── Config ─────────────────────────────────────────────────────────────────

#[derive(Resource, Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct AmbiancesConfig {
    pub floors: HashMap<String, FloorPalette>,
    pub ambiances: HashMap<String, Ambiance>,
    pub rotation: Rotation,
}

impl Default for AmbiancesConfig {
    fn default() -> Self {
        let mut floors = HashMap::new();
        floors.insert(FALLBACK_FLOOR.to_string(), FloorPalette::forge_pierre());
        let mut ambiances = HashMap::new();
        ambiances.insert(FALLBACK_AMBIANCE.to_string(), Ambiance::forge_ardente());
        Self {
            floors,
            ambiances,
            rotation: Rotation::default(),
        }
    }
}

impl AmbiancesConfig {
    /// PUR — testable. Un génome illisible retombe sur le miroir Rust et le DIT.
    pub fn parse_toml(content: &str) -> Self {
        let mut c: Self = match toml::from_str(content) {
            Ok(c) => c,
            Err(e) => {
                warn!("[ambiances] génome illisible ({e}) — MIROIR RUST (forge seule)");
                return Self::default();
            }
        };
        // Bornage + rejet des palettes de sol inutilisables. Un sol à 2 tuiles
        // ferait planter le mélangeur, un pas à 0,1 m ferait 2,5 M de tuiles.
        c.floors.retain(|id, f| {
            let ok = f.is_usable();
            if !ok {
                warn!(
                    "[ambiances] sol '{id}' REJETÉ ({} tuiles, pas {:.2} m) — il en faut 3 et un pas dans [1, 16] m",
                    f.tiles.len(),
                    f.tile_size_m
                );
            }
            ok
        });
        c.ambiances = c
            .ambiances
            .into_iter()
            .map(|(k, a)| (k, a.clamped()))
            .collect();
        // Les replis existent TOUJOURS : sans eux, une arène pourrait n'avoir
        // aucun sol, ce qui est une chute infinie et pas un défaut cosmétique.
        c.floors
            .entry(FALLBACK_FLOOR.to_string())
            .or_insert_with(FloorPalette::forge_pierre);
        c.ambiances
            .entry(FALLBACK_AMBIANCE.to_string())
            .or_insert_with(Ambiance::forge_ardente);
        // Une rotation qui pointe une ambiance inexistante donnerait un round
        // sans univers : on filtre, et on le dit.
        let before = c.rotation.order.len();
        c.rotation
            .order
            .retain(|id| c.ambiances.contains_key(id.as_str()));
        if c.rotation.order.len() != before {
            warn!(
                "[ambiances] rotation : {} entrées pointaient une ambiance absente — retirées",
                before - c.rotation.order.len()
            );
        }
        if c.rotation.order.is_empty() {
            warn!("[ambiances] rotation VIDE — repli sur '{FALLBACK_AMBIANCE}'");
            c.rotation.order.push(FALLBACK_AMBIANCE.to_string());
        }
        c
    }

    pub fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[ambiances] {GENOME_PATH} illisible ({e}) — forge historique seule");
                Self::default()
            }
        }
    }

    /// **L'horloge de run** : l'univers suit la profondeur, puis boucle.
    ///
    /// La boucle est voulue — les rounds sont infinis, la liste ne l'est pas. Ce
    /// qui distingue le 2ᵉ tour du 1ᵉʳ est la difficulté, pas le décor.
    pub fn ambiance_id_for_round(&self, round: u32, run_seed: u64) -> &str {
        let n = self.rotation.order.len().max(1);
        let offset = if self.rotation.shuffle_start_by_seed {
            (run_seed % n as u64) as usize
        } else {
            0
        };
        let idx = (round as usize).wrapping_add(offset) % n;
        self.rotation
            .order
            .get(idx)
            .map(String::as_str)
            .unwrap_or(FALLBACK_AMBIANCE)
    }

    pub fn ambiance(&self, id: &str) -> &Ambiance {
        self.ambiances.get(id).unwrap_or_else(|| {
            self.ambiances
                .get(FALLBACK_AMBIANCE)
                .expect("le repli est garanti par parse_toml/Default")
        })
    }

    /// Palette de sol d'une ambiance, avec repli explicite : jamais « pas de sol ».
    pub fn floor_of(&self, ambiance_id: &str) -> &FloorPalette {
        let key = &self.ambiance(ambiance_id).floor;
        self.floors.get(key.as_str()).unwrap_or_else(|| {
            self.floors
                .get(FALLBACK_FLOOR)
                .expect("le repli est garanti par parse_toml/Default")
        })
    }

    /// Tous les chemins de tuiles déclarés — le préchargement doit les couvrir
    /// TOUS, puisqu'on ne sait pas au boot quels univers la run traversera.
    pub fn all_floor_paths(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .floors
            .values()
            .flat_map(|f| f.tiles.iter().cloned())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

// ─── Plomberie ──────────────────────────────────────────────────────────────

#[derive(Resource, Default, Debug)]
pub struct AmbiancesWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Ambiance en vigueur, résolue à chaque changement de round. Les consommateurs
/// (atmosphère, ciel, sol) la lisent au lieu de recalculer chacun de leur côté.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct CurrentAmbiance {
    pub id: String,
    pub round: u32,
}

pub fn sys_init_ambiances(mut commands: Commands) {
    let cfg = AmbiancesConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    let mut ids: Vec<&str> = cfg.ambiances.keys().map(String::as_str).collect();
    ids.sort_unstable();
    info!(
        "[ambiances] {} univers ({}) — {} sols, {} tuiles distinctes, rotation de {}",
        ids.len(),
        ids.join(", "),
        cfg.floors.len(),
        cfg.all_floor_paths().len(),
        cfg.rotation.order.len()
    );
    commands.insert_resource(cfg);
    commands.insert_resource(AmbiancesWatch {
        last_mtime: mtime,
        reload_count: 0,
    });
    commands.insert_resource(CurrentAmbiance {
        id: FALLBACK_AMBIANCE.to_string(),
        round: 0,
    });
}

/// Poll mtime 1 Hz. Le rechargement prend effet sur l'atmosphère IMMÉDIATEMENT
/// (couleurs) et sur le sol à la prochaine arène (les tuiles sont déjà posées).
pub fn sys_hot_reload_ambiances(
    time: Res<Time<Real>>,
    mut cfg: ResMut<AmbiancesConfig>,
    mut watch: ResMut<AmbiancesWatch>,
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
    let next = AmbiancesConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count = watch.reload_count.saturating_add(1);
    info!(
        "[ambiances] génome HOT-RELOADED (#{}) — couleurs tout de suite, sol à la prochaine arène",
        watch.reload_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_genome_falls_back_to_the_forge_instead_of_no_floor() {
        let c = AmbiancesConfig::parse_toml("ceci n'est pas du TOML {{{");
        assert_eq!(c.ambiance_id_for_round(0, 0), FALLBACK_AMBIANCE);
        assert!(c.floor_of(FALLBACK_AMBIANCE).is_usable());
        assert_eq!(c.floor_of(FALLBACK_AMBIANCE).tiles.len(), FLOOR_TILE_SLOTS);
    }

    /// Un sol à 2 tuiles ferait planter le mélangeur (kind 0/1/2) : il doit être
    /// rejeté à la lecture, pas découvert au spawn.
    #[test]
    fn an_incomplete_floor_palette_is_rejected_at_parse_time() {
        let c = AmbiancesConfig::parse_toml(
            r#"
[floors.bancal]
tile_size_m = 4.0
tiles = ["a.glb", "b.glb"]
[floors.trop_fin]
tile_size_m = 0.1
tiles = ["a.glb", "b.glb", "c.glb"]
[floors.bon]
tile_size_m = 4.0
tiles = ["a.glb", "b.glb", "c.glb"]
"#,
        );
        assert!(!c.floors.contains_key("bancal"));
        assert!(!c.floors.contains_key("trop_fin"), "un pas de 0,1 m = 2,5 M de tuiles");
        assert!(c.floors.contains_key("bon"));
        assert!(c.floors.contains_key(FALLBACK_FLOOR), "le repli est garanti");
    }

    /// L'horloge de run : l'univers suit la profondeur, puis boucle.
    #[test]
    fn the_ambiance_follows_the_round_then_loops() {
        let c = AmbiancesConfig::parse_toml(
            r#"
[ambiances.a]
floor = "f"
[ambiances.b]
floor = "f"
[ambiances.c]
floor = "f"
[floors.f]
tile_size_m = 4.0
tiles = ["1.glb", "2.glb", "3.glb"]
[rotation]
order = ["a", "b", "c"]
shuffle_start_by_seed = false
"#,
        );
        let seq: Vec<&str> = (0..7).map(|r| c.ambiance_id_for_round(r, 0)).collect();
        assert_eq!(seq, ["a", "b", "c", "a", "b", "c", "a"]);
    }

    /// Une rotation qui pointe une ambiance absente donnerait un round sans
    /// univers. Elle est filtrée à la lecture.
    #[test]
    fn a_rotation_pointing_at_a_missing_ambiance_is_filtered() {
        let c = AmbiancesConfig::parse_toml(
            r#"
[ambiances.reelle]
floor = "f"
[floors.f]
tile_size_m = 4.0
tiles = ["1.glb", "2.glb", "3.glb"]
[rotation]
order = ["reelle", "fantome", "reelle"]
"#,
        );
        assert!(!c.rotation.order.iter().any(|id| id == "fantome"));
        assert_eq!(c.rotation.order.len(), 2);
    }

    /// Deux runs ne doivent pas démarrer dans le même univers, mais l'ORDRE
    /// derrière doit rester le même — sinon l'horloge ne dit plus rien.
    #[test]
    fn the_seed_shifts_the_start_without_breaking_the_order() {
        let c = AmbiancesConfig::parse_toml(
            r#"
[ambiances.a]
floor = "f"
[ambiances.b]
floor = "f"
[ambiances.c]
floor = "f"
[floors.f]
tile_size_m = 4.0
tiles = ["1.glb", "2.glb", "3.glb"]
[rotation]
order = ["a", "b", "c"]
shuffle_start_by_seed = true
"#,
        );
        let starts: Vec<&str> = (0..3).map(|s| c.ambiance_id_for_round(0, s)).collect();
        assert_eq!(starts, ["a", "b", "c"], "la graine décale le départ");
        // Et l'ordre relatif tient quel que soit le décalage.
        for seed in 0..3u64 {
            let a = c.ambiance_id_for_round(0, seed);
            let b = c.ambiance_id_for_round(1, seed);
            let ia = c.rotation.order.iter().position(|x| x == a).unwrap();
            let ib = c.rotation.order.iter().position(|x| x == b).unwrap();
            assert_eq!(ib, (ia + 1) % 3, "l'ordre doit rester cyclique");
        }
    }

    /// Le génome livré doit être lisible, complet, et pointer des fichiers qui
    /// existent — sinon on découvre le trou en jeu, pas en test.
    #[test]
    fn the_shipped_genome_is_complete_and_its_tiles_exist_on_disk() {
        let content = fs::read_to_string(GENOME_PATH)
            .or_else(|_| fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_ambiances.toml introuvable");
        let c = AmbiancesConfig::parse_toml(&content);
        assert!(c.ambiances.len() >= 6, "seulement {} univers", c.ambiances.len());
        assert!(c.rotation.order.len() >= 6, "rotation trop courte");

        let root = if fs::metadata("assets").is_ok() { "assets" } else { "../../assets" };
        let missing: Vec<&String> = c
            .all_floor_paths()
            .iter()
            .filter(|p| fs::metadata(format!("{root}/{p}")).is_err())
            .cloned()
            .collect::<Vec<String>>()
            .leak()
            .iter()
            .collect();
        assert!(missing.is_empty(), "tuiles de sol absentes du disque : {missing:#?}");

        // Chaque ambiance doit pointer un sol RÉEL, pas retomber en silence.
        for (id, a) in &c.ambiances {
            assert!(
                c.floors.contains_key(a.floor.as_str()),
                "l'ambiance '{id}' pointe le sol '{}' qui n'existe pas",
                a.floor
            );
        }
    }
}

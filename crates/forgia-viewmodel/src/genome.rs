//! Couche **data** du viewmodel — équivalent `weapon_script.txt` Source SDK
//! (cf `developer.valvesoftware.com/wiki/Authoring_a_weapon_entity`).
//!
//! Tous les paramètres per-arme TOML hot-reloadable :
//! - **Render-only** : `offset_*`, `rotation_*_deg`, `fallback_scale`, `target_size`, `barrel_length`
//! - **ADS pose** : `ads_offset_*`, `ads_fov_deg`, `ads_scale_factor`, `hipfire_tilt_y_deg`
//! - **Sight align (CoD style)** : `sight_local_*`, `sight_distance`
//! - **Fade en ADS** : `scope_glass_alpha_ads`, `ads_viewmodel_fade_alpha`
//! - **Feel ADS** : `ads_move_speed_factor`, `ads_mouse_sensitivity_factor`
//! - **Sniper overlay** : `sniper_scope_fullscreen`
//! - **Gameplay** (lus par `forgia-fps::fire_weapon_minimal`) : fire_mode, damage,
//!   fire_rate, range, pellets, spread, burst_count, head_damage_mul, falloff_*,
//!   shake/recoil/fov_punch (juice), hit_flash, mag/reserve/reload
//!
//! Tous les champs gameplay restent ici pour rester sur **une seule source de
//! vérité TOML par arme** (`viewmodel_arena.toml`). Si plus tard on veut une
//! API "balance-only" pour les bots IA, on pourra splitter — pas avant.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use forgia_combat::weapons::WeaponType;
use forgia_genome_core::Genome;
use serde::Deserialize;
use std::collections::HashMap;

/// Container TOML : map `clé arme → entry`. Cf `assets/genomes/viewmodel_arena.toml`.
#[derive(Deserialize, TypePath, Clone)]
pub struct ViewmodelGenome {
    pub weapons: HashMap<String, ViewmodelGenomeEntry>,
}

#[derive(Deserialize, TypePath, Clone)]
pub struct ViewmodelGenomeEntry {
    pub target_size: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
    pub rotation_y_deg: f32,
    pub fallback_scale: f32,
    #[serde(default)]
    pub rotation_x_deg: f32,
    #[serde(default)]
    pub rotation_z_deg: f32,
    /// Distance du pivot viewmodel jusqu'au bout du canon (m).
    /// Utilisé pour spawn muzzle flash + tracer à la bonne position monde.
    #[serde(default = "default_barrel_length")]
    pub barrel_length: f32,
    /// Ancres de prise PAR-ARME (story-661, mains GLB) — camera-local (m),
    /// relatives au centre viewmodel hipfire. Calibrées via les tubes MK_R/MK_L
    /// (`tools/blender/preview_ingame.py` + `read_grip_markers.py`).
    /// `None` → fallback fractions globales `[viewmodel_arms]` (fps_tuning).
    #[serde(default)]
    pub grip_anchor: Option<[f32; 3]>,
    #[serde(default)]
    pub barrel_anchor: Option<[f32; 3]>,
    /// Masque la main soutien (gauche) — armes une-main type pistolet (story-661).
    /// Défaut `false` (deux mains). Hot-reload.
    #[serde(default)]
    pub hide_support_hand: bool,
    /// Dossier des frames pixel art (relatif à `assets/`). Vide = viewmodel 3D
    /// GLB classique. Renseigné → l'arme est rendue en sprite sur un quad, et les
    /// bras 3D sont masqués (la main fait partie du sprite).
    ///
    /// Aucune DURÉE d'animation ici : les clips se calent sur les valeurs
    /// gameplay qui existent déjà (`reload_time_secs`, `fire_rate`). Une durée
    /// d'anim écrite à part serait la même grandeur écrite deux fois, et les
    /// deux finiraient par diverger au premier passage de balance.
    #[serde(default)]
    pub sprite_dir: String,
    /// Nombre de frames du clip de tir (fichiers `<arme>_fire_NN.png`).
    #[serde(default)]
    pub sprite_fire_frames: usize,
    /// Nombre de frames du clip de rechargement (`<arme>_reload_NN.png`).
    #[serde(default)]
    pub sprite_reload_frames: usize,
    /// Frames du clip de repos. > 1 → l'arme s'anime au repos (Pépin cligne des
    /// yeux). C'est la seule durée d'animation stockée ici : contrairement au
    /// rechargement et au tir, aucune valeur de gameplay ne porte le rythme d'un
    /// battement de paupière.
    #[serde(default = "default_sprite_idle_frames")]
    pub sprite_idle_frames: usize,
    #[serde(default = "default_sprite_idle_secs")]
    pub sprite_idle_secs: f32,
    /// Frames du clip de VISÉE. L'arme y est vue plein dos, dans l'axe du canon
    /// (convention CoD) — ce n'est pas le sprite de hanche recadré, c'est la même
    /// arme regardée d'ailleurs, donc un clip à part.
    #[serde(default)]
    pub sprite_ads_frames: usize,
    /// Roulis PAR-ARME (deg) autour de l'axe avant-bras, ajouté à la pose bakée →
    /// oriente la paume par-arme sans re-baker le GLB (story-661). Défaut 0.
    /// `grip_` = main crosse (droite), `barrel_` = main soutien (gauche).
    #[serde(default)]
    pub grip_roll_deg: f32,
    #[serde(default)]
    pub barrel_roll_deg: f32,
    #[serde(default = "default_ads_offset_x")]
    pub ads_offset_x: f32,
    #[serde(default = "default_ads_offset_y")]
    pub ads_offset_y: f32,
    #[serde(default = "default_ads_offset_z")]
    pub ads_offset_z: f32,
    #[serde(default = "default_ads_fov")]
    pub ads_fov_deg: f32,
    /// Position du SIGHT/scope dans le mesh LOCAL space (avant rotation/scale auto).
    /// Quand ADS, on calcule la translation viewmodel pour que ce point se projette
    /// pile sur l'axe de vision cam → red dot aligné au viseur de l'arme (style CoD).
    /// Si laissé à 0 → fallback `ads_offset_*` manuel.
    #[serde(default)]
    pub sight_local_x: f32,
    #[serde(default)]
    pub sight_local_y: f32,
    #[serde(default)]
    pub sight_local_z: f32,
    #[serde(default = "default_sight_distance")]
    pub sight_distance: f32,
    #[serde(default = "default_ads_move_speed_factor")]
    pub ads_move_speed_factor: f32,
    #[serde(default = "default_scope_glass_alpha_ads")]
    pub scope_glass_alpha_ads: f32,
    /// Tilt additionnel Y en hipfire (style CoD "carry angle"). En ADS, slerp vers 0.
    #[serde(default = "default_hipfire_tilt_y_deg")]
    pub hipfire_tilt_y_deg: f32,
    // ─── Fire behavior (genome-driven gameplay) ─────────────
    #[serde(default = "default_fire_mode")]
    pub fire_mode: String,
    #[serde(default = "default_burst_count")]
    pub burst_count: u8,
    #[serde(default = "default_damage")]
    pub damage: f32,
    #[serde(default = "default_fire_rate")]
    pub fire_rate: f32,
    #[serde(default = "default_range")]
    pub range: f32,
    #[serde(default = "default_pellets")]
    pub pellets: u8,
    #[serde(default = "default_spread_deg")]
    pub spread_deg: f32,
    // ─── Sniper scope fullscreen ─────────────────────────────
    #[serde(default)]
    pub sniper_scope_fullscreen: bool,
    // ─── ADS scale shrink ────────────────────────────────────
    #[serde(default = "default_ads_scale_factor")]
    pub ads_scale_factor: f32,
    // ─── Juice per-arme (camera shake / recoil / FOV punch) ──
    #[serde(default = "default_shake_trauma")]
    pub shake_trauma: f32,
    #[serde(default = "default_recoil_pitch_deg")]
    pub recoil_pitch_deg: f32,
    #[serde(default = "default_recoil_yaw_random_deg")]
    pub recoil_yaw_random_deg: f32,
    #[serde(default = "default_fov_punch_deg")]
    pub fov_punch_deg: f32,
    // ─── TTK Balance Overwatch-style ─────────────────────────
    #[serde(default = "default_head_damage_mul")]
    pub head_damage_mul: f32,
    #[serde(default = "default_damage_falloff_start")]
    pub damage_falloff_start: f32,
    #[serde(default = "default_damage_falloff_end")]
    pub damage_falloff_end: f32,
    #[serde(default = "default_damage_falloff_min")]
    pub damage_falloff_min: f32,
    // ─── ADS visibility & feel ───────────────────────────────
    #[serde(default = "default_ads_viewmodel_fade_alpha")]
    pub ads_viewmodel_fade_alpha: f32,
    #[serde(default = "default_ads_mouse_sensitivity_factor")]
    pub ads_mouse_sensitivity_factor: f32,
    // ─── Hit feedback timings ────────────────────────────────
    #[serde(default = "default_hit_flash_duration")]
    pub hit_flash_duration: f32,
    // ─── Ammo / Reload ──────────────────────────────────────
    #[serde(default = "default_mag_size")]
    pub mag_size: u32,
    #[serde(default = "default_reserve_max")]
    pub reserve_max: u32,
    #[serde(default = "default_reload_time_secs")]
    pub reload_time_secs: f32,
    #[serde(default = "default_reload_kind")]
    pub reload_kind: String,
    #[serde(default)]
    pub infinite_ammo: bool,
    #[serde(default = "default_low_ammo_threshold")]
    pub low_ammo_threshold: f32,
}

fn default_barrel_length() -> f32 {
    0.55
}
fn default_ads_offset_x() -> f32 {
    0.0
}
fn default_ads_offset_y() -> f32 {
    -0.10
}
fn default_ads_offset_z() -> f32 {
    -0.30
}
fn default_ads_fov() -> f32 {
    25.0
}
fn default_sight_distance() -> f32 {
    0.35
}
fn default_ads_move_speed_factor() -> f32 {
    0.65
}
fn default_scope_glass_alpha_ads() -> f32 {
    0.25
}
fn default_hipfire_tilt_y_deg() -> f32 {
    5.0
}
fn default_fire_mode() -> String {
    "auto".to_string()
}
fn default_burst_count() -> u8 {
    3
}
fn default_damage() -> f32 {
    25.0
}
fn default_fire_rate() -> f32 {
    10.0
}
fn default_range() -> f32 {
    100.0
}
fn default_pellets() -> u8 {
    1
}
fn default_spread_deg() -> f32 {
    0.0
}
fn default_hit_flash_duration() -> f32 {
    0.15
}
fn default_ads_viewmodel_fade_alpha() -> f32 {
    0.4
}
fn default_ads_mouse_sensitivity_factor() -> f32 {
    0.7
}
fn default_head_damage_mul() -> f32 {
    1.5
}
fn default_damage_falloff_start() -> f32 {
    30.0
}
fn default_damage_falloff_end() -> f32 {
    80.0
}
fn default_damage_falloff_min() -> f32 {
    0.6
}
fn default_shake_trauma() -> f32 {
    0.06
}
fn default_recoil_pitch_deg() -> f32 {
    0.4
}
fn default_recoil_yaw_random_deg() -> f32 {
    0.1
}
fn default_fov_punch_deg() -> f32 {
    0.0
}
fn default_ads_scale_factor() -> f32 {
    0.7
}
fn default_mag_size() -> u32 {
    30
}
fn default_reserve_max() -> u32 {
    120
}
fn default_sprite_idle_frames() -> usize {
    1
}

fn default_sprite_idle_secs() -> f32 {
    3.5
}

fn default_reload_time_secs() -> f32 {
    1.8
}
fn default_reload_kind() -> String {
    "mag".to_string()
}
fn default_low_ammo_threshold() -> f32 {
    0.25
}

/// Handle persistant vers le Genome chargé au Startup.
#[derive(Resource)]
pub struct ViewmodelGenomeHandle(pub Handle<Genome<ViewmodelGenome>>);

/// Bundle SystemParam pour lookup genome viewmodel (réduit le param count des systèmes
/// hot-path qui frôlent la limite Bevy 16).
#[derive(SystemParam)]
pub struct ViewmodelGenomeCtx<'w> {
    pub handle: Option<Res<'w, ViewmodelGenomeHandle>>,
    pub assets: Res<'w, Assets<Genome<ViewmodelGenome>>>,
}

impl ViewmodelGenomeCtx<'_> {
    pub fn entry(&self, w: WeaponType) -> Option<&ViewmodelGenomeEntry> {
        let h = self.handle.as_deref()?;
        lookup_genome_entry(&self.assets, h, w)
    }
}

/// Map `WeaponType` → clé TOML (`[weapons.<key>]`).
///
/// Mapping Arena V1 (legacy enum → arme V2 réelle) :
/// - `ModernAR` → `pepin` (pistolet semi)
/// - `AssaultRifle` → `bourrasque` (SMG full-auto)
/// - `Shotgun` → `madame_lenoir` (sniper V2)
/// - `RocketLauncher` → `boucherie` (shotgun pump V2)
///
/// La table elle-même vit désormais sur `WeaponType` (`forgia-combat`) : elle
/// avait deux copies, et l'arme tenue en main à la 3ᵉ personne en réclamait une
/// troisième. Cette fonction reste pour ses appelants, et délègue.
pub fn weapon_genome_key(w: WeaponType) -> &'static str {
    w.genome_key()
}

/// Lit l'entry genome pour `w`. None si genome pas encore chargé OU clé absente.
pub fn lookup_genome_entry<'a>(
    genome_assets: &'a Assets<Genome<ViewmodelGenome>>,
    handle: &ViewmodelGenomeHandle,
    w: WeaponType,
) -> Option<&'a ViewmodelGenomeEntry> {
    let g = genome_assets.get(&handle.0)?;
    g.data.weapons.get(weapon_genome_key(w))
}

/// Startup : load le genome viewmodel TOML.
/// Hot-reload Bevy natif via Shift+F12 ou save TOML.
pub fn load_viewmodel_genome(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<ViewmodelGenome>> = asset_server.load("genomes/viewmodel_arena.toml");
    commands.insert_resource(ViewmodelGenomeHandle(handle));
    info!("[forgia-viewmodel] genome loading : genomes/viewmodel_arena.toml");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_genome_key_arena_v1_distinct() {
        // Les 4 armes V1 doivent mapper à des clés distinctes.
        let p = weapon_genome_key(WeaponType::ModernAR);
        let b = weapon_genome_key(WeaponType::AssaultRifle);
        let l = weapon_genome_key(WeaponType::Shotgun);
        let bo = weapon_genome_key(WeaponType::RocketLauncher);
        assert_eq!(p, "pepin");
        assert_eq!(b, "bourrasque");
        assert_eq!(l, "madame_lenoir");
        assert_eq!(bo, "boucherie");
        // Sanity : pas de doublon
        let all = [p, b, l, bo];
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "clés genome doivent être distinctes");
                }
            }
        }
    }

    #[test]
    fn defaults_within_safe_ranges() {
        // Si TOML omet un field, défauts doivent rendre l'arme jouable.
        assert!(default_damage() > 0.0);
        assert!(default_fire_rate() > 0.0);
        assert!(default_range() > 0.0);
        assert!((0.0..=1.0).contains(&default_ads_scale_factor()));
        assert!((0.0..=1.0).contains(&default_ads_viewmodel_fade_alpha()));
        assert!((0.0..=1.0).contains(&default_low_ammo_threshold()));
        assert_eq!(default_fire_mode(), "auto");
        assert_eq!(default_reload_kind(), "mag");
    }
}

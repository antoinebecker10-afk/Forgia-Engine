//! # forgia-fps
//!
//! Mode FPS Arena — assets KayKit Dungeon Pack.
//!
//! Pattern V1 :
//! - KayKit walls **`WALL_Y = 0.0`** (LOCK absolu : pivot mesh au sol, pas centre)
//! - `TILE_SIZE = 4.0` (KayKit dungeon convention)
//! - Forgia scaled scene pattern : parent scale=1 + child SceneRoot scale (rapier3d 0.33 quirk)

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::*;
use forgia_combat::weapons::{EquippedWeapons, WeaponFireCooldown, WeaponType};
use forgia_core::prelude::*;
use forgia_effects::prelude::{
    spawn_hitscan_tracer, spawn_impact_vfx, spawn_muzzle_flash, TracerResources,
    WeaponVfxEffects,
};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_crosshair::CrosshairTuning;
use forgia_juice_camera_shake::{
    CameraShake, CameraShakeTuning, ForgiaJuiceCameraShakePlugin, ShakeImpulse,
};
use forgia_juice_fov_punch::{ForgiaJuiceFovPunchPlugin, FovPunchImpulse, FovPunchTuning};
use forgia_juice_hit_stop::HitStopState;
use forgia_juice_recoil::{ForgiaJuiceRecoilPlugin, WeaponRecoilImpulse};
use forgia_player::prelude::MouseLookTuning;
use forgia_mode_fps_arena::TargetCube;
use forgia_player::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

mod ads;
mod ammo_systems;
mod hitscan_sensor;
pub use hitscan_sensor::{HitscanCategory, HitscanLogEntry, HitscanSensorState};
mod score;
mod scope_glass;

pub mod prelude {
    pub use crate::ForgiaFpsPlugin;
    pub use crate::score::{ArenaScore, ArenaScorePlugin, ScoreboardVisible};
}

/// État du clic gauche pour dispatch fire_mode (V2 : ButtonInput consommé par egui →
/// tracking via MessageReader<MouseButtonInput>).
/// `just_pressed` = transition Released→Pressed cette frame (mode semi/pump/burst).
/// `held` = bouton actuellement enfoncé (mode auto).
#[derive(Resource, Default)]
pub struct LeftMouseState {
    pub just_pressed: bool,
    pub held: bool,
}

/// État d'une rafale en cours (fire_mode = "burst").
/// Inséré au just_pressed initial, retiré quand `shots_remaining == 0`.
/// Pendant qu'il existe, le cooldown standard est bypassé — le timer interne pilote la cadence.
#[derive(Resource)]
pub struct BurstState {
    /// Nombre de shots restants à tirer dans la rafale (le 1er tir est immédiat au just_pressed).
    pub shots_remaining: u8,
    /// Timer entre shots (1/fire_rate).
    pub interval_timer: Timer,
}

/// Viewmodel 1P enfant de FpsCamera. Stocke l'arme actuellement rendue
/// pour détecter les changements et swap le SceneRoot.
#[derive(Component)]
pub struct WeaponViewmodel {
    pub current: WeaponType,
}

/// Scene handles pré-chargés pour les 4 armes V1 Arena (load_weapon_models au Startup).
/// Slot 1 (ModernAR)     = Pépin         (~17MB)
/// Slot 2 (AssaultRifle) = Bourrasque    (~18MB)
/// Slot 3 (Shotgun)      = Madame Lenoir (~18MB)
/// Slot 4 (Rocket)       = Boucherie     (~17MB)
/// Tous dans `assets/models/weapons/forgia/`.
#[derive(Resource)]
pub struct WeaponModelAssets {
    pub pepin: Handle<Scene>,
    pub bourrasque: Handle<Scene>,
    pub madame_lenoir: Handle<Scene>,
    pub boucherie: Handle<Scene>,
}

// ============================================================================
// Genome viewmodel — config TOML hot-reloadable (assets/genomes/viewmodel_arena.toml)
// ============================================================================

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
    // Axes rotation supplémentaires (défaut 0 si absent dans TOML — option = backward compat).
    #[serde(default)]
    pub rotation_x_deg: f32,
    #[serde(default)]
    pub rotation_z_deg: f32,
    /// Distance du pivot viewmodel jusqu'au bout du canon (m).
    /// Utilisé pour spawn muzzle flash + tracer à la bonne position monde.
    #[serde(default = "default_barrel_length")]
    pub barrel_length: f32,
    /// Position viewmodel en ADS (Aim Down Sight, clic droit).
    /// Sight aligné centre écran → souvent X≈0, Y un peu plus haut, Z plus proche cam.
    #[serde(default = "default_ads_offset_x")]
    pub ads_offset_x: f32,
    #[serde(default = "default_ads_offset_y")]
    pub ads_offset_y: f32,
    #[serde(default = "default_ads_offset_z")]
    pub ads_offset_z: f32,
    /// FOV en degrés quand ADS actif (smaller = more zoom).
    #[serde(default = "default_ads_fov")]
    pub ads_fov_deg: f32,
    /// Position du SIGHT/scope dans le mesh LOCAL space (avant rotation/scale auto).
    /// Quand ADS, on calcule la translation viewmodel pour que ce point se projette
    /// pile sur l'axe de vision cam → red dot aligné au viseur de l'arme (style CoD).
    /// Si laissé à 0 → fallback ads_offset_* manuel.
    #[serde(default)]
    pub sight_local_x: f32,
    #[serde(default)]
    pub sight_local_y: f32,
    #[serde(default)]
    pub sight_local_z: f32,
    /// Distance cam → sight en world units (m). Default 0.35m typique FPS.
    #[serde(default = "default_sight_distance")]
    pub sight_distance: f32,
    /// Facteur vitesse de déplacement en ADS. 1.0 = pas de ralenti, 0.65 = -35% (style CoD).
    #[serde(default = "default_ads_move_speed_factor")]
    pub ads_move_speed_factor: f32,
    /// Alpha de la lentille du scope en full ADS (0.0 totalement transparent, 1.0 opaque).
    /// Default 0.25 = visible mais see-through. Forgia-mesh-fader gère le lerp.
    #[serde(default = "default_scope_glass_alpha_ads")]
    pub scope_glass_alpha_ads: f32,
    /// Tilt additionnel sur Y appliqué UNIQUEMENT en hipfire (style CoD "carry angle").
    /// En ADS, l'arme se redresse (pas de tilt) pour être centrée dans le viseur.
    /// Slerp Quat lerp progress 0→1 entre hipfire (rotation_y + tilt) et ADS (rotation_y).
    #[serde(default = "default_hipfire_tilt_y_deg")]
    pub hipfire_tilt_y_deg: f32,
    // ─── Phase B — Fire behavior (genome-driven gameplay) ─────────────
    /// "semi" | "auto" | "burst" | "pump". Default "auto".
    #[serde(default = "default_fire_mode")]
    pub fire_mode: String,
    /// Nombre de tirs par rafale en mode "burst". Default 3.
    #[serde(default = "default_burst_count")]
    pub burst_count: u8,
    /// Dégâts par projectile (single ray ou par pellet pour shotgun cone).
    #[serde(default = "default_damage")]
    pub damage: f32,
    /// Cadence de tir (shots/seconde). Cooldown = 1/fire_rate.
    #[serde(default = "default_fire_rate")]
    pub fire_rate: f32,
    /// Portée max raycast (m).
    #[serde(default = "default_range")]
    pub range: f32,
    /// Nombre de pellets par tir. 1 = single ray. >1 = cone spread (shotgun/pump).
    #[serde(default = "default_pellets")]
    pub pellets: u8,
    /// Angle cone spread (degrés) pour multi-pellets. 0 = perfect accuracy.
    #[serde(default = "default_spread_deg")]
    pub spread_deg: f32,
    // ─── Phase C — Sniper scope fullscreen ────────────────────────────
    /// Si true, en ADS affiche un overlay fullscreen style sniper CoD
    /// (cercle noir vignette + reticle) + FOV cam très réduit.
    #[serde(default)]
    pub sniper_scope_fullscreen: bool,
    // ─── Phase H — ADS scale shrink (2026-05-18) ─────────────────────
    /// Multiplier scale viewmodel en ADS full (lerp 1.0 hipfire → factor full ADS).
    /// Sniper Lenoir = 1.0 (pas de changement, il a son scope overlay).
    /// Autres = 0.65-0.75 = gun réduit pour ne pas bloquer le crosshair.
    #[serde(default = "default_ads_scale_factor")]
    pub ads_scale_factor: f32,
    // ─── Phase G — Juice per-arme (camera shake / recoil / FOV punch) ─
    /// Trauma ajouté par tir (0..1). 0.04 = SMG léger, 0.22 = Shotgun heavy.
    /// AAA range : 0.04-0.22 pour rester confortable (anti-nausée).
    #[serde(default = "default_shake_trauma")]
    pub shake_trauma: f32,
    /// Recoil visuel pitch up par tir (degrés). 0.2-0.8° AAA standard.
    #[serde(default = "default_recoil_pitch_deg")]
    pub recoil_pitch_deg: f32,
    /// Recoil yaw random max par tir (degrés). 0 = pas de yaw kick.
    /// Determine la "wiggle" horizontale. SMG : ±0.15°, Sniper : 0 (precise).
    #[serde(default = "default_recoil_yaw_random_deg")]
    pub recoil_yaw_random_deg: f32,
    /// FOV punch peak en degrés. 0.0 = SKIP (CS2 philosophy — AR/SMG).
    /// 0.5-1.5° pour Shotgun / Sniper / LMG uniquement.
    #[serde(default = "default_fov_punch_deg")]
    pub fov_punch_deg: f32,
    // ─── Phase F — TTK Balance Overwatch-style (2026-05-18) ──────────
    /// Multiplicateur dégâts en headshot. 1.0 = pas de bonus, 2.0 = double.
    /// Sniper Lenoir = 2.0 (one-shot head). Shotgun = 1.2 (pas de bonus marqué).
    #[serde(default = "default_head_damage_mul")]
    pub head_damage_mul: f32,
    /// Distance à partir de laquelle le damage falloff commence (m). dmg = base avant.
    #[serde(default = "default_damage_falloff_start")]
    pub damage_falloff_start: f32,
    /// Distance où le falloff atteint son minimum (m). dmg = base × falloff_min après.
    #[serde(default = "default_damage_falloff_end")]
    pub damage_falloff_end: f32,
    /// Multiplicateur dmg au-delà de falloff_end. 0.2 = -80% (shotgun très penalty long).
    /// 1.0 = pas de falloff (sniper).
    #[serde(default = "default_damage_falloff_min")]
    pub damage_falloff_min: f32,
    // ─── Phase E — ADS visibility & feel (genome-driven, 2026-05-18) ──
    /// Alpha du BODY viewmodel (mesh entier, hors lentille scope) quand ADS full.
    /// 1.0 = pas de fade (sniper qui a son scope fullscreen overlay).
    /// 0.4 = semi-transparent (laisse voir à travers le canon vers la cible).
    /// Lerp via forgia-mesh-fader piloté par AdsState.progress.
    #[serde(default = "default_ads_viewmodel_fade_alpha")]
    pub ads_viewmodel_fade_alpha: f32,
    /// Multiplicateur de sensibilité souris en ADS full. 1.0 = pas de changement.
    /// Sniper Lenoir = 0.3 (visée lente précise). SMG Bourrasque = 0.7. AR = 0.7.
    /// Shotgun = 0.85 (pas besoin de précision fine sur cone spread).
    /// Lerp 1.0 → factor selon AdsState.progress.
    #[serde(default = "default_ads_mouse_sensitivity_factor")]
    pub ads_mouse_sensitivity_factor: f32,
    // ─── Phase D — Hit feedback timings (genome-driven) ───────────────
    /// Durée du flash blanc sur la cible touchée (secondes). Default 0.15s.
    #[serde(default = "default_hit_flash_duration")]
    pub hit_flash_duration: f32,
    /// Durée du hit-stop (ralenti time scale) après un hit (secondes). Default 0.05s.
    /// Snipers : 0.10s (sensation lourde). SMG : 0.03s (rapide).
    #[serde(default = "default_hit_stop_duration")]
    pub hit_stop_duration: f32,
    /// Vitesse relative du temps pendant hit-stop (0.05 = 5% = très lent). Default 0.05.
    #[serde(default = "default_hit_stop_speed")]
    pub hit_stop_speed: f32,
    // ─── Story-455 Phase A — Ammo / Reload (2026-05-18) ──────────────────
    /// Taille du chargeur (1 mag). Default 30 (rifle classique).
    /// Sniper Lenoir = 5, Shotgun Boucherie = 6, SMG Bourrasque = 35, Pistolet Pépin = 12.
    #[serde(default = "default_mag_size")]
    pub mag_size: u32,
    /// Munitions en réserve (hors mag). Default 120.
    /// Pickup loot world peut augmenter, capé à reserve_max.
    #[serde(default = "default_reserve_max")]
    pub reserve_max: u32,
    /// Durée totale d'un reload (secondes). Pour ShellPerShell = durée 1 shell.
    /// Rifle ~1.8s, SMG ~1.5s, Sniper ~2.5s, Shotgun pump per-shell ~0.4s.
    #[serde(default = "default_reload_time_secs")]
    pub reload_time_secs: f32,
    /// "mag" (batch transfer) ou "shell_per_shell" (pump shotgun). Default "mag".
    #[serde(default = "default_reload_kind")]
    pub reload_kind: String,
    /// Toggle dev/playtest : true = munitions infinies. Default false.
    #[serde(default)]
    pub infinite_ammo: bool,
    /// Seuil low-ammo (fraction mag) déclenchant flash rouge HUD. Default 0.25 (= 25%).
    #[serde(default = "default_low_ammo_threshold")]
    pub low_ammo_threshold: f32,
}

fn default_barrel_length() -> f32 { 0.55 }
fn default_ads_offset_x() -> f32 { 0.0 }
fn default_ads_offset_y() -> f32 { -0.10 }
fn default_ads_offset_z() -> f32 { -0.30 }
fn default_ads_fov() -> f32 { 25.0 }
fn default_sight_distance() -> f32 { 0.35 }
fn default_ads_move_speed_factor() -> f32 { 0.65 }
fn default_scope_glass_alpha_ads() -> f32 { 0.25 }
fn default_hipfire_tilt_y_deg() -> f32 { 5.0 }
fn default_fire_mode() -> String { "auto".to_string() }
fn default_burst_count() -> u8 { 3 }
fn default_damage() -> f32 { 25.0 }
fn default_fire_rate() -> f32 { 10.0 }
fn default_range() -> f32 { 100.0 }
fn default_pellets() -> u8 { 1 }
fn default_spread_deg() -> f32 { 0.0 }
fn default_hit_flash_duration() -> f32 { 0.15 }
fn default_hit_stop_duration() -> f32 { 0.05 }
fn default_hit_stop_speed() -> f32 { 0.05 }
fn default_ads_viewmodel_fade_alpha() -> f32 { 0.4 }
fn default_ads_mouse_sensitivity_factor() -> f32 { 0.7 }
fn default_head_damage_mul() -> f32 { 1.5 }
fn default_damage_falloff_start() -> f32 { 30.0 }
fn default_damage_falloff_end() -> f32 { 80.0 }
fn default_damage_falloff_min() -> f32 { 0.6 }
fn default_shake_trauma() -> f32 { 0.06 }
fn default_recoil_pitch_deg() -> f32 { 0.4 }
fn default_recoil_yaw_random_deg() -> f32 { 0.1 }
fn default_fov_punch_deg() -> f32 { 0.0 }
fn default_ads_scale_factor() -> f32 { 0.7 }
fn default_mag_size() -> u32 { 30 }
fn default_reserve_max() -> u32 { 120 }
fn default_reload_time_secs() -> f32 { 1.8 }
fn default_reload_kind() -> String { "mag".to_string() }
fn default_low_ammo_threshold() -> f32 { 0.25 }

#[derive(Resource)]
pub struct ViewmodelGenomeHandle(pub Handle<Genome<ViewmodelGenome>>);

/// Bundle SystemParam pour lookup genome viewmodel (réduit le param count des systèmes
/// hot-path comme `fire_weapon_minimal` qui frôlent la limite Bevy 16).
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

/// Bundle des resources timing pour fire system (cooldown + burst + time virtuels/réels).
/// Réduit le param count global de `fire_weapon_minimal` (limite Bevy 16).
#[derive(SystemParam)]
pub struct FireTimingCtx<'w> {
    pub cooldown: Option<Res<'w, WeaponFireCooldown>>,
    pub burst_state: Option<ResMut<'w, BurstState>>,
    pub time: Res<'w, Time>,
    pub virtual_time: ResMut<'w, Time<Virtual>>,
}

/// Bundle des MessageWriters juice (shake / recoil / fov punch) pour fire system.
/// Réduit param count (limite Bevy 16) et centralise les déclenchements.
#[derive(SystemParam)]
pub struct JuiceWriters<'w> {
    pub shake: MessageWriter<'w, ShakeImpulse>,
    pub recoil: MessageWriter<'w, WeaponRecoilImpulse>,
    pub fov_punch: MessageWriter<'w, FovPunchImpulse>,
}

impl JuiceWriters<'_> {
    /// Emit les 3 impulses depuis le genome de l'arme. Yaw random uniforme [-yaw_max..+yaw_max].
    /// `seed` pour PRNG yaw (pseudo-déterministe par tir).
    pub fn emit_from_genome(&mut self, e: &ViewmodelGenomeEntry, seed: u32) {
        if e.shake_trauma > 0.0 {
            self.shake.write(ShakeImpulse { trauma: e.shake_trauma });
        }
        if e.recoil_pitch_deg.abs() > 0.001 || e.recoil_yaw_random_deg.abs() > 0.001 {
            // PRNG yaw : [-1, 1] depuis pseudo_rand, scale par yaw_random_deg
            let yaw_signed = (pseudo_rand(seed) - 0.5) * 2.0 * e.recoil_yaw_random_deg;
            self.recoil.write(WeaponRecoilImpulse {
                pitch_rad: e.recoil_pitch_deg.to_radians(),
                yaw_rad: yaw_signed.to_radians(),
            });
        }
        if e.fov_punch_deg.abs() > 0.01 {
            self.fov_punch.write(FovPunchImpulse { peak_deg: e.fov_punch_deg });
        }
    }
}

/// Story-453 baseline reset (2026-05-18) — query simplifiée : ray hit le PARENT
/// directement (bot = 1 entité unique avec capsule + Health + TargetCube),
/// plus de Head/Body split, plus de ChildOf walk.
#[derive(SystemParam)]
pub struct HitApplyCtx<'w, 's> {
    pub health: Query<
        'w,
        's,
        (
            &'static mut Health,
            Option<&'static MeshMaterial3d<StandardMaterial>>,
        ),
        With<TargetCube>,
    >,
}

/// Bundle hitscan diagnostic — q_children pour predicate récursif + sensor state.
/// Évite de dépasser la limite Bevy 16 params dans `fire_weapon_minimal`.
#[derive(SystemParam)]
pub struct HitscanCtx<'w, 's> {
    pub q_children: Query<'w, 's, &'static Children>,
    pub q_child_of: Query<'w, 's, &'static ChildOf>,
    pub q_name: Query<'w, 's, &'static Name>,
    pub sensor: ResMut<'w, HitscanSensorState>,
}

/// Multiplicateur damage falloff selon distance. Linéaire entre start et end.
/// Avant start = 1.0, après end = falloff_min.
pub fn falloff_multiplier(toi: f32, e: &ViewmodelGenomeEntry) -> f32 {
    if toi <= e.damage_falloff_start {
        return 1.0;
    }
    if toi >= e.damage_falloff_end {
        return e.damage_falloff_min;
    }
    let span = (e.damage_falloff_end - e.damage_falloff_start).max(0.001);
    let t = ((toi - e.damage_falloff_start) / span).clamp(0.0, 1.0);
    1.0_f32.lerp(e.damage_falloff_min, t)
}

/// Map WeaponType → clé TOML (`[weapons.<key>]`).
fn weapon_genome_key(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "pepin",
        WeaponType::AssaultRifle => "bourrasque",
        WeaponType::Shotgun => "madame_lenoir",
        WeaponType::RocketLauncher => "boucherie",
        _ => "pepin",
    }
}

/// Lit l'entrée genome pour `w`. Si genome pas encore chargé OU clé absente, retourne None
/// → fallback hardcodé `viewmodel_transform` / `viewmodel_target_size` / `viewmodel_fallback_scale`.
fn lookup_genome_entry<'a>(
    genome_assets: &'a Assets<Genome<ViewmodelGenome>>,
    handle: &ViewmodelGenomeHandle,
    w: WeaponType,
) -> Option<&'a ViewmodelGenomeEntry> {
    let g = genome_assets.get(&handle.0)?;
    g.data.weapons.get(weapon_genome_key(w))
}

fn scene_for_weapon(a: &WeaponModelAssets, w: WeaponType) -> Handle<Scene> {
    match w {
        WeaponType::ModernAR => a.pepin.clone(),
        WeaponType::AssaultRifle => a.bourrasque.clone(),
        WeaponType::Shotgun => a.madame_lenoir.clone(),
        WeaponType::RocketLauncher => a.boucherie.clone(),
        _ => a.pepin.clone(),
    }
}

/// Marker : viewmodel attend la mesure AABB pour calculer son scale réel.
/// Pattern porté de V1 `combat/viewmodel.rs:603` (auto_scale_system).
#[derive(Component)]
pub struct NeedsAutoScale {
    pub target_size: f32, // taille cible en mètres (largest axis)
}

/// Scale "de base" du viewmodel après auto-calibration AABB (hipfire).
/// Lu par `apply_ads_viewmodel` pour lerp scale en ADS sans drift par frame.
/// Phase H 2026-05-18 : ADS scale shrink genome-driven.
#[derive(Component, Debug, Clone, Copy)]
pub struct ViewmodelBaseScale(pub f32);

// ─── FPS Tuning Genome (anti-hardcode pass 2026-05-18) ────────────────
//
// Toutes les constantes "feel" qui étaient hardcodées dans les crates juice /
// crosshair / player / ads. Loadées au Startup depuis `assets/genomes/fps_tuning.toml`
// et propagées par `sync_fps_tuning` à chaque downstream Resource Tuning.

#[derive(Deserialize, TypePath, Clone)]
pub struct FpsTuning {
    pub camera_shake: FtCameraShake,
    pub fov_punch: FtFovPunch,
    pub ads: FtAds,
    pub mouse_look: FtMouseLook,
    pub crosshair_hipfire: FtCrosshairHipfire,
    pub crosshair_ads_dot: FtCrosshairAdsDot,
    pub crosshair_sniper_overlay: FtCrosshairSniper,
}

#[derive(Deserialize, Clone)]
pub struct FtCameraShake {
    pub default_decay: f32,
    pub default_max_rotation_rad: f32,
    pub yaw_factor: f32,
    pub roll_factor: f32,
    pub pitch_upward_bias: f32,
    pub sample_rate_hz: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtFovPunch {
    pub attack_secs: f32,
    pub decay_secs: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtAds {
    pub lerp_speed: f32,
    pub default_fov_deg: f32,
    pub punch_attenuation: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtMouseLook {
    pub base_sensitivity: f32,
    pub recoil_decay_per_sec: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairHipfire {
    pub cross_len: f32,
    pub cross_stroke: f32,
    pub cross_alpha: u8,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairAdsDot {
    pub outer_radius: f32,
    pub inner_radius: f32,
}

#[derive(Deserialize, Clone)]
pub struct FtCrosshairSniper {
    pub scope_radius_factor: f32,
    pub dim_alpha: u8,
    pub dim_color: [u8; 3],
    pub vignette_rings: u32,
    pub ring_thickness_factor: f32,
    pub ring_max_alpha: f32,
    pub ring_color: [u8; 3],
    pub ring_outer_extent: f32,
    pub border_width: f32,
    pub border_inner_width: f32,
    pub border_inner_offset: f32,
    pub reticle_gap: f32,
    pub reticle_line_factor: f32,
    pub reticle_color: [u8; 3],
    pub reticle_line_stroke: f32,
    pub reticle_tick_stroke: f32,
    pub reticle_tick_size: f32,
    pub red_dot_radius: f32,
    pub red_dot_color: [u8; 3],
}

#[derive(Resource)]
pub struct FpsTuningHandle(pub Handle<Genome<FpsTuning>>);

/// Resource locale forgia-fps : constantes ADS (FOV cam, lerp speed, atténuation).
/// Lu par `update_ads_progress` + `apply_ads_camera_fov`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct AdsTuning {
    pub lerp_speed: f32,
    pub default_fov_deg: f32,
    pub punch_attenuation: f32,
}

impl Default for AdsTuning {
    fn default() -> Self {
        Self {
            lerp_speed: 12.0,
            default_fov_deg: 45.0,
            punch_attenuation: 0.7,
        }
    }
}

/// Offset + rotation du viewmodel par arme (le scale vient de auto_scale_viewmodel via AABB).
/// Valeurs portées de V1 `combat/viewmodel.rs` (fallback genome). Scale 1.0 = sera réécrit.
///
/// Note rotation : GLB face -X par défaut → rotation Y +90° pour aligner canon vers -Z (forward).
/// Transform local du viewmodel. Si `genome` fourni, ses valeurs ont priorité.
/// Sinon fallback hardcodé (boot avant chargement TOML, ou clé absente).
/// Rotation hipfire = rotation base + tilt Y additionnel ("carry angle" CoD).
pub fn viewmodel_rotation_hipfire(g: &ViewmodelGenomeEntry) -> Quat {
    Quat::from_rotation_x(g.rotation_x_deg.to_radians())
        * Quat::from_rotation_y((g.rotation_y_deg + g.hipfire_tilt_y_deg).to_radians())
        * Quat::from_rotation_z(g.rotation_z_deg.to_radians())
}

/// Rotation ADS = rotation base (sans tilt, arme droite centrée viseur).
pub fn viewmodel_rotation_ads(g: &ViewmodelGenomeEntry) -> Quat {
    Quat::from_rotation_x(g.rotation_x_deg.to_radians())
        * Quat::from_rotation_y(g.rotation_y_deg.to_radians())
        * Quat::from_rotation_z(g.rotation_z_deg.to_radians())
}

fn viewmodel_transform(w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> Transform {
    if let Some(g) = genome {
        return Transform {
            translation: Vec3::new(g.offset_x, g.offset_y, g.offset_z),
            rotation: viewmodel_rotation_hipfire(g), // hipfire avec tilt par défaut
            scale: Vec3::splat(1.0),
        };
    }
    // Fallback hardcodé (boot précoce, genome TOML pas encore loaded).
    let base_rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let offset = match w {
        WeaponType::ModernAR => Vec3::new(0.25, -0.30, -0.65),
        WeaponType::Shotgun => Vec3::new(0.30, -0.35, -0.60),
        WeaponType::RocketLauncher => Vec3::new(0.35, -0.40, -0.70),
        WeaponType::AK47 => Vec3::new(0.32, -0.38, -0.70),
        WeaponType::AssaultRifle => Vec3::new(0.35, -0.35, -0.65),
        WeaponType::PlasmaRifle => Vec3::new(0.35, -0.35, -0.65),
        WeaponType::Chainsaw => Vec3::new(0.30, -0.30, -0.65),
    };
    Transform {
        translation: offset,
        rotation: base_rot,
        scale: Vec3::splat(1.0),
    }
}

fn viewmodel_target_size(w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> f32 {
    if let Some(g) = genome {
        return g.target_size;
    }
    match w {
        WeaponType::Shotgun => 0.8,
        WeaponType::Chainsaw => 0.6,
        _ => 0.75,
    }
}

fn viewmodel_fallback_scale(_w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> f32 {
    genome.map(|g| g.fallback_scale).unwrap_or(1.0)
}

pub struct ForgiaFpsPlugin;

impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        // Add MeshFaderPlugin si pas déjà ajouté (idempotent — plusieurs crates peuvent l'utiliser)
        if !app.is_plugin_added::<forgia_mesh_fader::MeshFaderPlugin>() {
            app.add_plugins(forgia_mesh_fader::MeshFaderPlugin);
        }
        // Arena spawn/cleanup + clouds — crate dédié (règle fine-grained-crates).
        if !app.is_plugin_added::<forgia_mode_fps_arena::ForgiaModeFpsArenaPlugin>() {
            app.add_plugins(forgia_mode_fps_arena::ForgiaModeFpsArenaPlugin);
        }
        // Juice plugins (idempotent — checks anti double-add).
        if !app.is_plugin_added::<ForgiaJuiceCameraShakePlugin>() {
            app.add_plugins(ForgiaJuiceCameraShakePlugin);
        }
        if !app.is_plugin_added::<ForgiaJuiceFovPunchPlugin>() {
            app.add_plugins(ForgiaJuiceFovPunchPlugin);
        }
        if !app.is_plugin_added::<ForgiaJuiceRecoilPlugin>() {
            app.add_plugins(ForgiaJuiceRecoilPlugin);
        }
        app.add_plugins((
                score::ArenaScorePlugin,
                ads::AdsPlugin,
                scope_glass::ScopeGlassPlugin,
            ))
            .init_resource::<EquippedWeapons>()
            .init_resource::<LeftMouseState>()
            .init_resource::<AdsTuning>()
            .init_resource::<HitscanSensorState>()
            .add_systems(Update, hitscan_sensor::write_hitscan_sensor)
            .init_asset::<Genome<ViewmodelGenome>>()
            .init_asset::<Genome<FpsTuning>>()
            .register_asset_loader(GenomeLoader::<ViewmodelGenome>::default())
            .register_asset_loader(GenomeLoader::<FpsTuning>::default())
            .add_systems(
                Startup,
                (load_weapon_models, load_viewmodel_genome, load_fps_tuning),
            )
            .add_systems(Update, sync_fps_tuning)
            .add_systems(OnExit(GameMode::Fps), despawn_viewmodel)
            // Fire system genome-driven : dispatch fire_mode (auto/semi/pump) + multi-pellets
            // + per-weapon damage/fire_rate/range/spread depuis ViewmodelGenomeEntry TOML.
            // Reconstruit 2026-05-17 depuis memories après perte WIP 2026-05-16 PM.
            // Limitations : fire_mode "burst" NON implémenté (fallback semi + warn).
            // Story-455 Phase A — sync_ammo_slots_from_genome NE DOIT PAS être gated Fps.
            // Le genome ViewmodelGenome se charge au Startup → AssetEvent::Added arrive
            // ~50-200ms après boot, alors que GameMode = None (menu). Si gated Fps, l'event
            // expire du buffer Bevy avant que le user entre Arena → slots jamais peuplés
            // (sensor `forgia_hud_ammo.json: slots: []` observé runtime 2026-05-18).
            //
            // Fix : sync system tourne en permanence (idempotent, no-op si handle absent).
            // Les autres 3 ammo systems restent gated Fps (input/state runtime gameplay).
            .add_systems(Update, ammo_systems::sync_ammo_slots_from_genome)
            .add_systems(
                Update,
                (
                    track_left_mouse_state,
                    weapon_select_system,
                    ammo_systems::cancel_reload_on_weapon_switch,
                    ammo_systems::reload_key_input,
                    ammo_systems::tick_ammo_reload,
                    fire_weapon_minimal,
                    despawn_dead_cubes,
                )
                    .chain()
                    .in_set(GameSet::Combat)
                    .run_if(in_state(GameMode::Fps)),
            )
            .add_systems(
                Update,
                (
                    attach_viewmodel_to_camera,
                    update_viewmodel_on_switch,
                    auto_scale_viewmodel,
                    ensure_camera_shake_component,
                )
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}

/// PRNG pseudo-déterministe ultra-léger (xorshift32). Out [0, 1).
/// Helper de fire_weapon_minimal multi-pellets (stubbed — voir TODO plugin).
#[allow(dead_code)]
fn pseudo_rand(seed: u32) -> f32 {
    let mut x = seed.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32) / (u32::MAX as f32)
}

/// Walk ChildOf ancestors max `max_depth` niveaux pour trouver l'entité qui porte
/// `Health` (typiquement le parent TargetCube d'un bot — story-453 architecture
/// `AsyncSceneCollider ConvexHull` : ray hit child mesh, parent porte Health).
///
/// Utilisé par :
/// - `fire_weapon_minimal` damage path (applique dégâts au parent)
/// - `fire_weapon_minimal` sensor categorization (BUG-RUN-1 fix story-455)
///
/// Return : Some(parent_entity) si trouvé ; None si la chaîne se termine sans Health.
fn find_health_ancestor(
    hit_entity: Entity,
    q_child_of: &Query<&ChildOf>,
    health_query: &Query<
        (&mut Health, Option<&MeshMaterial3d<StandardMaterial>>),
        With<TargetCube>,
    >,
    max_depth: u32,
) -> Option<Entity> {
    let mut current = hit_entity;
    for _ in 0..max_depth {
        if health_query.get(current).is_ok() {
            return Some(current);
        }
        match q_child_of.get(current) {
            Ok(co) => current = co.parent(),
            Err(_) => return None,
        }
    }
    None
}

/// Despawn les cubes morts (HP=0). Système séparé chained après fire.
fn despawn_dead_cubes(
    mut commands: Commands,
    q: Query<(Entity, &Health), With<TargetCube>>,
) {
    for (entity, hp) in &q {
        if hp.is_dead() {
            // try_despawn (Bevy 0.18) idempotent — silent si entity déjà
            // queued for despawn par un autre system (cf race conditions
            // bot death + hp floater UI + damage numbers).
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            info!("[death] cube {:?} despawned (HP=0)", entity);
        }
    }
}

/// Switch arme via Digit1-4 (Pépin / Bourrasque / Madame Lenoir / Boucherie).
/// Reconstruction minimale 2026-05-17 après perte du WIP fire system (session 2026-05-16 PM).
/// Mapping : `forgia_combat::weapons::ARENA_V1_WEAPONS[idx]` (ModernAR/AssaultRifle/Shotgun/RocketLauncher).
fn weapon_select_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut equipped: ResMut<EquippedWeapons>,
) {
    let new_idx: Option<usize> = if keys.just_pressed(KeyCode::Digit1) {
        Some(0)
    } else if keys.just_pressed(KeyCode::Digit2) {
        Some(1)
    } else if keys.just_pressed(KeyCode::Digit3) {
        Some(2)
    } else if keys.just_pressed(KeyCode::Digit4) {
        Some(3)
    } else {
        None
    };
    if let Some(i) = new_idx {
        let target = forgia_combat::weapons::ARENA_V1_WEAPONS[i];
        if equipped.current != target {
            equipped.current = target;
            info!("[forgia-fps] weapon_select : Digit{} → {:?}", i + 1, target);
        }
    }
}

/// Maintien `LeftMouseState` (held + just_pressed) via MessageReader<MouseButtonInput>.
/// Doit run AVANT `fire_weapon_minimal` dans le chain.
fn track_left_mouse_state(
    mut evs: MessageReader<MouseButtonInput>,
    mut state: ResMut<LeftMouseState>,
) {
    state.just_pressed = false; // reset chaque frame
    for ev in evs.read() {
        if ev.button == MouseButton::Left {
            match ev.state {
                ButtonState::Pressed => {
                    state.held = true;
                    state.just_pressed = true;
                }
                ButtonState::Released => {
                    state.held = false;
                }
            }
        }
    }
}

/// Fire system genome-driven (Forgia V2 — reconstruit 2026-05-17 depuis memories
/// `reference_v2_fire_modes_genome_driven` après perte WIP 2026-05-16 PM).
///
/// Dispatch via `ViewmodelGenomeEntry.fire_mode` :
/// - `"auto"` : tire tant que held (Bourrasque SMG)
/// - `"semi"` : tire UNIQUEMENT sur just_pressed (Pépin, Madame Lenoir sniper)
/// - `"pump"` : just_pressed + multi-pellets cone spread (Boucherie shotgun)
/// - `"burst"` : ⚠ NON IMPLÉMENTÉ — fallback semi + log warn (dette tech identifiée)
///
/// Cooldown = `1.0 / entry.fire_rate` secondes. Damage/range/pellets/spread depuis genome.
/// Muzzle flash spawn à `origin + direction * entry.barrel_length`.
/// Multi-pellets : xorshift32 PRNG déterministe seed=position+pellet_idx pour reproducibility.
#[allow(clippy::too_many_arguments)]
fn fire_weapon_minimal(
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    mut hit_ctx: HitApplyCtx,
    mut commands: Commands,
    flash_cache: Res<HitFlashCache>,
    tracer_res: Option<Res<TracerResources>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
    mut timing: FireTimingCtx,
    mut hit_events: MessageWriter<CombatHitEvent>,
    mut juice: JuiceWriters,
    left: Res<LeftMouseState>,
    mut ammo: ammo_systems::AmmoCtx,
    genome_ctx: ViewmodelGenomeCtx,
    ads: Res<ads::AdsState>,
    mut hitscan_ctx: HitscanCtx,
) {
    let entry = genome_ctx.entry(ammo.equipped.current);
    let fire_mode = entry.map(|e| e.fire_mode.as_str()).unwrap_or("auto");
    let is_burst_mode = fire_mode == "burst";

    // Tick burst state (fait avancer le timer interne). just_finished cette frame = fire.
    let mut burst_fires_now = false;
    let mut burst_active = false;
    let mut burst_will_terminate = false;
    if let Some(burst) = timing.burst_state.as_mut() {
        burst_active = true;
        if burst.interval_timer.tick(timing.time.delta()).just_finished() {
            burst_fires_now = true;
            burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
            if burst.shots_remaining == 0 {
                burst_will_terminate = true;
            }
        }
    }

    // Pendant un burst actif, on bypass le cooldown standard (pacing géré par BurstState).
    if timing.cooldown.is_some() && !burst_active {
        return;
    }

    // Dispatch trigger selon fire_mode
    let mut starts_burst = false;
    let trigger = match fire_mode {
        "auto" => left.held,
        "semi" | "pump" => left.just_pressed,
        "burst" => {
            if burst_active {
                burst_fires_now
            } else if left.just_pressed {
                starts_burst = true;
                true // 1er tir immédiat, BurstState inséré en fin de fonction
            } else {
                false
            }
        }
        other => {
            warn!("[fire] fire_mode inconnu '{}' — fallback semi", other);
            left.just_pressed
        }
    };
    if !trigger {
        return;
    }

    // Story-455 Phase A — Ammo gate. Tente de consommer 1 shot AVANT toute action coûteuse
    // (juice/raycast/vfx). Si mag vide : auto-start reload (si reserve > 0) + early return.
    // Si reload en cours et user tire : cancel reload (AAA feel, CoD/Apex parity).
    if !ammo.try_fire() {
        return;
    }

    let Ok(cam_tf) = q_cam.single() else {
        warn!("[fire] FpsCamera not found");
        return;
    };
    let Ok(ctx) = rapier.single() else {
        warn!("[fire] RapierContext not found");
        return;
    };

    // Juice per-arme : shake camera + recoil + FOV punch (conservative AAA values
    // dans TOML, anti-nausée par design). Seed PRNG basé sur time pour yaw recoil.
    if let Some(e) = entry {
        let juice_seed = (timing.time.elapsed_secs() * 1000.0) as u32;
        juice.emit_from_genome(e, juice_seed);
    }

    let origin = cam_tf.translation();
    let direction = cam_tf.forward().as_vec3();

    // Cooldown depuis genome (fallback 0.1s = ModernAR 10 shots/s)
    let cooldown_s = entry.map(|e| 1.0 / e.fire_rate.max(0.1)).unwrap_or(0.1);
    // Burst : pas de cooldown standard entre les shots de la rafale (interval géré par BurstState).
    // Insertion du cooldown UNIQUEMENT à la fin de la rafale (post-burst recovery 3x interval).
    if !is_burst_mode {
        commands.insert_resource(WeaponFireCooldown {
            timer: Timer::from_seconds(cooldown_s, TimerMode::Once),
        });
    } else if starts_burst {
        // Démarre la rafale : insère BurstState. Le 1er tir est ce tir-ci.
        let burst_count = entry.map(|e| e.burst_count.max(1)).unwrap_or(3);
        commands.insert_resource(BurstState {
            shots_remaining: burst_count.saturating_sub(1), // 1er shot consommé
            interval_timer: Timer::from_seconds(cooldown_s, TimerMode::Repeating),
        });
    } else if burst_will_terminate {
        // Rafale finie : cooldown long avant pouvoir re-trigger.
        commands.remove_resource::<BurstState>();
        commands.insert_resource(WeaponFireCooldown {
            timer: Timer::from_seconds(cooldown_s * 3.0, TimerMode::Once),
        });
    }
    // Cas restant : burst en cours (intermediate shot) → pas de cooldown, rien à faire.

    // Muzzle flash : position au BOUT DU CANON, lerp hipfire ↔ ADS via AdsState.progress.
    // story-450 v5 (2026-05-18 PM) — fix user "projectiles pas sur canon en visant".
    // En ADS l'arme se recentre (ads_offset_x=0) ET rétrécit (ads_scale_factor=0.60-0.70),
    // donc le canon visible est plus court. Le VFX doit lerp les 3 axes ET le scale.
    let barrel_len_base = entry.map(|e| e.barrel_length).unwrap_or(0.55);
    let p = ads.progress.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * p;
    let gun_off_x = entry
        .map(|e| lerp(e.offset_x, e.ads_offset_x))
        .unwrap_or(0.22);
    let gun_off_y = entry
        .map(|e| lerp(e.offset_y, e.ads_offset_y))
        .unwrap_or(-0.35);
    let gun_off_z = entry
        .map(|e| lerp(e.offset_z, e.ads_offset_z))
        .unwrap_or(-1.30);
    let viewmodel_scale = entry
        .map(|e| lerp(1.0, e.ads_scale_factor))
        .unwrap_or(1.0);
    let barrel_len = barrel_len_base * viewmodel_scale;
    let forward_dist = (-gun_off_z) + barrel_len;
    let cam_right_v = cam_tf.right().as_vec3();
    let cam_up_v = cam_tf.up().as_vec3();
    let barrel_tip = origin
        + direction * forward_dist
        + cam_right_v * gun_off_x
        + cam_up_v * gun_off_y;
    if let Some(vfx) = weapon_vfx.as_deref() {
        spawn_muzzle_flash(&mut commands, vfx, barrel_tip, direction, &ammo.equipped.current);
    }

    // Params raycast
    let range = entry.map(|e| e.range).unwrap_or(100.0);
    let damage = entry.map(|e| e.damage).unwrap_or(25.0);
    let pellets = entry.map(|e| e.pellets.max(1)).unwrap_or(1);
    let spread_rad = entry.map(|e| e.spread_deg.to_radians()).unwrap_or(0.0);

    // Exclure Player ET TOUS ses descendants (FpsCamera, viewmodel mesh, weapon child
    // colliders) du raycast. Sans ça, le ray peut hit le viewmodel ou un Mesh3d enfant
    // → first-hit early-out, dommages jamais appliqués ("aiming on target, no damage").
    //
    // Research industry pattern : Overwatch Architecture Tim Ford GDC 2017 — exclure
    // récursivement le shooter. Pre-build le HashSet une fois par fire pour O(1) lookup.
    let mut excluded: std::collections::HashSet<Entity> = std::collections::HashSet::default();
    if let Ok(player_entity) = q_player.single() {
        excluded.insert(player_entity);
        let mut stack = vec![player_entity];
        while let Some(e) = stack.pop() {
            if let Ok(children) = hitscan_ctx.q_children.get(e) {
                for c in children.iter() {
                    if excluded.insert(c) {
                        stack.push(c);
                    }
                }
            }
        }
    }
    let predicate = |e: Entity| !excluded.contains(&e);

    let right = cam_tf.right().as_vec3();
    let up = cam_tf.up().as_vec3();

    // Seed PRNG basé sur position cam + ms hash — reproductibilité par tir.
    let seed_base = (origin.x.abs() * 1000.0) as u32
        ^ (origin.z.abs() * 1000.0) as u32
        ^ (origin.y.abs() * 1000.0) as u32;

    let mut any_hit_dist = 50.0_f32;
    let mut hit_record: Option<(Entity, f32)> = None;

    for pellet_idx in 0..pellets {
        let pellet_dir = if pellets > 1 && spread_rad > 0.0 {
            let seed = seed_base
                .wrapping_add(u32::from(pellet_idx))
                .wrapping_mul(2654435761);
            let r1 = pseudo_rand(seed) - 0.5;
            let r2 = pseudo_rand(seed.wrapping_mul(0x9E3779B1)) - 0.5;
            let dev = right * (r1 * spread_rad) + up * (r2 * spread_rad);
            (direction + dev).normalize()
        } else {
            direction
        };

        // Story-453 baseline reset (2026-05-18) — ray cast simple, first hit only.
        // Plus de wallbang, plus de sphere cast forgiveness. Tu vises = tu hit.
        let filter = QueryFilter::default().predicate(&predicate);
        let hit_result = ctx.cast_ray(origin, pellet_dir, range, true, filter);

        if let Some((_, toi)) = hit_result {
            if toi < any_hit_dist {
                any_hit_dist = toi;
            }
        }

        // Tracer + impact par pellet (tous visuels pour shotgun cone visible)
        let hit_dist = hit_result.map(|(_, t)| t).unwrap_or(range);
        if let Some(tres) = tracer_res.as_deref() {
            spawn_hitscan_tracer(
                &mut commands,
                tres,
                origin,
                pellet_dir,
                hit_dist,
                &ammo.equipped.current,
                range.min(120.0),
                0.30,
            );
        }
        if let Some((_, toi)) = hit_result {
            let impact_pos = origin + pellet_dir * toi;
            if let Some(vfx) = weapon_vfx.as_deref() {
                spawn_impact_vfx(&mut commands, vfx, impact_pos, &ammo.equipped.current);
            }
        }

        // Sensor log : BUG-RUN-1 fix (story-455 audit runtime 2026-05-18).
        // Le ray hit un child Mesh3d (collider ConvexHull AsyncSceneCollider), pas le
        // parent TargetCube. Sans walk ChildOf, le sensor classait 100% des hits sur
        // bot mesh comme "blocker" → faux négatif (34 kills réels mais hits_with_damage=0
        // dans forgia_hitscan.json). Walk identique au damage path pour cohérence.
        let target_ancestor = match hit_result {
            None => None,
            Some((entity, _)) => find_health_ancestor(
                entity,
                &hitscan_ctx.q_child_of,
                &hit_ctx.health,
                8,
            ),
        };
        let (sensor_category, sensor_hit_idx, sensor_name, sensor_toi) = match hit_result {
            None => (HitscanCategory::Miss, None, None, None),
            Some((entity, toi)) => {
                let cat = if target_ancestor.is_some() {
                    HitscanCategory::HitZoneBody
                } else {
                    HitscanCategory::BlockerNonZone
                };
                // Nom = ancestor TargetCube si trouvé (lisible "Bot_3"), sinon entity raw.
                let display_entity = target_ancestor.unwrap_or(entity);
                let name = hitscan_ctx
                    .q_name
                    .get(display_entity)
                    .ok()
                    .map(|n| n.as_str().to_string());
                (cat, Some(entity.to_bits()), name, Some(toi))
            }
        };
        hitscan_ctx.sensor.push(HitscanLogEntry {
            t: timing.time.elapsed_secs(),
            weapon: ammo.equipped.current,
            origin,
            dir: pellet_dir,
            hit_entity_idx: sensor_hit_idx,
            hit_name: sensor_name,
            toi: sensor_toi,
            category: sensor_category,
        });

        // Apply damage : ray hit le collider ConvexHull généré par AsyncSceneCollider.
        // Le collider est plusieurs niveaux sous le bot parent (SceneRoot child →
        // Mesh3d nodes → rapier collider entities). `target_ancestor` calculé plus haut
        // pour le sensor — réutilisé ici (DRY, fix BUG-RUN-1 cohérent).
        if let Some((_, toi)) = hit_result {
            if let Some(entity) = target_ancestor {
                if let Ok((mut hp, mat_opt)) = hit_ctx.health.get_mut(entity) {
                    let falloff_mul = entry.map(|e| falloff_multiplier(toi, e)).unwrap_or(1.0);
                    let effective_dmg = damage * falloff_mul;
                    hp.current = (hp.current - effective_dmg).max(0.0);
                    let dead = hp.is_dead();
                    let new_hp = hp.current;

                    if let Some(mat_comp) = mat_opt {
                        let flash_dur = entry.map(|e| e.hit_flash_duration).unwrap_or(0.15);
                        commands
                            .entity(entity)
                            .insert(MeshMaterial3d(flash_cache.flash_material.clone()))
                            .insert(HitFlashTimer {
                                timer: Timer::from_seconds(flash_dur, TimerMode::Once),
                                original_emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
                                original_handle: Some(mat_comp.0.clone()),
                            });
                    }

                    // Story-455 Phase C — étendu attacker/headshot/world_pos/weapon.
                    // Headshot toujours `false` en attendant hitzone Head/Body split (story-456).
                    let attacker_entity = q_player.single().ok();
                    let hit_world = origin + pellet_dir * toi;
                    hit_events.write(CombatHitEvent {
                        target: entity,
                        attacker: attacker_entity,
                        damage: effective_dmg,
                        is_kill: dead,
                        is_headshot: false,
                        hit_world_pos: hit_world,
                        weapon: Some(ammo.equipped.current),
                    });

                    if hit_record.is_none() {
                        hit_record = Some((entity, toi));
                    }
                    info!(
                        "[fire] pellet {}/{} HIT {entity:?} toi={toi:.2}m dmg={effective_dmg:.1} hp={new_hp:.1} dead={dead}",
                        pellet_idx + 1, pellets
                    );
                }
            }
        }
    }

    // Hit-stop UNE FOIS par tir (pas par pellet) si au moins une cible touchée
    if hit_record.is_some() {
        let hs_dur = entry.map(|e| e.hit_stop_duration).unwrap_or(0.05);
        let hs_speed = entry.map(|e| e.hit_stop_speed).unwrap_or(0.05);
        timing.virtual_time.set_relative_speed(hs_speed);
        commands.insert_resource(HitStopState {
            timer: Timer::from_seconds(hs_dur, TimerMode::Once),
            restore_speed: 1.0,
        });
    } else {
        info!("[fire] miss ({} pellets, {:?})", pellets, ammo.equipped.current);
    }
}

/// Startup : load le genome viewmodel TOML (hot-reload Bevy via Shift+F12 ou save TOML).
fn load_viewmodel_genome(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<ViewmodelGenome>> =
        asset_server.load("genomes/viewmodel_arena.toml");
    commands.insert_resource(ViewmodelGenomeHandle(handle));
    info!("[forgia-fps] viewmodel genome loading : genomes/viewmodel_arena.toml");
}

/// Startup : load fps_tuning.toml — toutes constantes "feel" hardcodées avant.
fn load_fps_tuning(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<FpsTuning>> = asset_server.load("genomes/fps_tuning.toml");
    commands.insert_resource(FpsTuningHandle(handle));
    info!("[forgia-fps] fps_tuning genome loading : genomes/fps_tuning.toml");
}

/// Sync system : lit FpsTuning genome et push vers chaque downstream Resource Tuning.
/// Hot-reload : éditer le TOML → Bevy Asset re-load → ce système push automatiquement.
#[allow(clippy::too_many_arguments)]
fn sync_fps_tuning(
    handle: Option<Res<FpsTuningHandle>>,
    assets: Res<Assets<Genome<FpsTuning>>>,
    mut cs_tuning: ResMut<CameraShakeTuning>,
    mut fp_tuning: ResMut<FovPunchTuning>,
    mut ml_tuning: ResMut<MouseLookTuning>,
    mut ch_tuning: ResMut<CrosshairTuning>,
    mut ads_tuning: ResMut<AdsTuning>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    let t = &g.data;
    // Camera shake
    cs_tuning.default_decay = t.camera_shake.default_decay;
    cs_tuning.default_max_rotation = t.camera_shake.default_max_rotation_rad;
    cs_tuning.yaw_factor = t.camera_shake.yaw_factor;
    cs_tuning.roll_factor = t.camera_shake.roll_factor;
    cs_tuning.pitch_upward_bias = t.camera_shake.pitch_upward_bias;
    cs_tuning.sample_rate_hz = t.camera_shake.sample_rate_hz;
    // FOV punch
    fp_tuning.attack_secs = t.fov_punch.attack_secs;
    fp_tuning.decay_secs = t.fov_punch.decay_secs;
    // Mouse look
    ml_tuning.base_sensitivity = t.mouse_look.base_sensitivity;
    ml_tuning.recoil_decay_per_sec = t.mouse_look.recoil_decay_per_sec;
    // ADS
    ads_tuning.lerp_speed = t.ads.lerp_speed;
    ads_tuning.default_fov_deg = t.ads.default_fov_deg;
    ads_tuning.punch_attenuation = t.ads.punch_attenuation;
    // Crosshair
    ch_tuning.hipfire_cross_len = t.crosshair_hipfire.cross_len;
    ch_tuning.hipfire_cross_stroke = t.crosshair_hipfire.cross_stroke;
    ch_tuning.hipfire_cross_alpha = t.crosshair_hipfire.cross_alpha;
    ch_tuning.ads_dot_outer_radius = t.crosshair_ads_dot.outer_radius;
    ch_tuning.ads_dot_inner_radius = t.crosshair_ads_dot.inner_radius;
    let s = &t.crosshair_sniper_overlay;
    ch_tuning.sniper_scope_radius_factor = s.scope_radius_factor;
    ch_tuning.sniper_dim_alpha = s.dim_alpha;
    ch_tuning.sniper_dim_color = s.dim_color;
    ch_tuning.sniper_vignette_rings = s.vignette_rings;
    ch_tuning.sniper_ring_thickness_factor = s.ring_thickness_factor;
    ch_tuning.sniper_ring_max_alpha = s.ring_max_alpha;
    ch_tuning.sniper_ring_color = s.ring_color;
    ch_tuning.sniper_ring_outer_extent = s.ring_outer_extent;
    ch_tuning.sniper_border_width = s.border_width;
    ch_tuning.sniper_border_inner_width = s.border_inner_width;
    ch_tuning.sniper_border_inner_offset = s.border_inner_offset;
    ch_tuning.sniper_reticle_gap = s.reticle_gap;
    ch_tuning.sniper_reticle_line_factor = s.reticle_line_factor;
    ch_tuning.sniper_reticle_color = s.reticle_color;
    ch_tuning.sniper_reticle_line_stroke = s.reticle_line_stroke;
    ch_tuning.sniper_reticle_tick_stroke = s.reticle_tick_stroke;
    ch_tuning.sniper_reticle_tick_size = s.reticle_tick_size;
    ch_tuning.sniper_red_dot_radius = s.red_dot_radius;
    ch_tuning.sniper_red_dot_color = s.red_dot_color;
}

/// Startup : pré-charge les 3 GLB viewmodel (handles partagés, 1 load chacun).
fn load_weapon_models(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(WeaponModelAssets {
        pepin: asset_server.load("models/weapons/forgia/pepin.glb#Scene0"),
        bourrasque: asset_server.load("models/weapons/forgia/bourrasque.glb#Scene0"),
        madame_lenoir: asset_server.load("models/weapons/forgia/madame_lenoir.glb#Scene0"),
        boucherie: asset_server.load("models/weapons/forgia/boucherie.glb#Scene0"),
    });
}

/// Attache un viewmodel enfant de FpsCamera s'il n'en a pas encore.
/// Offset (0.3, -0.25, -0.5) = bras droit-bas-devant, scale 0.3 pour ne pas
/// remplir l'écran (ajustable). Pattern Valorant style fixed viewmodel.
fn attach_viewmodel_to_camera(
    mut commands: Commands,
    q_cam: Query<(Entity, Option<&Children>), With<FpsCamera>>,
    q_viewmodel: Query<&WeaponViewmodel>,
    assets: Option<Res<WeaponModelAssets>>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    let Some(assets) = assets else { return };
    for (cam, children) in &q_cam {
        let has_vm = children
            .map(|c| c.iter().any(|child| q_viewmodel.get(child).is_ok()))
            .unwrap_or(false);
        if has_vm {
            continue;
        }
        let entry = genome_handle
            .as_deref()
            .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
        let scene = scene_for_weapon(&assets, equipped.current);
        let tf = viewmodel_transform(equipped.current, entry);
        let target = viewmodel_target_size(equipped.current, entry);
        // Pattern V1 : spawn flat + add_child (vs with_children, ne change rien fonctionnellement
        // mais aligne avec V1). Hidden tant qu'AABB pas mesurée pour ne pas voir 1 frame d'arme géante.
        let vm = commands
            .spawn((
                WeaponViewmodel {
                    current: equipped.current,
                },
                SceneRoot(scene),
                tf,
                Visibility::Hidden,
                NeedsAutoScale { target_size: target },
                Name::new("WeaponViewmodel"),
            ))
            .id();
        commands.entity(cam).add_child(vm);
        info!(
            "[forgia-fps] viewmodel spawned ({:?}, target {:.2}m, awaiting AABB)",
            equipped.current, target
        );
    }
}

/// Swap SceneRoot du viewmodel quand EquippedWeapons.current change.
fn update_viewmodel_on_switch(
    mut commands: Commands,
    assets: Option<Res<WeaponModelAssets>>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    mut q: Query<(Entity, &mut SceneRoot, &mut Transform, &mut Visibility, &mut WeaponViewmodel)>,
) {
    if !equipped.is_changed() {
        return;
    }
    let Some(assets) = assets else { return };
    let entry = genome_handle
        .as_deref()
        .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
    for (entity, mut scene, mut tf, mut vis, mut vm) in &mut q {
        if vm.current == equipped.current {
            continue;
        }
        scene.0 = scene_for_weapon(&assets, equipped.current);
        *tf = viewmodel_transform(equipped.current, entry);
        *vis = Visibility::Hidden;
        commands.entity(entity).insert(NeedsAutoScale {
            target_size: viewmodel_target_size(equipped.current, entry),
        });
        vm.current = equipped.current;
    }
}

/// Auto-scale BFS pattern V1 (`combat/viewmodel.rs:603`).
/// Combine les Aabb de tous les descendants Mesh3d, calcule scale = target / max_extent,
/// puis applique au Transform + retire NeedsAutoScale + révèle (Visibility::Inherited).
fn auto_scale_viewmodel(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsAutoScale, &Transform, &WeaponViewmodel)>,
    q_children: Query<&Children>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
    q_aabb: Query<&bevy::camera::primitives::Aabb>,
) {
    for (entity, auto, tf, vm) in q_needs.iter() {
        let mut g_min = Vec3::splat(f32::MAX);
        let mut g_max = Vec3::splat(f32::MIN);
        let mut found = false;

        let mut stack = vec![entity];
        while let Some(e) = stack.pop() {
            if let Ok(aabb) = q_aabb.get(e) {
                let c: Vec3 = aabb.center.into();
                let h: Vec3 = aabb.half_extents.into();
                g_min = g_min.min(c - h);
                g_max = g_max.max(c + h);
                found = true;
            }
            if let Ok(children) = q_children.get(e) {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        if !found {
            // Scene encore en load — Aabb pas calculé. Retry next frame.
            continue;
        }

        let size = g_max - g_min;
        let max_extent = size.x.max(size.y).max(size.z);
        if max_extent < 0.001 {
            continue;
        }
        // AABB corrompu (e.g. ak47.glb max 16383m = i16 quantization mal décodée) :
        // utiliser fallback scale au lieu de target/max_extent qui donne ~0.
        let new_scale = if max_extent > 100.0 {
            let entry = genome_handle
                .as_deref()
                .and_then(|h| lookup_genome_entry(&genome_assets, h, vm.current));
            let fallback = viewmodel_fallback_scale(vm.current, entry);
            warn!(
                "[forgia-fps] viewmodel AABB CORROMPU ({:.0}m) {:?} → fallback scale {:.4}",
                max_extent, vm.current, fallback
            );
            fallback
        } else {
            auto.target_size / max_extent
        };
        info!(
            "[forgia-fps] viewmodel AABB ({:.2},{:.2},{:.2}) max {:.2}m → scale {:.4}",
            size.x, size.y, size.z, max_extent, new_scale
        );
        commands
            .entity(entity)
            .remove::<NeedsAutoScale>()
            .insert(Transform {
                scale: Vec3::splat(new_scale),
                ..*tf
            })
            .insert(ViewmodelBaseScale(new_scale))
            .insert(Visibility::Inherited);
    }
}

/// Insert `CameraShake` Component sur FpsCamera si pas déjà présent.
/// Single-shot effectif (Without<CameraShake>). Évite ajouter dep forgia-juice-camera-shake
/// dans forgia-player (upstream).
fn ensure_camera_shake_component(
    mut commands: Commands,
    q: Query<Entity, (With<FpsCamera>, Without<CameraShake>)>,
    cs_tuning: Res<CameraShakeTuning>,
) {
    for e in &q {
        commands.entity(e).insert(CameraShake {
            decay: cs_tuning.default_decay,
            max_rotation: cs_tuning.default_max_rotation,
            ..CameraShake::default()
        });
        info!(
            "[forgia-fps] CameraShake attaché (decay={:.1}, max_rot={:.4})",
            cs_tuning.default_decay, cs_tuning.default_max_rotation
        );
    }
}

/// OnExit(Fps) : despawn le viewmodel pour ne pas l'avoir en mode RPG.
fn despawn_viewmodel(mut commands: Commands, q: Query<Entity, With<WeaponViewmodel>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaFpsPlugin;
    }

    // ─── Phase 2 (dette tech 2026-05-18) — headless fire tests ──────────

    #[test]
    fn left_mouse_state_default_idle() {
        let s = LeftMouseState::default();
        assert!(!s.held, "default held doit être false");
        assert!(!s.just_pressed, "default just_pressed doit être false");
    }

    #[test]
    fn pseudo_rand_in_unit_range() {
        for seed in [1u32, 42, 12345, u32::MAX / 2] {
            let v = pseudo_rand(seed);
            assert!((0.0..1.0).contains(&v), "pseudo_rand({}) = {} hors [0,1)", seed, v);
        }
    }

    #[test]
    fn pseudo_rand_deterministic_same_seed() {
        // Reproducibilité critique pour shotgun cone : même seed = même pattern.
        assert_eq!(pseudo_rand(12345), pseudo_rand(12345));
        assert_ne!(pseudo_rand(12345), pseudo_rand(12346));
    }

    #[test]
    fn viewmodel_genome_defaults_are_safe() {
        // Si TOML omet un field, les defaults doivent permettre un arme jouable.
        assert_eq!(default_fire_mode(), "auto");
        assert_eq!(default_burst_count(), 3);
        assert!(default_damage() > 0.0);
        assert!(default_fire_rate() > 0.0);
        assert!(default_range() > 0.0);
        assert_eq!(default_pellets(), 1);
        assert_eq!(default_spread_deg(), 0.0);
        assert!(default_hit_flash_duration() > 0.0);
        assert!(default_hit_stop_duration() > 0.0);
        assert!(default_hit_stop_speed() > 0.0 && default_hit_stop_speed() < 1.0);
    }

    #[test]
    fn burst_state_decrement_via_timer() {
        // Simule la boucle interne sans App : tick timer just_finished + decrement.
        let mut burst = BurstState {
            shots_remaining: 3,
            interval_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
        };
        // Avant tick : pas encore fini
        assert!(!burst.interval_timer.just_finished());
        // Tick > duration → just_finished
        burst.interval_timer.tick(std::time::Duration::from_millis(60));
        assert!(burst.interval_timer.just_finished());
        burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
        assert_eq!(burst.shots_remaining, 2);
    }

    #[test]
    fn burst_state_terminates_at_zero() {
        let mut burst = BurstState {
            shots_remaining: 1,
            interval_timer: Timer::from_seconds(0.05, TimerMode::Repeating),
        };
        burst.shots_remaining = burst.shots_remaining.saturating_sub(1);
        assert_eq!(burst.shots_remaining, 0, "doit atteindre 0 → trigger remove_resource côté system");
    }

    #[test]
    fn track_left_mouse_pressed_sets_both() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        // Envoyer Pressed event puis update
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(s.held, "Pressed doit set held=true");
        assert!(s.just_pressed, "Pressed doit set just_pressed=true");
    }

    #[test]
    fn track_left_mouse_just_pressed_resets_each_frame() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        // Pas de nouvel event : just_pressed doit retomber, held reste
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(s.held, "held doit persister sans Released");
        assert!(!s.just_pressed, "just_pressed doit reset à frame N+1");
    }

    #[test]
    fn track_left_mouse_released_clears_held() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Released,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(!s.held, "Released doit clear held");
    }

    #[test]
    fn track_left_mouse_ignores_other_buttons() {
        let mut app = App::new();
        app.add_message::<MouseButtonInput>()
            .init_resource::<LeftMouseState>()
            .add_systems(Update, track_left_mouse_state);

        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Right,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();

        let s = app.world().resource::<LeftMouseState>();
        assert!(!s.held, "Right mouse ne doit pas affecter LeftMouseState");
        assert!(!s.just_pressed);
    }
}

//! Helpers purs de **calibration** — transforms et rotations dérivés du genome.
//!
//! Sources des conventions :
//! - Bevy 0.18 official example `first-person-view-model` : viewmodel = child de la
//!   caméra avec `RenderLayers` dédié (cf bevy.org/examples/camera/first-person-view-model).
//! - Source SDK : viewmodel positioné en local-space caméra, axis convention -Z forward.
//!
//! Fallbacks hardcodés conservés pour le frame de boot où le genome TOML n'est
//! pas encore chargé. Tous les autres call-sites doivent passer `Some(genome)`.

use bevy::prelude::*;
use forgia_combat::weapons::WeaponType;

use crate::genome::ViewmodelGenomeEntry;

/// Rotation hipfire = base rotation + tilt Y additionnel ("carry angle" CoD).
pub fn viewmodel_rotation_hipfire(g: &ViewmodelGenomeEntry) -> Quat {
    Quat::from_rotation_x(g.rotation_x_deg.to_radians())
        * Quat::from_rotation_y((g.rotation_y_deg + g.hipfire_tilt_y_deg).to_radians())
        * Quat::from_rotation_z(g.rotation_z_deg.to_radians())
}

/// Rotation ADS = rotation base (sans tilt, arme centrée viseur).
pub fn viewmodel_rotation_ads(g: &ViewmodelGenomeEntry) -> Quat {
    Quat::from_rotation_x(g.rotation_x_deg.to_radians())
        * Quat::from_rotation_y(g.rotation_y_deg.to_radians())
        * Quat::from_rotation_z(g.rotation_z_deg.to_radians())
}

/// Transform local du viewmodel (relative à FpsCamera). Si `genome` fourni,
/// ses valeurs ont priorité. Sinon fallback hardcodé pour le boot pré-genome.
pub fn viewmodel_transform(w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> Transform {
    if let Some(g) = genome {
        return Transform {
            translation: Vec3::new(g.offset_x, g.offset_y, g.offset_z),
            rotation: viewmodel_rotation_hipfire(g),
            scale: Vec3::splat(1.0),
        };
    }
    // Fallback hardcodé (boot précoce, genome TOML pas encore loaded).
    // Convention GLB face -X par défaut → rotation Y +90° pour aligner canon vers -Z.
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

/// Taille cible du viewmodel (max extent en mètres) — pour auto-scale AABB.
pub fn viewmodel_target_size(w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> f32 {
    if let Some(g) = genome {
        return g.target_size;
    }
    match w {
        WeaponType::Shotgun => 0.8,
        WeaponType::Chainsaw => 0.6,
        _ => 0.75,
    }
}

/// Scale fallback si AABB est corrompu (i16 quantization mal décodé → ~16383m).
pub fn viewmodel_fallback_scale(_w: WeaponType, genome: Option<&ViewmodelGenomeEntry>) -> f32 {
    genome.map(|g| g.fallback_scale).unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_entry() -> ViewmodelGenomeEntry {
        // Toutes les valeurs minimales pour construire un entry — defaults `serde` ne
        // sont pas appelés en code natif.
        ViewmodelGenomeEntry {
            target_size: 0.9,
            offset_x: 0.22,
            offset_y: -0.35,
            offset_z: -1.30,
            rotation_y_deg: -90.0,
            fallback_scale: 1.0,
            rotation_x_deg: 0.0,
            rotation_z_deg: 0.0,
            barrel_length: 0.55,
            grip_anchor: None,
            barrel_anchor: None,
            hide_support_hand: false,
            grip_roll_deg: 0.0,
            barrel_roll_deg: 0.0,
            ads_offset_x: 0.0,
            ads_offset_y: -0.28,
            ads_offset_z: -0.80,
            ads_fov_deg: 30.0,
            sight_local_x: 0.0,
            sight_local_y: 0.0,
            sight_local_z: 0.0,
            sight_distance: 0.40,
            ads_move_speed_factor: 0.75,
            scope_glass_alpha_ads: 0.20,
            hipfire_tilt_y_deg: 5.0,
            fire_mode: "semi".to_string(),
            burst_count: 0,
            damage: 28.0,
            fire_rate: 6.0,
            range: 80.0,
            pellets: 1,
            spread_deg: 0.0,
            sniper_scope_fullscreen: false,
            ads_scale_factor: 0.7,
            shake_trauma: 0.03,
            recoil_pitch_deg: 0.25,
            recoil_yaw_random_deg: 0.05,
            fov_punch_deg: 0.0,
            head_damage_mul: 2.0,
            damage_falloff_start: 30.0,
            damage_falloff_end: 80.0,
            damage_falloff_min: 0.6,
            ads_viewmodel_fade_alpha: 1.0,
            ads_mouse_sensitivity_factor: 0.7,
            hit_flash_duration: 0.15,
            mag_size: 12,
            reserve_max: 84,
            reload_time_secs: 1.2,
            reload_kind: "mag".to_string(),
            infinite_ammo: false,
            low_ammo_threshold: 0.25,
        }
    }

    #[test]
    fn hipfire_rotation_applies_tilt() {
        let mut e = mock_entry();
        e.hipfire_tilt_y_deg = 10.0;
        e.rotation_y_deg = 0.0;
        let hipfire = viewmodel_rotation_hipfire(&e);
        let ads = viewmodel_rotation_ads(&e);
        // Hipfire ≠ ADS si tilt non nul.
        assert!(
            hipfire.angle_between(ads) > 0.0,
            "tilt non nul doit produire rotation différente"
        );
    }

    #[test]
    fn ads_rotation_ignores_tilt() {
        let mut e = mock_entry();
        e.hipfire_tilt_y_deg = 10.0;
        e.rotation_y_deg = -90.0;
        let ads = viewmodel_rotation_ads(&e);
        let expected = Quat::from_rotation_y((-90.0_f32).to_radians());
        // Robust same-rotation check : |dot| ≈ 1.0 (Quat double-cover safe).
        let same = (1.0 - ads.dot(expected).abs()).abs();
        assert!(
            same < 1e-4,
            "ADS rotation = base seule (tilt ignoré), 1-|dot|={same}"
        );
    }

    #[test]
    fn transform_uses_genome_when_provided() {
        let e = mock_entry();
        let tf = viewmodel_transform(WeaponType::ModernAR, Some(&e));
        assert!((tf.translation.x - 0.22).abs() < 1e-5);
        assert!((tf.translation.y - (-0.35)).abs() < 1e-5);
        assert!((tf.translation.z - (-1.30)).abs() < 1e-5);
    }

    #[test]
    fn transform_fallback_when_genome_none() {
        let tf = viewmodel_transform(WeaponType::Shotgun, None);
        // Fallback hardcodé Shotgun : Vec3::new(0.30, -0.35, -0.60)
        assert!((tf.translation.x - 0.30).abs() < 1e-5);
    }

    #[test]
    fn target_size_uses_genome() {
        let e = mock_entry();
        assert_eq!(viewmodel_target_size(WeaponType::ModernAR, Some(&e)), 0.9);
        assert_eq!(viewmodel_target_size(WeaponType::Chainsaw, None), 0.6);
    }

    #[test]
    fn fallback_scale_uses_genome_when_provided() {
        let e = mock_entry();
        assert_eq!(
            viewmodel_fallback_scale(WeaponType::ModernAR, Some(&e)),
            1.0
        );
        // Sans genome : 1.0 (no-op).
        assert_eq!(viewmodel_fallback_scale(WeaponType::ModernAR, None), 1.0);
    }
}

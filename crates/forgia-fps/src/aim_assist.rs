//! # aim_assist — Bullet magnetism (story-615)
//!
//! Modèle **souris** : on corrige le *tir*, jamais la caméra. La direction du tir
//! est légèrement courbée vers la meilleure cible dans un cône réticule-centré,
//! bornée par un angle de correction max (anti-aimbot). Le joueur garde 100 % du
//! contrôle de sa visée — pas de pull rotationnel (anti-pattern souris, cf. veille
//! Apex/Destiny/COD : pull+friction = manette only, bullet magnetism = les deux).
//!
//! `strength = 0.0` ⇒ strictement off (respect strict de l'intention joueur).
//!
//! Story-649 (2026-07-03) — **cohérence façon CoD BO6** : l'aide n'est jamais
//! binaire. Deux falloffs multiplient la force (réf. patch notes BO6 : « aim
//! assist strength now linearly interpolates — much weaker at point-blank,
//! smoothly increasing over short range ») :
//! - **angulaire** : pleine au centre du réticule → 0 au bord du cône (fini le
//!   « j'ai clairement raté mais ça touche » — c'était le défaut du modèle
//!   binaire : une cible à 6.9° recevait la même aide qu'une cible à 0.5°) ;
//! - **distance** : réduite à bout portant (le joueur contrôle), pleine à
//!   mi-portée, dégressive vers 0 à `engage_distance_m`.
//!
//! Cross-mode FPS + Roguelite : la cible est filtrée par `Mortal Without<Player>`
//! (présent sur tous les ennemis des deux modes, découplé du type `Health` — évite
//! le piège des deux `Health` qui rendait l'ancien aim assist mort en Roguelite).
//!
//! Tuning hot-reload via `fps_tuning.toml [aim_assist]`.

use bevy::prelude::*;

/// Tuning bullet magnetism (hot-reload via fps_tuning.toml [aim_assist]).
#[derive(Resource, Debug, Clone, Copy)]
pub struct AimAssistTuning {
    /// Force 0.0..1.0 : fraction de l'écart angulaire refermée par tir. 0.0 = off.
    pub strength: f32,
    /// Demi-angle du cône de candidature (degrés) : au-delà, la cible est ignorée.
    pub max_angle_deg: f32,
    /// Borne dure sur la correction appliquée par tir (degrés) — anti-aimbot.
    pub max_correction_deg: f32,
    /// Distance max d'engagement (mètres).
    pub engage_distance_m: f32,
}

impl Default for AimAssistTuning {
    fn default() -> Self {
        // Story-649 : resserré (était 0.6 / 7° / 5° / 60 m — 45 % des tirs
        // corrigés en session réelle, magnétisme visible). Le cône étroit +
        // falloffs = l'aide finit ce que le joueur a presque réussi, jamais plus.
        Self {
            strength: 0.5,
            max_angle_deg: 3.5,
            max_correction_deg: 2.0,
            engage_distance_m: 40.0,
        }
    }
}

// ── Forme des falloffs (invariants de courbe, pas du tuning gameplay) ────────
// Fractions de `engage_distance_m` délimitant la rampe distance (façon BO6 :
// faible bout portant → plateau plein → fondu vers 0 à la portée max).
/// Fin de la rampe bout-portant (aide réduite en-dessous de 20 % de la portée).
const DIST_RAMP_END_FRAC: f32 = 0.2;
/// Début du fondu lointain (aide pleine entre 20 % et 60 % de la portée).
const DIST_FADE_START_FRAC: f32 = 0.6;

/// Smoothstep classique 0..1 (C1-continu — pas de « cran » senti au bord).
fn smoothstep01(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Falloff distance : rampe bout-portant → plateau → fondu (0 à la portée max).
fn distance_falloff(dist: f32, engage: f32) -> f32 {
    if engage <= 0.0 {
        return 0.0;
    }
    let d = dist / engage;
    if d < DIST_RAMP_END_FRAC {
        smoothstep01(d / DIST_RAMP_END_FRAC)
    } else if d <= DIST_FADE_START_FRAC {
        1.0
    } else {
        smoothstep01((1.0 - d) / (1.0 - DIST_FADE_START_FRAC))
    }
}

/// Métriques bullet magnetism — alimentées par le fire path, lues par le sensor.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct AimAssistMetrics {
    /// Total de tirs soumis au bullet magnetism (toute issue).
    pub shots_total: u64,
    /// Tirs effectivement corrigés (bend appliqué).
    pub shots_corrected: u64,
    /// Dernière correction appliquée (degrés).
    pub last_correction_deg: f32,
}

/// État interne du sensor (timing 1Hz + snapshot pour le rate de correction).
#[derive(Resource, Default)]
pub struct AimAssistSensorState {
    pub last_write_secs: f32,
    pub last_total: u64,
    pub last_corrected: u64,
}

/// Cœur pur du bullet magnetism : renvoie la direction de tir (éventuellement
/// courbée vers la meilleure cible) + l'angle de correction appliqué en radians
/// (0.0 si aucune correction).
///
/// Sélectionne la cible la plus *alignée* (plus grand dot avec `forward`) dans le
/// cône `max_angle_deg` et la distance `engage_distance_m`, puis tourne `forward`
/// vers elle de `min(angle_total * strength, max_correction_deg)`.
pub fn bend_fire_direction(
    origin: Vec3,
    forward: Vec3,
    targets: impl Iterator<Item = Vec3>,
    tuning: &AimAssistTuning,
) -> (Vec3, f32) {
    let strength = tuning.strength.clamp(0.0, 1.0);
    if strength <= 0.0 {
        return (forward, 0.0);
    }
    let cone_rad = tuning.max_angle_deg.to_radians();
    let max_cos = cone_rad.cos();
    let max_dist_sq = tuning.engage_distance_m * tuning.engage_distance_m;

    // Meilleure cible = plus grand dot (la plus centrée sur le réticule) dans le cône.
    let mut best: Option<(Vec3, f32, f32)> = None; // (dir unitaire, dot, dist)
    for tpos in targets {
        let to = tpos - origin;
        let dist_sq = to.length_squared();
        if dist_sq < 0.01 || dist_sq > max_dist_sq {
            continue;
        }
        let dist = dist_sq.sqrt();
        let dir = to / dist;
        let dot = dir.dot(forward);
        if dot < max_cos {
            continue;
        }
        match best {
            Some((_, best_dot, _)) if best_dot >= dot => {}
            _ => best = Some((dir, dot, dist)),
        }
    }

    let Some((dir, dot, dist)) = best else {
        return (forward, 0.0);
    };

    let full_angle = dot.clamp(-1.0, 1.0).acos();
    if full_angle < 1e-5 {
        return (forward, 0.0); // déjà pile dessus
    }
    // Story-649 — falloffs (cohérence BO6) : l'aide est graduée, jamais binaire.
    // Angulaire : pleine au centre du réticule, 0 au bord du cône.
    let ang_falloff = smoothstep01(1.0 - (full_angle / cone_rad.max(1e-6)).clamp(0.0, 1.0));
    // Distance : réduite bout portant, pleine à mi-portée, 0 à la portée max.
    let dist_falloff = distance_falloff(dist, tuning.engage_distance_m);
    let max_corr = tuning.max_correction_deg.max(0.0).to_radians();
    let bend = (full_angle * strength * ang_falloff * dist_falloff).min(max_corr);
    if bend < 1e-5 {
        return (forward, 0.0);
    }
    let axis = forward.cross(dir).normalize_or_zero();
    if axis == Vec3::ZERO {
        return (forward, 0.0); // forward et dir colinéaires (cas dégénéré)
    }
    let bent = (Quat::from_axis_angle(axis, bend) * forward).normalize();
    (bent, bend)
}

const SENSOR_PATH: &str = "forgia2_aimassist.json";

/// Seuil tirs/fenêtre du health check. 3 = se déclenche même sur arme semi
/// (Pépin/Lenoir ~1-2 tirs/s), pas seulement auto (Bourrasque 10/s). qa-615-03.
const HEALTH_CHECK_MIN_SHOTS: u64 = 3;

/// Décision sensor pure (testable) sur la **fenêtre glissante** (deltas 1s).
/// Retourne `(severity, next_step, corrected_pct_fenêtre)`.
///
/// Health check : `warn` si l'assist est actif mais ne corrige aucun tir sur la
/// fenêtre (cône trop étroit ou cibles introuvables — aurait attrapé le bug
/// `forgia_damage::Health` qui rendait l'ancien aim assist mort en Roguelite).
fn evaluate_window(total_delta: u64, corr_delta: u64, strength: f32) -> (&'static str, &'static str, f32) {
    let pct = if total_delta > 0 {
        100.0 * corr_delta as f32 / total_delta as f32
    } else {
        0.0
    };
    let (severity, next_step) =
        if strength > 0.0 && total_delta >= HEALTH_CHECK_MIN_SHOTS && corr_delta == 0 {
            (
                "warn",
                "aim assist actif mais 0 tir corrige sur la fenetre : cone trop etroit ou cibles introuvables (Mortal manquant ?)",
            )
        } else {
            ("ok", "")
        };
    (severity, next_step, pct)
}

/// Sensor 1Hz `forgia2_aimassist.json` — observabilité bullet magnetism.
pub fn write_aim_assist_sensor(
    time: Res<Time>,
    tuning: Res<AimAssistTuning>,
    metrics: Res<AimAssistMetrics>,
    mut state: ResMut<AimAssistSensorState>,
) {
    let now = time.elapsed_secs();
    if now - state.last_write_secs < 1.0 {
        return;
    }
    state.last_write_secs = now;

    let total_delta = metrics.shots_total.saturating_sub(state.last_total);
    let corr_delta = metrics.shots_corrected.saturating_sub(state.last_corrected);
    state.last_total = metrics.shots_total;
    state.last_corrected = metrics.shots_corrected;

    let (severity, next_step, corrected_pct) =
        evaluate_window(total_delta, corr_delta, tuning.strength);
    // Dernière correction : 0 si rien corrigé sur la fenêtre (sinon le sensor
    // affiche une correction « fantôme » d'il y a plusieurs secondes — qa-615-05).
    let last_correction_deg = if corr_delta > 0 {
        metrics.last_correction_deg
    } else {
        0.0
    };

    let json = format!(
        r#"{{"id":"aimassist","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"strength":{:.2},"cone_deg":{:.1},"max_correction_deg":{:.1},"engage_distance_m":{:.1},"shots_total":{},"shots_corrected":{},"corrected_pct_window":{:.1},"last_correction_deg":{:.2}}}"#,
        now,
        tuning.strength,
        tuning.max_angle_deg,
        tuning.max_correction_deg,
        tuning.engage_distance_m,
        metrics.shots_total,
        metrics.shots_corrected,
        corrected_pct,
        last_correction_deg,
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-fps] aim_assist_sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> AimAssistTuning {
        AimAssistTuning {
            strength: 1.0,
            max_angle_deg: 7.0,
            max_correction_deg: 5.0,
            engage_distance_m: 60.0,
        }
    }

    #[test]
    fn default_is_bullet_magnetism_off_friendly() {
        // Story-649 : resserré vs story-615 (0.6/7°/5°/60m → 45% de tirs corrigés).
        let t = AimAssistTuning::default();
        assert_eq!(t.strength, 0.5);
        assert_eq!(t.max_angle_deg, 3.5);
        assert_eq!(t.max_correction_deg, 2.0);
        assert_eq!(t.engage_distance_m, 40.0);
    }

    #[test]
    fn strength_zero_never_bends() {
        let mut t = tuning();
        t.strength = 0.0;
        let fwd = Vec3::NEG_Z;
        // Cible légèrement à droite (devrait être ignorée car strength=0).
        let target = Vec3::new(0.1, 0.0, -10.0);
        let (dir, corr) = bend_fire_direction(Vec3::ZERO, fwd, [target].into_iter(), &t);
        assert_eq!(corr, 0.0);
        assert_eq!(dir, fwd);
    }

    #[test]
    fn no_target_returns_forward_unchanged() {
        let fwd = Vec3::NEG_Z;
        let (dir, corr) = bend_fire_direction(Vec3::ZERO, fwd, std::iter::empty(), &tuning());
        assert_eq!(corr, 0.0);
        assert_eq!(dir, fwd);
    }

    #[test]
    fn target_outside_cone_is_ignored() {
        let fwd = Vec3::NEG_Z;
        // Cible à 45° du forward → hors cône de 7°.
        let target = Vec3::new(10.0, 0.0, -10.0);
        let (dir, corr) = bend_fire_direction(Vec3::ZERO, fwd, [target].into_iter(), &tuning());
        assert_eq!(corr, 0.0);
        assert_eq!(dir, fwd);
    }

    #[test]
    fn target_in_cone_bends_toward_it_and_reduces_miss() {
        let fwd = Vec3::NEG_Z;
        // Cible ~3° à droite, à 20m.
        let angle = 3.0_f32.to_radians();
        let target = Vec3::new((angle.sin()) * 20.0, 0.0, -(angle.cos()) * 20.0);
        let (dir, corr) = bend_fire_direction(Vec3::ZERO, fwd, [target].into_iter(), &tuning());
        // Story-649 : l'écart n'est plus refermé en entier (falloff angulaire),
        // mais la direction doit toujours se rapprocher de la cible.
        assert!(corr > 0.0, "doit corriger");
        let to_target = (target - Vec3::ZERO).normalize();
        // La direction bendée est plus alignée avec la cible que le forward brut.
        assert!(dir.dot(to_target) > fwd.dot(to_target));
    }

    #[test]
    fn correction_is_clamped_by_max_correction() {
        let mut t = tuning();
        t.max_angle_deg = 20.0; // cône large
        t.max_correction_deg = 2.0; // mais on borne à 2°
        let fwd = Vec3::NEG_Z;
        // Cible à 10° → bend voudrait 10° mais clamp à 2°.
        let angle = 10.0_f32.to_radians();
        let target = Vec3::new((angle.sin()) * 20.0, 0.0, -(angle.cos()) * 20.0);
        let (_dir, corr) = bend_fire_direction(Vec3::ZERO, fwd, [target].into_iter(), &t);
        assert!(
            (corr - 2.0_f32.to_radians()).abs() < 1e-4,
            "correction bornee a max_correction_deg (2°), got {} deg",
            corr.to_degrees()
        );
    }

    #[test]
    fn sensor_window_pct_uses_delta_not_cumulative() {
        // 100 tirs anciens (cumul) hors fenêtre ; fenêtre courante = 4 tirs, 2 corrigés.
        let (_sev, _step, pct) = evaluate_window(4, 2, 0.6);
        assert!((pct - 50.0).abs() < 1e-3, "pct fenêtre = 2/4 = 50%, got {pct}");
    }

    #[test]
    fn sensor_health_check_warns_on_semi_weapon() {
        // 3 tirs sur la fenêtre (arme semi), 0 corrigé, assist actif → warn.
        let (sev, _step, _pct) = evaluate_window(3, 0, 0.6);
        assert_eq!(sev, "warn");
    }

    #[test]
    fn sensor_health_check_ok_when_off_or_correcting() {
        // strength 0 → jamais warn même sans correction.
        assert_eq!(evaluate_window(10, 0, 0.0).0, "ok");
        // au moins 1 correction → ok.
        assert_eq!(evaluate_window(10, 1, 0.6).0, "ok");
        // sous le seuil de tirs → ok (pas assez d'échantillon).
        assert_eq!(evaluate_window(2, 0, 0.6).0, "ok");
    }

    // ── Story-649 : falloffs (cohérence BO6) ────────────────────────────────

    #[test]
    fn edge_of_cone_gets_almost_no_help() {
        // Cible à 6.8° dans un cône de 7° : l'aide doit être quasi nulle —
        // AVANT story-649 elle recevait 60-100% de fermeture (« raté net mais
        // touché », le défaut signalé par le user).
        let t = tuning(); // strength 1.0, cône 7°
        let a = 6.8_f32.to_radians();
        let target = Vec3::new(a.sin() * 20.0, 0.0, -a.cos() * 20.0);
        let (_dir, corr) = bend_fire_direction(Vec3::ZERO, Vec3::NEG_Z, [target].into_iter(), &t);
        assert!(
            corr.to_degrees() < 0.1,
            "bord de cône ≈ zéro aide, got {}°",
            corr.to_degrees()
        );
    }

    #[test]
    fn point_blank_help_is_reduced() {
        // Même angle (2°), cible à 3 m (bout portant, <20% de 60 m) vs 25 m
        // (plateau) : l'aide bout-portant doit être plus faible (pattern BO6
        // « much weaker at point-blank range »).
        let t = tuning();
        let a = 2.0_f32.to_radians();
        let near = Vec3::new(a.sin() * 3.0, 0.0, -a.cos() * 3.0);
        let mid = Vec3::new(a.sin() * 25.0, 0.0, -a.cos() * 25.0);
        let (_d1, corr_near) =
            bend_fire_direction(Vec3::ZERO, Vec3::NEG_Z, [near].into_iter(), &t);
        let (_d2, corr_mid) = bend_fire_direction(Vec3::ZERO, Vec3::NEG_Z, [mid].into_iter(), &t);
        assert!(
            corr_near < corr_mid,
            "bout portant ({:.2}°) doit aider moins que mi-portée ({:.2}°)",
            corr_near.to_degrees(),
            corr_mid.to_degrees()
        );
    }

    #[test]
    fn long_range_help_fades_out() {
        // Même angle, 25 m (plateau) vs 57 m (fondu final, engage 60 m).
        let t = tuning();
        let a = 2.0_f32.to_radians();
        let mid = Vec3::new(a.sin() * 25.0, 0.0, -a.cos() * 25.0);
        let far = Vec3::new(a.sin() * 57.0, 0.0, -a.cos() * 57.0);
        let (_d1, corr_mid) = bend_fire_direction(Vec3::ZERO, Vec3::NEG_Z, [mid].into_iter(), &t);
        let (_d2, corr_far) = bend_fire_direction(Vec3::ZERO, Vec3::NEG_Z, [far].into_iter(), &t);
        assert!(
            corr_far < corr_mid,
            "longue portée ({:.2}°) doit fondre vs plateau ({:.2}°)",
            corr_far.to_degrees(),
            corr_mid.to_degrees()
        );
    }

    #[test]
    fn distance_falloff_shape() {
        // Rampe bout-portant → plateau → fondu → 0 exact à la portée max.
        assert_eq!(distance_falloff(0.0, 60.0), 0.0);
        assert!((distance_falloff(20.0, 60.0) - 1.0).abs() < 1e-6); // plateau (33%)
        assert!(distance_falloff(50.0, 60.0) < 1.0); // fondu entamé (83%)
        assert!(distance_falloff(60.0, 60.0) < 1e-6); // portée max = 0
        assert_eq!(distance_falloff(10.0, 0.0), 0.0); // engage 0 = pas d'aide
    }

    #[test]
    fn nearest_to_reticle_is_chosen() {
        let fwd = Vec3::NEG_Z;
        let a3 = 3.0_f32.to_radians();
        let a6 = 6.0_f32.to_radians();
        let near = Vec3::new(a3.sin() * 20.0, 0.0, -a3.cos() * 20.0); // 3°
        let far = Vec3::new(a6.sin() * 20.0, 0.0, -a6.cos() * 20.0); // 6°
        let (dir, _corr) = bend_fire_direction(Vec3::ZERO, fwd, [far, near].into_iter(), &tuning());
        // Doit bend vers la cible la plus centrée (3°), donc x>0 mais modéré.
        let to_near = (near).normalize();
        let to_far = (far).normalize();
        assert!(dir.dot(to_near) > dir.dot(to_far));
    }
}

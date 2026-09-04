//! Energy overlay Roguelite — Story-528 AC4 (rename HP→Énergie + cœurs cartoon).
//!
//! Pattern : overlay non-destructif au-dessus de la HP bar `player_hp.rs`.
//! - Couvre le label "HP" par "ÉNERGIE" (texte foreground même position).
//! - Ajoute 3 cœurs ♥ cartoon à gauche du label (épuisement = cœurs vides).
//! - Fade warm-orange plein écran quand Énergie < 30% (tone fatigué bible v1).
//! - Voiceline placeholder Maître Forgeron à l'épuisement (HP = 0).
//!
//! Anti-canon respecté : aucun mot "die/death/blood/HP", uniquement "Énergie / Repos".
//! Cf `.claude/rules/no-speculative-fix.md` — `player_hp.rs` n'est PAS modifié.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_damage::Health as DamageHealth;
use forgia_player::Player;

const ENERGY_LOW_THRESHOLD: f32 = 0.30;

/// Track edge HP > 0 → HP == 0 pour émettre la voiceline une seule fois.
#[derive(Resource, Default)]
pub(crate) struct EnergyExhaustionLatch {
    pub already_exhausted: bool,
}

// Note (2026-07-22) : le label « ÉNERGIE » + cœurs, jadis peints ici en overlay
// non-destructif au-dessus de `player_hp`, sont désormais intégrés à la carte
// vitals unique (`forgia-mode-roguelite/src/hud.rs::draw_vitals_card`) pour
// supprimer les chevauchements bas-gauche. Ce module ne garde que le fade
// warm-orange plein écran + la voiceline d'épuisement (hors carte).

/// Overlay warm-orange semi-transparent quand énergie < 30%. Pulse léger pour
/// signaler la fatigue sans masquer le gameplay.
pub(crate) fn draw_warm_orange_fade(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    time: Res<Time>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    let Ok(health) = q_player.single() else {
        return;
    };
    let frac = health.fraction();
    if frac > ENERGY_LOW_THRESHOLD {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Pulse alpha 30..55 sinusoidale 1.2Hz. Plus opaque quand frac bas.
    let intensity = 1.0 - (frac / ENERGY_LOW_THRESHOLD).clamp(0.0, 1.0);
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * std::f32::consts::TAU * 1.2).sin();
    let alpha = (intensity * (35.0 + 25.0 * pulse)) as u8;
    let warm = egui::Color32::from_rgba_unmultiplied(255, 107, 53, alpha);
    let rect = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("forgia_energy_warm_fade"),
    ));
    painter.rect_filled(rect, 0.0, warm);
}

/// Système edge-trigger : émet voiceline Maître Forgeron à l'épuisement (HP=0).
/// Reset latch quand HP > 0 (respawn / retour hub).
pub(crate) fn sys_energy_exhaustion_voiceline(
    mut latch: ResMut<EnergyExhaustionLatch>,
    game_mode: Res<State<GameMode>>,
    q_player: Query<&DamageHealth, With<Player>>,
) {
    if *game_mode.get() != GameMode::Roguelite {
        latch.already_exhausted = false;
        return;
    }
    let Ok(health) = q_player.single() else {
        latch.already_exhausted = false;
        return;
    };
    let exhausted = health.current <= 0.0;
    if exhausted && !latch.already_exhausted {
        latch.already_exhausted = true;
        // Placeholder voiceline — audio pipeline TBD (cf ROADMAP_ROGUELITE Tier 3 TTS).
        info!("[forgia-ui-lib::energy] Maître Forgeron: «Repose-toi, Apprenti. La forge te rappellera.»");
    } else if !exhausted {
        latch.already_exhausted = false;
    }
}

pub struct EnergyOverlayPlugin;

impl Plugin for EnergyOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EnergyExhaustionLatch>()
            .add_systems(
                EguiPrimaryContextPass,
                draw_warm_orange_fade.run_if(gameplay_hud_visible),
            )
            .add_systems(Update, sys_energy_exhaustion_voiceline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latch_default_not_exhausted() {
        let l = EnergyExhaustionLatch::default();
        assert!(!l.already_exhausted);
    }

    #[test]
    fn plugin_constructible() {
        let _p = EnergyOverlayPlugin;
    }

    #[test]
    fn energy_low_threshold_is_30_pct() {
        assert!((ENERGY_LOW_THRESHOLD - 0.30).abs() < 1e-5);
    }
}

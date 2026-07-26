//! kill_popup.rs — Story-558 P2 Vlambeer juice (2026-05-29).
//!
//! Pop cartoon texte ("BAM!", "PIF!", "KABOOM!") au point d'impact lors d'un
//! kill enemy en Roguelite. Variante par `EnemyArchetype`. Anim pop scale
//! ease-out-back 450ms.
//!
//! Pattern : Vlambeer "Art of Screenshake" + EarthBound damage numbers +
//!   Cult of the Lamb juice (audit roguelite-engagement-2026-05-29 §6).
//!   Bible v1 cartoon family-friendly (anti-canon Borderlands DPS soup).
//!
//! Cibles d'extension : SFX cartoon (bell/forge clang), particle burst
//! `bevy_hanabi` 1-shot (Phase P3+).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_core::prelude::*;
use forgia_player::FpsCamera;
use forgia_ui_lib::style::ease_out_back;

use crate::enemies::EnemyArchetype;

const POPUP_LIFETIME_SECS: f32 = 0.55;
const POPUP_MAX_RISE_M: f32 = 1.2;
const MAX_POPUPS: usize = 12;

#[derive(Debug, Clone)]
pub struct KillPopup {
    pub world_pos: Vec3,
    pub text: &'static str,
    pub color: egui::Color32,
    pub spawned_secs: f32,
}

#[derive(Resource, Default, Debug)]
pub struct KillPopupState {
    pub active: Vec<KillPopup>,
}

/// Variante texte + couleur par archetype. Vocab CE2 kid-friendly.
pub fn onomatopoeia_for(archetype: EnemyArchetype) -> (&'static str, egui::Color32) {
    match archetype {
        EnemyArchetype::Tank => ("BAM !", egui::Color32::from_rgb(244, 196, 48)), // FORGE_OR
        EnemyArchetype::Runner => ("PIF !", egui::Color32::from_rgb(60, 174, 163)), // FORGE_TEAL
        EnemyArchetype::Sniper => ("PAN !", egui::Color32::from_rgb(231, 76, 60)), // FORGE_BRAISE
        EnemyArchetype::Boss => ("KABOOM !", egui::Color32::from_rgb(255, 244, 220)), // FORGE_CREME
    }
}

/// Listen `CombatHitEvent.is_kill` → push popup avec text par archetype.
/// Cap MAX_POPUPS pour anti-spam (rare en runtime, défense en profondeur).
pub fn sys_track_kill_popups(
    mut events: MessageReader<CombatHitEvent>,
    time: Res<Time>,
    enemies_q: Query<&EnemyArchetype>,
    mut state: ResMut<KillPopupState>,
) {
    for ev in events.read() {
        if !ev.is_kill {
            continue;
        }
        let Ok(archetype) = enemies_q.get(ev.target) else {
            continue;
        };
        let (text, color) = onomatopoeia_for(*archetype);
        state.active.push(KillPopup {
            world_pos: ev.hit_world_pos,
            text,
            color,
            spawned_secs: time.elapsed_secs(),
        });
        if state.active.len() > MAX_POPUPS {
            state.active.remove(0);
        }
    }
    // Expire les popups écoulés.
    let now = time.elapsed_secs();
    state
        .active
        .retain(|p| now - p.spawned_secs < POPUP_LIFETIME_SECS);
}

/// Draw les popups en EguiPrimaryContextPass. World→viewport via Camera.
/// (ease_out_back centralisé dans `forgia_ui_lib::style` — story-596.)
pub fn draw_kill_popups(
    mut contexts: EguiContexts,
    time: Res<Time>,
    state: Res<KillPopupState>,
    cam: Query<(&Camera, &GlobalTransform), With<FpsCamera>>,
) {
    if state.active.is_empty() {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Ok((camera, cam_xf)) = cam.single() else {
        return;
    };
    let now = time.elapsed_secs();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_roguelite_kill_popups"),
    ));
    for p in &state.active {
        let lifetime_t = (now - p.spawned_secs) / POPUP_LIFETIME_SECS;
        if !(0.0..=1.0).contains(&lifetime_t) {
            continue;
        }
        // Rise vertical 1.2m over lifetime + fade in 1st quart, fade out last half.
        let world = p.world_pos + Vec3::Y * (POPUP_MAX_RISE_M * lifetime_t);
        let Ok(screen) = camera.world_to_viewport(cam_xf, world) else {
            continue;
        };
        // Pop scale ease-out-back (overshoot) sur les 200ms initiales.
        let pop_t = (lifetime_t / 0.35).min(1.0);
        let pop_scale = ease_out_back(pop_t);
        // Fade-out 50% dernière moitié.
        let alpha = if lifetime_t < 0.5 {
            1.0
        } else {
            (1.0 - (lifetime_t - 0.5) / 0.5).max(0.0)
        };
        let size = 30.0 * pop_scale;
        let mut color = p.color;
        color = egui::Color32::from_rgba_unmultiplied(
            color.r(),
            color.g(),
            color.b(),
            (alpha * 255.0) as u8,
        );
        let pos = egui::pos2(screen.x, screen.y);
        // Outline noir épais (cartoon "chunky" bible).
        for dx in [-2.0f32, 0.0, 2.0] {
            for dy in [-2.0f32, 0.0, 2.0] {
                if dx == 0.0 && dy == 0.0 {
                    continue;
                }
                painter.text(
                    pos + egui::vec2(dx, dy),
                    egui::Align2::CENTER_CENTER,
                    p.text,
                    egui::FontId::proportional(size),
                    egui::Color32::from_rgba_unmultiplied(20, 20, 20, (alpha * 255.0) as u8),
                );
            }
        }
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            p.text,
            egui::FontId::proportional(size),
            color,
        );
    }
}

pub struct RogueliteKillPopupPlugin;

impl Plugin for RogueliteKillPopupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KillPopupState>()
            .add_systems(
                Update,
                sys_track_kill_popups
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(EguiPrimaryContextPass, draw_kill_popups);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onomatopoeia_tank_is_bam() {
        let (text, _) = onomatopoeia_for(EnemyArchetype::Tank);
        assert_eq!(text, "BAM !");
    }

    #[test]
    fn onomatopoeia_boss_is_kaboom() {
        let (text, _) = onomatopoeia_for(EnemyArchetype::Boss);
        assert_eq!(text, "KABOOM !");
    }

    #[test]
    fn onomatopoeia_all_archetypes_distinct() {
        let t = onomatopoeia_for(EnemyArchetype::Tank).0;
        let r = onomatopoeia_for(EnemyArchetype::Runner).0;
        let s = onomatopoeia_for(EnemyArchetype::Sniper).0;
        let b = onomatopoeia_for(EnemyArchetype::Boss).0;
        assert_ne!(t, r);
        assert_ne!(r, s);
        assert_ne!(s, b);
    }

    #[test]
    fn ease_out_back_boundaries() {
        assert!((ease_out_back(0.0) - 0.0).abs() < 1e-3);
        assert!((ease_out_back(1.0) - 1.0).abs() < 1e-3);
        // Overshoot intermédiaire (signature ease-out-back) — > 1.0 vers 60-80%.
        let mid = ease_out_back(0.7);
        assert!(
            mid > 1.0,
            "ease-out-back doit overshoot vers 70% (got {mid})"
        );
    }

    #[test]
    fn state_default_empty() {
        let s = KillPopupState::default();
        assert!(s.active.is_empty());
    }
}

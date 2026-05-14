//! forgia-xp-curves — XP / level progression with configurable curves.
//!
//! Two curve types supported :
//! - `Linear { base, per_level }` : xp_required(L) = base + per_level * L
//! - `Exponential { base, growth }` : xp_required(L) = base * growth.powi(L)
//!
//! Genome-driven : curve config loaded from TOML (forgia-genome-economy).
//! Components : `XpProgress` (current xp + level), `LevelUpEvent` on tick.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum XpCurve {
    Linear { base: u32, per_level: u32 },
    Exponential { base: u32, growth: f32 },
}

impl XpCurve {
    pub fn xp_required(&self, level: u32) -> u32 {
        match self {
            XpCurve::Linear { base, per_level } => base + per_level * level,
            XpCurve::Exponential { base, growth } => {
                (*base as f32 * growth.powi(level as i32)) as u32
            }
        }
    }
}

impl Default for XpCurve {
    fn default() -> Self {
        XpCurve::Linear { base: 100, per_level: 50 }
    }
}

#[derive(Component, Debug, Clone, Copy)]
pub struct XpProgress {
    pub level: u32,
    pub xp: u32,
    pub curve: XpCurve,
}

impl Default for XpProgress {
    fn default() -> Self {
        Self { level: 1, xp: 0, curve: XpCurve::default() }
    }
}

impl XpProgress {
    pub fn xp_to_next(&self) -> u32 {
        self.curve.xp_required(self.level).saturating_sub(self.xp)
    }

    /// Add xp; returns number of levels gained.
    pub fn add_xp(&mut self, amount: u32) -> u32 {
        self.xp += amount;
        let mut levels_gained = 0;
        while self.xp >= self.curve.xp_required(self.level) {
            self.xp -= self.curve.xp_required(self.level);
            self.level += 1;
            levels_gained += 1;
        }
        levels_gained
    }
}

#[derive(Message, Debug, Clone, Copy)]
pub struct XpGain {
    pub entity: Entity,
    pub amount: u32,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct LevelUpEvent {
    pub entity: Entity,
    pub new_level: u32,
}

pub struct ForgiaXpCurvesPlugin;

impl Plugin for ForgiaXpCurvesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<XpGain>()
            .add_message::<LevelUpEvent>()
            .add_systems(Update, apply_xp_gain);
    }
}

fn apply_xp_gain(
    mut events: MessageReader<XpGain>,
    mut q: Query<&mut XpProgress>,
    mut level_w: MessageWriter<LevelUpEvent>,
) {
    for ev in events.read() {
        let Ok(mut prog) = q.get_mut(ev.entity) else { continue };
        let gained = prog.add_xp(ev.amount);
        for _ in 0..gained {
            level_w.write(LevelUpEvent {
                entity: ev.entity,
                new_level: prog.level,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_curve_xp_required() {
        let c = XpCurve::Linear { base: 100, per_level: 50 };
        assert_eq!(c.xp_required(0), 100);
        assert_eq!(c.xp_required(1), 150);
        assert_eq!(c.xp_required(10), 600);
    }

    #[test]
    fn add_xp_levels_up() {
        let mut p = XpProgress::default(); // base=100, per_level=50, start L1 (need 150)
        let gained = p.add_xp(200);
        assert_eq!(gained, 1); // L1->L2
        assert_eq!(p.level, 2);
        assert_eq!(p.xp, 50); // 200 - 150 leftover
    }

    #[test]
    fn add_xp_multi_levels() {
        let mut p = XpProgress::default();
        // L1 needs 150, L2 needs 200, L3 needs 250 ... so 1000 xp -> L1+L2+L3+L4 ≈ 4 levels
        let gained = p.add_xp(1000);
        assert!(gained >= 3);
    }
}

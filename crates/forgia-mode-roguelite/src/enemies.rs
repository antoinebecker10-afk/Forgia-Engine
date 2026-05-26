//! enemies.rs — M2 step 2 : 3 archetypes ennemis Roguelite (data-driven).
//!
//! Story-470 M2 step 2 — résout le bug "ennemis trop collés" en variant
//! `stop_distance` et `attack_range` par archetype : naturellement dispersés
//! par design plutôt que via Boids separation (option B reportée).
//!
//! | Archetype | HP   | Speed | Stop | Attack | Cooldown | Couleur     | Rôle |
//! |-----------|------|-------|------|--------|----------|-------------|------|
//! | Tank      |  120 |  2.8  |  3.0 |   4.0  |   1.8s   | rouge sombre | front-line tank |
//! | Runner    |   35 |  7.0  |  6.0 |   7.0  |   0.7s   | orange       | rusher proche |
//! | Sniper    |   45 |  3.2  | 22.0 |  24.0  |   1.6s   | violet       | back-line ranged |
//!
//! Hot-reload TOML : reporté M2 step 3 (`config/genomes/roguelite_enemies.toml`).

use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;

/// Marker Component pour identifier l'archetype runtime (debug + futur loot/xp).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnemyArchetype {
    Tank,
    Runner,
    Sniper,
    /// M3 step 1 — boss final wave 3. Unique, tanky, 2 phases (enrage à 50% HP).
    Boss,
}

impl EnemyArchetype {
    /// Nom court pour logs / sensor / debug.
    pub fn label(self) -> &'static str {
        match self {
            Self::Tank => "tank",
            Self::Runner => "runner",
            Self::Sniper => "sniper",
            Self::Boss => "boss",
        }
    }
}

/// Stats d'un archetype — data pure (struct = source of truth, pas de TOML M2 step 2).
#[derive(Debug, Clone, Copy)]
pub struct EnemyStats {
    pub hp: f32,
    pub speed: f32,
    pub stop_distance: f32,
    pub attack_range: f32,
    pub detect_range: f32,
    pub attack_cooldown: f32,
    pub warmup_secs: f32,
    pub capsule_radius: f32,
    pub capsule_half_height: f32,
    /// Couleur base RGB linéaire (StandardMaterial.base_color).
    pub color_rgb: [f32; 3],
    /// Couleur émissive RGB linéaire (faible — silhouette pop sans flash).
    pub emissive_rgb: [f32; 3],
}

/// Stats par archetype — pure, testable, data-driven.
pub fn stats_for(archetype: EnemyArchetype) -> EnemyStats {
    match archetype {
        EnemyArchetype::Tank => EnemyStats {
            hp: 120.0,
            speed: 2.8,
            stop_distance: 3.0,
            attack_range: 4.0,
            detect_range: 22.0,
            attack_cooldown: 1.8,
            warmup_secs: 2.0,
            capsule_radius: 0.55,
            capsule_half_height: 0.75,
            color_rgb: [0.55, 0.10, 0.10],
            emissive_rgb: [0.25, 0.03, 0.03],
        },
        EnemyArchetype::Runner => EnemyStats {
            hp: 35.0,
            speed: 7.0,
            stop_distance: 6.0,
            attack_range: 7.0,
            detect_range: 40.0,
            attack_cooldown: 0.7,
            warmup_secs: 1.2,
            capsule_radius: 0.32,
            capsule_half_height: 0.60,
            color_rgb: [0.95, 0.45, 0.10],
            emissive_rgb: [0.40, 0.18, 0.04],
        },
        EnemyArchetype::Sniper => EnemyStats {
            hp: 45.0,
            speed: 3.2,
            stop_distance: 22.0,
            attack_range: 24.0,
            detect_range: 55.0,
            attack_cooldown: 1.6,
            warmup_secs: 1.8,
            capsule_radius: 0.30,
            capsule_half_height: 0.85,
            color_rgb: [0.55, 0.20, 0.80],
            emissive_rgb: [0.22, 0.08, 0.35],
        },
        // M3 step 1 — boss : tanky, mi-range, intimidant. Capsule géante 3×Tank.
        // Phase 2 enrage à 50% HP : speed×1.8, cooldown×0.55 (cf sys_boss_enrage).
        EnemyArchetype::Boss => EnemyStats {
            hp: 800.0,
            speed: 3.5,
            stop_distance: 10.0,
            attack_range: 30.0,
            detect_range: 80.0,
            attack_cooldown: 1.3,
            warmup_secs: 2.5,
            capsule_radius: 1.4,
            capsule_half_height: 2.2,
            color_rgb: [0.90, 0.10, 0.50],
            emissive_rgb: [0.50, 0.05, 0.25],
        },
    }
}

/// KayKit Skeleton GLB asset path par archetype (story-517 fix).
/// Mapping : Tank=Warrior, Runner=Minion, Sniper=Mage, Boss=Warrior×2.5.
/// Memory ref : reference_roguelite_enemy_skeleton_mapping.md.
pub fn skeleton_asset_path(archetype: EnemyArchetype) -> &'static str {
    match archetype {
        EnemyArchetype::Tank => "models/kaykit/skeletons/Skeleton_Warrior.glb",
        EnemyArchetype::Runner => "models/kaykit/skeletons/Skeleton_Minion.glb",
        EnemyArchetype::Sniper => "models/kaykit/skeletons/Skeleton_Mage.glb",
        EnemyArchetype::Boss => "models/kaykit/skeletons/Skeleton_Warrior.glb",
    }
}

/// Scale uniforme à appliquer au SceneRoot KayKit pour matcher la silhouette
/// capsule originale (visual cohérence avec les colliders existants).
pub fn skeleton_scale(archetype: EnemyArchetype) -> f32 {
    match archetype {
        EnemyArchetype::Tank => 1.4,
        EnemyArchetype::Runner => 1.0,
        EnemyArchetype::Sniper => 1.1,
        EnemyArchetype::Boss => 2.5,
    }
}

/// Construit un `ArenaBot` configuré pour l'archetype donné.
/// Réutilise les champs default pour les LOS/strafe/alert (préservés inchangés).
pub fn arena_bot_for(archetype: EnemyArchetype) -> ArenaBot {
    let s = stats_for(archetype);
    ArenaBot {
        speed: s.speed,
        detect_range: s.detect_range,
        attack_range: s.attack_range,
        attack_cooldown: s.attack_cooldown,
        attack_left: s.warmup_secs,
        stop_distance: s.stop_distance,
        ..ArenaBot::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archetype_labels_distinct() {
        assert_ne!(EnemyArchetype::Tank.label(), EnemyArchetype::Runner.label());
        assert_ne!(
            EnemyArchetype::Runner.label(),
            EnemyArchetype::Sniper.label()
        );
        assert_eq!(EnemyArchetype::Tank.label(), "tank");
    }

    #[test]
    fn stats_tank_high_hp_low_speed() {
        let t = stats_for(EnemyArchetype::Tank);
        let r = stats_for(EnemyArchetype::Runner);
        assert!(t.hp > r.hp, "Tank doit avoir plus d'HP qu'un Runner");
        assert!(t.speed < r.speed, "Tank doit être plus lent qu'un Runner");
    }

    #[test]
    fn stats_sniper_long_range() {
        let s = stats_for(EnemyArchetype::Sniper);
        let t = stats_for(EnemyArchetype::Tank);
        assert!(
            s.attack_range > t.attack_range * 4.0,
            "Sniper doit avoir une range >4× celle d'un Tank"
        );
        assert!(s.stop_distance >= 20.0, "Sniper s'arrête au moins à 20m");
    }

    #[test]
    fn stop_distances_distinct_for_spread() {
        // Cœur du fix sticky AI : les 3 archetypes ont des stop_distance distincts
        // donc se positionnent naturellement à des cercles différents autour du player.
        let t = stats_for(EnemyArchetype::Tank).stop_distance;
        let r = stats_for(EnemyArchetype::Runner).stop_distance;
        let s = stats_for(EnemyArchetype::Sniper).stop_distance;
        assert!(t < r, "Tank stop < Runner stop");
        assert!(r < s, "Runner stop < Sniper stop");
        assert!(s - t > 15.0, "Spread total entre Tank et Sniper > 15m");
    }

    #[test]
    fn arena_bot_for_uses_stats() {
        let bot = arena_bot_for(EnemyArchetype::Runner);
        let stats = stats_for(EnemyArchetype::Runner);
        assert!((bot.speed - stats.speed).abs() < 0.001);
        assert!((bot.stop_distance - stats.stop_distance).abs() < 0.001);
        assert!((bot.attack_range - stats.attack_range).abs() < 0.001);
        assert!((bot.attack_left - stats.warmup_secs).abs() < 0.001);
    }
}

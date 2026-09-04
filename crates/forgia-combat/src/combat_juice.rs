#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/effects/combat_juice.rs` (V1).
//! Combat Juice â€” Hitstop, Trauma Shake, Hit Flash, Kill SlowMo
//!
//! Transforms combat from "numbers going down" to visceral impact.
//! Uses observer pattern: detects health changes on goblins each frame.

use bevy::prelude::*;

// TODO: port from V1 â€” ChromaticAberration
// use bevy::post_process::effect_stack::ChromaticAberration;

// TODO: port from V1 â€” components (GoblinGuard, FpsCamera, CameraState)
// use forgia_player::components::{FpsCamera, CameraState};

// TODO: port from V1 â€” ai::arena_bot::BotHealth
// use forgia_ai_arena_bot::BotHealth;

// TODO: port from V1 â€” resources::FpsTuning, WeaponFireFlash
// use forgia_core::resources::{FpsTuning, WeaponFireFlash};

/// Message emitted when damage is detected on an enemy.
///
/// Story-455 Phase C (2026-05-18) — étendu pour alimenter kill feed (Phase D) et
/// damage direction indicator (Phase E) sans pull-based queries.
///
/// Fields :
/// - `target` : entité qui a reçu le coup (porte `Health`).
/// - `attacker` : entité qui a tiré (Player ou Bot ; None pour world damage futur).
/// - `damage` : dégâts effectifs appliqués (après falloff/headshot mul).
/// - `is_kill` : true si HP atteint 0 après ce coup.
/// - `is_headshot` : true si le ray a touché une `HitZoneHead`. ⚠ Story-455 Phase C
///   reste à `false` jusqu'à hitzone Head/Body split (story-456 deferred).
/// - `hit_world_pos` : position monde du point d'impact (pour DDI angle + popup spawn).
/// - `weapon` : arme utilisée (pour kill feed icon mapping). None = world/melee.
/// - `body_zone` : zone du corps touchée (story-457, 2026-05-19). Pilote
///   damage multiplier + visual style (couleur/taille floating number +
///   label nameplate). `Body` par défaut si aucun `HitZoneTag` rencontré.
#[derive(Message)]
pub struct CombatHitEvent {
    pub target: Entity,
    pub attacker: Option<Entity>,
    /// Dégât **nominal** de l'arme (falloff + zone + crit + mods), AVANT les
    /// modificateurs propres à la cible (vulnérabilité `Vulnerability`, absorption
    /// `DefenseLayer`). Contrat stable : base sur laquelle la couche élémentaire
    /// (forgia-mode-roguelite) calcule bonus/réactions SANS double-compter la vuln
    /// (story-642 P0-4 Inc.3). Le dégât réellement retiré à la Vie peut être supérieur
    /// (×vuln) ou inférieur (absorbé par la couche). `is_kill` reflète la mort RÉELLE.
    pub damage: f32,
    pub is_kill: bool,
    pub is_headshot: bool,
    pub hit_world_pos: Vec3,
    pub weapon: Option<crate::weapons::WeaponType>,
    pub body_zone: forgia_damage::HitZone,
}

/// Story-650 — mult knockback par arme (mapping WeaponType legacy → arme V2,
/// même convention que weapon_vfx). Les valeurs vivent dans le genome
/// (`roguelite_gamefeel.toml [knockback_mult_*]`) : Boucherie PROJETTE, Pépin picote.
fn weapon_knockback_mult(
    w: crate::weapons::WeaponType,
    t: &forgia_juice_lib::knockback::KnockbackTuning,
) -> f32 {
    use crate::weapons::WeaponType;
    match w {
        WeaponType::ModernAR => t.mult_pepin, // Pépin (pistolet)
        WeaponType::AssaultRifle => t.mult_bourrasque, // Bourrasque (SMG)
        WeaponType::Shotgun => t.mult_lenoir, // Madame Lenoir (sniper)
        WeaponType::RocketLauncher => t.mult_boucherie, // Boucherie (pump)
        _ => t.mult_default,
    }
}

/// Story-650 — knockback à l'impact (trick Vlambeer) : chaque `CombatHitEvent`
/// pousse l'ennemi dans la direction attaquant→cible (horizontale), kill = plus
/// fort + pop vertical. Les ennemis étant kinematic (waves.rs), la poussée passe
/// par le composant `Knockback` (forgia-juice-lib) qui déplace le Transform avec
/// décroissance — additif, compose avec le mouvement des bots.
pub fn sys_apply_hit_knockback(
    mut commands: Commands,
    mut events: MessageReader<CombatHitEvent>,
    tuning: Option<Res<forgia_juice_lib::knockback::KnockbackTuning>>,
    mut stats: Option<ResMut<forgia_juice_lib::knockback::KnockbackStats>>,
    q_pos: Query<&GlobalTransform>,
    mut q_kb: Query<&mut forgia_juice_lib::knockback::Knockback>,
    // Story-680 cran 5 — `PlayerCombatMods.knockback_strength` était calculé par
    // `boons_apply` et **personne ne le lisait**. Les 2 atouts « Impact » du
    // catalogue ne faisaient donc RIEN. La poussée ne venait que du génome
    // d'arme ; les boons n'y contribuaient pas.
    mods: Option<Res<crate::combat_mods::PlayerCombatMods>>,
) {
    use forgia_juice_lib::knockback::{
        accumulate_knockback, displacement_m, knockback_velocity, Knockback,
    };
    let Some(tuning) = tuning else { return };
    for ev in events.read() {
        let Some(attacker) = ev.attacker else {
            continue; // world damage : pas de direction de poussée
        };
        let (Ok(a_gt), Ok(t_gt)) = (q_pos.get(attacker), q_pos.get(ev.target)) else {
            continue;
        };
        let dir = t_gt.translation() - a_gt.translation();
        let weapon_mult = ev
            .weapon
            .map(|w| weapon_knockback_mult(w, &tuning))
            .unwrap_or(tuning.mult_default); // None = melee/world → défaut
                                             // Les atouts s'ajoutent MULTIPLICATIVEMENT au multiplicateur d'arme :
                                             // « Impact » renforce le caractère de l'arme au lieu de l'écraser — une
                                             // Boucherie boostée projette encore plus qu'un Pépin boosté.
        let boon_mult = 1.0 + mods.as_deref().map(|m| m.knockback_strength).unwrap_or(0.0);
        let weapon_mult = weapon_mult * boon_mult;
        let v = knockback_velocity(dir, &tuning, weapon_mult, ev.is_kill);
        if v == Vec3::ZERO {
            continue;
        }
        if let Ok(mut kb) = q_kb.get_mut(ev.target) {
            kb.vel = accumulate_knockback(kb.vel, v, &tuning);
        } else if let Ok(mut ec) = commands.get_entity(ev.target) {
            ec.try_insert(Knockback {
                vel: accumulate_knockback(Vec3::ZERO, v, &tuning),
            });
        }
        if let Some(stats) = stats.as_mut() {
            stats.pushes = stats.pushes.saturating_add(1);
            if ev.is_kill {
                stats.kill_pushes = stats.kill_pushes.saturating_add(1);
            }
            stats.last_displacement_m = displacement_m(v);
        }
    }
}

/// Story-559 slice B (2026-06-04) — émis à CHAQUE tir effectif du joueur (hit OU
/// miss), juste après la consommation de munition. Porte le `WeaponType` courant
/// pour jouer le son de tir propre à l'arme (audio Roguelite) + futur muzzle flash.
#[derive(Message)]
pub struct WeaponFiredEvent {
    pub shooter: Entity,
    pub weapon: crate::weapons::WeaponType,
}

/// Tracks previous health to detect damage via change detection.
#[derive(Component)]
pub struct PrevHealth(pub f32);

// TODO: detect_combat_hits requires GoblinGuard + BotHealth (cross-crate deps)
// pub fn detect_combat_hits(...) { ... }

// â”€â”€ Resources â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

// ⛔ HITSTOP : RETIRÉ DÉFINITIVEMENT — décision produit, 2026-08-12.
//
// « On le retire, je n'en veux pas, jamais. »
//
// Ce n'est pas un manque à combler, c'est un choix de game feel. Il avait déjà été
// désactivé le 2026-07-20 pour une raison mesurée : **micro-freezes ressentis en
// tir soutenu**, invisibles aux capteurs de frame-time. Un gel par-hit à 16 tirs/s
// ne se lit pas dans une moyenne de frame — il se sent, et il gênait.
//
// Historique, pour que personne ne le redécouvre : `HitStopState` avait été migré
// vers une crate `forgia-juice-hit-stop` le 2026-05-17, supprimée neuf jours plus
// tard par ADR-0002. Les gènes `wfx_hit_stop_*` ont été retirés des génomes le
// 2026-08-12 (ils n'avaient aucun lecteur).
//
// NE PAS LE RECRÉER, ni sous ce nom ni sous un autre (« freeze frame », « impact
// pause », « gel d'impact »). Si le besoin de poids à l'impact revient, la réponse
// est ailleurs : knockback, screenshake, flash — tous présents dans
// `forgia-juice-lib`, tous sans gel du temps.

#[derive(Resource, Default)]
pub struct CameraTrauma {
    pub trauma: f32,
    pub time_acc: f32,
}

impl CameraTrauma {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
    pub fn shake_amount(&self) -> f32 {
        self.trauma * self.trauma
    }
}

/// Emissive du flash de hit — blanc HDR. Invariant juice interne (valeur héritée
/// de l'ex-`HitFlashCache.flash_material`, story-432 V4) ; pas exposé créateur.
pub const HIT_FLASH_EMISSIVE: LinearRgba = LinearRgba::new(8.0, 8.0, 8.0, 1.0);

#[derive(Component)]
pub struct HitFlashTimer {
    pub timer: Timer,
    /// Emissive d'ORIGINE du matériau, capturée au 1er hit — restaurée à expiry.
    pub original_emissive: LinearRgba,
    /// Audit fire-path 2026-07-20 (suspect « hit-flash swap ») : le flash mute
    /// désormais `emissive` EN PLACE sur le matériau de l'entité (handle
    /// inchangé → l'entité ne quitte jamais son batch GPU ; zéro swap de
    /// `MeshMaterial3d` per-hit, zéro `HitFlashCache`). Remplace le swap vers
    /// un flash-material partagé (story-432 V4). Prérequis : le handle est
    /// propre à l'entité (muter un matériau partagé flasherait tous les pairs
    /// — bug 2026-04-27) ; seul chemin poseur = racine ennemie portant son
    /// propre `MeshMaterial3d` (forgia-fps fire path).
    pub material: Handle<StandardMaterial>,
}

// â”€â”€ Systems â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

// TODO: combat_juice_event_system requires FpsTuning + ChromaticAberration (Bevy pp) + HitFlashCache
// pub fn combat_juice_event_system(...) { ... }

// ⛔ `hitstop_tick_system` : retiré définitivement — cf en-tête « HITSTOP ».
// L'ancienne ligne affirmait « Wiring : ForgiaJuiceHitStopPlugin ajouté idempotent
// dans ForgiaCombatPlugin ». Ce plugin n'existe nulle part, et ne doit pas revenir.

pub fn trauma_decay_system(time: Res<Time>, mut trauma: ResMut<CameraTrauma>) {
    if trauma.trauma > 0.001 {
        trauma.trauma *= (-4.0 * time.delta_secs()).exp();
        trauma.time_acc += time.delta_secs();
        if trauma.trauma < 0.001 {
            trauma.trauma = 0.0;
        }
    }
}

/// Camera shake offset from trauma (call from camera system).
pub fn compute_trauma_offset(trauma: &CameraTrauma) -> Vec3 {
    if trauma.trauma < 0.001 {
        return Vec3::ZERO;
    }
    let shake = trauma.shake_amount();
    let t = trauma.time_acc;
    let offset_x = (t * 23.7).sin() * (t * 41.3).cos();
    let offset_y = (t * 31.1).sin() * (t * 53.7).cos();
    let max_offset = 0.15;
    Vec3::new(
        offset_x * shake * max_offset,
        offset_y * shake * max_offset,
        0.0,
    )
}

pub fn hit_flash_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut HitFlashTimer)>,
) {
    // Audit fire-path 2026-07-20 : tick le timer ; à expiry, restaure
    // l'emissive d'origine EN PLACE (même handle → l'entité ne quitte
    // jamais son batch GPU ; remplace le swap MeshMaterial3d story-432 V4).
    for (entity, mut flash) in &mut query {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            if let Some(mat) = materials.get_mut(&flash.material) {
                mat.emissive = flash.original_emissive;
            }
            commands.entity(entity).remove::<HitFlashTimer>();
        }
    }
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Camera Recoil (story-432 V2) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//
// Pattern Apex/COD : sur tir, push pitch â†‘ + petit yaw random, puis "dette"
// dÃ©croÃ®t exponentiellement â†’ camÃ©ra recentre auto si pas d'input joueur.
// Modifie `CameraState.pitch/yaw` directement (aim authentique), pas un overlay
// visuel. Le joueur peut compenser activement en pullant le mouse (skill).
//
// Event-driven : `weapon_fire_system` Ã©met `WeaponRecoilImpulse`, lu ici.

// WeaponRecoilImpulse + WeaponRecoilDebt : extraits vers `forgia-juice-recoil` (Tier 1E, 2026-05-17).
// Re-export backward compat (prelude). Preferer `forgia_juice_lib::recoil::*` direct dans le nouveau code.
pub use forgia_juice_lib::recoil::{WeaponRecoilDebt, WeaponRecoilImpulse};

// TODO: weapon_recoil_system requires CameraState + FpsCamera (forgia-camera-fps)
//       + WeaponRecoilImpulse MessageReader (Bevy 0.18 message API)
// pub fn weapon_recoil_system(...) { ... }

// TODO: weapon_fire_flash_system requires ChromaticAberration (bevy::post_process)
//       + FpsTuning + WeaponFireFlash + FpsCamera
// pub fn weapon_fire_flash_system(...) { ... }

#[cfg(test)]
mod tests {
    use super::*;

    // â”€â”€ CameraTrauma::default â”€â”€

    #[test]
    fn camera_trauma_default_is_zero() {
        let t = CameraTrauma::default();
        assert_eq!(t.trauma, 0.0);
        assert_eq!(t.time_acc, 0.0);
    }

    // â”€â”€ CameraTrauma::add â”€â”€

    #[test]
    fn camera_trauma_add_accumulates() {
        let mut t = CameraTrauma::default();
        t.add(0.3);
        assert!((t.trauma - 0.3).abs() < 1e-5);
        t.add(0.4);
        assert!((t.trauma - 0.7).abs() < 1e-5);
    }

    #[test]
    fn camera_trauma_add_clamps_to_one() {
        let mut t = CameraTrauma::default();
        t.add(0.6);
        t.add(0.6); // would overshoot to 1.2 â†’ must clamp.
        assert_eq!(t.trauma, 1.0);
    }

    #[test]
    fn camera_trauma_add_single_huge_value_clamps_too() {
        let mut t = CameraTrauma::default();
        t.add(5.0);
        assert_eq!(t.trauma, 1.0);
    }

    #[test]
    fn camera_trauma_add_zero_is_noop() {
        let mut t = CameraTrauma::default();
        t.add(0.5);
        let before = t.trauma;
        t.add(0.0);
        assert_eq!(t.trauma, before);
    }

    // â”€â”€ CameraTrauma::shake_amount â”€â”€

    #[test]
    fn shake_amount_is_quadratic() {
        let mut t = CameraTrauma::default();
        // shake_amount = traumaÂ²
        assert_eq!(t.shake_amount(), 0.0);
        t.trauma = 0.5;
        assert!((t.shake_amount() - 0.25).abs() < 1e-5);
        t.trauma = 1.0;
        assert!((t.shake_amount() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shake_amount_grows_faster_than_trauma() {
        // Quadratic curve: between 0 and 1, shake is always <= trauma.
        let mut t = CameraTrauma::default();
        for trauma in [0.1f32, 0.3, 0.5, 0.7, 0.9] {
            t.trauma = trauma;
            assert!(
                t.shake_amount() <= trauma,
                "quadratic shake must be <= linear trauma at {trauma}: got {}",
                t.shake_amount()
            );
        }
    }
}

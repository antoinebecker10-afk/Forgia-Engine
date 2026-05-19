//! forgia-damage — Health + DamageEvent + DeathEvent atomic crate + HitZone types.
//!
//! Story-457 (2026-05-19) — Ajout couche hitzone : `HitZone` enum,
//! `HitZoneTag` component (collider-side), `HitFeedbackTuning` genome-driven.
//!
//! No deps on weapons/AI/UI — they emit `DamageEvent`, this crate consumes
//! them and mutates `Health`, then emits `DeathEvent` when HP <= 0.
//!
//! Bevy 0.18.1 — `Event` renamed to `Message`.

use bevy::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;

/// Per-entity health. Add to any entity that can take damage.
#[derive(Component, Debug, Clone, Copy)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 { 0.0 } else { (self.current / self.max).clamp(0.0, 1.0) }
    }
}

/// Marker for entities whose death should trigger respawn / cleanup elsewhere.
#[derive(Component, Default)]
pub struct Mortal;

#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    pub target: Entity,
    pub source: Option<Entity>,
    pub amount: f32,
    pub kind: DamageKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageKind {
    Physical,
    Fire,
    Poison,
    Explosion,
    Fall,
    Other,
}

/// Story-466 — DeathEvent migré Message → EntityEvent (Observer pattern Bevy 0.18).
///
/// Le champ `target` est annoté `#[event_target]` car le derive `EntityEvent`
/// route auto vers les observers per-entity. Le nom `target` est conservé
/// (vs renommer `entity`) pour préserver l'API existante (`ev.target` lecture).
///
/// Consume via Observer : `app.add_observer(|event: On<DeathEvent>, ...| {...})`.
/// Trigger via `commands.trigger(DeathEvent { target, source, final_kind })`.
#[derive(EntityEvent, Debug, Clone, Copy)]
pub struct DeathEvent {
    #[event_target]
    pub target: Entity,
    pub source: Option<Entity>,
    pub final_kind: DamageKind,
}

// ─── HitZone (story-457) ─────────────────────────────────────────────

/// Zone du corps touchée par un projectile. Détermine le multiplicateur de
/// dégâts + la couleur/taille du floating number + le label sur le nameplate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HitZone {
    Head,
    #[default]
    Body,
    Limb,
}

impl HitZone {
    pub fn as_str(self) -> &'static str {
        match self {
            HitZone::Head => "head",
            HitZone::Body => "body",
            HitZone::Limb => "limb",
        }
    }
}

/// Tag inséré sur un *child* collider (typiquement un Sensor sphere proxy)
/// pour signaler que tout ray hit doit être qualifié comme cette zone.
///
/// Le système fire walk `ChildOf` ancestors pour trouver le Health, puis lit
/// le tag du collider directement frappé pour qualifier la zone.
#[derive(Component, Debug, Clone, Copy)]
pub struct HitZoneTag(pub HitZone);

/// Tuning genome-driven : multiplicateurs damage + visual style par zone.
///
/// Schéma volontairement plat (vs nested) pour TOML lisible + hot-reload.
#[derive(Deserialize, TypePath, Clone, Debug)]
pub struct HitFeedbackTuning {
    pub head_damage_mul: f32,
    pub body_damage_mul: f32,
    pub limb_damage_mul: f32,

    /// Couleur RGB floating number par zone (linear).
    pub head_color: [f32; 3],
    pub body_color: [f32; 3],
    pub limb_color: [f32; 3],

    /// Taille font px floating number par zone.
    pub head_font_size: f32,
    pub body_font_size: f32,
    pub limb_font_size: f32,
}

impl Default for HitFeedbackTuning {
    fn default() -> Self {
        Self {
            head_damage_mul: 2.0,
            body_damage_mul: 1.0,
            limb_damage_mul: 0.7,
            head_color: [1.0, 0.9, 0.2],
            body_color: [1.0, 1.0, 1.0],
            limb_color: [0.8, 0.8, 0.8],
            head_font_size: 30.0,
            body_font_size: 20.0,
            limb_font_size: 16.0,
        }
    }
}

impl HitFeedbackTuning {
    pub fn damage_mul(&self, zone: HitZone) -> f32 {
        match zone {
            HitZone::Head => self.head_damage_mul,
            HitZone::Body => self.body_damage_mul,
            HitZone::Limb => self.limb_damage_mul,
        }
    }
    pub fn color(&self, zone: HitZone) -> [f32; 3] {
        match zone {
            HitZone::Head => self.head_color,
            HitZone::Body => self.body_color,
            HitZone::Limb => self.limb_color,
        }
    }
    pub fn font_size(&self, zone: HitZone) -> f32 {
        match zone {
            HitZone::Head => self.head_font_size,
            HitZone::Body => self.body_font_size,
            HitZone::Limb => self.limb_font_size,
        }
    }
}

#[derive(Resource)]
pub struct HitFeedbackTuningHandle(pub Handle<Genome<HitFeedbackTuning>>);

/// Resource exposée au runtime — sync depuis le genome chargé.
/// Existe toujours (Default au boot, écrasée au hot-reload).
#[derive(Resource, Default, Debug, Clone)]
pub struct HitFeedback(pub HitFeedbackTuning);

// ─── Event "post-application damage" (story-457 fwd-compat) ──────────

/// Émise APRÈS que la `Health` ait été mutée, contrairement à `DamageEvent`
/// qui est la demande PRE-application. Permet aux consommateurs UI/VFX de
/// lire l'état final sans rejouer l'arithmétique.
///
/// Note 2026-05-19 : la fps lib.rs utilise actuellement `CombatHitEvent`
/// directement. Ce type reste exposé pour un futur refacto où le flux
/// passerait par `DamageEvent` → `apply_damage` → `DamageAppliedEvent`.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageAppliedEvent {
    pub target: Entity,
    pub source: Option<Entity>,
    pub amount: f32,
    pub zone: HitZone,
    pub kind: DamageKind,
    pub final_hp: f32,
    pub is_kill: bool,
}

// ─── Plugin ──────────────────────────────────────────────────────────

pub struct ForgiaDamagePlugin;

impl Plugin for ForgiaDamagePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<DamageEvent>()
            // Story-466 — DeathEvent migré vers EntityEvent (Observer). Plus
            // d'add_message car les consommateurs utilisent `On<DeathEvent>`.
            .add_message::<DamageAppliedEvent>()
            .init_asset::<Genome<HitFeedbackTuning>>()
            .register_asset_loader(GenomeLoader::<HitFeedbackTuning>::default())
            .init_resource::<HitFeedback>()
            .add_systems(Startup, load_hit_feedback)
            .add_systems(Update, (apply_damage, sync_hit_feedback));
    }
}

fn load_hit_feedback(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<HitFeedbackTuning>> =
        asset_server.load("genomes/damage_feedback/hit_feedback.toml");
    commands.insert_resource(HitFeedbackTuningHandle(handle));
}

fn sync_hit_feedback(
    handle: Option<Res<HitFeedbackTuningHandle>>,
    assets: Res<Assets<Genome<HitFeedbackTuning>>>,
    mut feedback: ResMut<HitFeedback>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    feedback.0 = g.data.clone();
}

fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut healths: Query<&mut Health>,
    mut applied: MessageWriter<DamageAppliedEvent>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let Ok(mut hp) = healths.get_mut(ev.target) else { continue };
        if !hp.is_alive() { continue; }
        hp.current = (hp.current - ev.amount).max(0.0);
        let is_kill = hp.current <= 0.0;
        applied.write(DamageAppliedEvent {
            target: ev.target,
            source: ev.source,
            amount: ev.amount,
            zone: HitZone::Body,
            kind: ev.kind,
            final_hp: hp.current,
            is_kill,
        });
        if is_kill {
            // Story-466 — DeathEvent fire via Observer (EntityEvent).
            // commands.trigger routes auto vers observers ciblant `target`.
            commands.trigger(DeathEvent {
                target: ev.target,
                source: ev.source,
                final_kind: ev.kind,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitzone_default_is_body() {
        assert_eq!(HitZone::default(), HitZone::Body);
    }

    #[test]
    fn hit_feedback_default_multipliers_sane() {
        let t = HitFeedbackTuning::default();
        assert!(t.head_damage_mul > t.body_damage_mul);
        assert!(t.body_damage_mul > t.limb_damage_mul);
    }

    #[test]
    fn damage_mul_dispatch() {
        let t = HitFeedbackTuning::default();
        assert_eq!(t.damage_mul(HitZone::Head), 2.0);
        assert_eq!(t.damage_mul(HitZone::Body), 1.0);
        assert_eq!(t.damage_mul(HitZone::Limb), 0.7);
    }
}

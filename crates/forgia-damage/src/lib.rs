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

pub mod defense;
pub use defense::{DamageChannel, DefenseLayer, ElementAffinity};

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
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }
}

/// Marker for entities whose death should trigger respawn / cleanup elsewhere.
#[derive(Component, Default)]
pub struct Mortal;

/// Story-558 Phase 4b (2026-05-29) — réduit le `DamageEvent.amount` final
/// avant soustraction Health. `reduction` ∈ [0..1] = fraction de dégâts
/// évitée. Inséré et maintenu par `forgia-mode-roguelite` sur le Player
/// quand des boons `damage_reduction` sont actifs (e.g. Bénédiction de
/// l'Enclume = 0.10 = -10% dégâts subis).
///
/// Pattern cross-crate clean : forgia-damage reste agnostique du concept
/// "Player" (lookup générique sur le component, no dep inversion).
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct HealthGuard {
    pub reduction: f32,
}

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
    guards: Query<&HealthGuard>,
    // Story-640 P0-2 — couche défensive optionnelle (Bouclier/Armure) sur la cible
    // (typiquement le joueur, attaché par forgia-mode-roguelite). Absorbe AVANT la Vie.
    mut defenses: Query<&mut DefenseLayer>,
    mut applied: MessageWriter<DamageAppliedEvent>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let Ok(mut hp) = healths.get_mut(ev.target) else {
            continue;
        };
        if !hp.is_alive() {
            continue;
        }
        // Story-558 Phase 4b (2026-05-29) — HealthGuard component (forgia-mode-roguelite
        // l'insère sur Player avec reduction = combat_mods.damage_reduction).
        // Réduit `amount` final avant soustraction Health. Cumul multiplicatif
        // si plusieurs guards (rare — actuellement 1 sur Player Roguelite).
        let reduction = guards
            .get(ev.target)
            .map(|g| g.reduction.clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let effective_amount = ev.amount * (1.0 - reduction);
        // Story-640 P0-2 — la couche défensive absorbe Bouclier → Armure avant la Vie.
        // Canal dérivé du `kind` : Feu/Poison bypassent (couplage « Feu→Vie »,
        // « Poison DoT pur ») ; le reste (physique/explosion/chute) est absorbé. Tout
        // coup gèle la régénération (`note_hit`), même entièrement absorbé.
        let to_health = if let Ok(mut dl) = defenses.get_mut(ev.target) {
            let channel = match ev.kind {
                DamageKind::Fire | DamageKind::Poison => DamageChannel::TrueHealth,
                _ => DamageChannel::Physical,
            };
            dl.note_hit();
            dl.absorb(effective_amount, channel)
        } else {
            effective_amount
        };
        hp.current = (hp.current - to_health).max(0.0);
        let is_kill = hp.current <= 0.0;
        applied.write(DamageAppliedEvent {
            target: ev.target,
            source: ev.source,
            amount: effective_amount,
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

    // ─── apply_damage headless (story-594 M2-B7, audit 2026-06-10 P1) ───
    // Le chemin chaque-frame du combat n'avait AUCUN test. Harness minimal :
    // App nu + messages + observer DeathEvent collecté dans une Resource.

    #[derive(Resource, Default)]
    struct AppliedLog(Vec<DamageAppliedEvent>);

    #[derive(Resource, Default)]
    struct DeathLog(Vec<Entity>);

    fn collect_applied(mut reader: MessageReader<DamageAppliedEvent>, mut log: ResMut<AppliedLog>) {
        for ev in reader.read() {
            log.0.push(*ev);
        }
    }

    fn damage_test_app() -> App {
        let mut app = App::new();
        app.add_message::<DamageEvent>()
            .add_message::<DamageAppliedEvent>()
            .init_resource::<AppliedLog>()
            .init_resource::<DeathLog>()
            .add_systems(Update, (apply_damage, collect_applied).chain())
            .add_observer(|ev: On<DeathEvent>, mut log: ResMut<DeathLog>| {
                log.0.push(ev.target);
            });
        app
    }

    #[test]
    fn apply_damage_subtracts_health() {
        let mut app = damage_test_app();
        let target = app.world_mut().spawn(Health::new(100.0)).id();
        app.world_mut().write_message(DamageEvent {
            target,
            source: None,
            amount: 30.0,
            kind: DamageKind::Physical,
        });
        app.update();

        assert_eq!(app.world().get::<Health>(target).unwrap().current, 70.0);
        let log = app.world().resource::<AppliedLog>();
        assert_eq!(log.0.len(), 1);
        assert_eq!(log.0[0].final_hp, 70.0);
        assert!(!log.0[0].is_kill);
        assert!(app.world().resource::<DeathLog>().0.is_empty());
    }

    #[test]
    fn apply_damage_health_guard_reduces_and_clamps() {
        let mut app = damage_test_app();
        // Guard 0.5 → 30 dmg devient 15.
        let guarded = app
            .world_mut()
            .spawn((Health::new(100.0), HealthGuard { reduction: 0.5 }))
            .id();
        // Guard absurde 1.5 → clamp 1.0 → 0 dégât (invulnérable, pas de heal inversé).
        let over_guarded = app
            .world_mut()
            .spawn((Health::new(100.0), HealthGuard { reduction: 1.5 }))
            .id();
        for target in [guarded, over_guarded] {
            app.world_mut().write_message(DamageEvent {
                target,
                source: None,
                amount: 30.0,
                kind: DamageKind::Fire,
            });
        }
        app.update();

        assert_eq!(app.world().get::<Health>(guarded).unwrap().current, 85.0);
        assert_eq!(
            app.world().get::<Health>(over_guarded).unwrap().current,
            100.0,
            "reduction clampée à 1.0 → aucun dégât"
        );
    }

    #[test]
    fn apply_damage_kill_triggers_death_event_once() {
        let mut app = damage_test_app();
        let target = app.world_mut().spawn(Health::new(20.0)).id();
        app.world_mut().write_message(DamageEvent {
            target,
            source: None,
            amount: 25.0,
            kind: DamageKind::Explosion,
        });
        app.update();

        assert_eq!(app.world().get::<Health>(target).unwrap().current, 0.0);
        let log = app.world().resource::<AppliedLog>();
        assert!(log.0[0].is_kill);
        assert_eq!(
            app.world().resource::<DeathLog>().0,
            vec![target],
            "exactement 1 DeathEvent"
        );
    }

    #[test]
    fn apply_damage_ignores_dead_target() {
        let mut app = damage_test_app();
        let target = app
            .world_mut()
            .spawn(Health {
                current: 0.0,
                max: 100.0,
            })
            .id();
        app.world_mut().write_message(DamageEvent {
            target,
            source: None,
            amount: 50.0,
            kind: DamageKind::Physical,
        });
        app.update();

        assert!(
            app.world().resource::<AppliedLog>().0.is_empty(),
            "cible morte → aucun DamageApplied, aucun double-kill"
        );
        assert!(app.world().resource::<DeathLog>().0.is_empty());
    }

    #[test]
    fn apply_damage_two_events_same_frame_cumulate_single_kill() {
        let mut app = damage_test_app();
        let target = app.world_mut().spawn(Health::new(100.0)).id();
        for _ in 0..2 {
            app.world_mut().write_message(DamageEvent {
                target,
                source: None,
                amount: 60.0,
                kind: DamageKind::Physical,
            });
        }
        app.update();

        assert_eq!(app.world().get::<Health>(target).unwrap().current, 0.0);
        let log = app.world().resource::<AppliedLog>();
        assert_eq!(log.0.len(), 2, "les 2 events de la frame sont appliqués");
        assert!(!log.0[0].is_kill, "1er hit : 100→40, pas un kill");
        assert!(log.0[1].is_kill, "2e hit : 40→0, kill");
        assert_eq!(
            app.world().resource::<DeathLog>().0.len(),
            1,
            "un seul DeathEvent malgré 2 hits"
        );
    }
}

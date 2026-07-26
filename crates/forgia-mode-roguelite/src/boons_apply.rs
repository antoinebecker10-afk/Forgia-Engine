//! boons_apply.rs — Story-558 Phase 4 (2026-05-29).
//!
//! Lit `ActiveBoons` (forgia-rpg-data) + `BoonsCatalogue` et mute la Resource
//! globale `PlayerCombatMods` (forgia-combat) consommée par forgia-fps fire
//! system. Heal-on-kill géré séparément via DeathEvent observer.
//!
//! Scope Phase 4 (3 effets sur 7) :
//! - `DamageMul { factor }` → `combat_mods.damage_mul *= factor` (cumul multiplicatif)
//! - `FireRateMul { factor }` → `combat_mods.fire_rate_mul *= factor`
//! - `HealOnKill { hp }` → cumul stack, appliqué via observer
//!
//! Reportés Phase 4b (besoin hit_ctx + crit roll + hitscan chain mod) :
//! - `DamageReduction`, `ChainTargets`, `Knockback`, `FlatBonus` (crit/headshot)

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::combat_mods::PlayerCombatMods;
use forgia_combat::Health as EnemyHealth;
use forgia_damage::{DamageChannel, DeathEvent, DefenseLayer, Health, HealthGuard};
use forgia_player::Player;
use forgia_rpg_data::boons::{ActiveBoons, BoonEffectKind, BoonsCatalogue};

/// Cumul des effets HealOnKill actifs (somme des `hp` per kill).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct HealOnKillCumul {
    pub hp_per_kill: f32,
}

/// Recompute `PlayerCombatMods` + `HealOnKillCumul` quand `ActiveBoons` change.
/// Reset à neutre puis cumul multiplicatif (damage/fire_rate) et additif (heal).
pub fn sys_recompute_boon_mods(
    active: Res<ActiveBoons>,
    catalogue: Res<BoonsCatalogue>,
    // Story-591 — bonus permanents (méta) composés par-dessus les boons per-run.
    perm: Res<crate::meta_shop::PermanentPlayerMods>,
    // P3 — bonus de maîtrise d'arme (niveau), composé comme les mods méta.
    mastery: Res<crate::meta_shop::WeaponMasteryMods>,
    // Story-653 — La Trempe (progression in-run de l'arme), composée comme perm/mastery.
    trempe: Res<crate::trempe::WeaponTrempeState>,
    mut mods: ResMut<PlayerCombatMods>,
    mut heal: ResMut<HealOnKillCumul>,
) {
    // Fix 2026-06-28 — PLUS de garde `is_changed` : la détection de changement sur
    // ActiveBoons ratait les picks en runtime (boons inertes — crit/damage jamais
    // appliqués malgré boon actif ; diag : forgia2_run.log, recompute figé à boot
    // `crit=0% (active 0)` alors que le capteur montrait 2 boons actifs). Recompute
    // INCONDITIONNEL (coût négligeable : ≤18 boons + compose perm/mastery chaque
    // frame Roguelite) → PlayerCombatMods reflète TOUJOURS l'état réel. Log ci-dessous
    // uniquement quand le résultat change (pas de spam every-frame).
    let mut new_mods = PlayerCombatMods::default();
    let mut new_heal = 0.0_f32;
    for id in &active.active {
        let Some(def) = catalogue.find(id) else {
            continue;
        };
        match &def.effect {
            BoonEffectKind::DamageMul { factor } => new_mods.damage_mul *= factor,
            BoonEffectKind::FireRateMul { factor } => new_mods.fire_rate_mul *= factor,
            BoonEffectKind::HealOnKill { hp } => new_heal += hp,
            // Story-558 Phase 4b (2026-05-29) — cumul des 4 effets restants.
            BoonEffectKind::DamageReduction { factor } => {
                // Cumul additif clampé à 0.85 (jamais 100% invincible — anti-cheese).
                new_mods.damage_reduction = (new_mods.damage_reduction + factor).min(0.85);
            }
            BoonEffectKind::Knockback { strength } => {
                // Cumul additif (somme des forces de soufflage).
                new_mods.knockback_strength += strength;
            }
            BoonEffectKind::ChainTargets { count } => {
                new_mods.chain_extra_targets = new_mods.chain_extra_targets.saturating_add(*count);
            }
            BoonEffectKind::FlatBonus { stat, amount } => match stat.as_str() {
                "crit_chance" => {
                    new_mods.crit_chance = (new_mods.crit_chance + amount).min(1.0);
                }
                "headshot_mul" => {
                    new_mods.headshot_bonus_mul += amount;
                }
                _ => {
                    // Stat inconnue — log au debug, pas warn (TOML peut évoluer).
                    debug!("[boons] flat_bonus stat unknown: {stat}");
                }
            },
        }
    }
    // Story-591 — compose les bonus PERMANENTS (méta) par-dessus les boons,
    // AVANT l'overwrite (sinon sys_recompute les écraserait à chaque boon).
    new_mods.damage_mul *= perm.damage_mul;
    // P3 — maîtrise d'arme (niveau) : multiplicatif comme les autres bonus de dégâts.
    new_mods.damage_mul *= mastery.damage_mul;
    // Story-653 — La Trempe (in-run) : multiplicatif, même couche que perm/mastery.
    new_mods.damage_mul *= trempe.damage_mul;
    new_mods.damage_reduction = (new_mods.damage_reduction + perm.damage_reduction).min(0.85);
    // Log seulement au changement (recompute tourne chaque frame désormais).
    let changed = *mods != new_mods || (heal.hp_per_kill - new_heal).abs() > f32::EPSILON;
    *mods = new_mods;
    heal.hp_per_kill = new_heal;
    if changed {
        info!(
            "[boons] mods recomputed — damage×{:.2} fire_rate×{:.2} reduction={:.0}% crit={:.0}% head+{:.2} knockback={:.1} chain+{} heal={:.1}/kill (active {})",
            mods.damage_mul,
            mods.fire_rate_mul,
            mods.damage_reduction * 100.0,
            mods.crit_chance * 100.0,
            mods.headshot_bonus_mul,
            mods.knockback_strength,
            mods.chain_extra_targets,
            heal.hp_per_kill,
            active.active.len()
        );
    }
}

/// Story-558 Phase 4b — synchronise HealthGuard sur Player avec
/// `PlayerCombatMods.damage_reduction`. Insert/update component. forgia-damage
/// le lit dans apply_damage pour réduire les dégâts incoming.
pub fn sys_sync_player_health_guard(
    mut commands: Commands,
    mods: Res<PlayerCombatMods>,
    q_player: Query<(Entity, Option<&HealthGuard>), With<Player>>,
) {
    if !mods.is_changed() && !q_player.iter().any(|(_, g)| g.is_none()) {
        return;
    }
    for (e, guard) in &q_player {
        let target = HealthGuard {
            reduction: mods.damage_reduction,
        };
        let needs_update = match guard {
            Some(g) => (g.reduction - target.reduction).abs() > f32::EPSILON,
            None => true,
        };
        if needs_update {
            commands.entity(e).insert(target);
        }
    }
}

/// OnExit Roguelite — retire HealthGuard du Player (sinon Arena/RPG hérite).
pub fn sys_remove_player_health_guard(
    mut commands: Commands,
    q_player: Query<Entity, (With<Player>, With<HealthGuard>)>,
) {
    for e in &q_player {
        commands.entity(e).remove::<HealthGuard>();
    }
}

/// Story-558 Phase 4b — knockback on hit : translate Transform du target
/// par `dir * strength * KNOCKBACK_DT_SCALE`. Approche pragmatique pour
/// enemy KinematicPositionBased (Rapier impulse n'agit pas dessus).
///
/// MessageReader<CombatHitEvent> car CombatHitEvent est Message (forgia-combat).
pub fn sys_apply_knockback_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    mods: Res<PlayerCombatMods>,
    mut q_transform: Query<&mut Transform>,
) {
    if mods.knockback_strength <= 0.0 {
        return;
    }
    // Translation immédiate ~10cm par unité de strength (Ouragan 25 ≈ 2.5m).
    const KNOCKBACK_SCALE: f32 = 0.10;
    for ev in events.read() {
        let Some(attacker) = ev.attacker else {
            continue;
        };
        let Ok([mut target_t, atk_t]) = q_transform.get_many_mut([ev.target, attacker]) else {
            continue;
        };
        let dir = (target_t.translation - atk_t.translation).normalize_or_zero();
        if dir == Vec3::ZERO {
            continue;
        }
        target_t.translation += dir * mods.knockback_strength * KNOCKBACK_SCALE;
    }
}

/// Story-558 Phase 4b — chain hitscan. Sur CombatHitEvent (1er hit), si
/// `chain_extra_targets > 0`, saute aux N enemies les plus proches dans rayon
/// CHAIN_RANGE et applique un dégât réduit (CHAIN_DAMAGE_FACTOR du damage
/// original) par mutation directe de `forgia_combat::Health`, routé Bouclier→
/// Armure→Vie via `DefenseLayer::absorb` (canal Physical) — même pattern que
/// l'arc Shock d'`elements.rs`.
///
/// Fix audit 2026-07-19 : l'ancienne version émettait `DamageEvent`, pipeline
/// joueur-only (`apply_damage` ne query que `forgia_damage::Health`, absente
/// des ennemis) → 0 dégât silencieux. Pas de `CombatHitEvent` ré-émis non plus :
/// un chain hit qui en émettrait re-déclencherait ce système (cascade).
/// La mort (HP≤0) reste gérée par `despawn_dead_cubes` → DeathEvent → loot/heal.
///
/// Pas de raycast Rapier ici (CombatHitEvent ne porte pas le ctx) — proxy via
/// distance Transform (cheap, no LOS check). Bible kid-friendly : « Ça saute
/// partout ! » accepte raccourci visuel.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_chain_targets(
    mut events: MessageReader<CombatHitEvent>,
    mods: Res<PlayerCombatMods>,
    q_targets: Query<(Entity, &GlobalTransform), With<crate::enemies::EnemyArchetype>>,
    mut q_enemy_hp: Query<&mut EnemyHealth, With<crate::enemies::EnemyArchetype>>,
    mut q_enemy_def: Query<&mut DefenseLayer, With<crate::enemies::EnemyArchetype>>,
    // Lot C perf tir (audit 2026-07-20) : scratch réutilisé, zéro alloc par hit.
    mut candidates: Local<Vec<(f32, Entity)>>,
) {
    let n = mods.chain_extra_targets;
    if n == 0 {
        return;
    }
    const CHAIN_RANGE: f32 = 8.0;
    const CHAIN_DAMAGE_FACTOR: f32 = 0.6;
    for ev in events.read() {
        let origin = ev.hit_world_pos;
        // Collect candidates (distance, entity) sauf target original
        candidates.clear();
        candidates.extend(q_targets.iter().filter_map(|(e, gt)| {
            if e == ev.target {
                return None;
            }
            let d = (gt.translation() - origin).length();
            if d <= CHAIN_RANGE {
                Some((d, e))
            } else {
                None
            }
        }));
        candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let chain_dmg = ev.damage * CHAIN_DAMAGE_FACTOR;
        let mut applied = 0u32;
        for (_, target) in candidates.iter().take(n as usize) {
            // Bouclier→Armure→Vie, comme le hit de base (forgia-fps) et l'arc Shock.
            let leak = if let Ok(mut dl) = q_enemy_def.get_mut(*target) {
                dl.note_hit();
                dl.absorb(chain_dmg, DamageChannel::Physical)
            } else {
                chain_dmg
            };
            if let Ok(mut hp) = q_enemy_hp.get_mut(*target) {
                hp.current = (hp.current - leak).max(0.0);
                applied += 1;
            }
        }
        if applied > 0 {
            // Lot A perf tir : per-hit → debug! (stdout synchrone par tir).
            debug!("[boons] chain hit ×{applied} targets (~{chain_dmg:.1} dmg each)");
        }
    }
}

/// OnExit Roguelite — reset Mods + heal cumul à neutre.
/// Évite que les boons d'une run Roguelite polluent Arena/RPG.
pub fn sys_reset_boon_mods(mut mods: ResMut<PlayerCombatMods>, mut heal: ResMut<HealOnKillCumul>) {
    mods.reset();
    heal.hp_per_kill = 0.0;
}

/// Observer DeathEvent — si target = enemy Roguelite (a `EnemyArchetype`),
/// soigne le Player de `HealOnKillCumul.hp_per_kill`. Capped à `Health.max`.
pub fn obs_heal_on_kill(
    event: On<DeathEvent>,
    enemies_q: Query<&crate::enemies::EnemyArchetype>,
    mut q_player_hp: Query<&mut Health, With<Player>>,
    heal: Res<HealOnKillCumul>,
) {
    if heal.hp_per_kill <= 0.0 {
        return;
    }
    if enemies_q.get(event.target).is_err() {
        return;
    }
    let Ok(mut hp) = q_player_hp.single_mut() else {
        return;
    };
    let before = hp.current;
    hp.current = (hp.current + heal.hp_per_kill).min(hp.max);
    let healed = hp.current - before;
    if healed > 0.0 {
        // Lot A perf tir : per-kill → debug!.
        debug!(
            "[boons] heal_on_kill +{:.1} HP ({:.1} → {:.1})",
            healed, before, hp.current
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_rpg_data::boons::{BoonDef, BoonId, BoonRarity};

    fn def(id: &str, effect: BoonEffectKind) -> BoonDef {
        BoonDef {
            id: BoonId(id.into()),
            name: id.into(),
            voiceline_preview: String::new(),
            effect,
            tags: vec![],
            rarity: BoonRarity::Common,
            weapon_filter: None,
            souls_cost: None,
        }
    }

    #[test]
    fn damage_mul_stacks_multiplicatively() {
        let mut cat = BoonsCatalogue::default();
        cat.entries
            .push(def("a", BoonEffectKind::DamageMul { factor: 1.15 }));
        cat.entries
            .push(def("b", BoonEffectKind::DamageMul { factor: 1.75 }));
        let mut active = ActiveBoons::default();
        active.active.push(BoonId("a".into()));
        active.active.push(BoonId("b".into()));
        // simulate inline
        let mut mods = PlayerCombatMods::default();
        for id in &active.active {
            let d = cat.find(id).unwrap();
            if let BoonEffectKind::DamageMul { factor } = d.effect {
                mods.damage_mul *= factor;
            }
        }
        assert!((mods.damage_mul - 1.15 * 1.75).abs() < 1e-5);
    }

    #[test]
    fn heal_on_kill_cumul_sums() {
        let mut cat = BoonsCatalogue::default();
        cat.entries
            .push(def("a", BoonEffectKind::HealOnKill { hp: 5.0 }));
        cat.entries
            .push(def("b", BoonEffectKind::HealOnKill { hp: 3.0 }));
        let mut active = ActiveBoons::default();
        active.active.push(BoonId("a".into()));
        active.active.push(BoonId("b".into()));
        let mut heal = 0.0_f32;
        for id in &active.active {
            let d = cat.find(id).unwrap();
            if let BoonEffectKind::HealOnKill { hp } = d.effect {
                heal += hp;
            }
        }
        assert_eq!(heal, 8.0);
    }

    #[test]
    fn no_boons_means_neutral_mods() {
        let cat = BoonsCatalogue::default();
        let active = ActiveBoons::default();
        let mut mods = PlayerCombatMods::default();
        for id in &active.active {
            if let Some(d) = cat.find(id) {
                if let BoonEffectKind::DamageMul { factor } = d.effect {
                    mods.damage_mul *= factor;
                }
            }
        }
        assert_eq!(mods.damage_mul, 1.0);
        assert_eq!(mods.fire_rate_mul, 1.0);
    }
}

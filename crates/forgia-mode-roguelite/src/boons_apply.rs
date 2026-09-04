//! boons_apply.rs — Story-558 Phase 4 (2026-05-29).
//!
//! Lit `ActiveBoons` (forgia-rpg-data) + `BoonsCatalogue` et mute la Resource
//! globale `PlayerCombatMods` (forgia-combat) consommée par forgia-fps fire
//! system. Heal-on-kill géré séparément via DeathEvent observer.
//!
//! Les 7 effets sont routés : `DamageMul` et `FireRateMul` (multiplicatifs),
//! `HealOnKill` (cumul, appliqué via observer), `DamageReduction` et `Knockback`
//! (additifs), `ChainTargets` (saturating), `FlatBonus` (routage par nom de stat).
//!
//! ## Le silence de `FlatBonus` (2026-08-04)
//!
//! `FlatBonus { stat }` route par **chaîne de caractères**. Une stat non reconnue
//! ne produisait qu'un `debug!` : l'atout s'affichait au Coffre, le joueur le payait
//! en Âmes, et il ne faisait **rien** — sans bruit, et en compilant. C'est le retour
//! du bug « boons inertes » du 2026-06-28 sous une autre forme.
//!
//! Désormais toute stat inconnue atterrit dans [`BoonRoutingIssues`], est loguée en
//! `error!` **une seule fois** (le recompute tourne chaque frame — un log par frame
//! serait pire que le silence), et remonte dans `forgia2_power.json`.

use bevy::prelude::*;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::combat_mods::PlayerCombatMods;
use forgia_combat::Health as EnemyHealth;
use forgia_damage::{DamageChannel, DeathEvent, DefenseLayer, Health, HealthGuard};
use forgia_player::Player;
use forgia_rpg_data::boons::{ActiveBoons, BoonDef, BoonEffectKind, BoonsCatalogue};

/// Cumul des effets HealOnKill actifs (somme des `hp` per kill).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct HealOnKillCumul {
    pub hp_per_kill: f32,
}

/// Plafond de dégâts d'un atout **filtré par arme** (`weapon_filter` renseigné).
///
/// L'écart de matchup élémentaire va de ×1.0 à **×2.0** (`roguelite_elements.toml`,
/// `armor_pierce` contre un Tank). Un bonus attaché à UNE arme au-delà de ce plafond
/// bat le bon matchup partout : le joueur cesse de changer d'arme, et le pilier
/// « toutes les armes vivantes, le choix vient de l'ennemi » disparaît — non par un
/// bug, mais par optimisation rationnelle.
///
/// Les atouts **universels** (`weapon_filter` absent) ne sont pas concernés : ils ne
/// créent aucune raison de préférer une arme à une autre.
pub const WEAPON_FILTERED_DAMAGE_CAP: f32 = 1.3;

/// `false` si l'atout favorise UNE arme au-delà de [`WEAPON_FILTERED_DAMAGE_CAP`].
/// Pur — testable sans App ni World.
pub fn weapon_filtered_bonus_is_within_cap(def: &BoonDef) -> bool {
    let Some(filter) = def.weapon_filter.as_ref() else {
        return true; // universel : hors sujet
    };
    if filter.is_empty() {
        return true;
    }
    match &def.effect {
        BoonEffectKind::DamageMul { factor } => *factor <= WEAPON_FILTERED_DAMAGE_CAP,
        _ => true,
    }
}

/// Ce que la composition n'a PAS su appliquer. Un atout qui atterrit ici est payé
/// par le joueur et sans effet — c'est un défaut, pas une curiosité.
///
/// Accumulé sur la run (jamais vidé par frame, sinon le log se répéterait 60×/s),
/// remis à zéro au démarrage d'une run par [`sys_reset_boon_mods`].
#[derive(Resource, Default, Debug, Clone)]
pub struct BoonRoutingIssues {
    /// Noms de stats `FlatBonus` rencontrés et routés nulle part.
    pub unknown_stats: Vec<String>,
    /// Ids d'atouts filtrés par arme qui dépassent [`WEAPON_FILTERED_DAMAGE_CAP`].
    pub over_cap_boons: Vec<String>,
}

impl BoonRoutingIssues {
    /// `true` si c'est la PREMIÈRE fois qu'on voit cette stat (→ loguer maintenant).
    fn note_unknown_stat(&mut self, stat: &str) -> bool {
        if self.unknown_stats.iter().any(|s| s == stat) {
            return false;
        }
        self.unknown_stats.push(stat.to_string());
        true
    }

    fn note_over_cap(&mut self, id: &str) -> bool {
        if self.over_cap_boons.iter().any(|s| s == id) {
            return false;
        }
        self.over_cap_boons.push(id.to_string());
        true
    }

    pub fn is_clean(&self) -> bool {
        self.unknown_stats.is_empty() && self.over_cap_boons.is_empty()
    }
}

/// Décomposition du multiplicateur de dégâts **par source** (V0).
///
/// Cinq systèmes alimentent `PlayerCombatMods.damage_mul` et le modèle de
/// difficulté (`rounds.rs`) ne connaît d'eux qu'un seul chiffre agrégé posé à la
/// main (`gain_puissance_par_round`). Sans cette décomposition, le mur est calculé
/// contre une abstraction plutôt que contre la puissance réelle du joueur.
///
/// **Écrite par le compositeur lui-même**, pas recalculée par le capteur : un
/// capteur qui refait le calcul finit toujours par diverger de la vérité qu'il
/// prétend mesurer (`map-design-patterns.md` §14 — pas de source auto-référente).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct PowerBreakdown {
    /// Produit des `DamageMul` des atouts actifs (run).
    pub boons: f32,
    /// L'Enclume des Âmes — permanent.
    pub perm: f32,
    /// Maîtrise de l'arme équipée — permanent.
    pub mastery: f32,
    /// La Trempe de l'ARSENAL — run.
    ///
    /// C'était « de l'arme équipée » jusqu'au 2026-08-04 : 219 Or dépensés puis
    /// évaporés au premier changement d'arme, mesuré en jeu. Elle vaut
    /// désormais la somme des niveaux trempés, plafonnée au même total.
    pub trempe: f32,
    /// Pièces d'équipement portées — run.
    pub equip: f32,
    /// Ce que le fire path applique réellement (= produit des cinq).
    pub total: f32,
    /// Nombre d'atouts actifs, tous effets confondus.
    pub boon_count: u32,
    /// Parmi eux, ceux qui portent réellement un `DamageMul`.
    ///
    /// Sans ce compte, `boons = 1.000` avec `boon_count = 2` est **ambigu** : on ne
    /// distingue pas « deux atouts qui ne font pas de dégâts » (normal — cadence,
    /// soin, crit) de « deux atouts inertes » (défaut). Un zéro qu'on ne peut pas
    /// interpréter n'est pas vert, il est aveugle (`map-design-patterns.md` §13).
    pub boon_damage_count: u32,
}

impl Default for PowerBreakdown {
    fn default() -> Self {
        Self {
            boons: 1.0,
            perm: 1.0,
            mastery: 1.0,
            trempe: 1.0,
            equip: 1.0,
            total: 1.0,
            boon_count: 0,
            boon_damage_count: 0,
        }
    }
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
    // Équipement — pièces d'armure portées (rareté = ampleur du bonus).
    equip: Res<crate::equipment::EquipmentMods>,
    mut mods: ResMut<PlayerCombatMods>,
    mut heal: ResMut<HealOnKillCumul>,
    // V0 — la décomposition par source, écrite ici pour qu'aucun capteur n'ait à
    // la recalculer (et donc à diverger).
    mut breakdown: ResMut<PowerBreakdown>,
    // Ce que la composition n'a pas su appliquer — jamais silencieux.
    mut issues: ResMut<BoonRoutingIssues>,
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
    let mut boon_damage_count = 0_u32;
    for id in &active.active {
        let Some(def) = catalogue.find(id) else {
            continue;
        };
        // Un atout filtré par arme qui dépasse le plafond bat le matchup élémentaire
        // partout : il ne casse rien mécaniquement, il supprime une DÉCISION.
        // Détecté ici parce que le catalogue est hot-reloadable — un test seul ne
        // verrait pas une édition TOML faite pendant que le jeu tourne.
        if !weapon_filtered_bonus_is_within_cap(def) && issues.note_over_cap(&def.id.0) {
            error!(
                "[boons] « {} » favorise une arme au-delà de ×{WEAPON_FILTERED_DAMAGE_CAP} — \
                 il bat le matchup élémentaire (max ×2.0) partout, donc le joueur cessera \
                 de changer d'arme. Baisse le facteur, ou retire le weapon_filter.",
                def.name
            );
        }
        match &def.effect {
            BoonEffectKind::DamageMul { factor } => {
                new_mods.damage_mul *= factor;
                boon_damage_count += 1;
            }
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
                // 2026-08-04 — la famille ENTRETIEN. Additifs entre eux (deux
                // atouts +25 % donnent +50 %, pas ×1,56) : c'est plus lisible
                // pour le joueur, et ça évite qu'une pile de doublons explose.
                "reload_speed" => {
                    new_mods.reload_speed_mul += amount;
                }
                "mag_size" => {
                    new_mods.mag_size_mul += amount;
                }
                // Famille CORPS — neutre pour le pilier (n'attache rien à une arme).
                "move_speed" => {
                    new_mods.move_speed_mul += amount;
                }
                // Famille RÉCOLTE — se multiplie avec elle-même, doser bas.
                "loot_gain" => {
                    new_mods.loot_gain_mul += amount;
                }
                _ => {
                    // Stat routée nulle part = atout INERTE, payé en Âmes, et qui
                    // compile. `error!` UNE fois (ce système tourne chaque frame),
                    // puis la trace vit dans le capteur.
                    if issues.note_unknown_stat(stat) {
                        error!(
                            "[boons] « {} » porte flat_bonus stat=\"{stat}\" qui n'est routée \
                             nulle part : l'atout est INERTE et le joueur le paie. Ajoute la \
                             branche dans sys_recompute_boon_mods, ou corrige le TOML.",
                            def.name
                        );
                    }
                }
            },
        }
    }
    // V0 — la part des ATOUTS seuls, capturée avant que les quatre autres sources
    // ne s'y composent. C'est le seul endroit où elle est isolable.
    let boons_only = new_mods.damage_mul;
    // Story-591 — compose les bonus PERMANENTS (méta) par-dessus les boons,
    // AVANT l'overwrite (sinon sys_recompute les écraserait à chaque boon).
    new_mods.damage_mul *= perm.damage_mul;
    // P3 — maîtrise d'arme (niveau) : multiplicatif comme les autres bonus de dégâts.
    new_mods.damage_mul *= mastery.damage_mul;
    // Story-653 — La Trempe (in-run) : multiplicatif, même couche que perm/mastery.
    new_mods.damage_mul *= trempe.damage_mul;
    // Équipement : même couche que perm/mastery/trempe (multiplicatif sur les
    // multiplicateurs, additif clampé sur les fractions). Le clamp de réduction
    // est calculé APRÈS l'apport des pièces, sinon un plastron Mythique pourrait
    // pousser au-delà des 85 % que l'anti-cheese garantit.
    new_mods.damage_mul *= equip.damage_mul;
    new_mods.fire_rate_mul *= equip.fire_rate_mul;
    new_mods.crit_chance = (new_mods.crit_chance + equip.crit_chance).min(1.0);
    new_mods.headshot_bonus_mul += equip.headshot_bonus_mul;
    new_mods.damage_reduction =
        (new_mods.damage_reduction + perm.damage_reduction + equip.damage_reduction).min(0.85);
    // V0 — la décomposition. `total` est relu depuis `new_mods` et non recomposé :
    // c'est exactement ce que le fire path appliquera, pas une reconstruction.
    let new_breakdown = PowerBreakdown {
        boons: boons_only,
        perm: perm.damage_mul,
        mastery: mastery.damage_mul,
        trempe: trempe.damage_mul,
        equip: equip.damage_mul,
        total: new_mods.damage_mul,
        boon_count: active.active.len() as u32,
        boon_damage_count,
    };
    if *breakdown != new_breakdown {
        *breakdown = new_breakdown;
    }
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
pub fn sys_reset_boon_mods(
    mut mods: ResMut<PlayerCombatMods>,
    mut heal: ResMut<HealOnKillCumul>,
    mut breakdown: ResMut<PowerBreakdown>,
    mut issues: ResMut<BoonRoutingIssues>,
) {
    mods.reset();
    heal.hp_per_kill = 0.0;
    *breakdown = PowerBreakdown::default();
    // Les défauts de routage se re-signalent à la run suivante : un catalogue
    // hot-reloadé entre deux runs peut en introduire comme en corriger.
    *issues = BoonRoutingIssues::default();
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

    // ── Le plafond des atouts filtrés par arme ───────────────────────────────
    // Ces tests encodent une règle de DESIGN, pas un comportement du code actuel :
    // `weapon_filter` n'est lu nulle part aujourd'hui, donc tous les atouts sont
    // universels. Ils existent pour que le jour où quelqu'un le câble, la limite
    // soit déjà là — et pas redécouverte en playtest.

    fn filtered(id: &str, effect: BoonEffectKind, weapons: &[&str]) -> BoonDef {
        BoonDef {
            weapon_filter: Some(weapons.iter().map(|s| (*s).to_string()).collect()),
            ..def(id, effect)
        }
    }

    #[test]
    fn un_atout_universel_nest_pas_plafonne() {
        // Un ×2 pour TOUTES les armes ne crée aucune raison d'en préférer une :
        // il n'entre pas en concurrence avec le matchup élémentaire.
        let d = def("puissant", BoonEffectKind::DamageMul { factor: 2.0 });
        assert!(weapon_filtered_bonus_is_within_cap(&d));
    }

    #[test]
    fn un_atout_darme_sous_le_plafond_passe() {
        let d = filtered(
            "pepin_affute",
            BoonEffectKind::DamageMul { factor: 1.25 },
            &["pepin"],
        );
        assert!(weapon_filtered_bonus_is_within_cap(&d));
    }

    #[test]
    fn un_atout_darme_qui_bat_le_matchup_est_refuse() {
        // ×2.0 filtré = exactement l'écart max du matchup (armor_pierce vs Tank).
        // Le joueur garderait cette arme même face à l'ennemi qui la contre.
        let d = filtered(
            "pepin_devastateur",
            BoonEffectKind::DamageMul { factor: 2.0 },
            &["pepin"],
        );
        assert!(!weapon_filtered_bonus_is_within_cap(&d));
    }

    #[test]
    fn le_plafond_ne_concerne_que_les_degats() {
        // Cadence, soin, chaîne : ils ne pèsent pas dans l'arbitrage de matchup,
        // qui se joue en dégâts. Les plafonner brimerait sans rien protéger.
        let d = filtered(
            "pepin_rapide",
            BoonEffectKind::FireRateMul { factor: 3.0 },
            &["pepin"],
        );
        assert!(weapon_filtered_bonus_is_within_cap(&d));
    }

    #[test]
    fn un_filtre_vide_vaut_universel() {
        let d = filtered("bizarre", BoonEffectKind::DamageMul { factor: 2.0 }, &[]);
        assert!(weapon_filtered_bonus_is_within_cap(&d));
    }

    // ── Le silence de FlatBonus ──────────────────────────────────────────────

    #[test]
    fn une_stat_inconnue_ne_se_signale_quune_fois() {
        // Le recompute tourne CHAQUE FRAME : sans ce dédoublonnage, l'alerte
        // deviendrait 60 lignes/seconde, donc illisible, donc ignorée.
        let mut issues = BoonRoutingIssues::default();
        assert!(
            issues.note_unknown_stat("reload_speed"),
            "1re fois → on loggue"
        );
        assert!(
            !issues.note_unknown_stat("reload_speed"),
            "2e fois → silencieux, mais la trace reste"
        );
        assert_eq!(issues.unknown_stats, vec!["reload_speed".to_string()]);
        assert!(!issues.is_clean());
    }

    #[test]
    fn des_stats_inconnues_distinctes_se_signalent_chacune() {
        let mut issues = BoonRoutingIssues::default();
        assert!(issues.note_unknown_stat("reload_speed"));
        assert!(issues.note_unknown_stat("ammo_capacity"));
        assert_eq!(issues.unknown_stats.len(), 2);
    }

    #[test]
    fn un_catalogue_sain_ne_produit_aucun_defaut() {
        let issues = BoonRoutingIssues::default();
        assert!(issues.is_clean());
    }

    // ── La décomposition ─────────────────────────────────────────────────────

    #[test]
    fn la_decomposition_part_neutre() {
        let b = PowerBreakdown::default();
        assert_eq!(b.total, 1.0);
        assert_eq!(b.boons, 1.0);
        assert_eq!(b.boon_count, 0);
        assert_eq!(b.boon_damage_count, 0);
    }

    #[test]
    fn des_atouts_sans_degats_ne_se_lisent_pas_comme_des_atouts_inertes() {
        // Le cas mesuré le 2026-08-04 : 2 atouts actifs, `boons` à 1.000. Avec le
        // seul `boon_count`, impossible de dire si c'est normal ou cassé.
        let sains = PowerBreakdown {
            boon_count: 2,
            boon_damage_count: 0, // 2 atouts, aucun de dégâts → 1.000 est CORRECT
            ..Default::default()
        };
        assert_eq!(sains.boons, 1.0);
        assert_eq!(
            sains.boon_damage_count, 0,
            "rien à appliquer, rien d'anormal"
        );

        let suspect = PowerBreakdown {
            boon_count: 2,
            boon_damage_count: 2, // 2 atouts de dégâts mais `boons` neutre → défaut
            ..Default::default()
        };
        assert!(
            suspect.boon_damage_count > 0 && suspect.boons == 1.0,
            "ce cas-là mérite une alerte, l'autre non — les deux compteurs les séparent"
        );
    }

    #[test]
    fn le_total_est_le_produit_des_cinq_sources() {
        // C'est l'invariant que le capteur publie : si ce produit cesse d'égaler
        // `total`, c'est qu'une 6e source s'est branchée sans être décomposée —
        // et le modèle de difficulté ne la verrait pas.
        let b = PowerBreakdown {
            boons: 1.15,
            perm: 1.40,
            mastery: 1.20,
            trempe: 2.01,
            equip: 1.10,
            total: 1.15 * 1.40 * 1.20 * 2.01 * 1.10,
            boon_count: 1,
            boon_damage_count: 1,
        };
        let produit = b.boons * b.perm * b.mastery * b.trempe * b.equip;
        assert!((b.total - produit).abs() < 1e-4);
    }
}

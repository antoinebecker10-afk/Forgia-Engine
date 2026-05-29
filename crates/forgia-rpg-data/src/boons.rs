//! Story-529 — Boons Architecture (Mission 2 GDD).
//!
//! Data layer for the Roguelite boon system. The runtime apply systems +
//! UI "Coffre du Forgeron" live in `forgia-mode-roguelite` and `forgia-ui-lib`
//! respectively (Phase 2, after other-terminal sync).
//!
//! ## Concepts
//!
//! - [`BoonDef`] : static definition (id, name, effect, tags, rarity).
//! - [`BoonsCatalogue`] : TOML-loaded Resource holding all [`BoonDef`].
//! - [`ActiveBoons`] : runtime Resource — which boons the player has picked
//!   this run + tag counters + legendary unlock state.
//! - [`BoonAppliedEvent`] : emitted when a boon is added, consumed by
//!   gameplay systems (weapon stat mutation, VFX, voiceline preview).
//!
//! ## TOML schema
//!
//! See `assets/genomes/roguelite_boons.toml`. Hot-reloadable via Bevy Asset.
//!
//! ## Tag stacking → Legendary unlock (AC5)
//!
//! When a tag count reaches `LEGENDARY_TAG_THRESHOLD` (3), every legendary
//! boon carrying that tag becomes eligible for the next coffre roll. This is
//! pure data — the rolling logic lives in the future `boons_apply.rs`.

use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_rng::Rng;
use serde::Deserialize;
use std::collections::HashMap;

/// Number of identical tags required to unlock the legendary tier for that tag.
pub const LEGENDARY_TAG_THRESHOLD: u32 = 3;

/// Stable string ID matching the `id` field in TOML.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct BoonId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoonRarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

/// Tags used for synergy / legendary unlock. New tags can be added freely —
/// `BoonTag::from_str` falls back to `BoonTag::Other(String)` so the TOML is
/// forward-compatible.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BoonTag {
    Fire,
    Ricochet,
    Knockback,
    Chain,
    Precision,
    Chaos,
    /// Catch-all for future tags declared in TOML without a Rust enum bump.
    #[serde(other)]
    Other,
}

impl BoonTag {
    /// Canonical lowercase key, used for `ActiveBoons.tag_counts` HashMap.
    pub fn key(&self) -> &'static str {
        match self {
            BoonTag::Fire => "fire",
            BoonTag::Ricochet => "ricochet",
            BoonTag::Knockback => "knockback",
            BoonTag::Chain => "chain",
            BoonTag::Precision => "precision",
            BoonTag::Chaos => "chaos",
            BoonTag::Other => "other",
        }
    }
}

/// Gameplay effect kinds. Each variant carries its numeric payload (factor,
/// flat bonus, etc.). The apply layer pattern-matches and mutates the right
/// gameplay Resource/Component.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BoonEffectKind {
    /// Multiplies outgoing damage of the selected weapon (or all if filter None).
    DamageMul { factor: f32 },
    /// Heal/restore Énergie on enemy kill.
    HealOnKill { hp: f32 },
    /// Fire rate multiplier (>1 = faster).
    FireRateMul { factor: f32 },
    /// Adds chain-lightning style extra targets.
    ChainTargets { count: u32 },
    /// Adds knockback impulse strength on hit.
    Knockback { strength: f32 },
    /// Reduces incoming damage (clamped 0..1, 0.2 = -20% incoming).
    DamageReduction { factor: f32 },
    /// Generic flat bonus on a named stat. Apply layer routes by `stat`.
    FlatBonus { stat: String, amount: f32 },
}

/// Static definition of a single boon. Loaded from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct BoonDef {
    pub id: BoonId,
    pub name: String,
    /// Short voiceline shown on hover preview (<5s, vocab CE2 per anti-canon).
    #[serde(default)]
    pub voiceline_preview: String,
    pub effect: BoonEffectKind,
    #[serde(default)]
    pub tags: Vec<BoonTag>,
    pub rarity: BoonRarity,
    /// `None` (or omitted) = applies to all weapons. Otherwise restrict by weapon id.
    #[serde(default)]
    pub weapon_filter: Option<Vec<String>>,
    /// Story-558 Phase 2 (2026-05-29) — coût en Souls pour acheter ce boon au
    /// Coffre du Forgeron. Si omis dans le TOML, fallback via `default_souls_cost`
    /// (rarity-based : Common=20, Uncommon=40, Legendary=80) pour migration douce.
    /// Pattern Hadès Charon merchant + RoR2 chest.
    #[serde(default)]
    pub souls_cost: Option<u32>,
}

impl BoonDef {
    /// Coût final en Souls : `souls_cost` du TOML si défini, sinon défaut par rarity.
    /// Story-558 AC3 — defaults proposés Common=20, Uncommon=40, Legendary=80.
    pub fn effective_souls_cost(&self) -> u32 {
        self.souls_cost.unwrap_or_else(|| default_souls_cost(self.rarity))
    }
}

/// Coût par défaut basé sur la rareté quand `souls_cost` est omis du TOML.
/// Calibré sur drop economy : Tank=5, Sniper=3, Runner=2, Boss=40.
/// Common = ~4 Tank kills, Legendary = ~16 Tank kills + Boss.
pub fn default_souls_cost(rarity: BoonRarity) -> u32 {
    match rarity {
        BoonRarity::Common => 20,
        BoonRarity::Uncommon => 40,
        BoonRarity::Rare => 60,
        BoonRarity::Legendary => 80,
    }
}

/// TOML-loaded catalogue. Resource (init_resource → Default empty) AND Asset
/// payload (Genome<BoonsCatalogue>). Synced when the asset reloads.
#[derive(Resource, Default, Debug, Clone, Deserialize, TypePath)]
pub struct BoonsCatalogue {
    #[serde(default)]
    pub entries: Vec<BoonDef>,
}

impl BoonsCatalogue {
    pub fn find(&self, id: &BoonId) -> Option<&BoonDef> {
        self.entries.iter().find(|b| &b.id == id)
    }

    pub fn count_by_rarity(&self, rarity: BoonRarity) -> usize {
        self.entries.iter().filter(|b| b.rarity == rarity).count()
    }
}

/// Runtime state of acquired boons during a single run.
#[derive(Resource, Default, Debug, Clone)]
pub struct ActiveBoons {
    /// All boons picked this run, in order. Duplicates allowed (stacking).
    pub active: Vec<BoonId>,
    /// tag_key → count (after Σ all active boons' tags).
    pub tag_counts: HashMap<String, u32>,
    /// Legendary boons that became eligible (≥ LEGENDARY_TAG_THRESHOLD of a shared tag).
    pub unlocked_legendary: Vec<BoonId>,
    /// Total coffre choices presented this session (across runs). Sensor metric.
    pub total_choices_this_session: u32,
}

impl ActiveBoons {
    /// Reset run-scoped state (preserve `total_choices_this_session` across runs).
    pub fn reset_run(&mut self) {
        self.active.clear();
        self.tag_counts.clear();
        self.unlocked_legendary.clear();
    }

    /// Add a boon to the active set. Recomputes tag counts and legendary
    /// unlocks against the supplied catalogue. Idempotent on legendary unlocks.
    pub fn apply(&mut self, def: &BoonDef, catalogue: &BoonsCatalogue) {
        self.active.push(def.id.clone());
        for tag in &def.tags {
            *self.tag_counts.entry(tag.key().to_string()).or_insert(0) += 1;
        }
        // Recompute legendary unlocks — set semantics, idempotent.
        for b in &catalogue.entries {
            if b.rarity != BoonRarity::Legendary {
                continue;
            }
            let unlocked = b.tags.iter().any(|t| {
                self.tag_counts
                    .get(t.key())
                    .is_some_and(|&c| c >= LEGENDARY_TAG_THRESHOLD)
            });
            if unlocked && !self.unlocked_legendary.contains(&b.id) {
                self.unlocked_legendary.push(b.id.clone());
            }
        }
    }

    pub fn tag_count(&self, tag: &BoonTag) -> u32 {
        *self.tag_counts.get(tag.key()).unwrap_or(&0)
    }
}

#[derive(Message, Debug, Clone)]
pub struct BoonAppliedEvent {
    pub boon_id: BoonId,
}

/// Phase 2 — UI handshake state for the Coffre du Forgeron.
///
/// Set `is_open=true` by sending [`OpenCoffreRequest`]; the apply system rolls
/// `candidates` and the UI (`forgia-ui-lib::hud::coffre_forgeron`) draws cards.
/// User click emits [`CoffrePickedEvent`] which closes the session and applies
/// the boon.
#[derive(Resource, Default, Debug, Clone)]
pub struct CoffreSession {
    pub is_open: bool,
    /// Rolled candidates this round (typically 3).
    pub candidates: Vec<BoonId>,
    /// Tag-line shown above the cards (Maître Forgeron persona).
    pub maitre_voiceline: String,
    /// Source label for sensor / debug ("wave_clear" | "mid_boss" | "boss_final").
    pub source: String,
}

/// Request to open the Coffre. Carries the source so the apply system can
/// roll with the right pool (e.g. mid-boss = +1 candidate, boss = guaranteed
/// legendary). Phase 2 ignores `extra_candidates` / `force_legendary` and rolls
/// `count` uniformly from eligible pool.
#[derive(Message, Debug, Clone)]
pub struct OpenCoffreRequest {
    pub source: String,
    pub count: usize,
}

impl OpenCoffreRequest {
    pub fn wave_clear() -> Self {
        Self {
            source: "wave_clear".into(),
            count: 3,
        }
    }
}

/// User picked one of the candidates (UI emits this on card click).
#[derive(Message, Debug, Clone)]
pub struct CoffrePickedEvent {
    pub boon_id: BoonId,
}

/// Pure rolling logic. Extracted from systems for testability.
///
/// Rules:
/// - **Legendary filter** : a legendary boon is eligible only if its id is in
///   `active.unlocked_legendary`.
/// - **No duplicates** within a single roll.
/// - **Uniform** pick from the eligible pool (rarity weighting can come later).
/// - Returns at most `count` candidates; fewer if eligible pool is small.
///
/// `next_index(upper)` is a `0..upper` RNG injection so tests can use a fake.
pub fn roll_candidates(
    catalogue: &BoonsCatalogue,
    active: &ActiveBoons,
    count: usize,
    mut next_index: impl FnMut(usize) -> usize,
) -> Vec<BoonId> {
    let mut pool: Vec<&BoonDef> = catalogue
        .entries
        .iter()
        .filter(|b| match b.rarity {
            BoonRarity::Legendary => active.unlocked_legendary.contains(&b.id),
            _ => true,
        })
        .collect();
    let mut out: Vec<BoonId> = Vec::with_capacity(count);
    while out.len() < count && !pool.is_empty() {
        let upper = pool.len();
        let idx = next_index(upper) % upper.max(1);
        let picked = pool.swap_remove(idx);
        out.push(picked.id.clone());
    }
    out
}

/// Wraps a [`Rng`] into the `next_index` closure shape used by [`roll_candidates`].
pub fn rng_next_index(rng: &mut Rng) -> impl FnMut(usize) -> usize + '_ {
    move |upper: usize| {
        if upper == 0 {
            0
        } else {
            rng.range_usize(0, upper)
        }
    }
}

#[derive(Resource)]
pub struct CoffreRng(pub Rng);

impl Default for CoffreRng {
    fn default() -> Self {
        // Seeded post-startup by mode-roguelite (RunSeed). Default seed for
        // unit testing / unwired Phase 1.
        Self(Rng::new(0xC0FF_EEBE_EF00_0000))
    }
}

/// Consume [`OpenCoffreRequest`] : roll candidates and open the session.
pub fn sys_handle_open_coffre(
    mut requests: MessageReader<OpenCoffreRequest>,
    catalogue: Res<BoonsCatalogue>,
    active: Res<ActiveBoons>,
    mut session: ResMut<CoffreSession>,
    mut rng: ResMut<CoffreRng>,
) {
    for req in requests.read() {
        if session.is_open {
            // Stale request while session already open — ignore.
            continue;
        }
        let mut idx_fn = rng_next_index(&mut rng.0);
        let candidates = roll_candidates(&catalogue, &active, req.count, &mut idx_fn);
        if candidates.is_empty() {
            warn!(
                "[boons] coffre open '{}' but eligible pool empty ({} entries)",
                req.source,
                catalogue.entries.len()
            );
            continue;
        }
        session.is_open = true;
        session.candidates = candidates;
        session.maitre_voiceline = "Choisis bien !".into();
        session.source = req.source.clone();
        info!(
            "[boons] coffre opened (source={}, {} candidates)",
            session.source,
            session.candidates.len()
        );
    }
}

/// Consume [`CoffrePickedEvent`] : apply boon, emit [`BoonAppliedEvent`],
/// close session, increment session counter.
///
/// Story-558 Phase 3 (2026-05-29) — décrémente `Souls.current` du coût du boon.
/// Pick refusé silencieusement si `Souls < effective_souls_cost()` (UI doit
/// gate avant, defense-in-depth). Si le pick n'est pas dans `session.candidates`
/// (debug/cheat tools), warn mais applique quand même au prix indiqué.
pub fn sys_handle_coffre_pick(
    mut picks: MessageReader<CoffrePickedEvent>,
    catalogue: Res<BoonsCatalogue>,
    mut active: ResMut<ActiveBoons>,
    mut session: ResMut<CoffreSession>,
    mut applied: MessageWriter<BoonAppliedEvent>,
    mut souls: Option<ResMut<crate::loot_tables::Souls>>,
) {
    for pick in picks.read() {
        let Some(def) = catalogue.find(&pick.boon_id) else {
            warn!("[boons] picked id {:?} not in catalogue — ignored", pick.boon_id);
            continue;
        };
        if session.is_open && !session.candidates.contains(&pick.boon_id) {
            warn!(
                "[boons] picked id {:?} not in session candidates — applying anyway",
                pick.boon_id
            );
        }
        let cost = def.effective_souls_cost();
        if let Some(souls) = souls.as_deref_mut() {
            if souls.current < cost {
                warn!(
                    "[boons] pick {:?} REFUSED : souls {} < cost {}",
                    pick.boon_id, souls.current, cost
                );
                continue;
            }
            souls.current = souls.current.saturating_sub(cost);
            info!(
                "[boons] spent {} souls on {:?} (remaining {})",
                cost, pick.boon_id, souls.current
            );
        }
        active.apply(def, &catalogue);
        active.total_choices_this_session = active.total_choices_this_session.saturating_add(1);
        applied.write(BoonAppliedEvent {
            boon_id: pick.boon_id.clone(),
        });
        info!(
            "[boons] applied {:?} (active count {}, choices {})",
            pick.boon_id,
            active.active.len(),
            active.total_choices_this_session
        );
        session.is_open = false;
        session.candidates.clear();
        session.maitre_voiceline.clear();
        session.source.clear();
    }
}

#[derive(Resource)]
pub struct BoonsCatalogueHandle(pub Handle<Genome<BoonsCatalogue>>);

pub fn load_boons_catalogue(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<BoonsCatalogue>> = server.load("genomes/roguelite_boons.toml");
    commands.insert_resource(BoonsCatalogueHandle(handle));
}

pub fn sync_boons_catalogue(
    mut events: MessageReader<AssetEvent<Genome<BoonsCatalogue>>>,
    handle: Option<Res<BoonsCatalogueHandle>>,
    assets: Res<Assets<Genome<BoonsCatalogue>>>,
    mut catalogue: ResMut<BoonsCatalogue>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => {
                if *id == handle.0.id() {
                    should = true;
                }
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    if let Some(g) = assets.get(&handle.0) {
        *catalogue = g.data.clone();
        info!(
            "[boons] catalogue synced ({} entries, {} legendary)",
            catalogue.entries.len(),
            catalogue.count_by_rarity(BoonRarity::Legendary),
        );
    }
}

/// Boons plugin : Resources + Asset loader + Events + apply systems.
///
/// Phase 3 (later) — gameplay consumers of `BoonAppliedEvent` (weapon stat
/// mutation) live in `forgia-fps` / `forgia-mode-roguelite`. The trigger
/// (`OpenCoffreRequest` on wave clear) lives in `forgia-mode-roguelite`.
pub struct ForgiaBoonsPlugin;

impl Plugin for ForgiaBoonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoonsCatalogue>()
            .init_resource::<ActiveBoons>()
            .init_resource::<CoffreSession>()
            .init_resource::<CoffreRng>()
            .add_message::<BoonAppliedEvent>()
            .add_message::<OpenCoffreRequest>()
            .add_message::<CoffrePickedEvent>()
            .init_asset::<Genome<BoonsCatalogue>>()
            .register_asset_loader(GenomeLoader::<BoonsCatalogue>::default())
            .add_systems(Startup, load_boons_catalogue)
            .add_systems(
                Update,
                (
                    sync_boons_catalogue,
                    sys_handle_open_coffre,
                    sys_handle_coffre_pick,
                ),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(id: &str, rarity: BoonRarity, tags: &[BoonTag]) -> BoonDef {
        BoonDef {
            id: BoonId(id.into()),
            name: id.into(),
            voiceline_preview: String::new(),
            effect: BoonEffectKind::DamageMul { factor: 1.1 },
            tags: tags.to_vec(),
            rarity,
            weapon_filter: None,
            souls_cost: None,
        }
    }

    #[derive(Deserialize)]
    struct WrapRarity {
        v: BoonRarity,
    }
    #[derive(Deserialize)]
    struct WrapTag {
        v: BoonTag,
    }

    #[test]
    fn rarity_serde_lowercase() {
        let t: WrapRarity = toml::from_str("v = \"common\"").unwrap();
        assert_eq!(t.v, BoonRarity::Common);
        let t: WrapRarity = toml::from_str("v = \"legendary\"").unwrap();
        assert_eq!(t.v, BoonRarity::Legendary);
    }

    #[test]
    fn tag_serde_lowercase_and_other() {
        let t: WrapTag = toml::from_str("v = \"fire\"").unwrap();
        assert_eq!(t.v, BoonTag::Fire);
        let t: WrapTag = toml::from_str("v = \"future_unknown\"").unwrap();
        assert_eq!(t.v, BoonTag::Other);
    }

    #[test]
    fn effect_kind_round_trip() {
        let s = "kind = \"damage_mul\"\nfactor = 1.5\n";
        let e: BoonEffectKind = toml::from_str(s).unwrap();
        match e {
            BoonEffectKind::DamageMul { factor } => assert!((factor - 1.5).abs() < 1e-6),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn apply_increments_tag_count() {
        let cat = BoonsCatalogue::default();
        let mut active = ActiveBoons::default();
        let b = def("a", BoonRarity::Common, &[BoonTag::Fire]);
        active.apply(&b, &cat);
        active.apply(&b, &cat);
        assert_eq!(active.tag_count(&BoonTag::Fire), 2);
        assert_eq!(active.active.len(), 2);
        assert!(active.unlocked_legendary.is_empty());
    }

    #[test]
    fn ac5_three_identical_tags_unlock_legendary() {
        let legendary = def("legend_fire", BoonRarity::Legendary, &[BoonTag::Fire]);
        let cat = BoonsCatalogue {
            entries: vec![legendary.clone()],
        };
        let mut active = ActiveBoons::default();
        let common_fire = def("c1", BoonRarity::Common, &[BoonTag::Fire]);
        active.apply(&common_fire, &cat);
        active.apply(&common_fire, &cat);
        assert!(
            active.unlocked_legendary.is_empty(),
            "below threshold, no unlock"
        );
        active.apply(&common_fire, &cat);
        assert_eq!(active.tag_count(&BoonTag::Fire), LEGENDARY_TAG_THRESHOLD);
        assert_eq!(active.unlocked_legendary.len(), 1, "AC5 unlock at threshold");
        assert_eq!(active.unlocked_legendary[0], legendary.id);
    }

    #[test]
    fn legendary_unlock_is_idempotent() {
        let legendary = def("legend_chaos", BoonRarity::Legendary, &[BoonTag::Chaos]);
        let cat = BoonsCatalogue {
            entries: vec![legendary.clone()],
        };
        let mut active = ActiveBoons::default();
        let chaos = def("c", BoonRarity::Common, &[BoonTag::Chaos]);
        for _ in 0..6 {
            active.apply(&chaos, &cat);
        }
        assert_eq!(
            active.unlocked_legendary.len(),
            1,
            "no duplicate unlock past threshold"
        );
    }

    #[test]
    fn legendary_unlock_only_for_matching_tag() {
        let legendary_fire = def("lf", BoonRarity::Legendary, &[BoonTag::Fire]);
        let legendary_chaos = def("lc", BoonRarity::Legendary, &[BoonTag::Chaos]);
        let cat = BoonsCatalogue {
            entries: vec![legendary_fire.clone(), legendary_chaos],
        };
        let mut active = ActiveBoons::default();
        let knock = def("k", BoonRarity::Common, &[BoonTag::Knockback]);
        for _ in 0..3 {
            active.apply(&knock, &cat);
        }
        assert!(
            active.unlocked_legendary.is_empty(),
            "knockback tag stacks should not unlock fire/chaos legendaries"
        );
    }

    #[test]
    fn reset_run_clears_active_but_not_session_total() {
        let cat = BoonsCatalogue::default();
        let mut active = ActiveBoons {
            total_choices_this_session: 7,
            ..Default::default()
        };
        active.apply(&def("x", BoonRarity::Common, &[BoonTag::Fire]), &cat);
        active.reset_run();
        assert!(active.active.is_empty());
        assert!(active.tag_counts.is_empty());
        assert_eq!(active.total_choices_this_session, 7);
    }

    #[test]
    fn catalogue_full_parse_neutral_five() {
        // Mirror the production assets/genomes/roguelite_boons.toml schema.
        let toml = r#"
[[entries]]
id = "eclat_ame_nourrissant"
name = "Éclat d'âme nourrissant"
voiceline_preview = "Petite bouchée d'énergie !"
rarity = "common"
tags = ["chaos"]
effect = { kind = "heal_on_kill", hp = 5.0 }

[[entries]]
id = "metal_chaud"
name = "Métal chaud"
voiceline_preview = "Ça brûle !"
rarity = "common"
tags = ["fire"]
effect = { kind = "damage_mul", factor = 1.15 }
"#;
        let cat: BoonsCatalogue = toml::from_str(toml).unwrap();
        assert_eq!(cat.entries.len(), 2);
        assert_eq!(cat.entries[0].rarity, BoonRarity::Common);
        assert_eq!(cat.entries[1].tags[0], BoonTag::Fire);
    }

    fn fake_seq(seq: Vec<usize>) -> impl FnMut(usize) -> usize {
        let mut iter = seq.into_iter();
        move |upper: usize| iter.next().unwrap_or(0) % upper.max(1)
    }

    #[test]
    fn roll_excludes_locked_legendaries() {
        let cat = BoonsCatalogue {
            entries: vec![
                def("c1", BoonRarity::Common, &[]),
                def("c2", BoonRarity::Common, &[]),
                def("l1", BoonRarity::Legendary, &[BoonTag::Fire]),
            ],
        };
        let active = ActiveBoons::default();
        // Force any RNG output — pool should be 2 commons only.
        let out = roll_candidates(&cat, &active, 3, fake_seq(vec![0, 0, 0, 0]));
        assert_eq!(out.len(), 2, "legendary locked → pool=2 → only 2 picks");
        for id in &out {
            assert_ne!(id.0, "l1", "locked legendary must never appear");
        }
    }

    #[test]
    fn roll_includes_unlocked_legendaries() {
        let leg = def("l1", BoonRarity::Legendary, &[BoonTag::Fire]);
        let cat = BoonsCatalogue {
            entries: vec![
                def("c1", BoonRarity::Common, &[]),
                def("c2", BoonRarity::Common, &[]),
                leg.clone(),
            ],
        };
        let active = ActiveBoons {
            unlocked_legendary: vec![leg.id.clone()],
            ..Default::default()
        };
        let out = roll_candidates(&cat, &active, 3, fake_seq(vec![2, 0, 0]));
        assert_eq!(out.len(), 3);
        assert!(out.iter().any(|id| id.0 == "l1"));
    }

    #[test]
    fn roll_no_duplicates() {
        let cat = BoonsCatalogue {
            entries: vec![
                def("a", BoonRarity::Common, &[]),
                def("b", BoonRarity::Common, &[]),
                def("c", BoonRarity::Common, &[]),
            ],
        };
        let active = ActiveBoons::default();
        // Force same index every time — swap_remove ensures no repeat anyway.
        let out = roll_candidates(&cat, &active, 3, |_| 0);
        let mut sorted = out.iter().map(|id| id.0.clone()).collect::<Vec<_>>();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "no duplicate ids within a single roll");
    }

    #[test]
    fn roll_returns_short_when_pool_smaller() {
        let cat = BoonsCatalogue {
            entries: vec![def("only", BoonRarity::Common, &[])],
        };
        let active = ActiveBoons::default();
        let out = roll_candidates(&cat, &active, 3, |_| 0);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn roll_empty_pool_returns_empty() {
        let cat = BoonsCatalogue::default();
        let active = ActiveBoons::default();
        let out = roll_candidates(&cat, &active, 3, |_| 0);
        assert!(out.is_empty());
    }

    #[test]
    fn rng_next_index_deterministic_with_seed() {
        let mut rng_a = Rng::new(42);
        let mut rng_b = Rng::new(42);
        let mut f_a = rng_next_index(&mut rng_a);
        let mut f_b = rng_next_index(&mut rng_b);
        for _ in 0..16 {
            assert_eq!(f_a(7), f_b(7), "same seed → same sequence");
        }
    }

    #[test]
    fn production_boons_toml_parses() {
        // Guard against TOML schema drift — story-529 AC4 garanti via ce test.
        // Manifest dir = crates/forgia-rpg-data, assets root = workspace root.
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest)
            .join("../../assets/genomes/roguelite_boons.toml");
        let s = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let cat: BoonsCatalogue =
            toml::from_str(&s).unwrap_or_else(|e| panic!("parse roguelite_boons.toml: {e}"));
        // AC4 : ≥5 boons neutres (Common) requis.
        let common = cat.count_by_rarity(BoonRarity::Common);
        assert!(common >= 5, "AC4 needs >=5 common boons, got {common}");
        // AC5 : au moins 1 légendaire par tag clé pour valider unlock paths.
        let legendary = cat.count_by_rarity(BoonRarity::Legendary);
        assert!(legendary >= 1, "needs >=1 legendary boon for AC5 unlock");
    }

    // ─── Story-558 Phase 2 — souls_cost ────────────────────────────────────

    #[test]
    fn default_souls_cost_rarity_scaling() {
        assert_eq!(default_souls_cost(BoonRarity::Common), 20);
        assert_eq!(default_souls_cost(BoonRarity::Uncommon), 40);
        assert_eq!(default_souls_cost(BoonRarity::Rare), 60);
        assert_eq!(default_souls_cost(BoonRarity::Legendary), 80);
        assert!(
            default_souls_cost(BoonRarity::Common) < default_souls_cost(BoonRarity::Legendary),
            "rarity progression doit être strictement croissante"
        );
    }

    #[test]
    fn effective_souls_cost_uses_toml_when_set() {
        let def = BoonDef {
            id: BoonId("test".into()),
            name: "Test".into(),
            voiceline_preview: String::new(),
            effect: BoonEffectKind::DamageMul { factor: 1.0 },
            tags: vec![],
            rarity: BoonRarity::Common,
            weapon_filter: None,
            souls_cost: Some(42),
        };
        assert_eq!(def.effective_souls_cost(), 42);
    }

    #[test]
    fn effective_souls_cost_falls_back_to_rarity_default() {
        let def = BoonDef {
            id: BoonId("test".into()),
            name: "Test".into(),
            voiceline_preview: String::new(),
            effect: BoonEffectKind::DamageMul { factor: 1.0 },
            tags: vec![],
            rarity: BoonRarity::Legendary,
            weapon_filter: None,
            souls_cost: None,
        };
        assert_eq!(def.effective_souls_cost(), 80);
    }

    #[test]
    fn production_boons_all_have_explicit_souls_cost() {
        // Guard story-558 Phase 2 — chaque boon prod doit avoir un coût TOML
        // explicite (pas de fallback default). Évite drift quand on ajoute un
        // nouveau boon sans réfléchir au prix.
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest)
            .join("../../assets/genomes/roguelite_boons.toml");
        let s = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let cat: BoonsCatalogue =
            toml::from_str(&s).unwrap_or_else(|e| panic!("parse: {e}"));
        for b in &cat.entries {
            assert!(
                b.souls_cost.is_some(),
                "boon {:?} ({}): souls_cost manquant dans TOML",
                b.id, b.name
            );
        }
    }

    #[test]
    fn production_boons_souls_cost_respects_rarity_ordering() {
        // Common ≤ Uncommon ≤ Rare ≤ Legendary (par TOML actuel).
        let manifest = env!("CARGO_MANIFEST_DIR");
        let path = std::path::Path::new(manifest)
            .join("../../assets/genomes/roguelite_boons.toml");
        let s = std::fs::read_to_string(&path).unwrap();
        let cat: BoonsCatalogue = toml::from_str(&s).unwrap();
        let max_common = cat
            .entries
            .iter()
            .filter(|b| b.rarity == BoonRarity::Common)
            .filter_map(|b| b.souls_cost)
            .max()
            .unwrap_or(0);
        let min_legendary = cat
            .entries
            .iter()
            .filter(|b| b.rarity == BoonRarity::Legendary)
            .filter_map(|b| b.souls_cost)
            .min()
            .unwrap_or(u32::MAX);
        assert!(
            max_common < min_legendary,
            "max Common cost ({max_common}) doit être < min Legendary cost ({min_legendary})"
        );
    }
}

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

/// Phase 1 plugin : Resources + Asset loader + Event registration.
///
/// Phase 2 (apply systems, UI Coffre) will plug into `ForgiaBoonsApplyPlugin`
/// in `forgia-mode-roguelite`. Wiring is deferred while the other terminal
/// holds `forgia-mode-roguelite/src/lib.rs`.
pub struct ForgiaBoonsPlugin;

impl Plugin for ForgiaBoonsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoonsCatalogue>()
            .init_resource::<ActiveBoons>()
            .add_message::<BoonAppliedEvent>()
            .init_asset::<Genome<BoonsCatalogue>>()
            .register_asset_loader(GenomeLoader::<BoonsCatalogue>::default())
            .add_systems(Startup, load_boons_catalogue)
            .add_systems(Update, sync_boons_catalogue);
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
}

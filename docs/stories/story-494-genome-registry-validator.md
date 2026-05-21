# Story-494 — Genome registry validator cross-crate

**Status:** DRAFT
**Scale:** BMAD Enterprise (10+ fichiers, plan mode recommandé, scope transverse)
**Created:** 2026-05-21
**Blocks:** Migration V1→V2 genome safe · Détection divergence schema entre crates productrices/consommatrices
**Related:** memory `[[reference-v2-genome-t-registration-pattern]]` · audit §6 action #10

---

## 1. Contexte

`forgia-genome-core` actuel = 94 LOC, scaffold. Aucune validation type cross-crate.

**Problème** :
- Chaque crate parse son TOML genome localement (`init_asset + register_asset_loader`)
- Aucune assertion centrale : "tous les genomes du registre déclarent les fields attendus par leurs consumers"
- Détection des divergences uniquement au runtime (panic au premier `get_field`)
- RPG + Roguelite gèrent leurs TOML → risque de drift schema invisible

**Symptômes observés (recensés audit)** :
- `genome-registry` / `validator` scaffolds bloquant migration V1→V2 (audit §5 dette transverse)
- Bug fields `Option<T>` parsé `None` silencieux dans `BiomeGenomeOverrides`
- LOCK-INV-1 80 slots hardcoded vs gene `inventory.max_slots` non-validé

## 2. Goals

1. Crate `forgia-genome-core` peuple : registry typé + validator
2. Macro / trait `Genome` pour déclarer schema attendu par crate consumer
3. Validation au boot (Startup) : iter tous les genomes loaded → assert fields requis présents
4. Sensor `forgia2_genome_validator.json` : count genomes loaded, validation OK/KO, missing fields
5. Health alert si validation fail → severity critical + next_step

## 3. Non-Goals

- Hot-reload validator (Shift+F12 force re-check) → optionnel phase 2
- UI in-game "Genome Inspector" → out-of-scope, dev tool only
- Génération code (proc-macro derive) → garder simple trait-based d'abord

## 4. Acceptance Criteria

- [ ] AC1 — `forgia-genome-core::GenomeRegistry` Resource indexe tous les `Handle<Genome<T>>` loaded
- [ ] AC2 — Trait `GenomeSchema` : `fn required_fields() -> &'static [&'static str]` + `fn validate(&self, toml: &Value) -> Result<(), GenomeError>`
- [ ] AC3 — Au Startup, après asset load, `genome_validator_system` itère registry + appelle `validate` sur chaque
- [ ] AC4 — Sensor `forgia2_genome_validator.json` 1Hz : `{loaded_count, validated, errors: Vec<{path, missing_field}>}`
- [ ] AC5 — Health alert `genome_validator` severity critical si `errors.len() > 0`, next_step pointe le path + field manquant
- [ ] AC6 — Au moins 3 crates migrées vers `GenomeSchema` : `forgia-biome` (BiomeGenomeOverrides), `forgia-inventory` (LOCK-INV-1 max_slots), `forgia-stage-arena` (stage TOML)
- [ ] AC7 — Tests purs : `validate_ok` avec TOML complet, `validate_fail` avec field manquant, `validate_partial` avec field optionnel `#[serde(default)]`
- [ ] AC8 — Doc : `.claude/rules/genome-schema.md` règle bloquante "tout nouveau genome déclare son `GenomeSchema` impl"
- [ ] AC9 — `cargo check + clippy --workspace` clean, 0 warning
- [ ] AC10 — In-game : casser intentionnellement un TOML (retirer field requis) → boot affiche health alert + sensor reflète l'erreur

## 5. Architecture & Patterns

```rust
// forgia-genome-core
pub trait GenomeSchema {
    fn schema_id() -> &'static str;
    fn required_fields() -> &'static [&'static str];
    fn validate(toml: &toml::Value) -> Result<(), GenomeError>;
}

#[derive(Resource, Default)]
pub struct GenomeRegistry {
    pub schemas: HashMap<&'static str, ValidatorFn>,
    pub loaded: HashMap<AssetId, GenomeInfo>,
}
```

**Pattern Epic Data Registry-inspired** : registres typés versionnés, ID stable (`stage.crypts_of_anvil@v2`), validation au load.

## 6. Files Touchés (estim)

- `crates/forgia-genome-core/src/{lib.rs, schema.rs, validator.rs, sensor.rs}` (peuple)
- `crates/forgia-biome/src/lib.rs` (impl GenomeSchema)
- `crates/forgia-inventory/src/lib.rs` (impl GenomeSchema)
- `crates/forgia-stage-arena/src/lib.rs` (impl GenomeSchema)
- `.claude/rules/genome-schema.md` (nouveau)

## 7. Risques

- Boot-time cost : iter N genomes × M fields — mesurer, accepter si < 50 ms
- Trait `GenomeSchema` à backporter sur 20+ genomes existants → migration progressive, pas big-bang
- Conflit avec story-491 si stubs reimpl modifient les schemas concernés → séquencer après #491

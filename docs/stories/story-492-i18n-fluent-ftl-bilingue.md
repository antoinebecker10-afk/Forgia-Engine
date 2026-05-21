# Story-492 — i18n Fluent + .ftl bilingue EN/FR (barks roguelite)

**Status:** DRAFT
**Scale:** BMAD Standard (~6-8 fichiers, story requise, checklist post-impl)
**Created:** 2026-05-21
**Blocks:** Ship Steam Next Fest démo (EN obligatoire 99% marché)
**Related:** story-468 § 0.1 BLOQUANT B4 · audit `docs/audit/audit-rpg-roguelite-2026-05-21.md` §6 action #5

---

## 1. Contexte

Story-468 deep audit (5 agents //) a flagué comme BLOQUANT B4 :
24 barks × 4 armes Roguelite (`pepin`, `bourrasque`, `lenoir`, `boucherie`) sont **strings FR inline dans les TOML**. Aucun roguelite shippé Steam Next Fest démo solo en FR-only (99% du marché exige EN).

**État actuel** :
- `assets/genomes/voicelines/*.toml` ou équivalent : barks en clair FR
- `forgia_audio_voicelines::BarkKind` (quand re-impl post-#491) référence keys en dur
- Aucune crate `forgia-i18n`, aucun `.ftl`

## 2. Goals

1. Introduire dépendance `fluent` + `fluent-bundle` (canonical i18n Rust)
2. Crate `forgia-i18n` minimale : load `.ftl` au boot, lookup `t!("bark.pepin.kill")`
3. Migrer 24 × 4 = 96 strings vers `forgia-i18n/locales/{en,fr}.ftl`
4. Sélection locale via genome `core_gameplay.toml::locale` ou `LANG` env fallback
5. Re-générer barks Tier 1.5 en EN (Claude assist génération)

## 3. Non-Goals

- UI runtime locale switcher in-game → story future
- i18n des dialogues RPG (Aldric, Lyra) → story future (scope distinct)
- TTS / lip-sync — ce n'est pas un audio pipeline

## 4. Acceptance Criteria

- [ ] AC1 — Crate `forgia-i18n` créée (lib, ~150 LOC), `Cargo.toml` dépend `fluent = "0.16"`, `fluent-bundle = "0.15"`
- [ ] AC2 — `forgia-i18n::I18nPlugin` charge `assets/locales/{en,fr}.ftl` au Startup
- [ ] AC3 — Macro/fn `t(key)` ou `i18n.get("bark.pepin.kill")` retourne `Cow<str>` avec fallback EN
- [ ] AC4 — 96 strings migrés (24 barks × 4 armes) — TOML voicelines référence keys, pas strings
- [ ] AC5 — `assets/locales/en.ftl` complet 96 entries (Claude génère, user valide)
- [ ] AC6 — `assets/locales/fr.ftl` complet 96 entries (existant + cleanup)
- [ ] AC7 — Genome `core_gameplay.toml` ajoute `locale = "en"` (default) avec doc
- [ ] AC8 — Sensor `forgia2_i18n.json` 1Hz : `{locale, strings_loaded, missing_keys}`
- [ ] AC9 — Test : `i18n.get("bark.unknown")` retourne key fallback + missing_keys++ dans sensor
- [ ] AC10 — `cargo check + clippy` clean, tests verts
- [ ] AC11 — In-game : changer `locale = "fr"` → Shift+F12 hot-reload → barks switch EN→FR

## 5. Architecture & Patterns

```rust
// forgia-i18n
pub struct I18n {
    bundles: HashMap<LocaleId, FluentBundle<FluentResource>>,
    active: LocaleId,
}

impl I18n {
    pub fn get(&self, key: &str) -> Cow<str> { ... }
}
```

**Pattern observabilité** (rule `observability-required.md`) :
- Sensor `forgia2_i18n.json` : `locale_active`, `strings_loaded`, `missing_keys_session`
- Health alert si `missing_keys > 5` → severity warn + next_step "ajouter clés manquantes dans `.ftl`"

## 6. Files Touchés (estim)

- `crates/forgia-i18n/{Cargo.toml, src/lib.rs, src/sensor.rs}` (nouveau)
- `assets/locales/{en,fr}.ftl` (nouveau)
- `crates/forgia-audio-voicelines/src/lib.rs` (consume `t!()`)
- `assets/genomes/voicelines/*.toml` (FR strings → keys)
- `assets/genomes/core_gameplay.toml` (+ `locale` field)
- `crates/forgia-game/src/lib.rs` (add `I18nPlugin`)

## 7. Risques

- Fluent API évolue lentement, stable 0.16
- 96 strings EN à valider qualité — Claude génère, user passe
- Hot-reload `.ftl` via AssetEvent — vérifier si fluent supporte (sinon restart)

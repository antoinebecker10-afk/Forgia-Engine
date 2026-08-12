# Story-471 — `forgia-analytics` Sentry crash dump (P0 V7)

> ⛔ **CANCELLED 2026-08-12 — cible supprimée par [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md)**
> La crate `forgia-analytics` a été supprimée le 2026-05-26 par le cleanup 266 → 62 crates,
> cinq jours après l'invalidation ci-dessous. Le travail décrit n'est donc plus
> réalisable tel quel : il n'a plus de cible. Le ratchet `cargo xtask no-scaffold`
> interdit désormais de recréer la crate à vide.
>
> **Ce fichier reste une spécification valide** : si le besoin revient, il faut une
> story neuve qui choisit un foyer existant (cf `.claude/rules/fine-grained-crates.md`
> — une crate se justifie par ses consommateurs, pas par son existence).
>
> **Statut** : CANCELLED


> 🚨 **STATUT INVALIDÉ 2026-05-21** — audit RPG/Roguelite a révélé que les claims ci-dessous (12/12 tests, crate peuplé) ne correspondent pas à la réalité :
> - `crates/forgia-analytics/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap commit `19ece92`**
> - Plugin body = `// TODO: implement`
> - `Cargo.toml` n'a aucune dep `sentry`
> - Ce fichier story est `??` (untracked, jamais commité)
> - **0 test** présent dans `src/`
>
> **Vrai statut : DRAFT** (planification valide, implémentation non réalisée).
> Voir `feedback_fictive_done_status_2026_05_21.md` + `docs/audit/audit-rpg-roguelite-2026-05-21.md` §9 R10.

> **Statut d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — crate peuplé, 12/12 tests verts, 0 clippy warning, binary compile.
> **Reste** : wiring `app.add_plugins(ForgiaAnalyticsPlugin)` dans `forgia-game` (Quick BMAD séparée, hors scope cette story).
> **Scale BMAD** : Standard (~5 fichiers : Cargo + lib + genome + story + checklist)
> **Date** : 2026-05-19
> **Origine** : Audit maturité crates 2026-05-19 — P0 #8 (1j scope, infrastructure ship V7)
> **Bloque** : ship Next Fest démo solo (Sentry obligatoire pour capter crash post-publication)

## Pitch

Peupler scaffold `forgia-analytics` (16 LOC) avec **Sentry Rust SDK crash dump opt-in RGPD-compliant**. Capture `panic!` automatiquement, envoie au DSN configuré si user opt-in. Aucune télémétrie comportementale au MVP — uniquement crash dumps. PostHog/events reportés P2 post-launch.

## Acceptance Criteria

- [x] `AnalyticsConfig` Resource chargée depuis `assets/genomes/analytics.toml`
- [x] Default `opt_in = false` (RGPD)
- [x] Sentry init seulement si `opt_in == true` ET `dsn` non vide
- [x] `ClientInitGuard` Bevy NonSendResource (drop = flush + close au Shutdown)
- [x] Panic hook auto-register via feature `panic` Sentry
- [x] Sensor JSON `forgia_analytics.json` 1Hz : `{ opt_in, sentry_running, dsn_configured, release, environment }`
- [x] Tests purs : config parse, default off, empty DSN no-op
- [x] `cargo check -p forgia-analytics` vert
- [x] `cargo clippy -p forgia-analytics --no-deps -- -D warnings` vert
- [x] Aucun hardcode DSN (rule `no-hardcode.md`)

## Architecture

```
forgia-analytics/
  src/lib.rs        — AnalyticsConfig + AnalyticsState + ForgiaAnalyticsPlugin + sensor writer
assets/genomes/
  analytics.toml    — opt_in=false (default), dsn="", release="0.1.0-dev", environment="dev"
```

API exposée :

```rust
pub use { ForgiaAnalyticsPlugin, AnalyticsConfig, AnalyticsState };
```

## Sources industrie

- [Sentry Rust SDK docs](https://docs.sentry.io/platforms/rust/) — `sentry::init` + panic feature
- [Sentry game dev solution](https://sentry.io/for/game-development/) — pattern crash dump indé
- [RGPD opt-in default OFF](https://gdpr-info.eu/art-7-gdpr/) — consent obligatoire UE

## Risques

- **`ClientInitGuard` is !Send** → NonSendResource obligatoire (pas Resource). Documenté.
- **DSN config sensible** : `analytics.toml` ne doit PAS être commité avec un vrai DSN prod. Convention : `analytics.toml.example` checked in, `analytics.toml` gitignore.
- **Panic hook conflict** : Sentry register par-dessus existing hook. Forgia n'a pas de panic_hook custom (vérifié grep). OK.

## Out of scope (P2 post-launch)

- PostHog events opt-in (behavioral analytics)
- Steam Stats API integration (achievements/leaderboards)
- Tracing breadcrumbs Sentry (frame-by-frame context)
- User feedback Sentry dialog
- Performance monitoring Sentry (transactions)

# Story-638 — P0-1 : stats ennemis data-driven (genome + sensor)

> **Source** : plan de production `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5,
> Phase P0, item **P0-1**. Direction : `docs/design/direction-forgia-gunfire.md`.
> **Pourquoi** : les stats ennemis sont hardcodées (`enemies.rs::stats_for`/`bot_shoot_for`) —
> le commentaire L13 note déjà « Hot-reload TOML : reporté M2 step 3 ». C'est le **prérequis
> du scaling de difficulté** (P0-2 défense tri-couche + paliers) et ça respecte `no-hardcode`.
> **Scale BMAD** : Standard (≥2 crates : `forgia-mode-roguelite` + genome). **Date** : 2026-07-01.
> **Statut** : IN_PROGRESS.

## Objectif
Extraire les stats des 4 archétypes (Tank/Runner/Sniper/Boss) vers un genome
`roguelite_enemies.toml` **hot-reloadable**, chargé au boot, avec un **sensor
`forgia2_enemies.json`**. Zéro changement de comportement (Default = miroir exact
des valeurs actuelles).

## Critères d'acceptance
| # | AC | Preuve |
|---|---|---|
| AC1 | `roguelite_enemies.toml` = stats des 4 archétypes (hp/speed/ranges/capsule/couleurs + combat dmg/range/jitter) | fichier genome |
| AC2 | `EnemyStatsConfig` Resource (Deserialize, Default = miroir) + parse-or-Default + hot-reload mtime 1Hz | `enemies.rs` |
| AC3 | Config Resource + hot-reload de la Resource livrés (prérequis P0-2). **Spawn-live = différé P0-2** (le sensor expose `spawn_live:false` ; le spawn lit encore le Default = valeurs shippées identiques). P0-2 réécrit le spawn (défense tri-couche) et consommera la config live. | `enemies.rs` + `lib.rs` |
| AC4 | Sensor `forgia2_enemies.json` (stats live par archétype + reload_count) | `enemies.rs` + registre |
| AC5 | Zéro régression : les tests `enemies` existants passent, comportement identique si TOML absent | `cargo test` |
| AC6 | 0 warning clippy, no-hardcode respecté (littéraux → Default de la config) | `cargo clippy` |

## Décisions
- Pattern calqué sur `elements.rs` / `ultimate_config.rs` : `Default` Rust = miroir exact
  du TOML ; `parse_toml` → `unwrap_or_else(Default)` (garbage/absent → défaut complet).
- Les valeurs quittent les `match` de `stats_for`/`bot_shoot_for` → vont dans le `Default`
  de `EnemyStatsConfig` (centralisées, TOML-overridable). Les fns libres délèguent au Default
  (compat tests + appelants purs) ; le spawn lit la Resource LIVE.
- `tracer_emissive`/`shoulder_y` de `BotShootConfig` restent en code (visuel), seuls les
  scalaires de combat (dmg/range/jitter) passent en genome.

## Suite
P0-2 = `DefenseLayer{Health,Shield,Armor}` — consommera `EnemyStatsConfig` pour le scaling.

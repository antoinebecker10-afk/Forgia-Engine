# Story-517 — UI + Weapons cleanup (DELETE 14 scaffolds + FUSION 6 UI réelles)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `lib.rs`, symbole `ForgiaUiLibPlugin`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status** : WIP
**BMAD Scale** : Standard (~25 fichiers, bounded scope)
**Created** : 2026-05-26
**Branch** : `cleanup/story-517-ui-weapons-fusion`
**Predecessor** : story-516 (DONE, PR #2 merged 2026-05-26)
**Source plan** : `docs/audit/scaffolds-audit-2026-05-23.md` (révisé : audit "0 consumer partout" obsolète, 6 UI ont consumers réels)

---

## 1. Contexte

Audit 2026-05-23 proposait "fusion 12 UI + 5 weapons → 2 meta-crates". Inspection 2026-05-26 montre :

- **10 UI sont pures scaffolds** 16 LOC, 0 consumer → simple DELETE (pattern story-516)
- **4 weapons sont scaffolds** 16 LOC, 0 consumer → simple DELETE
- **6 UI sont substantielles** (99-690 LOC) avec consumers réels (`forgia-game`, `forgia-killfeed`, `forgia-juice-screen-flash`, `forgia-mode-roguelite`) → vraie fusion avec migration call sites
- **1 weapon (hitscan)** 150 LOC, 0 consumer → orphelin code réel, à garder isolée

Stratégie pragmatique : 2 phases.

---

## 2. Phase A — DELETE 14 scaffolds (pattern story-516)

### A1 — 10 UI scaffolds 16 LOC, 0 consumer

| Crate | Justif |
|---|---|
| forgia-ui-credits | Plugin stub |
| forgia-ui-gauges | idem |
| forgia-ui-inventory | idem |
| forgia-ui-loadscreen | idem |
| forgia-ui-menu | idem |
| forgia-ui-minimap | idem |
| forgia-ui-notifications | idem |
| forgia-ui-objectives | idem |
| forgia-ui-settings-panel | idem |
| forgia-ui-tooltip | idem |

### A2 — 4 weapon scaffolds 16 LOC, 0 consumer

| Crate | Justif |
|---|---|
| forgia-weapon-beam | Plugin stub |
| forgia-weapon-charged | idem |
| forgia-weapon-melee | idem |
| forgia-weapon-projectile | idem |

**Garder** : `forgia-weapon-hitscan` (150 LOC, 0 consumer mais code réel — à wire en consumer plus tard ou story-518 verdict).

---

## 3. Phase B — FUSION 6 UI substantielles → `forgia-ui-lib`

### B1 — Crates à fusionner

| Source | LOC | Module cible | Consumer(s) |
|---|---|---|---|
| forgia-ui-style | 179 | `style` | hud, hud-ammo, damage-direction, pause-menu, killfeed, juice-screen-flash, mode-roguelite |
| forgia-ui-hud | 292 | `hud` | forgia-game |
| forgia-ui-hud-ammo | 690 | `hud_ammo` | forgia-game |
| forgia-ui-pause-menu | 374 | `pause_menu` | forgia-game |
| forgia-ui-damage-direction | 395 | `damage_direction` | forgia-game |
| forgia-ui-dialogue | 99 | `dialogue` | 0 consumer (but real code, keep) |

**Total** : 2029 LOC consolidées dans 1 crate `forgia-ui-lib`.

### B2 — Architecture cible

```rust
// crates/forgia-ui-lib/src/lib.rs
pub mod style;
pub mod hud;
pub mod hud_ammo;
pub mod pause_menu;
pub mod damage_direction;
pub mod dialogue;

pub struct ForgiaUiLibPlugin;
impl Plugin for ForgiaUiLibPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            style::ForgiaUiStylePlugin,
            hud::ForgiaUiHudPlugin,
            hud_ammo::ForgiaUiHudAmmoPlugin,
            pause_menu::ForgiaUiPauseMenuPlugin,
            damage_direction::ForgiaUiDamageDirectionPlugin,
            dialogue::ForgiaUiDialoguePlugin,
        ));
    }
}
```

Consumers utilisent soit `ForgiaUiLibPlugin` (bundle) soit modules individuels.

### B3 — Call sites à migrer

- `crates/forgia-game/Cargo.toml` : 4 deps `forgia-ui-{hud,hud-ammo,pause-menu,damage-direction}` → 1 dep `forgia-ui-lib`
- `crates/forgia-game/src/lib.rs` : imports `forgia_ui_hud::*` → `forgia_ui_lib::hud::*`
- `crates/forgia-killfeed/Cargo.toml` + src : `forgia-ui-style` → `forgia-ui-lib::style`
- `crates/forgia-juice-screen-flash/Cargo.toml` + src : idem
- `crates/forgia-mode-roguelite/Cargo.toml` + src : idem
- `crates/forgia-ui-damage-direction`, `forgia-ui-hud-ammo`, `forgia-ui-hud`, `forgia-ui-pause-menu` (les internes qui se réfèrent à style) : également migrer

---

## 4. Acceptance Criteria

- [ ] AC1 — 14 dirs scaffolds (10 UI + 4 weapons) supprimés
- [ ] AC2 — Cargo.toml racine nettoyé (14 members + 14 deps + 14 allowlist entries)
- [ ] AC3 — `crates/forgia-ui-lib/` créé avec 6 modules + plugin meta
- [ ] AC4 — 6 anciennes crates UI supprimées
- [ ] AC5 — 4 consumers externes migrés (forgia-game, forgia-killfeed, forgia-juice-screen-flash, forgia-mode-roguelite)
- [ ] AC6 — `cargo check --workspace` PASS
- [ ] AC7 — `cargo clippy --workspace --no-deps` PASS 0 warning
- [ ] AC8 — `cargo xtask no-scaffold` PASS
- [ ] AC9 — Allowlist 75 → ~61 entries (14 retraits)
- [ ] AC10 — Commits propres par phase (A1, A2, B1-création, B2-migration, B3-deletes)

---

## 5. Anti-patterns à éviter

- ❌ Fusion sans audit consumers (audit 2026-05-23 disait "0 partout" → faux pour 6 crates)
- ❌ Renommer les Plugin types pendant fusion (garder `ForgiaUiHudPlugin` etc., juste réorganiser leur path)
- ❌ Mélanger Phase A (deletes) et Phase B (fusion) dans même commit
- ❌ `git add -u .` (stowaway pattern Windows, cf memory)

---

## 6. Result expected

- **Crates avant** : 191 (post story-516)
- **Crates après Phase A** : 191 - 14 = **177**
- **Crates après Phase B** : 177 - 6 + 1 = **172**
- **Total story-517** : **-19 crates**
- Allowlist no-scaffold : 75 → ~61

---

## 7. Stories follow-up

- **story-518** : Genome cleanup (5 crates delete/implement)
- **story-519** : Input + Audio IMPLEMENT (12 crates, consumers existent)
- **story-520** : Player controllers + Misc (8 crates)
- **story-test-fix** : tests pré-existants `in_state`/`default` (forgia-rpg, forgia-mode-roguelite, forgia-effects)
- **story-arch-rpg-split** : split forgia-rpg/lib.rs (1952 LOC monolithique)

# Story-588 — VFX colorés des éléments (rendre le système d'éléments visible)

**Statut** : EN COURS
**Niveau BMAD** : Standard
**Date** : 2026-06-09
**Cible** : SHIP Roguelite — "A5-bis" de [[project-roguelite-element-progression-design]]. Le design dit explicitement : **VFX de Phase A AVANT** de câbler la progression (Phase B). Aujourd'hui les éléments (feu/poison/explosif/perforant) tournent (story-582) mais sont **invisibles** → un système qu'on ne voit pas n'existe pas (`observability-required`).

## Demande
À chaque hit élémentaire, un **flash coloré à l'impact** (couleur par élément) + un **pulse coloré** sur les ennemis en DoT (burn/poison), pour que le joueur RESSENTE l'élément de son arme.

| Élément | Arme | Couleur |
|---|---|---|
| 🔥 Feu | SMG Bourrasque | orange |
| 🟣 Poison | pompe Boucherie | vert |
| 💥 Explosif | pistolet Pépin | jaune |
| 🎯 Perforant | sniper Lenoir | cyan |

## Architecture (perf — par-hit, haute fréquence)
- **1 mesh sphère + 4 matériaux partagés** (1/élément), construits une fois (`ElementVfxAssets`). Hot-reload couleur **en place** (`materials.get_mut`, mêmes handles → sparks vivants mis à jour). **Zéro alloc matériau/hit** (vs `shockwave.rs` qui alloue/cast, OK car cooldown long).
- Fade **par scale** (sphère → 0) + intensité de lumière fadée par-entité (PointLight sur l'entité spark, pas un enfant). Pas de fade alpha (matériau partagé).
- **Cap** d'instances actives (anti-spam SMG) + lumière sur les impacts seulement (les pulses DoT n'ont pas de lumière → borne le nombre de PointLights).
- Data-driven : section `[vfx]` dans `roguelite_elements.toml` (couleurs RGB + scale + ttl + intensité), miroir `VfxParams::default()`, `#[serde(default)]` (backward-compat si absent).

## Implémentation (self-contained `forgia-mode-roguelite`)
| Fichier | Rôle |
|---|---|
| `elements.rs` | + `VfxParams` (genome) + champ `ElementConfig.vfx` + `Element::idx()`/`rgb()` |
| `roguelite_elements.toml` | + section `[vfx]` |
| `element_vfx.rs` (nouveau) | `ElementVfxAssets`, `ElementSpark`, spawn impact + pulse DoT + tick fade + refresh mats hot-reload + sensor `forgia2_element_vfx.json` |
| `lib.rs` | `pub mod element_vfx;` + enregistrement systèmes |

## QA
- [x] `cargo check -p forgia-mode-roguelite` + clippy 0 (2026-06-09)
- [x] Tests purs (idx/rgb stables, VfxParams default sain, [vfx] absent → default) — 98 tests verts
- [x] Auto-QA points ouverts vérifiés (L1 intact, 2 lecteurs CombatHitEvent indépendants, 0 alloc/hit, hot-reload OK)
- [ ] **Runtime** : tirer chaque arme → flash de la bonne couleur à l'impact ; ennemi en feu/poison pulse orange/vert ; `forgia2_element_vfx.json` (sparks_spawned monte)

## Reste / suite
- [ ] Phase B : câbler choix portail → déblocage d'élément (remplace `always_on`)
- [ ] bevy_hanabi (particules GPU) au lieu de sphères si besoin de densité
- [ ] tint du matériau ennemi (vs pulse) — nécessite matériaux per-instance

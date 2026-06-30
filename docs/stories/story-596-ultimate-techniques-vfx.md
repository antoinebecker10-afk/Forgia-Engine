# Story-596 — État Ultime (F) + techniques signature par arme + VFX

> **Source** : demande game-maker 2026-06-30 — « VFX complexes par arme : Pépin
> explose, Bourrasque électrise en chaîne, Mme Lenoir perfore + empoisonne, Pompe
> givre en zone ».
> **Design validé** (Antoine, 2026-06-30) :
> - **1A** : le 4ᵉ slot Boucherie (lance-roquettes) devient **Pompe** (fusil à pompe givrant).
> - **2A** : la touche **F** n'est plus un cast instantané — elle ouvre un **État Ultime
>   de 10 s** pendant lequel l'arme équipée débloque sa technique signature.
> - **Passif inchangé** : hors Ultime, l'arme garde son élément actuel ; la technique
>   s'ajoute PAR-DESSUS pendant les 10 s.
> - **VFX** : flipbook billboard (assets CC0 placeholder) maintenant, Hanabi ensuite.
> **Scale BMAD** : Enterprise (multi-crate, 10+ fichiers). **Date** : 2026-06-30.
> **Statut** : **IN_PROGRESS** (T1 en cours — NE PAS marquer DONE sans `xtask story-gate`).

## Techniques d'Ultime (10 s) par arme

| Arme (slot) | Technique pendant l'Ultime | Réutilise |
|---|---|---|
| **Pépin** (ModernAR) | tirs → **Explosion AOE** | `Element::Explosive` splash existant, params boostés |
| **Bourrasque** (AssaultRifle) | tirs → **Électrique en chaîne** | hook `PlayerCombatMods.chain_extra_targets` (inutilisé) |
| **Mme Lenoir** (Shotgun=sniper) | tirs → **Perforation + Poison** | `line_strike` (shockwave) + `StatusPoison` existant |
| **Pompe** (RocketLauncher→pompe) | tirs → **Gel de zone (AOE)** | nouveau `StatusFreeze` calqué sur `StatusBurn` |

## Découpage en tiers (compile-vérifié entre chaque)

| Tier | Contenu | Crates | Risk | Statut |
|---|---|---|---|---|
| **T1** | `UltimateState` (timer 10 s + cooldown + sensor) + tick | forgia-combat | 🟢 Low | ✅ done |
| **T1b** | Input F → `try_activate` + sensor `forgia2_ultimate.json` | forgia-mode-roguelite | 🟡 Med | ✅ done |
| **T2** | Logique pure : chaîne électrique + modèle de gel + tunables | forgia-mode-roguelite | 🟡 Med | ✅ done |
| **T3** | Branchement Ultime→technique sur le hit (gaté `is_active`) + gel runtime + sensor `forgia2_ultimate_tech.json` | roguelite | 🟠 High | ✅ done (mécanique ; tuning/VFX = T4 VS Code) |
| **T4a** | Genome de tuning `roguelite_ultimate.toml` (hot-reload durées/rayons/dégâts) | forgia-mode-roguelite, assets | 🟡 Med | ✅ done (15 tests, 0 warning) |
| **T4b** | VFX flipbook billboard + SFX par technique (assets CC0) | forgia-effects, assets | 🟡 Med | à faire (VS Code, GPU + œil) |

## Critères d'acceptance

| # | AC | Statut | Preuve |
|---|---|---|---|
| AC1 | `UltimateState` : F active 10 s puis cooldown, verrou total = durée+cd | ⏳ | `forgia-combat/src/ultimate.rs` + 6 tests unitaires |
| AC2 | Tunables data-driven (durée/cooldown genome), 0 hardcode enfoui | ⏳ | champs `duration`/`cooldown`, genome story de suivi |
| AC3 | Boucherie renommée **Pompe** (identité fusil à pompe givrant) | ⏳ | T1b/T2 |
| AC4 | Pendant l'Ultime, chaque arme applique sa technique signature | ⏳ | T3 |
| AC5 | Électrique en chaîne (N voisins) via `chain_extra_targets` | ⏳ | T2/T3 |
| AC6 | Gel de zone : `StatusFreeze` ralentit/immobilise X s | ⏳ | T2/T3 |
| AC7 | VFX placeholder + SFX par technique (attribution CREDITS.md) | ⏳ | T4 |
| AC8 | Sensor `forgia2_ultimate.json` (actif, cd, activations) | ⏳ | T1b |

## Décisions d'architecture

- **`UltimateState` dans forgia-combat** (dep root), neutre/testable headless, calqué
  sur `PlayerCombatMods`. L'input F + le dispatch technique vivent dans
  forgia-mode-roguelite (qui a déjà `RunState`, `EquippedWeapons`, `elements.rs`).
- **Techniques ≠ éléments passifs** : on NE pollue PAS l'enum `Element` (système
  passif). Les techniques d'Ultime sont une couche distincte gatée par
  `is_active()`. `StatusFreeze` est un nouveau component, pas un nouvel `Element`.
- **Réutilisation maximale** : explosion = splash `Element::Explosive` existant ;
  poison = `StatusPoison` existant ; perforation = `line_strike` (ex-shockwave) ;
  chaîne = `chain_extra_targets` déjà prévu dans `PlayerCombatMods`.

## Anti-traps respectés

- Ennemis = `forgia_combat::Health` (PAS `forgia_damage::Health`) → mutation directe
  + `CombatHitEvent`, mort déléguée à `despawn_dead_cubes` (jamais `DamageEvent`).
- VFX Hanabi (T4 phase 2) : pré-spawn dummy `Visibility::Hidden` au Startup.
- 0 alloc hot path : buffers `Local<Vec<Entity>>` réutilisés (cf. AOE explosif).

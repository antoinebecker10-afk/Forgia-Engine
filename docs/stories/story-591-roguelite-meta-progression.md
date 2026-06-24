# Story-591 — L'Enclume des Âmes (méta-progression permanente)

**Statut** : EN COURS
**Niveau BMAD** : Enterprise (7 fichiers + changement de flow Lobby)
**Date** : 2026-06-09
**Cible** : SHIP Roguelite — le sink inter-run qui manque. Les Âmes s'accumulent (persistant en session) mais **rien où les dépenser** ET **rien n'est sauvé sur disque** (perdu au reboot). Recherche+plan via workflow `roguelite-meta-progression-design` (10 agents).

## Décision (workflow + user)
- **Design** : **stat-shop linéaire** (« L'Enclume des Âmes »). 4 upgrades à rangs, coût croissant en Âmes.
- **Emplacement (user)** : **Hub Lobby**. ⚠️ Le Lobby auto-démarrait la run (`auto_start_run_on_enter`) → je le retire : entrée Roguelite **stoppe au Lobby** (Enclume affichée), **ENTRÉE = lancer la run**, et Victory/Defeat → **retour Lobby** (au lieu de StartRun direct) → vrai hub re-visitable.
- **Upgrades = hooks VÉRIFIÉS sans édition cross-crate** (vitesse droppée car `MovementSpeedMultiplier` écrasé chaque frame par l'ADS ; fire_rate droppé car aucun consommateur) :
  | Upgrade | Effet | Hook |
  |---|---|---|
  | Vitalité | +PV max | `Health.max` au run-start (miroir reset HP run.rs:675) |
  | Puissance | +% dégâts | `PlayerCombatMods.damage_mul` via `PermanentPlayerMods` |
  | Armure | +% réduction | `PlayerCombatMods.damage_reduction` → HealthGuard |
  | Pactole | Or de départ | `Gold.current` au run-start |

## Persistance
Aucun système de save dans Forgia. Pattern config réutilisé (`toon_config.rs` : `fs` + `serde` + `toml`, déjà en deps). Fichier `meta_shop_save.toml` dans `forgia_terrain::config_dir::find_config_dir()`, schéma versionné `MetaShopSave { version, souls_total, ranks }`. **Save événementiel** (achat + OnExit + OnEnter Victory/Defeat). **Réconciliation** : `save.souls_total = MetaSouls.current` avant chaque save. **Load au Startup** (1×) → `MetaSouls.current` (évite l'écrasement au re-entry).

## Implémentation
| Fichier | Rôle |
|---|---|
| `meta_shop.rs` (nouveau) | `MetaShopSave` (persist), `MetaShopCatalogue` (data-driven), `PermanentPlayerMods`, `MetaEffect`, input clavier 1-4/ENTRÉE, draw Lobby egui, flush save, `MetaShopPlugin` |
| `run.rs` | `sys_start_run` : applique maxhp (closure HP) + or de départ + `PermanentPlayerMods` |
| `boons_apply.rs` | `sys_recompute_boon_mods` compose `PermanentPlayerMods` (damage_mul/reduction) AVANT l'overwrite + recompute si perm changé |
| `lib.rs` | register `MetaShopPlugin` ; **retire `auto_start_run_on_enter`** |
| `hud.rs` | Victory/Defeat « Nouvelle Run » → `RunState::Lobby` |
| `sensor.rs` | `forgia2_roguelite_state.json` + meta_souls_total + ranks |
| `roguelite_meta_shop.toml` (nouveau) | catalogue data-driven |

## QA
- [ ] check + clippy 0 ; tests purs (cumul bonus, save round-trip, cost/rank)
- [ ] Runtime : entrée Roguelite → Lobby Enclume → acheter (1-4) décrémente Âmes + monte rang → ENTRÉE lance la run avec les bonus (PV/dégâts/armure/or) → mourir → retour Lobby → Âmes persistées ; relancer le jeu → Âmes + rangs conservés (disque)
- [ ] `forgia2_roguelite_state.json` : meta_souls_total + ranks

## Reste / suite
- [ ] Upgrade vitesse (besoin d'un hook propre côté forgia-player/viewmodel)
- [ ] Élément/boon de départ ; respec ; tiers Hadès Mirror (variantes A/B)

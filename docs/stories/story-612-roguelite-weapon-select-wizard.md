# Story-612 — Roguelite : Wizard de choix d'arme de départ (Phase 0)

> **Statut** : DRAFT — prototype egui livré, validation runtime à faire
> **Niveau BMAD** : Standard (module `weapon_select.rs` + 1 edit wiring lib.rs)
> **Demande user** : « comment fonctionnent les interfaces pour choisir ton
> personnage, ton arme, avec niveaux/dégâts/types de dégâts… fais un audit
> internet et un rapport pour améliorer notre wizard de roguelite ».
> **Rapport source** : [best-practices-wizard-roguelite-2026-06-23](../best-practices-wizard-roguelite-2026-06-23.md).

## Contexte

Il n'existe **aucun wizard** de pré-run : le joueur démarre toujours avec Pépin
(`EquippedWeapons::default()`), zéro stat affichée. Or la donnée existe déjà.
Phase 0 = premier choix signifiant (l'arme de départ) qui surface les stats réelles
+ le système d'éléments (story-582) au Lobby (L'Enclume des Âmes).

> Pas de sélection de **héros** : 1 seul perso (« Apprenti ») → un character-select
> à 1 entrée serait vide. Reporté à 2+ héros. Le choix d'arme est immédiatement
> signifiant (4 playstyles × 4 éléments) et coûte **0 nouvelle donnée**.

## ⚠️ Décision d'architecture (concept-first) — vraie source de stats

| Genome | Statut | À utiliser ? |
|---|---|---|
| [`viewmodel_arena.toml`](../../assets/genomes/viewmodel_arena.toml) | ✅ **vraie source combat** (lue par `forgia-fps` via `ViewmodelGenomeEntry`) | **OUI** |
| `roguelite_weapons.toml` | ❌ genome MORT (0 consommateur Rust, valeurs divergentes : Pépin dmg 18 vs 28) | NON |

Brancher la carte sur `roguelite_weapons.toml` ferait **mentir** le wizard. Le
module lit donc `viewmodel_arena.toml` **en direct** (fs + mtime hot-reload), même
pattern que [`meta_shop.rs`](../../crates/forgia-mode-roguelite/src/meta_shop.rs) et
[`elements.rs`](../../crates/forgia-mode-roguelite/src/elements.rs) dans cette
crate → zéro nouvelle dépendance cross-crate.

**Piège de nommage** (legacy enum V1) : `WeaponType::Shotgun` = **Madame Lenoir
(sniper !)**, `WeaponType::RocketLauncher` = Boucherie. Clé genome via `vm_key()`.

## Architecture (concept-first)

- Concept = `combat` / UI sélection (couche fw + def TOML). Net = local. Script = interne.
- **Producteur (vérité)** :
  - stats combat = `viewmodel_arena.toml` (DMG/cadence/portée/chargeur/recharge/pellets/head_mul) ;
  - éléments + matchups = `ElementConfig` (Resource déjà init, story-582).
- **État du choix** : `StartingWeaponChoice { idx }` (index dans `ARENA_V1_WEAPONS`,
  default 0 = Pépin → backward-compatible).
- **Consommateurs** :
  - `sys_weapon_select_input` (frame, `GameSet::UI`, run_if `RunState::Lobby`) — ← / →
    cyclent l'arme choisie (layout-agnostic AZERTY, pas de conflit avec les 1-4 du meta-shop).
  - `draw_weapon_select` (`EguiPrimaryContextPass`) — carte ancrée à droite de L'Enclume.
  - `sys_apply_weapon_choice` (`OnExit(RunState::Lobby)`) — applique le choix à
    `EquippedWeapons.current` (le joueur démarre en main son arme choisie).
- **DPS** : fn pure `weapon_dps(damage, fire_rate, pellets)` ; cas roquette
  (`damage = 0`, Boucherie) → étiquette « roquette AOE » (pas de DPS inventé).
- **Réutilisé** : `hud::speaker_color` (accent persona), `run::weapon_to_speaker`,
  `ElementConfig::element_for/matchup_for`, `Element::fr_name/tag`.

## Contenu de la carte (≤ 6 metrics + élément + matchup, cf Loi #3/#4 du rapport)

| Arme | DMG | Cadence | DPS | Mag | Recharge | Portée | Élément |
|---|---|---|---|---|---|---|---|
| Pépin (ModernAR) | 28 | 6.0/s | **168** | 12 | 1.2 s | 80 m | Explosif |
| Bourrasque (AssaultRifle) | 11 | 11.0/s | **121** | 30 | 1.6 s | 30 m | Feu |
| Madame Lenoir (Shotgun) | 50 | 0.8/s | **40** (one-shot tête ×2) | 5 | 2.5 s | 300 m | Perforant |
| Boucherie (RocketLauncher) | — | 0.9/s | **AOE** (roquette 70) | 3 | 1.33 s | 60 m | Poison |

+ ligne « Fort vs / Faible vs » calculée (max/min des multiplicateurs sur les 4
archétypes Tank/Runner/Sniper/Boss).

## Critères d'acceptation

- [ ] Au Lobby, une carte d'arme s'affiche à côté de L'Enclume (egui, gated InGame+Roguelite+Lobby).
- [ ] ← / → changent l'arme sélectionnée parmi les 4 (cycle), sans casser les 1-4 / ENTRÉE du meta-shop.
- [ ] La carte montre DMG, Cadence, DPS, Chargeur, Recharge, Portée **lus de `viewmodel_arena.toml`** (valeurs réelles, hot-reload).
- [ ] Boucherie (`damage=0`) affiche « roquette AOE », pas un DPS faux.
- [ ] Badge élément + identité (persona + tagline) + « Fort vs / Faible vs » corrects (depuis `ElementConfig`).
- [ ] ENTRÉE lance la run **avec l'arme choisie en main** (`EquippedWeapons.current` = choix).
- [ ] Default idx 0 = Pépin → aucune régression si le joueur ne touche à rien.
- [ ] `cargo check -p forgia-mode-roguelite` + clippy 0 warning + tests verts (weapon_dps, parse, strong/weak).

## Test runtime (à valider)

1. **Action** : lancer le Roguelite → au Lobby, appuyer ← / → puis ENTRÉE.
2. **Rechargement** : rebuild (`cargo run -p forgia`) ; les stats hot-reload (Shift+F12 / save TOML).
3. **Effet attendu** : la carte change d'arme à chaque ← / → ; après ENTRÉE, le joueur
   tient l'arme choisie (slot actif au HUD).
   ⚠️ L'**élément** armé de départ ne suivra le choix qu'après le fix Phase 1 (cf Suivi) :
   `sys_reset_element_unlocks` ne tourne qu'`OnEnter(GameMode::Roguelite)`, pas par-run.
4. **Où observer** : HUD bas-droite (slot actif surligné = arme choisie). `forgia2_elements.json`
   `unlocked` reflètera encore l'arme d'entrée-de-mode au 1er run (limite connue, Phase 1).
5. **Variantes si KO** :
   - Carte vide / « — » partout → `viewmodel_arena.toml` introuvable (vérifier cwd) ou parse KO → fallback mirror.
   - L'arme ne change pas en jeu → `sys_apply_weapon_choice` ne tourne pas / `EquippedWeapons` absent au Lobby.
   - DPS faux → vérifier `fire_rate` = tirs/s (pas s/tir) dans le genome.

## Suivi (Phase 1, hors scope story-612)

- Stat block au survol/switch dans le HUD in-run (Loi #3).
- Comparaison à la prise de loot avec flèches ↑/↓ (Loi #6).
- Bordure de carte colorée par rareté (loot pool).
- Reset de `sys_reset_element_unlocks` aussi au run-start (pas que OnEnter mode) pour
  que l'élément de départ suive un changement d'arme entre 2 runs sans ressortir du mode.
- Externaliser les taglines persona en genome (v1 = texte UI local, cosmétique).

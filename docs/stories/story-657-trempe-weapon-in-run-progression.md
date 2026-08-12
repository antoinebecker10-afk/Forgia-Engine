# Story-657 — La Trempe : progression de l'arme en cours de run (Inc.1)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_trempe.json`, fichier `boons_apply.rs`, symbole `sys_recompute_boon_mods`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> Renumérotée 653→657 (2026-07-02) : collision avec `story-653-aura-shock-arcs-electriques`
> de l'autre terminal. Le code référence encore « story-653 » dans ses commentaires.
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS
**Niveau BMAD** : Standard (1 module neuf + 1 ligne dans le recompute + 1 TOML, 1 crate)
**Crate** : `forgia-mode-roguelite`
**GDD** : [gdd-run-structure-weapon-progression.md](../design/gdd-run-structure-weapon-progression.md) — mécanique M3
**Date** : 2026-07-02

## But

Donner un **intérêt à monter les stats de l'arme in-run** (demande Antoine, référence Gunfire
Reborn). Chez Forgia, l'arme est une amie — on ne la jette pas, on la **forge** : « La Trempe ».
Cet incrément livre la boucle cœur : *gagner de l'Or → tremper l'arme chez le forgeron →
l'arme fait plus mal*. Les Gravures (choix aux paliers 3/5) et le scaling ennemi sont des
incréments suivants (E1/E4 du GDD).

## Concept-first (mapping vérifié)

- **Injection dégâts** : `PlayerCombatMods.damage_mul` (forgia-combat), déjà lu par le tir
  forgia-fps. Recalculé chaque frame par `sys_recompute_boon_mods` (boons_apply.rs) qui
  compose déjà `× perm.damage_mul × mastery.damage_mul`. J'ajoute `× trempe.damage_mul` sur
  la MÊME ligne → un seul writer, stacking multiplicatif (voulu par le GDD), zéro conflit.
- **Économie** : l'Or in-run = `forgia_rpg_data::loot_tables::Souls` (alias `Gold`), gagné
  aux kills (Tank 5 / Sniper 3 / Runner 2, `run.rs::obs_roguelite_enemy_death`), perdu à la
  mort → puits parfait, zéro inflation méta.
- **Station** : co-localisée au **marchand** (`merchant.rs`), qui EST « LE FORGERON
  ITINÉRANT » — diégétiquement le bon endroit pour tremper. Réutilise sa présence physique
  et sa proximité (`MerchantStats.near_player`), pas de nouveau GLB/spawn. Touche E (le
  marchand utilise 1-9). *Station Enclume dédiée en salle Rest = incrément futur (RunGraph).*

## Fichiers

- `crates/forgia-mode-roguelite/src/trempe.rs` (nouveau) : config genome hot-reload,
  `WeaponTrempeState` (level/damage_mul/or_spent, per-run), input E, panneau egui, sensor
  `forgia2_trempe.json`, tests purs.
- `crates/forgia-mode-roguelite/src/boons_apply.rs` : +1 param + `*= trempe.damage_mul`.
- `crates/forgia-mode-roguelite/src/lib.rs` : `pub mod trempe;` + plugin.
- `assets/genomes/roguelite/roguelite_progression.toml` (nouveau) : `[trempe]`.

## Amendements (retour Antoine 2026-07-02)

- **Trempe PAR ARME** : `WeaponTrempeState.levels: HashMap<WeaponType, u32>` (chaque arme
  se forge indépendamment). `damage_mul` = niveau de l'arme ÉQUIPÉE, resync chaque frame
  (`sys_sync_trempe_current`) → changer d'arme applique SA trempe (arme non trempée = ×1.0).
- **HUD sans superposition** : panneau Trempe ancré **bas-gauche** (`LEFT_BOTTOM`) — le
  marchand est bas-centre, les munitions + « ⇧ DASH » bas-droite. Titre = nom de l'arme
  courante (`WeaponType::display_name`).

## Design (cibles — valeurs en genome, hot-reload)

- `damage_per_level` = 0.15 (+15 %/niveau, multiplicatif) · `level_cap` = 5 (→ ×2.01 au max).
- `cost_base` = 20 Or · `cost_growth` = 1.4 → coûts 20/28/39/55/77 (total ~219). Premier
  palier atteignable en ~4-10 kills = testable vite. ⚠️ Le revenu Or (~150/run) < coût total
  des 5 trempes → arbitrage réel avec le marchand (heal/ammo), et un futur bump du revenu Or
  se calibrera sur le sensor `or_spent`.

## Hors scope (incréments suivants, notés)

- Gravures aux paliers 3/5 (M4) · scaling ennemi par depth (M1, donne son « intérêt » plein
  sens à la montée) · station Enclume dédiée en salle Rest (dépend RunGraph M2) · VFX
  rougeoiement + bark d'arme à la trempe (audio armes déféré par Antoine).

## Acceptance criteria

- [x] Test pur : `cost_for_next` croît géométriquement ; `damage_mul_for_level` = (1 +
      level × per_level) ; parse TOML + clamp + fallback Default ; reset neutre ; severity.
- [ ] Runtime : près du marchand, E dépense l'Or, le niveau monte, les damage numbers
      augmentent ; `forgia2_trempe.json` montre level/damage_mul/or_spent cohérents.
- [x] `PlayerCombatMods.damage_mul` reçoit `× trempe.damage_mul` sur la même ligne que
      perm/mastery (un seul writer, pas d'écrasement) — `boons_apply.rs`.
- [x] `cargo check` vert + clippy 0 warning fichiers touchés + 260 tests verts (+7 Trempe).

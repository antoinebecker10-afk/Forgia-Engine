# Story-589 — Phase B : progression d'élément (déblocage au portail)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_elements.json`, fichier `element_vfx.rs`, symbole `sys_reset_element_unlocks`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : EN COURS
**Niveau BMAD** : Standard (5-6 fichiers, 1 crate `forgia-mode-roguelite`)
**Date** : 2026-06-09
**Cible** : SHIP Roguelite — Phase B de [[project-roguelite-element-progression-design]]. Recherche+plan via workflow `roguelite-phase-b-design` (8 agents).

## Décision design (workflow + user)
- **Mécanique** : **déblocage progressif** (`Set<Element>` armé). Au portail de fin de zone, le choix 1-parmi-3 **arme un élément** (au lieu d'un boon). Le flag `always_on` (Phase A test) devient un **override dev** ; défaut **ship = progression**.
- **Dosage (user)** : **départ armé + 2 unlocks = 3/4 éléments par run**. Le Set est pré-rempli au reset avec l'élément de l'**arme de départ** (`EquippedWeapons.current`), puis 2 portails `Next` arment 2 de plus. 1 arme reste neutre = spécialisation.
- **Identité arme↔élément FIXE intacte** : `element_for(weapon)` (source unique) non touché.

## Plan (vérifié dans le code par le workflow)
| Fichier | Changement |
|---|---|
| `elements.rs` | `Element: Hash` ; `ElementUnlocks(HashSet<Element>)` Resource + helpers ; `sys_reset_element_unlocks` (OnEnter : départ armé OU 4 si always_on) ; gate apply = `is_unlocked` (au lieu de `!always_on return`) ; sensor + `unlocked` + severity ; `always_on` défaut → **false** |
| `roguelite_elements.toml` | `always_on = false` (ship progression) |
| `element_vfx.rs` | même gate `is_unlocked` (cohérence VFX↔apply obligatoire) |
| `loot_room.rs` | `ChoiceKind{Boon,Element}` + `ZoneReward.element_candidates` ; roll : si éléments verrouillés → offre éléments (sinon boons) ; pick : branche Element → `unlock` + TP |
| `hud.rs` | cartes colorées « ARME UN ÉLÉMENT » (couleur `Element::rgb`) selon `kind` |
| `lib.rs` | `init_resource::<ElementUnlocks>` + `sys_reset_element_unlocks` OnEnter |

Réutilisé tel quel : `ZoneReward`/`RewardPhase`/roll/pick/cartes, `roll_candidates` (boons quand tout armé), `GameSet.chain()` (pick Movement < apply Effects même frame), `element_for`/`Element::idx/rgb`.

## QA
- [x] `cargo check` + clippy 0 ; **99 tests verts** (2026-06-09)
- [x] Auto-QA verifier (mécanique) + qa-lead (0 bloquant ; D2 ordering = faux positif réfuté MessageReader multi-cast ; D3 prompt dynamique corrigé)
- [x] `forgia.exe` rebuild frais (18:22 ≥ sources)
- [ ] **Runtime** : run → départ avec 1 élément (arme de départ) actif ; portail z1→z2 → cartes « ARME UN ÉLÉMENT » colorées → pick → élément armé (VFX + dégâts) ; idem z2→z3 → 3 armés ; `forgia2_elements.json::unlocked`
- [ ] Dev : `always_on=1` (hot-reload) → 4 armés, portail repasse aux boons (story-585)

## Reste / suite
- [ ] Itération (B) : choix d'élément aussi au Coffre du Forgeron (waves.rs) pour 3-4/run
- [ ] Tiers/augments d'élément (post-ship)

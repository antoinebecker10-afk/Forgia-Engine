# story-697 — Les éléments s'appliquent, mais ne réagissent JAMAIS

**Statut** : DRAFT
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (moteur de réactions + VFX + audio, ≥ 2 crates)
**Origine** : run de validation du 2026-08-12, lecture des capteurs `elements` et
`element_vfx`.
**Bloque** : [story-611](story-611-reaction-combustion.md) (Combustion), et par
extension tout le pilier « réactions élémentaires ».

---

## Symptôme, mesuré

Sur une run complète, `forgia2_elements.json` :

| Ce qui marche | Ce qui ne marche pas |
|---|---|
| `burns_applied: 176` | `reactions.combustions: **0**` |
| `shocks_applied: 102` | `reactions.miasmas: **0**` |
| `poisons_applied: 1` | `reactions.surcharges: **0**` |
| `hits: {fire:192, shock:133, armor_pierce:13, poison:1}` | |
| `elem_absorbed: 3503` · `aoe_hits: 23` · `executes: 3` | |

Corroboré par `forgia2_element_vfx.json` : `sparks_spawned: 339`,
`dot_pulses: 66`, mais **`reaction_bursts: 0`**.

**Les quatre éléments sont débloqués** (`unlocked_count: 4`) et **s'appliquent
massivement**. C'est l'étage au-dessus — la table de réactions — qui ne produit
jamais rien.

## Pourquoi c'est grave au-delà du bug

Le GDD *The Spared* fait des **réactions inter-acteurs le cœur du coop** : *« l'un
applique, l'autre détone »*, et les ultimes y sont décrits comme « les applicateurs
d'élément de masse — le moment de spectacle ». Un moteur de réactions qui ne
déclenche jamais vide cette promesse.

C'est aussi la deuxième moitié d'un constat déjà connu : `forgia2_power.json`
donne `boon_damage_count: 0` (courbe de dégâts plate). Deux systèmes de montée en
puissance inertes en même temps.

## Pistes, à falsifier — ne pas patcher avant d'avoir mesuré

1. **Un seul élément par acteur à la fois ?** Le GDD dit qu'une grosse réaction
   « exige deux poseurs ». Si un ennemi ne peut porter qu'un statut, la condition
   à deux n'est jamais réunie en solo. Vérifier `active_burns` /`active_shocks` :
   ils sont à **0** au moment de l'écriture, ce qui n'exclut ni l'un ni l'autre.
2. **Le mapping arme→élément est-il trop uniforme ?** `pistol:shock, smg:fire,
   sniper:armor_pierce, pompe:poison` — avec une seule arme en main la plupart du
   temps, deux éléments distincts coexistent-ils jamais sur la même cible ?
3. **La `ReactionTable` est-elle consommée ?** Chercher son point d'appel réel,
   pas sa définition. **Grep le CONCEPT** (« reaction, combustion, detonate »),
   pas le nom du type — leçon de story-626 ce même jour.

## Critères d'acceptation

- [ ] La cause est **nommée et prouvée** par une mesure, pas déduite
- [ ] `reactions.combustions` > 0 après une run où feu et choc sont posés
- [ ] `element_vfx.reaction_bursts` > 0 en même temps (le visuel suit la logique)
- [ ] Un test couvre la condition de déclenchement, pas seulement l'application
- [ ] story-611 peut être validée ou infirmée sur pièces

## Cross-refs

- `reference_elemental_reaction_engine_and_shock` (mémoire) — ReactionTable/Shock/Miasma
- GDD *The Spared* §4 « Réactions inter-acteurs (le cœur coop) »
- `.claude/rules/no-speculative-fix.md` — trois pistes, aucune n'est encore une cause

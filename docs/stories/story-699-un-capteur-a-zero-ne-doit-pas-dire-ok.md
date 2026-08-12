# story-699 — Un capteur dont le compteur est à zéro ne doit pas dire « ok »

**Statut** : DRAFT
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (règle transverse à ~30 capteurs de feature)
**Origine** : tentative de tri automatique des 89 stories ouvertes. L'heuristique
« capteur `ok` ⇒ la feature marche » s'est **réfutée elle-même** en trois cas.

---

## Ce qui s'est passé

Pour trier 89 stories sans les lire une par une, on a croisé « le code existe »
avec « son capteur dit quoi ». Résultat : 30 stories avec un capteur `severity:
"ok"`, dont 6 semblaient fermables.

**Trois de ces six étaient déjà connues comme cassées le jour même** — stories
696, 697, 698, ouvertes une heure plus tôt sur preuve chiffrée :

| Story | Capteur | `severity` | Ce que dit vraiment le contenu |
|---|---|---|---|
| 648 hitstop | `forgia2_gamefeel.json` | **ok** | `hitstop_counts` = `{hit:0, crit:0, kill:0, multikill:0}` |
| 652 VFX kill | `forgia2_weapon_vfx.json` | **ok** | `kill_bursts: 0` malgré 51 kills |
| 611 combustion | `forgia2_elements.json` | **ok** | `reactions: {combustions:0, miasmas:0, surcharges:0}` |

**Le capteur dit « ok » parce que rien n'a échoué. Or rien ne s'est produit non
plus.** Un système inerte ne lève aucune erreur : c'est précisément ce qui le rend
invisible.

## La règle manquante

`map-design-patterns.md` §13 l'énonce déjà pour la géométrie :

> **Zéro mesuré n'est pas vert, c'est aveugle.** Tout contrôle expose la taille de
> son échantillon. Un seuil qui n'a rien à mesurer renvoie `info` + « aveugle »,
> jamais `ok`.

Elle n'a jamais été appliquée aux capteurs de **feature**. `stage_layout` la
respecte (« AVEUGLE : le capteur ne dit PAS que l'arène est vide, il dit qu'il n'a
rien vu ») — les autres non.

## Ce qu'il faut, et ce qu'il ne faut pas

**Il ne faut pas** passer tous les compteurs à zéro en `warn` : au menu, `hitstop`
à 0 est normal, et un chien qui crie au loup finit ignoré (leçon du chien de garde
étendu le même jour, cf `sensor_health_sensor.rs`).

**Il faut** que la sévérité tienne compte du **contexte d'attente** : un compteur
de combat à zéro **pendant un combat** est un défaut ; le même à zéro au menu ne
dit rien. La distinction existe déjà ailleurs : `severity_for_render` ne crie que
si `camera_present`, et `severity_for_spawn_budget` distingue le pic de la
saturation durable.

Forme cible, à décliner par capteur :

```
si (le système est censé tourner) et (compteur == 0 depuis N s) -> "warn" + next_step
si (le système n'est pas censé tourner)                          -> "info" + « aveugle »
sinon                                                             -> "ok"
```

## Portée

~30 capteurs de feature ont aujourd'hui une sévérité qui ne regarde que les
erreurs, jamais l'inactivité. Trois sont déjà prouvés menteurs. Les autres n'ont
pas été audités — **ce chiffre est un plancher, pas un total**.

## Pourquoi ça bloque autre chose

Le tri des 89 stories ouvertes ne peut pas être automatisé tant que ce défaut
existe : **80 d'entre elles ont du code vivant**, et le seul signal mécanique
disponible pour distinguer « livré et fonctionnel » de « livré et inerte » est
justement la sévérité du capteur. Tant qu'elle ment, chaque story doit être
vérifiée à la main.

Corriger ce point rend le tri de masse possible **et** attrape les régressions
futures sans que personne ne regarde.

## Critères d'acceptation

- [ ] Les ~30 capteurs de feature sont **inventoriés** avec, pour chacun, le
      compteur qui prouve son activité et la condition « censé tourner »
- [ ] Un helper partagé porte la règle, plutôt qu'une re-implémentation par capteur
- [ ] `gamefeel`, `weapon_vfx`, `elements` passent en `warn` quand leur compteur
      reste à 0 en combat — vérifié sur une run réelle
- [ ] Aucun `warn` au menu ou hors du contexte d'attente (pas de faux positif)
- [ ] Un test par capteur corrigé couvre les deux sens : inactif-en-contexte →
      `warn`, inactif-hors-contexte → `info`

## Cross-refs

- stories **696** (hitstop), **697** (réactions), **698** (kill burst) — les trois
  défauts que cette cécité a laissés passer
- `map-design-patterns.md` §13-14 · `.claude/rules/observability-required.md`
- `crates/forgia-observability/src/sensor_health_sensor.rs` — le chien de garde
  traite la **fraîcheur** ; celle-ci traite la **vacuité**. Les deux sont
  nécessaires : un capteur peut être frais ET vide.

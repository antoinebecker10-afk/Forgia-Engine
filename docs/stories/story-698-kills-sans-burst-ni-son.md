# story-698 — 51 kills, 0 burst visuel et 2 sons de mort

**Statut** : DRAFT
**Créée** : 2026-08-12
**Niveau BMAD** : Quick (deux compteurs à zéro, même cause probable : l'événement de mort)
**Origine** : run de validation du 2026-08-12, recoupement de trois capteurs.
**Bloque** : [story-652](story-652-vfx-visibles-multiplicateurs-genome.md) (VFX
visibles / kill burst), et le chantier « kill satisfaisant » de la ROADMAP.

---

## Symptôme, mesuré — trois capteurs qui se contredisent

| Capteur | Champ | Valeur |
|---|---|---|
| `forgia2_knockback.json` | `kill_pushes` | **51** |
| `forgia2_elements.json` | `executes` | 3 |
| `forgia2_weapon_vfx.json` | `kill_bursts` | **0** |
| `forgia2_roguelite_audio.json` | `kills` | **2** |

**51 morts d'ennemis produisent 0 burst visuel et 2 sons.** Le knockback, lui,
sait qu'il y a eu 51 kills — donc l'information « cet ennemi meurt » existe bien
quelque part et arrive à au moins un consommateur.

Les multiplicateurs VFX sont pourtant chargés et généreux :
`size_mult: 5.00`, `count_mult: 3.00`, `lifetime_mult: 1.60`. Ce n'est donc pas
un réglage trop discret — **rien ne part**.

## L'hypothèse la plus économique

Un seul producteur d'événement de mort, plusieurs consommateurs, et deux d'entre
eux ne sont pas branchés — ou sont branchés sur un chemin de mort différent de
celui qu'emprunte réellement le knockback. Deux compteurs à zéro **en même temps**
sur le même déclencheur pointent vers la source commune, pas vers deux bugs
indépendants.

À falsifier avant de coder : il se peut aussi que `kill_pushes` compte autre chose
qu'une mort (un hit létal *potentiel*, un coup sur un ennemi déjà mourant). Dans ce
cas le vrai nombre de morts serait proche de 2, et le défaut serait dans le
compteur de knockback. **Commencer par établir combien d'ennemis sont réellement
morts** — `forgia2_roguelite_state.json` et le killfeed le savent.

## Pourquoi ça compte

La ROADMAP décrit le « kill satisfaisant (mort en 4 temps) » comme *« le dernier
gros morceau du game-feel »*, avec ses ingrédients déjà livrés (stories 648/650/655)
et « il ne reste que l'assemblage ». Ce ticket montre que l'assemblage ne tient
pas : sur les quatre temps annoncés — anticipation, burst, débris, permanence —
le burst ne part pas et le son non plus.

## Critères d'acceptation

- [ ] Le **nombre réel de morts** sur une run est établi et recoupé sur 2 capteurs
- [ ] La cause commune est nommée (un producteur, des consommateurs débranchés) ou
      réfutée (les compteurs ne comptent pas la même chose)
- [ ] `weapon_vfx.kill_bursts` ≈ nombre de morts après correction
- [ ] `roguelite_audio.kills` ≈ nombre de morts après correction
- [ ] story-652 peut être validée ou infirmée sur pièces

## Cross-refs

- ROADMAP « Kill satisfaisant (mort en 4 temps) », statut `READY`
- `feedback_les_agregats_cachent_la_chronologie_tranche` (mémoire) — un compte
  n'est une preuve que si on sait ce qu'il compte. C'est exactement le doute ici.

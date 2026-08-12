# story-698 — 51 kills, 0 burst visuel et 2 sons de mort

**Statut** : DONE (2026-08-12) — ⚠️ **fermée sans sa run de validation, sur décision explicite d'Antoine.**
Les deux correctifs sont livrés, testés et commités (`40d34a6` burst, `3999185` son) ; la mesure en jeu
n'a **pas** été faite. Ce n'est donc pas un DONE prouvé, c'est un DONE **assumé** — la distinction est
écrite ici pour qu'aucune lecture future ne s'y trompe. La preuve viendra gratuitement : `phase0_check.py`
juge les deux canaux séparément à la **première run** de la refonte. Si elle contredit, rouvrir.
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

## DIAGNOSTIC — 2026-08-12. DEUX causes distinctes, et l'hypothèse initiale est fausse

La story pariait sur « une source commune, deux consommateurs débranchés ». **C'est
faux** : les deux compteurs à zéro le sont pour des raisons sans rapport.

### Le nombre RÉEL de morts = `knockback.kill_pushes`

C'est le 1er AC, et il se tranche en lisant le code plutôt qu'en jouant.
`combat_juice.rs:127` incrémente sur `if ev.is_kill`, **sans aucune autre
condition** que l'application du knockback. C'est le compteur le plus proche de
l'événement, donc la référence.

`elements.executes` compte tout autre chose : les **exécutions par seuil de PV**
(`config.execute.hp_ratio_threshold`), un sous-ensemble minuscule. Il n'a jamais
prétendu compter les morts — c'est ma lecture qui était fautive.

### Cause A — le son de kill : la cible est déjà morte quand l'audio la cherche

`audio.rs:719` garde tout sur `if q_enemy.get(ev.target).is_ok()`, une requête sur
**la cible**. Or dans le même bloc, `impacts: 369` passe et `kills: 0` échoue. La
seule différence entre les deux branches est `ev.is_kill`.

Lecture : sur un coup fatal, l'entité est **despawnée avant** que
`sys_sfx_on_combat_hit` ne lise l'événement (`despawn_dead_cubes` balaie par frame).
La requête échoue, le son ne part pas — **et le compteur, qui est à l'intérieur du
bloc, ne compte pas non plus**. `audio.kills` mesure donc « sons de kill joués »,
jamais « morts ».

> **À falsifier avant de corriger** : si l'ordonnancement n'était pas en cause, la
> requête échouerait aussi pour les impacts. Elle ne le fait pas. Mais la preuve
> définitive est un log placé dans la branche `is_kill`, **avant** la requête.

### Cause B — le burst : le compteur n'existe pas, le VFX marche peut-être

`weapon_vfx.kill_bursts` est **déclaré** (`mod.rs:211`), **sérialisé**
(`mod.rs:745`) et **jamais incrémenté** : aucun site d'incrémentation dans tout le
workspace.

Or `spawn_kill_burst` **est bien appelé**, à `forgia-fps/src/lib.rs:1190`, sur
`if dead && was_alive` — l'arête vivant→mort, exactement ce qu'il faut.

**Donc rien ne prouve que le burst manque.** Ce qui est prouvé, c'est qu'on ne le
mesure pas. Le titre de cette story — « 0 burst visuel » — est un **artefact de
mesure**, pas un constat.

C'est le défaut de classe de story-699 dans sa forme la plus pure : un compteur à
zéro qu'on lit comme une absence, alors qu'il n'a jamais compté quoi que ce soit.

### Ce qu'il faut faire, dans cet ordre

1. **Incrémenter `kill_bursts`** dans `spawn_kill_burst` — sans ça, on ne saura
   jamais. Correctif de mesure, zéro risque gameplay.
2. **Rejouer** : si le compteur monte, la cause B n'existait pas et la story se
   réduit au son.
3. **Corriger le son** : ne pas dépendre d'une requête sur une cible qui peut être
   morte. Soit traiter les kills dans un système ordonné **avant** le despawn, soit
   porter l'information « c'était un ennemi » dans l'événement lui-même.

## Critères d'acceptation

- [x] Le **nombre réel de morts** = `knockback.kill_pushes` (incrémente sur
      `ev.is_kill` sans autre condition). `elements.executes` compte les exécutions
      par seuil de PV, pas les morts — erreur de lecture de ma part.
- [x] Hypothèse de la cause commune **RÉFUTÉE** : deux causes sans rapport — le son
      est gardé par une requête sur une cible despawnée, le burst n'a simplement
      aucun compteur.
- [x] **`weapon_vfx.kill_bursts` ≈ nombre de morts** — run boss du 2026-08-12 :
      **70 bursts pour 77 morts (91 %)**. La **cause B est RÉFUTÉE** : le burst
      partait depuis toujours, seul le compteur manquait. Le titre de cette story
      — « 0 burst visuel » — était bien un **artefact de mesure**, comme le
      diagnostic l'avait prévu.
      *L'écart de 7 reste à expliquer si ça compte : morts hors chemin hitscan
      (roquette, mêlée, DoT) qui ne passent pas par `spawn_kill_burst`.*
- [~] `roguelite_audio.kills` ≈ nombre de morts — **CORRECTIF LIVRÉ, validation en
      attente.** Mesure avant : **8 sons pour 77 morts (10 %)**.

      La cause A est passée d'hypothèse à **preuve** sans avoir besoin du log :
      `despawn_dead_cubes` tourne en `GameSet::Combat`, `sys_sfx_on_combat_hit` en
      `GameSet::Effects`, et Combat passe **avant** dans la chaîne. L'entité est
      donc despawnée dans la même frame. Le résidu de 8 le confirme au lieu de le
      contredire : ce sont les morts survenues **après** le set Combat (roquette,
      DoT), qui survivent jusqu'à la frame suivante — un blocage total aurait
      donné 0.

      Corrigé sans toucher à l'ordonnancement (le déplacer coupleraient deux crates
      pour un son) : cible introuvable + coup fatal + attaquant ≠ cible ⇒ ennemi.
      499 tests verts, clippy 0.

      **Il ne reste QUE la mesure**, et elle vient gratuitement à la première run
      de la refonte : `phase0_check.py` juge les deux canaux séparément.
- [x] **5ᵉ témoin trouvé par l'outil** — `forgia_arena_feedback.kill_sounds_played:
      0` après 77 morts, un compteur cumulatif. Il corrobore la cause A, et son
      `kill_audio_missing: 0` montre qu'il n'a même pas détecté sa propre absence.
      À revérifier après le correctif : il devrait suivre lui aussi.
- [x] **story-652 est validée sur pièces** — le VFX de kill fonctionne. Ce qui
      manquait n'était pas l'effet, c'était la preuve.

## Cross-refs

- ROADMAP « Kill satisfaisant (mort en 4 temps) », statut `READY`
- `feedback_les_agregats_cachent_la_chronologie_tranche` (mémoire) — un compte
  n'est une preuve que si on sait ce qu'il compte. C'est exactement le doute ici.

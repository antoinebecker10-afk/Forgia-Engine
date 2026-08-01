# story-677 — La boucle de rounds et son mur

**Statut** : IN_PROGRESS (livré, runtime non validé)
**Date** : 2026-08-01
**Niveau BMAD** : Standard
**Related** : story-676 (univers d'arène), story-669 (composition de vague), story-658 (scaling)

---

## L'intention

Des arènes qui s'enchaînent — **pas de carte, pas de choix de porte**. Round 1,
round 2, … La difficulté monte, et elle monte plus vite que ce que le joueur
gagne s'il ne fait rien : passer un round exige de monter ses perfs.

---

## Pourquoi la courbe change

L'ancien scaling était **linéaire** : `1 + round × 0,35`
(`roguelite_progression.toml [scaling]`).

Une droite finit toujours par se faire rattraper par un joueur qui compose ses
multiplicateurs. La Trempe seule vaut **×2,01** (5 paliers de +15 %,
multiplicatifs) et rattrape un +35 %/round vers le round 3. Passé là, la pression
s'évapore — et c'est exactement ce que le rapport du 2026-07-31 pointait :
*« 100 % de la difficulté est statistique »*, sans que cette statistique tienne.

Maintenant : **géométrique + paliers**.

```
menace_pv(r) = 1,16^r × 1,22^floor(r / 3)
```

La croissance continue tient la pression sur toute la montée ; les paliers
donnent le sursaut qu'on **ressent**. Gunfire Reborn monte par paliers, Risk of
Rain 2 en continu — on prend les deux. Les dégâts montent plus doucement (1,07)
que la vie : un round qui tue en deux balles n'est pas dur, il est injuste. La
difficulté se paie en temps de tir.

---

## Le mur se CALCULE

Un round se passe si la vague est nettoyée dans le budget de temps :

```
ttk(r) = pv_vague(r) / (dps_base × efficacité × puissance(r))
mur    = premier r où ttk(r) > budget_temps_round_s
```

**Les entrées sont mesurées, pas choisies :**

| Entrée | Valeur | D'où elle vient |
|---|---|---|
| `dps_reference` | **168** | Pépin, `viewmodel_arena.toml` |
| `pv_vague_reference` | **1355** | 2 vagues réelles : 7 tank × 120 + 7 runner × 35 + 6 sniper × 45 (`roguelite_waves.toml` + `roguelite_enemies.toml`) |
| `efficacite_tir` | 0,35 | déplacement, rechargement, visée, approche |
| `budget_temps_round_s` | 90 | durée visée d'un round à 2 vagues |

**Le facteur d'efficacité n'était pas dans ma première version, et le test l'a
attrapé** : sans lui le modèle prétendait qu'un round se nettoie en 8 s, et le
mur tombait au round 19 — c'est-à-dire jamais ressenti. Un FPS ne délivre pas son
DPS théorique ; l'ignorer rendait tout le modèle faux.

### Résultat mesuré

```
mur : round 7 sans progression, round 15 en prenant tout
```

L'écart **7 → 15** est la valeur de la progression, et il est vérifié par un
test qui échoue si on le laisse se refermer. Sans cet écart, monter ses perfs
serait décoratif et la boucle ne serait qu'un compte à rebours.

---

## Ce qui est livré

- **`roguelite_rounds.toml`** — courbe, mur, rythme des récompenses. Miroir Rust
  exact, vérifié par test (`the_shipped_genome_matches_the_rust_mirror`).
- **`rounds.rs`** — `threat(round)`, `time_to_clear`, `wall_round`, `margin`,
  plus le rythme (`is_tier_round`, `is_respite_round`, `grants_equipment`).
- **Une seule courbe s'applique.** `enemy_scaling` délègue à `rounds::threat()`
  quand la boucle est active ; le linéaire reste le chemin d'avant si
  `[boucle] enabled = false`. Deux sources de difficulté, c'est une balance
  qu'on ne sait plus lire.
- **Plus de choix de porte en boucle** : le graphe n'est pas consulté, les arènes
  s'enchaînent. C'est ce que « on ne branche pas le parcours » veut dire.
- **Le type de salle vient du rythme déclaré** (respiration tous les 5 rounds),
  pas d'un nœud de graphe.
- **Capteur `forgia2_rounds.json`** — round, menace, ttk, budget, **marge**, et
  les deux murs. La marge est la lecture qui compte : négative = le mur est
  franchi. Severity `error` si le mur est franchi *même en prenant tout* — ça,
  c'est un défaut de balance, pas une intention.

---

## Acceptance criteria

- [x] La menace croît **strictement** à chaque round (test sur 60 rounds) —
      un plateau est un round gratuit, défaut déjà payé sur le budget du directeur
- [x] Le palier est une **marche**, pas une pente (test : saut > 1,15 × le pas ordinaire)
- [x] Le mur existe **et** la progression le repousse d'au moins ×2 (test)
- [x] Le mur sans progression tombe **tôt** (rounds 2-8) — la pression doit se sentir
- [x] Les premiers rounds passent **sans aucune amélioration**
- [x] Un génome hostile ne peut pas produire une courbe décroissante ni une
      division par zéro (test)
- [x] Une seule courbe de difficulté s'applique à la fois
- [x] Capteur avec severity + next-step actionnable
- [ ] **Validé en jeu** — non fait

---

## Ce que ça ne fait PAS, et qu'il faut dire

- **Le boss n'est plus dans la boucle.** En mode infini (`max_rounds = 0`) il n'y
  a pas de round de boss : le boss met fin à la run dans le flux actuel
  (boss → salle de butin → Victoire), ce qui contredit une boucle sans fin. Le
  recâbler en **jalon périodique** (boss tous les 10 rounds, la run continue)
  touche `boss_portal` et `loot_room` — c'est la suite, pas cette story.
- **Le round 255 est un plafond dur** : `wave.stage` est un `u8`. Ce n'est pas un
  choix de design, c'est la représentation. `saturating_add` évite l'overflow et
  le round 255 scelle la run.
> **Mise à jour story-679** — l'indicateur écran ne dépend plus d'aucune des deux
> estimations ci-dessous : il affiche le **temps de combat mesuré** face au budget,
> qui est exactement la grandeur qui définit le mur. Les estimations ne servent plus
> qu'à *prédire* le mur (`wall_lazy` / `wall_full` du capteur). Et comme le capteur
> porte maintenant les deux — la prédiction et la mesure — on peut enfin calibrer
> l'une sur l'autre au lieu de la deviner.

- **`gain_puissance_par_round = 0,34` est une estimation**, pas une mesure. Les
  vraies sources de puissance (boons, équipement story-675, Trempe) n'exposent
  pas de total agrégé. Tant que ce n'est pas mesuré, le mur « en prenant tout »
  est un ordre de grandeur, pas un chiffre. Le mur « sans progression », lui, est
  exact — il ne dépend d'aucune estimation.
- **`efficacite_tir = 0,35` est une hypothèse.** Elle déplace les deux murs
  proportionnellement. À mesurer en jeu (temps de tir effectif / durée de round)
  avant de régler la courbe finement.
- **Rien ne vérifie que la récompense suit.** `boon_par_round` et
  `equipement_tous_les` décrivent le rythme visé ; le câblage de la distribution
  effective est le flux existant, non modifié ici.

---

## Cross-refs

- `docs/design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md` — le rapport qui
  a nommé le problème (« la run est un couloir », difficulté 100 % statistique)
- `.claude/rules/map-design-intention.md` §4.1 — le rythme a besoin de relâche
- `.claude/rules/observability-required.md` — le capteur et son next-step
- story-676 — les univers que la boucle traverse

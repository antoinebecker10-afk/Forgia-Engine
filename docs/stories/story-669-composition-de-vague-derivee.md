# story-669 — La composition de vague redevient une dérivation

**Statut** : IN_PROGRESS (implémentation livrée, validation runtime en attente)
**Niveau BMAD** : Standard (1 nouveau module + 1 genome + 4 fichiers touchés, 1 crate)
**Date** : 2026-07-31
**Related** : [rapport des boucles](../design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md), story-668 (Vague 0), story-646 (multi-salles + choix de porte), story-470 (StageGraph)

---

## Demande

> « ok pour ta recommandation » — point 3 : *changer la signature de
> `wave_composition`, c'est ouvrir quatre ruptures d'un coup.*

---

## Le constat : quatre symptômes, une cause

L'audit du 2026-07-31 a listé 22 ruptures de la boucle roguelite. Quatre des plus
graves partageaient **une seule cause** — une fonction qui ne recevait pas ses
paramètres :

```rust
pub fn wave_composition(wave: u8) -> Vec<(EnemyArchetype, u32, f32)>
```

Ni `stage`, ni `kind`, ni `seed`. En cascade :

| Symptôme | Ce qui manquait |
|---|---|
| les 3 salles de combat rejouaient les mêmes 8 puis 12 ennemis | `stage` |
| le choix de porte ne changeait rien (`room_kind` écrit, jamais relu) | `kind` |
| les positions de spawn étaient figées, mémorisables en 2 runs | `seed` |
| la difficulté ne montait que par les PV | `difficulty_budget`, calculé puis jeté |

C'est exactement le motif que le feedback du 2026-07-29 demande de traiter :
**au 3ᵉ défaut semblable, chercher la CLASSE, pas le symptôme.** Ici la classe
était visible — un littéral de trop et trois paramètres manquants.

---

## Ce qui est livré

### 1. La composition passe en couche definition

Nouveau genome `assets/genomes/roguelite/roguelite_waves.toml` (hot-reload 1 Hz,
miroir Rust exact) et nouveau module `crates/forgia-mode-roguelite/src/wave_comp.rs`.

```text
count(archétype) = round( base × densité(salle) × modificateur(type de salle) )
densité(salle)   = budget_director(salle) / budget_director(0)
```

**Aucune valeur n'est déclarée deux fois** : les anneaux de la vague 2 se dérivent
de ceux de la vague 1 par `wave2_bonus_m`, au lieu d'être une seconde table.

### 2. Le choix de porte change enfin le combat

`RogueliteWave.room_kind` est **consommé**. Six profils, en TOML :

| Type de salle | Profil | Intention |
|---|---|---|
| **Combat** | 1.0 / 1.0 / 1.0 | la référence, l'équilibre historique |
| **Élite** | tank ×1.7, runner ×0.4 | un mur qui avance — ça se joue au repositionnement |
| **Événement** | runner ×1.8, tank ×0.5 | l'essaim — ça se joue au contrôle et au recul |
| **Boutique** | ×0.5 partout | combat léger, puis on dépense son Or |
| **Repos** | ×0.3 partout | la respiration |
| **Trésor** | sniper ×1.6, contact ×0.6 | gardé de loin — aller au coffre coûte de s'exposer |

### 3. Le `difficulty_budget` a enfin un lecteur

`StageNode.difficulty_budget` était calculé à chaque run depuis story-470 et
**jeté** (zéro lecteur dans tout le workspace). Il est désormais capturé sur le
nœud **réellement choisi** (`RogueliteWave.room_budget`) et pilote la densité.

Avec les gènes par défaut (`base 2.0`, `×1.25/salle`) : densité **×1.00 → ×1.25 →
×1.56** sur les 3 salles de combat. Un levier de difficulté par la **composition**,
à côté du multiplicateur de PV — ce que le Pacte de Hades fait à 11 conditions sur 15.

### 4. La graine de run atteint enfin le placement

Le placement dérivait de `WAVE_BASE_SEED ^ wave`, une **constante**. Il dérive
maintenant de `run_seed × salle × vague`, plus une dispersion de rayon
(`ring.jitter_m = 2.0`). Deux runs ne sont plus superposables.

### 5. Le TODO « refactor abandonné » est levé

`run.rs` spawnait la salle 0 via un fallback commenté
`TODO(story-471..479): API removed, refactor abandonné`. La salle 0 tire de nouveau
son type et son budget du graph.

---

## Garde-fous

| Garde | Pourquoi |
|---|---|
| **Total ≥ 1 ennemi, toujours** | une vague vide ne poserait jamais `seen_alive` → la salle ne se nettoierait jamais → **run figée**. Le code l'impose quels que soient les nombres du TOML. |
| **`density.max_factor = 2.5`** | borne le budget de frame (spawn + IA + raycasts LOS). |
| **Le boss n'est ni densifié ni modulé** | il est unique par construction ; 0 boss = run sans fin. |
| **`kind = None` → neutre** | graph absent ⇒ équilibre de référence, jamais de dérive silencieuse. |
| **Genome partiel → miroir Rust par champ** | et tout fallback `warn!` (leçon de l'auto-QA de story-668). |

### Plafond Bevy

`sys_start_run` était **déjà à 16 SystemParams**. Le nouveau `WaveSpawnConfigs`
(`SystemParam` groupant stats + défense + composition) libère la place — c'est le
remède que prescrit `scalability.md` (« bundle quand > 12 params »).

---

## Fichiers touchés

| Fichier | Nature |
|---|---|
| `assets/genomes/roguelite/roguelite_waves.toml` | **definition** — nouveau |
| `crates/forgia-mode-roguelite/src/wave_comp.rs` | **framework** — nouveau module (dérivation pure + hot-reload) |
| `crates/forgia-mode-roguelite/src/waves.rs` | `WaveSpawnConfigs`/`WaveSpawnCtx`, `room_budget`, `node_budget`, placement seedé |
| `crates/forgia-mode-roguelite/src/run.rs` | salle 0 tirée du graph (TODO levé), bundle SystemParam |
| `crates/forgia-mode-roguelite/src/sensor.rs` | `room_kind`, `room_budget`, `room_density` |
| `crates/forgia-mode-roguelite/src/lib.rs` | module + 2 systèmes (Startup + hot-reload) |

## Critères d'acceptation

- [x] `wave_composition(wave)` supprimée — la composition reçoit salle, type et graine
- [x] `room_kind` **consommé** (il n'était lu que par un `info!`)
- [x] `StageNode.difficulty_budget` a un lecteur pour la première fois
- [x] Le placement dérive de la graine de RUN, plus d'une constante
- [x] Équilibre de référence **inchangé** (salle 0, Combat, densité 1.0 → 8 puis 12)
- [x] Invariant de non-blocage : une vague ne peut jamais être vide
- [x] Densité bornée (`max_factor`) — budget de frame protégé
- [x] Genome hot-reload + fallback par champ + `warn!` sur tout repli
- [x] Observable : `room_kind` / `room_budget` / `room_density` dans le capteur
- [x] `cargo check --workspace` vert
- [x] `cargo clippy -p forgia-mode-roguelite --all-targets -- -D warnings` : **0 warning**
- [x] `cargo test -p forgia-mode-roguelite --lib` : **294 passed, 0 failed** (+10)
- [x] `xtask sensor-audit` : **124/124**
- [ ] **Validation runtime** — ⚠️ **PARTIELLE, et le récap ci-dessous est PÉRIMÉ.**
      - ✅ Vérifié en jeu le 2026-08-12 : progression de salle et densité dérivée —
        log `SALLE 2/4 (Some(Combat))`, `forgia2_roguelite_state.json` →
        `room_density: 1.25`, `room_kind: "combat"`.
      - ❌ **Invérifiable en l'état** : le récap demande de comparer une porte
        **Élite** à une porte **Événement**. Antoine (2026-08-12) : *« Élite vs
        Événement n'existe plus »* — le choix n'est plus atteignable en jeu.
      - 🔎 **Écart à instruire, ne pas refermer en silence** : `StageKind::Elite` et
        `::Event` existent toujours (`forgia-stage/src/graph.rs:64`),
        `generate_run_graph` tire encore des kinds variés par depth, `hud.rs:904`
        mappe encore leurs icônes, et `wave_comp.rs:330` a encore leurs
        multiplicateurs. Mais les portails du mode sont `PortalKind::Enter/Next/
        Return` (`loot_room.rs:89`) — **linéaires, sans choix de kind**. Donc soit
        du contenu mort, soit un consommateur débranché.
      - **Ne PAS cocher cet AC en réécrivant le récap** : ce serait la DONE fictive
        que story-495 et `story-done-gate.md` existent pour empêcher. La suite est
        une story de diagnostic sur le portail de choix (cf story-646 Inc.2, donnée
        pour livrée), pas une retouche de critère.

## Tests (10 nouveaux)

| Test | Ce qu'il prouve |
|---|---|
| `defaults_reproduce_the_historical_table` | l'équilibre de référence n'a pas bougé (8 / 12 / 5) |
| `the_door_choice_now_changes_the_fight` | Combat ≠ Élite ≠ Événement, et Élite a plus de tanks, Événement plus de runners |
| `density_grows_with_depth_and_is_bounded` | ×1 → ×1.25 → ×1.56, puis plafonné |
| `density_disabled_pins_every_room_to_the_reference` | l'interrupteur du genome marche |
| `a_wave_can_never_be_empty_whatever_the_genome_says` | **invariant anti-run-figée**, même avec des modificateurs à 0 |
| `the_boss_is_never_scaled_nor_modulated` | 1 boss, quelles que soient densité et porte |
| `an_unknown_or_absent_kind_stays_neutral` | graph absent → pas de dérive silencieuse |
| `a_partial_genome_keeps_the_rust_mirror_for_the_rest` | fallback par champ |
| `the_real_genome_file_parses_and_keeps_the_reference_balance` | le TOML livré **est** le miroir du Rust (et le test ne peut pas se sauter en silence) |
| `wave_two_rings_are_wider` | les anneaux de la vague 2 sont dérivés, pas déclarés |

Test **modifié** : `qa_wave_soak_keeps_combat_entities_bounded_across_rooms`
n'assertionne plus « 8 ennemis par salle » (l'effectif varie désormais, c'est le
but) mais son invariant réel : **1 visuel par bot** (pas de fuite), effectif non
vide et borné.

---

## Correctif post-runtime — le budget était quantifié trop grossièrement

**Trouvé en jeu**, sur la première run réelle (capteur du 2026-07-31 18:07) :

```
room: 2   room_kind: "event"   room_budget: 3   room_density: 1.50
```

`room_density` valait **1,50 en salle 3**, alors que cette story annonçait 1,56.
En remontant : `director_budget_for_depth` renvoyait des **crédits entiers**
(`u32` arrondi), soit `2 / 3 / 3 / 4` — la courbe réelle était donc :

| Salle | Densité **avant** | Densité **après** |
|---|---|---|
| 1 | ×1,00 | ×1,00 |
| 2 | ×1,50 | ×1,25 |
| 3 | **×1,50** ← plateau | ×1,57 |
| boss | ×2,00 | ×1,96 |

Deux défauts : un saut de +50 % en salle 2 au lieu de +25 %, et surtout **les
salles 2 et 3 avaient exactement la même densité**. La moitié de la montée en
difficulté que cette story était censée apporter était annulée par un arrondi.

**Correctif** : le budget est stocké en **centi-crédits** (`DIRECTOR_BUDGET_SCALE
= 100`) → `200 / 250 / 313 / 391`. Le champ est renommé
`StageNode.difficulty_budget_centi` : l'unité est dans le nom, parce qu'une unité
implicite est exactement le piège qui a déjà coûté une passe sur
`damage_per_level` (commentaire en %, valeur en fraction). Les gènes créateur
restent en crédits — l'échelle est une résolution de champ, pas un slider.

**Pourquoi le test ne l'avait pas vu** : `director_budget_grows_with_depth`
assertait `b1 >= b0` — un `>=` tolère le plateau, donc ne mesure pas la
croissance. Remplacé par une croissance **strictement** monotone sur toutes les
profondeurs consécutives, plus un test qui vérifie que les rapports suivent bien
la courbe géométrique des gènes (1,25 / 1,5625 / 1,9531).

Le capteur expose désormais `room_budget_credits` (en crédits, avec l'unité dans
le nom) au lieu de `room_budget`.

---

## Ce que cette story ne fait PAS

- **Aucun nouvel archétype.** « Élite » module les archétypes existants ; il n'y a
  toujours ni ennemi d'élite, ni soigneur (le levier le moins cher du marché, cf
  le Grentin de High on Life dans le rapport).
- **Les salles non-combat n'existent toujours pas spatialement** : « Boutique » et
  « Repos » ne changent que la composition, pas la scène. `stage_id_for_depth`
  reste piloté par la parité de profondeur.
- **`forced_kind_for_depth` n'active Rest qu'à `total >= 5` et Treasure qu'à
  `total >= 8`** : avec 4 salles, ces deux profils restent inatteignables par le
  graph. Leurs entrées TOML sont prêtes, pas mortes — mais pas encore tirées.
- **Le CoffreRng n'est toujours pas reseedé** depuis `RunSeed` (item du rapport).
- **La récompense n'est pas annoncée sur la porte** — le levier que Hades et Slay
  the Spire utilisent contre le choix illusoire. Le type est affiché, pas le gain.

---

## Récap de test runtime

1. **Action** : lancer le Roguelite, nettoyer la salle 1 (2 vagues), puis **choisir
   une porte** — et refaire une 2ᵉ run en choisissant l'AUTRE porte.
2. **Rechargement** : `cargo run -p forgia` (Rust → rebuild). ⚠️ le binaire est
   `forgia.exe` (paquet `forgia`), **pas** `-p forgia-game`.
3. **Effet attendu** :
   - la salle 2 ne ressemble plus à la salle 1 : **plus d'ennemis** (densité ×1.25)
     et une **répartition différente** selon la porte choisie ;
   - une porte **Élite** donne un combat lourd et lent, une porte **Événement** un
     essaim de Runners — visible à l'œil, sans compter ;
   - les ennemis n'arrivent plus aux mêmes endroits d'une run à l'autre ;
   - le log annonce `→ SALLE 2/4 — porte choisie : Elite (densité ×1.25)`.
4. **Où observer** — `forgia2_roguelite_state.json` :
   - `room_kind` doit correspondre à la porte cliquée (avant : le champ n'existait
     pas, et `room_kind` n'était lu par personne) ;
   - `room_density` doit passer de `1.00` (salle 1) à `1.25` puis `1.56` ;
   - `bots_alive` au spawn doit croître d'une salle à l'autre.
   - Tuning à chaud : éditer `roguelite_waves.toml` et sauver → le log
     `[wave-comp] genome HOT-RELOADED` sort, **effet à la vague suivante**.
5. **Variantes si KO** :
   - `room_kind: "none"` en salle 2+ → le graph n'a pas produit de variants :
     vérifier `roguelite_branching_choices` (défaut 2) et que l'overlay de porte
     s'affiche bien ;
   - `room_density` bloqué à `1.00` → `room_budget` vaut 0, donc le graph est
     absent : vérifier que `RunGraph` est bien inséré par `sys_start_run` ;
   - toutes les salles identiques → `density.enabled = false` dans le TOML, ou le
     genome n'est pas trouvé (chemin **relatif au CWD** : lancer depuis la racine
     du repo — le fallback miroir Rust a les mêmes valeurs, donc c'est indolore
     sauf si tu as édité le TOML) ;
   - **la run se fige sur une salle** → ce serait une vague vide : lire
     `bots_alive` (doit être > 0 au spawn). L'invariant est testé, mais c'est le
     symptôme à surveiller en priorité ;
   - aucun effet du tout → **exe périmé** : comparer `stat` de
     `target/*/forgia.exe` et de `crates/forgia-mode-roguelite/src/wave_comp.rs`
     **avant** de conclure quoi que ce soit.

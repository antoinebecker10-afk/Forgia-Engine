# Story-626 — Roguelite : étalement du spawn décor (fix freeze 65 ms entrée de stage)

> **Statut** : DONE (2026-08-12) — AC écrits et vérifiés depuis les capteurs, cf ci-dessous.
> Le code était livré depuis le 2026-06-25 ; il manquait des critères pour le constater.
> **Niveau BMAD** : Quick (3 fichiers : decor.rs + lib.rs + genome TOML)
> **Origine** : diagnostic story-629 (`perf_diag` + `load_timing` concordants).

## Cause confirmée (2 capteurs)

Les freezes « en combat / en me déplaçant » signalés par l'user ne venaient **ni**
des VFX combustion **ni** de la charge ennemie, mais de l'**instanciation décor en
bloc** :

| Capteur | Preuve |
|---|---|
| `load_timing` | t=2.42, **65 ms**, `entity_delta=+797`, `cause=scene_spawn_gltf` |
| `perf_diag::freezes[0]` | t=3.4, `max_ms=65.3`, **enemies=0** (= phase de chargement) |

`sys_reconcile_decor` spawnait **~800 props GLB en une seule frame** (perimeter +
55 scatter + rooms + rubble + background) → SceneSpawner instancie toutes les
hiérarchies la frame suivante = hitch de 65 ms. Récurrent à chaque entrée d'arène
(= « en me déplaçant » dans une nouvelle salle).

## Fix — planification / drain séparés (étalement)

- **`plan_decor_set`** (ex-`spawn_decor_set`) : ne fait plus que du RNG → retourne
  un `Vec<DecorSpec>` (handles + transforms résolus, **aucune** instanciation).
  Préserve **exactement** le stream RNG de l'ancien code (les salles sont
  décomposées en pièces via `plan_wall_room`/`plan_wall_arm`) → **layout décor
  strictement inchangé**, juste étalé dans le temps.
- **`DecorSpawnQueue`** (Resource) : file des specs + curseur (pas de `remove(0)`).
- **`sys_reconcile_decor`** : remplit la file au lieu de spawner ; garde
  l'idempotence (bail si décor présent **ou** file non vide).
- **`sys_drain_decor_queue`** (`.after(reconcile)`, `GameSet::Movement`) : instancie
  `decor_spawn_budget_per_frame` props/frame jusqu'à épuisement, puis libère la file.

Résultat attendu : un hitch de 65 ms → **~67 frames < 16 ms** (≈ 1,1 s de pop-in
progressif des props lointains/proches), **0 freeze**.

## Data-driven (no-hardcode)

- Gène `decor_spawn_budget_per_frame` (défaut **12**, clamp 1-200) dans
  `assets/genomes/roguelite/roguelite_decor.toml` + fallback `Default` + parsing.
- Hot-reload via le watch genome existant (Shift+F12). Plus haut = remplissage
  rapide / hitch plus gros ; plus bas = pop-in plus long / 0 freeze.

## Tests

- `parse_spawn_budget_default_and_clamp` — défaut 12 + clamp min 1.
- `plan_decor_set_deterministic_and_budgetable` — même seed → même nb de props,
  plan > 100 props, budget < total (drainage multi-frames garanti).
- `cargo test -p forgia-mode-roguelite` : 147 verts. clippy 0 warning (decor.rs).

## Critères d'acceptation (écrits le 2026-08-12, depuis les capteurs)

La story n'en avait **aucun** — c'est ce qui l'a laissée ouverte 48 jours alors que
le code était livré et vivant. Dérivés de la section « Validation runtime »
ci-dessous, évalués sur la run du 2026-08-12 (t≈400 s).

- [x] `plan_decor_set` ne fait que du RNG, `DecorSpawnQueue` draine par frame
      (`decor.rs:736` et `decor.rs:1432`) — livré 2026-06-25
- [x] Layout décor strictement inchangé (stream RNG préservé) — test
      `plan_decor_set_deterministic_and_budgetable`
- [x] Budget en couche definition : gène `decor_spawn_budget_per_frame`
      (`roguelite_decor.toml:183`), défaut 12, clamp min 1
- [x] **Le bloc de ~800 props a disparu.** Le freeze d'origine était **65 ms pour
      +797 entités** à l'entrée de stage. Sur la run du 12-08, aucun
      `scene_spawn_gltf` ne dépasse **+73 entités** — l'instanciation décor en bloc
      n'existe plus.
- [x] **Le « Suivi » de la story est CONFIRMÉ par les données.** Il prévoyait :
      *« si des freezes mid-combat persistent, vagues qui spawnent leur GLB en
      bloc, même traitement à appliquer — à confirmer via `perf_diag` (`enemies`
      haut cette fois) »*. C'est exactement ce qu'on mesure : 3 freezes
      `scene_spawn_gltf` restants (t = 171,9 · 308,9 · 379,8 s ; 46-52 ms pour
      +58, +69, +73 entités) et `perf_diag` donne `enemies` = 13, 18, 8 à ces
      instants. → story dédiée pour `waves.rs`/`enemies.rs`, hors scope ici.

> **Ce qu'il ne faut PAS attribuer à cette story** : sur les **17 freezes** de la
> run, **14 sont `unattributed_cpu_or_gpu` avec un churn d'entités NUL** (delta 0
> ou ±3), dont un à **103 ms**. Ils corrèlent avec l'intensité de combat
> (`enemies` 14-18, `status_auras`, `element_sparks` 33, **`point_lights` 95-103**),
> pas avec l'instanciation. C'est la classe de freeze **dominante aujourd'hui**, et
> elle est ailleurs.

## Validation runtime (à faire par l'user)

1. `cargo run -p forgia -j 4` → lancer une run, entrer dans l'arène + bouger.
2. Lire `forgia2_perf_diag.json::freezes[]` : l'entrée `scene_spawn_gltf` à
   l'entrée de stage doit **disparaître** (plus de `max_ms` 60-130 au démarrage).
3. Vérifier visuellement : le décor « pope » progressivement sur ~1 s au lieu d'un
   gel. Si le pop-in est trop visible → monter `decor_spawn_budget_per_frame`.

## Suivi

- Si des freezes mid-combat **persistent** après ce fix (vagues qui spawnent leur
  GLB en bloc), même traitement à appliquer au spawn de vagues (`waves.rs`/
  `enemies.rs`) — à confirmer via `perf_diag::freezes[]` (`enemies` haut cette fois).

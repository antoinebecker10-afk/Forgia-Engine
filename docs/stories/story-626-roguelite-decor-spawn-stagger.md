# Story-626 — Roguelite : étalement du spawn décor (fix freeze 65 ms entrée de stage)

> **Statut** : CODE-COMPLETE (2026-06-25) — validation runtime user en attente
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

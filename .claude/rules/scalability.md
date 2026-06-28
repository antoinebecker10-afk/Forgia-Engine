---
paths:
  - "**/*.rs"
---

# Scalability Rule — 10K+ Joueurs (Forgia)

## Architecture reseau
- Tout systeme gameplay doit fonctionner en mode autoritatif serveur (pas de trust client)
- Les entites joueur/NPC/projectiles doivent avoir un `NetworkId` pour la replication
- Pas de `Res<Player>` singleton — toujours `Query<(&Player, ...)>` pour supporter N joueurs
- Les events gameplay (damage, spawn, pickup) doivent etre serialisables (serde) pour le netcode

## Performance budget par frame (60 FPS)
- Max 16ms/frame total, budget systemes gameplay < 8ms
- Pas de `Vec::new()` / `HashMap::new()` dans les hot paths — utiliser `Local<>` + `clear()`
- Pas de `String` allocation dans les systemes per-frame — utiliser `&str`, `Cow`, ou cache
- Queries: toujours filtrer avec `With<>` / `Without<>`, jamais iterer toutes les entites
- Spatial: utiliser les chunks/grilles spatiales existants, pas de brute-force O(n²)

## Data-driven & config
- Toute constante gameplay = FpsTuning ou genome TOML (jamais hardcode)
- Les nouveaux systemes doivent supporter le hot-reload (Shift+F12 genome, runtime FpsTuning via Grimoire)
- Les limites (max entities, max chunks, budgets) doivent etre configurables, pas en dur

## Streaming & LOD
- Entites monde (vegetation, batiments, NPCs, ennemis): streaming par chunk obligatoire
- Budget max par type: configurable dans `config/tuning.json` ou FpsTuning
- LOD distance-based pour tout ce qui a un mesh visible (pattern LodChain existant)
- Despawn hors range: obligatoire, pas d'entites invisibles qui consomment du CPU

## Base de donnees & persistence
- Les saves doivent etre versionnees (schema version dans le JSON)
- Pas de full-scan a chaque save — incremental ou dirty-flag
- Les donnees joueur (inventaire, progression, stats) doivent etre separees des donnees monde

## Patterns obligatoires pour le scale
- `SystemParam` bundle quand > 12 params (eviter l'overflow Bevy 16 params)
- `run_if` sur CHAQUE systeme qui ne tourne pas en permanence
- Events > mutation directe pour le decouplage (combat, economie, triggers)
- Tout nouveau module doit documenter son cout CPU/memoire attendu

# AUDIT PERF — CHEMIN DU TIR — 2026-07-20

> Symptôme user : « je perds des perfs quand je tire ». Audit du per-shot path complet
> (agent dédié, 59 lectures) + mesures session 2026-07-20 01:49 + best practices web.
> Complète l'audit 360° (§perf) et story-663 (merge statique, −65 % entités déjà livré).

## Verdict

Le tir déclenche **~14 spawns/despawns d'entités et 8-9 instances Hanabi jetables par tir touché**, plus **1-7 lignes de log synchrones** poussées dans un pipe PowerShell. À cadence Bourrasque (11 tirs/s), c'est ~90 création/destruction d'effets GPU par seconde + 11-15 lignes de log/s — un coût structurel proportionnel à la cadence, exactement le ressenti « je perds des fps en tirant ». Les sensors, l'audio et le gameplay élémentaire sont propres (buffers `Local`, handles cachés, 1 Hz).

## La chaîne du tir (résumé)

`fire_weapon_minimal` (forgia-fps/lib.rs:717, FixedUpdate 64 Hz) → gates → juice → aim assist → **muzzle flash = 6 entités fraîches** (5 ParticleEffect + 1 PointLight TTL 70 ms) → HashSet+Vec d'exclusion **alloués par tir** + walk du subtree Player → cast_ray → tracer (2 entités, handles pré-construits ✓) → **impact = 2 PE + 1 light** → DefenseLayer→Health → hit flash (swap matériau) → `CombatHitEvent` (26 lecteurs : element burst = 1 PE + 1 light/hit, knockback, chain, **Text2d dégâts spawné/hit**, hitmarker, SFX) → kill : despawn + observers (loot avec **mesh/material créés à la volée**, +2-3 logs, 2 Text2d, kill burst).

## Findings (triés par coût)

| # | Localisation | Problème | Sévérité |
|---|---|---|---|
| 1 | `forgia-effects/weapon_vfx/mod.rs:457,562,416` + `element_vfx.rs:323` | **8-9 ParticleEffect Hanabi JETABLES par tir touché** (5 muzzle + 2 impact + 1 element, +2/kill). Chaque instance = slot EffectCache + bind group + dispatch init GPU. Anti-pattern n°1 du framework ([hanabi #255](https://github.com/djeedai/bevy_hanabi/issues/255) : pool de spawners réutilisés) | **Critique** |
| 2 | `forgia-fps/lib.rs:1166,1202,1083` + hitmarker:80 + score:69 + boons_apply:245,281 [CHAUD] | **≥1 `info!` par tir garanti**, jusqu'à ~7 le tick d'un kill. stdout synchrone → **pipe PowerShell** (`run_debug.ps1` `*> forgia2_run.log`) = backpressure possible → le jeu bloque sur le write. Profil compatible avec les freezes 50-64 ms « non corrélés à la charge » ([bevy #10082](https://github.com/bevyengine/bevy/discussions/10082)) | **Haut** |
| 3 | weapon_vfx + element_vfx | **3-4 PointLight jetables par tir** (TTL 40-80 ms) → clustering + shading | Moyen |
| 4 | `run.rs:371-437` | Par kill : `meshes.add` + `materials.add` **neufs par drop de cœur** + SceneRoot GLB par pièce → hitch cumulé sur la frame de kill | Moyen |
| 5 | `forgia-fps/lib.rs:910-923` | HashSet + Vec d'exclusion alloués + walk descendants Player **par tir** (le subtree bras GLB grossit avec story-661) | Bas |
| 6 | `damage_numbers.rs:44` + score.rs:56 | Text2d spawné par hit / par kill (layout glyphes) | Bas |
| 7 | `boons_apply.rs:214-228` [CHAUD] | Chain : Vec collect + sort par hit | Bas |
| 8 | hot-reload `roguelite_vfx.toml` | Reconstruit les EffectAssets SANS re-warmup → 1er tir post-reload paie la compile shader | Bas |

Sains (vérifiés) : tracers (handles pré-construits), audio (handles cachés), sensors (1 Hz), elements matchup/arc (buffers `Local`), hit-stop, aim assist, waves/enemies [CHAUD] (aucun travail par-tir).

## Best practices (web, sourcées)

- **Hanabi** : GPU-first — le coût CPU vient des *instances*, pas des particules. Pattern recommandé : **pool de spawners persistants** (`EffectSpawner` reset + reposition), jamais spawn/despawn d'entités effet par événement. [hanabi #255](https://github.com/djeedai/bevy_hanabi/issues/255), [docs bevy_hanabi](https://docs.rs/bevy_hanabi/latest/bevy_hanabi/)
- **Logging** : l'écriture console Windows est lente ; logs parallèles sur stdout = overhead documenté ; à réserver aux transitions, jamais au per-shot. [bevy #10082](https://github.com/bevyengine/bevy/discussions/10082), [cheat book](https://bevy-cheatbook.github.io/fundamentals/log.html)

## PLAN DE CORRECTIF (lots par ratio effort/gain)

### Lot A — Logs per-shot → `debug!` (Quick ~30 min, teste l'hypothèse n°2 immédiatement)
`[fire] pellet/miss/CRIT`, `DING — POW!`, `[score] KILL`, `[boons] chain/heal_on_kill`, `[death]` → `debug!` (filtrés par défaut, activables via RUST_LOG). La donnée reste disponible via les sensors 1 Hz (hitscan/stats). Fichiers : forgia-fps/lib.rs, hitmarker.rs, score.rs + boons_apply.rs [CHAUD, 2 lignes]. **Validation : freezes 50-64 ms disparus ou pas → tranche le débat.**

### Lot B — Pool Hanabi + lights (Standard ~½ journée, LE gain structurel)
1 entité persistante par type d'effet (muzzle ×5 couches, impact, element burst, kill burst) : reposition + `EffectSpawner::reset()` au lieu de spawn/despawn. PointLight muzzle persistante (intensité animée), cap global sur les lights transitoires. Fichiers : forgia-effects/weapon_vfx/* (SAFE) + element_vfx.rs (SAFE). Gain attendu : suppression du churn GPU ~90 instances/s en auto-fire.

### Lot C — Micro-allocs per-shot/per-kill (Quick ~1 h)
Assets pickups cachés au Startup (run.rs) ; `Local<HashSet/Vec>` pour l'exclusion du tir (forgia-fps) ; pool de Text2d dégâts (damage_numbers) ; chain `Local<Vec>` + `select_nth_unstable` (boons_apply [CHAUD], au merge).

### Lot D — Mesure de contrôle
Re-run + `forgia2_perf_diag.json` : cible = **0 freeze > 40 ms en tir soutenu**, frame stable à cadence Bourrasque. Si résidu → capture Tracy ciblée (le pinpoint fin de l'audit 360° reste valable).

Ordre recommandé : **A (teste) → B (structure) → C (finitions) → D (preuve)**. A et C-partiel touchent 2 lignes de fichiers [CHAUD] (boons_apply) — le reste est hors arbre chaud.

## LIVRAISON (2026-07-20, « go tout ») — clippy `-D warnings` + tests verts partout

- **Lot A ✅** : 7 sites per-shot/per-kill → `debug!` (fire pellet/miss/CRIT, death, DING, chain, heal_on_kill). Données conservées via sensors 1 Hz.
- **Lot B ✅** : pools Hanabi persistants — muzzle 5 couches + 1 light (1 set repositionné/tir), impact 8 slots round-robin, kill burst 4 slots, element burst pool partagé 8 slots (element_vfx.rs). Trigger = `remove::<EffectSpawner>()` (re-add auto par `tick_spawners`, vérifié dans le source vendored 0.18 — ne ré-invalide PAS le cache compilé). Lights jetables → `PooledLightFade` (decay d'intensité sans despawn). Hot-reload VFX = rebuild du pool (rare, dev-only, assumé). **Fin des ~90 create/destroy GPU/s.**
- **Lot C partiel ✅** : chain boon → `Local<Vec>` scratch (0 alloc/hit). **Différés documentés** : cache assets cœur (run.rs — signature), Local exclusion tir (forgia-fps — budget 16 params), pool Text2d dégâts.
- **Lot D à faire (user)** : re-mesure en tir soutenu — cible 0 freeze > 40 ms ; **+ vérif visuelle** : muzzle/impacts/bursts identiques, et le point non tranché de l'agent : le swap de handle du pool élément ne doit pas scintiller (si scintillement → me ping).

## LIVRAISON bis (2026-07-20, passe 2 — suite mesure `spikes_15`) — clippy `-D warnings` + 366 tests verts

La re-mesure post-passe-1 (`forgia2_perf_diag.json`) montrait encore des freezes 64-80 ms **corrélés `status_auras` > 0** : les auras de statut n'étaient PAS couvertes par le Lot B.

- **Pool auras de statut ✅** (suspect n°1 restant) : `status_vfx.rs` réécrit — pool partagé de 12 slots Hanabi persistants (`StatusVfxPool`), lease immédiat dans la Resource (anti-course intra-frame), attach = swap handle+texture (capacités des 3 effets UNIFIÉES dans `weapon_vfx/status.rs` → même layout, pas de réalloc EffectCache) + retrigger `remove::<EffectSpawner>` ; detach = `spawner.active = false` + slot caché. Reset des leases `OnExit(Roguelite)` (sinon auras orphelines au Bourg). Suivi `sys_follow_status_vfx` + capteur `perf_diag.status_auras` inchangés (via `StatusVfxLink` sur les slots). Warmup shader des 3 effets à la 1re émission cachée des slots (PostStartup).
- **Hit-flash par emissive ✅** (suspect secondaire) : `HitFlashCache` SUPPRIMÉ — le flash mute `emissive` EN PLACE (handle inchangé → zéro re-batch GPU, zéro swap `MeshMaterial3d` per-hit) avec garde anti double-capture (timer reset si flash actif + égalité emissive pour les pellets même frame — corrige au passage un bug latent : l'ancien code re-capturait le flash-material comme « original » sur hits chevauchés). **Constat honnête** : ce chemin ne s'exécutait de toute façon JAMAIS sur les ennemis GLB (aucun `MeshMaterial3d` sur la racine `TargetCube`, visuel = SceneRoot enfant) — il ne pouvait donc PAS être la source des spikes ; converti quand même (correct si un porteur racine réapparaît) et machinerie swap retirée.
- **Pool Text2d dégâts ✅** (reliquat Lot C) : `damage_numbers.rs` — 16 slots round-robin persistants, texte réécrit en place (`fmt::Write`, 0 alloc/hit), slot expiré caché (jamais despawné).
- **Assets icônes de statut partagés ✅** (M1 de l'auto-QA post-livraison) : `spawn_status_icon` faisait `meshes.add` + `materials.add` PAR icône (même cadence événementielle que les auras) → quad 1×1 + 3 matériaux (par kind) construits une fois au PostStartup (`StatusIconAssets`), taille portée par le scale du Transform (suit le tuning nameplate), teintes baked au boot (précédent `ElementBurstAssets`).
- **Auto-QA passée** (verifier 5/5 PASS + qa-lead) : 0 Bloquant ; le Majeur (icônes) corrigé ci-dessus ; 2 commentaires inexacts corrigés (saturation du pool : un refresh par overwrite ne réarme pas `Added<T>` → pas de retry avant expiration+réapplication, visuel seulement ; hot-reload : les leases actives GARDENT l'ancien rendu, pas de perte). Écarté après vérif : race FixedUpdate/Update sur expiration+refresh même frame (RunFixedMainLoop flush avant Update, l'insert gagne toujours) ; slots Text2d complets via required components de `Text2d` (bevy_sprite 0.18.1).
- **Reste identifié non traité** (hors scope passe 2) : Text2d de `score.rs` par kill ; cache assets cœur (`run.rs`) ; `Local` exclusion tir (forgia-fps) ; 1re icône de statut = compile pipeline possible (variante Blend+unlit+texture, one-shot ~ms, non warmée).
- **Mesure de contrôle (user)** : re-run + tir soutenu avec DoT (Bourrasque/Boucherie sur pack) → cible `spikes_15` ≈ 0 hors secondes de spawn de vague, plus aucun freeze > 45 ms corrélé `status_auras`.

---

*Sources : [bevy_hanabi #255](https://github.com/djeedai/bevy_hanabi/issues/255) · [bevy_hanabi docs](https://docs.rs/bevy_hanabi/latest/bevy_hanabi/) · [Bevy #10082](https://github.com/bevyengine/bevy/discussions/10082) · [Bevy Cheat Book — log](https://bevy-cheatbook.github.io/fundamentals/log.html). Audit agent 59 lectures + mesures session 01:49 (23 tirs, 458 lignes de log).*

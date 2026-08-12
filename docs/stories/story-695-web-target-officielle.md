# Story-695 — La cible web (wasm/WebGPU) devient un build officiel de Forgia

**Statut** : ⏸️ STOPPÉE (décision Antoine 2026-08-12 — reprise APRÈS le redesign
du concept du jeu ; alors gate 3 étages puis migration « quand c'est propre »).
inc.1+2+3+4b ✅ livrés — restent : 5 manifeste, 6 parité, 7 pipeline (entamé).
⚠️ Au gel : le site public porte un build qui panique à 1-3 min (RenderDiagnostics/
WebGPU) ; le fix est commité (forgia-observability) mais PAS publié — bundle
rebuilt prêt dans web-demo/, gate staging bridé jamais passé.
**Niveau BMAD** : Enterprise (7 incréments, ~15 crates touchées au total)
**Origine** : nuit du 2026-08-10→11 — portage prototype validé en jeu dans Chrome
(menu, run, Hall — 185 fps menu vs 240 natif). Audit complet :
[web-port-audit-2026-08-11](../audits/web-port-audit-2026-08-11.md).
**Décision produit** : la version web est le canal de test des nouveaux joueurs,
alimentée régulièrement depuis le repo. Le build web doit donc se reconstruire
depuis `main`/branche, pas depuis un worktree patché à la main.

## Incréments

| # | Contenu | Statut |
|---|---|---|
| 1 | Les 8 correctifs du prototype portés proprement (cfg-gates wasm, web-time, tonemapping) — `cargo check` vert sur les DEUX cibles | ✅ DONE 2026-08-11 |
| 2 | Sink d'observabilité web (capteurs → mémoire + `forgia_dump_sensors()` + bouton 📋 Diag) | ✅ DONE 2026-08-11 |
| 3 | Persistance web (localStorage via `persist.rs` : Enclume, équipement, identité, FTUE) | ✅ DONE 2026-08-11 |
| 4 | (fusionné dans inc.1 : graine + timestamps via web-time) | DONE-par-1 |
| 4b | **Parité découverte au playtest testeur** (2026-08-11 après-midi) : écran noir arène = bataille de tonemapping (`apply_tonemapping_to_cameras` réapplique TonyMcMapface — LUT neutralisée sur wasm — contre la garde AcesFitted ; l'ordre des systèmes décidait du gagnant) → résolu À LA SOURCE (`tonemapping_from_str` ne rend jamais une LUT sur wasm). Avatar menu absent = `roguelite_equipment.toml` lu via `std::fs` → 0 slot → pas de body_model → `forgia_core::def_io` : pack genomes+registry EMBARQUÉ à la compilation (build.rs, 1,1 Mo TOML) ; equipment + mushrooms migrés. Boucle re-spawn mushrooms chaque frame gardée (0 cluster). Capteur `avatar_ready` mesurait `is_some()` → mesure les pièces montées. Shaders `.wgsl` ajoutés au bundle (toon postprocess silencieusement absent). | ✅ DONE 2026-08-11 |
| 5 | Manifeste d'assets déclaré + validé au build (fin du grep) ; vidéo menu → .webm | TODO |
| 6 | Matrice de parité web/natif complète (amorce dans l'audit §2). **Reste connu** : ~30 lecteurs `std::fs` de genomes non migrés vers `def_io` (ambiances, weapon_vfx, death_ascension, knockback, color_grading, foliage, castle_*, gait, items…) — en défauts silencieux sur web ; textures KTX2 UASTC (jolcham_oak) intranscodables sans basis ; UserSettings (pause_menu) non persistés sur web. | TODO |
| 7 | Pipeline de publication à 3 étages (décision Antoine 2026-08-12, « pas d'itérations en boucle sur la prod ») : **local** (localhost:8907, validation headless CDP : boot + menu avec avatar + arène + dump capteurs) → **staging HTTPS** (même bundle, réseau bridé simulant le CDN via CDP `Network.emulateNetworkConditions` + tunnel cloudflared pour test mobile réel ; gate : 4 min de session sans panic) → **Pages** (seulement si le gate passe). Origine : le crash RenderDiagnostics ne se déclenchait QUE sous calages réseau — invisible en local non bridé, découvert en prod. Versioning/cache-busting : TODO | EN COURS |

## Acceptance criteria — incrément 1

- [x] `cargo check` (natif) vert, 0 warning nouveau
- [x] `cargo check --target wasm32-unknown-unknown` vert
- [x] `cargo clippy --workspace` : 0 warning sur fichiers touchés
- [x] Natif inchangé : file_watcher (hot-reload) et basis-universal toujours actifs
      en natif via dépendance target-gated ; `WgpuSettingsPriority` par défaut en natif
- [x] Tests existants verts (+ fix des 2 tests barks, rouge préexistant du checkpoint 08-10)
- [x] Le worktree `D:\forgia-web-worktree` peut être supprimé :
      `tools/web/build-web.ps1` reconstruit depuis la branche (sortie `web-demo/`, gitignorée)

## Les 8 correctifs (détail : audit §3, mémoire reference_wasm_web_port_forgia)

basis-universal/file_watcher target-gatés · zstd target-gaté (terrain) · feature
`webgpu` · sensor_io no-op wasm · web-time (horloges, ~10 fichiers) · asset root
relatif wasm · gardes Rgba16Unorm + tonemapping AcesFitted wasm · limites WebGPU
Functionality (wasm seulement).

## Hors scope story-695

Contrôles tactiles, PWA iOS, multithread wasm (atomics/COOP-COEP), transcodeur
basis pour le web — stories suiveuses si le canal testeurs le justifie.

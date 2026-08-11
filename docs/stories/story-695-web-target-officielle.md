# Story-695 — La cible web (wasm/WebGPU) devient un build officiel de Forgia

**Statut** : IN_PROGRESS (incrément 1 ✅ livré 2026-08-11 — suite : inc.2 sink observabilité)
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
| 2 | Sink d'observabilité web (capteurs → console structurée) — prérequis canal testeurs | TODO |
| 3 | Persistance web (localStorage : Enclume, équipement) | TODO |
| 4 | (fusionné dans inc.1 : graine + timestamps via web-time) | DONE-par-1 |
| 5 | Manifeste d'assets déclaré + validé au build (fin du grep) ; vidéo menu → .webm | TODO |
| 6 | Matrice de parité web/natif complète (amorce dans l'audit §2) | TODO |
| 7 | Pipeline de publication GitHub Pages versionné | TODO |

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

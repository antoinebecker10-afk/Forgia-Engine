# Audit portage web — nuit du 2026-08-10 → 11

> **Statut : jeu fonctionnel dans le navigateur (Chrome/WebGPU), lag en cours de traque.**
> Prototype dans le worktree isolé `D:\forgia-web-worktree` — **rien de commité**, la
> carte des correctifs ci-dessous EST le plan du portage propre (story candidate : 695+).
> Décision produit (Antoine, 2026-08-11 ~01h30) : la version web devient le **canal de
> test des nouveaux joueurs**, alimentée régulièrement — le workflow doit être parfait.

---

## 1. Baseline AVANT (natif, run du 2026-08-10 23:46, capteurs)

| Métrique | Valeur |
|---|---|
| FPS lissé | **240,5** |
| Frame time avg / min / max | 4,17 / 3,61 / 4,89 ms |
| GPU par frame | 0,61 ms (`gpu_frame_ratio` 0,147 — **headroom**) |
| Stutters >30 ms sur 30 s | **12** (max 92 ms — préexiste au web, capteur `lag_events` warn) |

Le jeu natif a une marge énorme. Le lag web est un problème de **portage**, pas de gameplay.

## 2. APRÈS (web, Chrome WebGPU sur la même machine)

- **Boote, atteint le menu, boucle de jeu vivante 120 s+ sans panic** (validé headless + visuel Antoine).
- Symptôme rapporté : « bourré de lag » (menu) — **cause n°1 confirmée et corrigée** (boucle 404
  menu_video, cf §4.1). Mesuré build 11 (headless, vrai GPU, scène menu, diagnostics Bevy) :

| Métrique | Natif | Web build 10 (avant fix) | Web build 11 |
|---|---|---|---|
| FPS moyen | 240 | « bourré de lag » (non mesuré) | **~185** |
| Frame time avg | 4,2 ms | — | **5,6 ms** |
| Spam menu_video/s | — | ~24 fetch 404 + WARN | **0** |
| Pics observés | 12×>30 ms/30 s | — | ~21 ms isolés |

Le web atteint **~77 % du natif** au menu. Reste à mesurer : in-run (vraie partie), et le
ressenti Antoine (validation manette en main, seule qui compte — cf map-design-intention §5.3).

**Validation Antoine (2026-08-11 ~01h35, build 11)** — amorce de la matrice de parité :

| Système | Web | Note |
|---|---|---|
| Menu principal + navigation | ✅ | validé manette en main |
| Lancement d'une run | ✅ | in-run atteint |
| Hall de Forgia (castle hub 3D) | ✅ | praticable |
| FPS chiffré in-run | à relever | compteur en haut à droite |

## 3. Les 8 murs franchis (= la carte du portage propre)

| # | Mur | Cause | Correctif (worktree) |
|---|---|---|---|
| 1 | `basis-universal` ne compile pas | C++ sans sysroot wasm | feature bevy retirée |
| 2 | `zstd-sys` exige clang | dep C ; clang Windows sans backend wasm | zstd target-gated + octets bruts sur wasm (`forgia-terrain/chunk.rs`) |
| 3 | hanabi panic `limit is 0` | **build silencieusement WebGL2** (feature défaut bevy) | feature `webgpu` ajoutée — VFX OK |
| 4 | panic thread capteurs | pas de threads wasm | `sensor_io::enqueue/remove` no-op wasm (`forgia-core`) |
| 5 | panics `SystemTime/Instant` (~40 sites) | horloges std absentes sur wasm | **`web-time`** (9 fichiers, 7 crates) + stubs ciblés ; `AnimTimer` → `bevy::platform::time::Instant` |
| 6 | assets en `file:///D:/...` | asset root absolu Windows | `file_path: "assets"` relatif sur wasm (`forgia-game`) |
| 7 | panic `Rgba16Unorm` | LUT tonemapping (TonyMcMapface/AgX) sans équivalent WebGPU, chargées au boot | garde wasm : neutralisation des images Rgba16Unorm + bascule caméras vers `AcesFitted` |
| 8 | limites WebGPU minimales | Bevy demande les minima de la spec | `WgpuSettingsPriority::Functionality` (⚠ à cfg-gater wasm au portage propre) |

## 4. Hypothèses lag (par ordre de conviction)

1. **Boucle 404 menu_video** *(corrigée build 11)* — sur wasm `fs::read_dir` échoue → fallback
   « 361 frames » codé en dur → player 24 fps sur des fichiers absents = ~24 fetch 404/s +
   spam WARN console (chaque log wasm traverse JS, coûteux). Le chemin gracieux `frame_count == 0`
   ne se déclenchait jamais. Fix : 0 forcé sur wasm → fond uni.
2. **Monothread intégral** — pas de threads wasm : tout Bevy (systèmes, rayon, asset decode)
   sérialise sur un cœur. Le natif tire parti du multicœur partout. Mitigable (atomics +
   COOP/COEP), mais chantier ; d'abord mesurer ce qu'il reste après (1).
3. **RTT du menu** — le diorama arène+avatar rendu en texture chaque frame derrière l'UI
   (cf `reference_menu_arena_backdrop_rtt`) : double scène 3D sur un seul cœur.
4. **Spam console résiduel** — ~90 assets référencés absents du bundle → WARN par événement.
   Après (1), volume à re-mesurer ; filtre possible (niveau log `warn→error` sur wasm).
5. **wasm-opt -Oz** (taille) vs -O3 (vitesse) — quelques % éventuels, à tester en dernier.

## 5. Dégradations connues et assumées (prototype)

- Saves (Enclume, équipement) **non persistées** (fs mort — piste : localStorage/IndexedDB au portage).
- Graine de run **fixe** sur web (`run.rs` — TODO js Date via web-time) → toutes les runs identiques.
- Vidéo du menu absente (52 MB de frames webp — pipeline `.webm` à prévoir si voulu).
- 3 KTX2 jolcham (Basis supercompressé) refusés — transcodeur retiré (mur 1) ; fallback matériaux.
- Capteurs `forgia2_*.json` silencieux sur web — **trou d'observabilité** : pour le canal
  testeurs il faudra un sink web (console structurée, ou POST vers un endpoint de collecte).
- Sons : OK après le clic JOUER (geste utilisateur requis par les navigateurs).

## 6. Workflow actuel (reproductible en 1 commande)

`D:\forgia-web-worktree\build-web.ps1` : build wasm → bindgen → (`-Opt` : wasm-opt 108→63 MB)
→ sync assets référencés (120 fichiers + genomes, 193 MB) → serveur local :8907 → tunnel
cloudflared (URL https affichée). `-ServeOnly` pour relancer serveur+tunnel seuls.

**Diffusion** : tunnel = éphémère, PC allumé requis. **Cible pérenne** : push du dossier
`web-demo` vers `antoinebecker10-afk.github.io` (dépôt créé, renommé, vide — publication
automatique GitHub Pages, https, URL stable). Une commande depuis le PC, credentials locaux.

## 7. Prochaines étapes

1. Mesurer le lag post-fix menu_video (compteur page + console F12) — menu ET in-run.
2. Trancher les hypothèses restantes dans l'ordre (2) → (5) avec mesures.
3. Décider : publier la démo sur Pages dès « jouable », ou après tuning.
4. **Story 695+** : porter les 8 correctifs en propre dans le repo principal
   (cfg-gates wasm, web-time en workspace dep, tonemapping conditionnel, story-gate).
5. Observabilité web (sink capteurs) — prérequis au rôle « testeur des nouveaux joueurs ».
6. Contrôles tactiles + PWA iOS (plein écran) — après validation desktop web.

---

*Rédigé pendant la traque, build 11 en compilation. Mesures « APRÈS » à compléter à chaud.*

# Story-631 — Présence viewmodel : sway/bob + bras cartoon procéduraux

**Statut** : CODE COMPLETE — en attente validation runtime + commit
**Niveau BMAD** : Standard (forgia-viewmodel + forgia-fps + genome)
**Créée** : 2026-06-24
**Origine** : feedback user « l'arme est seule au milieu de l'écran, ça fait vide ». Audit :
le viewmodel n'a AUCUN sway/bob/idle (seulement la pose ADS) → arme collée/morte. Veille
(sujet 2) : « sans sway l'arme paraît collée à l'écran » = ingrédient de présence manquant.
User n'a pas d'assets de bras → demande des bras générés en cartoon.

---

## Décisions de design

- **Inc 1 — Mouvement (code seul, 0 asset)** : sway (l'arme traîne quand on tourne la souris),
  bob de marche, respiration idle. Couche **additive** appliquée APRÈS `apply_ads_viewmodel`
  dans le chain pose → zéro accumulation (la base est réécrite chaque frame par l'ADS).
  Data-driven via `fps_tuning.toml [viewmodel_motion]` (hot-reload), comme camera_shake/ads.
- **Inc 2 — Bras cartoon procéduraux (mesh généré en code, 0 asset externe)** : 2 avant-bras
  (cylindres effilés) + mains-moufles (blocs, sans doigts = cartoon), couleur toon plate.
  Enfants de la FpsCamera (PAS du viewmodel arme : l'arme est auto-scalée par AABB, les bras
  doivent garder une taille constante). Reçoivent le MÊME offset sway/bob que l'arme (présence
  cohérente). Pose de repos data-driven (reach vers le grip).

---

## Acceptance Criteria

- [ ] AC1 — En tournant la souris, l'arme traîne puis se recentre (sway), sans bouger la visée réelle.
- [ ] AC2 — En marchant, l'arme oscille (bob) ; à l'arrêt, micro-respiration (idle). Subtil.
- [ ] AC3 — Le sway/bob NE perturbe PAS le tir (direction de tir = caméra, inchangée) ni l'ADS.
- [ ] AC4 — 2 bras cartoon visibles tenant l'arme, taille constante quelle que soit l'arme équipée.
- [ ] AC5 — Bras + arme bougent ensemble (même sway/bob) → cohérent.
- [ ] AC6 — Tout data-driven (`fps_tuning.toml [viewmodel_motion]`, hot-reload Shift+F12). 0 hardcode feel.
- [ ] AC7 — `cargo check`/`clippy` vert (forgia-viewmodel + forgia-fps), 0 warning ; tests purs verts.

---

## Fichiers touchés

| # | Fichier | Inc | Rôle |
|---|---|---|---|
| 1 | `crates/forgia-viewmodel/src/pose.rs` | 1 | `ViewmodelMotionTuning` + `apply_viewmodel_sway_bob` (additif, chain) |
| 2 | `crates/forgia-fps/src/lib.rs` | 1 | `FtViewmodelMotion` + sync_fps_tuning → ViewmodelMotionTuning |
| 3 | `assets/genomes/fps_tuning.toml` | 1 | `[viewmodel_motion]` |
| 4 | `crates/forgia-viewmodel/src/arms.rs` (nouveau) | 2 | Mesh procédural cartoon + spawn enfant caméra + base pose |
| 5 | `crates/forgia-viewmodel/src/lib.rs` | 2 | mod arms + wire plugin |

---

## Notes

- Pas d'anim squelettique des bras (procédural statique) ; la présence vient du sway/bob partagé.
- Placement main par-arme = candidat tuning futur (v1 : pose de repos générique).

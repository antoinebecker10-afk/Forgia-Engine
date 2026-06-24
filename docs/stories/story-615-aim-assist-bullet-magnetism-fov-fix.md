# Story-615 — Aim assist « bullet magnetism » (souris) + FOV joueur réellement appliqué

**Statut** : CODE COMPLETE — en attente validation runtime + commit (gate story-done : story untracked)
**Niveau BMAD** : Standard (cross-crate, 7 fichiers)
**Créée** : 2026-06-24
**Origine** : demande user « audite la caméra + améliore l'aim assist ». Audit Concept-First +
veille web (état de l'art 2023-2026 : Apex/Destiny/COD/Halo, Eiserloh GDC 2016).

---

## Contexte / findings de l'audit

1. **🔴 Aim assist mort en Roguelite** (mode ship prioritaire). `aim_assist.rs` requêtait
   `With<forgia_damage::Health>` ; or les ennemis Roguelite portent `forgia_combat::Health`
   (swap volontaire story-490, `waves.rs:27`). Le cône frontal ne matchait **aucun** ennemi
   → `aim_assist_engagements_total` bloqué à 0.
2. **🔴 Mauvais modèle pour la souris.** L'impl faisait un **pull rotationnel de la caméra**
   (bouge la visée à la place du joueur). La veille est catégorique : à la souris c'est un
   anti-pattern rejeté (cf. alpha *Marathon* 2025). Apex/Destiny/COD réservent pull+friction
   à la manette et appliquent aux deux le **bullet magnetism** (on corrige le *tir*, pas la
   caméra). Décision user : **bullet magnetism seul**.
3. **🟠 Slider FOV mort.** Le slider existe déjà (`pause_menu.rs`, défaut 90°) mais
   `apply_ads_camera_fov` (viewmodel) réécrit le FOV à `default_fov_deg=45°` **chaque frame**
   → le réglage joueur est clobbé. Décision user : FOV élargi (défaut 90°) + slider **vivant**.

---

## Décisions de design

- **Aim assist = bullet magnetism uniquement.** On bend la direction du tir vers la meilleure
  cible dans un cône réticule-centré, borné par un angle de correction max (anti-aimbot). La
  caméra n'est **jamais** déplacée par le jeu. `strength=0` ⇒ strictement off.
- **Cible cross-mode découplée du type Health** : filtre `With<forgia_damage::Mortal>,
  Without<Player>` (présent sur tous les ennemis FPS + Roguelite, jamais sur le joueur).
  Élimine structurellement le piège des deux `Health`.
- **FOV hipfire = ressource `forgia_player::CameraFov`** (dépendance commune viewmodel ⋂ ui-lib,
  zéro nouvelle dép, zéro risque de cycle). `pause_menu` y propage `UserSettings.fov_deg` ;
  `apply_ads_camera_fov` l'utilise comme base hipfire. Le hipfire FOV n'est plus un gène genome
  (c'est une préf joueur) → retiré de `AdsTuning`/`FtAds`/`fps_tuning.toml`.

---

## Acceptance Criteria

- [ ] AC1 — Tirer à ~3° à côté d'un ennemi (cône) connecte ; tirer franchement à côté (hors cône) rate.
- [ ] AC2 — La caméra ne bouge JAMAIS toute seule (pas de pull). La souris garde 100% du contrôle.
- [ ] AC3 — `strength=0.0` ⇒ aucun bend (tir = direction caméra brute).
- [ ] AC4 — Fonctionne en Roguelite ET en FPS Arena (cible `Mortal`, pas `Health` typé).
- [ ] AC5 — Slider FOV du menu ESC modifie réellement le FOV hipfire en jeu (live), défaut 90°.
- [ ] AC6 — ADS continue de zoomer correctement (lerp hipfire→ads_fov) par-dessus le FOV joueur.
- [ ] AC7 — Sensor `forgia2_aimassist.json` (1Hz) : strength, cône, % tirs corrigés, dernière
      correction ; health check « warn » si actif mais 0 correction sur >N tirs.
- [x] AC8 — `cargo check`/`clippy` vert sur les 4 crates (0 warning hors préexistant forgia-core),
      `cargo test` : 10 tests aim_assist + 29 ui-lib/viewmodel verts. Auto-QA qa-lead : 0 Bloquant/Majeur
      (3 cosmétiques sensor corrigés + tests anti-régression ajoutés ; HashSet hot-path = dette préexistante hors scope).

---

## Fichiers touchés

| # | Fichier | Inc | Rôle |
|---|---|---|---|
| 1 | `crates/forgia-fps/src/aim_assist.rs` | 1 | Réécriture : tuning + métriques + `bend_fire_direction` (pure) + sensor + tests |
| 2 | `crates/forgia-fps/src/lib.rs` | 1 | HitscanCtx (cible+tuning+métriques), bend dans fire path, schedule, sync, FtAds/FtAimAssist |
| 3 | `assets/genomes/fps_tuning.toml` | 1+2 | `[aim_assist]` reframe (+max_correction_deg) ; `[ads]` retire default_fov_deg |
| 4 | `crates/forgia-player/src/lib.rs` | 2 | Ressource `CameraFov` (hipfire FOV partagé) + prelude |
| 5 | `crates/forgia-viewmodel/src/pose.rs` | 2 | AdsTuning retire default_fov_deg ; apply_ads lit CameraFov |
| 6 | `crates/forgia-ui-lib/src/pause_menu.rs` | 2 | apply_fov → écrit CameraFov (au lieu de clobber Projection) |
| 7 | `docs/stories/_index.md` | — | Index |

---

## Notes

- LOS check (ne pas bend vers un ennemi derrière un mur) volontairement **hors v1** : le raycast
  post-bend gère déjà l'occlusion (un tir bendé dans un mur frappe le mur, comme un miss normal).
  Candidat tuning futur si edge case « bend dans le mur » observé.

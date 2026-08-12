# Story-649 — Aim assist cohérent : falloffs gradués façon CoD BO6

> **Statut** : IN_PROGRESS (validation feel user en attente)
> **Niveau BMAD** : Quick (2 fichiers + TOML)
> **Origine** : feedback user 2026-07-03 « l'aim assist est trop importante, rends-la cohérente, regarde les CoD récents ». Mesure capteur : **45 % des tirs corrigés** (17/38), jusqu'à 5° de courbure.

## Recherche (CoD récents)

- CoD = **jamais de courbure de balle** ; manette uniquement : slowdown + rotational AA.
- **Le principe BO6 transposable** (patch notes, [PC Gamer](https://www.pcgamer.com/games/call-of-duty/call-of-duty-black-ops-6s-new-set-of-patch-notes-reveals-that-aim-assist-will-be-much-weaker-at-point-blank-range/)) : « aim assist strength now linearly interpolates — much weaker at point-blank, smoothly increasing over short range » ; BO6 vs MW3 : rotation 25° vs 68° en close range, identique ≥10 m ([gist.ly](https://gist.ly/youtube-summarizer/analyzing-aim-assist-in-black-ops-6-comparison-with-modern-warfare-3)).
- Le bullet magnetism reste LE mécanisme souris (story-615) — c'est sa **gradation** qui manquait.

## Défaut du modèle story-615

Aide **binaire** : une cible à 6.9° du réticule (bord du cône de 7°) recevait la même fermeture de 60 % qu'une cible à 0.5°, à 60 m comme à 5 m → « j'ai clairement raté mais ça touche ».

## Changements (`aim_assist.rs` + `fps_tuning.toml`)

1. **Falloff angulaire** (smoothstep) : aide pleine au centre du réticule → **0 au bord du cône**.
2. **Falloff distance** (pattern BO6) : rampe 0→plein sur 0-20 % de la portée (bout portant = le joueur contrôle), plateau 20-60 %, fondu 60-100 % → 0.
3. **Retune** : strength 0.6→0.5, cône 7°→3.5°, correction max 5°→2°, portée 60→40 m.
4. Fractions de rampe = consts de forme de courbe documentées (pas des genes — creator-simplicity : 4 sliders suffisent, les falloffs sont la structure).

## Acceptance criteria

- [x] Bord de cône ≈ zéro aide (test `edge_of_cone_gets_almost_no_help`)
- [x] Bout portant < mi-portée (test `point_blank_help_is_reduced`, pattern BO6)
- [x] Fondu longue portée (test `long_range_help_fades_out`)
- [x] `strength=0` reste strictement off ; borne anti-aimbot conservée
- [x] 38 tests verts, clippy 0 warning introduit, capteur `forgia2_aimassist.json` inchangé (mêmes champs)
- [x] **La moitié MESURABLE de l'AC est atteinte** — run du 2026-08-12,
      `forgia2_aimassist.json` : `shots_corrected: 156` sur `shots_total: 496`,
      soit **31 %**, contre les **~45 %** d'avant. La baisse attendue est là.
      Réglages actifs : `strength 0.50`, `cone 3.5°`, `max_correction 2.0°`,
      `engage_distance 40 m`. Recoupé par
      `forgia2_fps_feel.json::aim_assist_engagements_total: 156`.
- [ ] **Validation feel user** : plus aucun « raté net mais touché »
      → *reste ouvert à dessein : c'est la moitié subjective, et aucun compteur
      ne distingue une correction légitime d'une correction volée.*

## Rollback / tuning

Tout dans `fps_tuning.toml [aim_assist]` (reload Shift+F12). Revenir au feel story-615 : 0.6/7.0/5.0/60.0 (les falloffs resteront — ils sont strictement plus cohérents).

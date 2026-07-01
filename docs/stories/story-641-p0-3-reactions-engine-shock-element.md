# Story-641 — P0-3 : moteur de réactions générique + Element::Shock (Électrique)

> **Source** : masterplan `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5, item **P0-3**.
> Direction : `docs/design/direction-forgia-gunfire.md` §5. Suite de [story-640](story-640-p0-2-defense-layer-shield-armor.md) (P0-2).
> **Décision user 2026-07-01** : **Explosif → Électrique** (colle à la direction verrouillée
> « 4 éléments : Feu/Poison/Électrique/Perforant »). Remap destructif assumé.
> **Scale BMAD** : Standard (≥2 crates : `forgia-mode-roguelite` + `forgia-fps` + genome).
> **Statut** : ✅ DONE 2026-07-01 — **Inc.1 + Inc.2 + Inc.3 FAITS**. Commit `fdaec7d`.
> Compile (`forgia-mode-roguelite` + binaire `forgia` 62 crates) + clippy (0 warning sur fichiers
> touchés) + tests **228/228** verts. Auto-QA passée (verifier PASS ; qa-lead WARN → fix #1 appliqué
> [Miasma skip si `is_kill`], #2 additif triple-élément documenté comme payoff assumé).
> **story-gate --story 641 : PASS** (G1 tracked + G3 crate-LOC 22402). Runtime : Combustion + Miasma
> confirmés en jeu (`forgia2_elements.json`). Le hit-de-base vuln (+10 %) reste P0-4 (re-route via
> `DefenseLayer`), comme prévu.

## Objectif
Passer le 4e élément d'**Explosif** à **Électrique (`Shock`)** et **généraliser le moteur de
réactions** (aujourd'hui la Combustion Feu+Poison est câblée en dur) en une `ReactionTable`
data-driven couvrant les 3 réactions de la direction :

| Réaction | Statuts co-présents | Effet |
|---|---|---|
| **Combustion** | Feu + Poison | burst AOE = % du tir *(existe déjà, story-611)* |
| **Miasma** | Électrique + Poison | DoT % PV max, stackant |
| **Surcharge** | Feu + Électrique | décharge / AOE |

+ `StatusShock` : marque électrique appliquée par les hits Électriques, **+10 % de vulnérabilité**
(dégâts subis ×1.1) tant qu'elle tient. Le couplage **Électrique→Bouclier** (canal Shield de
`DefenseLayer`) reste **P0-4** (re-route matchup→couche) — ici on ajoute l'élément + les réactions.

## Incréments (chacun compile + tests verts)
- **Inc.1 — Rename Explosif → Électrique (`Shock`)** : `Element::Explosive`→`Element::Shock`
  (label/from_key/idx=2/rgb/fr_name/tag/popup/all) + genome `roguelite_elements.toml`
  (`mapping.modern_ar="shock"`, `[matchup.shock]`, `shock_rgb` bleu électrique) + `element_vfx.rs`
  (array idx + flag) + sensor JSON + tests. Le **splash AOE** de l'ex-Explosif est **reflavoré
  « arc électrique »** (même code splash-voisins → identité chaînage électrique du pistolet Pépin).
- **Inc.2 — `StatusShock` + vulnérabilité** : composant `StatusShock{secs_left,tick_accum}`
  appliqué sur hit Électrique ; **+10 % dégâts subis** appliqué au hit de base (forgia-fps) +
  aux dégâts d'éléments. Genome `[shock]` (duration, vuln_mul). Sensor.
- **Inc.3 — Moteur de réactions générique** : `ReactionTable` (paires de statuts → `ReactionKind`)
  remplace le bloc Combustion câblé. Ajoute **Miasma** (Élec+Poison → DoT %PV stackant) et
  **Surcharge** (Feu+Élec → décharge AOE). Genome `[miasma]`, `[surcharge]`. Sensor réactions.

## Critères d'acceptance
| # | AC | Preuve |
|---|---|---|
| AC1 | `Element::Shock` remplace `Explosive` partout (enum+genome+vfx+sensor+tests), pistolet Pépin = Électrique | `elements.rs`, genome |
| AC2 | `DamageKind::Explosion` (roquette Boucherie) **inchangé** (concept séparé, pas touché) | `boucherie_rocket.rs` |
| AC3 | `StatusShock` appliqué sur hit Électrique + +10 % vulnérabilité (base hit + éléments) | `elements.rs`, `forgia-fps` |
| AC4 | `ReactionTable` data-driven : Combustion + Miasma + Surcharge ; Combustion inchangée fonctionnellement | `elements.rs`, genome |
| AC5 | Sensor `forgia2_elements.json` expose Shock + réactions (miasma/surcharge counts) | `elements.rs` |
| AC6 | 0 warning clippy (fichiers touchés), tests purs verts, no-hardcode (genome) | `cargo clippy`/`test` |

## Hors scope
- **Électrique→Bouclier** (canal Shield de `DefenseLayer`, ×fort vs bouclier) → **P0-4** (re-route matchup→couche).
- **Manipulation** (réaction 4e mentionnée ailleurs) → post-P0-3 si besoin.
- HUD réactions / VFX dédiés Miasma/Surcharge → réutilisent le pipeline VFX existant (couleurs), polish P1.

## Suite
P0-4 = re-router le matchup élémentaire vers la **couche de défense** (Feu→Vie, Électrique→Bouclier,
Perforant→Armure) + router combustion/AOE/dérivés via `DefenseLayer` (dette tracée story-640 §Hors scope).

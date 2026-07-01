# Story-642 — P0-4 : affinité élément ↔ couche de défense (matchup → DefenseLayer)

> **Source** : masterplan `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5, item **P0-4**.
> Suite de [story-640](story-640-p0-2-defense-layer-shield-armor.md) (P0-2) et
> [story-641](story-641-p0-3-reactions-engine-shock-element.md) (P0-3).
> **Scale BMAD** : Enterprise (rewire du chemin combat committé, cross-crate
> `forgia-damage` + `forgia-mode-roguelite` + `forgia-fps`).
> **Statut** : IN_PROGRESS — **Inc.1 FAIT** (non commité) : `ElementAffinity` +
> `absorb_elemental` (forgia-damage, pur) + genome `[affinity.*]` + le bonus de matchup
> route via `DefenseLayer` (affinité par couche) + sensor `bonus_absorbed`. **259 tests
> verts** (forgia-damage 24 + forgia-mode-roguelite 235), clippy 0-warn sur fichiers
> touchés, binaire `forgia` compile. Auto-QA : verifier (manuel) + qa-lead **VALIDÉ**
> (0 Bloquant/Majeur ; +2 tests anti-régression : exécution bloquée par bouclier plein,
> robustesse mult dégénéré). **Inc.2/Inc.3/Inc.4 RESTENT** (dette tracée : bursts/arc/
> Miasma + hit de base bypassent encore la couche → compteur `bypass_layer_damage` à
> ajouter avec Inc.2).

## Objectif
Router les dégâts **élémentaires** à travers le `DefenseLayer` avec une **affinité par
couche** (au lieu de muter `combat::Health` en direct), pour que chaque élément soit
**fort contre sa couche** et **faible contre les autres** :

| Élément | Couche favorite | Effet (critère masterplan) |
|---|---|---|
| **Feu** | Vie (rouge) | +50 % Vie, −25 % Bouclier/Armure |
| **Électrique** | Bouclier (bleu) | +50 % Bouclier, −25 % autres |
| **Perforant** | Armure (jaune) | +50 % Armure, −25 % autres |
| **Poison** | Armure (jaune) | +50 % Armure, −25 % autres |

**Critère de fin (masterplan)** : tirer du Feu fait +50 % sur la barre rouge et −25 %
sur la bleue ; les 228 tests élémentaires passent toujours.

## Constats concept-first (cartographie)
- **Bug de couche actuel** : `resolve_target_hit` ([elements.rs](../../crates/forgia-mode-roguelite/src/elements.rs))
  soustrait le bonus de `hp.current` **en direct** → court-circuite le `DefenseLayer`.
  Idem bursts (Combustion/Surcharge), arc électrique, DoT Miasma.
- `DamageChannel` (forgia-damage) = `Physical` | `TrueHealth` seulement. Pas d'affinité.
- **Vuln hit-de-base bloquée** : `StatusShock` vit dans forgia-mode-roguelite → invisible
  à `forgia-fps` (qui applique le hit de base). Nécessite un composant générique
  `Vulnerability` dans `forgia-damage` (relocalisation).

## Incréments (chacun compile + tests verts)
- **Inc.1 — Primitive d'affinité + routage du bonus on-hit** :
  `forgia-damage`: `ElementAffinity{shield_mult,armor_mult,life_mult}` +
  `DefenseLayer::absorb_elemental(raw, &aff) -> leak` (efficacité par couche, pure/testée).
  `forgia-mode-roguelite`: genome `[affinity]` (miroir Default) + le **bonus de matchup**
  (`resolve_target_hit`) draine le `DefenseLayer` de la cible via `absorb_elemental` (au
  lieu de `hp.current -=`). Sensor : affinité effective. Tests.
- **Inc.2 — Cohérence : bursts + arc + Miasma via DefenseLayer** — ✅ **FAIT** (non commité) :
  helper `route_elemental_damage` (couche→Vie, affinité) ; Combustion/Surcharge (burst,
  affinité de `ReactionKind::damage_element`), arc électrique (Shock) et DoT Miasma (Poison,
  routé dans `sys_tick_element_status` — burn/poison restent TrueHealth) drainent la couche.
  `bonus_absorbed`→`elem_absorbed` (couvre tous les canaux routés). 263 tests, clippy 0-warn
  touché, binaire compile. Auto-QA qa-lead **WARN→traité** : §1 ordering `sys_regen_defense`
  `.after(sys_tick_element_status)` (déterminisme story-634) FIXÉ ; §2/§7 documentés (3 passes de
  drain/tir, choix affinité Combustion→Feu) ; §3 **décision balance en attente user** (Miasma gèle
  la régén du bouclier tant qu'il tient — cohérent mais fort, test garde-fou posé). §5 dette
  télémétrie (aoe_hits ne compte pas les voisins de burst). **Le hit de base reporté Inc.3**
  (forgia-fps ne voit pas le mapping élément → même contrainte crate que la vuln).
- **Inc.3 — Vuln hit-de-base (StatusShock, différée P0-3)** : composant générique
  `Vulnerability{mult,secs_left}` dans `forgia-damage`, posé par le hit électrique, lu par
  `forgia-fps` (hit de base ×mult) + par elements.rs. Remplace/complète `StatusShock`. Tests.
- **Inc.4 (optionnel) — Réaction Manipulation** (4e réaction du masterplan) : à spécifier.

## Décisions à trancher (avant Inc.1)
1. **Modèle d'affinité** : efficacité par couche (1 pt `raw` retire `mult` pts de la couche ;
   matché 1.5 / non-matché 0.75). ← recommandé (colle au critère, data-driven).
2. **Périmètre session** : Inc.1 seul (borné, testable) vs Inc.1+2 (cohérence complète) —
   Inc.3 (relocalisation `Vulnerability`) séparé car architectural.

## Hors scope
- HUD segmenté rouge/bleu/jaune (P1). Barre de boss (P1).
- Manipulation détaillée (Inc.4, si retenu).

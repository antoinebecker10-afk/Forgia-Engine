# Fine-Grained Crates (Forgia Rewrite V2) — RÈGLE FONDATRICE

> **Convention V2 : 1 concept = 1 crate.** Antoine a explicitement choisi 237 crates
> fine-grained (vs ~13 V1). Cette architecture est délibérée et structurante :
> chaque crate scaffoldé EXISTE POUR ÊTRE PEUPLÉ, pas pour rester vide.

---

## 1. Règle inviolable

**Avant d'ajouter du code dans un crate existant gros (`forgia-fps`, `forgia-ui`,
`forgia-player`, etc.), vérifier obligatoirement** :

1. **Existe-t-il un crate scaffold déjà créé pour ce concept ?**
   `cargo metadata --no-deps | grep <concept>` ou `ls crates/forgia-<concept>*`
2. **Si oui** → peupler le crate scaffold, PAS le gros crate orchestrator.
3. **Si non** → créer un nouveau crate dédié si concept réutilisable (≥ 2 callers attendus).

## 2. Pourquoi cette règle

- **Convention V2 honorée** : 237 crates fine-grained vs scaffold morts à 16 LOC = dette structurelle silencieuse
- **Isolation tests** : chaque crate testable sans pull tout l'écosystème
- **Compile time** : crate isolé recompile vite quand on modifie juste ce module
- **Réutilisabilité** : pattern V2 prévu pour 5+ modes (FPS, RPG, Platformer, Race, Survival...) qui partagent crates atomiques
- **Lisibilité** : `forgia-fps` doit rester **orchestrator** (~500 LOC max), pas god-crate

## 3. Pattern correct — Orchestrator + atomic crates

```rust
// crates/forgia-fps/src/lib.rs (orchestrator)
pub struct ForgiaFpsPlugin;
impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            forgia_mode_fps_arena::ArenaSpawnPlugin,
            forgia_crosshair::CrosshairPlugin,
            forgia_hitmarker::HitmarkerPlugin,
            forgia_weapon_hitscan::WeaponHitscanPlugin,
            forgia_weapon_viewmodel::WeaponViewmodelPlugin,
            forgia_juice_hit_stop::HitStopPlugin,
            forgia_juice_recoil::RecoilPlugin,
            forgia_mesh_fader::MeshFaderPlugin,
        ));
    }
}
// Aucune logique gameplay ici. Juste wire-up.
```

## 4. Anti-patterns à bannir

- ❌ "Je mets ça dans `forgia-fps` pour aller vite, refacto plus tard" → la dette s'accumule, refacto n'arrive jamais
- ❌ Ignorer les crates scaffold existants → ils restent vides + duplication ailleurs
- ❌ Mettre du gameplay réutilisable dans un crate **mode-specific** (e.g. fire system dans `forgia-fps`) → bots IA / RPG combat ne peuvent pas réutiliser
- ❌ Mettre de l'UI dans `forgia-ui` quand un crate UI-spécifique existe (`forgia-crosshair`, `forgia-hitmarker`, etc.)
- ❌ Créer un nouveau crate **sans** vérifier qu'un scaffold existant convient déjà

## 5. Décision arbre — où mettre du nouveau code

```
Nouveau code à ajouter
  │
  ├─ Concept existe déjà comme crate scaffold ? (ls crates/forgia-<concept>*)
  │   │
  │   ├─ OUI → peupler le crate scaffold ✓
  │   │
  │   └─ NON
  │       │
  │       ├─ Concept réutilisable ≥ 2 callers prévus ? 
  │       │   │
  │       │   ├─ OUI → créer nouveau crate fine-grained ✓
  │       │   │       (add workspace member + dep)
  │       │   │
  │       │   └─ NON → mettre dans le crate caller le plus proche
  │       │
  │       └─ Concept mode-specific uniquement (e.g. FPS-only)
  │           → crate `forgia-mode-<name>` ou crate du mode
```

## 6. Audit régulier obligatoire

Avant chaque livraison majeure, vérifier :

```bash
# Liste les crates scaffold (16 LOC = vide) susceptibles de devoir être peuplés
find crates -name lib.rs -exec wc -l {} \; | awk '$1 <= 20 {print}'
```

Si un crate vide a un nom qui correspond exactement au code qu'on vient d'ajouter
dans un autre crate → **dette détectée, à corriger avant merge**.

## 7. Tier 1 / Tier 2 / Tier 3

Workflow d'évaluation avant code :

- **Tier 1 — Crate scaffold existe** : peupler directement (cohérence V2 immédiate, 0 nouveau crate)
- **Tier 2 — Crate doit être créé, ≥ 2 callers** : créer le crate maintenant, cohérence V2 + reuse futur
- **Tier 3 — Code mode-specific 1 caller** : mettre dans `forgia-mode-<name>` ou crate orchestrator du mode

## 8. Origine de la règle

- **2026-05-14** : Antoine choisit 237 crates V2 (vs plan 13). Décision justifiée :
  visu features + AI nav + uniform debug pattern. Risque burnout documenté.
  Référence : memory `reference_v2_237_crates_decision`.
- **2026-05-16** : Audit dette technique révèle 5 crates scaffold (`forgia-crosshair`,
  `forgia-hitmarker`, `forgia-mode-fps-arena`, `forgia-juice-recoil`, `forgia-juice-hit-stop`)
  à 16 LOC pendant que `forgia-fps` grossit à ~1200 LOC.
  Code mal placé identifié : `CrosshairMode` (forgia-ui), `spawn_arena` (forgia-fps),
  `HitStopState` (forgia-combat), etc.
- Antoine formalise : "**ça doit être une règle fondatrice**" → cette règle.

## 9. Cross-refs

- `reference_v2_237_crates_decision.md` (memory) — justification 237 crates
- CLAUDE.md global Forgia §1 Vision — convention 1 concept = 1 file/module
- `concept-first.md` rule — étape 0 "data ou code" / couche correcte

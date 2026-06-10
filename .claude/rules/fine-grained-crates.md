# Crates fines à la demande (Forgia Rewrite V2) — RÈGLE RÉVISÉE 2026-06-10

> **Doctrine actuelle : 1 concept = 1 crate, créée AU MOMENT DU BESOIN — jamais en réserve.**
>
> ⚠️ Version précédente OBSOLÈTE (« 237 crates scaffold, chaque crate scaffoldé existe pour
> être peuplé ») : le cleanup du 2026-05-26 a supprimé ~200 scaffolds (266 → 62 crates,
> voir [ADR-0002](../../docs/adr/ADR-0002-cleanup-crates-266-to-62.md)) et le ratchet
> `cargo xtask no-scaffold` BLOQUE leur retour. Cette règle est la doctrine d'après.

---

## 1. Règle

**Avant d'ajouter du code dans une crate orchestrator existante (`forgia-fps`,
`forgia-ui`, `forgia-rpg`, etc.), vérifier** :

1. **Une crate dédiée au concept existe-t-elle déjà ?** `ls crates/forgia-<concept>*`
2. **Si oui** → y mettre le code, PAS dans l'orchestrator.
3. **Si non** → créer une crate dédiée SEULEMENT si le concept est réutilisable
   (≥ 2 consommateurs réels ou prévus à court terme). Sinon : module dans la crate
   du mode / le caller le plus proche.

**Interdit** : créer une crate « pour plus tard » sans code réel ni consommateur.
Le ratchet `no-scaffold` (xtask) échoue sur toute crate < 50 LOC sans justification.

## 2. Pourquoi (leçon des deux extrêmes)

- **L'extrême 266 crates (2026-05) a échoué** : 85 % de scaffolds vides = bruit de
  navigation IA, temps de compile workspace, ARCHITECTURE.md mensonger, dette de
  gouvernance. L'audit forensic puis le cleanup l'ont acté.
- **L'extrême god-crate (V1) a échoué aussi** : main.rs 41 KB, god-files intestables.
  L'audit 2026-06-10 pointe encore 4 god-files V2 (forgia-rpg/lib.rs 2 345 LOC).
- **L'équilibre** : crate fine quand le concept est RÉEL et PARTAGÉ ; module local sinon.
  Une crate se justifie par ses consommateurs, pas par son existence.

## 3. Pattern correct — Orchestrator + crates atomiques (inchangé)

```rust
// crates/forgia-fps/src/lib.rs (orchestrator)
pub struct ForgiaFpsPlugin;
impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            forgia_crosshair::CrosshairPlugin,
            forgia_viewmodel::ForgiaViewmodelPlugin,
            // ... wire-up, pas de logique gameplay massive ici
        ));
    }
}
```

L'orchestrator reste fin ; la logique vit dans les crates atomiques testables.

## 4. Arbre de décision — où mettre du nouveau code

```
Nouveau code à ajouter
  │
  ├─ Une crate dédiée au concept existe ? (ls crates/forgia-<concept>*)
  │   ├─ OUI → y mettre le code ✓
  │   └─ NON
  │       ├─ Réutilisable, ≥ 2 consommateurs réels/proches ?
  │       │   ├─ OUI → créer la crate fine maintenant (workspace member + dep) ✓
  │       │   └─ NON → module dans la crate appelante la plus proche ✓
  │       └─ Mode-specific 1 caller → module dans forgia-mode-<name> ✓
```

## 5. Anti-patterns à bannir

- ❌ Crate créée « en réserve » sans code (le ratchet no-scaffold la bloquera)
- ❌ "Je mets ça dans l'orchestrator pour aller vite, refacto plus tard"
- ❌ Gameplay réutilisable enfoui dans une crate mode-specific
- ❌ Laisser grossir un god-file >1 000 LOC au lieu d'extraire les modules mûrs
  (extraction en crate seulement si ≥ 2 consommateurs ; sinon simple split en modules)

## 6. Gardes mécaniques

- `cargo xtask no-scaffold` — bloque le retour des crates vides (protège le cleanup)
- `cargo xtask arch-drift` — ARCHITECTURE.md doit lister exactement les members réels
- Audit god-files : `find crates -name "*.rs" -exec wc -l {} \; | awk '$1 > 1200'`

## 7. Historique

- **2026-05-14** : décision initiale 237 crates fine-grained (réservation de namespaces).
- **2026-05-19** : audit forensic — 220 scaffolds <50 LOC sur 258 crates.
- **2026-05-26** : cleanup 266 → 62 crates (-77 %), ratchet no-scaffold ([ADR-0002]).
- **2026-06-10** : audit complet — cette règle était restée à l'état pré-cleanup et
  induisait les sessions IA en erreur ; réécrite (story-593, M1.3).

## 8. Cross-refs

- [ADR-0002 — cleanup crates](../../docs/adr/ADR-0002-cleanup-crates-266-to-62.md)
- `concept-first.md` — étape 0 data/code, couche correcte
- ARCHITECTURE.md — état réel des crates (gardé par arch-drift)
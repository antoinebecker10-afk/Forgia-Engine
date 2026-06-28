# Verify — Verification post-implementation

Verifie l'etat du code apres implementation :

**Contexte** : $ARGUMENTS

## Instructions

Execute ces checks en parallele :

### 1. Compilation
```bash
cargo check -p forgia-game
cargo clippy --workspace -- -W warnings
```
Resultat attendu : 0 errors, 0 warnings.

### 2. Scope drift
Compare les fichiers modifies (git diff --name-only) avec la liste "Fichiers autorises" du plan.
Signale tout fichier modifie qui n'etait pas dans le plan.

### 3. Stability Locks
Utilise l'outil MCP `check_stability_locks` pour verifier qu'aucun Lock n'est viole.

### 4. No-hardcode check
Grep les fichiers modifies pour des literals suspects :
- Nombres magiques dans le gameplay (pas dans UI layout)
- Chemins d'assets en dur (hors asset_paths.rs)
- Couleurs hardcodees (hors theme.rs)

### 5. Rapport final
```
## Verification Report
- Compilation: OK/FAIL
- Clippy: OK/FAIL (N warnings)
- Scope drift: OK/DEVIATION (fichiers hors plan)
- Locks: OK/VIOLATION (details)
- Hardcode: OK/FOUND (details)
- Status: PRET A COMMIT / CORRECTIONS REQUISES
```

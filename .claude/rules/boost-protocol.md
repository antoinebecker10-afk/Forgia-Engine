# Boost Protocol (Forgia)

Quand l'utilisateur dit **"Boost"**, executer ce protocole :

## Phase 0 : Nettoyage disque
- `cargo clean` si target/ > 20 Go (libere ~50-100 Go)
- Detecter les copies recursives (dossiers imbriques, ex: `assets/models/**/Forgia/`)
- Signaler si le repo depasse 5 Go hors target/
- Supprimer les fichiers temporaires (.tmp, *.bak, build artifacts orphelins)

## Phase 1 : Audit code
- Scanner : code mort, duplications, warnings clippy
- Identifier systemes sans `run_if`
- Verifier Stability Locks (aucune violation)
- Audit unwrap/as casts dans hot paths

## Phase 2 : Propositions
- Lister ameliorations par priorite + risque
- Plan d'execution ordonne

## Phase 3 : Refactoring
- Executer les modifications approuvees une par une
- `cargo check` apres chaque modification
- Annuler immediatement si regression

## Phase 4 : Roadmap
- Mettre a jour le backlog
- Proposer prochains objectifs

**Regle** : chaque phase se termine par `cargo check`. Jamais de phase incomplete.

## Phases d'optimisation (historique)

| Phase | Statut |
|-------|--------|
| 1-7 | FAIT (code mort, duplications, run_if, GameAssets, raycasts, cache UI, split modules) |
| 8 | A FAIRE (optimisations avancees Bevy) |
| 9 | FAIT (clippy 0 warnings) |
| 10 | A FAIRE (ameliorations architecturales) |
| 11 | A FAIRE (ameliorations UX) |

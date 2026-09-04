## Story

<!-- story-NNN-slug si applicable, sinon "quick fix : <description>" -->

## Résumé

<!-- 1-2 phrases : ce qui change et pourquoi -->

## Checklist (BMAD)

- [ ] `cargo check --workspace` vert
- [ ] `cargo clippy --workspace -- -D warnings` 0 warning
- [ ] `cargo test -p <crate touchée>` vert
- [ ] Concept-First appliqué (`CLAUDE.md` §2)
- [ ] Tests headless ajoutés si applicable (`#[cfg(test)] mod tests`)
- [ ] Sensor `forgia2_<feature>.json` ajouté si nouvelle feature observable
- [ ] Health alert avec next-step si échec silencieux possible
- [ ] 0 hardcode gameplay (genome ou FpsTuning sinon)
- [ ] 0 `#[allow(dead_code)]` ajouté
- [ ] Stability Lock touché ? Listé explicitement avec autorisation du mainteneur

## Hot path check (si applicable)

Si le code touche un système qui tourne chaque frame :
- [ ] Query filtrée `With<>`/`Without<>`
- [ ] `Changed<T>` ou `Added<T>` si travail conditionnel
- [ ] 0 allocation dans la closure (`Vec::new()`, `HashMap::new()`, `String::new()`)
- [ ] `Local<T>` + `.clear()` pour buffers réutilisés
- [ ] `run_if(condition)` si pas tournant en permanence

## Risques signalés (même hors scope)

<!-- Bugs invisibles, dette future, anti-patterns repérés en passant -->

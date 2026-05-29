# Story-542 — Plugin double-add guard (panic-risk P0)

**Status** : DRAFT
**Priorité** : 🔴 P0 — panic potentiel runtime
**Scale BMAD** : Quick (≤3 fichiers)
**Origine** : audit wiring 2026-05-27 (`docs/audit/wiring-2026-05-27.md` §3)

## Problème

Bevy 0.18 panic si un même Plugin est `add_plugins(...)` deux fois sans guard `is_plugin_added::<>()`. Deux Plugins critiques sont actuellement double-câblés sans protection :

1. **ForgiaDamagePlugin** — `forgia-game/src/lib.rs:65` + `forgia-ai-arena-bot/src/lib.rs:167`. Chaîne game → mode-fps-arena → ai-arena-bot = transitive double-add.
2. **ForgiaJuiceRecoilPlugin** — `forgia-combat/src/lib.rs:135` + `forgia-fps/src/lib.rs:301`. Game wire combat ET fps directement.

## Critères d'acceptation

- [ ] AC1 — Ajouter `is_plugin_added::<ForgiaDamagePlugin>()` guard sur `forgia-ai-arena-bot/src/lib.rs:167` (le bot est sub-system, game est ownership principal)
- [ ] AC2 — Ajouter `is_plugin_added::<ForgiaJuiceRecoilPlugin>()` guard sur `forgia-fps/src/lib.rs:301` (combat est ownership principal)
- [ ] AC3 — `cargo check -p forgia-game` clean
- [ ] AC4 — `cargo clippy -p forgia-ai-arena-bot -p forgia-fps --no-deps` 0 warning
- [ ] AC5 — Test runtime : lancer Arena mode → pas de panic au boot

## Référence pattern

[reference_plugin_idempotent_guard.md](../../../../d--Forgia/memory/feedback_plugin_idempotent_guard.md) — règle "Plugins shared/foundational → TOUJOURS `is_plugin_added::<X>()` guard".

## Test in-game recap

1. **Action** : `cargo run -p forgia-game --profile release-fast` puis sélectionner Arena depuis menu
2. **Pas de redémarrage** entre les 2 tests si compile incrementale OK
3. **Effet attendu** : boot Arena sans panic `Plugin already added`
4. **Sensor** : `forgia2_diagnostics.json` → pas d'erreur boot
5. **Variantes si KO** : si panic persiste, vérifier guard syntactiquement correct (`if !app.is_plugin_added::<...>() { app.add_plugins(...); }`)

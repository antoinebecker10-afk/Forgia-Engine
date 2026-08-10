# story-679 — Manette au menu : valider ce qui a été livré sans manette

**Statut** : BLOCKED (matériel absent)
**Niveau BMAD** : Quick
**Origine** : AC6 de [story-678](story-678-hub-premium.md), sorti de son scope le 2026-08-06
**Crates** : forgia-ui (`gamepad_nav.rs`)

## Pourquoi cette story existe

La navigation manette du menu a été écrite le 2026-08-05 (`gamepad_nav.rs`, phase 6 de
story-678). Elle compile, elle est clippy-propre, et **personne ne l'a jamais essayée** :
Antoine ne joue pas à la manette pour l'instant.

Laisser AC6 coché aurait été une DONE fictive — le cas exact que
`.claude/rules/story-done-gate.md` interdit. Laisser story-678 ouverte sur du matériel
absent aurait bloqué sept critères pour un huitième invérifiable. D'où cette story
séparée, explicitement **BLOCKED**, qui porte la dette sans la cacher.

## Ce qui est en place (non vérifié)

- La manette est **traduite en clavier egui** : D-Pad → flèches, A → Entrée, B → Échap,
  première pression directionnelle → Tab pour amorcer le focus.
- Injection entre `ProcessInput` et `BeginPass` (hook documenté bevy_egui).
- LB/RB = changement d'onglet, avec le son `Tab`.
- Anneau de focus doré (`sys_style_gamepad_focus`).
- Barre de hints affichée **seulement** quand la dernière entrée vient de la manette
  (`LastInputKind`).

## Limites assumées de la v1

- **D-Pad seul** : les sticks demandent des timers de répétition (non écrits).
- **Menu seul** : ni le menu pause, ni le coffre.

## Acceptance Criteria

- [ ] AC1 : parcourir les 9 onglets à la croix directionnelle, l'élément visé porte
      visiblement l'anneau doré
- [ ] AC2 : A valide, B revient — sur l'accueil, l'Enclume et le Forgeron au minimum
- [ ] AC3 : LB/RB changent d'onglet et jouent le son `Tab`
- [ ] AC4 : la barre de hints n'apparaît qu'en manette et disparaît dès qu'on touche la souris
- [ ] AC5 : **la souris reste intacte** — aucune régression du parcours clavier/souris
- [ ] AC6 : le carrousel de chapitres de l'accueil est atteignable et actionnable

## Ce qui débloque

Une manette branchée. Rien d'autre — aucun développement n'est prévu avant l'essai :
tenter d'améliorer à l'aveugle du code jamais exécuté ajouterait des hypothèses aux
hypothèses.

## Cross-refs

- [story-678](story-678-hub-premium.md) — le hub premium, dont AC6 est issu
- `.claude/rules/story-done-gate.md` — pourquoi on ne coche pas sur la foi du code
- `crates/forgia-ui/src/gamepad_nav.rs` — l'implémentation à valider

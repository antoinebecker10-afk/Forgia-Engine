# story-700 — E1 inc.1 : le navmesh existe, compile et répond

**Statut** : REVIEW (2026-08-12 — livré, compilé, 11 tests verts, clippy 0 ; reste la validation en jeu)
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (crate neuve + génome + câblage workspace)
**Origine** : [REFONTE_GDD.md](../REFONTE_GDD.md) Phase 1 — épic **E1 Compagnon**.
**Prouve** : hypothèse **H2** du [GDD](../design/gdd-forgia-the-spared.md) §14.
**Débloque** : le compagnon (E1), le remède structurel aux mobs coincés
([spawn-clearance.md](../../.claude/rules/spawn-clearance.md) §5), et à terme les sbires de E10.

---

## Le problème

`forgia-ai-arena-bot` avance en ligne droite vers sa cible et pousse dans les colliders. Un mob
né derrière un pilier ne se débloque **jamais** : ce n'est pas un ralentissement, c'est un ennemi
retiré du combat, et une vague qui ne se nettoie pas si le joueur ne va pas le chercher.

Le compagnon de E1 aggrave le défaut au lieu de le subir : un mob coincé à 40 m passe inaperçu,
un compagnon dont le point ne bouge plus sur la minimap se voit **toutes les trois secondes**.

## H2 — prouvée, à deux niveaux

Le GDD listait H2 comme *« veille OK, prototype E1 = preuve »*. La preuve est faite :

| Niveau | Résultat |
| --- | --- |
| **Résolution** | `vleue_navigator 0.15.0` + `polyanya 0.16.1` contre **un seul** `bevy_ecs 0.18.1` — aucun doublon de version, 564 paquets |
| **Compilation** | `cargo check -p forgia-navmesh` — 58,7 s, propre |
| **Exécution** | 11 tests headless verts, dont deux qui interrogent un vrai maillage |

**Sans migrer vers bevy 0.19** — le report de migration reste intact (revérifié : `bevy_rapier3d`
sans release depuis 0.35.0, `rapier3d` en 0.35.0-**beta**, donc toujours pré-release).

## Ce qui est livré

- **`crates/forgia-navmesh/`** — convertit des solides (`forgia_core::layout`) en maillage polyanya
  et répond « chemin de A à B ». Pas de système Bevy, pas de composant de suivi.
- **`assets/genomes/navmesh.toml`** — rayon d'agent, ressaut franchissable, qualité d'approximation.
- **Câblage workspace** — member + `[workspace.dependencies]`, `default-features = false`
  (une lib n'impose pas `bevy_gizmos`).

### Trois décisions de conception

1. **Le seuil de navigation n'est PAS celui du couvert.** Réutiliser `SolidDisc::breaks_sight()`
   (1,80 m) était le réflexe — c'était sous la main, et c'est faux. Un muret d'un mètre ne masque
   personne et arrête pourtant un agent **qui ne saute pas**
   ([map-design-intention.md](../../.claude/rules/map-design-intention.md) §2.5). La moitié des
   obstacles serait devenue invisible au maillage. Prédicat retenu : `h > step_height_m` (0,45 m,
   `MaxStepHeight` d'Unreal). Un test dédié interdit la confusion des deux seuils.
2. **On sur-approxime toujours.** Les disques deviennent des polygones **circonscrits**, jamais
   inscrits : un polygone inscrit laisserait des chemins mordre dans l'obstacle. Un agent qui
   contourne large coûte un détour ; un agent qui frotte coûte un blocage.
3. **L'API prend un bord quelconque, pas un `ArenaGeometry`.** L'Expédition (Phase 2) n'a pas
   d'arène hexagonale et doit réutiliser la fonction telle quelle.

Plus : `BuildReport::is_blind()` — **zéro obstacle mesuré n'est pas un succès**
([map-design-patterns.md](../../.claude/rules/map-design-patterns.md) §13). Le rapport le dit au
lieu de laisser croire à une arène dégagée.

## Critères d'acceptation

- [x] `vleue_navigator` résout et compile contre bevy 0.18.1, sans doublon dans le lock
- [x] Crate `forgia-navmesh` créée, membre du workspace, `cargo check` propre
- [x] Zéro littéral numérique dans le code — tout vient de `assets/genomes/navmesh.toml`
- [x] Lecture du génome via `forgia_core::def_io` (jamais `std::fs` — échec silencieux sur wasm)
- [x] Un test compare le TOML au repli Rust : s'ils divergent, il casse
- [x] Le seuil de navigation est testé distinct du seuil de couvert
- [x] Un test prouve que la dilatation **ferme** un passage plus étroit que l'agent
- [x] `cargo test -p forgia-navmesh` — **11 passés, 0 échec**
- [x] `cargo clippy -p forgia-navmesh --all-targets` — **0 warning** (vrai cargo, pas RTK)
- [ ] **Validation en jeu** — aucun consommateur n'est encore câblé (cf. inc. suivants)

## Ce que cette story ne fait PAS

Volontairement hors scope, chacun étant un incrément à part :

- **Personne ne consomme le maillage.** `forgia-ai-arena-bot` avance toujours en ligne droite.
- **Le désenlisement** d'un agent déjà coincé — chien de garde séparé, à livrer **avec** le
  compagnon et non après, puisque la carte le rendra visible.
- **La régénération par chunk** d'un terrain streamé — Phase 2.
- **L'évitement dynamique** entre agents — le maillage est statique.

## Suite

| Inc. | Contenu |
| --- | --- |
| **2** | Brancher `ArenaGeometry` → `forgia-navmesh` : ressource maillage reconstruite au bâti du stage, capteur `forgia2_companion.json` (dont **temps bloqué**) |
| **3** | `forgia-ai-arena-bot` suit le chemin au lieu de la ligne droite + chien de garde de désenlisement |
| **4** | Le compagnon : suivre, se poster, barre PV permanente |
| **5** | Le compagnon porte le **second élément** → jalon hérité de [story-697](story-697-reactions-elementaires-jamais-declenchees.md) : les réactions partent en combat ordinaire, plus seulement sur boss |

## Test runtime

Sans objet à cet incrément — **aucun effet observable en jeu**, par construction : rien ne consomme
encore le maillage. La preuve de cet incrément est mécanique (build + 11 tests + clippy), et c'est
exactement ce qu'elle prétend être. Le premier récap de test runtime viendra à l'inc. 3, quand un
bot suivra réellement un chemin.

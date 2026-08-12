# story-701 — `forgia-stage/lib.rs` : 1 912 lignes de code, 7 sujets

**Statut** : DRAFT — plan écrit, découpe non commencée
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (découpe mécanique, 1 crate, zéro changement de comportement)
**Origine** : question d'Antoine le 2026-08-12 — *« forgia-stage ne serait-il pas trop gros ?
Regarde layout, ça mériterait pas une nouvelle archi ? »*

---

## Le diagnostic, et il contredit l'intuition de départ

La mesure brute (`wc -l`) désigne `layout.rs` comme deuxième plus gros fichier de la crate.
**C'est un faux positif.** Un fichier Rust compte ses tests dedans :

| Fichier | Total | **Code** | Tests | Verdict |
| --- | --- | --- | --- | --- |
| **lib.rs** | 2 263 | **1 912** | 350 | 🔴 le god-file |
| layout.rs | 1 383 | **582** | 800 | ✅ sain |
| floor_merge.rs | 844 | 557 | 286 | ✅ |
| rooms.rs | 852 | 488 | 363 | ✅ |
| layout_sensor.rs | 646 | 426 | 219 | ✅ |
| graph.rs | 922 | **106** | 815 | ⚠️ ratio 1:8, à regarder |

### `layout.rs` ne doit PAS être touché

582 lignes de code, 800 de tests, **pur et headless** — aucune dépendance `Commands` ni
`AssetServer` — avec ses six invariants documentés en tête et leurs sources (COD WW2,
Level Design Book). C'est le fichier le mieux architecturé de la crate. Le découper
casserait une frontière propre pour satisfaire un compteur.

### Le capteur de dette est lui-même défectueux

La détection de god-files du projet ([`fine-grained-crates.md`](../../.claude/rules/fine-grained-crates.md) §6)
est `wc -l > 1200`. Elle **ne sépare pas le code des tests** : elle accuse `layout.rs` qui
va très bien, et raterait un fichier de 1 150 lignes de code pur sans un seul test.

C'est la classe de défaut de [story-699](story-699-un-capteur-a-zero-ne-doit-pas-dire-ok.md)
appliquée à la métrique de dette : **l'instrument ment**. À corriger avec cette story.

## La crate, elle, ne doit pas être éclatée

`forgia-stage` fusionne bien deux sujets sans rapport — chargeur d'arène et graphe de run
DAG — fusion actée au cleanup 266 → 62 crates ([ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md)).
Conceptuellement impur, **mécaniquement justifié** : `graph.rs` ne fait que 106 lignes de
code, en faire une crate déclencherait le ratchet `no-scaffold`. On n'y touche pas.

## Les 7 sujets de `lib.rs`

| Lignes | Sujet | Nature |
| --- | --- | --- |
| 58-484 | 18 structs/enums de données + `splitmix64` | Types |
| 485-593 | Le plugin et son câblage | Orchestration |
| 594-650 | Chargement des génomes + préchargement de scènes | Systèmes |
| 650-800 | **Le capteur `forgia2_stage`** + severity/next_step | Observabilité |
| 802-956 | Géométrie hexagonale des remparts + ancres POI | **Pur, headless** |
| 958-1030 | Éclairage et glow par biome | **Pure table** |
| 1031-1912 | Systèmes de spawn de l'arène | Systèmes |

### L'incohérence qui saute aux yeux

`layout_sensor.rs` a été extrait dans son propre fichier. **L'autre capteur de la même
crate est resté dans `lib.rs`.** Deux capteurs, deux traitements, aucune raison.

## Le plan

Quatre modules, tous déjà cohésifs — découpe **mécanique**, zéro changement de comportement :

| Module | Contenu | ~LOC | Gain |
| --- | --- | --- | --- |
| `defs.rs` | Les 18 structs/enums + `splitmix64` | 330 | Lisibilité |
| `stage_sensor.rs` | Capteur + severity/next_step | 150 | **Corrige l'incohérence** |
| `ramparts.rs` | Géométrie hexagonale + ancres POI | 155 | **Testable headless** |
| `biome_look.rs` | Lighting/glow par biome | 75 | **Testable headless** |

`lib.rs` retomberait à **≈ 1 200 lignes de code** : le plugin, le chargement, les systèmes
de spawn. Trois modules purs deviendraient testables isolément.

Recette existante : mémoire `reference_decoupe_mecanique_god_file` — sed bas→haut,
en-tête générique, `cargo fix`, puis **preuve que le câblage est identique à HEAD**.

## Critères d'acceptation

- [ ] `lib.rs` ≤ 1 250 lignes de code (hors tests)
- [ ] Les 4 modules créés, chacun avec son en-tête expliquant **ce qu'il ne fait pas**
- [ ] `ramparts.rs` et `biome_look.rs` testés **headless** (aucune `App` Bevy montée)
- [ ] `cargo test -p forgia-stage` — même nombre de tests qu'avant, tous verts
- [ ] `cargo clippy -p forgia-stage --all-targets` — 0 warning (**vrai cargo**, RTK masque)
- [ ] **Preuve de non-régression** : le `git diff` ne montre aucun changement de logique,
      uniquement des déplacements et des `pub use`
- [ ] `xtask arch-drift` + `sensor-audit` toujours verts
- [ ] La détection de god-files du projet sépare code et tests (corrige l'instrument)
- [ ] `layout.rs` **non modifié** — c'est un critère, pas un oubli

## Ce que cette story ne fait PAS

- **Éclater la crate** — justifié mécaniquement, cf. ci-dessus
- **Toucher `layout.rs`** — il va bien
- **Changer le moindre comportement** — si le jeu bouge, la découpe est ratée
- **Traiter `graph.rs`** (106 LOC pour 815 de tests) — anomalie réelle mais autre sujet :
  sur-test, ou module vidé dont les tests sont restés orphelins ? À trancher séparément.

## Priorité

**Basse.** C'est de la dette de lisibilité, pas un défaut fonctionnel. Elle ne bloque
aucune phase de [la refonte](../REFONTE_GDD.md) — mais `forgia-stage` est sur le chemin de
E1 (le navmesh y est bâti, [story-700](story-700-navmesh-fondation-compagnon.md) inc.2),
donc chaque incrément suivant paiera un peu de ce god-file.

⚠️ `forgia-stage` est une crate que l'autre terminal peut toucher : **coordonner avant de
démarrer** (`multi-terminal-coordination.md` §3).

## Cross-refs

- [story-700](story-700-navmesh-fondation-compagnon.md) — le navmesh, qui vit en partie ici
- [story-699](story-699-un-capteur-a-zero-ne-doit-pas-dire-ok.md) — même classe : un
  instrument qui ne mesure pas ce qu'il prétend
- `reference_decoupe_mecanique_god_file` (mémoire) — la recette éprouvée
- [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md) — pourquoi la crate est fusionnée

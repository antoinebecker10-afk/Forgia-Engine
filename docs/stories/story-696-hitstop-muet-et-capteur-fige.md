# story-696 — Le hitstop est retiré définitivement (et il l'était déjà à moitié)

**Statut** : DONE (2026-08-12) — décision prise : le hitstop est **retiré définitivement**
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (une feature à restaurer + un défaut de classe à instruire)
**Origine** : run de validation du 2026-08-12. La première version accusait le
hitstop de « ne pas se déclencher ». La mesure dit tout autre chose.

---

## Ce que disait la v1, et pourquoi c'était faux

> *« `hitstop_counts: {hit:0, crit:0, kill:0, multikill:0}` sur une run à 51 kills.
> Le hitstop ne part jamais. »*

Deux erreurs, découvertes en écrivant `tools/ai/phase0_check.py` :

1. **`forgia2_gamefeel.json` date du 2026-07-21** — vingt-deux jours. Les zéros lus
   et relus toute la journée sont un artefact de juillet, pas une mesure.
2. **Aucun code n'écrit ce fichier.** C'est un **orphelin** : son producteur
   n'existe plus. Le lire revenait à interroger un mort.

## Ce qui s'est réellement passé

| Date | Événement |
|---|---|
| **2026-05-17** | `HitStopState` et `hitstop_tick_system` **migrés** de `forgia-combat` vers une crate dédiée `forgia-juice-hit-stop` (Tier 1D) |
| **2026-05-26** | [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md) supprime ~200 crates. `forgia-juice-hit-stop` en fait partie — **neuf jours après la migration** |
| depuis | La feature n'existe plus. Les commentaires, si |

Preuves, toutes vérifiables en une commande :

- `crates/forgia-juice-hit-stop` — **absente**
- `forgia-juice-lib/src/` contient `camera_shake`, `fov_punch`, `knockback`,
  `recoil` — **pas de `hit_stop.rs`**
- **zéro dépendance** `juice-hit-stop` dans tout le workspace
- `grep -rn "HitStopState\|hitstop_tick"` sur `crates/` → **uniquement des commentaires**

Et ces commentaires **mentent encore aujourd'hui** :

```rust
// combat_juice.rs:152  HitStopState : migré vers `forgia-juice-hit-stop` (Tier 1D).
// combat_juice.rs:197  Wiring : `forgia_juice_lib::hit_stop::ForgiaJuiceHitStopPlugin`
//                      ajouté idempotent dans `ForgiaCombatPlugin`.
// lib.rs:59            Importer directement : `use forgia_juice_lib::hit_stop::HitStopState;`
```

Aucun de ces trois chemins n'existe. Un lecteur — humain ou IA — qui les croit
conclura que le hitstop est câblé.

> **Réserve honnête** : une réimplémentation sous un autre nom n'aurait pas été
> attrapée par ces greps. Avant de reconstruire, chercher le **concept** (gel du
> temps à l'impact), pas le nom `hit_stop` — c'est la leçon de story-626.

## Le vrai sujet : la classe de défaut

Ce n'est pas un bug de gameplay, c'est une **perte silencieuse au cleanup**. ADR-0002
a supprimé ~200 crates, dont la quasi-totalité étaient des scaffolds vides — mais au
moins une contenait du code vivant, et rien ne l'a signalé.

Mesuré le 2026-08-12 : **44 crates supprimées sont encore citées dans le code**.
La plupart sont des notes historiques inoffensives (`forgia-juice-camera-shake` est
cité dans `juice-lib/src/camera_shake.rs` — le code y est bien). Le danger est le
cas `hit_stop` : **cité, mais sans fichier d'accueil**. Ce sont ceux-là qu'il faut
lister.

C'est directement le risque n°1 de [`REFONTE_GDD.md`](../REFONTE_GDD.md) §6
(« reconstruire ce qui existe »), pris **par l'autre bout** : reconstruire ce qui a
été supprimé sans le savoir, ou pire, croire que ça tourne encore.

## Critères d'acceptation

- [x] La cause est **nommée et prouvée** — crate supprimée par ADR-0002, capteur
      orphelin depuis le 2026-07-21, zéro producteur dans le code
- [x] **DÉCISION PRISE — 2026-08-12, Antoine : « on le retire, je n'en veux pas,
      jamais. »**
      Ce n'est pas un manque à combler, c'est un **choix de game feel**. Et il
      **réaffirme** un choix déjà pris le 2026-07-20 pour une raison mesurée :
      *micro-freezes ressentis en tir soutenu, invisibles aux capteurs de
      frame-time*. Un gel par-hit à 16 tirs/s ne se lit pas dans une moyenne de
      frame — il se sent. C'est aussi ce qui explique que la suppression de mai
      soit passée inaperçue : la feature était déjà éteinte en data.
- [x] Le hitstop est **retiré de la couche definition** : 3 gènes
      `wfx_hit_stop_ms` / `_threshold_dmg` / `_scale`, sans aucun lecteur, effacés
      des **deux** génomes. `validate-genomes` : 135 fichiers, 1883 gènes, OK.
- [x] Les commentaires menteurs sont **devenus des interdits explicites** —
      `combat_juice.rs` (×2), `combat-lib.rs`, `knockback.rs` (×2). Ils nomment la
      décision, sa date, son motif, et interdisent la recréation **sous un autre
      nom** (« freeze frame », « impact pause », « gel d'impact »).
- [x] `forgia2_gamefeel.json` orphelin **supprimé du dépôt de travail**
- [ ] **Inventaire** des 44 citations de crates supprimées, séparées en deux :
      note historique inoffensive / **référence sans code d'accueil**.
      *Seul AC restant — c'est la partie « classe de défaut », indépendante du
      hitstop lui-même.*

## Conséquences sur d'autres tickets

- **story-698** (kill sans burst ni son) invoquait « les ingrédients livrés
  (648/650/655) ». Le 648 était les **paliers de hitstop** — donc un des trois
  ingrédients du « kill satisfaisant » n'existe plus. Sa prémisse est à corriger.
- **story-699** gagne un quatrième mode de défaillance des capteurs : au-delà du
  chien de garde (fraîcheur) et de la sévérité vide (vacuité), un **fichier
  orphelin sans producteur** se lit comme une donnée vivante.

## Cross-refs

- [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md) — le cleanup 266 → 62
- `tools/ai/phase0_check.py` — l'outil qui a révélé l'orphelin
- `.claude/rules/fine-grained-crates.md` — la doctrine d'après le cleanup
- story-699 (capteurs qui mentent) · story-698 (kill satisfaisant)

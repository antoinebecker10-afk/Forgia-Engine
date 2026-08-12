# Ship P0 — première vérification du distribuable (2026-08-12)

> Item ROADMAP § NOW : « Lancer `scripts/build-dist.ps1` depuis un HEAD propre,
> décompresser le zip ailleurs, vérifier lancement standalone (assets/cwd/capteurs).
> Vérifie **d'un coup** la victoire au runtime (câblée depuis story-571, **jamais testée**). »
>
> Exécuté pour la première fois. **La moitié « standalone » est PASSÉE**, la moitié
> « victoire » reste ouverte. Deux défauts d'empaquetage trouvés — invisibles en dev,
> visibles seulement dans un build joueur.

## Conditions exactes de la mesure

| | |
|---|---|
| HEAD | `0ada0a1` (story-695 inc.4b) |
| Arbre | **pas strictement propre** — 3 fichiers non commités d'un autre terminal (`forgia-streaming/src/lib.rs`, `spawn_budget.rs`, `.github/workflows/ci.yml`). L'arbre **compilait** (check workspace 0 erreur / 0 warning), donc la baseline est saine, mais ce binaire n'est pas reproductible depuis un commit seul. |
| Build | `cargo build -p forgia --release -j 4` (LTO, codegen-units=1) — **12 min 29**, aucun cache release préexistant |
| Sortie | dossier **1 532 Mo** · zip **1 199 Mo** |
| Test | zip décompressé dans `C:\tmp\forgia-dist-test` (hors dépôt), `forgia.exe` lancé depuis son dossier |

## Ce qui PASSE

- **Le jeu lance et se joue** depuis le zip décompressé : entrée en run avec
  **4 135 entités / 337 colliders**, ESC → Pause, « Quitter vers le menu », retour
  au hub, sortie propre. **0 panic.**
- **Racine d'assets relative à l'exe** : avatar rebranché (68 os), diorama
  `forge_crepuscule` avec 17 props, skybox HDR sur 3 caméras.
- **Capteurs écrits dans le cwd du joueur** : **88 fichiers** `forgia2_*.json`.
- **La séparation de crates de la story-694 tient en release** :
  `forgia_ui::menu::shell` et `forgia_menu_hub::*` cohabitent dans le binaire livré.

## Défaut 1 — 3 `ERROR` d'assets dans un build joueur

```
ERROR bevy_asset::server: Path not found: assets\textures-v1/terrain/grass/{diff,normal,roughness}.jpg
```

La denylist de `build-dist.ps1` exclut `textures-v1` (214 Mo) en justifiant
« PBR terrain RPG (forgia-rpg/terrain), arène = sol plat ». La justification est
juste sur le CONSOMMATEUR et fausse sur le TIMING : `forgia-terrain/src/terrain_material.rs`
et `forgia-rpg/src/lib.rs` chargent ces textures **au démarrage**, quel que soit le
mode — les plugins sont câblés dans le binaire.

**Deux remèdes, à trancher :**

- rendre `textures-v1/terrain/grass` à la denylist — **13 Mo** sur 214 exclus, soit
  6 % de l'économie, et les 3 erreurs disparaissent ;
- ou rendre ces chargements conditionnels au mode — plus propre, plus cher.

Non tranché ici : demande de savoir si le sol de l'arène utilise réellement ce
matériau (mesure visuelle, pas déductible du log).

## Défaut 2 — `config/genomes/` n'est pas embarqué du tout

```
WARN [rpg-monitor]        Cannot read config/genomes/rpg_monitor.toml
WARN [forgia-streaming]   no genome at config/genomes/streaming.toml
```

Le script ne copie que `assets/`. La plupart des genomes ont migré vers
`assets/genomes/`, mais ces deux lecteurs pointent encore `config/genomes/`. Ils se
replient proprement (warn, pas crash) — donc **deux systèmes tournent sur leurs
valeurs par défaut, en silence, dans le build livré**. C'est la forme exacte que
`observability-required.md` appelle un échec silencieux.

## Défaut 3 (mineur) — le log du joueur est perdu

`forgia2_run.log` n'existe pas dans le dossier joueur : le log ne part qu'en
**stderr**, et un double-clic n'a pas de console. Les 88 capteurs sont là, mais la
chronologie ne l'est pas. Ça comptera au premier rapport de bug d'un testeur
externe.

## Ce qui RESTE — la victoire

Non vérifiée. Le chemin est identifié :

- **Boucle de chapitre** ([waves.rs:680](../../crates/forgia-mode-roguelite/src/waves.rs#L680)) —
  tuer le boss **est** la fin de run (`RunResult::Victory`).
- **Mode graphe** ([loot_room.rs:939](../../crates/forgia-mode-roguelite/src/loot_room.rs#L939)) —
  boss vaincu **+** parcours bouclé par le portail Retour.

**Pourquoi ce n'est pas un simple « joue jusqu'au bout »** : le boss tombe au round
**10** (~15 min), et `roguelite_rounds.toml` documente qu'un joueur qui ne construit
pas **meurt au round 7 et ne voit jamais le boss**. Un échec ne prouverait donc rien
sur le câblage.

**Le raccourci est prévu par le code**, pas bricolé — `boss_arena_for` documente :
« `max_rounds = 1` (chapitre réduit au boss) donne 0 arène de combat et le boss en
arène 0 ». Et `roguelite_rounds.toml` est en couche definition avec hot-reload 1 Hz.

Un `max_rounds = 1` a été posé **dans la copie standalone** `C:\tmp\forgia-dist-test`
(sauvegarde `.orig` à côté) — **le dépôt reste à 10**. Il suffit de lancer ce dossier,
tuer le boss du round 1, et vérifier :

- écran de Victoire (bouton « Back to Lobby ») ;
- `forgia2_roguelite_state.json` → `Victory`, `victories_total` **incrémenté** ;
- log : `BOSS VAINCU : CHAPITRE TERMINÉ`.

Symptôme à surveiller — il a déjà eu lieu : boss tué, `boss_defeated: true`, mais
`victories_total` figé et « il ne se passe plus rien ». C'est ce que le correctif du
2026-08-04 a réparé pour la boucle de chapitre ; s'il réapparaît, `loop_mode` est faux.

## Coût disque

`dist/` (gitignoré) pèse **2,7 Go** — dossier 1,5 + zip 1,2. À purger quand il n'est
plus utile. Le zip à 1 199 Mo est par ailleurs **lourd pour itch.io** : à trancher
avant publication, indépendamment des défauts ci-dessus.

# Outillage de session (Forgia)

> **Un outil dont personne ne vérifie la fraîcheur finit par répondre sur du
> code mort — et c'est pire que pas d'outil, parce que ça a l'air de marcher.**
> Origine : l'index grepai est resté figé 8 jours (30/07 → 07/08) sans un signal.
>
> ⚠️ Cette règle est **chargée à chaque session**. Elle ne garde donc que ce qui
> change une décision. Le détail — mesures, tentatives ratées, chiffres de perf —
> vit dans `[[reference_outillage_controle_automatique_session]]`, lu à la demande.

## 1. Le contrôle est automatique

| Quand | Hook | Ce qu'il fait |
|---|---|---|
| Ouverture de session | `.claude/hooks/tools-health.sh` | État des outils en une lecture **et relance grepai s'il est mort**. ~1,1 s |
| « mémorise » / « checkpoint de session » | `.claude/hooks/memorise-sync.sh` | Relance grepai si besoin, puis la checklist mémoire |

**Ce qui reste manuel est ce qui demande un jugement** : quoi retenir, où le
ranger, quelle leçon mérite une règle. Aucun script ne tranche ça.

## 2. Les cinq faits opérationnels

1. **`grepai watch --background`** relance le watcher. **`mcp-serve`** est le
   serveur MCP et **n'indexe rien**.
2. **La fraîcheur se lit dans `.grepai/config.yaml` → `watch.last_index_time`**,
   jamais sur la mtime de `index.gob` : une simple recherche la réécrit.
   *(Même piège avec `data_level0.bin` côté mempalace — reproduit le même jour.)*
3. **Un watcher lancé sans ollama démarre puis MEURT.** Tester ollama d'abord :
   la verticale de dépendance doit se lire dans l'ordre du script.
4. **Un seul watcher pour TOUS les terminaux.** `--background` refuse le second
   avec une **sortie 1** — ce n'est pas un échec, c'est l'autre terminal qui a
   gagné la course : re-mesurer `--status`. Et **`--stop` coupe tout le monde**.
5. **Personne n'a à lancer ollama** : son app de barre des tâches le relance
   seule. Le hook délègue la reprise à `grepai-autostart.ps1` sans l'attendre.

## 3. Interroger grepai efficacement

- **En ANGLAIS.** Mesuré : FR « comment le joueur est empêché de naître dans un
  rocher » → faux ; EN « prevent player spawning inside prop collider clearance »
  → `decor.rs`, qui contient `SpawnKeepout`.
- **Avec les identifiants du code** quand la cible est un TOML : `enemy_grunt hp
  speed vision` trouve, la langue naturelle non.
- **Juger par le FICHIER rendu, JAMAIS par le score.** Une question absurde
  (« recette de tarte aux pommes ») rend **0,586** quand une bonne réponse
  plafonne à 0,670. Le score ne discrimine pas.
- `search.hybrid` est **activé** (mesuré : 13/14 contre 11/14, 0 perte).

## 4. L'inventaire

| Outil | Apporte | Tombe en panne quand |
|---|---|---|
| **grepai** | Recherche sémantique du code | Le watcher meurt → index figé **en silence** |
| **mempalace** | Mémoire inter-sessions (~11 400 tiroirs) | Un memory ÉDITÉ après classement garde sa version d'origine |
| **ollama** | Embeddings `nomic-embed-text` | Éteint → grepai n'indexe plus rien |
| **bevy-brp** | Inspection ECS en direct | Sans `--features dev-brp` — **lancer avec `cargo forgia-dev`, jamais `cargo run -p forgia`** |
| `tools/ai/forgia_digest.py` | Logs + 98 capteurs → ~2,5 Ko | **Le réflexe sur « regarde »** |
| `cargo run -p xtask -- <cmd>` | `story-gate`, `no-scaffold`, `arch-drift`… | — |

## 5. Vérifier sans dépendre de ce que je raconte

```bash
grepai watch --status                                    # running, ou pas
sed -n 's/.*last_index_time: *//p' .grepai/config.yaml   # date réelle de l'index
grepai stats                                             # les recherches réellement faites
bash tools/ai/tests/hooks-securite.sh                    # 44 cas — après TOUTE modif de hook
ls "D:/IA Antoine/logs/breakers/"                        # non vide = un hook coupé en SILENCE 1 h
```

`grepai stats` est le seul juge de « est-ce que la recherche sémantique sert
vraiment ». Un compteur plat sur une session de refactor = protocole sauté.

## 5 bis. Lancer le jeu — une seule commande à retenir

```bash
cargo forgia-dev      # Tracy (temps) + BRP (état ECS en direct). LA commande de dev.
cargo run -p forgia   # ❌ ni l'un ni l'autre : on se prive du monde vivant
```

**Pourquoi c'est une règle et pas une préférence.** L'alias BRP existait depuis
des mois et n'a jamais servi une seule fois : chaque récap de test disait
`cargo run -p forgia`. Résultat, le 2026-08-16 — une session à écrire des
capteurs pour des questions ponctuelles (« où est l'os `RightArm` ? ») auxquelles
`mcp__bevy-brp__world_query` répond en un appel, sur le monde qui tourne.

Un outil qui a sa propre commande est un outil qu'on oublie. Il est maintenant
dans celle qu'on tape tous les jours.

## 6. Ce que cet outillage ne couvre PAS

La justesse de l'index (on mesure sa **date**, pas sa **qualité**) · que le
projet compile · les memories périmées · et surtout **que je me serve des
outils** : `trace-callers` est à **0 appel**, alors que c'est le vrai delta de
grepai sur un grep.

## 7. Cross-refs

`log-digest.md` · `session-checkpoint.md` · `multi-terminal-coordination.md` ·
`on-demand/map-design-patterns.md` §13-14 (« 0 mesuré n'est pas vert ») ·
[tools/ai/tests/LISEZMOI.md](../../tools/ai/tests/LISEZMOI.md) (les 5 défauts
que les bancs ont trouvés) · `[[reference_outillage_controle_automatique_session]]`

---

*Adoptée 2026-08-08, resserrée 2026-08-09 : elle pesait 14 468 o — le plus gros
fichier de règles du projet — pour une taxe de contexte à chaque session. Le
détail est parti dans la mémoire. **Une règle se paie à chaque session ; un
memory ne se paie que quand on le lit.***

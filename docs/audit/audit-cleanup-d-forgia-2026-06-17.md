# Audit nettoyage `D:\Forgia` (2026-06-17)

> Demande : « D:\Forgia ne sert plus qu'à piloter, le code est dans `Forgia Rewrite`. Faut-il nettoyer ? Audit précis, orienté best-practices, pas complaisant. »
> Méthode : mesures disque réelles + vérification des références dans la config active (CLAUDE.md, .mcp.json, settings.json, hooks). Aucune suppression effectuée.

---

## Verdict en une ligne

**Oui, il y a un nettoyage à faire — mais 99 % du gain est UNE action zéro-risque (supprimer 65,5 Go d'artefacts de build régénérables), pas « supprimer le V1 ».** Le vrai problème n'est pas le volume : c'est que `D:\Forgia` **mélange trois choses que les bonnes pratiques imposent de séparer** — le cockpit IA actif (~0,6 Go, le *moat* du projet), un legacy V1 de 69 Go, et des déchets. Et fait notable : le cockpit (le moat) **n'est pas sous git**, alors qu'un legacy régénérable de 69 Go, lui, l'est.

---

## 1. Faits mesurés

| Item | Taille | Modifié | Statut réel |
|---|---:|---|---|
| `RUST/` (V1) | **69,4 Go** | 2026-03-24 | dont `target/` = **65,5 Go** (régénérable) ; source+assets+git ≈ 3,9 Go |
| `dist/` | 392 Mo | 2026-03-26 | distribution V1 (`Forgia.exe` copié post-build) — workflow V1 mort |
| `tools/` | 330 Mo | 2026-04-20 | **`forgia-mcp` = ACTIF** (.mcp.json) ; reste = biome_generator/dashboard négligeables |
| `Forgia-dungeon-gen/` | 102 Mo | 2026-04-15 | expérience autonome, ~tout en `target/` propre |
| `docs/` | 95 Mo | 2026-06-17 | **`veille/` actif** ; le reste mixte (vision, ADR vide, vieux raw) |
| `website/` | 70 Mo | 2026-04-25 | site marketing — non référencé par la config active |
| `dashboard.disabled/` | 2,1 Mo | 2026-03-16 | littéralement désactivé |
| `vlm_test/` | 1,9 Mo | 2026-05-05 | expérience VLM — non référencée |
| `tmp_*.png` + `screen.png` | ~7,3 Mo | 2026-03-31 | captures jetables |
| `CUsersAntoiAppDataLocalTempfs_test/`, `Toute`, `game_session.log`, `DAILY_BRIEF.*`, `generate_dashboard.sh`, `watch_errors.sh` | ~0 | mars 2026 | artefacts accidentels / helpers V1 |
| `.claude/` + `CLAUDE.md` + `.mcp.json` | ~0,5 Mo | 2026-06-10 | **cockpit actif — cœur** |
| `production/session-state/` | ~0 | live | **écrit par 5 hooks actifs — NE PAS TOUCHER** |
| **TOTAL** | **~70,4 Go** | | dont **~66 Go régénérables** |

Sauvegardes git (les deux sont sur GitHub → le local n'est qu'un cache) :
- V1 : `origin → github.com/antoinebecker10-afk/forgia.git` (branche `master`, dernier commit 2026-06-04).
- V2 : `origin → github.com/antoinebecker10-afk/Forgia-Rewrite.git`.
- `D:\Forgia` (racine) : **pas un dépôt git** — le cockpit n'est pas versionné à ce niveau.

---

## 2. Ce qui est VIVANT (ne pas toucher) — preuves

- **`.claude/` + `CLAUDE.md` + `.mcp.json`** — le cockpit. Cœur du projet.
- **`tools/forgia-mcp`** — serveur MCP actif (`.mcp.json:5`). *Mais* lit les sensors V1 (cf audit best-practices §3.1) — à re-pointer, pas à supprimer.
- **`production/session-state/`** — `claim-task.sh`, `session-lock.sh`, `session-start.sh`, `terminal-register.sh`, `pre-compact.sh` y écrivent. Infra de coordination multi-terminal **live**. Un audit naïf l'aurait classé « vieux 0 Mo → delete » : ce serait une régression.
- **`docs/veille/`** — alimenté chaque jour par le hook veille (fichier 2026-06-17 présent).
- **`LEXIQUE_ECS.md`** (racine) — référencé par `CLAUDE.md §4` (« Reference complete »). Contenu daté V1 (2026-03-08) mais lien actif → garder, à rafraîchir un jour.
- **`.bmad` (junction)** — pointe dans `RUST/.../.bmad`, `config.yaml` valide, utilisé par les skills BMAD. **Dépendance vivante du cockpit logée DANS le legacy.**

---

## 3. Régénérable = zéro-risque (le vrai gain : ~66 Go)

`target/` n'est jamais « conservé » : il se reconstruit avec `cargo build`. Rien n'est perdu, le code V1 est sur GitHub.

- `RUST/Forgia/Forgia/target` → **65,5 Go**
- `Forgia-dungeon-gen/target` → ~0,1 Go
- `tools/forgia-mcp/target` (part des 330 Mo, garder l'exe release)
- `RUST/.../node_modules` → 0,02 Go

**Nuance non-complaisante (à trancher par toi) :** les sensors V1 ont été réécrits **hier (2026-06-16)** → tu **lances encore le V1**. Supprimer son `target` n'efface aucune donnée, mais impose un **rebuild complet (~15-30 min)** au prochain lancement V1. Donc :
- Si le V1 est un **banc de pièces que tu relances** → garder son `target` est légitime (c'est le prix du cache).
- Si le V1 est **archivé** (tu lis le code, tu ne le run plus) → supprime les 65,5 Go, c'est le plus gros gain zéro-perte du projet.

C'est la décision centrale de l'audit, et elle t'appartient — pas de réponse « par défaut ».

---

## 4. Legacy V1 (`RUST/`, hors target) — découpler PUIS archiver

Le V1 source (~3,9 Go) est sauvegardé sur GitHub. Best practice : **ne pas supprimer un legacy encore branché**. Aujourd'hui le cockpit en dépend par 3 fils :
1. `.bmad` junction → `RUST/.../.bmad`
2. grepai indexe `RUST/.../Forgia` (`.mcp.json:31`)
3. hooks/règles citent des chemins V1 (`concept-first-gate.sh`, `validate-commit.sh`, `data-driven-paths.md`…)

➜ Ordre correct : **d'abord couper ces 3 fils** (= le P0 V1→V2 de l'audit best-practices), **ensuite seulement** le V1 local devient supprimable sans rien casser (il reste sur GitHub, re-clonable à la demande). Tant que les fils ne sont pas coupés, on garde l'arbre (sans `target`).

---

## 5. Junk / hygiène (petit volume, mais propreté)

Sans valeur, non référencés par la config active :
- `tmp_Capture*.png`, `tmp_sky.png`, `screen.png` (~7 Mo de captures)
- `CUsersAntoiAppDataLocalTempfs_test/` (nom = chemin temp mal échappé → artefact accidentel)
- `Toute`, `game_session.log`, `DAILY_BRIEF.html`, `DAILY_BRIEF.md`
- `generate_dashboard.sh`, `watch_errors.sh` (helpers shell V1)
- `dashboard.disabled/` (désactivé), `.claude-memory-backup/` (backup mémoire de mars, l'actif est dans `~/.claude/projects/d--Forgia/memory/`)

Expériences V1 autonomes (archiver, pas supprimer en place tant que non re-vérifiées une par une) : `dist/`, `website/`, `vlm_test/`, `mcp_bridge/`, `Forgia-dungeon-gen/`.

---

## 6. Le vrai problème (best-practice, pas symptôme)

Supprimer des fichiers traite le symptôme. La cause : **`D:\Forgia` conflate trois rôles** que la séparation des préoccupations veut distincts :

| Rôle | Contenu | Poids | État souhaité (best-practice) |
|---|---|---:|---|
| **Cockpit / gouvernance** (le *moat* du projet, CLAUDE.md §1) | `.claude/`, `CLAUDE.md`, `docs/`, `tools/forgia-mcp`, mémoire | ~0,6 Go | **dépôt git propre et versionné**, c'est l'actif #1 |
| **Legacy V1** | `RUST/` | 69 Go | **distant GitHub + clone local slim optionnel**, pas imbriqué dans le cockpit |
| **Déchets / expériences** | tmp, dist, website, vlm_test… | ~0,6 Go | supprimés ou archivés hors-ligne |

Anomalie révélatrice : **le cockpit (l'actif stratégique) n'est pas sous git, mais le legacy régénérable de 69 Go, lui, l'est.** C'est inversé. La meilleure « action de nettoyage » à terme n'est pas `rm`, c'est : **faire du cockpit un petit dépôt git propre**, et sortir le V1 du dossier (il vit déjà sur GitHub).

Référence : séparation des préoccupations ; « infra/config as a clean, versioned repo » ; un artefact stratégique non versionné = bus-factor de 1 sur le moat lui-même.

---

## 7. Plan de nettoyage par tiers de risque

| Tier | Action | Gain | Risque | Pré-requis |
|---|---|---:|---|---|
| **T0** | Supprimer les `target/` (RUST + dungeon-gen + node_modules) | **~66 Go** | Zéro (régénérable, V1 sur GitHub) | Décider §3 : V1 archivé vs encore run |
| **T1** | Supprimer le junk §5 (tmp, Toute, fs_test, logs, briefs, helpers) | ~10 Mo | Zéro | — |
| **T2** | Archiver les expériences mortes (`dist`, `website`, `vlm_test`, `mcp_bridge`, `dashboard.disabled`, `.claude-memory-backup`) vers un `_archive/` externe ou suppression (toutes régénérables/sauvegardées) | ~470 Mo | Faible | vérif 1-par-1 |
| **T3** | Couper les 3 fils V1 (`.bmad`, grepai, hooks) → puis sortir/archiver le V1 source | ~3,9 Go | Moyen | = P0 audit best-practices |
| **T4** | Faire du cockpit un dépôt git propre ; le V1 ne vit que sur GitHub | structurel | — | T3 fait |

> Garde-fous permissions en place (sains) : `settings.json` **interdit déjà** `cargo clean*` et `rm -rf D:*`. La suppression de `target` se fera donc via `Remove-Item -Recurse -Force`, sur ton go explicite.

---

## 8. Commandes (à exécuter seulement sur ton accord)

```powershell
# T0 — récupère ~66 Go (régénérable). NE PAS lancer si tu run encore le V1 souvent.
Remove-Item -Recurse -Force "D:\Forgia\RUST\Forgia\Forgia\target"
Remove-Item -Recurse -Force "D:\Forgia\Forgia-dungeon-gen\target"
Remove-Item -Recurse -Force "D:\Forgia\RUST\Forgia\Forgia\node_modules"

# T1 — junk
Remove-Item -Force "D:\Forgia\tmp_*.png","D:\Forgia\screen.png","D:\Forgia\game_session.log","D:\Forgia\DAILY_BRIEF.html","D:\Forgia\DAILY_BRIEF.md","D:\Forgia\Toute","D:\Forgia\generate_dashboard.sh","D:\Forgia\watch_errors.sh"
Remove-Item -Recurse -Force "D:\Forgia\CUsersAntoiAppDataLocalTempfs_test"
```

T2/T3/T4 = à dérouler après décision §3 et après le P0 V1→V2.

---

*Sources internes : mesures disque 2026-06-17 ; `.mcp.json`, `settings.json`, hooks. Best-practices : séparation des préoccupations, build artifacts non versionnés/jetables, config-as-repo, bus-factor. Voir aussi `docs/best-practices-cockpit-tuning-2026-06-17.md` (le P0 V1→V2 conditionne T3).*

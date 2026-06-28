# Veille - Exploiter la veille technologique quotidienne Forgia

Interroge l'archive de veille (Rust/Bevy/Gaming/IA + assets CC0) et integre les items dans Forgia.

**Arguments** : $ARGUMENTS

## Modes d'utilisation

| Forme | Effet |
|---|---|
| `/veille` (sans arg) | Affiche la veille du jour (markdown). Si pas encore generee, dit comment la lancer. |
| `/veille today` | Identique a sans arg. |
| `/veille list` | Liste toutes les veilles archivees avec date + count + ACTION DU JOUR. |
| `/veille search <kw>` | Cherche un mot-cle dans tous les JSON archives (titres + bodies). |
| `/veille assets` | Filtre items assets CC0 du jour (Poly Haven, ambientCG, Sketchfab, etc.). |
| `/veille releases` | Filtre items releases du jour (Bevy, Rapier, egui, etc.). |
| `/veille community` | Filtre items communaute du jour (Reddit, HN, Lobste.rs). |
| `/veille tag <RPG\|FPS>` | Filtre items tagues pour ce mode dans le markdown du jour. |
| `/veille item <date> <id>` | Affiche le detail d'un item precis (date au format YYYY-MM-DD, id entier). |
| `/veille integrate <id>` | Cree une story BMAD d'integration de l'item du JOUR. Voir section "Auto-story" ci-dessous. |

## Execution

### Pour tous les modes sauf `integrate`

Execute le script de query :

```bash
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "D:/IA Antoine/veille/scripts/veille-query.ps1" <mode> <arg1> <arg2>
```

Exemples :
- `/veille search bevy` -> `... veille-query.ps1 search bevy`
- `/veille item 2026-05-13 7` -> `... veille-query.ps1 item 2026-05-13 7`

Affiche directement le resultat a l'utilisateur, sans paraphrase.

### Pour `/veille integrate <id>` (auto-story)

1. **Lis le JSON du jour** : `D:/IA Antoine/veille/archive/<TODAY>.json` (TODAY = date courante)
2. **Trouve l'item** dont `id` = argument fourni.
3. **Determine le type d'integration** :
   - Si `is_asset_cc0=true` : story type "asset-integration" (download + import + place dans la scene)
   - Si `is_release=true` : story type "dep-update" (bump Cargo.toml + adapt breaking changes)
   - Sinon : story type "tech-integration" (lire l'article, identifier le pattern, l'adapter)
4. **Lis CLAUDE.md** sections 7 (BMAD Workflow) + 9 (Patterns de code) pour respecter le format story.
5. **Determine le scale BMAD** :
   - Asset 1 modele 3D simple : Quick (no story file)
   - Lib externe ou multi-fichiers : Standard (story file requise)
6. **Genere le story file** dans `RUST/Forgia/Forgia/docs/stories/story-NNN-integrate-<slug>.md` ou NNN = next-id (cf SessionStart context).

Template story pour asset 3D CC0 (exemple) :

```markdown
# Story-NNN : Integration <nom asset>

**Status** : DRAFT
**Scale** : Standard
**Source** : Veille <date> item #<id>
**URL** : <url asset>
**Licence** : CC0

## Contexte
<resume body de l'item, 2-3 phrases>

## Acceptance criteria
- [ ] Asset telecharge dans `assets/models/<dossier>/`
- [ ] Reference ajoutee dans `forgia-game/src/resources/assets.rs` (L1 baseline a respecter)
- [ ] Asset visible en jeu (preciser scene/biome ou il apparait)
- [ ] Performance : 0 frame drop (verifier forgia_diagnostics.json)
- [ ] 0 clippy warning, 0 erreur cargo check

## Phases
1. **Download** : recuperer le pack depuis <url>. Verifier licence CC0/CC-BY/Apache.
2. **Import** : convertir au format Bevy (glb/gltf prefere). Stocker dans assets/.
3. **Reference** : ajouter handle dans GameAssets + update baseline si nouveau Handle<>.
4. **Place** : code Bevy pour spawn l'asset dans la scene cible (RPG biome / FPS arena).
5. **Verify** : cargo check + cargo clippy + visuel + perf.

## Fichiers attendus
- `assets/models/<sous-dossier>/...`
- `RUST/Forgia/Forgia/forgia-game/src/resources/assets.rs` (1 ligne)
- `RUST/Forgia/Forgia/forgia-game/src/<module pertinent>/...` (spawn logic)

## Locks impactes
- L1 (GameAssets baseline) : update obligatoire si nouveau Handle<>
- Aucun autre
```

Pour `is_release=true`, adapte le template (focus sur Cargo.toml bump + breaking changes + integration testing).
Pour `is_release=false` et `is_asset_cc0=false`, focus sur le pattern technique a porter.

**Apres creation** : affiche le path de la story creee + propose les 3 prochaines actions (lire la story, lancer Quick BMAD, mettre en backlog).

## Convention sortie

- Utilise le francais pour les explications a l'utilisateur.
- Code/identifiants en anglais.
- Pas de blabla : le script PS1 fait le travail, affiche son output, c'est tout.
- Pour `integrate`, sois concret : "Story creee a <path>. Acceptance criteria pretes. Tu veux qu'on attaque la phase 1 maintenant ?"
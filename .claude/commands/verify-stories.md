# Verify Stories — Lint du pipeline stories

Verifie l'integrite de toutes les stories dans `docs/stories/`.

**Contexte** : $ARGUMENTS (optionnel : "quick" = checks 1-3 seulement, "full" = tous les checks, defaut = full)

## Instructions

Execute ces checks en sequence :

### 1. Inventaire fichiers vs index

1. Lire `RUST/Forgia/Forgia/docs/stories/_index.md`
2. Glob tous les fichiers `story-*.md` dans le dossier
3. Detecter :
   - **Orphelins** : fichiers story-*.md presents sur disque mais absents de _index.md
   - **Fantomes** : stories referencees dans _index.md mais fichier inexistant
   - **Doublons ID** : meme story-NNN utilise par plusieurs fichiers (ex: story-303-*.md x3)

### 2. Validation metadata par story

Pour chaque fichier story-*.md, verifier la table metadata :

| Champ | Valeurs canoniques |
|-------|--------------------|
| **Statut** | `TODO`, `IN_PROGRESS`, `DONE`, `BLOCKED` (exact, case-sensitive) |
| **Priorite** | `P0`, `P1`, `P2`, `P3` (format court accepte) ou `P0-critical`, `P1-high`, `P2-medium`, `P3-low` |
| **Scale** | `quick`, `standard`, `enterprise` (lowercase) |

Signaler :
- Champs manquants (ID, Statut, Priorite, Scale obligatoires)
- Valeurs non-canoniques (ex: "Done", "PLANNED", "en cours", "a faire", "DEBUG PENDING")
- Table metadata absente (format libre au lieu de table markdown)

### 3. Coherence statuts index <-> fichier

Comparer le statut dans _index.md (emoji) avec le statut dans le fichier story :
- `:white_check_mark:` = DONE
- `:construction:` = IN_PROGRESS
- `:clipboard:` = TODO
- `:no_entry:` = BLOCKED

Signaler toute incoherence (ex: index dit DONE mais fichier dit IN_PROGRESS).

### 4. Acceptance Criteria format

Verifier que les AC utilisent le format checkbox `- [ ]` ou `- [x]`, pas des tirets simples.
Signaler les stories DONE dont les AC ne sont pas tous coches `[x]`.

### 5. Dependances valides

Si une story reference `story-NNN` dans sa section Dependances, verifier que ce fichier existe.
Detecter les dependances circulaires (A depend de B depend de A).

### 6. Stories DONE sans date

Pour chaque story DONE dans _index.md, verifier qu'une date est presente (format `DONE (YYYY-MM-DD)`).
Lister les stories DONE sans date de completion — important pour la tracabilite.

### 7. Stories stagnantes

Detecter les stories IN_PROGRESS depuis trop longtemps :
- Comparer la date du dernier commit touchant les fichiers de la story (si disponible via git log)
- Sinon, signaler toutes les IN_PROGRESS et laisser l'utilisateur evaluer
- Seuil d'alerte : >14 jours sans activite

### 8. Stories sans AC

Signaler les stories qui n'ont AUCUN critere d'acceptation (pas de section "Acceptance Criteria" ou section vide).
Une story sans AC ne peut pas etre validee DONE.

### 9. Stories trop volumineuses

Signaler les fichiers story-*.md > 300 lignes.
Une story trop longue est probablement un signe qu'elle devrait etre splittee en sous-stories.

### 10. Cycles vides

Detecter les cycles dans _index.md ou TOUTES les stories sont DONE ou SUPPRIMEES.
Ce ne sont pas des erreurs, mais signaler pour nettoyage eventuel (archivage).

### 11. Diff depuis dernier run

Lire la derniere entree `/verify-stories` dans `docs/log.md`.
Comparer les metriques actuelles vs precedentes et signaler les deltas :
- Nouvelles stories ajoutees
- Stories passees DONE depuis le dernier run
- Nouveaux problemes apparus
- Problemes resolus

### 12. Rapport

```
## Stories Verification Report — [date]

### Resume
- Total stories fichiers : N
- Total stories index : N
- Orphelins : N
- Fantomes : N
- Doublons ID : N
- Metadata invalide : N
- Statuts incoherents : N
- AC mal formates : N
- Stories DONE sans date : N
- Stories stagnantes (>14j) : N
- Stories sans AC : N
- Stories >300 lignes : N
- Deps cassees / circulaires : N
- Cycles archivables : N

### Deltas vs dernier run
[Si disponible : +N stories, +N DONE, +/-N problemes]

### Details
[Lister chaque probleme par severite : CRITIQUE > WARN > INFO]
[Pour chaque : fichier + description + correction suggeree]

### Status : SAIN / N PROBLEMES (N critiques, N warnings, N info)
```

Ajouter une entree dans `docs/log.md` avec le resultat.

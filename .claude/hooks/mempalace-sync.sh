#!/bin/bash
# Incrementally index newly-created canonical memory files at session end.
# MemPalace skips sources already present, so normal runs remain short.

PYTHON="${COCKPIT_PYTHON:-C:/Users/Antoi/AppData/Local/Programs/Python/Python314/python.exe}"
PALACE="${MEMPALACE_PALACE_PATH:-C:/Users/Antoi/.mempalace/palace}"
MEMORY_DIR="${COCKPIT_MEMORY_DIR:-C:/Users/Antoi/.claude/projects/c--Users-Antoi-Desktop-Forgia-Rewrite/memory}"

export PYTHONIOENCODING=utf-8
export PYTHONUTF8=1
export MEMPALACE_PALACE_PATH="$PALACE"

if [ ! -d "$MEMORY_DIR" ]; then
    echo "MEMPALACE SYNC SKIPPED: memory directory missing: $MEMORY_DIR" >&2
    exit 0
fi

# ⚡ PERF (2026-08-09) — ce hook coûtait 3 241 ms de moyenne sur 170 appels
# (~9 min cumulées) et a déjà touché son plafond de 20 s. Or un dépassement
# répété OUVRE le coupe-circuit du perf-guard : la mémoire cesserait d'être
# classée EN SILENCE — exactement la panne qu'on a passé la journée à traquer.
# Le coût n'est pas l'ingestion, c'est le re-scan des ~890 fichiers pour
# découvrir qu'il n'y a rien de neuf. Or on sait le dire en une comparaison de
# dates : si aucun .md n'est plus récent que le dernier passage, il n'y a rien
# à faire. Le jalon n'est mis à jour QUE si `mine` réussit — un échec sera
# donc réessayé à la session suivante, jamais avalé.
JALON="$MEMORY_DIR/.derniere-synchro-mempalace"
if [ -f "$JALON" ] && [ -z "$(find "$MEMORY_DIR" -maxdepth 1 -name '*.md' -newer "$JALON" -print -quit 2>/dev/null)" ]; then
    echo "MEMPALACE SYNC SKIPPED: aucun memory modifie depuis le dernier classement"
    exit 0
fi

"$PYTHON" -m mempalace.cli mine "$MEMORY_DIR" --wing forgia >/dev/null 2>&1
RC=$?
if [ "$RC" -eq 0 ]; then
    touch "$JALON"
    echo "MEMPALACE SYNC OK: $MEMORY_DIR"
else
    echo "MEMPALACE SYNC DEGRADED (exit $RC): memories remain available through docs/AI_MEMORY_MAP.md" >&2
fi

# Memory indexing is best-effort and must never prevent Claude from stopping.
exit 0

#!/bin/bash
# Validate the canonical Forgia Rewrite MemPalace and inject its state into
# Claude's SessionStart context. Read-only: no mining or database mutation.

PYTHON="${COCKPIT_PYTHON:-C:/Users/Antoi/AppData/Local/Programs/Python/Python314/python.exe}"
PALACE="${MEMPALACE_PALACE_PATH:-C:/Users/Antoi/.mempalace/palace}"

export PYTHONIOENCODING=utf-8
export PYTHONUTF8=1
export MEMPALACE_PALACE_PATH="$PALACE"

# 🚨 Redondance du contrôle des coupe-circuits — angle mort structurel mesuré
# le 2026-08-09 : `tools-health.sh` est le seul à signaler un hook désactivé,
# mais s'il est LUI-MÊME coupé (3 dépassements → 1 h de silence), plus personne
# ne le dit. Un garde qui ne peut pas signaler sa propre absence n'en est pas un.
# Ce hook-ci tourne au même SessionStart depuis une entrée distincte : il faut
# que les DEUX soient coupés pour que l'alerte disparaisse.
BRK="${COCKPIT_LOG_DIR:-D:/IA Antoine/logs}/breakers"
COUPES=$(ls "$BRK" 2>/dev/null | tr '\n' ' ')
if [ -n "$COUPES" ]; then
    echo "HOOKS COUPE-CIRCUIT ouvert(s) : ${COUPES}— ce ou ces hooks ne tournent plus et ne le disent pas. Corriger la cause, puis supprimer le fichier dans $BRK."
fi

STATUS=$("$PYTHON" -m mempalace.cli status 2>&1)
RC=$?

if [ "$RC" -ne 0 ] || echo "$STATUS" | grep -q "No palace found"; then
    echo "MEMPALACE UNAVAILABLE: canonical palace=$PALACE. Do not assume semantic memory was searched; fall back to docs/AI_MEMORY_MAP.md and report the failure."
    exit 0
fi

# 🚨 `cli status` PLAFONNE à 10 000 : il annonçait « 10000 drawers » là où la
# base en contient 11 378, avec toutes ses pièces sous-comptées. Un compteur
# bloqué ne peut plus bouger, donc ne mesure plus rien — il aurait affiché
# « READY, 10000 » pour toujours, y compris sur un palais en train de mourir.
# La vérité se lit dans la base, en lecture seule.
DRAWERS=$("$PYTHON" -c "
import sqlite3,sys
try:
    c=sqlite3.connect('file:'+sys.argv[1]+'/chroma.sqlite3?mode=ro',uri=True)
    print(c.execute('SELECT COUNT(*) FROM embeddings').fetchone()[0])
except Exception:
    print('')
" "$PALACE" 2>/dev/null)

# Les tiroirs sont-ils encore alimentés ? Un palais qui n'a rien reçu depuis
# des jours est le symptôme d'une ingestion morte (cf. l'index grepai figé 8 j).
DERNIER=$("$PYTHON" -c "
import sqlite3,sys
try:
    c=sqlite3.connect('file:'+sys.argv[1]+'/chroma.sqlite3?mode=ro',uri=True)
    r=c.execute(\"SELECT MAX(string_value) FROM embedding_metadata WHERE key='filed_at'\").fetchone()[0]
    print((r or '')[:10])
except Exception:
    print('')
" "$PALACE" 2>/dev/null)

echo "MEMPALACE READY: ${DRAWERS:-unknown} tiroirs, dernier classement ${DERNIER:-inconnu} ($PALACE). Le classement se fait au Stop hook et SAUTE les fichiers deja presents — un memory EDITE apres coup garde sa version d'origine. Avant toute modif de code non triviale, mcp__mempalace__mempalace_search avec les concepts de la tache, puis verifier contre le code actuel."
exit 0

#!/usr/bin/env bash
# SessionStart — état des outils de la session, en UNE lecture.
#
# Pourquoi ce hook existe : l'index grepai a pourri en silence du 30/07 au
# 07/08. Aucune commande ne mentait — personne ne regardait. Un outil dont
# personne ne vérifie la fraîcheur finit par répondre sur du code mort, et
# c'est pire que pas d'outil du tout : ça a l'air de marcher.
#
# Discipline appliquée (`map-design-patterns.md` §13) : un contrôle expose la
# taille de son échantillon. « 0 mesuré » n'est jamais vert — c'est AVEUGLE.
# Chaque ligne ci-dessous DOIT pouvoir rougir, sinon elle ne mesure rien.
#
# Budget : < 3 s. Aucun spawn Python (mempalace a déjà son propre hook).

RACINE="${PROJECT_ROOT:-C:/Users/Antoi/Desktop/Forgia Rewrite}"
cd "$RACINE" 2>/dev/null || { echo "OUTILS: racine introuvable ($RACINE)"; exit 0; }

MAINTENANT=$(date +%s)
LIGNES=()
ALERTES=0

# ── ollama D'ABORD : c'est lui qui décide si relancer grepai a un sens ───────
#
# 🚨 Ordre corrigé le 2026-08-09 : le contrôle annonçait « grepai RELANCE faite »
# alors qu'ollama était injoignable — donc un watcher relancé qui n'indexe
# RIEN, faute d'embeddings. Se féliciter d'une relance creuse est exactement le
# capteur menteur que ce fichier est censé interdire. La verticale de
# dépendance (ollama → grepai) doit se lire dans l'ORDRE du script.
#
# Quand ollama est down on ne se contente pas de le dire : on DÉLÈGUE la
# reprise complète à `grepai-autostart.ps1`, qui sait déjà enchaîner
# ollama → attente ≤30 s → watcher grepai. On ne l'ATTEND pas : 30 s crèveraient
# le budget du hook, et 3 dépassements ouvrent le coupe-circuit du perf-guard
# (le contrôle deviendrait muet une heure — la panne qu'on veut éviter).
# Le `.vbs` est le lanceur détaché déjà éprouvé (`Run(..., 0, False)`).
OLLAMA_OK=0
RELANCE_DELEGUEE=0
AUTOSTART="C:/Users/Antoi/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup/grepai-forgia-autostart.vbs"
# Même leçon qu'au bloc BRP : borner la CONNEXION (ce qui traîne quand ollama
# est éteint) et non la réponse. On divise par deux le cas « down » (2 s → 1 s)
# tout en donnant PLUS d'air à la réponse (2 s → 3 s) — meilleur sur les deux
# axes, au lieu du compromis que serait un `-m` unique.
TAGS=$(curl -s --connect-timeout 1 -m 3 http://localhost:11434/api/tags 2>/dev/null)
if [ -z "$TAGS" ]; then
    if [ -f "$AUTOSTART" ] && wscript "$AUTOSTART" >/dev/null 2>&1; then
        RELANCE_DELEGUEE=1
        LIGNES+=("ollama etait DOWN — reprise complete LANCEE en arriere-plan (ollama puis watcher grepai, pret sous ~30 s). Ne citer aucune recherche semantique dans cette fenetre ; l'index est fige en attendant.")
    else
        LIGNES+=("ollama INJOIGNABLE (11434) et reprise auto INDISPONIBLE — grepai ne peut plus indexer. A la main : powershell -File \"D:/IA Antoine/grepai-autostart.ps1\"")
    fi
    ALERTES=$((ALERTES + 1))
elif ! echo "$TAGS" | grep -q "nomic-embed-text"; then
    LIGNES+=("ollama UP mais nomic-embed-text ABSENT — l'indexation grepai échouera : ollama pull nomic-embed-text")
    ALERTES=$((ALERTES + 1))
else
    OLLAMA_OK=1
    LIGNES+=("ollama OK — nomic-embed-text présent")
fi

# ── grepai : le watcher tourne-t-il, et de QUAND date l'index ? ──────────────
# La mtime de index.gob ne dit rien : une simple recherche la réécrit. La seule
# vérité est `watch.last_index_time` dans .grepai/config.yaml.
if ! command -v grepai >/dev/null 2>&1; then
    LIGNES+=("grepai ABSENT du PATH — pas de recherche sémantique cette session")
    ALERTES=$((ALERTES + 1))
elif [ ! -f .grepai/config.yaml ]; then
    # Garde : ne JAMAIS lancer un watcher dans un dossier qui n'est pas un
    # projet grepai indexé (mesuré sur un dossier vide : la relance échouait,
    # et le message parlait d'un « index figé » qui n'a jamais existé).
    LIGNES+=("grepai non initialisé ici (.grepai/config.yaml absent) — aucune recherche sémantique possible depuis $RACINE")
    ALERTES=$((ALERTES + 1))
else
    WATCH=$(grepai watch --status 2>/dev/null | sed -n 's/^Status: //p' | head -1)
    STAMP=$(sed -n 's/.*last_index_time: *//p' .grepai/config.yaml 2>/dev/null | head -1)
    JOURS=-1
    if [ -n "$STAMP" ]; then
        IDX=$(date -d "$STAMP" +%s 2>/dev/null)
        [ -n "$IDX" ] && JOURS=$(( (MAINTENANT - IDX) / 86400 ))
    fi
    if [ "$WATCH" != "running" ]; then
        # On RELANCE, on ne se contente pas de constater. Un rapport qui dit
        # « c'est mort » sans agir est ce qui a laissé l'index pourrir 8 jours.
        # `watch --background` rend la main tout de suite et indexe derrière.
        if [ "$RELANCE_DELEGUEE" -eq 1 ]; then
            # L'autostart va le lancer lui-même une fois ollama prêt. Le doubler
            # ici ferait courir deux watchers sur le même index.
            LIGNES+=("grepai arrete (index du ${STAMP:0:10}, ${JOURS} j) — sa relance fait partie de la reprise deja lancee ci-dessus, ne pas la doubler.")
            ALERTES=$((ALERTES + 1))
        elif [ "$OLLAMA_OK" -eq 0 ]; then
            LIGNES+=("grepai arrete et ollama down — relancer le watcher maintenant ne sert a rien (il meurt sans embeddings). Index fige au ${STAMP:0:10} (${JOURS} j).")
            ALERTES=$((ALERTES + 1))
        elif grepai watch --background >/dev/null 2>&1; then
            LIGNES+=("grepai etait ARRETE (index du ${STAMP:0:10}, ${JOURS} j) — RELANCE faite, rattrapage en cours. Ne pas citer une recherche semantique avant que l'index ait rattrape.")
            ALERTES=$((ALERTES + 1))
        # 🚨 Course multi-terminal. Ces hooks vivent dans les settings du PROJET :
        # tout terminal ouvert ici les exécute. Si l'autre a démarré le watcher
        # entre notre `--status` et notre `--background`, grepai refuse avec
        # « watcher is already running (PID …) » et sort en 1. Prendre ce 1 pour
        # un échec ferait crier « index figé, code périmé » alors que tout va
        # bien — une fausse alerte qui décrédibilise tout le contrôle.
        # Le code de sortie ne tranche pas : l'ÉTAT tranche. On re-mesure.
        elif [ "$(grepai watch --status 2>/dev/null | sed -n 's/^Status: //p' | head -1)" = "running" ]; then
            LIGNES+=("grepai OK — watcher demarre par un AUTRE terminal (course gagnee par lui), index du ${STAMP:0:10}")
        else
            LIGNES+=("grepai ARRETE et relance ECHOUEE — index figé au ${STAMP:0:10} (${JOURS} j). Toute recherche sémantique répond sur du code périmé.")
            ALERTES=$((ALERTES + 1))
        fi
    elif [ "$JOURS" -ge 2 ]; then
        LIGNES+=("grepai watcher ON mais index vieux de ${JOURS} j (${STAMP:0:10}) — rattrapage en cours ou bloqué ; revérifier avant de citer un résultat")
        ALERTES=$((ALERTES + 1))
    else
        LIGNES+=("grepai OK — watcher actif, index du ${STAMP:0:10}")
    fi
fi

# ── capteurs : combien, quel âge, combien en alerte ─────────────────────────
# Le compte EST le contrôle : 0 capteur = le jeu n'a jamais tourné ici, et
# tout diagnostic « d'après les capteurs » serait une lecture de fossiles.
NB=$(ls forgia2_*.json 2>/dev/null | wc -l)
if [ "$NB" -eq 0 ]; then
    LIGNES+=("capteurs AUCUN forgia2_*.json — le jeu n'a pas tourné depuis ce dossier ; ne rien conclure d'une lecture de capteurs")
    ALERTES=$((ALERTES + 1))
else
    RECENT=$(ls -t forgia2_*.json 2>/dev/null | head -1)
    AGE_H=$(( (MAINTENANT - $(stat -c%Y "$RECENT" 2>/dev/null || echo "$MAINTENANT")) / 3600 ))
    # 🚨 Le motif a compté 0 alerte sur 8 réelles au premier essai : les
    # capteurs écrivent `warn`, pas `warning`, et certains espacent le `:`.
    # Un contrôle qui ne peut pas rougir est exactement ce que la famille IV
    # interdit — d'où les deux orthographes ET l'espacement tolérés ici.
    ROUGE=$(grep -lE '"severity" *: *"(warn|warning|error|critical)"' forgia2_*.json 2>/dev/null | wc -l)
    LIGNES+=("capteurs ${NB} fichiers, plus récent il y a ${AGE_H} h, ${ROUGE} en alerte — les lire via: python tools/ai/forgia_digest.py all (JAMAIS en brut)")
fi

# ── BRP : inspection ECS / captures d'écran, seulement si le jeu tourne ─────
#
# ⚡ Ce contrôle pesait 1046 ms — 57 % du hook à lui seul. Le port fermé
# n'émet pas de refus, il ABSORBE la connexion : on payait le délai d'attente
# entier, à chaque session, pour l'état le PLUS courant (jeu éteint).
#
# 🚨 Premier correctif RATÉ, et instructif : `-m 0.2` borne le temps TOTAL,
# réponse comprise. Sur un port ouvert répondant en 241 ms il échouait —
# j'aurais annoncé « BRP inactif » avec le jeu lancé. C'est `--connect-timeout`
# qu'il faut : il borne la POIGNÉE DE MAIN (ce qui traîne quand c'est fermé)
# et laisse la réponse prendre son temps. Mesuré : fermé 341 ms rc=28,
# ouvert 239 ms rc=0. (`/dev/tcp` essayé aussi : 2021 ms fermé, deux fois pire.)
if curl -s --connect-timeout 0.3 -m 2 -o /dev/null http://localhost:15702 2>/dev/null; then
    LIGNES+=("BRP ACTIF (15702) — mcp__bevy-brp__* disponible pour inspecter l'ECS en direct")
else
    LIGNES+=("BRP inactif — jeu non lancé, ou lancé sans --features dev-brp (les outils mcp__bevy-brp__* échoueront)")
fi

# ── standup multi-terminal (règle bloquante) ────────────────────────────────
# > 12 fichiers modifiés = un autre agent travaille dans cet arbre. Ne pas
# patcher ses erreurs, ne pas committer à l'aveugle.
#
# 🚨 `git status` qui ÉCHOUE donnait « git 0 fichiers modifiés » — soit un arbre
# annoncé propre là où il n'y a pas de dépôt du tout. « 0 mesuré » n'est pas
# vert, c'est aveugle (`map-design-patterns.md` §13) : les deux cas se séparent.
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    LIGNES+=("git AUCUN dépôt sous $RACINE — le standup multi-terminal est aveugle, ne rien conclure sur l'activité d'un autre agent")
    ALERTES=$((ALERTES + 1))
    CHURN=-1
else
    CHURN=$(git status --porcelain 2>/dev/null | wc -l)
fi
if [ "$CHURN" -lt 0 ]; then
    : # déjà rapporté ci-dessus
elif [ "$CHURN" -gt 12 ]; then
    LIGNES+=("git ${CHURN} fichiers modifiés — présumer un AUTRE terminal actif : vérifier les mtimes avant tout commit, ne pas patcher une erreur hors-scope (multi-terminal-coordination.md)")
    ALERTES=$((ALERTES + 1))
else
    LIGNES+=("git ${CHURN} fichiers modifiés")
fi

# ── binaire vs sources : la règle bloquante qui n'avait aucun automate ───────
# `multi-terminal-coordination.md` §5 : aucun diagnostic runtime n'est valide
# tant que source ≤ artefact. Un runner qui relance un vieil exe quand une
# dépendance casse ne prévient PAS — 30 min de diagnostic faux, déjà payées.
# Le nom se récupère par `find -printf`, jamais par `ls` (dont la sortie décorée
# casserait `-newer` en silence, et rendrait « 0 source récente » = faux vert).
EXE=$(find target -maxdepth 2 -name 'forgia.exe' -printf '%T@ %p\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-)
if [ -z "$EXE" ]; then
    LIGNES+=("binaire AUCUN forgia.exe compilé — rien à tester en jeu tant que 'cargo build -p forgia' n'a pas tourné")
else
    NEUVES=$(find crates -name '*.rs' -newer "$EXE" 2>/dev/null | wc -l)
    QUAND=$(date -d "@$(stat -c%Y "$EXE" 2>/dev/null)" '+%d/%m %H:%M' 2>/dev/null)
    if [ "$NEUVES" -gt 0 ]; then
        LIGNES+=("binaire PERIME — ${NEUVES} sources .rs plus recentes que l'exe du ${QUAND}. NE PAS dire « teste en jeu » : ce qui tourne ne contient pas ces changements. Rebuild d'abord.")
        ALERTES=$((ALERTES + 1))
    else
        LIGNES+=("binaire à jour (exe du ${QUAND}, 0 source plus récente)")
    fi
fi

# ── qui garde les gardiens : un hook coupé l'est en SILENCE pendant 1 h ──────
BRK="${COCKPIT_LOG_DIR:-D:/IA Antoine/logs}/breakers"
COUPES=$(ls "$BRK" 2>/dev/null | wc -l)
if [ "$COUPES" -gt 0 ]; then
    LIGNES+=("hooks ${COUPES} COUPE-CIRCUIT ouvert(s) dans $BRK ($(ls "$BRK" 2>/dev/null | tr '\n' ' ')) — ce ou ces hooks ne tournent plus, sans le dire. Corriger puis supprimer le fichier.")
    ALERTES=$((ALERTES + 1))
fi

echo "OUTILS DE SESSION — ${ALERTES} point(s) d'attention :"
for L in "${LIGNES[@]}"; do echo "  - $L"; done
echo "Rapporter cet état à l'utilisateur en une ligne dès la première réponse."
exit 0

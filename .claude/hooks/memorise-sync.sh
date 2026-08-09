#!/usr/bin/env bash
# UserPromptSubmit — déclenché quand l'utilisateur demande de mémoriser.
#
# Deux choses, dans cet ordre :
#   1. AGIR — relancer le watcher grepai s'il est mort, pour que l'index
#      sémantique rattrape le code écrit pendant la session. C'est mécanique,
#      donc ça n'a rien à faire dans une consigne : ça se fait.
#   2. RAPPELER — la liste des mémoires à mettre à jour, parce qu'elles, elles
#      demandent un jugement (quoi retenir) qu'aucun script ne rend.
#
# Origine : le 2026-08-07, l'index grepai avait 8 jours de retard et personne
# ne s'en était aperçu — les recherches répondaient sur du code d'avant.
# Le moment « mémorise » est le seul point de la session où l'on sait qu'il y a
# du neuf à indexer. Autant s'en servir.

RACINE="${PROJECT_ROOT:-C:/Users/Antoi/Desktop/Forgia Rewrite}"
BRUT=$(cat)

# Détection de l'INTENTION « range ce qu'on a appris ». Deux pièges, tous deux
# mesurés — le second a coûté 4 faux positifs sur des prompts de dev réalistes.
#
# 🚨 1. `m[ée]moris` NE MARCHE PAS : `é` fait 2 octets en UTF-8 et ce grep
#    (Git Bash) raisonne en octets, donc la classe `[ée]` ne le contient jamais.
#    Mesuré : 0 match sur « mémorise ». `m.{0,2}moris` couvre les deux graphies.
#
# 🚨 2. Trop LARGE est pire que trop étroit ici. Dans un JEU, « checkpoint » est
#    un mot MÉTIER : « ajoute un checkpoint dans le niveau 3 » déclenchait toute
#    la checklist mémoire. De même « la mémorisation des touches » (un nom, pas
#    un ordre) et « fais-moi le point sur les bugs » (une demande d'état).
#    Arbitrage : un faux négatif coûte UNE répétition ; un faux positif pollue
#    le contexte à chaque prompt métier. On penche donc vers la précision.
#      · le VERBE, pas le nom  → `moris(e|er|ez|ons)` exclut « mémorisation »
#      · `checkpoint` seulement s'il parle de session/mémoire
#      · « fais le point » RETIRÉ : trop souvent une demande d'état
if ! echo "$BRUT" | grep -qiE 'm.{0,2}moris(e|er|ez|ons)\b|checkpoint[^.]{0,24}(session|m.{0,2}moire)|(session|fin de session)[^.]{0,24}checkpoint|sauvegarde (la |cette )?session'; then
    exit 0
fi

cd "$RACINE" 2>/dev/null || exit 0

# ── 1. Agir : l'index sémantique doit contenir le code de CETTE session ──────
#
# 🚨 ollama se teste AVANT de relancer. Mesuré le 2026-08-09 : un watcher lancé
# alors qu'ollama est injoignable DÉMARRE puis MEURT — il ne peut pas calculer
# d'embeddings. Annoncer « relance faite » dans ce cas est un mensonge à retardement.
OLLAMA_OK=0
DELEGUE=0
AUTOSTART="C:/Users/Antoi/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/Startup/grepai-forgia-autostart.vbs"
if curl -s -m 2 http://localhost:11434/api/tags 2>/dev/null | grep -q "nomic-embed-text"; then
    OLLAMA_OK=1
elif [ -f "$AUTOSTART" ] && wscript "$AUTOSTART" >/dev/null 2>&1; then
    # Reprise complete deleguee au script eprouve (ollama -> attente -> grepai),
    # sans l'attendre : il lui faut jusqu'a 30 s, le hook a un budget de 15 s.
    DELEGUE=1
fi

GREPAI="grepai: absent du PATH"
if ! command -v grepai >/dev/null 2>&1; then
    :
elif [ ! -f .grepai/config.yaml ]; then
    GREPAI="grepai: pas de projet indexe ici (.grepai/config.yaml absent) — rien a resynchroniser"
else
    ETAT=$(grepai watch --status 2>/dev/null | sed -n 's/^Status: //p' | head -1)
    if [ "$ETAT" = "running" ] && [ "$OLLAMA_OK" -eq 1 ]; then
        GREPAI="grepai: watcher actif, index a jour"
    elif [ "$DELEGUE" -eq 1 ]; then
        GREPAI="grepai: ollama etait DOWN — reprise complete lancee en arriere-plan (ollama puis watcher, ~30 s). L'index NE contient pas encore le code de cette session."
    elif [ "$OLLAMA_OK" -eq 0 ]; then
        GREPAI="grepai: ollama INJOIGNABLE et reprise auto indisponible — l'index ne bougera pas. A la main: powershell -File \"D:/IA Antoine/grepai-autostart.ps1\""
    elif [ "$ETAT" = "running" ]; then
        GREPAI="grepai: watcher actif, index a jour"
    elif grepai watch --background >/dev/null 2>&1; then
        GREPAI="grepai: watcher etait MORT, relance faite — l'index rattrape le code de la session (verifier avant de citer une recherche)"
    elif [ "$(grepai watch --status 2>/dev/null | sed -n 's/^Status: //p' | head -1)" = "running" ]; then
        # Course multi-terminal : ces hooks sont dans les settings du PROJET, donc
        # chaque terminal ouvert ici les joue. grepai refuse un 2e watcher avec un
        # code 1 ; le prendre pour un echec crierait « code perime » a tort.
        GREPAI="grepai: watcher demarre par un AUTRE terminal — index partage, rien a faire"
    else
        GREPAI="grepai: watcher mort et relance ECHOUEE — les recherches semantiques repondent sur du code perime"
    fi
fi

cat <<EOF
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"CHECKPOINT — outils synchronises automatiquement:\n  $GREPAI\n\nRESTE A FAIRE (jugement requis, non automatisable):\n  1. memory/ — 1 fichier session_YYYY_MM_DD_<slug>.md (type project) + N fichiers reference_/feedback_ pour chaque pattern reutilisable ou lecon payee cher.\n  2. MEMORY.md — 1 ligne par memoire (<=150 car., lien + hook). Verifier qu'aucune entree existante ne couvre deja le fait: mettre a jour plutot que dupliquer.\n  3. mempalace — mcp__mempalace__mempalace_add_drawer pour les faits qui doivent ressortir par recherche semantique dans une AUTRE session. Verifier via mempalace_check_duplicate d'abord.\n  4. .claude/rules/ — si une lecon est BLOQUANTE (une classe de defaut, pas un cas), elle va dans une regle, pas dans un memory.\n  5. docs/stories/ + _index.md — statut des stories touchees. Une story ne passe DONE qu'apres 'cargo run -p xtask -- story-gate --story <id>' vert (story-done-gate.md).\n  6. .claude/rules/concept-first-table-forgia.md — re-pointer toute ligne dont un chemin a bouge.\nSuivre .claude/rules/session-checkpoint.md pour le format (audit du livre / audit du reste / persistance / prompt de reprise)."}}
EOF
exit 0

#!/usr/bin/env bash
# UserPromptSubmit — injecte l'observabilite quand l'utilisateur decrit un
# symptome, pour que le diagnostic parte des CAPTEURS et non d'une hypothese.
#
# Pourquoi : `log-digest.md` dit « sur "regarde", commencer par forgia_digest ».
# C'etait un REFLEXE a se rappeler. Un reflexe se perd ; un hook, non. Le
# 2026-08-09 j'ai failli rapporter un crash qui datait de six heures parce que
# je lisais les capteurs sans regarder leur age.
#
# 🚨 LE COUT EST PROPORTIONNEL AU SIGNAL — c'est ce qui rend le declencheur
# large supportable. Un faux positif coute ~20 tokens (une ligne), pas 600 :
#   · 0 alerte           -> une ligne, et on s'arrete
#   · des alertes        -> le digest complet
#   · capteurs perimes   -> l'age EN PREMIER, avant tout contenu
# Sans cette gradation, elargir le vocabulaire polluerait chaque prompt.

RACINE="${PROJECT_ROOT:-C:/Users/Antoi/Desktop/Forgia Rewrite}"
BRUT=$(cat 2>/dev/null)

# Vocabulaire LARGE du symptome. Deux precautions apprises a nos depens :
#   · aucune classe accentuee : `[ée]` ne matche JAMAIS en UTF-8 sous Git Bash
#     (l'accent fait 2 octets), d'ou des `.` a la place des lettres accentuees ;
#   · on vise le SYMPTOME, pas le domaine — « arme », « ennemi », « niveau »
#     sont des mots metier qui declencheraient sur de la conception.
MOTIFS='crash|plante|bug|freeze|fig.|bloqu.|cass.|erreur|probl.me'
MOTIFS+='|marche pas|fonctionne pas|marche plus|fonctionne plus|rien ne se passe'
MOTIFS+='|s.affiche pas|apparait pas|appara.t pas|disparu|invisible|manque'
MOTIFS+='|regarde|regardes|jette un oeil|c.est quoi ce'
MOTIFS+='|lag|rame|lent|saccade|chute de fps|perf'
MOTIFS+='|j.ai test|j.ai lanc|j.ai relanc|en jeu|au runtime'
MOTIFS+='|pourquoi (.a|il|elle|le|la|les) |comment .a se fait'
MOTIFS+='|capteur|sensor|log'

if ! printf '%s' "$BRUT" | grep -qiE "$MOTIFS"; then
    exit 0
fi

cd "$RACINE" 2>/dev/null || exit 0

NB=$(ls forgia2_*.json 2>/dev/null | wc -l)
if [ "$NB" -eq 0 ]; then
    CORPS="OBSERVABILITE : aucun capteur forgia2_*.json — le jeu n'a jamais tourne depuis ce dossier. Ne rien conclure d'une lecture de capteurs."
else
    RECENT=$(ls -t forgia2_*.json 2>/dev/null | head -1)
    AGE_MIN=$(( ($(date +%s) - $(stat -c%Y "$RECENT" 2>/dev/null || date +%s)) / 60 ))
    QUAND=$(date -d "@$(stat -c%Y "$RECENT" 2>/dev/null)" '+%H:%M' 2>/dev/null)
    ROUGE=$(grep -lE '"severity" *: *"(warn|warning|error|critical)"' forgia2_*.json 2>/dev/null | wc -l)

    # L'AGE d'abord : un capteur de six heures decrit un autre programme que
    # celui dont on parle. C'est l'erreur exacte commise le 2026-08-09.
    if [ "$AGE_MIN" -gt 30 ]; then
        FRAICHEUR="PERIMES (${QUAND}, il y a ${AGE_MIN} min) — ils decrivent le DERNIER run, pas forcement le symptome decrit. Verifier que le jeu a bien tourne depuis le changement."
    else
        FRAICHEUR="frais (${QUAND}, il y a ${AGE_MIN} min)"
    fi

    if [ "$ROUGE" -eq 0 ]; then
        # Rien a signaler : on ne paie qu'une ligne.
        CORPS="OBSERVABILITE : ${NB} capteurs ${FRAICHEUR}, 0 en alerte. Si le symptome est visible mais qu'aucun capteur ne rougit, c'est qu'il n'est pas instrumente — le nommer plutot que de deviner."
    else
        DIGEST=$(python tools/ai/forgia_digest.py sensors 2>/dev/null | head -20)
        CORPS="OBSERVABILITE : capteurs ${FRAICHEUR}.
${DIGEST}
Pour le log : python tools/ai/forgia_digest.py all (JAMAIS le brut). Un capteur nomme 'previous' est un ARTEFACT historique, pas une alerte vivante."
    fi
fi

# Encodage JSON par python : le digest contient guillemets, accents et sauts de
# ligne. Un echappement fait a la main casserait la sortie du hook au premier
# caractere inattendu — et un hook dont le JSON est invalide est ignore EN
# SILENCE, ce qui est exactement le genre de panne qu'on traque.
printf '%s' "$CORPS" | python -c "
import json,sys
print(json.dumps({'hookSpecificOutput':{'hookEventName':'UserPromptSubmit','additionalContext':sys.stdin.read()}}))
" 2>/dev/null || true

exit 0

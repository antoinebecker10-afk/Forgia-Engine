#!/bin/bash
# Banc 2 — les angles morts des 3 premiers bancs.
# On cherche ce qui n'a JAMAIS ete exerce, pas ce qu'on sait deja bon.

P="c:/Users/Antoi/Desktop/Forgia Rewrite"
cd "$P" || exit 1
export COCKPIT_LOG_DIR="D:/IA Antoine/logs"
OK=0; KO=0; ALERTE=""
chk(){ if [ "$1" = "$2" ]; then OK=$((OK+1)); echo "  ok    $3"; else KO=$((KO+1)); ALERTE="$ALERTE\n    $3 (attendu '$2', obtenu '$1')"; echo "  PROBLEME $3  (attendu '$2', obtenu '$1')"; fi; }

declenche(){ # rend 1 si memorise-sync produit sa checklist
  echo "{\"prompt\":\"$1\"}" | bash .claude/hooks/memorise-sync.sh 2>/dev/null | grep -c "CHECKPOINT" | head -1
}

echo "=== D1 · memorise-sync : FAUX POSITIFS sur des prompts de dev reels ==="
echo "    (dans un JEU, « checkpoint » est un mot metier — il ne devrait PAS declencher)"
chk "$(declenche 'memorise la session')"                          "1" "« memorise la session » declenche"
chk "$(declenche 'fais un checkpoint de session')"                "1" "« checkpoint de session » declenche"
chk "$(declenche 'ajoute un checkpoint dans le niveau 3')"        "0" "« checkpoint dans le niveau » ne declenche PAS"
chk "$(declenche 'le systeme de checkpoints de la run est casse')" "0" "« checkpoints de la run » ne declenche PAS"
chk "$(declenche 'fais moi le point sur les bugs restants')"      "0" "« fais le point sur les bugs » ne declenche PAS"
chk "$(declenche 'corrige la memorisation des touches clavier')"  "0" "« memorisation des touches » ne declenche PAS"
chk "$(declenche 'affiche la commemoration du studio')"           "0" "« commemoration » ne declenche PAS"

echo
echo "=== D2 · memorise-sync : le JSON survit-il a des prompts hostiles ? ==="
for pr in 'memorise "avec des guillemets"' 'memorise \\ antislash' 'memorise avec accents éàüç' 'memorise $(rm -rf /) injection'; do
  out=$(echo "{\"prompt\":\"$pr\"}" | bash .claude/hooks/memorise-sync.sh 2>/dev/null)
  v=$(printf '%s' "$out" | python -c "import json,sys;json.load(sys.stdin);print('valide')" 2>/dev/null || echo "CASSE")
  chk "$v" "valide" "JSON valide pour: ${pr:0:38}"
done

echo
echo "=== D3 · tools-health : la branche BRP POSITIVE (jamais exercee) ==="
# 🚨 1re version de ce test : un accept-loop mono-thread maison. Il perdait des
# connexions rapprochees — la sonde DIRECTE rendait « repond, muet, muet ». Le
# hook reflétait fidèlement ce faux signal et le banc l'accusait, LUI. C'est
# l'instrument qui mentait, pas le produit : même classe de défaut que les 4
# capteurs menteurs de la journée. Un vrai serveur HTTP règle la question.
python -m http.server 15702 --bind 127.0.0.1 >/dev/null 2>&1 &
FAUX=$!
sleep 2
r=$(echo '{}' | bash .claude/hooks/tools-health.sh 2>/dev/null | grep -c "BRP ACTIF")
chk "$r" "1" "BRP detecte ACTIF quand un service ecoute sur 15702"
kill $FAUX 2>/dev/null
sleep 2
r=$(echo '{}' | bash .claude/hooks/tools-health.sh 2>/dev/null | grep -c "BRP inactif")
chk "$r" "1" "BRP redevient inactif quand le service s'arrete"

echo
echo "=== D4 · tools-health : etats git inhabituels ==="
r=$(cd /tmp && bash "$P/.claude/hooks/tools-health.sh" </dev/null 2>/dev/null | grep -c "git")
chk "$r" "1" "une ligne git est TOUJOURS produite, meme hors depot"
r=$(echo '{}' | bash .claude/hooks/tools-health.sh 2>/dev/null | grep -cE "git [0-9]+ fichiers|git AUCUN")
chk "$r" "1" "la ligne git est bien formee dans le depot reel"

echo
echo "=== D5 · le coupe-circuit du perf-guard : angle mort structurel ==="
BRK="D:/IA Antoine/logs/breakers"; mkdir -p "$BRK"
echo "5" > "$BRK/faux-hook-test.fail"
r=$(echo '{}' | bash .claude/hooks/tools-health.sh 2>/dev/null | grep -c "COUPE-CIRCUIT")
chk "$r" "1" "tools-health SIGNALE un coupe-circuit ouvert"
# Le cas qui fait mal : c'est tools-health LUI-MEME qui est coupe.
echo "5" > "$BRK/tools-health.fail"
r=$(echo '{}' | bash .claude/hooks/mempalace-session-start.sh 2>/dev/null | grep -ci "coupe\|breaker")
chk "$r" "1" "un AUTRE hook signale que tools-health est coupe (redondance)"
rm -f "$BRK/faux-hook-test.fail" "$BRK/tools-health.fail"

echo
echo "=== D6 · grepai : requetes aux limites (ne doit jamais planter) ==="
for q in "a" "$(python -c 'print("bevy "*180)')" "select * from; DROP TABLE--" "éàü ïôç" "🔥🎮"; do
  rc=$(grepai search "$q" -j -c -n 2 >/dev/null 2>&1; echo $?)
  chk "$rc" "0" "requete tenue: ${q:0:34}"
done

echo
echo "=== E · prompt-observabilite : declencheur LARGE, mais pas bavard ==="
OBS="$P/.claude/hooks/prompt-observabilite.sh"
obs(){ local r got="silence"
  r=$(echo "{\"prompt\":\"$1\"}" | bash "$OBS" 2>/dev/null); [ -n "$r" ] && got="injecte"
  if [ "$got" = "$2" ]; then OK=$((OK+1)); echo "  ok    [$got] $1"
  else KO=$((KO+1)); ALERTE="$ALERTE\n    obs '$1' : attendu $2, obtenu $got"; echo "  ECHEC [$got] $1"; fi; }
# Symptomes -> doivent injecter. La couverture reste LARGE : demande explicite
# (« dans tous les contextes ou je pourrais en avoir besoin »).
obs "ca crash quand je tire"            injecte
obs "le menu ne s affiche pas"          injecte
obs "regarde"                           injecte
obs "j ai relance, ca marche pas"       injecte
obs "pourquoi ca rame autant"           injecte
obs "l arme est invisible en jeu"       injecte
# Conception / instruction -> silence. Un faux positif pollue CHAQUE prompt ;
# c'est la lecon du declencheur « checkpoint », mot metier dans un jeu.
obs "ajoute une arme au genome"         silence
obs "commit ce qui est bon"             silence
obs "refais un test apres"              silence
obs "equilibre la courbe de difficulte" silence
# Le digest contient guillemets et accents : un JSON casse serait ignore EN
# SILENCE par le harnais — la panne la plus difficile a voir.
if echo '{"prompt":"ca crash"}' | bash "$OBS" 2>/dev/null | python -c "import json,sys; json.load(sys.stdin)" 2>/dev/null; then
    OK=$((OK+1)); echo "  ok    sortie JSON valide (guillemets + accents du digest)"
else
    KO=$((KO+1)); ALERTE="$ALERTE\n    prompt-observabilite : JSON invalide"; echo "  ECHEC JSON invalide"
fi
rc=$(echo '{"prompt":"ca crash"}' | PROJECT_ROOT=/zzz/nope bash "$OBS" >/dev/null 2>&1; echo $?)
chk "$rc" "0" "sort 0 meme avec une racine inexistante"

echo
echo "════════════════════════════════════"
echo "   REUSSIS : $OK      PROBLEMES : $KO"
[ -n "$ALERTE" ] && printf "   Detail :%b\n" "$ALERTE"
echo "════════════════════════════════════"

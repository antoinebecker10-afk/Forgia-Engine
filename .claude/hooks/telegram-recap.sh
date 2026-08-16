#!/usr/bin/env bash
# SessionStart — pousse un recap Forgia sur Telegram (@ForgierBot).
#
# Pourquoi ce hook existe
# -----------------------
# L'etat du chantier et la veille vivent dans le depot ; le telephone, non. Sans
# ce pont, il faut ouvrir un terminal pour savoir qu'un capteur alerte ou qu'une
# release Bevy est sortie — donc on ne le sait jamais loin du bureau.
#
# Deux disciplines, apprises sur les pipelines precedents :
#
#   1. NE JAMAIS BLOQUER L'OUVERTURE. L'envoi part detache, la session continue.
#      Un appel reseau dans le chemin de demarrage, c'est un demarrage qui pend
#      quand le reseau tombe.
#   2. NE JAMAIS ECHOUER EN SILENCE. Le pipeline de veille est mort le 22/06 sur
#      un `api.telegram.org` non resolu, et personne ne l'a su pendant deux mois.
#      Ici, l'echec du dernier envoi est REMONTE a l'ouverture suivante.
#
# La regle de silence (rien de neuf -> rien d'envoye) vit dans le script PowerShell,
# pas ici : c'est lui qui sait ce qui a deja ete pousse.
#
# Budget : < 300 ms cote hook (le travail reel est detache).

set -u

RACINE="c:/Users/Antoi/Desktop/Forgia Rewrite"
SCRIPT="$RACINE/tools/ai/telegram_recap.ps1"
JOURNAL="$RACINE/.claude/telegram-recap.log"

[ -f "$SCRIPT" ] || exit 0

# ── remonter l'echec precedent AVANT de relancer ────────────────────────────
# Un instrument qui ne dit pas qu'il est casse est pire que pas d'instrument.
if [ -f "$JOURNAL" ]; then
  DERNIERE=$(tail -n 40 "$JOURNAL" 2>/dev/null | grep -E '^\[recap\]' | tail -n 1)
  case "$DERNIERE" in
    *ECHOUE*|*secrets\ absents*)
      echo "TELEGRAM — le dernier envoi a ECHOUE : ${DERNIERE#\[recap\] }"
      ;;
  esac
fi

# ── trouver un PowerShell ───────────────────────────────────────────────────
PS=""
for c in pwsh powershell; do
  if command -v "$c" >/dev/null 2>&1; then PS="$c"; break; fi
done
if [ -z "$PS" ]; then
  echo "TELEGRAM — aucun PowerShell trouve, recap non envoye"
  exit 0
fi

# ── detacher : la session ne doit rien attendre ─────────────────────────────
# Le journal ne doit pas grossir sans fin : on garde les 200 dernieres lignes.
if [ -f "$JOURNAL" ] && [ "$(wc -l <"$JOURNAL" 2>/dev/null || echo 0)" -gt 400 ]; then
  tail -n 200 "$JOURNAL" >"$JOURNAL.tmp" 2>/dev/null && mv -f "$JOURNAL.tmp" "$JOURNAL"
fi

{
  printf '\n=== %s ===\n' "$(date '+%Y-%m-%d %H:%M:%S')" >>"$JOURNAL"
  nohup "$PS" -NoProfile -NonInteractive -ExecutionPolicy Bypass \
        -File "$SCRIPT" >>"$JOURNAL" 2>&1 &
} >/dev/null 2>&1
disown 2>/dev/null || true

echo "TELEGRAM — recap en cours d'envoi en arriere-plan (silencieux s'il n'y a rien de neuf)"
exit 0

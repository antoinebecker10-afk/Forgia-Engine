#!/bin/bash
# Banc de robustesse — les hooks peuvent-ils BLOQUER, MENTIR ou PERDRE des donnees ?
# Un hook de SessionStart qui sort != 0 ou qui pend casse l'ouverture de session.
# Un jalon mal pose fait sauter le classement memoire POUR TOUJOURS, en silence.

P="c:/Users/Antoi/Desktop/Forgia Rewrite"
SC="$(dirname "$0")"
cd "$P" || exit 1
export COCKPIT_LOG_DIR="D:/IA Antoine/logs"
OK=0; KO=0
chk(){ if [ "$1" = "$2" ]; then OK=$((OK+1)); echo "  ok    $3"; else KO=$((KO+1)); echo "  ECHEC $3  (attendu '$2', obtenu '$1')"; fi; }

echo "=== B1 · sortie TOUJOURS 0, meme dégradé (sinon la session casse) ==="
for v in "nominal:" \
         "sans PATH:PATH=/usr/bin:/bin" \
         "racine inexistante:PROJECT_ROOT=/zzz/nexiste/pas" \
         "log dir inaccessible:COCKPIT_LOG_DIR=/zzz/nope"; do
  nom="${v%%:*}"; env="${v#*:}"
  rc=$(env $env bash .claude/hooks/tools-health.sh </dev/null >/dev/null 2>&1; echo $?)
  chk "$rc" "0" "tools-health sort 0 — $nom"
done
for v in "nominal:" "sans PATH:PATH=/usr/bin:/bin" "racine inexistante:PROJECT_ROOT=/zzz/nope"; do
  nom="${v%%:*}"; env="${v#*:}"
  rc=$(echo '{"prompt":"memorise"}' | env $env bash .claude/hooks/memorise-sync.sh >/dev/null 2>&1; echo $?)
  chk "$rc" "0" "memorise-sync sort 0 — $nom"
done
rc=$(echo '{}' | bash .claude/hooks/mempalace-session-start.sh >/dev/null 2>&1; echo $?)
chk "$rc" "0" "mempalace-session-start sort 0"

echo
echo "=== B2 · borne de temps (aucun hook ne doit PENDRE) ==="
for h in tools-health memorise-sync mempalace-session-start; do
  T=$(date +%s%N)
  echo '{"prompt":"x"}' | timeout 25 bash ".claude/hooks/$h.sh" >/dev/null 2>&1
  rc=$?; ms=$(( ($(date +%s%N)-T)/1000000 ))
  chk "$rc" "0" "$h termine en ${ms} ms (pas de timeout)"
done

echo
echo "=== B3 · execution CONCURRENTE (4 en parallele) — pas de corruption ==="
for i in 1 2 3 4; do ( bash .claude/hooks/tools-health.sh </dev/null > "$SC/conc$i.txt" 2>&1 ) & done
wait
n=$(sort -u "$SC"/conc*.txt | md5sum | cut -c1-8)
tous=$(for i in 1 2 3 4; do md5sum < "$SC/conc$i.txt" | cut -c1-8; done | sort -u | wc -l)
chk "$tous" "1" "les 4 executions concurrentes rendent un rapport IDENTIQUE"
w=$(powershell -NoProfile -Command "@(Get-CimInstance Win32_Process -Filter \"Name='grepai.exe'\" | Where-Object { \$_.CommandLine -match 'watch' }).Count" 2>/dev/null | tr -d '\r')
chk "$w" "1" "toujours UN SEUL watcher grepai apres 4 hooks concurrents"

echo
echo "=== C1 · jalon mempalace : un ECHEC ne doit PAS marquer comme fait ==="
JAL="C:/Users/Antoi/.claude/projects/c--Users-Antoi-Desktop-Forgia-Rewrite/memory/.derniere-synchro-mempalace"
touch "C:/Users/Antoi/.claude/projects/c--Users-Antoi-Desktop-Forgia-Rewrite/memory/MEMORY.md"
rm -f "$JAL"
# Python volontairement introuvable => mine echoue
COCKPIT_PYTHON="/zzz/python-inexistant" bash .claude/hooks/mempalace-sync.sh >/dev/null 2>&1
if [ -f "$JAL" ]; then KO=$((KO+1)); echo "  ECHEC jalon POSE malgre l'echec — les memoires seraient sautees pour toujours"
else OK=$((OK+1)); echo "  ok    jalon NON pose apres echec — le classement sera reessaye"; fi

echo "=== C2 · apres un succes, le jalon est pose et le 2e passage saute ==="
bash .claude/hooks/mempalace-sync.sh >/dev/null 2>&1
[ -f "$JAL" ] && { OK=$((OK+1)); echo "  ok    jalon pose apres succes"; } || { KO=$((KO+1)); echo "  ECHEC jalon absent apres succes"; }
out=$(bash .claude/hooks/mempalace-sync.sh 2>&1)
case "$out" in *SKIPPED*) OK=$((OK+1)); echo "  ok    2e passage saute";; *) KO=$((KO+1)); echo "  ECHEC 2e passage n'a pas saute : $out";; esac
touch "C:/Users/Antoi/.claude/projects/c--Users-Antoi-Desktop-Forgia-Rewrite/memory/MEMORY.md"
out=$(bash .claude/hooks/mempalace-sync.sh 2>&1)
case "$out" in *"SYNC OK"*) OK=$((OK+1)); echo "  ok    reprend le travail apres modification d'un memory";; *) KO=$((KO+1)); echo "  ECHEC ne reprend pas : $out";; esac

echo
echo "════════════════════════════════════"
echo "   REUSSIS : $OK      ECHECS : $KO"
echo "════════════════════════════════════"
[ "$KO" -eq 0 ] || exit 1

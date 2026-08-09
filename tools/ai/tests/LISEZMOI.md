# Bancs de test de l'outillage de session

> Écrits le 2026-08-09, après qu'une optimisation a **rétréci en silence** la
> couverture du scan anti-injection. Ils existent pour que ça se voie.

```bash
bash tools/ai/tests/hooks-securite.sh       # 44 cas — parité ancien/nouveau anti-injection
bash tools/ai/tests/hooks-robustesse.sh     # 17 cas — blocage, pendaison, concurrence, jalon
bash tools/ai/tests/hooks-angles-morts.sh   # 22 cas — faux positifs, BRP, git, coupe-circuits
python tools/ai/tests/grepai-non-regression.py   # 14 requêtes à réponse connue, hybride OFF vs ON
```

**À relancer après toute modification d'un hook**, et en particulier de
`anti-injection-scan.sh` : le banc sécurité compare la version courante à
`anti-injection-scan.sh.bak-2026-08-09`, seule référence de la sémantique d'origine.

## Ce que ces bancs ont trouvé, et qu'aucun usage normal n'aurait montré

| défaut | conséquence si non vu |
|---|---|
| `grep -I` au lieu de `-a` | un binaire porteur d'injection **n'était plus détecté** |
| `curl -m 0.2` | « BRP inactif » annoncé **avec le jeu lancé** |
| `checkpoint` non qualifié | la checklist mémoire s'injectait sur « ajoute un checkpoint au niveau 3 » |
| `m.{0,2}moris` trop large | déclenchait sur « la mémorisation des touches » |
| contrôle des coupe-circuits mono-source | `tools-health` coupé = **plus personne** ne le signale |

## Deux règles apprises à l'écriture de ces bancs

1. **Une sonde réseau se teste dans les DEUX sens.** Le cas « ça marche » est
   celui qu'on oublie, et c'est celui qui ment.
2. **L'instrument de mesure ment aussi.** Le premier test BRP utilisait un
   accept-loop maison qui perdait des connexions : il accusait le hook alors
   que la sonde directe était déjà incohérente. Toujours valider l'instrument
   avant d'accuser le produit.

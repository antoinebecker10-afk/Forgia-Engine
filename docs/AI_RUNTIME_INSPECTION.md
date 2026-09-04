# Inspection runtime de Forgia par Claude et Codex

Le pont Bevy Remote Protocol (BRP) est installé comme serveur MCP `bevy-brp`.
Il permet à un agent d'inspecter les entités, composants et ressources du jeu
pendant son exécution, au lieu de raisonner uniquement à partir du code.

Le pont est volontairement désactivé par défaut. Un build ou lancement normal
ne compile pas le transport distant et n'ouvre aucun port supplémentaire.

## Utilisation

1. Déclarer le serveur MCP `bevy-brp` (binaire `bevy_brp_mcp`) dans la configuration
   MCP de l'agent, par exemple un `.mcp.json` à la racine du dépôt, puis redémarrer l'agent.
2. Lancer le jeu depuis la racine avec `cargo forgia-brp`.
3. Demander à l'agent d'utiliser `bevy-brp` pour inspecter le jeu vivant.

## Premier scénario rejouable

Le scénario `roguelite_first_contact` lance le vrai binaire, attend une run
jouable, avance, tire, compare les snapshots BRP et collecte les capteurs :

```powershell
python tools/ai/run_brp_scenario.py
```

Le rapport et le log sont écrits dans `target/forgia_agent/`. Pour exercer un
jeu déjà lancé avec `cargo forgia-dev` :

```powershell
python tools/ai/run_brp_scenario.py --attach
```

La définition versionnée et ses seuils vivent dans
`tools/ai/scenarios/roguelite_first_contact.json`. Les seules commandes métier
ajoutées sont `forgia.scenario.act`, `forgia.scenario.aim_at` et
`forgia.scenario.snapshot` ; elles
n'existent pas sans la feature `dev-brp`.

Le scénario de combat vise ensuite un ennemi du snapshot avec la vraie caméra,
tire jusqu'au premier kill observé et vérifie dommage, killfeed, retrait de
l'entité, loot et progression de vague :

```powershell
python tools/ai/run_brp_scenario.py tools/ai/scenarios/roguelite_combat_first_kill.json
```

L'audit locomotion de l'Expédition exerce les entrées réelles et compare, pendant
chaque geste, l'état demandé au clip effectivement résolu :

```powershell
python tools/ai/run_brp_scenario.py tools/ai/scenarios/expedition_animation_audit.json
```

Il couvre repos, marche, sprint, recul, pas de côté, saut, accroupissement,
glissade et tir, plus diagonale, sprint+saut, sprint+virage et déplacements
accroupis. Chaque transition doit arriver en moins de 500 ms. Le rapport
certifie le câblage moteur ; il ne remplace pas une
revue visuelle des déformations du maillage, des cheveux ou de la cape.

La marche bornée du spawn jusqu'au premier campement vérifie la continuité du
déplacement et l'absence d'état d'atterrissage parasite sur les pentes :

```powershell
python tools/ai/run_brp_scenario.py tools/ai/scenarios/expedition_first_camp_walk.json
```

## Debug à la main : piloter le jeu depuis un terminal

Un scénario rejoue une preuve écrite d'avance. Pour diagnostiquer, il faut
pouvoir appuyer sur une touche et regarder — `tools/ai/brp.py` fait ça, sur le
jeu déjà lancé par `cargo forgia-dev` :

```powershell
python tools/ai/brp.py snapshot                  # une ligne : mode, position, vitesse, état, clip, touches tenues
python tools/ai/brp.py watch --seconds 8 --changes-only
python tools/ai/brp.py key KeyR                  # tape R ; --hold pour tenir, --release pour relâcher
python tools/ai/brp.py look --yaw 90             # tourne de 90° PAR LA SOURIS (vraie chaîne mouse_look)
python tools/ai/brp.py act sprint_forward --frames 60
python tools/ai/brp.py release-all               # le filet : rien ne reste collé
```

### Ce que l'entrée injectée traverse — et ce qu'elle ne traverse pas

🚨 L'appui part en message `KeyboardInput`, comme celui de winit : le moteur en
dérive lui-même `pressed` ET `just_pressed`. Une écriture directe dans
`ButtonInput<KeyCode>` depuis `First` ne le permettait pas — `keyboard_input_system`
appelle `clear()` en `PreUpdate` — et les **62 lecteurs** `just_pressed(KeyCode::…)`
du workspace (recharge, console, éditeur) restaient hors de portée : le symptôme
aurait été « aucun effet » sur une feature saine.

Le regard passe par `MouseMotion` et la sensibilité réellement en vigueur
(ADS comprise), donc par `mouse_look`. Reste **non couvert** : `forgia.scenario.aim_at`
écrit encore `Transform` + `Player.yaw` directement — c'est un raccourci de visée
pour les scénarios de combat, pas une preuve de la chaîne regard.

Le snapshot publie `inputs` (touches tenues, souris, touches collantes, panne du
harnais) : « il ne se passe rien » se tranche entre *rien n'a été demandé*, *la
touche part mais le jeu l'ignore*, et *le harnais est en panne* (fenêtre absente).

Commandes exposées par la feature `dev-brp` : `forgia.scenario.act`, `.stop`,
`.key`, `.look`, `.release_all`, `.aim_at`, `.follow_first_camp`, `.snapshot`.

BRP écoute par défaut uniquement sur `127.0.0.1:15702`. Ne pas activer la
feature `dev-brp` dans un build distribué.

## Audit des dépendances

`cargo forgia-supply-chain` contrôle les avis RustSec, les dépendances bannies
et l'origine des crates. La configuration initiale bloque les vulnérabilités et
les sources inattendues, mais laisse les doublons historiques en avertissement
afin de ne pas transformer l'adoption de l'outil en régression.

Le premier passage a détecté de la dette déjà présente dans `Cargo.lock`, dont
`crossbeam-epoch 0.9.18` (RUSTSEC-2026-0204) et `quick-xml 0.39.4`
(RUSTSEC-2026-0194). L'outil les rend visibles sans modifier automatiquement les
versions : leurs mises à niveau doivent être traitées et testées séparément.

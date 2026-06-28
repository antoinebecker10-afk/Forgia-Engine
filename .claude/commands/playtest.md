# Playtest — Plan de playtest + passe de balance (Forgia)

Prepare une session de playtest ciblee et une passe d'equilibrage data-driven, lue via les sensors.

**Cible** : $ARGUMENTS
> Ex : `/playtest armes`, `/playtest economie ames`, `/playtest difficulte run`.
> Defaut : la boucle Roguelite complete.

Adapte de BMAD Game Dev Studio v6.8 (`playtest-plan`, `balance-testing`, `performance`). Methode conservee, **metriques recablees sur les sensors Forgia + hot-reload genome**.

## 1. Definir l'hypothese (pas "on regarde si c'est fun")

Une session teste **UNE hypothese falsifiable**. Formuler :
> "Hypothese : <ex: Lenoir surclasse les 3 autres armes au-dela de la zone 2>. Refute si <sensor montre TTK Lenoir ≈ autres armes>."

## 2. Preparer le build (regle binaire = preuve)

- TOML genome modifie → **Shift+F12** (hot-reload, pas de rebuild).
- `.rs` modifie → `cargo run -p forgia --profile release-fast`.
- **Verifier la fraicheur** avant de conclure (multi-terminal-coordination §5) : `mtime(source) ≤ mtime(binaire) ≤ mtime(sensor)`. Sinon le sensor reflete l'ancien code.

## 3. Boucle de balance (5 etapes, BMAD adapte data-driven)

1. **Intention design** : quelle est la cible (ex : TTK fodder ≈ 0.3s, elite ≈ 2s) ? Vient du GDD §7.
2. **Modeliser dans le genome** : poser/ajuster les valeurs dans `config/genomes/*.toml` (couche `definition`, pas de hardcode Rust).
3. **Jouer** la session selon l'hypothese.
4. **Lire la telemetrie** (sensors, pas l'impression) — table §4.
5. **Iterer** via Shift+F12 (hot-reload genome live), re-mesurer, converger.

## 4. Metriques Forgia (quoi lire, ou)

| Dimension | Sensor (chemin V2) | Champs / signaux | Red flag |
|---|---|---|---|
| Armes / combat | `forgia2_combat.json` (+ `forgia2_pepin/bourrasque/lenoir/boucherie.json`) | TTK par arme, hs_ratio, DPS effectif | 1 arme domine (TTK tres < autres) |
| Elements | `forgia2_elements.json` | usage par element, matchups | 1 element couvre tout |
| Boons / build | `forgia2_boons.json` | pick rate par boon, diversite de build | boon jamais pris / toujours pris |
| Run / progression | `forgia2_roguelite_state.json` | longueur run, zone atteinte, morts par cause | feast/famine, mur de difficulte |
| Economie | `forgia2_roguelite_state.json` (ames/or) | taux de gain, depense | inflation / disette |
| Perf | `forgia2_perf.json`, `forgia2_lag_events.json` | FPS, frame budget, stutter | stutter > seuil sur spawn/VFX |
| Feel armes | `forgia2_barks.json`, `forgia2_fps_feel.json` | barks, recul/feedback | silence / feedback absent |

## 5. Pieges de balance a guetter (BMAD)

- **Power creep** : chaque nouvelle arme/boon plus forte que la precedente.
- **Strategie dominante** : 1 arme/element/build optimal → les autres morts.
- **Feast/famine** : economie binaire (trop riche ou ruine, jamais entre).
- **Analysis paralysis** : trop de boons/choix → le joueur subit au lieu de decider.

## 6. Sortie

1. **Plan de session** : hypothese + build a lancer + sensors a ouvrir + scenario precis (ex : "run jusqu'a zone 3 avec chaque arme").
2. **Recap test in-game** (regle in-game-test-recap, 5 elements) : action / hot-reload ou restart / effet attendu falsifiable / sensor+champ+valeur / 2 variantes si KO.
3. **Apres la session** : rapport court → `Forgia Rewrite/docs/design/playtest-<slug>-<date>.md` (hypothese, mesures sensor, verdict, ajustements genome proposes) → candidate `story-NNN` si un fix est requis.

## Fin

Ne PAS conclure "c'est equilibre" sans chiffre sensor. Toute reco de changement de valeur = (a) sensor qui montre le desequilibre, (b) gene genome a ajuster, (c) cible design (GDD §7). Sinon → no-speculative-fix.

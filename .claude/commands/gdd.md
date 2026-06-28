# GDD — Game Design Document (Forgia, FPS Roguelite)

Cree, met a jour ou valide le GDD d'un jeu Forgia.

**Cible / mode** : $ARGUMENTS
> Ex : `/gdd create roguelite`, `/gdd update armes`, `/gdd validate`.
> Defaut sans argument : le **Roguelite** (jeu Track SHIP, cf CLAUDE.md §1, reference **Gunfire Reborn**).

Adapte de BMAD Game Dev Studio v6.8 (`gds-gdd`, templates `roguelike` + `shooter` fusionnes). Methode conservee, **plomberie recablee sur la realite Forgia** (Bevy/Rust, genome, sensors, stories).

## Principe directeur (BMAD, garde-le)

- **Le GDD est la source de verite design** : il nourrit les stories, l'architecture, le playtest. Garde-le **lean, precis, tracable**.
- **Decouverte, pas questionnaire** : conversation, pas quiz. Si un game brief / une vision / des stories existent, **extraire** au lieu de re-demander.
- Chaine : **Core Fantasy → Piliers → Core Loop → Mecaniques → Epics**.
- **Chiffres concrets, jamais vagues** (cadence de tir, cooldown, degats, longueur de run) — mais voir la regle Forgia ci-dessous sur OU ils vivent.
- **Conventions de genre documentees, jamais supposees.**
- **Detail d'implementation moteur DEHORS ; ce que le JOUEUR vit DEDANS.**

## Regles Forgia (l'adaptation Bevy/Rust)

1. **Data vs code (concept-first etape 0 + no-hardcode)** : les valeurs numeriques de balance ne sont PAS dans le GDD en dur ni dans le Rust. Le GDD **nomme le gene** et la cible design ; la valeur vit dans `config/genomes/*.toml` (couche `definition`). Format : `degats_base = <gene: weapon_pepin_damage> (cible design : ~X, justifier)`.
2. **Observabilite (observability-required)** : chaque "Success Metric" doit pointer un **sensor** `forgia2_<feature>.json` + le champ a lire. Pas de metrique non observable.
3. **Epics = stories** : ne reinvente pas le suivi. Chaque epic se decline en `story-NNN` dans `Forgia Rewrite/docs/stories/`. Reference les IDs, ne duplique pas le contenu.
4. **Documente l'EXISTANT d'abord** : le Roguelite a deja des systemes livres (4 armes a identite : Pepin/Bourrasque/Lenoir/Boucherie ; elements par-arme ; boons ; meta-progression "L'Enclume des Ames" ; obstacles ; FTUE story-597). Le GDD acte ce qui existe (sensor a l'appui) AVANT de specifier le neuf.
5. **Vision = filtre de scope** : tout item passe le test "ca debloque le ship Roguelite ?" (CLAUDE.md §1, pivot 2026-06-04).

## Sortie

Ecrire dans **`C:/Users/Antoi/Desktop/Forgia Rewrite/docs/design/gdd-<slug>.md`** (le GDD voyage avec le jeu, versionne dans le repo V2).

## Template (maitre + sections FPS-roguelite fusionnees)

```
# GDD — {Titre}   (type: FPS Roguelite · plateforme: PC 1920x1080)

## 1. Resume executif
- Core concept (1-2 phrases) · Public cible · USP (vs Gunfire Reborn / Roboquest)

## 2. Objectifs & contexte
- Objectifs projet · Rationale · Lien Track SHIP

## 3. Core gameplay
- Piliers (3-5, falsifiables) · Core loop (boucle d'un run) · Conditions victoire/defaite

## 4. Mecaniques
- Mecaniques primaires · Controles & input (leafwing AZERTY)

## 5. Armes & combat (shooter)
- Types d'armes · Stats (degats/cadence/precision/reload/munitions) → genes genome
- Identite par arme (gimmick) · Feel (recul, son, impact) · Visee (hitscan vs projectile, weak points)

## 6. Structure roguelite
- Structure de run (longueur, etapes, scaling) · Generation procedurale (niveaux/loot/biomes/seed)
- Permadeath & ce qui persiste · Items/upgrades (boons, rarete, synergies, elements) · Personnages · Modificateurs de difficulte

## 7. Progression & balance
- Progression joueur (run + meta "Enclume des Ames") · Courbe de difficulte · Economie (ames/or)
- ⚠ Chaque valeur → gene `config/genomes/*.toml` (nommer le gene + cible design)

## 8. Level / arena design
- Types de salles · Flow d'arene (chokepoints, verticalite) · Placement power-ups · Hazards

## 9. Art & audio
- Style visuel (toon/PBR) · Audio/musique (biome, barks armes)

## 10. Specs techniques (Bevy/Rust)
- Budget frame · GameSet ordering impacte · Sensors a creer

## 11. Epics → stories
- Epic → liste story-NNN (statut). Pas de detail ici, juste le mapping.

## 12. Success metrics (→ sensors)
- Technique : forgia2_perf.json / forgia2_lag_events.json (FPS, stutter)
- Gameplay : TTK par arme (forgia2_combat.json), pick rate boons (forgia2_boons.json),
  longueur run (forgia2_roguelite_state.json), morts par cause (...)

## 13. Hors-scope
## 14. Hypotheses & dependances
```

## Modes

- **create** : conversation de decouverte → remplir le template. Section vide = question ciblee (1 a la fois), pas un quiz.
- **update** : lire le GDD existant, **signaler les conflits** avec les decisions anterieures AVANT de modifier, puis patch scope.
- **validate** : auditer contre — (a) conventions de genre FPS roguelite, (b) chiffres concrets ET loges en genome (pas de hardcode), (c) chaque epic a une story, (d) chaque success metric a un sensor, (e) detail moteur absent. Rapport : OK / a corriger par section.

## Fin

Afficher le chemin du GDD ecrit + 3 prochaines actions concretes (ex : "story-NNN a creer pour epic X", "gene Y manquant en genome", "sensor Z absent → observability-required").
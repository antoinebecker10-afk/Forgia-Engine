# Les Cryptes de l'Enclume

> 🌋 **Arène polish target V1**

## C'est quoi en 2 phrases

Un **vieux volcan transformé en usine-prison** par le Forgeron Noir. C'est là qu'il fabrique ses armes muettes et qu'il garde les âmes-armes dans des cages.

## Ambiance visuelle

**Mélange "donjon volcanique" + "fête foraine triste"** :
- Murs en roche sombre, lave coulant en arrière-plan (loin, jamais dangereuse pour le joueur)
- Cendres qui tombent doucement (ashfall, déjà dans le genome)
- **Mais aussi** : lampions roses, banderoles décrochées, écriteaux gravés par les âmes prisonnières (*"courage" / "Madame Lenoir était là" / "vive le marché"*)
- Petits **champignons lumineux** bleus, jaunes, roses (touche mignonne contre la noirceur)
- Forges éteintes, enclumes à moitié cassées, marteaux laissés là
- Tas de **cages en métal tordu** vides (anciennes prisons, libérées par d'autres ou abandonnées)

**Palette** : rouge braise + noir suie + **touches roses pastel** + bleu champignon. Le rose c'est le secret — il rend le lieu accueillant malgré l'enfer théorique.

## Ambiance sonore

- Fond : low rumble continu (volcan respire)
- Métal qui résonne au loin (Forgeron Noir bosse)
- Cendres qui tombent (très soft, presque pluie)
- **Petits tintements** quand le joueur passe près d'un coin avec champignons (mignonification)
- Musique : voir story future audio biome

## Identité narrative

C'est **pas** un cachot torture. C'est **pas** un enfer demonic. C'est :

- Un **lieu de travail mal organisé** où un grognon a accumulé du foutoir
- Avec des traces touchantes des âmes qui y ont vécu (graffiti, lampions)
- **Triste** quand on regarde de loin, **mignon** quand on regarde les détails

C'est l'esthétique de Cult of the Lamb : sinistre digestible, glissé de touches lumineuses.

## Sections de l'arène (à designer phase 2)

Cohérent avec `assets/genomes/roguelite_stages.toml:17` (volcanique, ashfall, sight-line longue + cover dense + sniper perch + melee pit).

| Section | Identité | Props clés |
|---|---|---|
| **L'entrée** | grande porte rouillée, premières cages vides, banderoles | grandes cages métal vides, banderole *"Bienvenue à l'Enclume"* (rongée), 2 lampions rose |
| **La cour des forges éteintes** | open mid, plusieurs enclumes éclatées | 4-6 enclumes en demi-cercle, 1 marteau planté dans le sol, vapeurs douces |
| **Le perchoir du contremaître** | sniper perch, vue plongeante, balustrade en métal tordue | échelle, 1 chaise renversée, écriteau *"je reviens dans 5min"* |
| **La fosse à mélée** | melee pit central, lave en contrebas (visuel, pas dangereux) | sol rond, 4 cages renversées en cover bas, gradins effrités |
| **Le couloir des graffiti** | corridor transition entre sections | murs couverts d'écriteaux gravés par les âmes (lore drop) |
| **L'arène du boss** | grande halle finale, le Forgeron Noir y trône | enclume géante centrale, son trône en boucliers fondus, fournaise active loin derrière |

## Assets à acquérir (PolyHaven CC0 + KayKit)

| Catégorie | Source proposée | Note |
|---|---|---|
| **Roche volcanique texture** | PolyHaven `volcanic_rock_*` | déjà disponible CC0 |
| **Marble Cliff 05** | PolyHaven (mention veille du jour 2026-05-26) | pour falaises arrière-plan |
| **KayKit Dungeon Pack** | déjà copié V1→V2 (chest, walls) | étendre avec props enclumes |
| **Lampions / banderoles** | à modéliser ou trouver pack médiéval-fête | touche mignonne |
| **Champignons lumineux** | bevy_hanabi VFX + meshes basiques | + emissive material |
| **Cendres qui tombent** | bevy_hanabi particles ambient | déjà supporté par stack |

## À NE PAS faire

- ❌ Esthétique gore / sang / crâne / chair (mauvaise audience)
- ❌ Trop sombre (joueur doit voir où il va et les couleurs des armes parlantes ressortent)
- ❌ Lave qui blesse (visuel only)
- ❌ Ennemis qui hurlent terrifiant (sons cartoon impact, pas horror)
- ❌ Symboles occultes / pentagrammes (zéro)
- ❌ Effets de "possession démoniaque" sur les âmes prisonnières (elles dorment ou pleurent doucement, c'est tout)

## Cross-refs

- Genome stage : `assets/genomes/roguelite_stages.toml` ID `crypts_of_anvil`
- Pipeline stage : `forgia-stage::layout` (ex-stage-arena)
- Sensor : `forgia2_stage.json` + `forgia2_stage_layout.json`
- Atmosphere : à wire (ambient sound, particle ashfall, lighting)

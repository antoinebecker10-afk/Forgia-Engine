# Le Forgeron Noir — le grand méchant

> 👹 **Antagoniste / Boss final**

## Qui c'est

Un **gros forgeron grognon** qui s'est dit un jour : *"Les armes qui parlent, c'est ringard. Une vraie arme se tait."* Il s'est mis à voler les âmes des armes pour fabriquer des fusils muets qu'il vend à des bandits. Pour lui c'est du **business**.

Il habite dans les **Cryptes de l'Enclume** (un volcan en partie effondré qu'il a transformé en usine-prison). Il fabrique des cages, il fabrique des armes muettes, il compte ses sous, il râle.

Il n'est pas démoniaque. Il n'a pas de plan d'invasion mondiale. **Il est juste un gros méchant grognon qui a un mauvais business model** et qui doit être arrêté.

## Apparence (briefing artiste)

- Très grand, très gros, **silhouette comique** (genre Bowser, Gaston, Ratigan)
- Tablier de cuir clouté
- Casque en forme d'enclume renversée (l'a soudé à la main, en est très fier)
- Marteau géant aussi haut que lui
- Yeux qui brillent comme des braises (rouge orangé), mais expression de **bouder**, pas de fureur démoniaque
- Tout couvert de suie. Laisse des traces noires partout.

## Comment il parle

| Tic | Exemple |
|---|---|
| **Râle tout le temps** | *"Pffff." / "Voyez-vous ça." / "Encore lui."* |
| **Vocabulaire commercial** | *"Stock" / "Marge" / "Inventaire" / "Clientèle"* |
| **Méprise les armes parlantes** | *"Bavardes. Inutiles. Mauvais investissement."* |
| **Pas de cri menaçant épique** | il **boude** plus qu'il ne menace |
| **Pas de gros mots** | *"Sacrebleu !" / "Crénom de crénom !"* |

## Voicelines à écrire (V1)

### Boss intro (long, ~5 sec)

> *"Tiens. L'Apprenti. T'aurais dû rester à ton atelier, gamin. Tes bavardes là, c'est du déchet. Allez, viens m'expliquer ton business model."*

### Boss combat (en boucle, ~12 lignes)

- *"Pffff. Encore debout."*
- *"Inadmissible. Tu casses mon stock !"*
- *"Voilà ce que ça donne, les bavardes. Médiocre."*
- *"Reste là. Bouge plus. Stop."*
- *"Tu vois ? Une arme qui se tait, ça marche mieux."*
- *"Ma marge, gamin. Tu touches à ma marge."*

### Phase 2 (50% HP, enrage)

> *"Bon. Ça suffit. J'augmente la cadence de production."*

### Défaite (perd le boss, ~3 sec)

> *"Pffff. Bavardes. ...Bon. Reposez-vous, alors. Vous l'avez mérité, je suppose."*

(Note : sur sa défaite il **libère lui-même** les dernières âmes, comme un comptable résigné. Pas de mort à l'écran, il tombe assis, fatigué. Ton "ok t'as gagné, j'ai compris".)

### Hub (post-victoire, V2 possible)

V2 : possibilité qu'il devienne un NPC grognon qui tient un magasin. Genre Hadès post-game.

## Voix souhaitée

- Homme 50-65 ans, voix grave, **bourrue mais comique**
- Pas de hurlement, pas de rire démoniaque
- Ton "marchand qui se plaint de ses charges"
- **Référence** : Gru jeune (Despicable Me), Père Fouras (mais méchant), Eddie Murphy en mode bougon, Bowser des films récents (râleur pas terrifiant)

## À NE PAS faire

- ❌ Forgeron Noir terrifiant horror (mauvaise audience)
- ❌ Mention de Liva, sœur, exil, passé tragique (V1 reste simple — V2 peut creuser)
- ❌ Rire démoniaque "MOUAHAHA"
- ❌ Voix démoniaque modulée caverne
- ❌ Mort à l'écran (on dit *"il s'assoit, il a fini sa journée"*)
- ❌ Religion / occulte / démon (zéro — c'est un grognon, point)

## Mécanique boss (rappel code)

Source : `assets/genomes/roguelite/roguelite_enemies.toml` (Le Forgeron Noir)

- Phase 1 : hammer slam AoE 6m
- Phase 2 : enrage à 50% HP (spawn 4 Runts, ×1.6 speed, ×1.4 dmg)
- Cost director : 200

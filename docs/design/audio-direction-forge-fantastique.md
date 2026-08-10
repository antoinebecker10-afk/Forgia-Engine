# Direction audio — Forge fantastique cartoon

## Promesse

Le joueur doit reconnaître une arme sans regarder l'écran. La forge est le
vocabulaire commun ; chaque persona possède ensuite sa matière et son rythme.

| Persona | Masse | Attaque | Signature | Espace |
|---|---:|---|---|---|
| Pépin | légère | claquement sec | petit mécanisme brillant | courte |
| Bourrasque | moyenne | souffle fractionné | montée d'air vive | mobile |
| Madame Lenoir | lourde tenue | détonation nette | cristal/métal noble | longue |
| Boucherie | très lourde | poussée de fournaise | grondement et débris | large |

## Monde

- Pas : six prises Foley réelles Kenney CC0, filtrées et atténuées ; jamais de
  tintement systématique ni de normalisation au plafond.
- Interface/récompense : métal harmonique ascendant, jamais un bip abstrait.
- Danger : fondamentale grave, frappes d'enclume et bruit filtré.
- Ambiance : cœur de forge, air chaud, grondement et frappes espacées.
- Musique : 96 BPM, pulsation d'enclume, bourdon modal, motif simple laissant
  de la place au combat et aux futures voix.

## Contraintes de mix

- Aucun SFX fréquent ne dépasse la signature weakspot en brillance.
- Boucherie porte le grave ; les autres armes évitent de remplir cette zone.
- Musique et ambiance restent sous les transitoires de gameplay.
- Toute variation aléatoire doit rester déterministe ou bornée côté runtime.
- Les boucles sont sans couture ; toute prise tierce doit être CC0, versionnée
  avec sa source et sa licence.

## Révision anti-fatigue après écoute — 2026-08-05

La première passe a été rejetée : musique désagréable, pas artificiels et fatigue
globale. Causes confirmées dans les masters : saturation appliquée partout,
partiels métalliques inharmoniques trop présents, `loudnorm` identique sur les
sons courts, boucle musicale de 32 s trop statique et pas modélisés comme une note.

Corrections V2 :

- aucune saturation globale ; true peak par catégorie (-4 à -8 dB) ;
- musique -24 LUFS, ambiance -30 LUFS, spectre limité à 8/6,5 kHz ;
- forme musicale 60 s A–B–A′, motif avec réponse, résolution et vraies respirations ;
- pas remplacés par six prises Foley Kenney CC0, filtrées à 45 Hz–5,2 kHz et -12 dB ;
- partiels métalliques quasi harmoniques et très décroissants ;
- stress previews 5 min dans `target/audio-fatigue/` pour musique et pire cas de pas répété.

Références : ITU-R BS.1770-5 pour la mesure de loudness/true peak ; AES
« Avoiding Tedium » pour la répétition ; Turchet et al. pour l'interaction
pied–sol et la variabilité spectrale ; Salimpoor et al. pour anticipation puis
récompense musicale.

## Révision des armes après écoute — 2026-08-05

La base synthétique des quatre tirs a été rejetée à l'écoute. Elle est remplacée
par quatre prises réelles CC0 de la Free Firearm Sound Library, sans couche
synthétique audible. Le traitement conserve les transitoires et la dynamique :
passe-haut/passe-bas léger, gain propre à chaque arme, limiteur sans compensation
automatique et marge true-peak mesurée de -5,9 à -10 dBFS.

- inspiration Fortnite : réponse immédiate et cadence lisible ;
- inspiration Overwatch : silhouette distincte reconnaissable hors écran ;
- inspiration Gunfire Reborn : exagération fantasy réservée aux armes rares ;
- règle anti-fatigue : Pépin et Bourrasque restent courts et moins massifs,
  Lenoir et Boucherie peuvent occuper davantage d'espace.

Il s'agit de principes de design, pas de copies de sons propriétaires.

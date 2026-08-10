# Forgia Original Audio — « Forge fantastique cartoon »

Les événements, impacts, pickups, mouvements, ambiances et musiques sont des
créations procédurales originales du projet Forgia, générées sans sample tiers
par `tools/audio/generate_forgia_da.py` (graine `0xF0A61A`).

Les quatre tirs sont des éditions d'enregistrements réels issus de la **Free
Firearm Sound Library**, CC0. Sources lossless, auteurs et licence :
`assets/audio/sources/free_firearm/`. Le design suit une hiérarchie anti-fatigue :
Pépin et Bourrasque, très fréquents, sont courts et faibles ; Lenoir et Boucherie,
plus rares, conservent davantage de masse et de queue.

Les six fichiers `footsteps/forge_stone_*.ogg` sont des éditions des prises
`footstep00.ogg` à `footstep05.ogg` du pack **RPG Audio** de Kenney Vleugels :

- Source : https://www.kenney.nl/assets/rpg-audio
- Licence : Creative Commons CC0 1.0
- Sources et licence : `assets/audio/sources/kenney_rpg/`
- Édition Forgia : passe-haut 45 Hz, passe-bas 5,2 kHz, gain -12 dB, mono 48 kHz.

- Format runtime : OGG Vorbis, 48 kHz.
- Mix : traitement différencié par catégorie ; aucune normalisation commune au
  plafond. Les tirs gardent au moins 5,9 dB de marge true-peak mesurée.
- Masters reproductibles : `target/audio-masters/` (non versionnés).
- Direction : forge fantastique cartoon — métal, souffle, braise, percussion
  d'enclume et modalité sombre mais chaleureuse.

## Bande-son (2026-08-05)

Les onze pistes `music/hub.ogg` et `music/chapter_01.ogg` → `chapter_10.ogg`
(thème du hub + une piste par chapitre du Livre) ont été composées via **Suno**
(plan payant — droits d'usage commercial) sur direction d'Antoine Becker :
orchestral élégant mené aux violons, dynamique sans agressivité, accents
d'enclume. Intégration Forgia : trim des silences, loudnorm I=-16 LUFS
(TP -1,5 dB), OGG Vorbis 48 kHz, lecture en boucle continue par chapitre.
`music/forged_destiny_loop.ogg` (procédural) reste le fallback des chapitres
sans piste dédiée.

Les créations procédurales appartiennent au projet Forgia. Les attributions
Kenney et Free Firearm Sound Library ne sont pas requises par CC0, mais elles
sont conservées volontairement.

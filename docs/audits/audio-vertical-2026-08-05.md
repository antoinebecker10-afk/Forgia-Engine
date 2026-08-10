# Audit et vertical slice audio — 2026-08-05

## Résultat

Le Roguelite dispose maintenant d'un chemin audio complet et pilotable : mixage
par catégories, musique continue, ambiance, SFX de combat et de boucle de jeu,
pas, branchement des voix par réplique et capteurs runtime.

## Couverture livrée

| Domaine | État | Source de vérité |
|---|---|---|
| Mixage | master + musique + SFX + voix + ambiance, persistant et live | `UserAudioVolumes`, `UserSettings` |
| Combat | tirs par arme, impacts, weakspots, kills, dégâts | `roguelite_audio.toml` |
| Mobilité | dash et pas cadencés selon vitesse/sol, variations légères | `roguelite_audio.toml` |
| Armes | début/fin reload, changement d'arme | événements `AmmoChanged` |
| Boucle | coffre, boon, début/fin de vague, boss, victoire/défaite | événements Roguelite |
| Musique | boucle sans redémarrage inutile, volume live | instance Kira suivie |
| Ambiance | boucle séparée, volume live | instance Kira suivie |
| Barks | texte existant + `audio_path` optionnel par ligne | `roguelite_dialogue.toml` |
| Observabilité | compteurs musique, ambiance, SFX, pas et voix | capteurs `forgia2_*audio.json` et `forgia2_barks.json` |

## Choix anti-régression

- Les volumes sont appliqués à chaque instance : le volume de canal ne compose
  pas de façon fiable avec `with_volume` dans bevy_kira_audio 0.25.
- Une musique ou ambiance déjà active change de gain sans redémarrer.
- Les barks restent fonctionnels en texte si aucune voix correspondante n'existe.
- Aucun fichier vocal anglais historique n'est plaqué sur une réplique française.
- Tous les nouveaux chemins et niveaux restent éditables à chaud dans le genome.

## Risques et contenu restant

1. **Licence audio-v1 à reconstituer avant distribution.** Le dossier hérité ne
   contient pas d'attribution fiable et il est ignoré par Git. Les nouveaux
   sons doivent privilégier `assets/audio/roguelite/`, dont les crédits sont versionnés.
2. **Voix françaises à produire.** L'infrastructure est prête (`audio_path` par
   ligne), mais les prises correspondant exactement aux quatre personas manquent.
3. **Mix final à l'oreille.** Les valeurs livrées sont des bases conservatrices ;
   une passe en jeu sur plusieurs casques/enceintes reste nécessaire.

## Validation mécanique

- `cargo check -p forgia-ui-lib -p forgia-mode-roguelite -p forgia-game`
- `cargo test -p forgia-audio --lib`
- `cargo test -p forgia-ui-lib --lib`
- `cargo test -p forgia-mode-roguelite --lib`
- lancement du binaire et lecture des capteurs audio.

## Passe DA originale — « Forge fantastique cartoon »

Un premier pack propriétaire remplace désormais toute la verticale Roguelite :
29 OGG générés sans sample tiers, dont quatre signatures d'armes, six pas,
combat/pickups, mouvements, événements, ambiance stéréo et musique stéréo.

- Générateur : `tools/audio/generate_forgia_da.py`.
- Direction : `docs/design/audio-direction-forge-fantastique.md`.
- Exports : `assets/audio/forgia_original/`.
- 48 kHz, cible -18 LUFS / true peak -1,5 dB ; 0 chemin manquant.
- Boucles fondées sur des composantes périodiques, sans fondu vers le silence à
  chaque répétition.
- Runtime Vulkan : musique + ambiance actives, 40 SFX joués en 13 s, 0 asset échoué.

La dette de licence `audio-v1` subsiste dans les autres modes, mais la verticale
audio Roguelite livrée ici n'en dépend plus.

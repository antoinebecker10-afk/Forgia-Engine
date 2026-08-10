# Classification des éléments V1 non portés

Date : 2026-08-05  
Scope : livraison du Roguelite V2

## Décision

Les commentaires « port from V1 » n'étaient pas un backlog fiable. Ils
mélangeaient des fonctions déjà remplacées, des systèmes RPG hors scope et des
prototypes sans implémentation. Aucun portage en masse n'est autorisé : chaque
retour doit répondre à un besoin joueur actuel et respecter l'architecture V2.

## Déjà remplacé en V2 — ne pas reporter

| Élément V1 | Équivalent actuel |
|---|---|
| Viewmodel / recoil / hit feedback | `forgia-combat`, crates `forgia-juice-*`, hitmarker et damage numbers |
| Reload / munitions | `forgia-combat::ammo` et configuration d'arme Roguelite |
| Health générique | `Health` partagé et composants propres aux modes |
| Nettoyage des VFX temporaires | `lifetime_tick` et `emissive_fade_tick` dans `forgia-effects` |
| Tracers et impacts | pools persistants et warmup Hanabi de `forgia-effects::weapon_vfx` |

## À livrer seulement avec une feature Roguelite identifiée

| Élément | Condition de réintroduction | Risque |
|---|---|---|
| SFX de combat / ambiance | Événements typés, assets audio validés et budget de mixage | Moyen |
| Boss VFX | Boss réellement présent dans la boucle jouable | Moyen |
| Level-up VFX | Événement de progression Roguelite stabilisé | Faible |
| Ciblage | Besoin explicite d'une arme ou aptitude | Moyen |
| Vignette | Besoin UX mesuré, intégrée à la passe post-process existante | Élevé |

## Hors scope Roguelite — différer

- `rpg_systems`, `gcd`, sorts feu/glace, bouclier RPG ;
- particules de biome, météo et vent dépendant du terrain monde ouvert ;
- mêlée V1 tant qu'aucune arme Roguelite ne la demande ;
- ambiance de biome et instances de village.

## À ne pas activer

- Les 43 effets post-process passthrough : ils exposent une plomberie Rust mais
  leurs shaders restent des stubs. Un shader doit être réel et testé avant que
  son plugin soit branché.
- Une seconde passe `outline` distincte de `toon` : la composition double passe
  a déjà provoqué un crash wgpu. Le contour est désormais intégré à `toon.wgsl`.
- Les portages verbatim V1 : ils réintroduiraient des dépendances et contrats
  supprimés pendant la réécriture.

## Dette restante assumée

L'audio de combat demeure la lacune joueur la plus importante de cette liste,
mais l'implémenter sans assets, événements et cible de mixage validés créerait
une fausse feature. Il doit faire l'objet d'une feature verticale distincte,
avec test en jeu, plutôt que d'un portage automatique de l'ancien module.

---
paths:
  - "**/*.rs"
  - "**/*.toml"
  - "**/*.json"
---

# Creator Simplicity Rule (Forgia)

> Le createur cible est un joueur de 14 ans qui n'a jamais programme. Chaque parametre expose doit etre comprehensible en 3 secondes.

## Nommage des parametres
- Labels courts et concrets : "Vitesse", "Degats", "Portee" — pas "Coefficient de friction laterale avant"
- Vocabulaire joueur, pas ingenieur : "Rebond" au lieu de "Damping", "Grip" au lieu de "Coefficient d'adherence"
- Si un parametre necessite une explication, il est trop technique pour etre expose tel quel

## Regroupement & hierarchie
- Max 5-8 parametres visibles par categorie dans le Grimoire/Genome Editor
- Les parametres avances (inertie, damping, coefficients physiques) sont caches par defaut dans une section "Avance" repliee
- L'utilisateur Free voit les sliders essentiels, le Premium voit tout

## Sliders > champs numeriques
- Tout parametre expose au createur = slider avec min/max bornes sensibles
- Pas de valeurs absurdes possibles : un createur ne doit jamais casser son jeu en bougeant un slider
- Valeurs par defaut = "ca marche bien" sans toucher a rien

## Simplification par abstraction
- Preferer 1 slider "Poids du vehicule" qui ajuste mass + inertie + suspension ensemble (via contraintes genome)
- plutot que 5 sliders techniques independants que personne ne comprend
- Les contraintes genome (ratio source→target) servent exactement a ca : lier les parametres entre eux
- Regle : si 2+ parametres bougent toujours ensemble, les fusionner en 1 meta-parametre

## Ce qui ne doit PAS etre expose
- Constantes physiques universelles (gravite terrestre, IOR eau, PI)
- Parametres de rendu internes (shader uniforms, buffer sizes)
- Seuils de performance (cull distances, LOD distances, chunks/frame)
- Deadzones, epsilon, clamp values — ce sont des details d'implementation

## Test du "createur de 14 ans"
- Avant d'exposer un nouveau parametre, se demander : "Est-ce qu'un ado qui joue a Roblox comprendrait ce slider ?"
- Si non, soit le renommer, soit le cacher en Avance, soit le supprimer et utiliser une contrainte automatique

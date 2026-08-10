# Forgia Rewrite — contrat permanent Codex

Ce fichier s'applique à tout le workspace. Codex doit le lire avant toute
analyse ou modification du projet.

## Mémoire obligatoire

La mémoire historique de Forgia fait partie des sources du projet. Avant toute
modification non triviale (feature, refactor, bug, architecture, performance,
UI, gameplay ou données), Codex doit :

1. lire `docs/AI_MEMORY_MAP.md` ;
2. lire l'index actif `MEMORY.md` indiqué dans ce registre ;
3. rechercher dans tous les espaces mémoire enregistrés les termes du concept,
   du crate, du système et du symptôme concernés ;
4. ouvrir les fichiers `feedback_*`, `reference_*` et `session_*` pertinents ;
5. confronter les souvenirs au code, aux tests, aux genomes et aux capteurs
   actuels : une mémoire est un indice historique, jamais une preuve supérieure
   à l'état présent du dépôt ;
6. appliquer les enseignements encore valides et signaler toute contradiction.

Ne jamais charger aveuglément toutes les mémoires dans le contexte : utiliser
les index et la recherche ciblée afin de conserver l'intégralité accessible sans
saturer la fenêtre de contexte.

Les historiques `.jsonl` sont la source brute de dernier recours : les consulter
si les mémoires Markdown ne suffisent pas, si une décision est ambiguë ou si
l'utilisateur demande de retrouver précisément un ancien échange.

Après une découverte durable ou une correction importante, mettre à jour la
mémoire active selon `.claude/rules/session-checkpoint.md`. Ne jamais écrire dans
les archives V1 sauf demande explicite.

## Contrat projet

Lire également `CLAUDE.md` : sa vision, ses contraintes d'architecture, ses
stability locks et ses règles qualité s'appliquent aussi à Codex.

Priorité : livrer le Roguelite. Français pour les échanges et la documentation,
anglais pour le code. Vérifier avant de modifier et valider les changements en
proportion du risque.

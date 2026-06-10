---
id: ADR-0003
title: Pivot vision — moteur IA-natif, priorité absolue au ship du Roguelite
status: ACCEPTED
date: 2026-06-04 (décision) — formalisé 2026-06-10 (story-593 M1.7)
authors: [Antoine]
supersedes: [vision "YouTube of gaming / plateforme UGC publier-monétiser" (2026-05)]
related:
  - docs/vision/FORGIA_VISION_2026-06-04.md (texte de vision complet)
  - docs/audit/roguelite-ship-readiness-2026-06-04.md (état au pivot, ~40 % MVG)
  - docs/ROADMAP_POST_AUDIT_2026-06-10.md (exécution M0-M5)
---

# ADR-0003 — Pivot vision : moteur IA-natif + ship Roguelite

## Contexte

La vision initiale V2 (« YouTube of gaming », funnel Play/Build/Edit, publication et
monétisation de jeux créés) dispersait l'effort entre plateforme, éditeur et jeux.
Constat 2026-06-04 : la valeur réellement construite et différenciante n'est pas la
plateforme — c'est le **système de production** (codebase observable par sensors,
data-driven par genomes, règles process) qui permet à une IA de construire un jeu de
façon fiable.

## Décision

1. **Forgia = moteur de jeu IA-natif** : le créateur décrit son jeu et importe SES
   assets, l'IA le construit. Pas du no-code à graphes : de l'IA-code.
2. **Priorité absolue Phase 0 : SHIPPER le premier jeu** — le Roguelite (FPS roguelite
   type Gunfire Reborn). Tout arbitrage de scope = « ça débloque le ship ? ».
3. **Modèle deux-tracks** : track SHIP (Roguelite) ← track FORGE (RPG = banc d'outils
   anim/rig/terrain qui refluent). Travail RPG autorisé seulement s'il construit un
   outil réutilisable accélérant le ship.
4. La plateforme/distribution (ancien cœur de vision) devient une destination
   lointaine (Phase 3), pas une promesse.

## Alternatives rejetées

- **Continuer la plateforme UGC** : aucun moat face à Roblox/Astrocade sans des années
  d'avance distribution ; l'étude marché (audit 2026-06-10) confirme que la fenêtre
  est sur la preuve qualité, pas la plateforme.
- **Vendre le moteur sans jeu shippé** : un moteur IA-natif sans jeu de référence est
  une affirmation invérifiable — le Roguelite EST la preuve marketing.

## Conséquences

### Positives
- Test de scope unique et brutal pour chaque décision (« ça avance le ship ? »).
- Le marketing s'aligne sur les griefs créateurs 2026 (anti-gameslop : « tes assets,
  ta direction, un vrai jeu natif possédé »).

### Négatives / risques
- TAB reste réservé au gameplay in-game (loadout type Gunfire) — le cycle Build/Edit
  devient feature moteur différée Phase 2.
- Le track FORGE est une tentation de dispersion permanente (l'audit 2026-06-10
  mesure que la moitié des P1 de dette vivent côté FORGE) — d'où la règle 1 de la
  roadmap post-audit.
- Fenêtre marché 12-24 mois (Roblox 4D, Unity agentique, Astrocade) : le pivot ne
  vaut que si le ship arrive avant fin 2026 (démo publique).

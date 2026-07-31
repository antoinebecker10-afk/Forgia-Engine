# Forgia — ROADMAP (source de vérité unique)

> **Un seul fichier de pilotage.** Toutes les autres roadmaps sont archivées (§ Roadmaps archivées).
> Format **Now / Next / Later** (sans dates) — survit au churn, contrairement aux roadmaps datées qui pourrissent.
> Priorité absolue : **SHIP le Roguelite** (CLAUDE.md §1). Track FORGE (RPG / anim) seulement s'il accélère le ship.
> Dernière consolidation : **2026-07-03** (fusion de 5 roadmaps concurrentes).

## 🚦 Règles de pilotage (garde-fous)

- **Limite WIP** : max **3 stories `IN_PROGRESS`** à la fois. *Stop starting, start finishing.* (Aujourd'hui : ~60 chantiers ouverts → à résorber.)
- **Statuts normalisés, un seul vocabulaire** : `DRAFT → READY → IN_PROGRESS → REVIEW → DONE`. (Bannir `EN COURS` / `IN PROGRESS` / `CODE-COMPLETE` → mapper sur `IN_PROGRESS` / `REVIEW`.)
- **Détail par chantier** : `docs/stories/story-NNN-*.md`.
- **Plan stratégique complet (7 phases P0→P6)** : [`masterplan gunfire-like`](audit/forgia-gunfire-masterplan-2026-07-01.md) — le « comment », phase par phase.
- **DONE mécanique** : `cargo run -p xtask -- story-gate` (jamais de DONE auto-déclaré).

---

## 🔵 NOW — en vol (on finit ça avant d'ouvrir autre chose)

| Chantier | Statut | Détail |
| --- | --- | --- |
| **story-596 — Ultime par arme** | `IN_PROGRESS` (autre terminal) | Confirmé git (commits `cc61183`/`aca4318`/`46f96df`). ⚠️ **Ne pas éditer `forgia-mode-roguelite` sans coordination** (arbre chaud). |
| **Hygiène git de la branche** | `IN_PROGRESS` | ✅ **PUSH FAIT 2026-07-03** (26 commits — gates réglés au passage : allowlist asset-load `weapon_vfx`, 7 capteurs enregistrés au registre, collision d'ID 647→660). Reste : câbler `forgia-mode-roguelite/lib.rs` (Shock 653 + bursts 655, entremêlé arbre chaud) + committer stories 656/657/658. ⚠️ Prendre son ID via `xtask story-ids` AVANT de créer une story (2 collisions déjà eues). |
| **R2 — FSM `RoomPhase` + Inc.3 salles typées** | `READY` (⏸️ arbre chaud) | Multi-salles (Inc.1) + portail de choix (Inc.2) **LIVRÉS + validés runtime** (fixes : boucle infinie re-break, caméra modal, layout portes). Reste : refactor FSM (`Fighting/Break/PortalChoice` — tue la classe de bugs à flags) PUIS Inc.3 (Élite = compo ×gene, Trésor/Repos/Boutique sans combat). **Design complet consigné dans story-646** (consommateurs, piège sensor JSON parsé par observability, genes). Reprendre dès que `waves.rs` sort du diff de l'autre terminal. |
| **Valider le Bourg de l'Enclume (story-660)** | `REVIEW` | Salle 2 authored livrée (village diurne medieval_hexagon ×5-10, 13 pièces, AABB mesurés, règles level-art appliquées : weenie/70-30/température). À valider visuellement in-game — ajustements = data-only (`arena_layouts.toml`, re-rentrer dans la salle suffit). |
| **Ship P0 — binaire dist + vérif victoire** | `TODO` (bloqué arbre chaud) | Lancer `scripts/build-dist.ps1` **depuis un HEAD propre**, décompresser le zip ailleurs, vérifier lancement standalone (assets/cwd/capteurs). Vérifie **d'un coup** la victoire au runtime (câblée depuis story-571, **jamais testée**). Cf `ROADMAP_ROGUELITE.md` § ship-gap. |
| **Kill satisfaisant (mort en 4 temps)** | `READY` | Anticipation (hitstop + flash + scale punch) → burst → débris physiques → permanence (corps + décal élément). Ingrédients livrés (648/650/655) ; reste l'assemblage. **Dernier gros morceau du game-feel** — après, on arrête le polish visuel. |

---

## 🟢 NEXT — chemin de ship immédiat (dès que NOW est vidé)

| Chantier | Détail |
| --- | --- |
| **Voix / SFX des armes** | 90 barks écrits, **0 audio**. L'identité unique de Forgia (« les armes qui parlent »). + SFX punchy sur chaque action de combat (tir/impact/kill/pickup/level-up). Royalty-free (Ovani). Masterplan P5-1. |
| **Parcours PLATFORMER entre les salles** | ✅ RunGraph consommé + portails de choix = **FAITS** (story-646 Inc.1/2, cf NOW pour FSM+Inc.3). Reste l'identité « niveaux à parcours » (rapport §R2.3) : les segments platformer (kit underworld, déjà 40 % construit) deviennent les **couloirs entre salles** — traversal risk/reward au lieu du swap sur place. |
| **Icônes de statut sur nameplates (HUD Inc.3, story-644)** | burn 🔥/poison ☠/shock ⚡/miasma sur le nameplate ennemi — rend les réactions élémentaires lisibles. Zéro collision arbre chaud (forgia-enemy-nameplate). |
| **Fond noir du lobby** | Signalé par le user 2026-07-02 (screenshot hub « TON FORGERON »), jamais diagnostiqué — l'arène de fond ne rend pas au Lobby. À trier (peut-être lié au chantier hub de l'autre terminal). |
| **Réaction Manipulation (P0-4 Inc.4)** | Déférée : conflit de paire Feu+Élec (= Surcharge, décision de contenu à trancher) + charme = re-targeting IA dans `forgia-ai-arena-bot` (coordonner). Cf `reference_elemental_reaction_engine_and_shock`. |
| **Télégraphe ennemi + lisibilité** | Windup ~0,25 s par archétype (anim/son/VFX) + projectiles ennemis en palette rouge distincte + screenshake explosions. **Levier anti-frustration n°1.** Masterplan P1-3. |
| **FTUE — première run scriptée** | 1 mécanique par palier, prompts contextuels (étend `ftue.rs`, déjà MVP). Pas de niveau tuto séparé. Masterplan P5-2. |
| **Playtest externe #1** | 3-5 testeurs, **1 seule question**. Dès que la boucle est fun. Apprend plus que 10 stories de polish. Jalon masterplan. |
| **Page Steam en tâche de fond** | Capsule + trailer + GIFs, **6-12 mois avant launch**. Les wishlists s'accumulent lentement — chaque semaine sans page = wishlists perdues. |

---

## 🟠 LATER — profondeur, contenu, ship infra (v0.2+)

- **Armes swappables (2) + inscriptions échangeables** au menu — cœur du build Gunfire. Masterplan Phase 3.
- **Contenu « lite »** (scope **GELÉ**, pas la parité Gunfire) : 6-8 armes, ~40 boons, élites, 3 actes × ~4 salles, 2e arène ressentie. Masterplan Phase 4.
- **Audio dynamique complet** (combat/break/boss) + **accessibilité** (remapping contrôles, colorblind, toggle screenshake) + resume de run. Masterplan Phase 5.
- **Steam launch** : démo 15-30 min → CTA wishlist (sortie 2-4 sem avant Next Fest), Steamworks, packaging signé 60 fps GTX 1060. Masterplan Phase 6.
- **Coop / netcode** (lightyear) — **solo d'abord**, le multi double la complexité.
- **Dette technique** :
  - `rustup update` → **Rust 1.96.1** (dès maintenant, zéro risque écosystème).
  - Calibration HDR / bloom (checklist prête dans story-647) — débloque le glow émissif.
  - Split des hotspots : `element_vfx.rs`, `weapon_vfx/mod.rs`, `status_vfx.rs`.
  - LOD particules si les FPS chutent en combat dense ; 3 warnings clippy `live` (`status_vfx.rs`).
  - Surveiller **Bevy 0.19** (bloqué par `bevy_rapier3d`, sans ETA) — **ne rien bloquer dessus**.
  - **Perf arène (>60fps, pas urgent — jeu à la cible)** : profilée `cpu_bound` = **scène statique** (13k entités / 2254 meshes visibles), **PAS** les bots (1 ennemi) ni les VFX (0 particule) → seul levier = réduire la densité d'entités/meshes (merge géométrie statique). L'audit avait misé sur l'IA des bots : **invalidé par le profiling**. Détail + backlog Phase B (B1/B2/B3 non pertinents, chunk async, CollisionGroups = code mort) : `docs/stories/story-643-perf-hotpath-allocations-phase-a.md` + `docs/audit/audit-2026-07-01-perfs-jeu-vs-industrie.md`. Outillé : `forgia2_perf.json` expose `bound_hint`/`render_cpu_ratio` (toute régression perf = diagnostic 1 lecture). Pinpoint fin = capture Tracy en arène.
  - ~~Avant `git push` : enregistrer `forgia2_volume.json`~~ ✅ FAIT 2026-07-03 (7 capteurs enregistrés d'un coup, push passé). Reste le nettoyage lié : le `set_volume` de canal (forgia-audio) est redondant pour SFX/musique depuis le fix volume instance-level (bevy_kira_audio 0.25, cf commit `a8b8d42`).
  - **Trade-offs boons** (R3.4 déféré) : nécessite un schéma multi-effets (`effects: Vec<...>` dans BoonDef) — story dédiée. L'empilement multiplicatif + tirage pondéré par rareté sont FAITS (story-645).
  - ~~`WEAPON_MASTERY_DMG_PER_LEVEL` (weapon_select.rs) : const à externaliser en genome.~~ ✅ FAIT 2026-07-31 (story-668) — section `[mastery]` de `roguelite_meta_shop.toml` (`max_level` + `damage_per_level`, bornées, hot-reload). Le const n'existe plus. ⚠️ **Valeur livrée 6 × 4 % (+20 % au plafond)** alors que le GDD M5 et l'audit balance 2026-07-19 visaient **10 × 2 % (+18 %)** — divergence assumée pour ne pas changer le gain par niveau des saves existantes, **à trancher** en passe de balance.
  - Best-run affiché au Lobby/accueil (story-645 ne l'affiche qu'aux écrans de fin de run).
  - `gen_voices.py` (proto gibberish 4 personas, scratchpad session 2026-07-02) : à re-versionner dans `tools/` si la voie gibberish est retenue (recette aussi dans le rapport §R1).
- **Track FORGE** (anim vendeur, auto-rig, outils RPG) — seulement si ça reflue vers le ship.

---

## 🗄️ Roadmaps archivées (superseded par ce fichier)

Contenu conservé pour l'historique / le détail, mais **plus de source d'autorité** :

- `docs/ROADMAP_CURRENT.md` — historique des vagues V1→V7 (mai-juin 2026), état sensors.
- `docs/ROADMAP_ROGUELITE.md` — bible, benchmarks all-time, 3 gaps, backlog vendeur, **§ ship-gap détaillé** (référence de fond utile).
- `docs/ROADMAP_POST_AUDIT_2026-06-10.md` — priorisation post-audit du 10 juin.
- `docs/roadmap-rendering-pipeline-2026-05-19.md` — pipeline de rendu (mai).

*Plan détaillé phase par phase : [`audit/forgia-gunfire-masterplan-2026-07-01.md`](audit/forgia-gunfire-masterplan-2026-07-01.md). Ce fichier-ci = le « quoi maintenant » ; le masterplan = le « comment, dans l'ordre ».*

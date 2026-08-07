# Index des stories — Forgia

> ⚙️ **Fichier généré par `cargo run -p xtask -- story-index`. Ne pas éditer à la main.**
> Board unique, statuts normalisés. Pilotage : [`../ROADMAP.md`](../ROADMAP.md). Gate anti-drift : `story-index --check`.

## Résumé

| Statut | Nombre |
| --- | --- |
| IN_PROGRESS | 46 |
| REVIEW | 58 |
| BLOCKED | 5 |
| DRAFT | 34 |
| UNKNOWN | 10 |
| DONE | 45 |
| CANCELLED | 1 |
| **Total** | **199** |

> 🚨 **46 stories `IN_PROGRESS` > limite WIP 3.** *Stop starting, start finishing.*

## IN_PROGRESS (46)

| ID | Titre | Fichier |
| --- | --- | --- |
| 442 | Procgen Village V1 | [story-442-procgen-village-v1.md](./story-442-procgen-village-v1.md) |
| 449 | Asset Pack Pipeline | [story-449-asset-pack-pipeline.md](./story-449-asset-pack-pipeline.md) |
| 450 | Chunk Streaming Foundation | [story-450-chunk-streaming-foundation.md](./story-450-chunk-streaming-foundation.md) |
| 454 | Anim Debug System V2 (Niveau A — Diagnostic bone-trace) | [story-454-anim-debug-system-niveau-a-diagnostic.md](./story-454-anim-debug-system-niveau-a-diagnostic.md) |
| 456 | Hit Feedback AAA (Nameplate billboards + layered damage + headshot routing) | [story-456-hit-feedback-aaa.md](./story-456-hit-feedback-aaa.md) |
| 464 | Bot LOS State Gating (Arena AI tracking fix) | [story-464-bot-los-state-gating.md](./story-464-bot-los-state-gating.md) |
| 465 | Sensor Fusion Tier 1 (forgia2_combat + forgia2_arena) | [story-465-sensor-fusion-tier1.md](./story-465-sensor-fusion-tier1.md) |
| 482 | Animation System Redesign (Enterprise) | [story-482-animation-system-redesign-enterprise.md](./story-482-animation-system-redesign-enterprise.md) |
| 490 | Roguelite Damage Routing Bridge (forgia_combat Health + DeathEvent trigger) | [story-490-roguelite-damage-routing-bridge.md](./story-490-roguelite-damage-routing-bridge.md) |
| 502 | Foliage coverage sensor | [story-502-A-foliage-coverage-sensor.md](./story-502-A-foliage-coverage-sensor.md) |
| 516 | DELETE vague AI + VFX + Misc (20 crates) | [story-516-delete-ai-vfx-misc.md](./story-516-delete-ai-vfx-misc.md) |
| 517 | UI + Weapons cleanup (DELETE 14 scaffolds + FUSION 6 UI réelles) | [story-517-ui-weapons-fusion.md](./story-517-ui-weapons-fusion.md) |
| 532 | 💨 Bourrasque Moveset Distinctive (Mission 3 GDD) | [story-532-bourrasque-moveset.md](./story-532-bourrasque-moveset.md) |
| 533 | 🎩 Madame Lenoir Moveset Distinctive (Mission 3 GDD) | [story-533-lenoir-moveset.md](./story-533-lenoir-moveset.md) |
| 534 | 🪓 Boucherie Moveset Distinctive (Mission 3 GDD) | [story-534-boucherie-moveset.md](./story-534-boucherie-moveset.md) |
| 553 | Skybox HDR KTX2 (foundation re-add HDR pipeline) | [story-553-skybox-hdr-ktx2.md](./story-553-skybox-hdr-ktx2.md) |
| 554 | Skybox cartoon procedural (Phase 1 : Rust generated cubemap) | [story-554-skybox-cartoon-procedural.md](./story-554-skybox-cartoon-procedural.md) |
| 555 | Skybox cartoon palette TOML data-driven per-biome (Phase 2 de 554) | [story-555-skybox-palette-toml-data-driven.md](./story-555-skybox-palette-toml-data-driven.md) |
| 574 | RPG dramatic relief (max_height 28→80) | [story-574-rpg-dramatic-relief.md](./story-574-rpg-dramatic-relief.md) |
| 581 | Monitor perf/mémoire enrichi + VRAM (port V1) | [story-581-perf-monitor-vram-enrich.md](./story-581-perf-monitor-vram-enrich.md) |
| 582 | Système d'éléments par-arme (Roguelite, Phase A) | [story-582-weapon-elements.md](./story-582-weapon-elements.md) |
| 583 | Budget par frame du spawn de foliage (anti-stutter chargement) | [story-583-foliage-spawn-budget.md](./story-583-foliage-spawn-budget.md) |
| 585 | Choix de boon 1-parmi-3 au portail de fin de zone (agency Hadès) | [story-585-roguelite-portal-boon-choice.md](./story-585-roguelite-portal-boon-choice.md) |
| 588 | Compression textures KTX2/UASTC (VRAM ÷4) | [story-588-ktx2-texture-vram.md](./story-588-ktx2-texture-vram.md) |
| 588 | VFX colorés des éléments (rendre le système d'éléments visible) | [story-588-roguelite-element-vfx.md](./story-588-roguelite-element-vfx.md) |
| 589 | Phase B : progression d'élément (déblocage au portail) | [story-589-roguelite-element-progression.md](./story-589-roguelite-element-progression.md) |
| 590 | Obstacles animés du parcours (façon Fall Guys) | [story-590-roguelite-animated-obstacles.md](./story-590-roguelite-animated-obstacles.md) |
| 591 | L'Enclume des Âmes (méta-progression permanente) | [story-591-roguelite-meta-progression.md](./story-591-roguelite-meta-progression.md) |
| 592 | M0 « Filet » post-audit (P0 + crash + stutter + CI) | [story-592-m0-filet-post-audit.md](./story-592-m0-filet-post-audit.md) |
| 603 | Roguelite : porte du socle (boss-gated) → parcours | [story-603-roguelite-boss-gate-portal.md](./story-603-roguelite-boss-gate-portal.md) |
| 632 | FTUE Roguelite : hints contextuels + première mort + lisibilité de puissance (DPS) | [story-632-roguelite-ftue-hints-power-legibility.md](./story-632-roguelite-ftue-hints-power-legibility.md) |
| 634 | Keystone : simulation déterministe (FixedUpdate + RunRng) | [story-634-keystone-fixedupdate-determinism.md](./story-634-keystone-fixedupdate-determinism.md) |
| 636 | Animation squelettique + rig de contrôle sur les ennemis Roguelite | [story-636-roguelite-enemy-skeletal-anim.md](./story-636-roguelite-enemy-skeletal-anim.md) |
| 637 | Anim procédurale : multi-perso + auto-pose + fallback topologie | [story-637-procedural-anim-multichar-autopose.md](./story-637-procedural-anim-multichar-autopose.md) |
| 638 | P0-1 : stats ennemis data-driven (genome + sensor) | [story-638-p0-1-enemy-stats-genome.md](./story-638-p0-1-enemy-stats-genome.md) |
| 642 | P0-4 : affinité élément ↔ couche de défense (matchup → DefenseLayer) | [story-642-p0-4-elemental-defense-affinity.md](./story-642-p0-4-elemental-defense-affinity.md) |
| 643 | Perf pass : allocations hot-path (Phase A) + backlog Phase B runtime-gated | [story-643-perf-hotpath-allocations-phase-a.md](./story-643-perf-hotpath-allocations-phase-a.md) |
| 644 | P1 : HUD défensif segmenté (Vie / Bouclier / Armure) | [story-644-p1-hud-defense-bar-segmented.md](./story-644-p1-hud-defense-bar-segmented.md) |
| 646 | R2 : consommer le stage-graph (multi-salles clear-to-progress) | [story-646-r2-consume-stage-graph.md](./story-646-r2-consume-stage-graph.md) |
| 647 | VFX authored Inc.1 : Bloom + textures sur les particules | [story-647-vfx-bloom-textures-particules.md](./story-647-vfx-bloom-textures-particules.md) |
| 656 | Hitbox tête ennemis : suivie de l'os + capsules recalibrées sur les meshs | [story-656-enemy-head-hitbox-bone-tracked.md](./story-656-enemy-head-hitbox-bone-tracked.md) |
| 657 | La Trempe : progression de l'arme en cours de run (Inc.1) | [story-657-trempe-weapon-in-run-progression.md](./story-657-trempe-weapon-in-run-progression.md) |
| 658 | Scaling ennemi par profondeur de salle (la pression) | [story-658-enemy-scaling-by-depth.md](./story-658-enemy-scaling-by-depth.md) |
| 661 | Story 661 — Bras viewmodel GLB cartoon (remplace les poings procéduraux) | [story-661-bras-viewmodel-glb-cartoon.md](./story-661-bras-viewmodel-glb-cartoon.md) |
| 666 | Fenêtre unique du Forgeron : achat souris + Trempe + dialogue E | [story-666-forge-shop-unified-window.md](./story-666-forge-shop-unified-window.md) |
| 678 | Hub Premium : le menu devient un vrai hub de roguelite | [story-678-hub-premium.md](./story-678-hub-premium.md) |

## REVIEW (58)

| ID | Titre | Fichier |
| --- | --- | --- |
| 0 | Story — Menu-titre devient le hub roguelite complet | [story-menu-hub-roguelite.md](./story-menu-hub-roguelite.md) |
| 455 | FPS UI Juice AAA (Ammo HUD + Kill Feed + Damage Direction + Cleanup + Pause) | [story-455-fps-ui-juice-aaa.md](./story-455-fps-ui-juice-aaa.md) |
| 467 | V5 Session B : sensors perf + entities + memory | [story-467-v5-session-b-perf-entities-memory.md](./story-467-v5-session-b-perf-entities-memory.md) |
| 469 | V5 Session C : sensors lifecycle + watchdog + audio + input + sensor_health | [story-469-v5-session-c-lifecycle-watchdog-audio-input.md](./story-469-v5-session-c-lifecycle-watchdog-audio-input.md) |
| 470 | V7 M1 Roguelite Fondations (scaffold MVP) | [story-470-v7-m1-roguelite-fondations.md](./story-470-v7-m1-roguelite-fondations.md) |
| 471 | `forgia-analytics` Sentry crash dump (P0 V7) | [story-471-forgia-analytics-sentry-crash-dump.md](./story-471-forgia-analytics-sentry-crash-dump.md) |
| 472 | `forgia-audio-voicelines` Tier 1 selection logic (P0 V7) | [story-472-forgia-audio-voicelines-tier1.md](./story-472-forgia-audio-voicelines-tier1.md) |
| 473 | `forgia-stage-graph` NEW crate (P0 V7) | [story-473-forgia-stage-graph.md](./story-473-forgia-stage-graph.md) |
| 474 | `forgia-loot-tables` (P0 V7) | [story-474-forgia-loot-tables.md](./story-474-forgia-loot-tables.md) |
| 475 | `forgia-equipment` (P0 V7) | [story-475-forgia-equipment.md](./story-475-forgia-equipment.md) |
| 477 | `forgia-audio-music-state` Tier 1 (P0 V7) | [story-477-forgia-audio-music-state.md](./story-477-forgia-audio-music-state.md) |
| 478 | `forgia-audio-ducking` Tier 1 (P0 V7) | [story-478-forgia-audio-ducking.md](./story-478-forgia-audio-ducking.md) |
| 479 | `forgia-scene` saves system (P0 V7) | [story-479-forgia-scene-saves.md](./story-479-forgia-scene-saves.md) |
| 481 | `forgia-audio-voicelines` Tier 1.5 wire-up (P0 V7 M4) | [story-481-audio-voicelines-tier1.5-wireup.md](./story-481-audio-voicelines-tier1.5-wireup.md) |
| 482 | `forgia-audio-voicelines` Tier 1.6 floating bark text overlay (P0 V7 M4) | [story-482-audio-voicelines-tier1.6-bark-text-overlay.md](./story-482-audio-voicelines-tier1.6-bark-text-overlay.md) |
| 514 | INVALIDATED (forgia-core split god-object) | [story-514-cancelled.md](./story-514-cancelled.md) |
| 531 | 🔫 Pépin Moveset Distinctive (Mission 3 GDD) | [story-531-pepin-moveset.md](./story-531-pepin-moveset.md) |
| 539 | Village hexagonal KayKit dans le RPG | [story-539-rpg-hex-village-kaykit.md](./story-539-rpg-hex-village-kaykit.md) |
| 546 | Sensor Registry + Audit gate | [story-546-sensor-registry-audit.md](./story-546-sensor-registry-audit.md) |
| 547 | forgia-debug crate (3-layer architecture) | [story-547-forgia-debug-crate.md](./story-547-forgia-debug-crate.md) |
| 548 | Console runtime debug (forgia-debug::console) | [story-548-console-runtime.md](./story-548-console-runtime.md) |
| 549 | Physics sensor (Rapier blind spot) | [story-549-physics-sensor-blind-spot.md](./story-549-physics-sensor-blind-spot.md) |
| 584 | Pose idle « bras le long » des PNJ RPG | [story-584-npc-idle-pose.md](./story-584-npc-idle-pose.md) |
| 586 | RPG : débrancher le village legacy (dédup), garder le hex | [story-586-rpg-village-dedup.md](./story-586-rpg-village-dedup.md) |
| 587 | Collider sur les mega-tiles LOD2 (anti chute-à-travers le sol) | [story-587-lod2-collider.md](./story-587-lod2-collider.md) |
| 594 | M2 session 1 : feel data-driven, anti-hitch, tests combat, KTX2 armes | [story-594-m2-session1-feel-tech.md](./story-594-m2-session1-feel-tech.md) |
| 610 | Roguelite : Commerçant d'arène (sink in-run Or + Âmes) | [story-610-roguelite-arena-merchant.md](./story-610-roguelite-arena-merchant.md) |
| 611 | Roguelite : Réaction élémentaire Combustion (Feu + Poison) | [story-611-elemental-reaction-combustion.md](./story-611-elemental-reaction-combustion.md) |
| 612 | Roguelite : Wizard de choix d'arme de départ (Phase 0) | [story-612-roguelite-weapon-select-wizard.md](./story-612-roguelite-weapon-select-wizard.md) |
| 613 | Roguelite : déblocage permanent des armes (progression « évolutive ») | [story-613-roguelite-weapon-unlocks-evolutif.md](./story-613-roguelite-weapon-unlocks-evolutif.md) |
| 614 | Wizard : aperçu d'arme 3D tournant + UI plein écran centrée | [story-614-wizard-3d-preview-fullscreen.md](./story-614-wizard-3d-preview-fullscreen.md) |
| 616 | Roguelite : déblocage des paliers d'atouts (boons) « évolutif » | [story-616-roguelite-boon-tier-unlocks.md](./story-616-roguelite-boon-tier-unlocks.md) |
| 617 | Roguelite : nettoyage de l'écran de sélection (Lobby) « quick wins » | [story-617-roguelite-lobby-ui-cleanup.md](./story-617-roguelite-lobby-ui-cleanup.md) |
| 618 | FOV viewmodel séparé (2e caméra + RenderLayers) + placement bras réglable | [story-618-viewmodel-separate-fov-camera.md](./story-618-viewmodel-separate-fov-camera.md) |
| 624 | Hub d'accueil Roguelite à onglets (P2) | [story-624-roguelite-home-hub-tabs.md](./story-624-roguelite-home-hub-tabs.md) |
| 626 | Roguelite : étalement du spawn décor (fix freeze 65 ms entrée de stage) | [story-626-roguelite-decor-spawn-stagger.md](./story-626-roguelite-decor-spawn-stagger.md) |
| 627 | Roguelite : pré-chauffe shaders (réduction des freezes first-use) | [story-627-roguelite-shader-prewarm.md](./story-627-roguelite-shader-prewarm.md) |
| 628 | Placement des mains AUTO par-arme + géométrie bras améliorée | [story-628-viewmodel-arms-per-weapon-placement.md](./story-628-viewmodel-arms-per-weapon-placement.md) |
| 629 | Roguelite : capteur de charge combat `perf_diag` (vision complète freezes) | [story-629-roguelite-perf-diag-sensor.md](./story-629-roguelite-perf-diag-sensor.md) |
| 631 | Présence viewmodel : sway/bob + bras cartoon procéduraux | [story-631-viewmodel-presence-sway-bob-procedural-arms.md](./story-631-viewmodel-presence-sway-bob-procedural-arms.md) |
| 648 | Paliers de hitstop : hit < crit < kill < multikill | [story-648-hitstop-paliers-kill-feel.md](./story-648-hitstop-paliers-kill-feel.md) |
| 649 | Aim assist cohérent : falloffs gradués façon CoD BO6 | [story-649-aim-assist-falloffs-bo6.md](./story-649-aim-assist-falloffs-bo6.md) |
| 650 | Knockback par hit : les ennemis encaissent physiquement | [story-650-knockback-par-hit.md](./story-650-knockback-par-hit.md) |
| 651 | Chime weakspot + variation de pitch (l'oreille distingue hit/tête/kill) | [story-651-chime-weakspot-pitch-variation.md](./story-651-chime-weakspot-pitch-variation.md) |
| 652 | VFX visibles : multiplicateurs genome + burst de kill | [story-652-vfx-visibles-kill-burst.md](./story-652-vfx-visibles-kill-burst.md) |
| 653 | Aura électrique : l'ennemi choqué grésille (identité Pépin) | [story-653-aura-shock-arcs-electriques.md](./story-653-aura-shock-arcs-electriques.md) |
| 655 | Fin des sphères procédurales : bursts hanabi texturés par élément | [story-655-fin-des-spheres-bursts-elementaires.md](./story-655-fin-des-spheres-bursts-elementaires.md) |
| 659 | Cohérence élémentaire des projectiles/tracers + retrait du « POW! » | [story-659-projectiles-elementaires-pow-retire.md](./story-659-projectiles-elementaires-pow-retire.md) |
| 660 | « Le Bourg de l'Enclume » : DA authored pour forge_sanctum (salle 2) | [story-660-bourg-enclume-forge-sanctum-authored.md](./story-660-bourg-enclume-forge-sanctum-authored.md) |
| 662 | Correctifs audit 360° : Vague 1, lot 1 (chaîne, capteur, robustesse, diamant) | [story-662-correctifs-audit-360-vague1-lot1.md](./story-662-correctifs-audit-360-vague1-lot1.md) |
| 663 | Perf : fusion de la géométrie statique d'arène par cellule × matériau (sol + murs) | [story-663-floor-merge-cellules-perf.md](./story-663-floor-merge-cellules-perf.md) |
| 665 | Éditeur in-game du Hall (lot 1 : props) | [story-665-editeur-hall-props.md](./story-665-editeur-hall-props.md) |
| 667 | Arena Test : le banc de blockout d'arène | [story-667-arena-test-blockout-bench.md](./story-667-arena-test-blockout-bench.md) |
| 668 | Vague 0 : remettre les invariants de la boucle roguelite debout | [story-668-vague0-invariants-boucle-roguelite.md](./story-668-vague0-invariants-boucle-roguelite.md) |
| 669 | La composition de vague redevient une dérivation | [story-669-composition-de-vague-derivee.md](./story-669-composition-de-vague-derivee.md) |
| 674 | L'aménagement se DÉRIVE : bruit bleu + compte depuis l'aire | [story-674-amenagement-derive-bruit-bleu.md](./story-674-amenagement-derive-bruit-bleu.md) |
| 675 | Personnage Trooper + équipement loot par rareté | [story-675-equipement-trooper-rarete.md](./story-675-equipement-trooper-rarete.md) |
| 677 | La boucle de rounds et son mur | [story-677-boucle-de-rounds-et-mur.md](./story-677-boucle-de-rounds-et-mur.md) |

## BLOCKED (5)

| ID | Titre | Fichier |
| --- | --- | --- |
| 468 | `forgia-mode-roguelite` MVP (3e jeu Forgia) | [story-468-mode-roguelite-mvp.md](./story-468-mode-roguelite-mvp.md) |
| 481 | Skeleton Template Declarative Bone Class (suite story-480) | [story-481-skeleton-template-declarative-class.md](./story-481-skeleton-template-declarative-class.md) |
| 597 | Roguefight UI Modernization, Phases C & D (suite de story-596) | [story-597-roguefight-ui-phase-c-d.md](./story-597-roguefight-ui-phase-c-d.md) |
| 623 | Parcours joueur Roguelite : Identité + Progression + Onboarding | [story-623-roguelite-player-journey-identity-progression-onboarding.md](./story-623-roguelite-player-journey-identity-progression-onboarding.md) |
| 679 | Manette au menu : valider ce qui a été livré sans manette | [story-679-manette-menu-validation.md](./story-679-manette-menu-validation.md) |

## DRAFT (34)

| ID | Titre | Fichier |
| --- | --- | --- |
| 491 | Workspace Re-compile : API bridge voicelines/loot/music/waves | [story-491-workspace-recompile-api-bridge.md](./story-491-workspace-recompile-api-bridge.md) |
| 492 | i18n Fluent + .ftl bilingue EN/FR (barks roguelite) | [story-492-i18n-fluent-ftl-bilingue.md](./story-492-i18n-fluent-ftl-bilingue.md) |
| 493 | DamageEvent multi-observer ordering fix (BufferedEvent + .chain()) | [story-493-damage-event-buffered-chain.md](./story-493-damage-event-buffered-chain.md) |
| 494 | Genome registry validator cross-crate | [story-494-genome-registry-validator.md](./story-494-genome-registry-validator.md) |
| 495 | Process gate anti-fictive-DONE | [story-495-process-gate-anti-fictive-done.md](./story-495-process-gate-anti-fictive-done.md) |
| 528 | FPS Feel Accessible (Mission 1 GDD) | [story-528-fps-feel-accessible.md](./story-528-fps-feel-accessible.md) |
| 529 | Boons Architecture + Coffre UI + 5 boons neutres (Mission 2 GDD) | [story-529-boons-architecture-coffre.md](./story-529-boons-architecture-coffre.md) |
| 530 | 24 Boons catalogue + 3 Anti-boons (Mission 2 GDD) | [story-530-boons-catalogue-anti-boons.md](./story-530-boons-catalogue-anti-boons.md) |
| 535 | 6 Ennemis V1 FSM + Contre-strats (Mission 4.3 GDD) | [story-535-enemies-v1-fsm.md](./story-535-enemies-v1-fsm.md) |
| 536 | Mid-boss + Boss Final (Mission 4.4 GDD) | [story-536-bosses-mid-final.md](./story-536-bosses-mid-final.md) |
| 537 | Méta-progression Hub Évolutif + Éclats persist (Mission 4.5 GDD) | [story-537-meta-progression-hub.md](./story-537-meta-progression-hub.md) |
| 538 | Polish VFX biome + Popup BD voicelines (Mission 1.3 + lore GDD) | [story-538-polish-vfx-popup-bd.md](./story-538-polish-vfx-popup-bd.md) |
| 539 | Multi-Mode Plugin Gating (RPG-only plugins tournent en Roguelite) | [story-539-multi-mode-plugin-gating.md](./story-539-multi-mode-plugin-gating.md) |
| 540 | Player stuck KCC contre modules intérieurs (Roguelite Crypts of Anvil) | [story-540-roguelite-player-stuck-kcc.md](./story-540-roguelite-player-stuck-kcc.md) |
| 541 | Roguelite mode integration broken (player invulnerable + bot AI no-LOS + Souls bridge + HP UI suspect) | [story-541-roguelite-mode-integration-broken.md](./story-541-roguelite-mode-integration-broken.md) |
| 542 | Plugin double-add guard (panic-risk P0) | [story-542-plugin-double-add-guard.md](./story-542-plugin-double-add-guard.md) |
| 543 | Re-câbler sub-plugins forgia-rpg-data | [story-543-rpg-data-subplugins-wire.md](./story-543-rpg-data-subplugins-wire.md) |
| 544 | Cleanup 3 crates orphelines (weapon-hitscan + postprocess + spline) | [story-544-orphan-crates-cleanup.md](./story-544-orphan-crates-cleanup.md) |
| 545 | Bot raycast self-hit : player invincible Roguelite | [story-545-bot-raycast-self-hit-player-invincible.md](./story-545-bot-raycast-self-hit-player-invincible.md) |
| 556 | Bug "joueur bloqué" investigation + fix KCC | [story-556-stuck-bug-investigation.md](./story-556-stuck-bug-investigation.md) |
| 558 | Souls → Coffre du Forgeron Economy + Break 15s | [story-558-souls-coffre-economy.md](./story-558-souls-coffre-economy.md) |
| 560 | Mouvement : Sprint + Crouch + Slide (game feel BO6 accessible) | [story-560-movement-sprint-crouch-slide.md](./story-560-movement-sprint-crouch-slide.md) |
| 562 | Structures praticables (intérieurs plain-pied) | [story-562-enterable-structures.md](./story-562-enterable-structures.md) |
| 563 | Verticalité : étages, plateformes, multi-niveaux | [story-563-verticality-elevation.md](./story-563-verticality-elevation.md) |
| 564 | Câbler les 4 gimmicks d'armes (livrer l'USP) | [story-564-weapon-gimmicks-wired.md](./story-564-weapon-gimmicks-wired.md) |
| 565 | Rendre les boons perceptibles (sortir de l'Excel) | [story-565-boons-perceptible.md](./story-565-boons-perceptible.md) |
| 566 | Recalibrage économie (débloquer les 18 boons) | [story-566-economy-recalibration.md](./story-566-economy-recalibration.md) |
| 567 | Variété de vagues + courbe d'intensité | [story-567-wave-variety-intensity.md](./story-567-wave-variety-intensity.md) |
| 568 | Poursuite de synergie de tags visible (objectif intra-run) | [story-568-tag-synergy-visible.md](./story-568-tag-synergy-visible.md) |
| 569 | Méta-progression hub (la boucle de retour) | [story-569-meta-progression-hub.md](./story-569-meta-progression-hub.md) |
| 572 | Sort F « Onde de choc » (AOE) | [story-572-shockwave-ability-f.md](./story-572-shockwave-ability-f.md) |
| 573 | Sorts F par arme parlante (identité) | [story-573-per-weapon-spells.md](./story-573-per-weapon-spells.md) |
| 625 | Arène : coquille authored data-driven (Tier 1, modèle Returnal) | [story-625-arena-authored-shell.md](./story-625-arena-authored-shell.md) |
| 633 | Dette : migrer les asset loads ad-hoc vers GameAssets (Lock L1) | [story-633-asset-load-preload-migration.md](./story-633-asset-load-preload-migration.md) |

## UNKNOWN (10)

| ID | Titre | Fichier |
| --- | --- | --- |
| 476 | `forgia-status-effects` (P0 V7) | [story-476-forgia-status-effects.md](./story-476-forgia-status-effects.md) |
| 599 | Réglages graphiques Tier 1 (VSync / MSAA / Tonemapping) dans le menu ESC | [story-599-graphics-settings-tier1.md](./story-599-graphics-settings-tier1.md) |
| 601 | Support A-pose : auto-rig + anim de personnages non-Rex (arms-down) | [story-601-apose-autorig-animate-non-rex.md](./story-601-apose-autorig-animate-non-rex.md) |
| 604 | Watchdog freeze externe (observabilité robustesse ship) | [story-604-watchdog-freeze-externe.md](./story-604-watchdog-freeze-externe.md) |
| 605 | Prédicteur de visibilité HUD (« caché par design » vs « cassé ») | [story-605-hud-visibility-predictor.md](./story-605-hud-visibility-predictor.md) |
| 606 | Détecteur de fuite mémoire (timeline + alerte croissance soutenue) | [story-606-memory-leak-detector.md](./story-606-memory-leak-detector.md) |
| 607 | Timeline d'événements gameplay (post-mortem de run) | [story-607-gameplay-events-timeline.md](./story-607-gameplay-events-timeline.md) |
| 608 | Console `:set` live-tuning (canal balance IA single-param) | [story-608-console-set-live-tuning.md](./story-608-console-set-live-tuning.md) |
| 609 | `cargo xtask gene-search` (introspection genome cross-pack) | [story-609-xtask-gene-search.md](./story-609-xtask-gene-search.md) |
| 654 | Nameplate v2 : vie seule, plaques de bouclier/armure, icônes de statut | [story-654-nameplate-v2-plaques-icones.md](./story-654-nameplate-v2-plaques-icones.md) |

## DONE (45)

| ID | Titre | Fichier |
| --- | --- | --- |
| 441 | Spawn Village V1 | [story-441-spawn-village-v1.md](./story-441-spawn-village-v1.md) |
| 447 | Village V2 W2 : Terrain Leveling & Debug Gizmos (Niveau A) | [story-447-village-terrain-leveling-debug.md](./story-447-village-terrain-leveling-debug.md) |
| 448 | V2 Arena Map Precise Colliders | [story-448-arena-precise-colliders.md](./story-448-arena-precise-colliders.md) |
| 449 | V2 Arena Bot Hitbox Auto-Calibrate | [story-449-arena-bot-hitbox-auto-calibrate.md](./story-449-arena-bot-hitbox-auto-calibrate.md) |
| 450 | Audit Manhattan Diamond Bug + Memory | [story-450-wave5-phase3-audit.md](./story-450-wave5-phase3-audit.md) |
| 452 | RPG Health Monitor | [story-452-rpg-health-monitor.md](./story-452-rpg-health-monitor.md) |
| 453 | V2 Arena Combat Baseline Reset | [story-453-arena-combat-baseline-reset.md](./story-453-arena-combat-baseline-reset.md) |
| 453 | RPG Monitor Debt Closure | [story-453-rpg-monitor-debt.md](./story-453-rpg-monitor-debt.md) |
| 458 | Concept mapping `locomotion-bone-cache` | [story-458-locomotion-bone-cache-concept-mapping.md](./story-458-locomotion-bone-cache-concept-mapping.md) |
| 466 | DeathEvent → Observer (Bevy 0.18 EntityEvent migration) | [story-466-death-event-observer-migration.md](./story-466-death-event-observer-migration.md) |
| 480 | Skeleton Template Single Source of Truth (AAA conformance) | [story-480-skeleton-template-single-source.md](./story-480-skeleton-template-single-source.md) |
| 483 | Roguelite Stage Arena Foundations (RoR2-like topology, scaling-ready) | [story-483-roguelite-stage-arena-foundations.md](./story-483-roguelite-stage-arena-foundations.md) |
| 485 | Arena Spatial Identity (Roguelite Cover & Lanes Foundations) | [story-485-arena-spatial-identity.md](./story-485-arena-spatial-identity.md) |
| 486 | Jolcham Oak Bark Wire-up (material_override trunk) | [story-486-jolcham-oak-bark-wireup.md](./story-486-jolcham-oak-bark-wireup.md) |
| 496 | Anim pipeline per-character + bone axis validation | [story-496-anim-pipeline-per-character.md](./story-496-anim-pipeline-per-character.md) |
| 512 | Workspace Purge Vagues 1 & 4 (cleanup stubs + modes inutilisés) | [story-512-workspace-purge-vague-1-4.md](./story-512-workspace-purge-vague-1-4.md) |
| 513 | Fusion 45 `forgia-pp-*` → 1 `forgia-postprocess` | [story-513-pp-fusion-postprocess.md](./story-513-pp-fusion-postprocess.md) |
| 557 | Audit + plan de restoration assets V1 legacy paths | [story-557-assets-v1-audit.md](./story-557-assets-v1-audit.md) |
| 559 | Audio + Impact de Tir (le moment « WHAM ») | [story-559-audio-impact-feel.md](./story-559-audio-impact-feel.md) |
| 561 | Points d'Intérêt (POI) : peupler les anchors de gameplay | [story-561-poi-points-of-interest.md](./story-561-poi-points-of-interest.md) |
| 570 | RPG Data Loop Wiring (dialogue → inventaire/quête → récompense → XP) | [story-570-rpg-data-loop-wiring.md](./story-570-rpg-data-loop-wiring.md) |
| 571 | RPG WoW-like Interaction UI (cartoon Forge) | [story-571-rpg-wow-interaction-ui.md](./story-571-rpg-wow-interaction-ui.md) |
| 571 | Split monnaie : Or (in-run) + Souls (méta persistant) | [story-571-split-currency-or-souls.md](./story-571-split-currency-or-souls.md) |
| 575 | Biome definition layer (config/biomes/*.toml) | [story-575-biomes-definition-layer.md](./story-575-biomes-definition-layer.md) |
| 576 | Terrain shape genome (data-driven, hot-reloadable) | [story-576-terrain-shape-genome.md](./story-576-terrain-shape-genome.md) |
| 577 | Trou de couverture LOD2 (gap annulaire) + sonde de couverture | [story-577-lod2-coverage-gap.md](./story-577-lod2-coverage-gap.md) |
| 578 | `forgia-worldgen` : moteur de génération procédurale (villes / villages / maps) | [story-578-worldgen-procgen.md](./story-578-worldgen-procgen.md) |
| 579 | Animation tunables genome (mouvement data-driven, multi-perso) | [story-579-animation-tunables-genome.md](./story-579-animation-tunables-genome.md) |
| 580 | Collider de tronc foliage découplé du scale (anti sky-high / sous-la-map) | [story-580-foliage-trunk-collider-scale.md](./story-580-foliage-trunk-collider-scale.md) |
| 589 | Fix régression végétation RPG invisible (clear lit GlobalTransform non-propagé) | [story-589-vegetation-globaltransform-clear.md](./story-589-vegetation-globaltransform-clear.md) |
| 593 | M1 « Moat honnête » (docs vraies, sensors véridiques, gates actifs) | [story-593-m1-moat-honnete.md](./story-593-m1-moat-honnete.md) |
| 595 | M2 session 2 : Options plancher Steam (volume, affichage, touches, %APPDATA%) | [story-595-m2-settings-plancher-steam.md](./story-595-m2-settings-plancher-steam.md) |
| 596 | Roguefight UI Modernization (roadmap A→D) | [story-596-roguefight-ui-modernization.md](./story-596-roguefight-ui-modernization.md) |
| 598 | Cyber City perf demo (3P Rex walkable) | [story-598-cyber-city-perf-demo.md](./story-598-cyber-city-perf-demo.md) |
| 615 | Aim assist « bullet magnetism » (souris) + FOV joueur réellement appliqué | [story-615-aim-assist-bullet-magnetism-fov-fix.md](./story-615-aim-assist-bullet-magnetism-fov-fix.md) |
| 619 | `xtask validate-genomes` (gate QA couche 1) | [story-619-xtask-validate-genomes.md](./story-619-xtask-validate-genomes.md) |
| 620 | Hygiène sensors : registre complet + gates verts en CI (Phase 0.6) | [story-620-sensor-registry-hygiene.md](./story-620-sensor-registry-hygiene.md) |
| 621 | forgia2_health.json actif en Roguelite (RGL-1/RGL-2) | [story-621-roguelite-health-checks.md](./story-621-roguelite-health-checks.md) |
| 622 | Réveil du bus QA : pont santé → BugReport + sensor forgia2_qa | [story-622-qa-bus-producers.md](./story-622-qa-bus-producers.md) |
| 639 | État Ultime (F) + techniques signature par arme + VFX | [story-639-ultimate-techniques-vfx.md](./story-639-ultimate-techniques-vfx.md) |
| 640 | P0-2 : défense tri-couche Vie / Bouclier / Armure | [story-640-p0-2-defense-layer-shield-armor.md](./story-640-p0-2-defense-layer-shield-armor.md) |
| 641 | P0-3 : moteur de réactions générique + Element::Shock (Électrique) | [story-641-p0-3-reactions-engine-shock-element.md](./story-641-p0-3-reactions-engine-shock-element.md) |
| 645 | R3 : sceller la boucle (Victory + maîtrise + best-run + boons pondérés) | [story-645-r3-seal-the-loop.md](./story-645-r3-seal-the-loop.md) |
| 664 | Warmup des pipelines PBR au Lobby (anti-freeze « tourner la caméra ») | [story-664-pipeline-warmup-pbr-lobby.md](./story-664-pipeline-warmup-pbr-lobby.md) |
| 690 | Le capteur d'arène mesure enfin la géométrie posée | [story-690-capteur-de-geometrie-d-arene.md](./story-690-capteur-de-geometrie-d-arene.md) |

## CANCELLED (1)

| ID | Titre | Fichier |
| --- | --- | --- |
| 630 | Anti-aliasing sur la caméra orbitale 3P (RPG + CyberCity) | [story-630-taa-orbit-camera.md](./story-630-taa-orbit-camera.md) |
| 691 | Hub menu : quick-wins perf (P1 audit 2026-08-07) — DONE | [story-691-hub-perf-quickwins.md](./story-691-hub-perf-quickwins.md) |
| 692 | Hub menu : responsive — échelle globale, borderless, backdrop aspect réel — DONE | [story-692-hub-responsive.md](./story-692-hub-responsive.md) |


# Ship-Readiness Audit — Forgia Roguelite

> **2026-06-04** — Auteur : sub-agent `game-maker` (client interne P3).
> Méthode : code `file:line` + sensors runtime vs Definition of MVG.
> Contexte : pivot vision 2026-06-04 → priorité Phase 0 = **ship le Roguelite** (FPS roguelite type Gunfire Reborn).
> Source vision : `docs/vision/FORGIA_VISION_2026-06-04.md`.

---

## 1. Clarification de scope — Roguelite vs FPS-Arena (tranché par le code)

| Mode | GameMode enum | Au menu ? | Verdict |
|---|---|---|---|
| **Roguelite** | `GameMode::Roguelite` (forgia-core/lib.rs:55) | ✅ bouton "🎲 Roguelite Run" (forgia-ui/lib.rs:165) | **LE SHIP** |
| FPS Arena (ex "Bots Brawl") | `GameMode::Fps` (forgia-core/lib.rs:52) | ❌ retiré 2026-06-04 (forgia-ui/lib.rs:143-151) | Infra partagée, pas un produit |
| RPG OpenWorld | `GameMode::Rpg` | ✅ bouton | Track FORGE (banc d'essai outils), pas le ship |

Nuance technique : `forgia-mode-roguelite` **dépend** de `forgia-mode-fps-arena` (`waves.rs:28` importe `TargetCube`, ennemi = `forgia-ai-arena-bot::ArenaBot`). Le crate fps-arena n'est pas supprimable sans refactor, mais comme **produit jouable** il n'existe plus. Un seul jeu ship = le Roguelite.

---

## 2. Definition of Done du MVG (dure, falsifiable)

Score : **5/16 ✅ — 7/16 🟡 — 4/16 ❌ → ~40% de complétude ship** (cohérent avec l'auto-éval roadmap 35%).

| # | Critère | Cible minimale | État |
|---|---|---|---|
| D1 | Boucle de run complète | start → ≥3 stages → boss → fin | 🟡 3 **vagues** dans **1 stage**, puis Victory |
| D2 | Durée de run | 8-15 min | 🟡 ~12 min mais sans variété |
| D3 | Armes distinctes | ≥3 armes gimmick perceptible en 30s | ❌ 4 personas TOML, 0 gimmick câblé |
| D4 | Variété d'ennemis | ≥4 archétypes lisiblement différents | 🟡 4 archétypes, 3 = mêmes stats |
| D5 | Boons/ascensions | ≥10 achetables ET perceptibles | 🟡 18 catalogués, 0 perceptible, ~2 achetables/run |
| D6 | Condition de victoire | boss → écran Victoire | ✅ boss enrage + VICTOIRE |
| D7 | Condition de défaite | mort → Défaite → relance | ✅ run.rs:221 |
| D8 | Méta-progression | ≥1 axe persistant entre runs | ❌ Souls sans sink |
| D9 | Persistance disque | progrès survit au quit | ❌ aucun save/load |
| D10 | Audio | SFX tir/impact/kill + musique | 🟡 slice A (placeholders), pas de SFX de tir |
| D11 | Game feel de tir | muzzle + hit-flash + tracer + hit-stop | ❌ aucun |
| D12 | Menu + flow | menu → jeu → fin → relance | ✅ |
| D13 | HUD lisible | wave/monnaies/armes/minimap/sort/HP | ✅ hud.rs complet |
| D14 | Onboarding | compréhension <30s sans wiki | ❌ aucun |
| D15 | Stages variés | ≥2 décors distincts ressentis | 🟡 2 stage_id, 1 visité/run |
| D16 | Polish bar | 0 crash/softlock, 141 FPS, contour cartoon | 🟡 outline Sobel OFF (crash wgpu) |

---

## 3. Shipping-blockers vs Nice-to-have

### 🔴 BLOCKERS — sans ça, pas de jeu fini
| # | Blocker | Pourquoi | Story |
|---|---|---|---|
| B1 | Gimmicks d'armes câblés | "4 façons de jouer" promis, "4 skins d'AR" livré | 564 |
| B2 | Boons perceptibles | 18 boons invisibles = Excel déguisé, 0 raison de refaire | 565 |
| B3 | Économie recalibrée | ~2/18 boons achetables → progression de run cassée | 566 |
| B4 | Méta-progression + sink souls | mourir ne rapporte rien de durable → 0 rétention | 569 |
| B5 | Persistance disque | progrès effacé au quit = non shippable | 569 AC4 |
| B6 | Impact de tir (slice B) | le tir ne "tape" pas, meilleur ROI feel | 559 slice B |
| B7 | Onboarding minimal | joueur Steam perdu sans contexte | NEW |
| B8 | Boucle multi-stage réelle OU honnêteté UI | UI ment ("WAVE X / 4 stages" vs 3 vagues 1 stage) | 567 + fix UI |

### 🟢 NICE-TO-HAVE — shippable sans
Sprint/crouch/slide (560) · variété de vagues (567) · tag-synergy (568) · verticalité/structures (562/563) · POI gameplay (561 DONE) · outline Sobel · voix forgeron · telegraph ennemi · particules ambient.

> Discipline P3 : les blockers sont **systémiques** (armes, boons, méta, persistance), pas du contenu. Gunfire Reborn lv1 n'avait ni slide ni verticalité.

---

## 4. Chemin critique (top stories priorisées)

| Rang | Story | Blocker | Effort | Dépendances |
|---|---|---|---|---|
| 1 | 559 slice B — Impact de tir | B6 | M | `WeaponFiredEvent` (fps/combat) |
| 2 | 564 — Gimmicks d'armes | B1 | L | coord forgia-combat |
| 3 | 566 — Recalibrage économie | B3 | S | genome/constantes (quick win fort levier) |
| 4 | 565 — Boons perceptibles | B2 | M | 559 (VFX) + 566 (atteignables) |
| 5 | 569 — Méta hub + persistance | B4+B5 | L | 564+565+566 |
| 6 | NEW — Onboarding minimal | B7 | S | — |
| 7 | NEW — Honnêteté boucle/UI multi-stage | B8 | S→M | 567 si multi-stage réel |
| 8 | 567 — Variété de vagues | N2 | S | 566 |

**Différés explicites (Phase 0.5, pas dans le chemin critique)** : 560, 562, 563, 568.
**Estimation grossière chemin critique : ~25-30 jours solo.**

---

## 5. Friction Log (manques constatés)

### Open P0 (bloque le ship)
- **FL-R01** Souls méta sans sink (run.rs:729 MetaSouls, 0 consumer) → 569
- **FL-R02** Aucune persistance disque (MetaSouls volatile) → 569 AC4
- **FL-R03** 4 armes = 1 AK générique, gimmicks TOML jamais lus (run.rs:278 `let _ = &equipped;`) → 564
- **FL-R04** 18 boons invisibles, pure mutation stat (boons_apply.rs:45-74) → 565
- **FL-R05** ~2/18 boons achetables/run, 48 souls morts (waves.rs:231-267) → 566
- **FL-R06** Aucun onboarding, joueur spawn sans contexte → NEW

### Open P1 (casse l'illusion de jeu fini)
- **FL-R07** UI "WAVE X / 4 stages" vs réalité 3 vagues 1 stage (hud.rs:78 vs waves.rs WAVES_TOTAL=3) → 567+fix
- **FL-R08** Tir sans muzzle/tracer/hit-flash/hit-stop (fps_feel.json seul retour) → 559 B
- **FL-R09** SFX du tir absent (arena.json damage_sounds_played:0) → 559 B
- **FL-R10** Sprint mappé mais non consommé, vitesse hardcodée 5.0 (forgia-player/lib.rs:431) → 560
- **FL-R11** Outline cartoon OFF, crash wgpu (lib.rs:119-127) → NEW tech
- **FL-R12** Bug hearts double-dip : soigne ET donne souls (run.rs:355-364) → 566 AC6

### Open P2 (dette avant ship)
- **FL-R13** ~15 TODO(story-471..479) "API removed" stubs no-op (barks/music/portal/notifications)
- **FL-R14** draw_portal_overlay / draw_bark_bubble / draw_stage_notification = stubs morts (hud.rs:470,510,525)
- **FL-R15** 5 hardcodes économie (story-566 AC2)
- **FL-R16** 2 systèmes de drop parallèles = 2 vérités (story-566 AC5)

### Stories candidates générées
- story-57X P0 : Onboarding minimal Roguelite (FL-R06)
- story-57X P1 : Honnêteté boucle multi-stage + alignement UI (FL-R07)
- story-57X P1 : Fix render graph outline Sobel (FL-R11)

---

## Verdict

**Roguelite NON shippable aujourd'hui (~40% MVG).** Ossature solide (Defeat/Victory, HUD, économie wirée, audio slice A, stage décoré) mais **4 piliers de genre absents** : armes distinctes (B1), boons ressentis (B2), boons atteignables (B3), rétention méta + persistance (B4/B5).

**Le travail n'est PAS "ajouter du contenu" — c'est CÂBLER ce qui est déjà à moitié construit.** Tout est défini en TOML/genome ; connecter, pas inventer. Profil "infra shippée non wirée" (cf MEMORY `feedback_infra_shipped_not_wired_pattern`).

**Chemin critique** : 559(B) → 564 → 566 → 565 → 569 + onboarding + honnêteté UI. Aucune reco ne touche au jardinage moteur ; chaque blocker est une feature exercée par le joueur en runtime.

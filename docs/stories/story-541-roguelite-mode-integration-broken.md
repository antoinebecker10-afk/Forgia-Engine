# Story-541 — Roguelite mode integration broken (player invulnerable + bot AI no-LOS + Souls bridge + HP UI suspect)

**Status:** DRAFT
**Scale:** BMAD Standard (cross-crate, story requise, checklist post-impl)
**Created:** 2026-05-27
**Blocks:** Roguelite playable end-to-end avec challenge gameplay
**Related:** memory `[[reference-roguelite-damage-bridge-pattern]]`, `[[feedback-fictive-done-status-2026-05-21]]`, story-490, story-517

---

## 1. Contexte

Session test 2026-05-27 (run Roguelite, depth 0, wave 2, 278 s = 4 min 38 s) après application story-540 phase 2 (corridor + KCC offset). User signalement subjectif : *"Soudainement ça apparaît comme si y'avait de la latence et je ne peux plus me déplacer correctement, c'est du hardcode ? C'est lié aux niveaux ? D'où vient le problème ? Je n'ai plus non plus mes HP UI et je ne subis aucun dégâts."*

**Phase 2 story-540 a marché** (preuves chiffrées) :
- `stuck_frames_consecutive`: 538 → 1
- `stuck_events_session`: 5 → 2
- `kcc_collisions`: 0 silencieux → 20 (vu)
- `lag_events`: CRITICAL 42/30s → WARN 12/30s
- Session 36 s → 278 s sans crash technique

**Mais 4 bugs additionnels exposés** :

| # | Bug | Sensor preuve | Impact gameplay |
|---|---|---|---|
| B1 | Player invulnérable | `forgia2_combat.screen_flash.damage_flashes_session: 0` sur 278 s | Zéro challenge |
| B2 | Bot AI no-LOS sur 100 % des bots | `forgia_bot_ai.bots_with_los: 0` sur 28 alive, `bots_attacking: 5` (state sans hit) | Bots inoffensifs |
| B3 | Souls bridge cassé | `total_kills_session: 30` mais `souls_current/total: 0` | Pas de progression économie |
| B4 | HP UI invisible (signalé subjectif) | `forgia2_player_hp_diag.last_skip_reason: "AppNotInGame"` (stale t=172.7 vs session 278.3) → sensor n'a pas tick en jeu | Pas de feedback HP |

## 2. Diagnostic préliminaire

### B1 — Player invulnérable

- Player a `forgia_damage::Health::new(100.0)` (forgia-player/lib.rs:141)
- Bots Roguelite ont `forgia_combat::Health` (waves.rs:129) — pattern story-490 maintenu
- `bots_with_los: 0` (B2) explique : les bots ne tirent jamais effectivement, donc aucun dégât au player
- **Cause root probable** : B2 cascade (pas LOS → pas tir → pas dégât)

### B2 — Bot AI no-LOS

`forgia_bot_ai.json` :
```json
{
  "bots_alive": 28, "bots_with_los": 0,
  "bots_in_grace": 16, "bots_alerted": 12,
  "bots_chasing": 9, "bots_attacking": 5,
  "los_checks_session": 5860,  // = 21 checks/s
  "tuning": { "los_hz": 8.0 }   // attendu 224 checks/s
}
```

**Anomalie quantitative** : `5860 / 278 = 21 checks/s` (mesure) vs `8 Hz × 28 bots = 224 checks/s` (attendu) → bot AI tourne à **9 % du rate prévu**.

5 bots en state `Attacking` mais `bots_with_los: 0` → les bots entrent en state Attacking via `alert_radius_m: 25` ou `chase → attack transition` sans vérifier LOS, et le tir lui-même fait un raycast qui échoue.

**Hypothèses** :
- H1 : Bot AI plugin (`forgia-ai-arena-bot`) gated `GameMode::Fps` quelque part, sous-tick en Roguelite
- H2 : Bot tir raycast filter exclut les bons groupes Roguelite (story-490 collision groups pas wired Roguelite)
- H3 : Bot LOS check vise une entité Player avec un Marker FPS-only (e.g. `FpsCamera`, `BotTarget` setup différemment)
- H4 : Story-539 plugin gating absent → systems run en double mais désynchronisés

### B3 — Souls bridge cassé

- 30 kills, 30 kill_flashes (UI marche)
- 0 souls_current/total dans `forgia2_roguelite_state.json`
- Pattern memory `[[reference-roguelite-damage-bridge-pattern]]` (story-490) explique la fix V1 : `despawn_dead_cubes` trigger DeathEvent. Si DeathEvent n'est pas Observer-listened côté Roguelite (pickup spawn / soul credit), kill compte mais souls=0.

**Hypothèse** : Observer story-490 manque sur Roguelite OU l'Observer attend `source: Some(Player)` mais despawn_dead_cubes envoie `source: None`.

### B4 — HP UI invisible (subjectif user)

`forgia-ui-lib/src/hud/player_hp.rs:22-25` gate :
```rust
if *app_state.get() != AppMode::InGame
    || !matches!(*game_mode.get(), GameMode::Fps | GameMode::Roguelite)
{
    return;
}
let Ok(health) = q_player.single() else { return; };
```

Conditions semblent OK pour Roguelite. Causes possibles :
- `q_player.single()` Err si 0 ou 2+ entités avec `DamageHealth` → mais audit confirme Player unique avec DamageHealth dans le workspace
- `ctx.content_rect()` retourne rect minuscule
- HP bar position bas-gauche masquée par autre overlay
- User n'a pas vu (la HP bar est très bas dans l'écran, 24 px du bord)

**Hypothèse principale** : confusion subjective user (HP bar présente mais peu visible) OU bug egui ctx_mut. À confirmer visuellement au prochain test.

## 3. Goals

1. Identifier la cause root de B2 (bot AI no-LOS) — c'est le bug cascade qui explique B1
2. Réparer le Souls bridge B3 (probablement Observer manquant dans Roguelite)
3. Confirmer ou infirmer B4 visuellement au prochain test (pas de patch avant preuve visuelle)
4. Préserver toutes les corrections phase 2 story-540 (corridor + KCC offset)

## 4. Non-Goals

- Refactor full bot AI cross-mode → si fix > 30 lignes, scinder en story-542
- Migration V7 damage pipeline uniforme (story-491 pending)
- HP UI design overhaul (style/position) — séparé

## 5. Acceptance Criteria

- [ ] AC1 — Phase 1 investigation : identifier la cause root de B2 (LOS, gate, filter, raycast layer ?)
- [ ] AC2 — `forgia_bot_ai.bots_with_los > 0` au moment où player est à portée d'un bot
- [ ] AC3 — `forgia2_combat.damage_flashes_session > 0` après 60 s de wave 2 sans cover
- [ ] AC4 — `forgia2_roguelite_state.souls_current > 0` après 1er kill
- [ ] AC5 — `forgia2_player_hp_diag.last_player_count == 1` quand in_run (sensor doit tick en jeu)
- [ ] AC6 — HP bar visible en bas-gauche en Roguelite (test visuel confirmé)
- [ ] AC7 — Stuck KCC story-540 reste corrigé (`stuck_frames < 60` sustained, `stuck_events_session ≤ 2`)
- [ ] AC8 — `cargo check --workspace` + clippy 0 warning
- [ ] AC9 — Tests existants forgia-stage + forgia-player + tests nouveaux story-540 verts

## 6. Plan d'implémentation

### Phase 1 — Audit B2 (M, 1 h)

- Lire `forgia-ai-arena-bot::los_check` system + filter Rapier
- Vérifier collision groups bot raycast vs Player Collider groups
- Vérifier gate `run_if(...)` autour des systems bot_ai
- Identifier si bot LOS raycast cible `Player` marker ou autre

### Phase 2 — Fix B2 selon cause (M, 30 min - 2 h)

Branche A : LOS filter cassé → corriger QueryFilter groups
Branche B : Gate run_if Fps-only → étendre à `GameMode::Fps | Roguelite`
Branche C : Bot tir cible mauvais marker → ajouter targeting Player marker cross-mode

### Phase 3 — Fix B3 Souls bridge (S, 30 min)

- Lire Observer `on_bot_death` dans forgia-mode-roguelite
- Vérifier qu'il listen DeathEvent + spawn Soul pickup
- Si manque : ajouter Observer ou corriger gate

### Phase 4 — Vérif B4 visuel (S, 5 min)

- Test runtime + screenshot HP bar
- Si absente : audit egui ctx + draw call
- Si présente : feedback user était subjective confusion, fermer B4

### Phase 5 — Verification (M, 30 min)

- `cargo check --workspace` clean
- `cargo clippy --workspace --no-deps -- -D warnings` 0 warning
- Tests purs verts (98+ forgia-stage)
- Runtime test : tirer sur bot, voir damage, prendre dégâts, voir HP descendre, kill → soul credit

### Phase 6 — Capitalisation (S, 15 min)

- Memory `[[reference-roguelite-mode-integration-checklist]]` : pour chaque nouveau mode V2, gate UI + bot AI + damage routing
- Update story-541 findings §10 avec root cause B2

## 7. Risques

| Risque | Mitigation |
|---|---|
| B2 cause = bot AI fondamentalement Fps-only, refactor lourd | Phase 1 borne le scope, escalade story-542 si > 30 LOC |
| Fix B2 réintroduit le stuck KCC story-540 (bots qui plus push) | AC7 garde-fou non-régression |
| Souls Observer attend `source: Some(Player)` mais despawn_dead_cubes envoie `None` | Soit ajouter player entity au DeathEvent, soit Observer accepte `None` avec default attribution |
| HP UI subjective : pas un vrai bug | Phase 4 confirme avant tout patch |
| Stutters lag_events 12/30s restent | Story-539 plugin gating distincte |

## 8. Definition of Done

- AC1-9 verts
- Sensors runtime preuve : bots_with_los > 0, damage_flashes > 0, souls > 0, HP UI visible
- Commit propre + memory capitalisée

## 9. Follow-ups (stories candidates)

- **Story-542** Bot AI cross-mode unification si phase 2 nécessite refactor
- **Story-543** HP UI design overhaul si B4 confirmé bug visuel (position/visibility)
- **Story-491** déjà candidate (V7 damage pipeline uniforme) — peut absorber B3 si déjà planifié

## 10. Findings phase 1 (à remplir)

_TO BE WRITTEN après audit phase 1 prochaine session._

## 11. Notes session 2026-05-27

- Phase 2 story-540 a corrigé le stuck principal — preuves chiffrées section §1
- User subjective : *"latence soudaine + je ne peux plus me déplacer correctement"* = combinaison probable :
  - Story-539 stutters cumulés (lag_events 12/30s)
  - B2 cascade : 28 bots qui font des transitions AI Attacking↔Chasing sans pouvoir agir = CPU overhead pour rien
  - Bot dogpile push (kcc_collisions=20) au moment T sensor
- `total_kills_session: 30` sur 278 s = 1 kill toutes les 9 s → user actif et compétent en shoot, mais la run est sans risque
- Aucun crash technique : 200576 ticks, RAM 1214 MB stable, watchdog OK

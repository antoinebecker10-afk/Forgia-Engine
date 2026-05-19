# Story-456 — Hit Feedback AAA (Nameplate billboards + layered damage + headshot routing)

**Statut** : IN PROGRESS
**Scope BMAD** : Enterprise (3 vagues, ~10 fichiers, ~3 sessions)
**Date** : 2026-05-19
**Prerequis** : story-455 FPS UI Juice DONE (player_hp + ammo HUD + kill feed)
**Sources research** : agent 2026-05-19 + memory `reference_aaa_juice_safe_values_2024.md`

---

## 1. Contexte

Le bot HP floater V1 (`forgia-ui-hud/src/bot_hp_floaters.rs`, 190 LOC) utilise `egui::Painter` + `Camera.world_to_viewport` — pattern legacy avec 4 défauts :

1. **Alloc egui par frame** (chunky_rect_filled + text_with_outline) — viol hot path observability-required + scalability
2. **Pas d'antialiasing shader** — barre pixely à distance
3. **Pas de Z-buffer** — barre traverse murs/terrain (bug arena story-447 colliders precise)
4. **Couplé à V1 `forgia-combat::CombatHealth` + `CombatHitEvent`** — duplique avec V2 `forgia-damage::Health` + `DamageEvent` qui est la source de vérité

De plus aucun feedback différencié headshot/bodyshot/limb visible côté tireur (popup blanc uniforme), aucune notion de shield/armor layered (Apex/OW2 standard), aucun audio cue distinct headshot (Halo `shield-break` pattern).

---

## 2. Objectifs (qualité non négociable, CLAUDE.md §3 Quality Gate)

- **Propre** : 0 warnings clippy strict, conventions Forgia, deprecate V1 sans laisser de code mort
- **Performant** : billboard GPU-rendered, 0 alloc CPU/frame, `Changed<Health>` driven
- **Optimisé** : genome-driven `hit_feedback.toml` (timings, colors, multipliers headshot/limb), hot-reload Shift+F12
- **Scalable** : N enemies sans cap, 4 tiers shield (Apex), 3 hitzones (Hunt/OW2), router CombatHitEvent V1→DamageEvent V2 propre
- **Observable** : sensor `forgia_hit_feedback.json` (last hit body_zone, headshot count, popups actifs, floaters actifs) + health check staleness

---

## 3. Architecture cible — 3 vagues

```
DamageEvent { target, amount, kind, source_pos: Option<Vec3>, body_zone: HitZone }
       ↓
   [V2: forgia-shield] apply_shield_first  (system ordering BEFORE damage)
       ↓ (shield broken → overflow)
   [V2: forgia-armor] apply_armor_mitigation (system ordering BEFORE damage)
       ↓ (DR% applied)
   [forgia-damage] apply_to_health (existant V2)
       ↓ emit DamageAppliedEvent (NEW, post-mitigation result)
   ┌───────────────────┬────────────────────┬─────────────────────┐
   ↓                   ↓                    ↓                     ↓
forgia-damage-       forgia-enemy-       forgia-juice-          forgia-audio
numbers              nameplate           screen-flash           (cue headshot)
(popup color-coded)  (3D billboard fade) (V3 headshot pulse)    (V3)
```

### VAGUE 1 — MVP visible (cette session)

**Sortie** : nameplate billboard 3D propre + popups color-coded + body_zone routing + headshot popup gold + bot_hp_floaters V1 deprecated.

Fichiers (~5) :

1. **`assets/genomes/hit_feedback.toml`** (NEW) — tuning genome :
   ```toml
   [popup]
   color_normal = [1.0, 1.0, 1.0, 1.0]        # blanc HP
   color_headshot = [1.0, 0.85, 0.2, 1.0]      # gold (Apex/Destiny)
   color_shield = [0.5, 0.8, 1.0, 1.0]         # bleu (OW2/Apex)
   color_armor = [1.0, 0.85, 0.4, 1.0]         # jaune (OW2)
   color_kill = [1.0, 0.3, 0.2, 1.0]           # rouge kill confirmed
   lifetime_secs = 1.0
   rise_speed = 1.6
   font_size_base = 22.0
   font_size_headshot = 30.0

   [hitzone_multipliers]
   head = 2.0
   body = 1.0
   limb = 0.75

   [nameplate]
   fade_duration_secs = 2.0
   distance_scale_curve = "inverse"  # scale = base / dist
   distance_scale_min = 0.4
   distance_scale_max = 1.6
   y_offset_world = 2.2
   width_world = 1.0
   height_world = 0.08
   color_bg = [0.06, 0.07, 0.09, 0.85]
   color_outline = [0.0, 0.0, 0.0, 0.95]
   ```

2. **`crates/forgia-damage/src/lib.rs`** (EXTEND) — ajout `HitZone` enum + champ `body_zone` dans `DamageEvent` + nouveau `DamageAppliedEvent` (post-mitigation, pour observers downstream)
3. **`crates/forgia-damage-numbers/src/lib.rs`** (UPGRADE) — lire `body_zone` + couleurs depuis tuning genome (Resource sync), font_size headshot doublé, outline noir (text_with_outline pattern)
4. **`crates/forgia-enemy-nameplate/`** (NEW crate) — Plugin + `HpFloater` Component + system spawn billboard 3D mesh sur DamageAppliedEvent + shader unlit WGSL `nameplate_hp.wgsl` (vertex billboard + fragment masque progress) + fade lifetime + sensor JSON
5. **`crates/forgia-ui-hud/src/bot_hp_floaters.rs`** (DEPRECATE) — supprimer complètement, le remplacement vit dans `forgia-enemy-nameplate`. Update `forgia-ui-hud/src/lib.rs` pour retirer le plugin.
6. **`crates/forgia-combat/src/lib.rs`** (BRIDGE) — `CombatHitEvent` reader qui forward vers `DamageEvent` V2 si pas déjà routé, avec body_zone détecté via ChildOf walk (HitZone Component existant ?)

**Critères acceptance V1** :
- [ ] Tirer un bot affiche nameplate 3D billboard fade-out 2s au-dessus
- [ ] Headshot affiche popup gold "−X" plus gros + numbers > 22pt
- [ ] Bodyshot affiche popup blanc, limbshot popup blanc plus petit
- [ ] Nameplate scale visible loin (test distance 30m)
- [ ] Nameplate occlusion : ne traverse pas terrain (Z-test enabled)
- [ ] Sensor `forgia_hit_feedback.json` écrit last_hit_zone + active_nameplates count
- [ ] 0 clippy warning workspace strict
- [ ] qa-lead + verifier sub-agents OK
- [ ] Genome hot-reload OK (Shift+F12 change couleur popup live)

### VAGUE 2 — Layered damage (session +1)

**Sortie** : shield bleu se vide avant armor jaune avant HP rouge, popup couleur = pool touché, nameplate 3-segment.

Fichiers :
- `crates/forgia-armor/src/lib.rs` (POPULER scaffold 16 LOC) — `Armor { current, max, dr_percent }` + system `apply_armor_mitigation` ordering `before(apply_damage)`
- `crates/forgia-shield/` (NEW crate scaffold-check first) — `Shield { current, max, regen_per_sec, regen_delay_secs, last_hit_at }` + system `apply_shield_first` + `tick_shield_regen`
- `crates/forgia-enemy-nameplate/` (EXTEND) — 3 segments visuels (shield cyan / armor jaune / HP rouge), shader nameplate_hp.wgsl à 3 progress
- `assets/genomes/hit_feedback.toml` (EXTEND) — `[shield]` + `[armor]` tuning
- `crates/forgia-damage-numbers/src/lib.rs` (EXTEND) — couleur popup = pool finalement touché (shield blanc / armor jaune / HP rouge)

**Critères acceptance V2** :
- [ ] Bot avec shield 50/100 + HP 100 absorbe les 50 premiers dmg en shield (popup blanc), puis bascule HP (popup rouge)
- [ ] Armor 100 avec DR 30% : popup montre dmg final post-DR
- [ ] Shield regen après 5s sans hit (genome `regen_delay_secs`)
- [ ] Nameplate affiche 3 segments empilés ou côte-à-côte
- [ ] 0 clippy + sub-agents OK

### VAGUE 3 — Polish AAA (session +2)

**Sortie** : audio cue distinct headshot + screen pulse subtil + settings toggle damage_numbers (opt-in/out style OW2).

Fichiers :
- `crates/forgia-audio-core/src/lib.rs` (probable) — `play_one_shot(handle)` API si pas déjà
- `assets/audio/hit/headshot_metal.ogg` + `bodyshot_thud.ogg` + `shield_break.ogg` (sources CC0 freesound — veille)
- `crates/forgia-juice-screen-flash/src/lib.rs` (EXTEND) — listener `DamageAppliedEvent { body_zone: Head, source: Player }` → flash léger (12% alpha, 80ms, jaune chaud Apex style)
- `crates/forgia-ui-settings/` (check scaffold) — toggle `show_damage_numbers: bool` persisté
- `assets/genomes/hit_feedback.toml` (EXTEND) — `[audio]` + `[settings_defaults]`

**Critères acceptance V3** :
- [ ] Headshot joue son métal + flash écran léger (toggleable)
- [ ] Bodyshot joue son sourd
- [ ] Shield break joue son distinct (Halo pattern)
- [ ] Settings menu permet de désactiver damage numbers (popups invisibles si off) — nameplate reste visible
- [ ] Anti-eye-strain : flash plafonné à `reference_aaa_juice_safe_values_2024.md` (12% alpha max, 80ms)

---

## 4. Stability Locks impactés

- **L1 GameAssets** : ajout `nameplate_hp.wgsl` shader handle + (V3) 3 audio handles → re-baseline `e1_e2_baseline_*.json` requis si dépassement seuil
- **L7 GameSet** : nouveau system order constraint — shield/armor systems doivent run dans `GameSet::Combat` BEFORE `apply_damage`

---

## 5. Sensors / Observability

- `forgia_hit_feedback.json` (NEW, 10s genome interval) :
  ```json
  {
    "timestamp_secs": 12345.6,
    "popups_active": 3,
    "nameplates_active": 1,
    "last_hit_zone": "Head",
    "last_hit_pool_touched": "Hp",
    "headshots_total": 42,
    "shield_breaks_total": 5,
    "active_floater_timers_max_secs": 1.8
  }
  ```
- Health check : `nameplates_active > 50` → alert "Nameplate leak — check despawn on DeathEvent"
- Health check : `popups_active > 30` → alert "Popup leak — check ttl decrement"

---

## 6. Sources industrie (sourcées, no hallucination)

- Valorant Hit Registration tech blog — hitmarker worldspace : <https://playvalorant.com/en-us/news/dev/the-state-of-hit-registration/>
- Apex damage colors (Shacknews) — shield tier color coding
- Halo Infinite Red Reticle Range (Gfinity) — RRR + shield-break audio cue
- Hunt: Showdown Hit Zone Damage Modifiers (dev tracker) — body multipliers
- Destiny 2 Precision Damage Wiki — yellow popup precision
- Overwatch 2 critical indicator (Blizzard forums) — crosshair yellow line on crit
- GDC Pennant "Designing for Color-Blindness" — couleur jamais seule, redondance audio/forme
- Memory : `reference_aaa_juice_safe_values_2024.md` (camera shake max 1.5°, screen flash max 12% alpha 80ms)

---

## 7. Hors scope

- Pas de **floating health bar permanente quand visé** (style Tarkov scan) — fade-out post-hit suffit MVP, peut être story-457 si user feedback
- Pas de **ragdoll** sur DeathEvent (story séparée — physique)
- Pas de **gibs/blood VFX** (story VFX hanabi séparée)
- Pas de **outline silhouette flash** style Hunt — story-458 si demandé
- Pas de **netcode replication** — single player V2 pour l'instant

---

## 8. Plan d'exécution Vague 1 (cette session)

1. Créer `assets/genomes/hit_feedback.toml` + ajouter au catalogue + sync resource
2. Extend `forgia-damage` : `HitZone` enum + `body_zone` field + `DamageAppliedEvent`
3. Router `CombatHitEvent` V1 → `DamageEvent` V2 dans `forgia-combat` (avec body_zone detection)
4. Upgrade `forgia-damage-numbers` : color-coded + font sizes + outline + lecture genome Resource
5. Créer crate `forgia-enemy-nameplate` : Plugin + Component + shader + spawn/fade systems + sensor JSON
6. Deprecate `bot_hp_floaters.rs` : delete file + retirer import dans `forgia-ui-hud/src/lib.rs`
7. Wire-up plugins dans `forgia-game/src/main.rs` ou root plugin chain
8. `cargo check -p forgia-enemy-nameplate -p forgia-damage -p forgia-damage-numbers`
9. `cargo clippy --workspace -- -W warnings`
10. Sub-agents `verifier` + `qa-lead` en parallèle
11. Test runtime : tirer bot Arena, vérifier nameplate + popup gold sur headshot, sensor JSON cohérent
12. Update story-456 statut Vague 1 DONE + checklist post-impl

---

*Story créée 2026-05-19. Vague 1 in progress.*

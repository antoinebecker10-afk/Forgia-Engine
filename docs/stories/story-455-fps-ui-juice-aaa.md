# Story-455 — FPS UI Juice AAA (Ammo HUD + Kill Feed + Damage Direction + Cleanup + Pause)

**Statut** : DRAFT (en attente validation Antoine)
**Scope BMAD** : Enterprise (10+ fichiers, 4-6 new crates, ~5-8 sessions)
**Date** : 2026-05-18
**Prerequis** : story-453 baseline combat reset DONE (ConvexHull AsyncSceneCollider, damage path ChildOf walk)
**Sources research** : 2 agents background 2026-05-18 (rapports archivés conversation)

---

## 1. Contexte

Bilan UI mode FPS Arena post-story-453 :

**Présent** : crosshair (forgia-crosshair), hitmarker (forgia-hitmarker), player HP bar + low-HP vignette (forgia-ui-hud::player_hp), wave counter (forgia-ui-hud::wave_counter), bot HP floaters + damage popups (forgia-ui-hud::bot_hp_floaters), scoreboard TAB (forgia-fps::score), pause overlay (forgia-ui).

**Trous critiques** :
1. **Aucune UI ammo / weapon active** — `EquippedWeapons.ammo_rifle=999` stubbed infinite, joueur ne voit ni clip ni reserve ni arme active.
2. **Pas de kill feed** — `ArenaScore.kills` s'incrémente silencieusement.
3. **Pas de damage direction indicator** — vignette rouge low-HP globale ne dit pas d'où vient le tir.
4. **9 scaffolds UI à 16 LOC** réservent des noms sans contenu.
5. **`ForgiaJuiceScreenFlashPlugin` mort** + red-vignette dupliqué inline (viol fine-grained-crates).
6. **Pause overlay minimal** — pas de bouton resume/quit/settings, juste texte.

---

## 2. Objectifs (qualité non négociable, CLAUDE.md §3 Quality Gate)

- **Propre** : 0 warnings clippy strict, conventions Forgia respectées
- **Performant** : event-driven (pas poll), `Local<T>` caches, 0 alloc hot path UI
- **Optimisé** : **genome-driven** (`hud_tuning.toml` + `killfeed_tuning.toml` + extension `viewmodel_arena.toml` pour ammo/mag/reload), hot-reload Shift+F12, backward compat
- **Scalable** : N armes (pas 4 hardcoded), N attackers (DDI multi-arcs), 4-6 crates fine-grained (règle `fine-grained-crates.md`)
- **Observable** : 1 sensor JSON par feature (`forgia_hud_ammo.json` / `forgia_killfeed.json` / `forgia_damage_dir.json`) + health checks staleness

---

## 3. Architecture cible — 5 phases ordonnées

### Phase A — Ammo gameplay foundation (PRÉREQUIS HUD)
**Sans ammo réel, l'UI ammo est cosmétique.** À faire AVANT phase B.

**Crate** : extension `forgia-combat` (data + state machine, pas new crate)

**Changements** :
- Étendre `ViewmodelGenomeEntry` (`crates/forgia-fps/src/lib.rs:99`) avec champs ammo :
  - `mag_size: u32` (default 30)
  - `reserve_max: u32` (default 120)
  - `reload_time_secs: f32` (default 1.8)
  - `reload_kind: String` ("mag" | "shell_per_shell" pour Boucherie pump)
  - `infinite_ammo: bool` (default false — toggle pour playtest/dev)
- Remplacer `EquippedWeapons` (`crates/forgia-combat/src/weapons.rs:163`) :
  ```rust
  pub struct EquippedWeapons {
      pub current: WeaponType,
      pub slots: HashMap<WeaponType, AmmoSlot>, // per-weapon ammo state
  }
  pub struct AmmoSlot {
      pub current_mag: u32,
      pub reserve: u32,
      pub reload_state: ReloadState, // Idle | Reloading { t_remaining }
  }
  ```
- Supprimer hardcode `ammo_rifle: 999` + `max_ammo() -> 999`.
- Système `consume_ammo_on_fire` : `fire_weapon_minimal` (`forgia-fps::lib.rs`) décrémente `current_mag` au lieu de no-op.
- Système `reload_state_machine` : touche R démarre `Reloading`, timer décrémenté, à fin transfer reserve→mag clamped `mag_size`.
- Genome-driven : valeurs lues depuis ViewmodelGenome via `viewmodel_arena.toml`, **pas de constantes Rust**.
- Hot-reload : Shift+F12 reload genome → recalcul max ammo (clamp current_mag).
- **Bus event** : `MessageWriter<AmmoChanged>` { weapon, current, reserve, kind: Fire|Reload|Pickup|WeaponSwitch }.

**Sensor** : étendre `forgia_hitscan.json` (déjà existant) avec sous-objet `ammo_state` per weapon.

**Tests** : reload mid-fire interrompu par switch arme, infinite_ammo flag, mag_size hot-reload pendant reloading.

---

### Phase B — `forgia-ui-hud-ammo` (NEW CRATE)
**Crate** : `crates/forgia-ui-hud-ammo/` (peuple le pattern fine-grained, pas dans forgia-ui-hud monolithe — éviter le drift "tout dans ui-hud")

**Layout** (research AAA : Halo/Apex/Destiny/Valorant bottom-right consensus) :
- Compteur principal **bottom-right** : `{current_mag} / {reserve}` font monospace bold 96px, color jaune cartoon
- Sous-titre `× {pellets}` si arme multi-pellets (Boucherie)
- Icône arme à gauche du nombre (texture from genome `display_name` → asset path `assets/icons/weapons/{slug}.png`, fallback rectangle)
- **Vertical slot strip droite** : N cases (1 par arme équipée), highlight active (scale 1.1 + outline jaune), inactives alpha 60%
- Hotkeys 1-N affichées en coin de case (lu via `KeybindRegistry`, pas hardcode `Digit1-4`)
- **Reload progress bar** : arc circular autour de l'icône pendant reload (`reload_state.t_remaining`)
- **Low-ammo flash** : pulse rouge 2Hz quand `current_mag ≤ mag_size × low_threshold` (genome `low_ammo_threshold: f32 = 0.25`)

**Genome** : `assets/genomes/hud_ammo_tuning.toml` (positions/tailles/seuils/couleurs) — pas de hardcode pixel.

**Performance** :
- `Local<AmmoHudState>` cache valeurs dernière frame, recompute layout seulement sur `AmmoChanged` event
- 1 painter Foreground layer
- No alloc in draw closure (préallouer String via `format_args!` ou pre-baked into Local)

**Sensor** : `forgia_hud_ammo.json` { active_weapon, slots: [...], last_render_ms, low_ammo_active }

**Cleanup** : delete scaffold `crates/forgia-ui-gauges` (16 LOC mort, son intent = ammo/HP bars = redondant avec player_hp + cette crate).

---

### Phase C — Étendre `CombatHitEvent` (BLOQUANT phases D & E)
**Migration breaking** — à faire avant kill feed + DDI.

`crates/forgia-combat/src/combat_juice.rs:24` actuel :
```rust
pub struct CombatHitEvent { target: Entity, damage: f32, is_kill: bool }
```

Cible :
```rust
pub struct CombatHitEvent {
    pub target: Entity,
    pub attacker: Option<Entity>,        // None = world damage (fall/lava future)
    pub damage: f32,
    pub is_kill: bool,
    pub is_headshot: bool,
    pub hit_world_pos: Vec3,             // pour DDI angle calc + popup spawn
    pub weapon: Option<WeaponType>,       // pour kill feed icon
}
```

**Producteurs** : `forgia-fps::fire_weapon_minimal` (player→bot), futur `forgia-ai-arena-bot` (bot→player).
**Consommateurs actuels** : `forgia-ui-hud::bot_hp_floaters::collect_hit_events`, `forgia-fps::score::record_kill_on_hit`, `forgia-hitmarker`.
Migration : ajouter `..Default::default()` ou builder pattern pour producteurs lazy.

**Headshot detection** : déjà absent du damage path. **Hors scope phase C** — flag toujours `false` en attendant phase F (hitzone reset, voir story-453 deferred). Annotation `TODO(story-456)` au point de spawn.

---

### Phase D — `forgia-killfeed` (NEW CRATE)
**Crate** : `crates/forgia-killfeed/`

**Layout** (research : Overwatch/CS2 top-right, FIFO, ~5-6s) :
- Position **top-right** sous wave counter (panel droit dédié, pas overlap)
- `VecDeque<KillFeedEntry>` cap 5
- Genome `killfeed_tuning.toml` : `max_entries`, `display_secs` (5.0), `fade_out_secs` (1.0), `slide_in_secs` (0.15), `entry_height_px`, font sizes
- Format ligne : `[icon arme]  {attacker_name} → {victim_name}  [★ if headshot]`
- Couleurs : attacker = player → vert (`C_PLAYER`), bot → rouge (`C_ENEMY`)
- Anim : slide-in latéral right→left sur push, fade alpha sur dernière 1s, FIFO pop oldest si full

**Multi-kill banner** (Halo-style sweeping) :
- Tracker `KillStreakState { last_kill_at, streak_count }` → si 2+ kills < 4s sweep banner CENTER_TOP "DOUBLE KILL" / "TRIPLE KILL" / "RAMPAGE"
- Genome `multikill_window_secs: f32 = 4.0`

**Source noms** : `attacker/victim` → query Name component fallback "Player" / "Bot {id}".

**Sensor** : `forgia_killfeed.json` { active_entries, total_kills_session, streak_current, last_kill_ms }.

**Performance** : painter Foreground, `Local<Vec<String>>` pre-allocated 8 capacity (`with_capacity` au build), reuse.

---

### Phase E — `forgia-ui-damage-direction` (NEW CRATE)
**Crate** : `crates/forgia-ui-damage-direction/`

**Pattern** (research : Stephenson UX analysis — 2D autour crosshair CoD-style le plus lisible) :
- Arc rouge centré sur crosshair, angle = `atan2(attacker_xz - player_xz)` projeté plan horizontal caméra
- Rayon outer = 80px (genome), thickness 6px (genome)
- Multi-attackers : arcs **distincts** (pas cumul Apex anti-pattern), max 4 simultanés (genome `max_arcs`)
- Fade : duration 1.2s (genome), alpha ease-out
- Intensity : alpha lerp `[0.5, 1.0]` selon `damage / player_max_hp`
- Arc span : 60° fixed (genome `arc_span_deg`)

**Architecture** :
```rust
#[derive(Resource, Default)]
struct DamageArcsState { arcs: SmallVec<[DamageArc; 4]> }
struct DamageArc { angle_rad: f32, intensity: f32, age: f32 }
```

**Consume** : `CombatHitEvent` filter `target == player_entity` && `attacker.is_some()`, push arc.

**Genome** : `damage_dir_tuning.toml` (radius/thickness/duration/max_arcs/arc_span).

**Sensor** : `forgia_damage_dir.json` { active_arcs, last_angle_deg, last_attacker_entity }.

**Performance** : SmallVec stack-alloc, painter Foreground, recompute only on event.

**Anti-pattern évité** : pas de full-screen edge vignette (Halo/BF "panic" style) — moins lisible pour cartoon, et le red-vignette low-HP existe déjà player_hp.rs (à extraire phase F).

---

### Phase F — Cleanup scaffolds + `forgia-juice-screen-flash` peuplé
**Pas de new crate** — peuple existant + delete morts.

**Peupler** `forgia-juice-screen-flash` :
- Migrer le code red-vignette low-HP de `forgia-ui-hud/src/player_hp.rs:34-44` → `forgia-juice-screen-flash`
- Ajouter triggers : `OnDamage` (red flash 150ms), `OnHeal` (green flash 200ms), `OnKill` (white flash 80ms)
- Genome `screen_flash_tuning.toml` : durations + alphas + colors
- Resource `ScreenFlashState { active: SmallVec<[FlashLayer; 4]> }`, lerp alpha vers 0
- Sensor `forgia_screen_flash.json`

**Delete** scaffolds 16 LOC sans usage (audit final avant suppression) :
- `forgia-ui-menu` (duplicate forgia-ui main_menu)
- `forgia-ui-gauges` (= ammo+HP, redondant phases A/B)
- `forgia-ui-loadscreen` (à garder ? À discuter — utile futur RPG)
- `forgia-ui-credits` (à garder — utile pré-ship)
- `forgia-ui-tooltip` (à garder — utile RPG inventory)
- Keep : `forgia-ui-notifications`, `forgia-ui-settings-panel`, `forgia-ui-minimap`, `forgia-ui-inventory`, `forgia-ui-objectives`, `forgia-input-rebind-ui` (peuplés phases ultérieures)

Decision delete vs keep : à confirmer avec Antoine avant action.

---

### Phase G — `forgia-ui-pause-menu` (NEW CRATE) + settings panel scaffold
**Crate** : `crates/forgia-ui-pause-menu/`

**Contenu** :
- Migrer pause overlay actuel `forgia-ui::pause_overlay` → crate dédié
- Boutons cliquables : Resume / Settings / Quit to Menu (au lieu de juste raccourcis clavier)
- Egui `egui::Area` CENTER_CENTER, frame chunky
- Mouse capture release auto en Paused
- Sub-menu Settings (peuple `forgia-ui-settings-panel`) :
  - Sensitivity mouse X/Y (sync `KeybindRegistry` / input config)
  - FOV slider
  - HUD opacity (sync genome hud_tuning)
  - Volume master/SFX/music (sync audio crate)
  - Reload-on-save (Shift+F12 trigger explicite)
- Persistence : `assets/user_settings.toml` (créé/mis à jour au save)

**Sensor** : `forgia_pause_menu.json` { open, sub_menu, last_settings_save_ms }

---

## 4. Ordre d'exécution recommandé

```
A (ammo gameplay) → B (ammo HUD) → C (event extend) → D (kill feed) → E (DDI) → F (cleanup) → G (pause menu)
```

Phases A et C sont **breaking changes** intra-codebase → faire en début, regrouper migrations consumers.

**Durée estimée** (pessimiste, qualité non négociable) :
- A : 1 session (state machine + tests)
- B : 1 session (HUD + sensor + genome)
- C : 0.5 session (migration event + consumer updates)
- D : 1 session (feed + multi-kill banner)
- E : 0.5 session (1 crate compact)
- F : 0.5 session (cleanup + screen-flash migration)
- G : 1 session (pause menu + settings persistence)

**Total** : ~5.5 sessions BMAD-Standard équivalent. Découpe en stories suiveuses possible (story-455-A, -B, etc.) si Antoine préfère commit-by-phase.

---

## 5. Risques identifiés

| ID | Risque | Mitigation |
|---|---|---|
| R1 | Migration `CombatHitEvent` casse 3+ consumers | Phase C dédiée, builder pattern + Default impl, cargo check après chaque consumer |
| R2 | Headshot flag toujours false sans hitzone Head/Body split (deferred story-453) | Annotation `TODO(story-456)`, kill feed marche sans star headshot en attendant |
| R3 | Genome hot-reload pendant Reloading state = comportement indéfini | Tester reload mid-state, clamp current_mag à new mag_size après reload TOML |
| R4 | Icon textures armes absentes assets/icons/weapons/ | Fallback rectangle colored + warn log, créer assets/icons/weapons/.gitkeep + TODO art pipeline |
| R5 | Scope crate "fine-grained" trop découpé (6 new crates) | Justifié par règle fine-grained-crates.md ; lib.rs ≤200 LOC chacun = sain |
| R6 | Pause menu mouse capture race avec ESC handler existant | Audit ordering `forgia-ui` vs new crate, gating explicite `AppMode::Paused` ⇆ `InGame` |

---

## 6. Critères d'acceptance globaux

- [ ] Phase A : ammo finite, R recharge, switch arme conserve clip, sensor `forgia_hitscan.json` montre ammo_state, 0 hardcode
- [ ] Phase B : compteur visible bottom-right, slot strip 4 armes, hotkeys depuis KeybindRegistry, low-ammo flash 25% genome-driven, sensor `forgia_hud_ammo.json`
- [ ] Phase C : `CombatHitEvent` étendu, 3 consumers migrés, cargo check workspace 0 erreur
- [ ] Phase D : kill feed visible top-right, FIFO 5 entries, multi-kill sweeping banner, sensor `forgia_killfeed.json`
- [ ] Phase E : arcs rouges autour crosshair sur dégâts reçus, multi-attackers distincts, sensor `forgia_damage_dir.json`
- [ ] Phase F : `forgia-juice-screen-flash` peuplé, red-vignette extrait de player_hp.rs, 2-3 scaffolds morts deletés
- [ ] Phase G : pause menu boutons cliquables, settings panel sliders, persistence TOML
- [ ] **Global** : 0 clippy warnings (cargo clippy --workspace -- -W warnings), 0 hardcode pixel ou seuil non genome-driven, 1 sensor JSON par feature, health alerts staleness
- [ ] Post-impl auto-QA (rule `post-impl-auto-qa.md`) : verifier + qa-lead pass sur chaque phase avant DONE

---

## 7. Sources research (archivées conversation)

**Ammo HUD AAA** : Halo Infinite Game Settings, York Univ. IEEE GEM 2015 (diegetic +35% accuracy), Doom Eternal weapon wheel critique, Apex/Destiny/Valorant Game UI Database, Dot Esports Valorant low-saturation backlash.

**Kill feed + DDI** : Jasper Stephenson UX analysis (Medium), CS2 patch killfeed icons, Overwatch Workshop KillFeed mod fade timings, Apex damage stacking critique EA Forums, GDC Vault Replay Tech Overwatch.

URLs complètes dans transcripts agents `a3d4f6b3dd037346b` (ammo) et `a6c27bbf8c2155a6e` (killfeed/DDI).

---

*Pas de blabla. Pas de hardcode. Pas de régression. Du concret, du stable, du livrable.*

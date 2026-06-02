# Story-573 — Sorts F par arme parlante (identité)

> **Status** : DRAFT (2026-06-02)
> **Scale** : Standard (refactor shockwave.rs + hud.rs + sensor.rs)
> **Décisions user 2026-06-02** : Bourrasque = REPOUSSE (gust), cooldown PAR ARME
> **Source design** : workflow design-per-weapon-spells (4 designers + synthèse)
> **Bible** : v1 cartoon family-friendly. Cf [[reference_two_health_types_combat_vs_damage]]

---

## 1. Vision

Le F devient **par arme** : chaque arme parlante a son sort = différenciateur identité #1.
Dispatch via `EquippedWeapons.current`. Cooldown **par arme** (HashMap<WeaponType,f32>).

| Arme / Persona | Sort | Mécanique |
|---|---|---|
| ModernAR / **Pépin** (vert) | Câlin de Pépin | Défensif : heal joueur +35% max HP + push doux, **0 dégât** |
| AssaultRifle / **Bourrasque** (bleu) | Coup de Bourrasque | **Repousse** fort radial + pop (push) + dégâts légers 25 |
| Shotgun / **Mme Lenoir** (violet) | Révérence Forcée | **Aspire** vers le joueur (pull) + chip 15 (combo shotgun) |
| RocketLauncher / **Boucherie** (rouge) | Boum-Bidoche | AOE **dirigé** au point visé (cam forward, 14m) : 70 dmg + pop vertical |

Push vs Pull = lisibilité max (Bourrasque repousse, Lenoir aspire).

## 2. Acceptance Criteria
- **AC1** : F dispatche selon `EquippedWeapons.current` (4 sorts distincts + fallback gust)
- **AC2** : cooldown PAR ARME (`ShockwaveAbility.cooldowns: HashMap<WeaponType,f32>`)
- **AC3** : dégâts via `forgia_combat::Health` (mut direct) + `CombatHitEvent` (chiffres/flash/killfeed/boons), mort via `despawn_dead_cubes`. Pépin = 0 dégât (heal `forgia_damage::Health` du Player via commands.queue, cf waves.rs).
- **AC4** : knockback généralisé `KbMode::{PushFrom, PullTo}` + clamp anti-overshoot pour les pulls (stop si <1.5m de la cible)
- **AC5** : VFX disque coloré par persona (expand pour push, contract pour pull) + CameraTrauma adapté (Pépin 0.2 doux → Boucherie 0.7 brutal)
- **AC6** : HUD indicateur F coloré par persona (speaker_color) + radial cooldown de l'arme courante
- **AC7** : Observability — `shockwave_casts` (existant) + `shockwave_cd` = max cooldown restant

## 3. Risques (du workflow)
- **Double knockback** : `boons_apply::sys_apply_knockback_on_hit` insère un push sur CombatHitEvent → combattrait les pulls SI un boon knockback est actif. **V1 = limitation documentée** (les pulls peuvent être réduits avec un boon knockback actif, rare early). Fix propre = story suiveuse.
- Pop vertical Boucherie : bots KinematicPositionBased sans gravité → l'AI les replace au sol au frame suivant (à vérifier ; sinon clamp y).
- Type Health (cf memory) : bots = forgia_combat::Health, player = forgia_damage::Health.

## 4. Coupes V1 (→ story suiveuse / 566)
- ❌ Mare-slow Boucherie (nouveau component slow sur AI)
- ❌ Float vert "+HP" Pépin (popup egui) — HP bar + VFX suffisent V1
- ❌ Externalisation genome des consts (→ story-566)
- ❌ Fix propre double-knockback boon

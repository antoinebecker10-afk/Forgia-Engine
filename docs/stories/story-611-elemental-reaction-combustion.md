# Story-611 — Roguelite : Réaction élémentaire Combustion (Feu + Poison)

> **Statut** : CODE-COMPLETE + QA OK (2026-06-23) — reste validation runtime
> **Niveau BMAD** : Standard (`elements.rs` + `element_vfx.rs` + genome + 2 edits wiring `lib.rs`)
> **Demande user** : « j'aimerais travailler sur [les éléments & réactions] ; fais
> une audit de nos crates, vérifie les best practices, propose des pistes » →
> piste A retenue (réaction sur l'existant) → identité « voie Gunfire — DPS soutenu ».

## Décision design (2 AskUserQuestion + benchmark genre)

1. **Direction = A « Réaction sur l'existant d'abord »** (vs C ajouter Lightning/Ice,
   vs B pivot total Gunfire). Respecte les identités d'armes story-591 (Fire=Bourrasque,
   Poison=Boucherie, Explosive=Pépin, ArmorPierce=Lenoir).
2. **Identité = « voie Gunfire — DPS soutenu »** (vs « voie PoE finisher », vs hybride
   togglable). Validée par un benchmark de 7 jeux du genre (Gunfire Reborn, Genshin,
   Warframe, Borderlands 3, RoR2, PoE, DRG), claims re-vérifiés sur sources indépendantes :
   - Le cœur (réaction sur cible entre 2 statuts co-présents → burst AOE, détection à
     l'application) = **exactement le modèle Gunfire Reborn** (Combustion = Burning+Decay,
     200% cible / 100% dans 5 m). D1/D2/D3 parfaitement ancrés.
   - **Scaling = % du TIR déclencheur** (PAS les stacks de poison) : le scaling-par-stacks
     n'existe sur aucune réaction inter-éléments du genre (Gunfire scale sur le tir, Genshin
     sur niveau+EM). Correction du design initial.
   - **Consume = false (garde les statuts, re-pulse)** : le défaut dominant du genre est
     *keep/re-trigger* (Gunfire), c'est ce qui rend les builds élémentaires « soutenus ».

## Architecture (concept-first)

- Concept = `combat` / réaction élémentaire (couche fw + def TOML). Net = local. Script = interne.
- **Contrainte porteuse (mémoire PINNED `reference_two_health_types_combat_vs_damage`)** :
  dégâts ennemis via `forgia_combat::Health` muté directement (query `With<EnemyArchetype>`),
  JAMAIS `DamageEvent` (no-op silencieux sur les ennemis).
- **Producteur (vérité)** : section `[combustion]` de `assets/genomes/roguelite/roguelite_elements.toml`
  (hot-reload mtime, miroir EXACT de `CombustionParams::Default`) + point de déclenchement
  dans `elements::sys_apply_elements_on_hit` (lit `CombatHitEvent`, `GameSet::Effects`, frame).
- **Détection** : event-driven à l'application d'un hit élémentaire — `had_burn`/`had_poison`
  lus AVANT (composants commités) puis `now_burn = had_burn || element==Fire`,
  `now_poison = had_poison || element==Poison`. **0 scan/frame** (pattern best-practice Bevy,
  vs le tick 0.12s de Gunfire). Détone aussi au coup fatal (frappe les voisins).
- **Throttle** : `CombustionCtx.cooldowns: Local<HashMap<Entity,f32>>`, décrément+purge/frame
  (`retain`), anti-spam fire-rate (0.8 s/cible). Bundle `CombustionCtx` (SystemParam) pour
  rester sous la limite de params Bevy (système à 11 params).
- **Effet** : `combustion_damage(ev.damage, target_pct, area_pct)` (fn pure testable) →
  cible ×2.0 (200%) + voisins dans `radius` ×1.0 (100%). Buffer `buf` réutilisé (0 alloc).
- **VFX** : `CombustionEvent` (Message) → `element_vfx::sys_spawn_combustion_vfx` spawn
  un burst orange (feu, grand + lumineux) + halo vert (poison) — la fusion est lisible.
- **Sensor** : `forgia2_elements.json` → champ `combustions` ; `forgia2_element_vfx.json`
  → `combustion_bursts`.

## Fichiers touchés

- `crates/forgia-mode-roguelite/src/elements.rs` — `CombustionParams` (+ champ ElementConfig
  + Default miroir), `CombustionEvent`, `CombustionCtx`, fn pure `combustion_damage`, hook
  dans `sys_apply_elements_on_hit`, champ sensor `combustions`, 3 tests.
- `crates/forgia-mode-roguelite/src/element_vfx.rs` — `sys_spawn_combustion_vfx`, stat
  `combustion_bursts`, sensor.
- `crates/forgia-mode-roguelite/src/lib.rs` — `.add_message::<elements::CombustionEvent>()`
  + enregistrement `sys_spawn_combustion_vfx` (GameSet::Effects, run_if Roguelite).
- `assets/genomes/roguelite/roguelite_elements.toml` — section `[combustion]`.

## QA (post-impl-auto-QA — qa-lead)

- 0 Bloquant. **1 Majeur corrigé** : combustion bloquée sur `is_kill` → déplacée hors du
  guard (détone au coup fatal, comme l'AOE explosif). **1 Mineur corrigé** : guard
  `ev.damage > 0` (un tir genome damage=0 ne brûle plus cooldown/VFX).
- 2 défauts **justifiés non corrigés** :
  - Fuite HashMap cooldowns : bornée ≤ `retrigger_cooldown` (0.8 s), auto-résolue, sûre
    au recyclage d'`Entity` (la génération diffère). Impact nul.
  - Pas de health-check `combustions==0` : ne pas comboter est un état de jeu valide
    (faux positif). Le compteur exporté `combustions` suffit à l'observabilité.

## Critères d'acceptation

- [x] `cargo check -p forgia-mode-roguelite` — 0 erreur
- [x] `cargo clippy` — 0 warning dans la crate
- [x] `cargo test -p forgia-mode-roguelite` — vert (126, dont 3 nouveaux Combustion)
- [x] Genome-driven (params en TOML, Default = miroir exact), hot-reloadable
- [x] Contrainte dual-Health respectée (forgia_combat::Health, pas DamageEvent)
- [x] Sensor `combustions` exposé
- [ ] Validation runtime (tirer Feu+Poison → burst + re-pulse ; cf récap test)

## Suite possible (hors scope)

- Phase 2 : ajouter Lightning (chaîne) + Ice (slow/wet) → matrice de 6 réactions.
- Voie « PoE finisher » togglable par genome (scaling stacks + consume + radius/stack).

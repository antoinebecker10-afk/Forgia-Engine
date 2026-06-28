# Story-627 — Roguelite : pré-chauffe shaders (réduction des freezes first-use)

> **Statut** : CODE-COMPLETE (2026-06-24) — validation runtime à faire
> **Niveau BMAD** : Standard (`weapon_select.rs`)
> **Origine** : freezes signalés « combat et en me déplaçant ». Diagnostic via
> `forgia2_load_timing.json` : 45-140 ms, `cause=gpu_or_shader_compile`, `entity_delta:0`
> = compilation de pipeline à la **première fois qu'un matériau/effet est rendu**.
> Handoff : l'autre terminal (VFX Hanabi + load_timing) a commité ; je reprends la passe.

## État avant

- **Hanabi : DÉJÀ pré-chauffé** (`forgia-effects::prespawn_hanabi_dummies`, story-594) —
  les 10 effets (muzzle ×5, impact ×3, status_flame, status_poison_cloud) spawnés
  cachés au PostStartup. `status_vfx` (nouveau) réutilise ces effets → couvert.
- **Gaps restants** (StandardMaterial, `Visibility::Hidden` ne pré-chauffe PAS) :
  1. **Aperçu 3D du wizard** : despawn+respawn un GLB à chaque `‹ ›` → ré-instanciation
     de scène + 1ère compile par arme = hitch au Lobby.
  2. **4 matériaux d'élément** (`element_vfx`, unlit/blend) : créés au Startup mais
     jamais rendus avant le 1er impact → compile au 1er hit élémentaire (freeze combat).

## Ce qui est fait

1. **Aperçu spawn-once + toggle** : les 4 armes sont spawnées **une fois** à l'entrée
   du Lobby (toutes `Hidden`), le `‹ ›` ne fait que **toggle la visibilité**
   (`sys_toggle_preview_visibility`). Plus de despawn/respawn → plus de ré-instanciation
   de scène par cycle (cycle instantané). Despawn de tout à la sortie du Lobby.
2. **Pré-chauffe des 4 matériaux d'élément** : à l'entrée du Lobby, spawn de 4 sphères
   minuscules (0.03) avec les 4 mats d'élément (`ElementVfxAssets`), **occluses par
   l'arme** → rendues 1× (pipeline compile) sans être visibles. Despawn à la sortie.

## Limites (honnêteté)

- `load_timing` est une **heuristique** : `gpu_or_shader_compile` = « frame lente sans
  spawn d'entité » → peut aussi être un **upload GPU de texture** (streaming en se
  déplaçant) ou un pic CPU. La pré-chauffe ne couvre que la compile de pipeline.
- **Matériaux d'ennemis** (1ère vague) + **arène/décor** (1er rendu en tournant la
  caméra) : compilent **une fois par session**, pas couverts ici (l'arène est déjà
  « prewarmée » en étant visible au Lobby ; les ennemis nécessiteraient un rig de
  pré-rendu plus lourd). À évaluer si le ressenti persiste.

## Critères d'acceptation

- [ ] Cycler les armes au Lobby (`‹ ›`) ne provoque plus de micro-freeze (instantané).
- [ ] Le 1er impact élémentaire en combat ne freeze plus (mats pré-chauffés).
- [ ] `forgia2_load_timing.json` : moins d'événements `gpu_or_shader_compile` au Lobby/early-combat.
- [x] `cargo check` + clippy clean + 143 tests + binaire `-j 4` OK.

## Test runtime

1. `cargo run -p forgia -j 4` → Lobby → **cycle les 4 armes plusieurs fois** (`‹ ›`) :
   doit être **fluide** (plus de hitch par changement).
2. Lance une run, déclenche des **impacts élémentaires** (tire) : le 1er hit ne doit plus figer.
3. Lis `forgia2_load_timing.json` après quelques minutes → comparer le nombre de freezes.

## Suivi

- Hotspot `weapon_select.rs` (30 edits) → extraire `weapon_preview.rs`.
- Rig de pré-chauffe ennemis/arène (si freezes « en me déplaçant » persistent).

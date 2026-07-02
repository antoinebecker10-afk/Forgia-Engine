# Story-647 — VFX authored Inc.1 : Bloom + textures sur les particules

> **Statut** : IN_PROGRESS
> **Niveau BMAD** : Standard (~9 fichiers)
> **Origine** : audit VFX 2026-07-02 (`docs/audit/audit-2026-07-02-vfx-etat-et-recommandations.md`) — décision user : « pas de boules procédurales, des vrais effets comme WoW/Gunfire Reborn ».
> **Parent** : principe directeur « effets composés, jamais de primitives nues » (audit §3).

## Problème

1. **Bloom absent** : ≥ 6 systèmes VFX écrivent des couleurs émissives HDR conçues pour le bloom (muzzle tints 3.0+, tracers, sparks EMISSIVE_BOOST=3.0, hit flash 8.0) — aucune caméra n'a `Hdr`+`Bloom`. Tout le glow est invisible.
2. **Zéro texture** : les 10 effets hanabi (`WeaponVfxEffects`) sont des particules unies → « boules procédurales ». Le code attend déjà des textures (`weapon_vfx/mod.rs:256` « when textures are available Phase 3b »).

## Incréments

### Inc.1a — Bloom (P0-1 audit)

- `Hdr` (marker `bevy::render::view::Hdr`) **à la création** de la FpsCamera (`forgia-player/src/lib.rs` children![]) — jamais post-hoc (leçon cyber_city 2026-06-16, warn render_sensor).
- Genes `roguelite_bloom_enabled` (default 1.0) + `roguelite_bloom_intensity` (default 0.15, clamp 0–0.5) dans `roguelite_render.toml` (pattern flat-genes local, hot-reload 1Hz).
- Attach/detach `Bloom` (bevy::post_process) sur les Camera3d **avec Hdr** hors ViewmodelCamera — pattern SSAO existant (`render_quality.rs`), OnEnter force-apply, OnExit detach.
- Capteur `forgia2_render_fx.json` : `bloom_enabled/bloom_attached/bloom_intensity` + health warn si enabled mais 0 attached.
- Tests purs : parse genes, severity.

### Inc.1b — Textures Kenney (CC0)

- Télécharger Kenney Particle Pack (CC0 vérifié, audit §4) ; extraire un subset (~10 PNG 512² : flare, spark, smoke, flame, scorch, star, twirl) vers `assets/textures/vfx/` + fichier LICENSE.
- Brancher via l'API texture hanabi 0.18 (texture slot + `ParticleTextureModifier` + `EffectMaterial` sur l'entité) sur les 10 effets `WeaponVfxEffects` : muzzle (core flash→flare, sparks→spark, smoke→smoke, heat_glow→light, forward flash→flare), impact (sparks→spark, dust→smoke, flash→flare), status (flame→flame, poison_cloud→smoke).
- Textures blanches teintées par les gradients HDR existants (une texture = 3 éléments).

## Acceptance criteria

- [x] `Hdr` posé à la création de la FpsCamera (pas post-hoc) — warn render_sensor à vérifier runtime
- [ ] Bloom toggleable en data (hot-reload) + visible : muzzle/tracers/hit-flash glowent en run — **à valider runtime par user**
- [x] Capteur render_fx expose l'état bloom + health check (bloom_enabled/attached/intensity + warn dédié)
- [x] 9 effets hanabi actifs texturés + 1 dormant préparé (slot "color" + ParticleTextureModifier + EffectMaterial : muzzle×5, impact×2, status×2 ; `impact_flash` reste commenté depuis story-432 — asset texturé, réactivation = décision perf hors scope) — finding QA Mineur traité
- [x] LICENSE CC0 Kenney committée à côté des PNG (`assets/textures/vfx/kenney/LICENSE-CC0-Kenney.txt`)
- [x] `cargo check` OK (3 crates) ; clippy 0 erreur, 0 warning introduit (7 pré-existants hors scope) ; 257 tests verts (dont 2 nouveaux bloom)
- [x] Récap test runtime fourni (règle in-game-test-recap)

> Note multi-terminal : implémentée pendant que l'autre terminal refactorait waves/sensor/hud (story-643) — zéro fichier partagé, compile validée après la pose de son refactor.

## Fichiers

- `crates/forgia-player/src/lib.rs` (Hdr caméra)
- `crates/forgia-mode-roguelite/src/render_quality.rs` (+TOML `assets/genomes/roguelite/roguelite_render.toml`)
- `crates/forgia-effects/src/weapon_vfx/{mod,muzzle,impact,status}.rs` + `lib.rs` (EffectMaterial)
- `assets/textures/vfx/` (nouveaux PNG + LICENSE)

## Incident runtime 2026-07-02 — écran noir hub FORGE (résolu)

**Symptôme** : fond noir dans l'onglet FORGE du hub (monde invisible), onglets ARMES/ENCLUME OK.
**Diagnostic** (capteurs `forgia2_render.json` + `forgia2_toon.json`, faux suspects écartés en live via kill-switches data bloom/toon) : l'onglet FORGE est le seul à forcer le viewmodel visible → la **ViewmodelCamera (LDR)** empilée sur la **FpsCamera (Hdr, story-647)** écrase la sortie monde — en Bevy, des caméras empilées sur la même fenêtre doivent partager le même réglage HDR.
**Fix** : `Hdr` aussi sur la ViewmodelCamera à sa création (`forgia-viewmodel/src/vm_camera.rs`, commentaire croisé).
**Leçon** : toute nouvelle caméra qui composite sur la fenêtre DOIT porter `Hdr` désormais (FpsCamera est HDR). Le render_sensor warne déjà sur Hdr/Bloom actifs — il ne détecte pas le mismatch entre caméras : candidat health-check futur (`hdr_mismatch`).
**Effet secondaire attendu** : le tonemapping (TonyMcMapface) s'applique désormais réellement (pipeline HDR) → la balance couleur du monde peut différer de la baseline LDR (rendu plus « filmique »/saturé). À juger par le user ; retouche éventuelle = fog/ambient/grading en data (hot-reload), pas dans le code.

## Verdict runtime final 2026-07-02 soir — HDR RETIRÉ, bloom différé

Après le fix ViewmodelCamera, deux problèmes runtime restants imputables au pipeline HDR :

1. **Teinte rouge saturée** (validé user "les couleurs ça va pas") : les 6 couches chaudes de l'arène Volcanic (soleil ambre 8k lux + fill + ambiante (1.0,0.45,0.22)×300 + brume rouge + halo + grading chaud) étaient calibrées SUR l'écrêtage LDR à 1.0 qui délavait leur cumul. En HDR le ratio réel s'exprime → rouge profond partout. Une passe de recalibrage data à chaud n'a pas suffi (le problème est systémique, pas un slider).
2. **Ghosting Text2d** (chiffres de dégâts qui "restent à l'écran") : la MenuCamera2d (3e caméra, egui/Text2d) était restée LDR → mismatch de format sur la fenêtre → buffer non nettoyé.

**Décision** : `Hdr` retiré des 2 caméras, `roguelite_bloom_enabled=0.0`. **Les textures Kenney restent livrées et actives** (indépendantes du HDR).

**Story suiveuse requise — « Calibration HDR »** (pré-requis au bloom) :

- [ ] `Hdr` sur LES TROIS caméras (Fps + Viewmodel + MenuCamera2d) — jamais partiel
- [ ] Recalibrer la pile chaude Volcanic pour tonemapping réel (sun/fill `forgia-stage:676`, ambiante `atmosphere.rs:29` — à exposer en genome, fog, grading)
- [ ] Health check `hdr_mismatch` dans render_sensor (caméras fenêtre avec réglages HDR divergents = warn)
- [ ] Session de calibration AVEC le user (hot-reload), pas de valeurs à l'aveugle

## Suite (hors scope, stories suivantes)

- Recette impact feu 7 couches complète (flipbook + ring mesh + décal scorch) — Inc.2
- Paliers hitstop / knockback / chime weakspot (P0-3/4/5 audit)
- Aura StatusShock + visuel Miasma

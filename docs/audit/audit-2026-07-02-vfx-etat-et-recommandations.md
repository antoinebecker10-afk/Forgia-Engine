# Audit VFX & Game-Feel — État + Recommandations « kill ultra satisfaisant »

> **Date** : 2026-07-02 · **Méthode** : workflow multi-agents (audit rendu complet + 3 recherches web vérifiées) + greps ciblés.
> **Objectif** : que tirer, toucher, tuer et se déplacer dans le Roguelite soit viscéralement satisfaisant.
> ⚠️ 4 audits code coupés par la limite de session — trous comblés par vérification directe (hitstop/FOV punch/mort ennemi confirmés).

---

## 1. Verdict TL;DR

**Les primitives de juice existent presque toutes et sont branchées — mais le multiplicateur n°1 (bloom) est ABSENT, la mort d'ennemi est une simple anim+despawn, et l'audio est muet.** Le jeu tourne à ~50 % du punch déjà codé :

1. 🔴 **P0 — Bloom inexistant** : ≥ 6 systèmes VFX écrivent des couleurs émissives HDR *conçues pour le bloom* (muzzle « blooms hard » `muzzle.rs:16`, tracers glow 2-4 HDR, sparks élémentaires EMISSIVE_BOOST=3.0, ultimes, champignons, hit flash blanc 8.0 HDR `combat_juice.rs:130`) — et **aucune caméra n'a Hdr+Bloom** (`forgia-player/src/lib.rs:372-381`). Tout le glow du jeu est invisible.
2. 🔴 **P0 — La mort d'ennemi n'est pas un événement** : clip `Death_A` joué (`enemy_anim.rs:211`) puis despawn. Pas de burst, pas de debris, pas de permanence, pas de récompense physique.
3. 🟠 **P1 — Toon possiblement OFF en run** : capteur `forgia2_toon.json` = « attached_cameras=0, post-process invisible » sur 2 sessions indépendantes. 5 min de vérif runtime tranchent.
4. 🟠 **P1 — Régressions V1 non portées** : chromatic aberration au tir = TODO (`combat_juice.rs:208`), outline Sobel OFF.
5. Synergie avec le rapport gunfire-like 2026-07-02 : **armes muettes** (90 barks écrits, 0 audio) — le son est la moitié du kill satisfaisant.

---

## 2. État des lieux

### ✅ Ce qui existe et fonctionne (ne pas retoucher)

| Système | Localisation | Notes |
| --- | --- | --- |
| Hitstop genome-driven | `forgia-juice-lib/hit_stop.rs`, wired via `ForgiaCombatPlugin` (`forgia-combat/src/lib.rs:137`) | dur 0.05 s / speed 0.05 par arme (`forgia-fps/src/lib.rs:1144-1147`) |
| FOV punch par arme | `forgia-juice-lib/fov_punch.rs`, wired `forgia-fps/src/lib.rs:466` | `FovPunchImpulse` émis au tir |
| CameraTrauma (shake) + HitFlash ennemis | `forgia-combat/src/lib.rs:148,199-200` ; `HitFlashTimer` `forgia-fps/src/lib.rs:1100` | flash blanc émissif 8.0 HDR — punch réel bridé par l'absence de bloom |
| Screen flash | `forgia-juice-screen-flash`, wired `forgia-game/src/lib.rs:93` | capteur forgia2_combat.sources |
| Lights dynamiques VFX | muzzle 2k-8k lm/50-80 ms, impact 1.5k-5k/40 ms (`weapon_vfx/mod.rs:231-305`), caps 64/96 élémentaires/ultimes | design borné anti-casse-Atmosphère — exemplaire |
| bevy_hanabi + prespawn warmup | `forgia-game/src/lib.rs:79` + `forgia-effects/src/lib.rs:64` | anti-trap shader-compile-lazy respecté |
| Color grading rose crypts, fog volcanique, tonemapping user, MSAA | `color_grading.rs:296`, `atmosphere.rs:32-93`, `pause_menu.rs:283-320` | hot-reload + capteurs — gouvernance exemplaire |
| Anim squelettique ennemis (95 clips KayKit) | `enemy_anim.rs` (story-636) | `Death_A` joué sur `BotState::Dead` |

### ❌ Trous / dettes

| Gap | Sévérité | Détail |
| --- | --- | --- |
| Bloom absent | **P0** | Jamais inséré ; le « bloom » forgia-postprocess est un stub passthrough. Leçon connue : Hdr+Bloom posés APRÈS coup cassent la passe (cyber_city 2026-06-16) → les poser **à la création** de la FpsCamera |
| Mort = anim + despawn sec | **P0** | Aucune séquence pop/debris/permanence, aucune récompense physique |
| Toon peut-être non appliqué | **P1** | Warn capteur ×2 sessions — vérifier `attached_cameras==1` en combat |
| Chromatic aberration au tir (V1) | **P1** | TODO `combat_juice.rs:204-210` ; la native `bevy::post_process::ChromaticAberration` existe en 0.18, inutilisée |
| Outline Sobel OFF | **P1** | Crash dual-pass FullscreenMaterial connu → fusionner Sobel **dans** toon.wgsl (1 seule passe), pas re-brancher la 2e |
| Damage numbers | **P1** | Inexistants (killfeed/kill_popup egui seulement) |
| Knockback par hit | **P1** | Pas d'impulse directionnelle sur CombatHitEvent |
| SSAO off en data | P2 | Conflit toon+SSAO (arène blanchie, 2026-06-30) à résoudre avant |
| Viewmodel exclu du toon | P2 | Mismatch stylistique au centre de l'écran (story-618) |
| 43 stubs post-process trompeurs | P2 | Seuls toon/outline.wgsl sont réels — ne pas « activer » un stub |
| Zéro marge frame | P2 | 16,57 ms moyen (audit perf 2026-07-01) → séquencer avec story-643, mesurer chaque ajout |

---

## 3. Recommandations — ordre coût/impact

### Principe directeur (décision user 2026-07-02) — des effets COMPOSÉS, jamais des primitives nues

> « Je ne veux pas des boules procédurales, je veux des vrais effets comme dans World of Warcraft ou Gunfire Reborn. »

Un VFX WoW/GR n'est jamais UN émetteur : c'est une **recette de 5-7 couches**, chacune texturée. Anatomie d'un impact feu façon Gunfire Reborn :

1. **Flash core** — billboard texture *flare* (Kenney), 2 frames, couleur HDR (bloom) ;
2. **Explosion flipbook** — sprite-sheet animée (`FlipbookModifier`), ~300 ms — c'est la couche qui « déplie » l'explosion au lieu d'une sphère qui grossit ;
3. **Étincelles** — burst hanabi, texture *spark* étirée dans le sens de la vélocité ;
4. **Fumée** — puffs texturés lents, teintés, qui traînent 1-2 s ;
5. **Anneau de choc** — **mesh** ring/quad avec texture + scale-up 200 ms (voir mesh-FX ci-dessous) ;
6. **Lumière** — PointLight bref (existe déjà ✅) ;
7. **Permanence** — décal scorch.

**La technique manquante n°1 (signature WoW) : les mesh-FX à texture défilante.** Les flammes en cône, anneaux de choc, runes au sol, beams = des **meshes simples** (cône/cylindre/ring/quad) avec un matériau custom qui fait **défiler la texture (UV panning) + érosion alpha** — pas des particules. En Bevy 0.18 : un petit WGSL via `ExtendedMaterial<StandardMaterial, _>` (uniform `time` → offset UV + seuil d'érosion), UN matériau réutilisable pour cône de flammes/ring/beam/rune. hanabi sait aussi rendre ses particules avec un mesh custom (`EffectAsset::mesh`, ≥ 0.13) pour les éclats.

**Règle de production** : chaque effet du jeu (muzzle, impact, statut, réaction, mort, ultime) se refait comme une *recette nommée* (couches + textures + durées dans un genome), et plus jamais comme un émetteur nu. Les textures viennent du §4 (Kenney = base blanche teintable, CGHEVEN/Unity Labs = flipbooks des gros moments, Kalponic = flipbooks peints toon).
**Pré-requis absolu : le bloom (P0-1)** — sans lui, même des textures magnifiques resteront ternes ; c'est lui qui donne le « magique » WoW.

### P0 « le week-end qui change le feel » (chaque item ≈ 1 session)

1. **Bloom natif derrière toggle data** — LE plus gros ROI du projet.
   `Hdr` + `Bloom` (preset NATURAL, intensity ~0.15) **dans le `children![]` de spawn** de la FpsCamera (`forgia-player/src/lib.rs:372-381`), jamais post-hoc. Gène `roguelite_bloom_enabled` dans `roguelite_render.toml` + champ dans `forgia2_render_fx.json` (recette garde-fou SSAO existante). Bloom natif tourne AVANT tonemapping → zéro conflit node_edges avec le toon. Débloque d'un coup muzzle/tracers/sparks/ultimes/champignons/hit-flash déjà écrits.
2. **Vérifier le toon runtime (5 min)** — lire `forgia2_toon.json` PENDANT un combat. Si `attached_cameras=0` : bug d'attach (timing OnEnter vs spawn caméra), tout le look cartoon est silencieusement OFF.
3. **Paliers de hitstop** — le hitstop existe mais uniforme (0.05 s). Différencier : hit normal 0-33 ms, crit/weakspot 50-80 ms, **kill 80-150 ms**, multi-kill 200+ ms (réfs SF2/ULTRAKILL/Vlambeer). Valeurs dans genome, `Time<Virtual>` (UI reste sur Real — règle anti-trap déjà en place).
4. **Knockback impulse par hit** — `ExternalImpulse` (Rapier) le long du tir sur `CombatHitEvent`, magnitude = `knockback_factor` par arme (`viewmodel_arena.toml`) × damage ; kill = ×3 + composante verticale +0.3 (« pop »). Boucherie doit PROJETER, Pépin picoter. Appliquer côté FixedUpdate avant `PhysicsSet::SyncBackend`.
5. **Chime weakspot + thump kill** (couplé au chantier voix gibberish déjà P0) — un son unique réservé au weakspot (référence directe Gunfire Reborn : chiffre jaune + ding), un son grave distinct au kill, pitch randomisé ±5 %. bevy_kira_audio déjà en stack.

### P1 « le kill devient un événement »

- **6. Séquence de mort en 4 temps** (template unique, décliné par archétype) :
  - *Anticipation* (~100 ms) : hitstop kill + hit flash prolongé 100 ms + scale punch 1.0→1.12→1.0 ;
  - *Pop* : **burst** hanabi world-space (jamais rate continu, rayon × skeleton_scale — pattern mémoire 2026-06-24b) + impulse dans la direction du tir ;
  - *Debris* : 3-4 meshes low-poly `RigidBody::Dynamic` TTL 4 s ;
  - *Permanence* : décal couleur élément au sol (brûlure/flaque/arc) TTL 10-20 s cap 40, corps qui reste 10-15 s (freeze AnimationPlayer sur la dernière frame, `RigidBody::Fixed`, fade) — l'arène raconte le combat.
- **7. Dissolve shader à la mort** : `ExtendedMaterial<StandardMaterial, DissolveExtension>` (noise + seuil + lisière émissive couleur de l'élément tueur). Exemple public : rust-adventure/bevy-examples (vérifier LICENSE). ⚠️ Piège connu : matériaux GLB KayKit **partagés** → cloner par entité mourante (pattern enemy_anim).
- **8. Damage numbers colorés par élément** : billboards world-space au point d'impact, spawn au frame exact, crit jaune 150-200 % plus lent, float 0.8 s ease-out, agrégation anti-spam. S'appuie sur l'infra nameplate 3D (story-644). Communique en même temps le système de réactions (Combustion/Miasma/Surcharge).
- **9. Chromatic aberration au tir** (port V1) : native 0.18, spike intensity 1-2 frames + decay expo. Idem pulse sur dégât reçu/low-HP.
- **10. Trails ribbons hanabi** (`Attribute::RIBBON_ID`, déjà payé dans la 0.18) : traînées projectiles Bourrasque, trail de dash. + **GPU spawn events** (`EmitSpawnEventModifier`/`EffectParent`) : particules mourantes qui engendrent étincelles/braises, 100 % GPU — parfait pour les réactions élémentaires.
- **11. Textures sur les particules** : les effets actuels semblent majoritairement des billboards sans texture → brancher les sprites Kenney (§4) via `ParticleTextureModifier` + teinte Gradient HDR > 1.0 (une texture blanche sert les 3 éléments). Explosions → `FlipbookModifier { sprite_grid_size }`.
- **12. Muzzle light couleur élément** : la light existe déjà — la teinter par l'élément de l'arme (orange/vert/bleu) : chaque tir peint le décor toon à sa couleur.

### P2 « spectacle & meta »

- **13. Orbes d'Âmes physiques aimantées** (ULTRAKILL « kill = fontaine de récompense ») : 6-10 sphères émissives couleur élément éjectées en arc à la mort, aimant `ConformToSphereModifier` (ou logique CPU) sous 4-6 m, TTL 5 s — les Âmes deviennent kinesthésiques au lieu d'un incrément silencieux.
- **14. Slow-mo « Zed Time »** : multi-kill (3+ en 1 s) ou dernier de la vague → `set_relative_speed(0.2-0.3)` 0.8-1.5 s + preset grading « killcam » (désaturation sauf couleur de l'élément). Pity timer en genome.
- **15. Flinch/stagger gradient DOOM** : vérifier si un clip « Hit » existe dans les GLB KayKit (règle : inspecter le GLB AVANT du procédural) ; seuil stagger (hit > 25 % PV max) → highlight + ×1.5 dégâts 1 s + drop bonus.
- **16. Décals d'impact natifs** : `bevy::pbr::decal::ForwardDecal` (exige DepthPrepass sur la caméra) — impacts hitscan, pool + fade, cap 128. En 0.18 : ClusteredDecal = 1 texture couleur (les 4-textures/1024 décals = 0.19).
- **17. Outline** : fusion Sobel dans toon.wgsl (une passe) derrière `outline_enabled` existant. **SSAO** : reprendre le conflit toon+SSAO ensuite.

### Gouvernance (obligatoire, règles projet)

- **`config/genomes/roguelite_gamefeel.toml`** = source unique : hitstop_ms par palier, trauma par event, flash_ms, scale_punch, knockback_factor/arme, slowmo, caps corps/décals. Hot-reload.
- **`forgia2_gamefeel.json`** : counts hitstop/flash/knockback/slowmo par minute + health « GAMEFEEL ZERO » si combat actif et 0 feedback émis.
- **Perf** : frame déjà à 16,57 ms → chaque passe/effet ajouté se mesure au capteur ; les fixes faibles du plan perf (mip cap textures, run_if chaînes IA) financent le coût GPU du bloom. Séquencer avec story-643.
- ⚠️ **Piège fullscreen** : tout nouvel effet plein-écran custom se COMBINE dans la passe toon (2 passes FullscreenMaterial = crash wgpu, mémoire 2026-06-26).
- ⚠️ **bevy_firework** : si jamais utilisé (particules CPU lisibles côté gameplay), SANS feature `physics` (tire avian3d ; deux moteurs physiques interdits).
- bevy_trauma_shake 0.7 = compat 0.18 confirmée mais conçu 2D → préférer le port interne du pattern trauma (~80 LOC, crate `noise` déjà en dép, offset rotationnel APRÈS l'input look, jamais `Projection.fov`).

---

## 4. Assets gratuits à télécharger

**✅ = URL + licence vérifiées par agent. ⚠️ = coupé par la limite de session — vérifier la licence au téléchargement (30 s).**

| Source | Licence | Contenu / usage Forgia |
| --- | --- | --- |
| ✅ [Kenney — Particle Pack](https://kenney.nl/assets/particle-pack) | CC0 | **LA base** : 80 sprites 512² blancs (smoke, flame, flare, magic, muzzle flash, scorch, spark, star...). Teinte HDR par gradient → une texture sert feu/poison/électrique. Les scorch = décals de brûlure |
| ✅ [Kenney — Smoke Particles](https://kenney.nl/assets/smoke-particles) | CC0 | 70 puffs cartoon — traînées, dashes, spawns. Colle au rendu toon |
| ✅ [Kenney — Blaster Kit](https://kenney.nl/assets/blaster-kit) | CC0 | 40 meshes low-poly sci-fi (projectiles, cibles) pour projectiles physiques Bourrasque/boules de feu |
| ⚠️ [OGA — Explosion sprite atlas](https://opengameart.org/content/explosion-particles-sprite-atlas) | CC0 annoncé | Flipbook 3×3 512² prêt pour `FlipbookModifier` tel quel |
| ⚠️ [Unity Labs — Free VFX flipbooks](https://unity.com/blog/engine-platform/free-vfx-image-sequences-flipbooks) | CC0 annoncé (page 403 aux bots — vérifier dans le zip) | Flipbooks Houdini AAA (smoke, fire, explosions) — pour les gros moments (boss). Réalise → tinter/posterizer |
| ⚠️ [CGHEVEN — Flipbooks](https://cgheven.com/assets/flipbooks) | CC0 site-wide | ~25 flipbooks + 19 packs explosions, sans compte. Downscale 1024 max |
| ⚠️ [CodeManu — Free VFX Pack](https://codemanu.itch.io/vfx-free-pack) | **traiter comme CC-BY 4.0** (metadata itch) | 22 effets pixel-art (hits, kabooms, electric shield) — feedback de procs élémentaires. Sampling nearest |
| ⚠️ [unTied Games — 5 Pixel Explosions](https://untiedgames.itch.io/five-free-pixel-explosions) | Gratuit, attribution, pas de revente | Explosions 60 fps très lisibles (sous-échantillonner 1/2) |
| ⚠️ [Kalponic — Free Stylized Sprite VFX](https://kalponic-studio.itch.io/free-stylized-sprite-vfx) | CC-BY 4.0 | Le seul pack nativement TOON/anime de la liste (15 flipbooks) |
| ⚠️ [OGA — Bullet Decal](https://opengameart.org/content/bullet-decal) | CC0 annoncé | bullet_hole.png 512² → ForwardDecal direct |
| ⚠️ [OGA — Bloodsplatter Animation](https://opengameart.org/content/bloodsplatter-and-bloodsplash-animation) | CC0 rapporté, à confirmer | Splats stylisés → recolorer rose/violet (palette + rating) |
| ✅ [Poly Pizza (filtre CC0)](https://poly.pizza/search/CC0) | CC0 via filtre (vérifier par modèle) | Meshes d'effets stylisés (éclairs, flammes cartoon, crystals) |
| [ambientCG](https://ambientcg.com/) | CC0 site-wide | Masques height/opacity pour décals — extraire le canal, jamais photoréaliste tel quel |
| [Quaternius](https://quaternius.com/) | CC0 | Crystals/shards = debris et pickups assortis à KayKit |

**Écartés** : Unity Asset Store « free » (EULA Unity-only), blackthornprod/Fellor (licence invérifiable), JoesAlotofthings (introuvable), bevy_vfx_bag (mort, ^0.10), bevy_camera_shake (plafonne 0.16), bevy_enoki (2D only).

**Règle DA** : privilégier textures **blanches/grayscale teintées par gradient HDR** plutôt que des textures déjà colorées — glow bloom néon garanti + 1 texture pour 3 éléments.

---

## 5. Sources game-feel (valeurs chiffrées)

- **Hitstop** : SF2 ~166 ms/hit ; ULTRAKILL Impact Hammer 0.2-0.5 s ; Vlambeer « 1-2 frames » par tir. Kills >> hits.
- **Trauma shake** (Eiserloh GDC 2016) : trauma ∈ [0,1], +0.2 mineur/+0.5 majeur, decay linéaire, intensité = trauma², **rotationnel uniquement en FPS** (translationnel 3D = nausée), Perlin par axe.
- **Hit flash** : blanc ~50 ms par hit, 100 ms sur kill + scale punch 1.1-1.15 ease-out 100-150 ms (Hades-like).
- **Damage numbers** (GameJuice) : spawn au frame exact, crit 150-200 % plus lent, float+fade 0.6-1.2 s, plancher de lisibilité.
- **Vlambeer ~30 tricks** : le cumul de micro-effets transforme le feel — checklist par arme : muzzle flash, impact CHAQUE balle, hit anim, knockback, permanence, camera kick opposé au tir, sleep frames, bass, douilles, morts spectaculaires.
- **DOOM 2016** : gradient twitch→falter→pushback (interrompt l'IA)→stagger→finisher ; récompense d'agressivité.
- **KF2 Zed Time** : slow-mo 20 % probabiliste sur kill « cool » + pity timer.

---

## 6. Compléments (vérifiés post-rapport, 2026-07-02)

### Faits code supplémentaires

- **Zéro texture dans tout `forgia-effects`** (vérifié par grep) : tous les effets actuels sont des primitives unies (billboards/meshes sans texture). Le code **attend déjà** des textures : « Bullet hole decal will be added when textures are available (Phase 3b) » (`weapon_vfx/mod.rs:256`, `impact.rs:10`). → Télécharger le Kenney Particle Pack débloque un TODO existant, pas seulement une amélioration.
- **Miasma n'a AUCUN visuel** : `element_vfx.rs:197` — « son visuel viendra de `status_vfx` (P1) », jamais livré. Combustion/Surcharge ont un burst 2 couleurs du couple d'éléments (`sys_spawn_reaction_vfx`, `element_vfx.rs:190-240`) mais **même forme de burst** → différencier la silhouette par réaction (Combustion = nova radiale, Surcharge = arcs/éclairs, Miasma = nuage montant vert) pour la lisibilité, pas seulement la couleur. Sensor `element_vfx` déjà en place.

### La chaîne élémentaire « façon Gunfire Reborn » — état maillon par maillon

| Maillon | État | Détail |
| --- | --- | --- |
| Tir (muzzle teinté élément) | 🟡 partiel | Muzzle flash + PointLight par **arme** (`weapon_vfx/mod.rs:231`) — teinter par **élément** = reco P1-12 |
| Impact (sparks élémentaires) | ✅ existe | `element_vfx.rs` : sparks 4 couleurs (fire/poison/shock/armor_pierce) + light cap 64 — glow amputé sans bloom |
| Statut sur l'ennemi (« il brûle ») | 🟡 2/4 | `status_vfx.rs` : aura hanabi continue **Burn** (flamme) + **Poison** (nuage), suivie par frame, scale par archétype, cap, genome hot-reload — pattern anti-occlusion 2026-06-24 respecté. **StatusShock = AUCUNE aura** (l'ennemi « électrisé » est invisible alors que la vuln ×1.1 est active) ; **Miasma = rien** |
| Réaction (burst) | 🟡 partiel | Combustion/Surcharge OK mais silhouettes identiques ; Miasma absent |
| Mort élémentaire | ❌ absent | `Death_A` + despawn, zéro composante élémentaire — couvert par les recos P1 items 6-7 (dissolve lisière couleur de l'élément tueur + décal élément + burst) |

**Ajout au plan P1** : aura StatusShock (arcs électriques intermittents — burst court répété plutôt que rate continu, pattern status_vfx existant à décliner) + visuel Miasma. Le squelette `status_vfx.rs` est bon : ce sont 2 déclinaisons, pas un nouveau système.

### La « balade » — couche d'ambiance (absente du plan combat)

Le rapport couvre le combat ; se *balader* dans l'arène demande une couche ambiante dédiée, coût faible, tout hanabi + genome :

1. **Particules atmosphériques autour du joueur** : dust motes / braises / spores (selon biome d'arène), rate faible world-space dans un rayon ~15 m suivant le joueur, drift lent + `LinearDrag` — c'est ce qui rend l'air « épais » (standard Returnal/DRG).
2. **Fireflies/spores lumineuses près des champignons** (les clusters émissifs existent déjà) — quelques particules orbitantes réutilisant leurs couleurs.
3. **Faux god rays** : pas de volumétrique natif exploitable → shaft meshes additifs figés aux ouvertures (pattern classique low-cost), émissifs pour le bloom.
4. **Réactivité au passage** : petit puff de poussière aux pas/atterrissages/dash (event locomotion FixedUpdate déjà mesuré par `PlayerLocomotion`).

Gouvernance identique : gènes `roguelite_ambient.toml` + compteurs dans un sensor.

### Hors scope de ce rapport (à traiter dans leurs chantiers propres)

- **HUD/crosshair juice** (hitmarker animé, crosshair réactif) — chantier UI/egui.
- **Audio design complet** (couches onset/body/thump/tail par arme) — chantier voix/audio P0 du rapport gunfire-like.
- **Water/sky avancés, boss telegraphs dédiés** — post-ship ou stories boss.

---

*Prochaine étape suggérée : stories BMAD — P0 items 1-5 = 1 story Standard chacune (ou 1 batch), la séquence de mort (item 6-7) = 1 story Standard dédiée.*

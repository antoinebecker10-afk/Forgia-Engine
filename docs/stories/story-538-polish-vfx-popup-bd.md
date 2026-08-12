# Story-538 — Polish VFX biome + Popup BD voicelines (Mission 1.3 + lore GDD)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : aucune trace.** Ni fichier, ni capteur, ni symbole
> parmi ceux qu'elle cite n'existe dans le dépôt. Le travail n'a pas été fait.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~5 jours
> **GDD ref** : [Mission 1.3 lisibilité](../design/gdd-roguelite-v1.md#13-lisibilité-visuelle) + lore voicelines
> **Prérequis** : story-528 (FPS feel), 531-534 (armes pour voicelines per action), 535 (ennemis pour popups kill)

## Pourquoi

Gap #3 du roadmap : 0 emissive, 0 ambient particles. Cible Cult of the Lamb (4.5M ventes $90M revenue) = signature visuelle. Popup BD = EarthBound pattern, utilise les ~200 voicelines déjà écrites sans coût audio.

## Acceptance Criteria

### VFX biome Volcanic

- [ ] AC1 — Wire `weather_override: "ashfall"` (déjà nommé `roguelite_stages.toml`) → bevy_hanabi ash particles falling 80m radius, density 60 particles/m², lifetime 10s, slow drift
- [ ] AC2 — Brazier wired props (existants) émettent flame VFX loop (radius 1.5m, hanabi ParticleEffect spawn on SceneInstance Observer)
- [ ] AC3 — Post-process color grading rouge/orange Volcanic (forgia-postprocess crate, tonemapping bias)
- [ ] AC4 — Audio biome Volcanic layered : wind + lava bubbling + distant forge hammers (forgia_audio::biome extension)
- [ ] AC5 — Emissive Brazier + Bloom HDR + TonyMcMapface tonemapping (cf Cult of the Lamb signature)
- [ ] AC6 — Mushroom emissive cyan clusters ambient (canon bible "petits champignons lumineux", 5-8 par arena)

### Popup BD voicelines (EarthBound pattern)

- [ ] AC7 — System `forgia-ui-lib::voiceline_popup` NEW : spawn texte BD bulle 2D au-dessus arme/joueur sur events
- [ ] AC8 — Events triggers : weapon_kill / weapon_miss / weapon_reload / low_energy / boss_phase
- [ ] AC9 — Style cartoon : bulle blanche bord noir 3px, font cartoon (e.g. Coiny ou similar), fade-in 100ms + duration 2s + fade-out 200ms
- [ ] AC10 — Position : screen-space attaché viewmodel arme (offset Y+40px) ou bottom-center pour Maître Forgeron narrator
- [ ] AC11 — Cooldown 3s par speaker (anti-spam, garantit lisibilité)
- [ ] AC12 — Pool ~270 voicelines déjà écrites `assets/genomes/roguelite/roguelite_dialogue.toml` consommé

### Skybox + fog

- [ ] AC13 — Skybox HDR PolyHaven volcanique (asset CC0 à wire — option `pastoral_night.hdr` ou `kloofendal_misty_morning.hdr`)
- [ ] AC14 — DistanceFog biome-driven (Volcanic = orange dense >30m, Forge = neutre clear)

### Sensors

- [ ] AC15 — `forgia2_visual_polish.json` : ash particles count active, emissive materials count, popup voicelines/min displayed

## Files
- `crates/forgia-effects/src/biome_volcanic_vfx.rs` NEW
- `crates/forgia-effects/src/brazier_flames.rs` NEW
- `crates/forgia-postprocess/src/biome_color_grading.rs` NEW
- `crates/forgia-ui-lib/src/voiceline_popup.rs` NEW
- `assets/textures/skybox/volcanic_*.hdr` NEW (PolyHaven CC0)
- `assets/genomes/roguelite/roguelite_dialogue.toml` (extend si manquant per-event)

## Anti-canon
- Bulles BD française (pas "speech bubble")
- "Volcan" ton chaud cartoon (pas "hell/dark")
- Popup voicelines respectent vocab CE2 strict

## Cross-refs
- GDD V1 Mission 1.3
- Bible v1 ambiance Crypts of Anvil (lampions roses, champignons cyan)
- story-528 (FPS feel popup post-kill)
- story-531-534 (popups voicelines per arme per event)

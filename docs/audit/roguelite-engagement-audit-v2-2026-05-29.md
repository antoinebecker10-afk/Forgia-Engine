# Audit V2 — Roguelite Addictif & Beau (2026-05-29 soir)

> **Supersede** [roguelite-engagement-audit-2026-05-29.md](roguelite-engagement-audit-2026-05-29.md)
> (matin), désormais obsolète car story-558 a fermé l'essentiel de son Tier 1/2.
> Audit croisé : best practices industry 2024-2026 + recherche web fraîche
> (game juice, sound design, FPS roguelite feel, art stylisé) vs **état réel
> post-story-558** du mode Roguelite Forgia V2.

**Auteur** : Claude Opus 4.8 (1M) + sub-agent Explore (état code) + 4 WebSearch
**Scope** : `crates/forgia-mode-roguelite/` + `assets/genomes/roguelite/`
**Bible-référence** : `docs/lore/` v1 (cartoon Overwatch×Hadès×Borderlands,
cible enfants+femmes)
**Méthode** : cartographie file:line de l'état WIRED vs STUB vs ABSENT, puis
priorisation par ROI (impact joueur / effort dev).

---

## TL;DR — Le diagnostic en une phrase

> **Le jeu est muet et le tir ne « tape » pas.** La boucle méta (Souls, Coffre,
> boons, carry-over) est désormais solide. Ce qui reste est la seconde-à-seconde —
> tirer, toucher, tuer — qui manque des deux retours sensoriels les plus
> rentables : **le son** et **l'impact du tir**.

**Le sprint qui change tout** = Audio + Impact de tir livrés **ensemble**
(son + hit-flash + muzzle + hit-stop + shake). Séparés ils déçoivent ; groupés,
le tir passe de « clic » à « WHAM ». Meilleur ROI feel/jour de dev du projet.

---

## 0. Ce que story-558 a déjà fermé (ne pas re-auditer)

État WIRED-AND-RUNNING confirmé (sub-agent Explore, 2026-05-29 soir) :

- ✅ Économie Souls : earn (Tank 5 / Sniper 3 / Runner 2 / Boss 40) + spend Coffre
- ✅ Coffre du Forgeron : OpenCoffreRequest au wave clear, CoffreSession + ActiveBoons (run.rs, waves.rs:256)
- ✅ 18 boons catalogue, **9/9 effets câblés** (DamageMul, FireRateMul, HealOnKill, DamageReduction, Knockback, ChainTargets, FlatBonus…) (boons_apply.rs:45-74)
- ✅ Carry-over 25% Souls à la Defeat (run.rs:495), 100% Victory
- ✅ Break 15s + reset HP + reset stations (waves.rs:38)
- ✅ Boss enrage télégraphié : banner « ⚒ FORGE EN COLÈRE ! » + camera trauma 0.65 (hud.rs:550-672)
- ✅ Kill popups cartoon « BAM!/PIF!/KABOOM! » ease-out-back (kill_popup.rs)
- ✅ Cel-shading toon hot-reload (toon_config.rs), palette bible appliquée (enemies.rs:71-113)
- ✅ Overlays Defeat/Victory cartoon (hud.rs:162-335)

**Conclusion** : l'ossature *engagement* est bonne. Le travail restant est
**sensoriel** (feel + beau), pas structurel.

---

## 🔴 Trou #1 — Le jeu est 100% silencieux (PRIORITÉ ABSOLUE)

**État** : 0 SFX, 0 musique, 0 voix.
- `MusicState` supprimé (story-471..479) ; `sys_apply_stage_toggles` (run.rs:174-193) log la météo mais aucun consumer musique
- `forgia_audio_voicelines` API retirée ; `BarkEvent`/`ActiveBark` en stub désactivé (run.rs:270-271, hud.rs:26-27, 430-439)
- Squelette voix présent : `weapon_to_speaker` (run.rs:552-560 → pepin/bourrasque/lenoir/boucherie), genome `roguelite_dialogue.toml`

**Pourquoi #1** : étude DOOM = joueurs scorent **~2× plus avec son activé**
([SFX Engine](https://sfxengine.com/blog/why-sound-effects-matter-in-games)).
Le son est le *« système nerveux »* du retour d'action. **Un hit-stop ou screen
shake SANS son ne sert quasiment à rien** — juice visuelle et audio se renforcent
mutuellement, isolées elles tombent à plat.

**Minimum vital par ROI** :
1. SFX tir + impact + kill (3 sons) — retour le plus dense
2. SFX ramassage Souls (le « ding » récompense = feedback loop addictive directe)
3. Musique combat + transition calme pendant break 15s (rythme tension/repos)
4. 1 voix Maître Forgeron mort/victoire (squelette prêt, narrative-as-reward Hadès)

**Stack** : `bevy_kira_audio` 0.25 (déjà dans le workspace) + assets CC0
(Freesound, Kenny.nl). ⚠️ vérifier les crates audio fusionnées 2026-05-26
(`forgia-audio` module `biome`) avant de re-câbler MusicState.

---

## 🟠 Trou #2 — Le tir est plat (le kill popup masque le vide)

**Ce qui marche** : kill popups (kill_popup.rs), knockback (boons_apply.rs:132),
camera trauma sur enrage boss.

**Ce qui manque au moment du tir lui-même** :
- ❌ Muzzle flash — absent
- ❌ Tracer joueur — absent (les *ennemis* ont `tracer_emissive` enemies.rs:149, pas le joueur)
- ❌ Hit-flash ennemi (tint blanc 1-frame à l'impact — retour le plus universel des FPS)
- ❌ Hit-stop / sleep frames 0.1-0.2s à l'impact (« secret Vlambeer », *barely visible, entirely different feel*)
- ❌ Particules — bevy_hanabi prêt mais 0 intégration (gap industry #3 historique, toujours ouvert)
- ❌ Screen shake sur tir joueur (existe seulement sur enrage boss)

**Calibrage best-practice** : shake 0.1-0.3s avec easing, hit-stop ~0.2s sur
crit/kill ([itch.io juice guide](https://itch.io/blog/1059831/making-a-game-feel-juicy-with-simple-effects)).
Référents FPS : Roboquest = *« weapons feel very punchy »*, Holy Shoot = *« shotguns
boom with chunky feedback »* ([Rogueliker](https://rogueliker.com/fps-roguelikes/)).

⚠️ Nuance ([Wayline](https://www.wayline.io/blog/the-juice-problem-how-exaggerated-feedback-is-harming-game-design)) :
la juice doit servir le gameplay, pas le masquer. Ici on part de **zéro feedback
de tir** sur un gameplay sain → chaque ajout est rentable, mais garder le shake **subtil**.

---

## 🟡 Trou #3 — Identité d'arme inexistante

Les 4 armes lore (Pépin/ricochet, Bourrasque/burst, Lenoir/lifesteal+headshot,
Boucherie/cleave) sont **définies en TOML** (`roguelite_weapons.toml`) **mais le
joueur tire une AK générique** (`WeaponType::ModernAR` hérité de l'Arena).
Aucun gimmick câblé dans le pipeline combat.

C'est le **gap industry #1** ([[reference_industry_3_gaps_forgia_roguelite]]),
toujours ouvert. Sans ça : 18 boons × 1 arme générique = combinatoire plate.
Avec 4 armes à personnalité : chaque boon × arme devient une synergie
(Mark Brown GMT « Synergies are the secret to StS's fun »).

---

## 🟡 Trou #4 — La moitié de l'identité cartoon est désactivée

**Le contour Sobel est OFF** (lib.rs:121, crash wgpu « SurfaceAcquireSemaphores
still in use » — conflit render graph Toon+Outline sur les mêmes edges
Tonemapping→X→EndMainPass). Gene `roguelite_outline_enabled` existe
(roguelite_toon.toml:78-86) mais `OutlinePlugin` pas ajouté.

Or cel-shading **sans outline** = moitié du look cartoon. Un style fort *est* la
stratégie marketing indie — c'est ce qui fait pause-scroll sur Steam
([Gianty 2025](https://www.gianty.com/top-2d-art-styles-in-2025/),
[Pixune](https://pixune.com/blog/games-with-unique-art-styles/)). Cell-shading
revient en force en 2025, bold colors + snappy animation
([ThinkGamerz](https://www.thinkgamerz.com/best-game-art-styles-2025/)).

**Fix** : réordonner les node edges d'`OutlineSettings` pour ne pas entrer en
conflit avec Toon. Bloom pour pousser les emissive (Souls, stations, boss enrage)
renforcerait le punch couleur — ⚠️ **mais** Bloom+HDR exige skybox HDR d'abord
([[feedback_hdr_pipeline_needs_hdr_skybox_first]] — régression écran noir déjà vécue).

---

## 🟢 Trou #5 — Anti-frustration enfants (mid-term)

- ❌ Telegraph wind-up ennemi : Tank attaque (warmup_secs enemies.rs:49) sans
  signal visuel. Perfect Information (StS/Into the Breach) = mort attribuable à
  une décision, pas un dé caché. Crucial pour cible enfants.
- ❌ Boon preview tooltip : champ `voiceline_preview` défini (BoonDef) mais pas
  câblé en UI au moment du pick.
- ❌ Variété décor : 3 archétypes + 1 boss (tous KayKit Skeleton GLB), 8 stations
  fonctionnelles, 0 prop décoratif (29 props CC0 Inferno wirés ailleurs — cf
  session 2026-05-26 — non utilisés ici).

---

## Recommandations priorisées

| # | Action | Impact | Effort | Tier |
|---|--------|--------|--------|------|
| 1 | **SFX tir/impact/kill + ding Souls** | 🔥🔥🔥 | S | **1** |
| 2 | **Hit-flash ennemi + muzzle flash + tracer joueur** | 🔥🔥🔥 | M | **1** |
| 3 | **Hit-stop 0.2s sur kill + screen shake tir** | 🔥🔥 | S | **1** |
| 4 | **Musique combat + break** | 🔥🔥 | S | **2** |
| 5 | **Ré-activer outline Sobel** (fix render graph) | 🔥🔥 | M | **2** |
| 6 | **Voix Maître Forgeron mort/victoire** (squelette prêt) | 🔥 | S | **2** |
| 7 | **4 armes mécaniquement distinctes** (gimmicks TOML→combat) | 🔥🔥 | L | **3** |
| 8 | **Particules hanabi** (impact, mort, ramassage) | 🔥 | M | **3** |
| 9 | **Telegraph wind-up ennemi** | 🔥 | M | **3** |

**Tier 1 = story-559** (Audio + Impact de tir, livrés ensemble). Le plus gros levier.

## À NE PAS faire (rappels bible + industrie)

- ❌ Grind linéaire +5% (anti-pattern + viole bible cible enfants)
- ❌ Rallonger run > 20 min (sessions courtes = atout différenciant)
- ❌ Juice qui masque le vide gameplay — garder le shake **subtil**
- ❌ Bloom/HDR avant skybox HDR (régression écran noir déjà vécue)
- ❌ Pool boons avec trash (toujours 3 choix viables)

---

## Sources

Recherche web fraîche 2026-05-29 (4 WebSearch) :
- [SFX Engine — Why Sound Effects Matter](https://sfxengine.com/blog/why-sound-effects-matter-in-games) (étude DOOM 2× score)
- [itch.io — Making a Game Feel Juicy with Simple Effects](https://itch.io/blog/1059831/making-a-game-feel-juicy-with-simple-effects)
- [Wayline — The Juice Problem](https://www.wayline.io/blog/the-juice-problem-how-exaggerated-feedback-is-harming-game-design)
- [Rogueliker — Best First-Person Roguelites](https://rogueliker.com/fps-roguelikes/) (Roboquest/Gunfire Reborn/Holy Shoot feel)
- [Gianty — Top 2D Art Styles 2025](https://www.gianty.com/top-2d-art-styles-in-2025/)
- [ThinkGamerz — Best Game Art Styles 2025](https://www.thinkgamerz.com/best-game-art-styles-2025/)
- [Pixune — Games With Unique Art Styles](https://pixune.com/blog/games-with-unique-art-styles/)

État code interne (sub-agent Explore 2026-05-29 soir) :
- `crates/forgia-mode-roguelite/` (lib, run, waves, enemies, hud, kill_popup, stations, boons_apply, toon_config, sensor, coffre_sensor)
- `assets/genomes/roguelite/` (toon, enemies, weapons, dialogue)

Memory références :
- [[reference_industry_3_gaps_forgia_roguelite]]
- [[reference_bible_forgia_roguelite_v1]]
- [[feedback_hdr_pipeline_needs_hdr_skybox_first]]
- [[project_session_2026_05_29_story_558_complete]]

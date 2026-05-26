# Forgia Roguelite — Roadmap V1 (2026-05-26)

> Roadmap dédiée Roguelite mode après session audit + bible + industry research 3 agents.
> Cible : Steam Next Fest démo + V1 shippable. Cartoon fantasy familial.

---

## Vision (3 phrases)

> *Un méchant a volé les âmes des armes. Toi tu es un apprenti gentil. Tu les libères, elles deviennent tes amies, et elles n'arrêtent pas de parler.*

Tone : **Overwatch × Hadès × Borderlands talking guns**, cible enfants + femmes + grand public.

Bible canonique : [`docs/lore/`](./lore/) (9 fichiers, commit 1e8c6a5).

---

## État Forgia Roguelite (2026-05-26 fin de session)

### Score honnête par axe

| Axe | Score | Détail |
|---|---|---|
| Gameplay sympa | **40%** | Pipeline 3 waves + boss tourne. 4 armes parlantes définies TOML mais joueur tire AK générique. Zéro boons. |
| Univers visuel | **45%** | Lighting cartoon key+fill biome shippé (commit e031927). 29 props CC0 Inferno wirés palette. Mais 0 emissive, 0 ambient particles, 0 skybox HDR. |
| Mécaniques simples | **70%** | FPS standard accessible, mais 0 différenciation per arme. |
| Lore + voicelines | **80%** | Bible v1 + ~200 lignes FR écrites (100 in-run + 40 hub + 60 boss/events). Audio SKIP user-decided. |

### Ship-readiness V1 démo Steam Next Fest : **~35%**

---

## 3 gaps "ÉNORMES" (convergence 3 agents industry research)

### Gap #1 — Identité weapon distinctive en 30s gameplay
Hadès 6 Infernal Arms = 6 movesets. Worms 50 armes-gags. EarthBound 1995 déjà "weapon = character". Forgia : 4 personas définies, **invisibles** en gameplay.

### Gap #2 — Boons mécaniques (synergies > stats)
Isaac/Spelunky/Hadès : items qui *changent un verbe* (dash devient teleport). Forgia : **0 boon**. Aucune raison de refaire un run.

### Gap #3 — Emissive + Bloom + Particles ambient
Cult of the Lamb (4,5M copies $90M revenue), Hadès, Death's Door : palette limited high-contrast + emissive partout. Forgia : **0 emissive material wiré**, 0 particle ambient.

---

## Top 10 benchmarks all-time (étudier ces jeux)

| Jeu | Année | Leçon Forgia |
|---|---|---|
| **Doom** | 1993 | Gunfeel via VFX/knockback/contraste > stats |
| **EarthBound** | 1995 | Pop-up texte = doublage pas obligatoire |
| **Worms** | 1995+ | 50 armes-gags, weapons-as-toys |
| **Mario 64** | 1996 | Joy of movement avant features |
| **Diablo II** | 2000 | Loot = paliers d'excitation (commun/rare/légendaire) |
| **Isaac/Spelunky** | 2008-11 | Synergies > stats |
| **Hadès** | 2018-20 | Boons rewire dash/attack, 21k voicelines |
| **Cult of the Lamb** | 2022 | Réf #1 visuel cartoon famille — 4,5M ventes |
| **Vampire Survivors** | 2022 | $5 impulse, run 20-30 min |
| **Balatro** | 2024 | Poker + jokers = 2M copies en 2 mois |

---

## Plan d'action prioritaire (7 items)

### TIER 1 — Story unique "concept tangible" (BMAD Standard)

| # | Tâche | Source consensus | Effort | Impact |
|---|---|---|---|---|
| **1** | **Wire 4 armes mécaniquement distinctes** : Pépin ricochet visible, Bourrasque knockback dust, Lenoir lifesteal red mist, Boucherie cleave AOE 2m | Doom + Worms + Hadès | **L** ~1 sem | ★★★★★ |
| **2** | **6-8 Boons mécaniques data-driven TOML** (ricochet+, lifesteal, multishot, piercing, double-jump, slow-on-hit, vampire-radius, explosive). 1 boon choix entre 3 par wave (pattern Hadès) | Isaac + Hadès | **M** ~3j | ★★★★★ |
| **3** | **Pop-up texte par arme sur events** (kill/miss/reload/low_hp) → utilise les 200 voicelines déjà écrites en floating text BD | EarthBound + Borderlands sans budget audio | **S** ~1j | ★★★★ |

**Story bundle suggérée** : story-528 "Wire 4 personas as playable distinctive + Boons MVP + Pop-up text". 2-3 semaines, transforme le concept en gameplay tangible.

### TIER 2 — Polish visuel (quick wins parallèles)

| # | Tâche | Source | Effort | Impact |
|---|---|---|---|---|
| **4** | **Emissive Brazier + Bloom HDR + Tonemapping** (Camera3d hdr=true + Bloom 0.25 + TonyMcMapface + StandardMaterial emissive Brazier via Observer SceneInstance) | Cult of the Lamb signature | **S** ~1j | ★★★★ |
| **5** | **Skybox HDR PolyHaven volcanique + DistanceFog biome** | Death's Door + Hadès | **S** ~half-day | ★★★ |
| **6** | **Ashfall particles bevy_hanabi** (wire weather_override="ashfall") | Hadès ambient permanent | **S-M** | ★★★ |
| **7** | **Mushroom emissive clusters** (champignons lumineux bible cyan glow) | Slime Rancher + bible | **S** | ★★★ |

### TIER 3 — Post-MVP / V2

- Wire pipeline voicelines (TTS Replica/ElevenLabs) — *décision skip pour l'instant*
- Re-skin viewmodel 4 armes (4 GLB stylés ou couleurs distinctives)
- Custom HUD cartoon (sprites bullés non-egui)
- Meta-progression entre runs (Roboquest pattern)
- Boss phase 2 visual transformation + cinematic
- Toon shader rim-light sur ennemis KayKit Skeleton
- Story Hub Maître Forgeron jouable (dialogues entre runs visuels)

---

## Session 2026-05-26 — Commits livrés

| Commit | Contenu |
|---|---|
| `d297db7` | cleanup: delete forgia-websocket (-307 LOC, 60 crates) |
| `1e8c6a5` | docs(lore): bible v1 Forgia (9 fichiers narratifs cartoon family-friendly) |
| `998098f` | feat(crypts): inferno donjon 29 GLB CC0 + palette modules enrichie |
| `e031927` | feat(stage): lighting cartoon biome-tuned key+fill (Crypts warm amber) |
| `7df4379` | docs(lore): hub dialogues Maître Forgeron + Apprenti (~40 lignes FR) |
| `7d6e082` | docs(lore): +60 voicelines V1 in-run (stage_clear/pickup/swap/boss) |

**Total livré session** : ~200 voicelines FR + lighting cartoon + 29 props CC0 + bible canonique. Workspace propre.

---

## Coupes assumées V1

- ❌ Pas d'audio voix (TTS ou voice actor) — texte popup BD à la place
- ❌ Pas de meta-progression entre runs (Tier 3 post-MVP)
- ❌ Pas de 2e arène (Crypts of Anvil polish only)
- ❌ Pas de co-op networking lightyear (solo only V1)
- ❌ Pas de re-skin original arts (CC0 + shaders Bevy stock)

---

## Cross-refs

- [Bible v1](./lore/README.md)
- [Crypts of Anvil location](./lore/locations/crypts_of_anvil.md)
- [Personas (4 armes + Apprenti + Forgeron Noir)](./lore/personas/)
- [Voicelines in-run](../assets/genomes/roguelite/roguelite_dialogue.toml) (~270 lignes)
- [Voicelines hub](../assets/genomes/roguelite/roguelite_hub_dialogues.toml) (~135 lignes)
- [Modules palette enrichie](../assets/genomes/level_modules.toml)

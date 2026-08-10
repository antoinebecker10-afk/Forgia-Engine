# Cartographie audio complète — Roguelite (2026-08-05)

> Croisement de 3 sources de vérité : assets (`assets/audio/forgia_original/`, 30 fichiers),
> genome (`roguelite_audio.toml`, 26 slots), code (`forgia-mode-roguelite/src/audio.rs`,
> 10 systèmes). Établie le jour du vertical audio de Codex (session
> `session_2026_08_05_audio_vertical`). Pack généré par `tools/audio/generate_forgia_da.py`
> (original, reproductible, zéro sample tiers).

## 1. ✅ Existant ET câblé (26 sons — le vertical d'aujourd'hui)

| Catégorie | Sons | Événement déclencheur |
|---|---|---|
| **Tir par arme** (4) | pepin_forge, bourrasque_gale, lenoir_royal, boucherie_furnace | `WeaponFiredEvent` |
| **Combat** (4) | impact, weakspot (pitch fixe, signature), kill, hurt joueur | `CombatHitEvent` |
| **Pickups** (2) | gold_spark, soul_echo | compteurs Or/Âmes |
| **Mouvement** (4) | dash_ember, reload_start, reload_complete, weapon_switch | `DashUsedEvent`, `AmmoChanged` |
| **Boucle** (7) | boon_forged, chest_open, wave_start, wave_clear, boss_enrage, victory, defeat | `BoonAppliedEvent`, `CoffrePickedEvent`, `BossEnrageTriggeredEvent`, `EndRunEvent` |
| **Pas** (6 var.) | forge_stone_01-06 | locomotion joueur |
| **Musique** (1) | forged_destiny_loop — continue, combat==break voulu (2026-06-05) | `sys_music_update` |
| **Ambiance** (1) | forge_heart_loop | `sys_ambience_update` |

Mix : master 0.9 · musique 0.45 · sliders pause menu (`UserAudioVolumes`). Hot-reload 1 Hz.
Capteur : `forgia2_roguelite_audio.json` (validé en jeu : 2 135 SFX sur une session).

Legacy à ignorer : `assets/audio/roguelite/` (ancien pack remplacé) · `assets/audio/sources/kenney_rpg/`.

## 2. 🎙️ Infra prête, contenu à produire — les VOIX (l'USP du jeu)

Les barks ont leur pipeline (`barks.rs`, champ `audio_path` par réplique) mais
`voiced_lines_loaded: 0` : **41 pools de répliques, zéro enregistrée**. « Chaque arme
a une voix » est la promesse du site — c'est LE plus gros trou entre la promesse et le
build. 4 personnalités : Pépin (timide), Bourrasque (extravertie), Mme Lenoir
(aristocrate), Boucherie (brute). Voix FR d'abord (jeu FR), EN ensuite.

## 3. ❌ Manquant — par priorité

### P1 — le feel du combat (les systèmes vérifiés SANS hook audio)

| Système | Sons nécessaires | Source vérifiée |
|---|---|---|
| **Réactions élémentaires** | combustion (explosion en chaîne), brûlure (tick), poison (tick/fonte armure), shock, miasma | `audio.rs` : 0 réf réaction |
| **Défense tri-couche** | bouclier touché ≠ armure ≠ vie · **bruit de casse** de couche (info gameplay critique) | `defense.rs` : 0 réf audio |
| **Ultimate** | prêt (notification) · activation · boucle active | `ultimate.rs` : 0 réf audio |
| **Ennemis** | swing mêlée (télégraphe !), tir distance, mort par archétype (Runt/Tireur/Brute), spawn/arrivée de vague spatialisée, charge de l'élite | seul le hit REÇU sonne |
| **Boss** | thème/layer dédié, attaques, transition de phase (au-delà du seul enrage) | 1 son sur ~5 nécessaires |
| **Joueur** | low-HP (battement), jump/land | rien |

### P2 — la boucle et les menus

| Système | Sons nécessaires |
|---|---|
| **UI menu** | click, hover, confirm, back, erreur (« pas assez d'Âmes ») — actuellement le menu est muet |
| **Enclume** | achat de palier/upgrade (satisfaction méta) |
| **Forgeron/équipement** | drop de pièce (jingle **par rareté** — commun→mythique), équiper |
| **Trempe** | achat de palier (feedback in-run) |
| **Marchand** | achat, vente |
| **Livre** | tourner la page, début/fin de chapitre |
| **Portail** | ouverture, traversée |
| **Rounds** | palier tous les 3 rounds (distinct de wave_start) |

### P3 — ambiance et montée en gamme (post-vertical)

| Domaine | État actuel | Cible |
|---|---|---|
| **Musique** | ✅ FAIT (2026-08-05, même jour) : bande-son Suno — thème HUB au menu + **1 piste par chapitre** (`music_hub` + `[[music_chapters]]`, sélection via `SelectedChapter`, capteur `music_current`) | layers d'intensité boss en option future |
| **Ambiances** | 1 (forge) pour 6 univers | 6 — la crypte qui goutte, le vent de la cime, etc. (l'ambiance EST l'identité d'un univers) |
| **Pas** | pierre uniquement | par matériau : bois (Halles), terre (Gorges), glace (Nécropole) — dérivable du sol de `roguelite_ambiances.toml` |
| **Obstacles** | muets | whoosh marteaux/spinners (télégraphe de danger) |

## 4. Comptes et reco de production

- **Câblé** : 26 slots · **manquant P1** ≈ 20-25 SFX · **P2** ≈ 15 · **P3** ≈ 12-15 pistes/loops · **voix** : 41 pools (~150-400 lignes).
- **SFX** → pipeline existant `generate_forgia_da.py` (synthèse originale, licence propre) ou ElevenLabs SFX.
- **Musiques/ambiances** → Suno (plan payant = droits commerciaux) : 6 thèmes d'univers + boss + menu. Pas d'API publique (2026-08) : génération manuelle web, intégration (loudnorm 48 kHz, boucle) automatisée.
- **Voix** → à trancher : TTS dirigé (ElevenLabs, rapide, 4 timbres) vs comédiens (qualité, coût). L'éval commerciale 2026-08-05 classe l'audio blocker n°2 — les voix sont la partie visible de ce blocker.

## Cross-refs

- `session_2026_08_05_audio_vertical` (mémoire) — le vertical livré par Codex
- `reference_evaluation_commerciale_2026_08_05` — audio = blocker n°2
- `roguelite_ambiances.toml` — les 6 univers (source des ambiances/pas à décliner)
- `.claude/rules/observability-required.md` — tout nouveau son passe par le capteur `forgia2_roguelite_audio.json`

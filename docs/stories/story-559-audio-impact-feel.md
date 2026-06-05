# Story-559 — Audio + Impact de Tir (le moment « WHAM »)

> **Status** : IN PROGRESS — **slices A + B (audio) DONE 2026-06-04** (NON COMMITÉS), slice C (impact visuel) à venir
>
> **Slice B livré (2026-06-04)** — son de TIR propre à chaque arme : `WeaponFiredEvent`
> (forgia-combat) émis par forgia-fps (HitscanCtx.fired, anti-16-params), lu par
> audio.rs → mapping WeaponType→son (Pépin=pistolet/Bourrasque=SMG/Lenoir=sniper/
> Boucherie=pompe). 4 sons **CC-BY 3.0** (Vincent Sevedge, CZ-52/SKS/Mosin/Shotgun)
> dans `assets/audio/roguelite/weapons/` + CREDITS.md. cargo check+clippy 0 warning.
> ⚠️ Attribution CC-BY → écran crédits in-game à ajouter (TODO).
> **Scale** : Standard (≥10 fichiers, story requise, checklist post-impl)
>
> **Slice A livré (2026-06-04)** — AC1 (SFX impact/kill/hurt + ding Or/Âmes) + AC5
> (sensor `forgia2_roguelite_audio.json`) + musique combat/break. 100% dans
> `forgia-mode-roguelite` (0 édition cross-crate, orthogonal autre terminal).
> Hook via `CombatHitEvent` (read-only) + diff Resources `Souls`/`MetaSouls`.
> SFX = placeholders V1-jingle data-driven (`roguelite_audio.toml`, hot-reload).
> `cargo check`+clippy+test 0 warning, asset-load+sensor-audit verts.
> **Slice B à venir** : AC1-bis « bang » du tir (besoin `WeaponFiredEvent`
> forgia-combat/fps) + AC2 hit-flash + AC3 muzzle/tracer + AC4 hit-stop/shake +
> remplacement des placeholders par SFX CC0 punchy dédiés. AC6 voix = optionnel.
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Audit source** : [docs/audit/roguelite-engagement-audit-v2-2026-05-29.md](../audit/roguelite-engagement-audit-v2-2026-05-29.md)
> **Prédécesseur** : story-558 (économie Souls/Coffre — DONE)

---

## 1. Contexte

L'audit V2 (2026-05-29 soir, post-558) a établi que la **boucle méta est solide**
mais que la **seconde-à-seconde est sensoriellement vide** :

- **Trou #1** : le jeu est **100% silencieux** (0 SFX, 0 musique, 0 voix). Étude
  DOOM = +2× score avec son ([SFX Engine](https://sfxengine.com/blog/why-sound-effects-matter-in-games)).
- **Trou #2** : le tir est **plat** — pas de muzzle flash, pas de tracer joueur,
  pas de hit-flash ennemi, pas de hit-stop, pas de shake sur tir. Seul le kill
  popup existe (kill_popup.rs).

**Insight clé de l'audit** : son et juice visuelle se renforcent mutuellement.
Un hit-stop SANS son ne sert à rien. **Donc on livre les deux ensemble** — c'est
ça qui transforme « clic » en « WHAM ». Meilleur ROI feel/jour de dev du projet.

---

## 2. Vision

Quand le joueur tire et touche un ennemi, **un seul moment d'impact cohérent**
se déclenche : son de tir + muzzle flash + tracer cartoon → impact sur l'ennemi
qui flash blanc + son d'impact + micro hit-stop → kill = son satisfaisant +
popup (déjà là) + ramassage Souls « ding ».

Le tout reste **cartoon family-friendly** : sons punchy mais pas réalistes/violents,
flash blanc pas gore, shake subtil (pas de motion sickness pour cible enfants).

---

## 3. Acceptance Criteria

### AC1 — SFX combat câblés ✅ **OBLIGATOIRE**

- 3 SFX minimum joués via `bevy_kira_audio` : **tir**, **impact ennemi**, **kill**
- 1 SFX **ramassage Souls** (le « ding » récompense)
- Assets CC0 (Freesound/Kenny.nl) dans `assets/audio/roguelite/`
- Données-driven : volumes/handles dans genome (pas de hardcode chemin)
- ⚠️ Vérifier compat crate `forgia-audio` (fusion 2026-05-26 module `biome`) avant câblage

### AC2 — Hit-flash ennemi ✅

- À chaque `CombatHitEvent` sur un ennemi : tint blanc 1-frame (ou ~0.08s) sur sa StandardMaterial
- Retour à la couleur archétype (palette bible enemies.rs:71-113) après le flash
- 0 allocation hot path (pré-stocker la couleur d'origine, pas de clone par frame)

### AC3 — Muzzle flash + tracer joueur ✅

- Muzzle flash à la bouche de l'arme à chaque tir (sprite/emissive court, ~0.05s)
- Tracer cartoon du joueur vers l'impact (réutiliser le pattern `tracer_emissive`
  des ennemis enemies.rs:149-181, mais côté joueur)
- Couleur tracer cohérente cartoon (pas réaliste)

### AC4 — Hit-stop + screen shake sur tir ✅

- Hit-stop ~0.15-0.2s **sur kill** uniquement (pas chaque hit, sinon saccadé)
- Screen shake **subtil** sur tir joueur (trauma faible ~0.1-0.15, easing, 0.1-0.3s)
- Réutiliser `CameraTrauma` (déjà utilisé par boss enrage hud.rs:572)
- ⚠️ Calibrage cartoon : shake doux, pas de motion sickness (cible enfants)

### AC5 — Sensor observability ✅ **OBLIGATOIRE** (règle observability-required)

- `forgia2_roguelite_feel.json` écrit ~1Hz : compteurs shots_fired, hits, kills,
  sfx_played, last_hitstop_secs, shake_trauma_current
- Permet de diagnostiquer « je tire mais pas de son/flash » sans relancer

### AC6 — Voix Maître Forgeron mort/victoire (si effort le permet) ⚠️ OPTIONNEL

- Réutiliser squelette `weapon_to_speaker` (run.rs:552) + genome `roguelite_dialogue.toml`
- 1 voiceline à la Defeat, 1 à la Victory (narrative-as-reward Hadès)
- Si API audio voicelines trop cassée (supprimée story-471..479) → **SKIP honnête**,
  noter en friction log, ne pas fabriquer (no-speculative-fix)

---

## 4. Hot path check (combat = tagué `hot`)

- [ ] Hit-flash : pré-stocker couleur origine dans Component, pas de lookup/clone par frame
- [ ] SFX : pas de `load()` par tir (handles pré-chargés au OnEnter)
- [ ] Tracer : pool/despawn timer, pas spawn illimité
- [ ] Systèmes gated `run_if(in_state(GameMode::Roguelite))`
- [ ] `Changed<T>` / events, pas de scan full archetype par frame

---

## 5. Fichiers candidats (estimation Standard)

| Fichier | Rôle |
|---|---|
| `crates/forgia-mode-roguelite/src/audio.rs` (NEW) | SFX combat + handles préchargés |
| `crates/forgia-mode-roguelite/src/feel.rs` (NEW) | hit-flash + muzzle + tracer + hit-stop |
| `crates/forgia-mode-roguelite/src/feel_sensor.rs` (NEW) | sensor AC5 |
| `crates/forgia-mode-roguelite/src/lib.rs` | wire plugins/systems |
| `crates/forgia-mode-roguelite/src/enemies.rs` | couleur origine pour hit-flash |
| `crates/forgia-mode-roguelite/src/run.rs` | SFX ramassage Souls |
| `assets/genomes/roguelite/roguelite_audio.toml` (NEW) | volumes/handles data-driven |
| `assets/audio/roguelite/*.ogg` (NEW) | assets CC0 |

⚠️ **Coordination multi-terminal** : binaire stale détecté + autre terminal
possiblement actif sur `forgia-combat` / `forgia-ai-arena-bot`. **Avant 1er Edit** :
`cargo check -p forgia-game` baseline + `git diff HEAD --name-only` claim check.
Travailler dans `forgia-mode-roguelite` (orthogonal) autant que possible ;
si hit-flash exige toucher `forgia-combat`, coordonner d'abord.

---

## 6. Test in-game (récap obligatoire)

1. **Action** : lancer Roguelite, tirer sur un ennemi, le tuer, ramasser le Soul
2. **Redémarrage** : `cargo run` (modif `.rs`) ; sons/volumes TOML → Shift+F12
3. **Effet attendu** :
   - Au tir : son + muzzle flash bouche d'arme + tracer cartoon + léger shake
   - À l'impact : ennemi flash blanc bref + son d'impact
   - Au kill : son satisfaisant + popup « BAM! » (déjà là) + micro hit-stop
   - Au ramassage Soul : « ding »
4. **Sensor** : `forgia2_roguelite_feel.json` → `shots_fired > 0`, `sfx_played > 0`,
   `hits > 0`, `kills > 0`
5. **Variantes si KO** :
   - Pas de son → vérifier handles préchargés + crate `forgia-audio` compat
   - Flash invisible → augmenter durée 0.08→0.15s ou couleur émissive
   - Shake trop fort (motion sickness) → trauma 0.15→0.08
   - Hit-stop saccadé → le limiter au kill uniquement, pas chaque hit

---

## 7. Definition of Done

- [ ] AC1-AC5 livrés (AC6 optionnel/skip honnête)
- [ ] `cargo check -p forgia-mode-roguelite` + `cargo clippy` 0 warning
- [ ] Sub-agents verifier + qa-lead (post-impl auto-QA Standard+)
- [ ] Sensor `forgia2_roguelite_feel.json` ajouté au SENSOR_REGISTRY + `xtask sensor-audit` vert
- [ ] Récap in-game fourni (§6)
- [ ] Story status → DONE + `_index.md` mis à jour
- [ ] Audit V2 ligne Trou #1/#2 marquée résolue

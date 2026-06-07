# Story-564 — Câbler les 4 gimmicks d'armes (livrer l'USP)

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard→Enterprise (~6-10 fichiers — touche pipeline de tir)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 "armes qui parlent" (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 1 "le hook" — la fracture #1 de la critique GD 2026-05-29
> **Priorité GD** : **1** (l'identité du jeu, actuellement à 0%)

---

## 1. Contexte

Critique GD (game-maker + economy, 2026-05-29) : **la promesse "armes qui parlent"
est trahie.** Les 4 armes lore sont définies en genome avec gimmicks signature,
mais **les gimmicks ne sont PAS câblés** — le joueur tire les armes Arena
génériques (AR/Shotgun/Rocket). Les personas ne servent qu'au dialogue
(`run.rs:552 weapon_to_speaker`).

> *"Le pitch promet 4 façons de jouer, le jeu livre 4 skins d'AR. Tant que les 4
> gimmicks ne sont pas câblés, 'armes qui parlent' est interdit en page Steam."*

### État vérifié

- `roguelite_weapons.toml` : 35 genes, 4 armes complètes avec gimmicks
  (Pépin ricochet, Bourrasque burst+knockback, Lenoir lifesteal+headshot+scope,
  Boucherie cleave). **0 consommation Rust** (grep workspace).
- Tir actuel = `forgia-combat::EquippedWeapons`/`WeaponType` (Arena).
- `let _ = &equipped;` (`run.rs:273`) — `EquippedWeapons` lu mais inexploité.

---

## 2. Vision

En 30 secondes de tir, le joueur **reconnaît quelle arme il tient sans lire l'UI** :

- **Pépin** (pistolet timide) : les balles **ricochent visiblement** vers une cible proche (decay dégâts par rebond).
- **Bourrasque** (SMG vent) : **rafale multi-projectiles** en éventail + **knockback** qui pousse les ennemis (coup de vent).
- **Madame Lenoir** (sniper snob) : un coup lourd, **lifesteal** (halo de soin au hit), **headshot ×2.5**, **scope** zoom.
- **Boucherie** (shotgun butcher) : **cleave** — les dégâts éclaboussent les ennemis proches au point-blank.

---

## 3. Acceptance Criteria

### AC1 — Le pipeline de tir lit `roguelite_weapons.toml` ✅ **OBLIGATOIRE**

- En mode Roguelite, les stats de tir (damage, fire_rate, range, mag, reload) viennent du genome lore, pas des constantes Arena
- Choix d'archi à acter en AC1 (plan) : étendre `forgia-weapon-hitscan`/`forgia-combat` pour lire un `WeaponGenome` roguelite, OU mapper les 4 personas sur des configs distinctes. Documenter.
- Hot-reload Shift+F12

### AC2 — Gimmick Pépin : ricochet ✅
- Au hit, le projectile rebondit vers la cible ennemie la plus proche dans `pepin_gimmick_ricochet_max_dist_m` (default 8m), jusqu'à `ricochet_count` (2), dégâts × `dmg_decay` (0.7) par rebond
- **Visible** : tracer qui change de direction (lien story-559 tracer)

### AC3 — Gimmick Bourrasque : multi-burst + knockback ✅
- 1 tir = `pellets_per_burst` (4) projectiles en éventail `burst_spread_deg` (4°)
- Knockback `knockback_n` (120) sur l'ennemi touché — **poussée visible** (synergie hazard lave story-561)

### AC4 — Gimmick Lenoir : lifesteal + headshot + scope ✅
- `lifesteal_pct` (0.20) des dégâts → soin joueur (**halo de soin visible**)
- Headshot ×`headshot_mult` (2.5) via `HitZoneTag(Head)` (déjà existant story-528)
- AltFire → scope zoom FOV `scope_zoom_fov_deg` (12°)

### AC5 — Gimmick Boucherie : cleave ✅
- Au hit point-blank, dégâts AoE `cleave_dmg_pct` (0.40) dans `cleave_radius_m` (2m) autour de la cible
- **Visible** : burst de particules / flash de zone (lien story-565)

### AC6 — Barks contextuels par arme ✅
- Réutiliser `weapon_to_speaker` + `roguelite_dialogue.toml` (~270 lignes écrites) : l'arme commente (kill, reload, swap)
- Si audio voix indispo → popup BD texte (bible-aligned), pas de SKIP du feedback

### AC7 — Observability ✅ **OBLIGATOIRE**
- `forgia2_combat.json` (ou nouveau `forgia2_weapons.json`) : `current_weapon`, `ricochets_total`, `cleave_hits_total`, `lifesteal_healed_total`, `headshots_total`
- Permet de vérifier "le gimmick se déclenche-t-il ?" sans relancer

---

## 4. Hot path check (combat = `hot`, every frame)

- [ ] Ricochet/cleave : recherche cible proche = query filtrée `With<Enemy>` + distance², pas de scan full archetype, pas d'alloc
- [ ] Multi-burst : pré-calculer les directions, pas de `Vec::new()` par tir
- [ ] `exclude_sensors` + `find_named_ancestor` (pattern hitscan existant 2026-05-22)
- [ ] Cap ricochets/cleave bornés (anti-explosion combinatoire)
- [ ] Systèmes `.in_set(GameSet::Combat)` + `run_if(in_state(GameMode::Roguelite))`

---

## 5. Fichiers candidats (~6-10)

| Fichier | Rôle |
|---|---|
| `crates/forgia-mode-roguelite/src/weapons.rs` (NEW) | lire genome lore + appliquer les 4 gimmicks |
| `crates/forgia-combat/...` ou `forgia-weapon-hitscan` | hook pour stats genome-driven (⚠️ autre terminal sur forgia-combat) |
| `assets/genomes/roguelite/roguelite_weapons.toml` | déjà complet, consommer |
| `crates/forgia-mode-roguelite/src/run.rs` | exploiter `equipped` (retirer `let _ =`) + barks |
| `crates/forgia-observability/...` | sensor AC7 |

⚠️ **Coordination CRITIQUE** : `forgia-combat` édité par l'autre terminal. Préférer
implémenter les gimmicks dans `forgia-mode-roguelite` (lecture du genome + logique
post-hit) sans modifier `forgia-combat`. Baseline + claim check obligatoires.

---

## 6. Test in-game (récap obligatoire)

1. **Action** : équiper chaque arme (cycle molette), tirer sur un groupe d'ennemis.
2. **Redémarrage** : `cargo run`. Stats/gimmicks → Shift+F12.
3. **Effet attendu** :
   - Pépin → balle rebondit sur un 2e ennemi proche
   - Bourrasque → éventail de projectiles + ennemi poussé
   - Lenoir → gros dégât, soin au hit, AltFire zoome, headshot one-shot runner
   - Boucherie → au contact, les ennemis autour prennent aussi des dégâts
4. **Sensor** : `forgia2_combat.json` → `ricochets_total`/`cleave_hits_total`/`lifesteal_healed_total` incrémentent
5. **Variantes si KO** :
   - Ricochet ne part pas → augmenter `ricochet_max_dist_m` ou vérifier query cible
   - Knockback invisible → augmenter `knockback_n`
   - Cleave ne touche personne → augmenter `cleave_radius_m`

---

## 7. Definition of Done

- [ ] AC1-AC7 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] Sub-agents verifier + qa-lead (+ edge-case-hunter si Enterprise)
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] `forgia-combat` NON modifié sans coordination
- [ ] Story DONE + ROADMAP Phase 1 mise à jour
- [ ] **Débloque la mention "armes qui parlent" en comm** (gate client interne)

---

## 8. Coupes assumées
- ❌ Re-skin viewmodel 4 GLB distincts (Tier 3 — couleurs/persona suffisent V1)
- ❌ Movesets complets type Hadès (V1 = gimmick signature, pas 6 attaques/arme)
- ❌ Plus de 4 armes (les 4 lore suffisent V1)

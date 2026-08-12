# Story-566 — Recalibrage économie (débloquer les 18 boons)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_boons.json`, fichier `boons.rs`, symbole `OpenCoffreRequest`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (~4-6 fichiers — surtout genome + constantes)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : fracture #2 (économie) de la critique GD 2026-05-29
> **Priorité GD** : **3** (effort S, débloque les 18 boons + légendaires d'un coup)
> **Source audit** : economy-designer 2026-05-29 (chiffré)

---

## 1. Contexte — diagnostic chiffré (economy-designer)

- **Budget run** : 115 souls bruts MAIS le Coffre n'ouvre que **2× par run**
  (`waves.rs:229-238`, quand `current_wave < 3`) → **~2 boons achetables sur 18**
  (sous-exploitation ×8). **~48 souls de la vague 3 meurent** (aucun coffre après).
- **Légendaires inatteignables** : double-gate `tag≥3` (`LEGENDARY_TAG_THRESHOLD=3`
  `boons.rs:34`) **ET** prix 70-90, sur un budget de 67 → 5/18 boons morts.
- **Carry-over négligeable** : `DEFEAT_SOULS_CARRYOVER=0.25` (`run.rs:495`) → mort
  en V2 = **11 souls ≈ un demi-boon**.
- **Choix Common indifférenciés** : prix 20≈25 pour des effets de force inégale.
- **⚠️ 2 systèmes de drop parallèles** : observer `run.rs:307-324` (vérité actuelle,
  5/3/2/40) vs pools `roguelite_loot.toml:179` (`currency_soul_large` **non consommé**).
- **⚠️ Bug hearts double-dip** : cœur porte `Pickup{value}` (`run.rs:367`) → soigne ET donne souls.
- **5 hardcodes** violant `no-hardcode.md` à externaliser.

---

## 2. Vision

Que **les 18 boons et les 5 légendaires soient réellement atteignables**, que la
**courbe de power suive la courbe de difficulté**, et que **chaque souls compte**.
Tout data-driven, hot-reloadable pour itérer la balance au playtest.

---

## 3. Acceptance Criteria

### AC1 — 3e Coffre (débloquer la monnaie morte) ✅ **OBLIGATOIRE**
- Émettre `OpenCoffreRequest` aussi avant le boss (entrée V3) OU après V3
- Cible : **3 ouvertures/run**, budget dépensable 67 → 115 souls (~100%)

### AC2 — Externaliser les 5 hardcodes en genes ✅ **OBLIGATOIRE** (no-hardcode)
| Constante | Fichier:ligne | Gene cible | Défaut proposé |
|---|---|---|---|
| `DEFEAT_SOULS_CARRYOVER=0.25` | `run.rs:495` | `roguelite_defeat_carryover` | **0.40** |
| `LEGENDARY_TAG_THRESHOLD=3` | `boons.rs:34` | `roguelite_legendary_tag_threshold` | **2** |
| `WAVES_TOTAL=3` | `waves.rs:32` | wirer gene `roguelite_stage_count` existant | 3 |
| Souls/archetype (5/3/2/40) | `run.rs:307-324` | `roguelite_souls_<archetype>` | 5/3/2/40 |
| Coffre `count=3` | `boons.rs:248` | `roguelite_coffre_candidates` | 3 |

### AC3 — Légendaires atteignables ✅
- Seuil tag 3→2 (AC2) + 3 coffres (AC1) → un joueur focalisant un tag débloque 1 légendaire en fin de run
- Garder prix 70-90 comme **vrai sink** des souls de fin de run

### AC4 — Différencier les Common ✅
- Étaler les prix Common **15-30** selon la force réelle de l'effet (gene `souls_cost` par boon, déjà supporté) — crée un trade-off prix/effet

### AC5 — Réconcilier les 2 systèmes de drop ✅ **BLOQUANT (concept-first étape 0)**
- Décider la **vérité unique** : observer `run.rs` OU pools `roguelite_loot.toml`
- Documenter ; supprimer/wirer le système redondant (pas 2 vérités)

### AC6 — Fix bug hearts double-dip ✅
- Un cœur soigne **OU** donne des souls, pas les deux (clarifier l'intention design)
- Confirmer runtime via sensor

### AC7 — Observability ✅
- Étendre `forgia2_roguelite_state.json` / `forgia2_boons.json` : `souls_earned_run`,
  `souls_spent_run`, `souls_wasted_run` (≈0 visé), `boons_bought_run`, `coffre_opens`

---

## 4. Hot path check
- [ ] Pas de hot path lourd (économie = events, OnEnter/wave clear)
- [ ] Lecture genes = au load/hot-reload, pas par frame

---

## 5. Fichiers candidats (~4-6)

| Fichier | Rôle |
|---|---|
| `assets/genomes/roguelite/roguelite_run.toml` | nouveaux genes carry-over, threshold, souls/archetype |
| `assets/genomes/roguelite/roguelite_boons.toml` | prix Common étalés 15-30 |
| `crates/forgia-mode-roguelite/src/waves.rs` | 3e coffre + WAVES_TOTAL gene |
| `crates/forgia-rpg-data/src/boons.rs` | threshold + count en genes |
| `crates/forgia-mode-roguelite/src/run.rs` | carry-over gene + souls/archetype + fix hearts + drop reconciliation |
| `crates/forgia-observability/...` | sensor AC7 |

---

## 6. Test in-game (récap obligatoire)

1. **Action** : jouer une run complète, acheter des boons aux 3 coffres, viser un légendaire (focaliser un tag), mourir une fois.
2. **Redémarrage** : `cargo run`. Tous les genes balance → Shift+F12 (itération sans rebuild).
3. **Effet attendu** :
   - Coffre s'ouvre **3×** (après V1, avant boss, après V2)
   - On peut s'offrir ~3-4 boons/run (vs 2)
   - En focalisant un tag, un légendaire devient achetable
   - Mort en V2 → on repart avec ~18 souls (vs 11), avance ressentie
4. **Sensor** : `forgia2_roguelite_state.json::souls_wasted_run` proche de 0 ; `coffre_opens=3`
5. **Variantes si KO** :
   - Souls encore gaspillés → ajouter coffre après V3
   - Légendaire encore hors d'atteinte → threshold 2→1 OU baisser prix légendaire
   - Trop facile (snowball) → remonter prix Common ou réduire souls/kill

---

## 7. Definition of Done
- [ ] AC1-AC7 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] **0 hardcode économie restant** (no-hardcode satisfait)
- [ ] AC5 drop reconciliation actée + documentée
- [ ] Sub-agents verifier + qa-lead
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] Story DONE + ROADMAP mise à jour

## 8. Coupes assumées
- ❌ Reroll payant au Coffre (story future si besoin)
- ❌ Meta-sink hub (story-569)
- ❌ Refonte complète des pools loot (juste réconcilier la currency ici)

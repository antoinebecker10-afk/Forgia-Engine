# Story-571 — Split monnaie : Or (in-run) + Souls (méta persistant)

> **Status** : ✅ DONE (2026-06-02 — validé runtime par user : 2 compteurs OR/ÂMES affichés, `souls_persistent:40` persistant sur 15 transitions, Coffre paie en Or)
> **Scale** : Standard (~5-6 fichiers, 3 crates dont 1 partagée)
> **Owner** : Claude Opus 4.8 (1M)
> **Décision canon** : user explicite 2026-06-02 — double monnaie modèle Gunfire Reborn
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Memory** : [[project_roguelite_dual_currency_or_souls]]

---

## 1. Contexte — pourquoi cette story

Le mode Roguelite n'a aujourd'hui qu'**UNE** monnaie (`forgia_rpg_data::loot_tables::Souls`)
qui fait double emploi : ramassée en run, dépensée au Coffre du Forgeron pour les
boons, **et** reportée partiellement à la mort (`DEFEAT_SOULS_CARRYOVER = 0.25`).

Décision user (canon, 2026-06-02) : passer au modèle **Gunfire Reborn à 2 monnaies** —
visible sur le screenshot de référence (gemme 127 + or 1581 en haut à droite).

Cette story **supersède** l'hypothèse mono-monnaie des drafts de l'autre terminal et
les **retargette** :
- **story-566** (recalibrage éco) → s'applique désormais à **l'Or** (économie in-run)
- **story-569** (hub méta) → s'applique aux **Souls** (sink persistant)

Cette story-571 fait **uniquement le SPLIT + le wiring minimal**. La balance fine (566)
et le hub (569) restent des follow-ups, désormais sans ambiguïté de cible.

---

## 2. Vision

| Monnaie | Rôle | Source | Sink | À la mort |
|---|---|---|---|---|
| **Or** 🪙 | Consommable in-run | Pickups (kills/loot) | Coffre du Forgeron (boons) | **Perdu à 100%** |
| **Souls** 💀 | Méta persistant | Fin de wave + bonus boss | Hub atelier (story-569, futur) | **Conservé à 100%** |

Tant que le hub (569) n'existe pas, les Souls **s'accumulent et s'affichent** au HUD
sans sink — comportement Gunfire early game assumé, pas un bug.

---

## 3. Acceptance Criteria

### AC1 — Resource `Or` in-run ✅ **OBLIGATOIRE**
- Nouvelle Resource `Or { current, total_collected }` **définie dans `forgia-mode-roguelite`**
  (crate isolée = évite de toucher `forgia-rpg-data/lib.rs`, en cours d'édition côté RPG)
- Les pickups actuellement `{value}souls` (`run.rs:307-324, 358-367`) **droppent de l'Or**

### AC2 — Coffre dépense l'Or (pas les Souls) ✅ **OBLIGATOIRE**
- Le Coffre du Forgeron débite `Or.current` à l'achat d'un boon
- `souls_cost` des boons → interprété comme **coût en Or** (pas de renommage TOML forcé V1 ; documenter)

### AC3 — Comportement à la mort ✅
- **Or perdu à 100%** (retirer le carry-over partiel `DEFEAT_SOULS_CARRYOVER` pour l'Or)
- **Souls conservés à 100%** entre les runs (persistance en mémoire ; save disque = story-569)
- Overlays Defeat/Victory (`hud.rs`) mis à jour : message cohérent (« tu gardes ◇ X Souls »)

### AC4 — Gain des Souls méta ✅ **[défaut à confirmer]**
- Souls gagnés **en fin de wave** + **bonus boss** (pas par kill — ça c'est l'Or)
- **Défaut recommandé** : `+5 Souls / wave clear`, `+25 Souls / boss` — data-driven (genome, no-hardcode)
- ⚠️ Valeurs à valider au playtest (566 fournira l'équilibrage fin)

### AC5 — HUD double compteur ✅
- Afficher **Or** + **Souls** séparément en haut-droite (`forgia-mode-roguelite/src/hud.rs`)
- Modèle Gunfire : Or (icône pièce / `FORGE_OR`) + Souls (gemme / `FORGE_TEAL`)
- Étend le `draw_souls_counter` existant

### AC6 — Observability ✅ (observability-required)
- Étendre `forgia2_roguelite_state.json` : `or_current`, `or_collected_run`, `souls_persistent`,
  `souls_earned_run`, `or_spent_run`
- `xtask sensor-audit` vert

### AC7 — no-hardcode ✅
- Montants gain Souls (AC4) + tout seuil → genes genome hot-reloadables

---

## 4. Hot path check
- [ ] Économie = événementiel (pickup, wave clear, achat Coffre) — **pas de hot path**
- [ ] Lecture genes au load/hot-reload, pas par frame
- [ ] HUD = vue egui (déjà gated `GameMode::Roguelite`)

---

## 5. Fichiers candidats (~5-6)

| Fichier | Crate | Rôle | Risque collision |
|---|---|---|---|
| `src/run.rs` | forgia-mode-roguelite | Or Resource, pickups→Or, mort (Or perdu/Souls gardés), souls fin de run | **À moi** ✅ |
| `src/waves.rs` | forgia-mode-roguelite | hook gain Souls fin de wave/boss | **À moi** ✅ |
| `src/hud.rs` | forgia-mode-roguelite | double compteur Or+Souls + overlays | **À moi** ✅ |
| `hud/coffre_forgeron.rs` | forgia-ui-lib | dépense Or au lieu de Souls | à vérifier |
| `loot_tables.rs` | **forgia-rpg-data** | Souls : retirer spend Coffre + carry-over → persist 100% | ⚠️ **partagé RPG** |
| `boons.rs` | **forgia-rpg-data** | `souls_cost` = coût Or (doc) | ⚠️ **partagé RPG** |
| genome `roguelite_*.toml` | assets | genes gain Souls (AC7) | à vérifier |
| observability | forgia-observability | sensor AC6 | à vérifier |

---

## 6. ⚠️ Coordination multi-terminal (BLOQUANT avant edit)

`forgia-rpg-data` est **partagé** (RPG + Roguelite) et l'autre terminal y travaille
(RPG : dialogue.rs, quests.rs, lib.rs modifiés au 2026-06-02).

**Stratégie d'isolation** :
1. Définir `Or` **dans `forgia-mode-roguelite`** (pas dans forgia-rpg-data) → 0 edit de `lib.rs` partagé
2. Avant tout edit de `loot_tables.rs` / `boons.rs` : `git diff HEAD --name-only` → si présents dans le diff de l'autre terminal, **STOP + coordonner**
3. Si bloqué sur forgia-rpg-data : minimiser à l'extrême (idéalement 0 ligne) en déportant la logique Souls-persist côté forgia-mode-roguelite

---

## 7. Test in-game (récap obligatoire — à fournir à la livraison)

1. **Action** : run complète, ramasser de l'Or, acheter un boon au Coffre, tuer un boss, mourir.
2. **Redémarrage** : `cargo run` (`.rs`) ; genes balance → Shift+F12.
3. **Effet attendu** : 2 compteurs HUD (Or qui monte aux pickups + descend à l'achat ; Souls qui montent en fin de wave/boss). Mort → Or = 0, Souls conservés.
4. **Sensor** : `forgia2_roguelite_state.json` → `or_current`, `souls_persistent` cohérents ; `or` tombe à 0 à la mort, `souls_persistent` inchangé.
5. **Variantes si KO** : Or non débité au Coffre → vérifier site de spend ; Souls reset à la mort → vérifier qu'on ne touche plus Souls dans le handler defeat ; pickups donnent encore des Souls → vérifier run.rs:307-367.

---

## 8. Definition of Done
- [ ] AC1-AC7 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] forgia-rpg-data touché au minimum (idéalement 0) — coordination §6 respectée
- [ ] Sub-agents verifier + qa-lead (post-impl-auto-qa)
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] Story DONE + ROADMAP + _index mis à jour

## 9. Coupes assumées
- ❌ Save/load disque des Souls → story-569
- ❌ Hub atelier (sink Souls) → story-569
- ❌ Équilibrage fin de l'Or (3 coffres, prix, légendaires) → story-566 retargettée
- ❌ Renommage `souls_cost` → `or_cost` dans les TOML (cosmétique, V2)

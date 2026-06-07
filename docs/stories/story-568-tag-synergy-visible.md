# Story-568 — Poursuite de synergie de tags visible (objectif intra-run)

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (~3-5 fichiers — surtout UI/feedback, infra existe)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : profondeur de build — critique GD 2026-05-29
> **Priorité GD** : **5** (effort S, active un levier déjà à moitié construit)
> **Dépend de** : story-566 (légendaires atteignables) + story-565 (boons perçus)

---

## 1. Contexte

Critique GD : *"Le genome a déjà les tags (fire/ricochet/chain/precision/chaos/
knockback) et la règle '3 tags identiques → légendaire'. Mais c'est INVISIBLE au
joueur. Donne un OBJECTIF intra-run."*

### État vérifié
- Tags synergie définis (`roguelite_boons.toml`) + `LEGENDARY_TAG_THRESHOLD`
  (`boons.rs:34`, passé à 2 par story-566).
- Le tracker de stacking de tags **n'a aucun feedback joueur** : on ne sait pas
  qu'on progresse vers un légendaire.

---

## 2. Vision

Transformer une mécanique cachée en **carotte de build** : le joueur voit qu'en
empilant des boons "feu", il approche de la **Tornade de Braise** (légendaire), et
choisit ses picks en conséquence. C'est ce qui fait dire *"j'hésite, je veux tester
cette synergie"* au lieu de *"je prends le plus cher"*.

---

## 3. Acceptance Criteria

### AC1 — Indicateur de progression de tags sur le HUD ✅ **OBLIGATOIRE**
- Pour chaque tag actif du joueur : compteur visible (ex "🔥 Feu ×2 / 2")
- Quand un seuil est atteint → feedback clair "LÉGENDAIRE DÉBLOQUÉ : Tornade de Braise"

### AC2 — Preview synergie au Coffre ✅
- Au moment du choix, indiquer si un boon **fait avancer une synergie** ("+1 Feu → débloque X")
- Perfect Information : le joueur choisit en connaissance de cause

### AC3 — Le légendaire débloqué devient achetable/offert ✅
- Cohérent story-566 : seuil 2 + 3 coffres → atteignable en fin de run
- Le légendaire apparaît dans l'offre du Coffre une fois le seuil franchi

### AC4 — Observability ✅
- `forgia2_boons.json` : `tag_counts` par tag, `legendaries_unlocked`, `legendaries_bought`

---

## 4. Hot path check
- [ ] Comptage tags = sur `Changed<ActiveBoons>`, pas par frame
- [ ] HUD redraw conditionnel
- [ ] Aucun nouveau scan combat

---

## 5. Fichiers candidats (~3-5)

| Fichier | Rôle |
|---|---|
| `crates/forgia-rpg-data/src/boons.rs` | exposer `tag_counts` + détection seuil |
| `crates/forgia-ui-lib/src/hud/...` | indicateur tags + preview Coffre |
| `crates/forgia-mode-roguelite/src/boons_apply.rs` | événement "légendaire débloqué" |
| `crates/forgia-observability/...` | sensor AC4 |

---

## 6. Test in-game (récap obligatoire)

1. **Action** : acheter 2 boons du même tag (ex Feu), observer le HUD et l'offre du Coffre suivant.
2. **Redémarrage** : `cargo run`.
3. **Effet attendu** :
   - HUD montre "🔥 Feu ×1", puis "×2/2" → "LÉGENDAIRE DÉBLOQUÉ"
   - Le Coffre suivant propose la Tornade de Braise
   - Au choix, preview "+1 Feu" sur les boons concernés
4. **Sensor** : `forgia2_boons.json::tag_counts.fire` monte ; `legendaries_unlocked` incrémente
5. **Variantes si KO** :
   - Indicateur illisible → simplifier (icône + nombre)
   - Légendaire n'apparaît pas → vérifier wiring seuil → pool offre (lien story-566)

---

## 7. Definition of Done
- [ ] AC1-AC4 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] Sub-agents verifier + qa-lead
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] Story DONE + ROADMAP mise à jour

## 8. Coupes assumées
- ❌ Anti-synergies / malus de mélange de tags (V2)
- ❌ Arbre de synergie complexe (V1 = compteur linéaire par tag)

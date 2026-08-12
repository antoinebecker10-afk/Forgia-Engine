# Story-567 — Variété de vagues + courbe d'intensité

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_roguelite_state.json`, fichier `enemies.rs`, symbole `RunSeed`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (~4-6 fichiers — réutilise les archétypes existants)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : fracture #3 (replayability) + dramaturgie — critique GD 2026-05-29
> **Priorité GD** : **4** (quick win, casse la répétition dès la run 1)

---

## 1. Contexte

Critique GD : *"8 ennemis → 12 ennemis → 1 boss. C'est une liste de courses, pas
une montée en tension. Même arène, mêmes squelettes, même rythme. Le seul pic
émotionnel en 12 min, c'est l'enrage du boss."*

### État vérifié
- Compositions fixes (`waves.rs:64-82`) : V1 = 3T/3R/2S, V2 = 4/4/4, V3 = boss+4R.
- Aucune variabilité de seed (même run à chaque fois) → **aucune raison de relancer**.
- 3 archétypes existants (Tank/Runner/Sniper) **sous-exploités** : seul le nombre change.

---

## 2. Vision

Donner à chaque vague une **saveur lisible et variable**, et créer des **pics
d'intensité** avant le boss — sans nouveau contenu (on réutilise les 3 archétypes) :

- **Saveurs de vague** : "Nuée" (full runners rapides), "Mur" (tanks lents),
  "Embuscade" (snipers + couvert), "Chaos" (mix dense).
- **Variabilité seed** : composition tirée du `RunSeed` (déterministe) → runs différentes.
- **Annonce dramaturgique** : chaque vague annoncée (banner + voiceline arme) — pédagogie + tension.
- **Mini-événement mi-run** : un pic avant le boss (vague élite, modificateur temporaire).

---

## 3. Acceptance Criteria

### AC1 — Saveurs de vague data-driven ✅ **OBLIGATOIRE**
- Schema `wave_flavor` dans genome : pondération par archétype, densité, modificateur optionnel
- ≥4 saveurs définies (Nuée / Mur / Embuscade / Chaos)
- Le nombre total d'ennemis reste calibré (cohérent story-566 budget souls)

### AC2 — Composition tirée du seed ✅
- Les saveurs des vagues sont sélectionnées via `RunSeed` (déterministe, rejouable) → variabilité run-à-run
- Garde-fou : V1 reste accessible (pas de "Mur" full-tank en ouverture pour cible enfants)

### AC3 — Annonce de vague lisible ✅
- Banner cartoon "⚔ NUÉE !" + (lien story-559) voiceline/bark arme
- Télégraphie le type qui arrive → Perfect Information, anti-frustration

### AC4 — Pic d'intensité avant le boss ✅
- 1 mini-événement (ex : vague élite à HP/vitesse boostés, ou modificateur "tout rapide 20s")
- Crée un 2e pic émotionnel en plus de l'enrage boss

### AC5 — Observability ✅
- `forgia2_roguelite_state.json` : `wave_flavor_current`, `wave_seed`, `intensity_event_active`

---

## 4. Hot path check
- [ ] Sélection saveur = OnEnter wave (events), pas par frame
- [ ] Spawn = batch existant, pas d'alloc supplémentaire
- [ ] Modificateurs de vague = champs Resource lus par les systèmes existants (pas de nouveau scan)

---

## 5. Fichiers candidats (~4-6)

| Fichier | Rôle |
|---|---|
| `assets/genomes/roguelite/roguelite_run.toml` (ou waves) | saveurs + pondérations |
| `crates/forgia-mode-roguelite/src/waves.rs` | sélection saveur via seed + compositions |
| `crates/forgia-mode-roguelite/src/hud.rs` | banner annonce vague |
| `crates/forgia-observability/...` | sensor AC5 |

⚠️ Réutilise les archétypes de `enemies.rs` — **ne pas créer de nouvel ennemi ici**
(ça c'est une story contenu ultérieure, et seulement après que le combat *se sente*).

---

## 6. Test in-game (récap obligatoire)

1. **Action** : jouer 2 runs et comparer ; observer les annonces de vague.
2. **Redémarrage** : `cargo run`. Saveurs/pondérations → Shift+F12.
3. **Effet attendu** :
   - Run A ≠ Run B (compositions différentes via seed)
   - Banner annonce le type ("NUÉE", "MUR"...)
   - Un pic d'intensité se déclenche avant le boss
4. **Sensor** : `forgia2_roguelite_state.json::wave_flavor_current` change ; `wave_seed` varie entre runs
5. **Variantes si KO** :
   - Runs trop similaires → augmenter le poids des saveurs distinctes
   - V1 trop dure → forcer une saveur douce en ouverture
   - Pic d'intensité illisible → renforcer banner + son

---

## 7. Definition of Done
- [ ] AC1-AC5 livrés (≥4 saveurs)
- [ ] `cargo check` + clippy 0 warning
- [ ] Sub-agents verifier + qa-lead
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] 0 nouvel ennemi créé (réutilisation pure)
- [ ] Story DONE + ROADMAP mise à jour

## 8. Coupes assumées
- ❌ Nouveaux archétypes d'ennemis (story contenu ultérieure)
- ❌ Génération procédurale d'arène (story-561/563)
- ❌ Plus de 3 vagues (garder run courte, cible enfants)

# Story-569 — Méta-progression hub (la boucle de retour)

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard→Enterprise (~6-9 fichiers — nouvel état hub + persistance)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : fracture #3 (replayability) — critique GD 2026-05-29
> **Priorité GD** : **6** (la colle qui fait revenir — APRÈS que les builds se sentent)
> **Dépend de** : story-564 + 565 (sinon on persiste de l'invisible) + 566 (souls sink)

---

## 1. Contexte

Critique GD + economy : *"Le carry-over 25% des souls n'est pas une raison de
revenir. Il n'y a RIEN à débloquer entre les runs. Hadès vit sur le Miroir des
Ténèbres, Isaac sur les unlocks. Forgia n'a aucun '+1' persistant → le joueur qui
meurt n'a rien gagné de durable → il ne relance pas."*

Et economy : *"70 souls reportés en Victory sans aucun sink — récompense sans usage."*

> ⚠️ **Ordre impératif** : NE PAS faire cette story avant 564+565. Une méta-progression
> qui persiste des effets invisibles ne crée aucun sentiment. Les builds doivent
> d'abord *se sentir* (fractures #1 et #2).

---

## 2. Vision

Un **hub entre les runs** (l'atelier du Maître Forgeron) où les souls accumulés
**achètent du pouvoir durable** — mais à la Hadès (choix mutuellement exclusifs),
**jamais** un grind linéaire +5% (anti-pattern bible + industrie).

Exemples d'axes persistants (1 seul axe suffit V1) :
- Débloquer une **arme de départ** (les 4 lore, déverrouillées une par une = lien fiction "libérer les âmes")
- **Atelier d'améliorations** : slots à choix exclusif (ex : "+1 charge de dash" OU "+15 HP max", pas les deux)
- Réduire le **prix des Coffres** OU augmenter le **carry-over**

---

## 3. Acceptance Criteria

### AC1 — État Hub entre les runs ✅ **OBLIGATOIRE**
- Nouvel état (ou réutiliser `RunState::Lobby`) = l'atelier du Maître Forgeron, accessible avant/après une run
- Le joueur y dépense ses souls **persistants** (sink des souls de Victory)

### AC2 — Choix mutuellement exclusifs (anti-grind) ✅
- Chaque upgrade = **un choix entre 2+ effets exclusifs** (canon Hadès Mirror), jamais +5% linéaire empilable
- ≥3 slots d'upgrade V1, data-driven (genome)

### AC3 — Déblocage des 4 armes lore ✅ (cohérent fiction)
- Les armes se débloquent une par une (souls) — "libérer les âmes des armes" (bible)
- Lien story-564 : l'arme débloquée a son gimmick câblé

### AC4 — Persistance (save/load) ✅ **OBLIGATOIRE**
- Les souls persistants + upgrades + armes débloquées **survivent à la fermeture du jeu**
- Format simple (JSON/ron), versionné
- ⚠️ V2 n'a "no save mechanism" documenté (cf MEMORY) — créer la fondation ici

### AC5 — Recalibrage souls Victory (sink réel) ✅
- Lien story-566 : les souls de Victory ont enfin un usage (hub). Ajuster le taux si besoin (100% → ce qui équilibre le hub)

### AC6 — Observability ✅
- `forgia2_meta.json` : `souls_persistent`, `upgrades_owned`, `weapons_unlocked`, `runs_total`, `wins_total`

---

## 4. Hot path check
- [ ] Hub = état hors combat, pas de hot path
- [ ] Save/load = événementiel (sur upgrade/fin de run), pas par frame
- [ ] Lecture meta = au boot/OnEnter, pas par frame

---

## 5. Fichiers candidats (~6-9)

| Fichier | Rôle |
|---|---|
| `crates/forgia-mode-roguelite/src/hub.rs` (NEW) | état atelier + UI dépense souls |
| `crates/forgia-rpg-data/src/meta.rs` (NEW) | `MetaProgress` Resource + upgrades data |
| `crates/forgia-mode-roguelite/src/save.rs` (NEW) | persistance JSON/ron |
| `assets/genomes/roguelite/roguelite_meta.toml` (NEW) | upgrades + coûts + armes unlock |
| `crates/forgia-mode-roguelite/src/run.rs` | brancher souls persistants au lieu de reset |
| `crates/forgia-observability/...` | sensor AC6 |

---

## 6. Test in-game (récap obligatoire)

1. **Action** : finir une run, aller à l'atelier, dépenser des souls (débloquer une arme / upgrade), fermer le jeu, relancer.
2. **Redémarrage** : `cargo run`. Upgrades/coûts → Shift+F12.
3. **Effet attendu** :
   - L'atelier propose des choix exclusifs + déblocage d'arme
   - Après dépense, l'effet persiste à la run suivante
   - Après fermeture/réouverture du jeu, le progrès est conservé
4. **Sensor** : `forgia2_meta.json::souls_persistent` baisse à l'achat ; `weapons_unlocked` monte ; persiste après relance
5. **Variantes si KO** :
   - Progrès perdu au relancement → vérifier save/load (chemin, sérialisation)
   - Upgrades qui empilent du linéaire → revoir AC2 (exclusivité)
   - Souls Victory toujours inutiles → vérifier le sink hub branché

---

## 7. Definition of Done
- [ ] AC1-AC6 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] Save/load testé (persiste après fermeture)
- [ ] Sub-agents verifier + qa-lead (+ edge-case-hunter si Enterprise)
- [ ] Sensor + `xtask sensor-audit` vert
- [ ] Récap in-game fourni
- [ ] **Aucun grind linéaire +5%** (AC2 — anti-pattern bible)
- [ ] Story DONE + ROADMAP mise à jour

## 8. Coupes assumées
- ❌ Système de Heat/Pacts (difficulté ascendante post-victoire) — V2
- ❌ Cosmétiques / collection — V2
- ❌ Arbre de talents complexe — V1 = quelques slots exclusifs suffisent

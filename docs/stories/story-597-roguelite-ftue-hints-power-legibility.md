# Story-597 — FTUE Roguelite : hints contextuels + première mort + lisibilité de puissance (DPS)

**Statut** : EN COURS — **Phase B incrément 1 livré** (2026-06-19) : « mort = centre de gravité ». Reste Phase A (hints), C (sensor funnel complet), D (DPS).

> **Incrément 1 (Phase B core) — fait** : nouveau `ftue.rs` (`FtueSave` persistée
> `ftue_save.toml`, séparée du shop) + `FtuePlugin` + sensor `forgia2_ftue.json`.
> `hud::draw_defeat_overlay` : 1re mort → 3 lignes pédago (Maître Forgeron) + flèche
> vers L'Enclume + Âmes **forgées cette run** (`meta.earned_run`, « toute run paie ») ;
> morts suivantes → version courte. Flag one-shot persisté, marqué au clic d'un bouton
> Defeat (L'Enclume **et** Menu → couvre les 2 sorties, fix qa BUG-597-B-01 : `OnExit`
> SubState ne fire pas sur le chemin Menu). Auto-QA : qa-lead 1 Majeur corrigé.
> Reste à faire (incréments suivants) : dialogue 3 lignes au **retour Lobby** +
> highlight visuel de l'Enclume (Phase B full), hints one-shot (A), DPS (D).

---
**(Plan d'origine ci-dessous, inchangé)**

**Statut original** : À FAIRE (préparée 2026-06-12, à implémenter plus tard)
**Niveau BMAD** : Enterprise (4 crates, ~10-12 fichiers, 4 phases livrables séparément)
**Date** : 2026-06-12
**Cible** : SHIP Roguelite — un nouveau joueur doit comprendre seul la boucle (run → mort → âmes → Enclume → plus fort) et *voir* sa puissance monter. Aujourd'hui le rythme existe (gating éléments, hub, Enclume story-591) mais il est **muet** : rien n'explique un pickup, une mort, ni l'effet d'un boon/achat.

**Recherche fondatrice** (3 rapports, 2026-06-11, `docs/research/`) :
- `research-2026-06-11-tutorial-ftue-best-practices.md` — teach-by-doing (Fan GDC 2012), ≤8 mots, just-in-time (Hodent GDC 2016), progressive disclosure, mort-professeur (Kasavin), funnel FTUE instrumenté, kid-friendly NN/g (1 notion/salle, prompts littéraux).
- `research-2026-06-11-roguelite-case-studies.md` — patterns vérifiés : chaque mort convertie en progrès visible (Gunfire SE use-it-or-lose-it, StS barre de déblocage), loot pool qui grandit (RoR2), post-mort = centre de gravité de l'onboarding.
- `research-2026-06-11-dps-stat-system-best-practices.md` — sustained DPS, fonction pure = même chaîne que le tir réel (anti-PoE tooltip), modificateurs typés + lazy recompute, deltas visibles (Achterman).

---

## Principes verrouillés (issus de la recherche — ne pas affaiblir à l'implémentation)

1. **Aucun écran/niveau « tutoriel »** — la zone 1 enseigne en jouant, les hints sont la seule couche explicite.
2. **Hint = one-shot À VIE** (persisté disque), ≤ 8 mots, vocabulaire enfant de 10 ans, voix bible v1 (Maître Forgeron), jamais bloquant.
3. **Le gating des éléments existant (`ElementUnlocks`) EST la progressive disclosure** — interdit de le court-circuiter (ex. pas de hint qui liste les 4 éléments en run 1).
4. **Le DPS affiché ne peut pas mentir** : calculé par la MÊME fonction que le dégât réel, neutre (sans élément/crit/conditionnels), golden test d'égalité obligatoire.
5. **Zéro hardcode** : textes des hints + déclencheurs + échelle DPS = TOML/genome hot-reloadable.

## Phase A — Hints contextuels one-shot

- **`roguelite_hints.toml`** (assets/genomes/roguelite/, même pattern que `roguelite_intro.toml`) : `[[hint]] id / trigger / speaker / text` (+ `text_en` si i18n story-492 actif au moment de l'impl).
- **Triggers v1** (events existants, pas de nouveau polling) : `first_pickup` (item GLB ramassé), `first_elemental_hit` (CombatHitEvent élémentaire), `first_element_unlock` (ChoiceKind portail), `first_low_hp` (<25 % la 1re fois), `first_obstacle_push` (AnimatedObstacle push).
- **Affichage** : pipeline bulle BD existante (typewriter, `forge_persona_color`) — variante courte non-bloquante (pas d'ESPACE requis, fade ~4 s).
- **Persistance** : `ftue_save.toml` via pattern story-591 (`fs`+`serde`+`toml`, `config_dir`, schéma versionné `FtueSave { version, seen: Vec<String>, timestamps }`). Fichier SÉPARÉ de `meta_shop_save.toml` (pas coupler shop et FTUE). Save événementiel (hint affiché, OnExit).

## Phase B — Script de première mort (le moment le plus important du funnel)

- Premier retour Lobby après une Defeat (flag `first_death_recap_seen` dans FtueSave) : séquence dialogue 3 lignes Maître Forgeron — enseigne la méta-boucle (« Tes âmes restent. » / « Dépense-les à l'Enclume. » / « Repars plus fort. » — à réécrire ton bible) + mise en avant visuelle de l'Enclume (highlight/flèche).
- **Jamais d'écran de défaite sec** : le récap Defeat → Lobby affiche les âmes gagnées de la run (pattern Gunfire/StS « toute run paie »).
- Runs suivantes : pas de dialogue, juste le compteur d'âmes au retour.

## Phase C — Sensor funnel `forgia2_ftue.json` (observability-required)

- Champs : `first_kill_secs`, `first_death_secs`, `first_element_unlock_secs`, `run1_completed`, `hub_first_visit_secs`, `return_run2` (bool), `hints_seen: [ids]`, `timestamp_secs`.
- Cibles directionnelles (benchmarks deltaDNA 2016, indicatif) : first_kill < 120 s, première mort « riche » < 15 min.
- Enregistrer dans le sensor-registry (gate audit story-546).

## Phase D — Lisibilité de puissance (DPS effectif)

- **`effective_dps()` fonction pure dans `forgia-combat`** : compose les MÊMES facteurs dans le MÊME ordre que la chaîne `effective_dmg` (genome arme × `PermanentPlayerMods` × boons × gimmick). ⚠️ Vérifier contre story-591 (`PermanentPlayerMods` composé AVANT overwrite dans `sys_recompute_boon_mods`) et story-582 (2 types Health — le DPS concerne le dégât SORTANT joueur, chaîne hitscan→`forgia_combat::Health` ennemis).
- **Formule affichée = sustained DPS** : `dégâts_chargeur / (temps_vider_chargeur + reload)`, neutre (sans élément/crit), arrondi entier. Élément = icône à côté, jamais plié dans le nombre.
- **Modificateurs typés** : additif intra-catégorie, multiplicatif inter-catégories (méta × boons × gimmick) — à valider contre la chaîne existante avant de figer.
- **Recalcul sur événement uniquement** (boon ramassé, achat Enclume, changement d'arme, hot-reload genome) → résultat caché en Component ; RIEN par frame (hot path).
- **3 surfaces UI** : (1) carte d'arme au ramassage/inspection : DPS actuel ; (2) delta au pickup boon : « +12 % dégâts → 145 » ~2 s ; (3) Enclume : aperçu avant→après achat.
- **Échelle** : calibrer (genome) pour des deltas ≥ 2 chiffres par upgrade (psychologie Achterman, « +1 ne procure rien »).
- **Sensor `forgia2_power.json`** : par arme — base, flats, % par catégorie, multiplicateurs, DPS final décomposé.

## Implémentation (estimation fichiers)

| Fichier | Phase | Rôle |
|---|---|---|
| `forgia-mode-roguelite/src/ftue.rs` (nouveau) | A+B+C | FtueSave, triggers→hints, script 1re mort, sensor ftue, `FtuePlugin` |
| `assets/genomes/roguelite/roguelite_hints.toml` (nouveau) | A | catalogue hints data-driven |
| `forgia-ui-lib` (bulle courte) | A | variante bulle BD non-bloquante fade |
| `forgia-mode-roguelite/hud.rs` | B | récap Defeat→Lobby : âmes de la run + highlight Enclume |
| `forgia-combat/src/dps.rs` (nouveau) | D | `effective_dps()` pure + modificateurs typés + tests |
| `forgia-fps` / chaîne hitscan | D | brancher la décomposition (mêmes facteurs) — édition minimale |
| `forgia-ui-lib/hud` | D | carte arme + delta boon |
| `forgia-mode-roguelite/meta_shop.rs` | D | aperçu avant/après Enclume |
| `forgia-mode-roguelite/sensor.rs` | C+D | `forgia2_ftue.json` + `forgia2_power.json` |

⚠️ **Multi-terminal** : `forgia-mode-roguelite` et `forgia-ui-lib` sont dans le diff non-commit d'un autre terminal au 2026-06-12. Au moment d'implémenter : standup + claim (`git diff HEAD --name-only`) obligatoires, sinon décaler.

## QA / Critères d'acceptance

- [ ] check + clippy 0 ; tests purs : sustained DPS (chargeur/reload), composition modificateurs (ordre, catégories), FtueSave round-trip, hint one-shot (2e trigger = silence)
- [ ] **Golden test anti-mensonge** : dégâts cumulés appliqués par la chaîne hitscan sur cible neutre pendant 1 cycle chargeur+reload == DPS affiché × durée (tolérance arrondi)
- [ ] Runtime A : nouvelle partie (ftue_save supprimé) → 1er pickup déclenche la bulle UNE fois (≤8 mots), relancer le jeu → plus jamais
- [ ] Runtime B : 1re Defeat → retour Lobby → dialogue 3 lignes + âmes affichées + Enclume mise en avant ; 2e Defeat → pas de dialogue
- [ ] Runtime D : ramasser boon dégâts → delta affiché + DPS carte d'arme monte ; achat Puissance Enclume → aperçu avant/après cohérent avec `forgia2_power.json`
- [ ] Sensors : `forgia2_ftue.json` (first_kill/first_death/hints_seen) + `forgia2_power.json` (décomposition) écrits et enregistrés au registry

## Récap test in-game (à servir à la livraison — règle in-game-test-recap)

1. **Action** : supprimer `ftue_save.toml` → `cargo run -p forgia-game` → Roguelite → ramasser un item, prendre un boon, mourir
2. **Redémarrage requis** (`.rs`) ; hints TOML ensuite hot-éditables
3. **Effet attendu** : bulle courte au 1er pickup ; delta « +X % → N » au boon ; au retour Lobby : 3 répliques + âmes + Enclume highlight
4. **Sensors** : `forgia2_ftue.json → first_death_secs > 0, hints_seen non vide` ; `forgia2_power.json → final_dps` qui monte après achat
5. **Variantes si KO** : hint absent → vérifier `ftue_save.toml` pas déjà peuplé + trigger event émis (log) ; DPS incohérent → comparer décomposition sensor vs achats/boons actifs ; bulle illisible → durée fade ×2 (genome)

## Hors scope (stories candidates suiveuses)

- Golem d'entraînement diégétique au Lobby entre Enclume et portail (pattern Skelly/Hades)
- Boon gratuit du Maître Forgeron après mort précoce runs 1-3 (pattern Neow's Lament, rubber-banding invisible)
- Gating spatial par éléments dans les zones (pattern runes Dead Cells) ; dépôt d'âmes au portail mi-run (pattern Collector)
- Benchmarks FTUE PC premium récents (question ouverte recherche #1)

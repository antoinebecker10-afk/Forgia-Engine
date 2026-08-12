# Story-560 — Mouvement : Sprint + Crouch + Slide (game feel BO6 accessible)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_fps_feel.json`, fichier `dash.rs`, symbole `KinematicCharacterController`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard (~3-5 fichiers, story requise, checklist post-impl)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 1 "le hook / le feel" — voir [ROADMAP_ROGUELITE.md](../ROADMAP_ROGUELITE.md)
> **Inspiration** : Call of Duty BO6 *Omnimovement* (version **accessible**, pas sweaty)

---

## 1. Contexte

Demande user (2026-05-29) : *« on pourrait pas ajouter plus de mouvement ?
Genre sprint, accroupi etc. comme dans Call of Duty BO6 ? »*

Décision de scope (AskUserQuestion) : **Sprint + Crouch + Slide** — le sweet spot
juteux ET accessible. L'omnimovement complet (slide-cancel, dive, prone) est
**écarté** : feel compétitif/sweaty qui entre en tension avec la bible
(héros doux, cible enfants+femmes).

### État vérifié du mouvement actuel (file:line)

- ⚠️ `PlayerAction::Sprint` **mappé sur ShiftLeft** (`forgia-input/src/lib.rs:89`)
  **mais NON consommé** : vitesse figée `let speed = 5.0 * speed_mul.0;`
  (`forgia-player/src/lib.rs:431`). Appuyer Shift ne fait rien aujourd'hui.
- ✅ Jump : `vertical_velocity` + grounded, KCC Rapier (`forgia-player/src/lib.rs:414` `player_movement`)
- ✅ Dash : « Pas de l'Apprenti », double-tap Espace, 2 charges (`forgia-player/src/dash.rs`, story-528)
- ❌ Crouch : absent
- ❌ Slide : absent
- Contrôleur = `KinematicCharacterController` Rapier (`forgia-player/src/lib.rs:220`)

---

## 2. Vision

Un mouvement qui **rend le tir plus fun** sans demander de skill compétitif :

- **Sprint** (Shift maintenu) : ×1.6 vitesse, accélère le rythme entre les paquets d'ennemis.
- **Crouch** (Ctrl) : capsule basse, ×0.5 vitesse, hitbox réduite — se cacher derrière un couvert, viser stable.
- **Slide** (Sprint + Ctrl) : **le moment "cool"** — glissade avec momentum qui décroît (~0.6s), hitbox basse, esquive juteuse. C'est le feel signature BO6, compris par un enfant en 2 secondes.

Cohérence bible : feel **doux + fun**, pas technique. Pas de slide-cancel, pas de dive.

---

## 3. Acceptance Criteria

### AC1 — Sprint câblé ✅ **OBLIGATOIRE** (quick win — touche déjà mappée)

- `player_movement` consomme `PlayerAction::Sprint` (held) → multiplie la vitesse horizontale
- Vitesse data-driven : `MovementTuning.sprint_mult` (default **1.6**) hot-reloadable genome (pas de hardcode `5.0`)
- Sprint **annulé** pendant le tir prolongé ? → NON (rester accessible). Sprint + tir autorisé.
- Sprint impossible en l'air ? → autorisé (garder simple), momentum conservé

### AC2 — Crouch ✅

- `PlayerAction::Crouch` ajouté à l'enum (`forgia-input`), mappé sur **ControlLeft**
- Held → capsule KCC réduite (hauteur ~0.5×), vitesse ×0.5 (`crouch_speed_mult` genome)
- Relâché → capsule restaurée, MAIS bloquer le stand-up si plafond au-dessus (raycast court anti-clip) — ou skip si pas de plafonds bas dans l'arène (noter le choix)
- Crouch réduit la hitbox (le collider bas est la vérité — pas de double système)

### AC3 — Slide ✅ **le cœur du game feel**

- Déclencheur : Sprint actif **+** Crouch pressé (front montant) **+** grounded **+** vitesse horizontale > seuil
- Impulse momentum initial (légère survitesse ~×1.8 du sprint) qui **décroît** linéairement sur `slide_duration_secs` (default **0.6s**)
- Pendant le slide : capsule basse (comme crouch), contrôle directionnel réduit (on garde la trajectoire), pas de re-trigger avant fin + petit cooldown
- Fin de slide → état crouch si Ctrl maintenu, sinon stand
- Tous les params data-driven : `slide_duration_secs`, `slide_impulse_mult`, `slide_min_speed`, `slide_cooldown_secs`
- Event `SlideUsedEvent` (canon pour brancher SFX/VFX plus tard, miroir `DashUsedEvent`)

### AC4 — Cohabitation avec Dash + Jump ✅

- Slide n'écrase pas le dash (dash = double-tap Espace, slide = Sprint+Ctrl) — vérifier l'ordre des systèmes (dash `pre-pass` puis slide, ou priorité explicite)
- Jump pendant slide → autorisé (saut conserve un peu de momentum slide), PAS de slide-cancel exploit (cooldown empêche le spam)
- Documenter l'ordre dans GameSet (Input → Movement)

### AC5 — Observability ✅ **OBLIGATOIRE** (observability-required)

- Étendre `forgia2_fps_feel.json` (story-528, déjà 14e sensor canonical) OU `forgia2_player.json` avec :
  `is_sprinting`, `is_crouching`, `is_sliding`, `slide_uses_total`, `current_speed`
- Permet de diagnostiquer "Shift fait rien" / "slide se déclenche pas" sans relancer

---

## 4. Hot path check (mouvement = tagué `hot`, every frame)

- [ ] `player_movement` reste 1 query filtrée `With<Player>` (single entity)
- [ ] 0 allocation dans la closure (réutiliser état dans Components, pas de Vec/HashMap par frame)
- [ ] Slide/crouch = champs sur un Component `MovementState` (comme `DashState`), pas de Resource globale par-frame
- [ ] Raycast anti-clip crouch stand-up : **1 raycast** max conditionnel (seulement au relâchement Ctrl), pas chaque frame
- [ ] Systèmes `.in_set(GameSet::Movement)` + `run_if(in_state(...))` si applicable
- [ ] Pas de modif de la capsule KCC chaque frame si l'état n'a pas changé (`Changed`-like guard local)

---

## 5. Fichiers candidats (estimation Standard ~4-5)

| Fichier | Rôle |
|---|---|
| `crates/forgia-input/src/lib.rs` | `PlayerAction::Crouch` + binding ControlLeft |
| `crates/forgia-player/src/movement_state.rs` (NEW) | `MovementTuning` (genome) + `MovementState` Component + sprint/crouch/slide logic + `SlideUsedEvent` |
| `crates/forgia-player/src/lib.rs` | `player_movement` consomme sprint/crouch/slide ; capsule height ; wire plugin |
| `assets/genomes/.../movement.toml` (NEW ou étendre existant) | tuning data-driven (sprint_mult, crouch_speed_mult, slide_*) |
| `crates/forgia-observability/src/...` | AC5 champs sensor |

⚠️ **Coordination multi-terminal** : `forgia-player` + `forgia-input` ne sont PAS
listées comme touchées par l'autre terminal (qui est sur `forgia-combat` /
`forgia-ai-arena-bot`) → crates orthogonales, **safe**. Confirmer au standup avant
1er Edit (`git diff HEAD --name-only`). Baseline `cargo check -p forgia-player`.

---

## 6. Test in-game (récap obligatoire)

1. **Action** : lancer Roguelite. Tenir **Shift** en bougeant (sprint). Tenir **Ctrl** (crouch). **Shift+Ctrl** en courant (slide).
2. **Redémarrage** : `cargo run` (modif `.rs`). Tuning `movement.toml` → Shift+F12 hot-reload.
3. **Effet attendu** :
   - Shift → on va **visiblement plus vite** (×1.6)
   - Ctrl → le perso **s'abaisse** (caméra descend), va plus lentement
   - Shift+Ctrl en course → **glissade** : on file vite puis on décélère sur ~0.6s, caméra basse
4. **Sensor** : `forgia2_fps_feel.json` (ou `forgia2_player.json`) → `is_sprinting:true` sous Shift, `is_sliding:true` pendant la glissade, `slide_uses_total` incrémente
5. **Variantes si KO** :
   - Sprint ne change rien → vérifier que `Sprint` est bien `held` consommé (pas `just_pressed`) + `sprint_mult` lu
   - Slide ne part pas → baisser `slide_min_speed` ou vérifier condition grounded + front montant Crouch
   - Slide trop court/long → ajuster `slide_duration_secs` (0.6 → 0.4 ou 0.9)
   - Caméra ne descend pas en crouch → vérifier que la hauteur capsule **ET** l'offset caméra changent

---

## 7. Definition of Done

- [ ] AC1-AC5 livrés
- [ ] `cargo check -p forgia-player -p forgia-input` + `cargo clippy` 0 warning
- [ ] Sub-agents verifier + qa-lead (post-impl auto-QA Standard+)
- [ ] Tests purs : `sprint_mult` appliqué, slide decay déterministe, front montant crouch
- [ ] Sensor champs ajoutés + `xtask sensor-audit` vert
- [ ] Récap in-game fourni (§6)
- [ ] Pas de hardcode vitesse (tout genome-driven, anti-pattern `5.0` éliminé)
- [ ] Story status → DONE + ROADMAP_ROGUELITE.md Phase 1 mise à jour

---

## 8. Coupes assumées (vs BO6 omnimovement complet)

- ❌ Slide-cancel (exploit compétitif, anti-bible)
- ❌ Dive / plongeon (trop technique pour cible)
- ❌ Prone supine (tir sur le dos — hardcore)
- ❌ Sprint omnidirectionnel à pleine vitesse en arrière (garder pénalité arrière classique pour lisibilité)

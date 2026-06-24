# Story-610 — Roguelite : Commerçant d'arène (sink in-run Or + Âmes)

> **Statut** : CODE-COMPLETE + QA OK (2026-06-20) — reste validation runtime
> **Niveau BMAD** : Standard (module `merchant.rs` + genome + sensor + 2 edits wiring)
> **Demande user** : « ajoute des choses à acheter avec les âmes, comme un
> commerçant dans l'arène qui lui ne bouge jamais de place. Il vendra plein de
> choses notamment des munitions. »

## Décision design (AskUserQuestion 2026-06-20)

**Monnaie = « les deux »** (choix user, contre ma reco initiale « Or seul ») :
- **Consommables in-run → Or** (`Gold` = `forgia_rpg_data::loot_tables::Souls`).
  L'Or n'avait AUCUN sink in-run → le commerçant le débouche enfin.
- **Items premium → Âmes** (`MetaSouls`). Dépense conservée à la mort (flush
  `OnEnter(Defeat/Victory)` réconcilie `souls_total = meta.current`), donc cohérent
  avec la persistance L'Enclume.

Rationale économie (story-571) : 2 monnaies distinctes. Le commerçant in-run ≠
L'Enclume (Lobby, upgrades permanents). La FTUE (story-597 Phase B) enseigne
« Âmes → Enclume » ; ici les Âmes premium restent un choix tactique exceptionnel,
pas le sink principal.

## Architecture (concept-first)

- Concept = `economy` / `shop` (couche fw + def TOML). Net = local. Script = interne.
- **Producteur (vérité)** : `MerchantCatalogue` (Resource, miroir du genome
  `assets/genomes/roguelite/roguelite_merchant.toml`, chargé au boot).
- **Monnaies (vérité)** : `Gold.current` (in-run, run.rs) + `MetaSouls.current` (méta).
- **Consommateurs** :
  - `merchant::sys_merchant_input` (frame, `GameSet::UI`, run_if InRun/Boss) — touches
    1-N = achat si proximité + monnaie suffisante.
  - `merchant::draw_merchant_panel` (EguiPrimaryContextPass) — panneau quand proche.
  - `merchant::sys_merchant_proximity` (frame, `GameSet::Input`) — flag `near_player`.
  - `run::obs_roguelite_player_death` (observer) — consomme `ReviveTokens` au lieu
    d'émettre `Defeat` si token > 0.
- **Effets réutilisés** : refill mag+réserve (miroir `stations.rs`), heal (miroir
  `stations.rs` heal via `forgia_damage::Health`).
- **Sensor** : `forgia2_merchant.json` (1Hz) — soldes Or/Âmes, near_player, achats,
  revives accordés + health check (catalogue vide).

## Catalogue v1 (data-driven)

| id | nom | monnaie | coût | effet |
|---|---|---|---|---|
| `ammo` | Munitions | Or | 30 | recharge réserve arme en main |
| `ammo_all` | Réassort complet | Or | 70 | recharge TOUTES les armes |
| `heal` | Soin | Or | 40 | +40 PV |
| `revive` | Second souffle | Âmes | 15 | survit à la prochaine mort (1×) |

## Piège 2 types de Health — RÉSOLU par traçage code

Mémoire `reference_two_health_types_combat_vs_damage` alertait sur combat vs damage
Health. **Traçage** : le joueur spawn avec `DamageHealth::new(100)` =
`forgia_damage::Health` (forgia-player/lib.rs:294) ; `DeathEvent` est émis par
`forgia-damage` quand CE Health atteint 0 (forgia-damage/lib.rs:7). Donc :
- **Heal** (item) mute `forgia_damage::Health` — correct (= Health réel du joueur,
  miroir `stations.rs` heal + `meta_shop.rs` revive Lobby).
- **Revive** restaure `forgia_damage::Health` à max — la source de vérité du
  `DeathEvent`. Pas de restauration combat::Health (le joueur n'en porte pas).

Validation runtime obligatoire (revive prévient-il l'écran Defeat ?).

## Critères d'acceptation

- [ ] Commerçant visible à une position FIXE de l'arène (ne bouge jamais), solide.
- [ ] À proximité (≤ rayon) : panneau shop affiché (egui), items + coûts + monnaie.
- [ ] Touche `1..N` achète l'item si monnaie suffisante (Or OU Âmes selon item).
- [ ] `Munitions` recharge la réserve de l'arme en main ; `Réassort complet` toutes.
- [ ] `Soin` rend +40 PV (clamp max).
- [ ] `Second souffle` (Âmes) : à la mort suivante, le joueur survit (HP restauré),
      pas d'écran Defeat, token consommé.
- [ ] Catalogue chargé depuis `roguelite_merchant.toml` (fallback Default si KO).
- [ ] `forgia2_merchant.json` écrit 1Hz + health check catalogue vide.
- [x] `cargo check -p forgia-mode-roguelite` + clippy 0 warning (1 warning pré-existant `forgia-core:58` hors scope) + 7 tests merchant verts.

## Auto-QA (post-impl, 2026-06-20)

- **verifier** : run interrompu (output tronqué) ; vérif manuelle effectuée à la
  place (gates verts + traçage Health).
- **qa-lead** : 5 findings. **1 réfuté, 4 corrigés** :
  - **#1 (Majeur, RÉFUTÉ)** « revive ne restaure pas `forgia_combat::Health` » —
    FAUX : le joueur ne porte QUE `forgia_damage::Health` (spawn forgia-player/
    lib.rs:294 ; combat::Health = ennemis-only, vérifié sur tout le crate). Aucune
    re-mort possible. Commentaire merchant.rs corrigé (disait « les deux Health »).
  - **#2 (corrigé)** `run_if(GameMode::Roguelite)` ajouté sur `draw_merchant_panel`.
  - **#3 (corrigé)** `last_purchase` échappé (`\`/`"`) avant le JSON sensor.
  - **#4 (corrigé)** hint « Touches 1-N » dynamique (`cat.items.len()`).
  - **#5 (corrigé)** `warn!` si achat Munitions sans slot d'arme (débit sans effet).
  - **Validés sans bug** : underflow u32 gardé, pas de double-achat (just_pressed),
    gate Lobby/InRun OK, reset revive OnEnter(Lobby), `single()` safe, genome↔Default
    cohérent.

## Itération visuelle (2026-06-20 — « vraie étale comme WoW »)

Pilier primitif → **GLB en assets locaux** (zéro téléchargement, choix user CC0) :
- **Étale** = `building_market_red.gltf` (pack KayKit Medieval Hexagon, CC0 — `.gltf`
  + `.bin` + `hexagons_medieval.png` présents).
- **PNJ vendeur** = `Gobli.glb` (gobelin marchand), debout derrière le comptoir,
  face au joueur.
- Machinerie réutilisée de `boss_portal.rs` (story-603) : `sys_calibrate_merchant`
  (scale AABB → taille cible) + `sys_ground_merchant` (pose la base au sol). Helpers
  AABB dupliqués (note : extraire un crate partagé si 3e consommateur de placement GLB).
- Parent = ancre `Merchant` + **collider cuboïde bloquant** indépendant du chargement
  async → l'interaction marche même si le GLB tarde/échoue.
- **Tunables** (réglage à l'œil après 1er rendu, je ne vois pas le mesh) :
  `STALL_TARGET_SIZE`, `GOBLI_TARGET_SIZE`, `GOBLI_LOCAL_OFFSET`, `STALL_YAW_OFFSET`,
  `STALL_COLLIDER_HALF`, `MERCHANT_POS`.

> Risque connu : les bâtiments KayKit Hexagon peuvent embarquer une base hexagonale
> → à valider en jeu (sinon ajuster le grounding / choisir une autre pièce).

## Suivi (hors scope v1)

- Modèle GLB du commerçant (primitive émissive en v1).
- i-frames courts post-revive (v1 = restauration HP brute).
- Externaliser coûts en gene genome hot-reload Shift+F12 (v1 = TOML + miroir, comme meta_shop).

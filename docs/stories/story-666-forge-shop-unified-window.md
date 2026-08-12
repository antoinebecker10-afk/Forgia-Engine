# Story-666 — Fenêtre unique du Forgeron : achat souris + Trempe + dialogue E

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `forge_shop.rs`, symbole `CursorOptions`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> Renumérotée story-659 → 666 le 2026-07-26 : collision d'ID avec
> story-659-projectiles-elementaires-pow-retire (antérieure, conservée).
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS
**Niveau BMAD** : Enterprise (nouveau module UI + refonte merchant/trempe + input/curseur + anim, 1 crate)
**Crate** : `forgia-mode-roguelite`
**Date** : 2026-07-02

## Demande Antoine (screenshot)

Les deux HUD (marchand bas-centre + Trempe bas-gauche) sont mal placés. Vouloir :
1. **UNE fenêtre centrée** : « LE FORGERON ITINÉRANT » à **gauche**, « TREMPE » à **droite**.
2. **Achat/trempe à la SOURIS** (boutons cliquables), plus seulement clavier.
3. **Ouverture par E** : on « parle » au gobelin (proximité → prompt → E ouvre/ferme), au lieu
   d'un panneau auto par proximité.
4. **Animer le gobelin** marchand.

## Contraintes vérifiées (concept-first)

- **Gobli.glb = mesh statique, AUCUN rig ni clip** (inspection glTF) → anim SKELETTIQUE
  impossible. Substitut : **anim procédurale** (bob + léger balancement idle + « hop » de scale
  à l'ouverture du dialogue). Chemin d'upgrade = asset gobelin riggé (futur).
- **Curseur + input modal** déjà résolus : `forgia_input::InputBlockers{block_look,block_fire}`
  (block_fire lu par `forgia-fps::fire_allowed`) + `CursorOptions`. Le Coffre de boons utilise
  ce pattern. `block_fire` n'est écrit qu'aux transitions (pause/end) → le shop le possède
  pendant l'ouverture sans conflit ; `sys_break_look_override` early-return en jeu normal →
  le shop possède le curseur sans conflit.

## Design

- **Nouveau module `forge_shop.rs`** (host UI) : `ForgeShopOpen` (Resource), E toggle (près du
  marchand), gate curseur/flags (ouvert → curseur libre + block_look/fire ; fermé → restaure +
  regrab), **fenêtre centrée 2 colonnes** (boutons egui), prompt « [E] Parler au forgeron »
  (proche + fermé), anim procédurale du gobelin.
- **Événementiel** (scalabilité « Events > mutation ») : clic bouton OU touche 1-4 → `PurchaseRequest`
  ; clic Tremper → `TemperRequest`. Systèmes `sys_apply_purchase` / `sys_apply_temper` appliquent
  (l'ancien `sys_merchant_input` devient producteur d'events, gaté sur shop ouvert).
- **merchant.rs** : marque le gobelin `MerchantVendor`, retire `draw_merchant_panel`, refactor
  achat en event-driven, garde spawn/proximité/sensor/effets.
- **trempe.rs** : expose `try_temper(...)`, retire son panneau bas-gauche + son input E (la
  trempe passe par le bouton de la fenêtre) ; garde state/config/sync/sensor.

## Acceptance criteria

- [ ] Runtime : une seule fenêtre centrée, Forgeron gauche / Trempe droite ; plus de
      panneaux bas-*.
- [ ] Runtime : près du marchand → prompt « [E] Parler ». E ouvre → curseur libre, clic
      boutons achète/trempe (sans tirer), E/éloignement referme → curseur re-grab + tir réactivé.
- [x] Achat clavier 1-4 émet `PurchaseRequest` (gaté sur fenêtre ouverte) ; boutons souris
      aussi → un seul `sys_apply_purchase`. Trempe → `TemperRequest` → `sys_apply_temper`.
- [x] Gobelin `MerchantVendor` animé (rotation seule : sway + respiration + hop à l'ouverture)
      — sans conflit avec la calibration (scale/Y).
- [x] `cargo check` vert + clippy 0 warning fichiers touchés + 267 tests verts.

# Story-624 — Hub d'accueil Roguelite à onglets (P2)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `hub.rs`, symbole `HubPlugin`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : 🚧 IN_PROGRESS (code livré, compile + clippy + tests verts — ⏳ validation runtime user)
**Niveau BMAD** : Standard (5 fichiers)
**Design** : `docs/design/roguelite-home-hub-proposal-2026-06-26.md` §4.3, §6 (P2)
**Dépend de** : story menu P1 (commit 5eb95c8, fix curseur Lobby) — sans curseur libre, hub non cliquable.

## Objectif

Regrouper les 3 panneaux du Lobby (Forgeron / Armes / Enclume), aujourd'hui affichés
TOUS en même temps à des coins différents, en un **hub à onglets** : un seul panneau
visible à la fois + bandeau (Âmes + nom + niveau placeholder) + bouton cliquable
**LANCER LA RUN** (équivalent souris de la touche Entrée).

## Livré

| # | Item | Fichier |
|---|------|---------|
| 1 | `HubTab` resource {Forge, Armes, Enclume, Talents} + reset OnEnter(Lobby) | `crates/forgia-mode-roguelite/src/hub.rs` (nouveau) |
| 2 | `draw_hub_chrome` : bandeau Âmes (haut-D), forgeron+Niv (haut-G), onglets (haut-C), LANCER (bas) | hub.rs |
| 3 | Run-conditions `on_forge_tab/on_armes_tab/on_enclume_tab` | hub.rs |
| 4 | Gating panneaux par onglet (`.run_if`) | weapon_select.rs:803, meta_shop.rs:661, identity.rs:335 |
| 5 | Wire `HubPlugin` | lib.rs (`pub mod hub;` + `add_plugins`) |
| 6 | Onglet Talents = placeholder (arbres = P5) | hub.rs |

- Aperçu 3D de l'arme : **toujours visible** (hero du hub), seuls les panneaux egui sont gatés.
- LANCER décalé bas-droite sur ARMES (le sélecteur d'arme occupe le bas-centre).
- Réutilise le thème Forge existant (`FORGE_OR`, `FORGE_PANEL`, `cartoon_btn`, `display_text`).
- Chemins clavier intacts (Entrée = lancer, ←/→ = arme, 1-7 = Enclume) : coexistent avec la souris.

## Validation

- `cargo check -p forgia-mode-roguelite` : ✅ 0 erreur.
- `cargo clippy -p forgia-mode-roguelite` : ✅ 0 warning (sur fichiers touchés ; reste 1 warning pré-existant forgia-core hors scope).
- `cargo test -p forgia-mode-roguelite --lib hub::` : ✅ 2 passed.

## Acceptance criteria (⏳ runtime)

- [ ] Au Lobby : 4 onglets cliquables FORGE / ARMES / ENCLUME / TALENTS.
- [ ] Un seul panneau visible à la fois selon l'onglet.
- [ ] Bandeau Âmes (haut-D) + nom forgeron (haut-G) corrects.
- [ ] Bouton LANCER LA RUN démarre la run (= Entrée).
- [ ] Aucun chevauchement gênant des panneaux.

## P2.1 — Relayout hub + nettoyage HUD (demande user 2026-06-26)

| # | Demande | Livré |
|---|---|---|
| 1 | Arme 3D plus grande | `PREVIEW_TARGET` 0.55→0.95, `PREVIEW_Y` -0.05→0.15 (vitrine relevée) — weapon_select.rs |
| 2 | Titre par onglet | `HubTab::title()` (TON FORGERON / CHOISIS TON ARME / L'ENCLUME DES ÂMES / TALENTS) sous les onglets — hub.rs ; titres internes retirés (weapon_select, meta_shop) |
| 3 | Onglet ↔ titre sans chevauchement | onglets y=12, titre y=64 — hub.rs |
| 4 | Stats arme centrées | 3 zones (titre/stats-gauche/sélecteur-bas) fusionnées en 1 carte centrée (CENTER_BOTTOM -98) + sélecteur ◄ ► intégré — weapon_select.rs |
| 5 | Forge + Enclume centrés | identity + meta_shop → CENTER_CENTER |
| 6 | LANCER bas-centre tous onglets | hub.rs (suppression du cas spécial ARMES) |
| 7 | Retirer le HUD in-run au Lobby | resource partagée `forgia_core::GameplayHudVisible` (false au Lobby) → gate ammo/PV/énergie/confiance/wave (forgia-ui-lib) + **ViewmodelCamera coupée** (forgia-viewmodel) ; le preview 3D (layer 0) reste |
| 8 | Barre d'XP sous le nom | `ProgressBar` placeholder dans le bandeau — hub.rs |
| 9 | Arme 3D incluse dans le panneau stats | preview agrandi centré juste au-dessus de la carte (bloc « vitrine ») |

**Architecture clé** : `GameplayHudVisible` vit dans forgia-core (DAG-libre) → les crates HUD partagées la lisent sans cycle vers forgia-mode-roguelite. Piloté OnEnter/OnExit(RunState::Lobby). Le viewmodel est masqué en coupant sa caméra (layer 1) — le monde + l'aperçu d'arme (layer 0) restent visibles.

Fichiers : forgia-core/lib.rs · forgia-ui-lib/{hud/player_hp,hud/energy,hud/confidence,hud/wave_counter,hud_ammo/mod} · forgia-viewmodel/vm_camera · forgia-mode-roguelite/{hub,weapon_select,meta_shop,identity,lib}. `cargo check` + clippy 0 warning (4 crates).

## Reste (design home-hub)

- **P3** : wizard Nouvelle partie (nom → style → récap).
- **P4-P6** : niveau/XP réel (remplace placeholder Niv. 1 + barre XP), arbres de talents (onglet Talents), Givre/Éclair + combos.

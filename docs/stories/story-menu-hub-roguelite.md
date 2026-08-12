# Story — Menu-titre devient le hub roguelite complet

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `hub.rs`, symbole `MetaSouls`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : DRAFT / plan (2026-07-21). Décision user validée : « le menu-titre devient le hub complet ».
> **Scale BMAD** : Enterprise (2 crates, machine à états, 3D→image, flux de lancement).
> **Parent DA** : reskin « Verre & Braise » livré (commits a5d8e48→9a728de). Voir memory `reference_ui_da_verre_braise_reskin`.

## Vision

Aujourd'hui : `Menu` (titre, `forgia-ui`, 4 boutons) → **Nouvelle Partie** → `InGame + GameMode::Roguelite + RunState::Lobby` (hub 3D atelier avec onglets Forge/Armes/Enclume/Talents, `forgia-mode-roguelite/hub.rs`) → **Lancer** → combat.

Cible (comme un roguelite type Gunfire/Hadès) : **le menu-titre EST le hub**. Toutes les sections (Forgeron, Armes, Talents, Enclume, Codex, Missions, Succès, Stats, Options) navigables **depuis le titre**, sans « entrer dans le jeu ». « Lancer une run » va **direct au combat** (plus de Lobby intermédiaire). Rendu proche de l'artifact (dashboard 2D verre & or).

## Fait décisif (dépendances)

- **`forgia-ui` → `forgia-mode-roguelite`** dep EXISTE (`crates/forgia-ui/Cargo.toml`), et `forgia-mode-roguelite` ne dépend PAS de `forgia-ui` (seulement `forgia-ui-lib`). → **Pas de cycle** : le menu peut lire les données roguelite (`MetaSouls`, `MetaShopSave`, `PlayerProgress`, `IdentitySave`, catalogues) et appeler des helpers de rendu.
- `MenuPage` (enum dans `forgia-ui/lib.rs`) = mécanisme de pages du menu (aujourd'hui `Root`/`Options`). **C'est le point d'extension** : ajouter une variante par section.

## Approche retenue

Le menu-titre (`AppMode::Menu`, `forgia-ui`) devient un **hub 2D** : bandeau (nom + Or/Âmes) + **navigation par onglets/sidebar** (verre) + panneau de la section active + CTA **LANCER**. Chaque section = une `MenuPage`.

- **Sections pures-UI / data** (rendables direct dans `forgia-ui` en lisant les Res roguelite) : Stats, Codex (archétypes), Talents/Missions/Succès (placeholders stylés — code déjà écrit dans `hub.rs`, à déplacer/partager), Options (existe : `draw_settings_controls`), Enclume (meta_shop : logique achat = touches/events, réutilisable).
- **Sections 3D → image** : Forgeron (perso) et Armes (arme) affichaient un aperçu 3D live (viewmodel/SceneRoot) qui n'existe PAS en `Menu` (pas de scène 3D chargée). → **Remplacer par une image/rendu statique** (screenshot pré-rendu par arme/couleur, ou icône disque colorée façon maquette). Les **stats/choix** restent (data).
- **Lancer** : depuis le hub-menu, `Nouvelle Partie`/`Lancer` doit passer `GameMode::None → Roguelite` ET aller **direct en combat** (pas Lobby). Vérifier la séquence d'init actuelle (`OnEnter(GameMode::Roguelite)` fait le setup ; `RunState::Lobby` est l'état par défaut). Option : garder un `RunState::Lobby` invisible/instantané qui auto-`StartRunEvent`, OU faire du hub-menu l'équivalent du Lobby et lancer via `StartRunEvent` en entrant InGame.

## Étapes (ordre + risque)

1. **[L] Cartographier le flux d'états exact** : lire `forgia-mode-roguelite/src/lib.rs` (OnEnter Roguelite, RunState default, ce que fait le Lobby au setup) + `run.rs` (StartRunEvent handler) + comment `forgia-ui` déclenche `Nouvelle Partie` (set GameMode + AppMode). Décider : hub-menu = nouvel état, ou Menu reste + orchestration.
2. **[M] Extension `MenuPage`** : Root(hub) + Forgeron / Armes / Talents / Enclume / Codex / Missions / Succes / Stats / Options. Barre de nav verre (réutiliser `glass_btn`/onglets stylés). Bandeau Or/Âmes (lire `MetaSouls` + `Gold`).
3. **[M] Déplacer les sections pures-UI** de `hub.rs` vers le menu : Codex (`draw_codex_section`), Talents/Missions/Succès (`section_intro`) — les extraire en helpers partagés (crate `forgia-ui-lib` ? ou `forgia-mode-roguelite` appelé par `forgia-ui`). Stats = nouveau panneau (lire `PlayerProgress` + `MetaShopSave` : runs/victoires/best).
4. **[M] Enclume au menu** : rendre `meta_shop` au niveau Menu (gating `AppMode::Menu` OU appel depuis forgia-ui). Achat clavier/souris déjà là.
5. **[H] Forgeron / Armes au menu (3D→image)** : remplacer l'aperçu 3D par une image statique par arme/couleur. Garder stats/choix. C'est l'étape la plus lourde (découpler du viewmodel).
6. **[H] Flux de lancement** : hub-menu → run directe (skip Lobby). Nettoyer/retirer le Lobby 3D (ou le réduire à un chargement).
7. **[L] Nettoyage** : retirer les onglets Codex/Missions/Succès du `hub.rs` Lobby (P1, mauvais endroit — commit 9a728de), ou supprimer le Lobby hub si absorbé.
8. **[L] check + clippy + tests + rebuild release-fast (jeu fermé) + revue user** à chaque étape.

## Risques / pièges

- **3D au menu** : le viewmodel/scène n'est pas chargé en `Menu`. Ne PAS tenter de forcer la 3D au menu → utiliser des images. (Sinon : charger une mini-scène 3D au menu = gros surcoût, à éviter.)
- **Init roguelite** : le setup `OnEnter(GameMode::Roguelite)` peut supposer qu'on passe par le Lobby. Vérifier avant de court-circuiter (risque : spawn joueur/monde mal initialisé si on saute le Lobby).
- **Persistance** : `MetaShopSave`/`IdentitySave` chargées au Startup — OK au menu.
- **Duplication** : les helpers Codex/section déjà dans `hub.rs` → extraire dans un module partagé, ne pas dupliquer.
- **Cycle crates** : forgia-ui→roguelite OK ; ne PAS faire roguelite→forgia-ui.
- **Rebuild** : `cargo build -p forgia --profile release-fast` ; **fermer le jeu avant** (Accès refusé sinon) ; `run_debug.bat` lance `target/release-fast/forgia.exe`.

## Critères d'acceptance

- Depuis le menu-titre, on navigue vers les 9 sections sans lancer de run.
- Rendu verre & or cohérent (proche artifact).
- « Lancer » démarre une run correctement (joueur/monde init OK, pas de régression).
- Aperçus 3D remplacés par images sans casser Forgeron/Armes.
- check + clippy 0 warning + tests verts. Récap runtime fourni.

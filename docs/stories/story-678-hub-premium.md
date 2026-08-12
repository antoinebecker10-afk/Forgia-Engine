# story-678 — Hub Premium : le menu devient un vrai hub de roguelite

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_menu_hub.json`, fichier `arena_backdrop.rs`, symbole `LastRunSummary`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (2026-08-05)
**Niveau BMAD** : Standard
**Plan** : [docs/thoughts/plan-hub-premium.md](../thoughts/plan-hub-premium.md) (SCOPE LOCK + phases détaillées)
**Crates** : forgia-ui, forgia-ui-lib, forgia-audio, forgia-mode-roguelite (meta_shop + waves), forgia-observability

## Contexte

Le menu (11 pages egui) a le contenu d'un hub de roguelite mais se présente comme un
classeur d'onglets : accueil statique, zéro son, zéro motion, zéro manette, rien ne
signale ce qui a changé. Décisions actées le 2026-08-05 : on reste 100 % egui (pas de
migration Bevy UI), cible « indie premium » (Hades/Dead Cells), pas Fortnite.

## Les 6 phases (ordre = rendement décroissant, chacune validable seule en jeu)

1. **Son d'UI** — 6 SFX (hover/clic/onglet/achat/déblocage/refus) via le générateur
   propriétaire (`tools/audio/generate_forgia_da.py`, conventions V2 anti-fatigue),
   `UiSfxEvent` dans forgia-audio, émission front-montant dans forgia-ui, genome.
2. **Transitions** — fade+slide ~150 ms entre pages, scale-punch boutons, toggle accessibilité.
3. **Accueil tableau de bord** — `LastRunSummary` persisté (save méta, serde default),
   carte chapitre en cours, bandeau dernière run, header Âmes/Puissance, « FORGER » 1 clic.
4. **Pastilles** — Enclume abordable / équipement non vu / chapitre débloqué.
5. **Fond RTT vivant** — avatar en pied + DA du chapitre, caméra lente (RTT obligatoire,
   jamais de scène 3D directe au menu).
6. **Manette** — focus visible, A/B/LB/RB, barre de hints, bascule auto souris↔manette.

## Acceptance Criteria

- [x] AC1 : chaque interaction menu joue un SFX distinct ; volume/toggle genome hot-reload ; **validation subjective Antoine obligatoire** (leçon V1 audio rejetée) — ✅ **VALIDÉ EN JEU 2026-08-06**
- [x] AC2 : plus aucun changement de page sec ; transitions désactivables — ✅ **VALIDÉ EN JEU 2026-08-06**
- [ ] AC3 : accueil = chapitre en cours + avatar + dernière run + header persistant ; run lancée en 1 clic
- [ ] AC4 : pastilles pilotées par l'état réel (abordable / non-vu / débloqué)
- [x] AC5 : fond RTT vivant thème chapitre, désactivable, coût GPU borné par gene —
  **RE-LIVRÉ puis ✅ VALIDÉ EN JEU 2026-08-06** (Antoine : « c'est beaucoup mieux »).
  Le fond est l'ARÈNE du chapitre atteint, pas une teinte. Preuve mécanique :
  `forgia.exe` bâti à 14:34:33 (après les sources), `forgia2_menu_hub.json` à 14:35
  → `backdrop_props: 17` (= 3 landmarks + 6 big + 5 scatter + 3 braseros, la palette
  était complète), `backdrop_ready: true`, `avatar_ready: true`, `severity: "info"`.
- ~~AC6 : menu 100 % navigable manette, focus visible, souris intacte~~ → **SORTI DU SCOPE 2026-08-06**, porté par [story-679](story-679-manette-menu-validation.md). Antoine n'utilise pas de manette : le code v1 (`gamepad_nav.rs`) reste en place et compile propre, mais **rien ne prouve qu'il fonctionne**. Le cocher sur la foi du code serait une DONE fictive ; laisser 678 bloquée sur du matériel absent n'apporte rien non plus.
- [ ] AC7 : capteur `forgia2_menu_hub.json` (cumuls session, front montant) + 2 health checks avec next-step
- [ ] AC8 : 0 clippy workspace, tests verts, saves méta antérieures compatibles, `xtask story-gate --story 678` vert

## Avancement

- **Phase 1 — ✅ VALIDÉE EN JEU 2026-08-06** (Antoine : « AC1 c'est ok ») : 6 SFX générés
  (`assets/audio/forgia_original/ui/`, RNG dédié `ui_rng` — preuve mécanique que
  l'ambiance/musique validées régénèrent bit-identiques, masters WAV comparés) ·
  `forgia-ui-lib/ui_sfx.rs` (file en mémoire egui, hover front-montant, cap 16) ·
  `cartoon_btn`/`glass_btn` instrumentés · Coffre pousse Buy/Denied · `audio.rs` :
  6 entrées genome + `sys_ui_sfx` NON gaté (menu) + hot-reload dégaté + compteur
  `ui_sfx` au capteur `forgia2_roguelite_audio.json` · genome `roguelite_audio.toml`
  (`ui_sfx_volume` + 6 tables). Clippy 0 warning.
  **Complété 2026-08-06** : les onglets de la sidebar étaient MUETS au survol —
  ils sont dessinés en `Button::selectable` bruts, donc hors des helpers de
  style qui sonnent. `instrument_hover` extrait de `instrument_button` (le clic
  garde son son `Tab` propre, sans doublon `Click`).
- **Phase 2 — ✅ VALIDÉE EN JEU 2026-08-06** :
  `forgia-ui-lib/motion.rs` (fade+slide 150 ms par section, anneau d'impulsion
  250 ms, toggle central) · `hub_section_panel` + Root animés · toggle
  **persisté** dans `UserSettings.ui_motion_enabled` (serde default, checkbox
  Options, miroir chaque frame SANS garde is_changed). Clippy 0 warning.

- **Phase 3 — CODE-COMPLETE 2026-08-05** (⚠ non validée en jeu) :
  `LastRunSummary` persisté dans `MetaShopSave` (`#[serde(default)]`, saves
  antérieures intactes), gelé par `sys_record_run_stats` aux DEUX sorties de
  run (une seule définition ; round canonique via `RogueliteWave::round`,
  Âmes via `MetaSouls::earned_run`) — **waves.rs non touché** (mieux que le
  plan : zéro contact avec le cycle de validation). Accueil :
  `sys_menu_root_dashboard` (système séparé, plafond 16 params respecté) —
  carte CHAPITRE EN COURS (n° + univers + menace ×N + progression Livre),
  chip Puissance (score + record), bandeau DERNIÈRE RUN (verdict, round,
  chrono, Âmes, ★ record). `chapter_da_name` rendu pub. Clippy 0 warning.

- **Phase 4 — CODE-COMPLETE 2026-08-05** (⚠ non validée en jeu) : pastilles
  dorées sur Enclume (achat abordable, `enclume_affordable` — badge honnête sur
  les 3 familles d'achats), Forgeron (pièce non vue, `seen_owned_total` dans
  EquipmentSave, scalaire AVANT les tables), Livre (chapitre débloqué,
  `seen_chapters_cleared`). Visiter la page éteint sa pastille. Écritures
  gardées (pas de change-detection à vide). `HubBadges` + `sys_hub_badges`.
- **Phase 5 — RE-LIVRÉE 2026-08-06** (⚠ non validée en jeu, build bloqué hors
  périmètre) : **le fond du menu EST l'arène du chapitre le plus haut atteint.**
  Nouveau `forgia-ui/arena_backdrop.rs` — un diorama RTT bâti avec les GLB de la
  palette du chapitre (`palette_id_for_chapter`), calibrés aux **mêmes tailles
  cibles** que la vraie arène (`RogueliteDecorConfig::target_*`) et noyés dans la
  **même brume** que son ambiance. Caméra en dérive lente (deux sinus de périodes
  premières entre elles). Gènes `ui_backdrop_enabled` / `ui_backdrop_height_px`
  (borné 240..1080) dans `roguelite_render.toml` → repli vidéo sans rebuild.
  Les deux pièges de la 1ʳᵉ tentative sont écartés **par construction** :
  la caméra du fond rend `CHARACTER_LAYER` en plus du sien (donc **un seul
  avatar** — pas de squelette dupliqué), et l'image est **opaque** (donc plus
  aucune composition alpha à réussir face au tonemapping). Zéro passe fullscreen
  ajoutée. Échelle entièrement dérivée : le personnage calibré vaut 2 m, tout le
  décor s'en déduit. 6 tests. Clippy 0 warning.
  **Relayout demandé par Antoine, livré avec** : navigation **horizontale
  centrée en haut** (elle mangeait 228 px de large sur toute la hauteur ; les
  pages ont récupéré le centre de l'écran et le décalage +100 px a disparu),
  identité du forgeron sortie en chip haut-gauche en miroir du chip Âmes,
  titre FORGIA passé à gauche en tête de la colonne de préparation, carte de
  chapitre en colonne gauche, **portrait en carte supprimé** (le personnage est
  dans le décor), panneau d'équipement réduit à ce qu'une image ne dit pas
  (Puissance + pièces + Personnaliser), scrim devenu **diagonal** sur l'arène
  pour ne pas noyer le personnage dans le même voile que le texte.
  Grandeurs de chrome (`HUB_TOP_BAR_H`, `ROOT_COL_X`) en **source unique**.
- **Phase 5 (1ʳᵉ tentative) — 2026-08-05, DÉFAITE le même jour** : l'avatar
  équipé EN PIED sur le fond du menu (droite, ghosté α120, dérive lente
  motion-gated) — réutilise `CharacterPreviewRtt` déjà rendu : **zéro passe de
  rendu ajoutée** (une passe fullscreen crashe DX12), **zéro squelette
  dupliqué** (piège shared-skeleton). Le fond vidéo + scrim (story-596)
  restent la base. Teinte par chapitre (couleurs d'ambiance) = incrément futur.
- **Phase 6 v1 — CODE-COMPLETE 2026-08-05, VALIDATION PORTÉE PAR [story-679](story-679-manette-menu-validation.md)** :
  `gamepad_nav.rs` — la manette est TRADUITE en clavier egui (D-Pad→flèches,
  A→Entrée, B→Échap ; 1ʳᵉ pression directionnelle→Tab pour amorcer le focus),
  injectée entre `ProcessInput` et `BeginPass` (hook documenté bevy_egui).
  LB/RB = onglets (+ son Tab). Anneau de focus doré. Barre de hints seulement
  quand la dernière entrée est manette (`LastInputKind`). v1 assumée : D-Pad
  seul (sticks = timers de répétition, incrément suivant), menu seul (pause à
  venir).
- **Transverse — CODE-COMPLETE 2026-08-05** : capteur `forgia2_menu_hub.json`
  (1 Hz, cumuls session en front montant) + 2 health checks avec next-step
  (MENU MUET, BACKDROP MORT) + ligne au SENSOR_REGISTRY.md. `story-gate` à
  passer AVANT tout passage DONE (le fichier story doit d'abord être commité —
  G1 git-tracked).

- **Refonte Dicero + inspection — 2026-08-05 (fin de journée)** : après
  inspection visuelle (captures) et retours utilisateur, l'Accueil est devenu un
  **écran de préparation** (carrousel ‹ chapitre ›, portrait cadré + pièces
  portées + Personnaliser, CONTINUER/Hall/QUITTER dans la carte, NOUVELLE
  PARTIE fusionné) ; **Livre + Forgeron retirés de la sidebar** (9 onglets,
  pages conservées, Forgeron a un ← Retour) ; 4 pages artisanales converties au
  chrome commun `hub_section_panel` ; tofus corrigés ; fantôme plein écran
  remplacé par le portrait (le tonemapping force alpha=1 sur les RTT — voir
  mémoire `reference_egui_menu_hub_patterns`). Clippy 0, binaire rebuilt.
  **Non tranché** : rotation de l'avatar (garder/ralentir/figer). **Non
  commité** : tout le chantier.

### 🎨 Les décors deviennent une COLLECTION (2026-08-06, demande Antoine)

Le fond suivait le chapitre le plus haut atteint : il **miroitait** la
progression. Il devient une **cosmétique** — on en débloque, on en possède
plusieurs, le joueur choisit celui qu'il affiche.

- **Un décor = une palette + une ambiance.** Les props disent ce qu'on voit,
  l'ambiance dit la lumière dans laquelle on le voit. C'est ce qui permet
  d'étendre la collection **sans un seul asset neuf** : « La Forge » et « La
  Forge au crépuscule » partagent leurs rochers, pas leur heure du jour.
- **Catalogue en data** — `assets/genomes/roguelite/roguelite_backdrops.toml`
  (hot-reload 1 Hz), 14 décors : 1 de départ, 9 gagnés en battant leur chapitre,
  4 achetables (300 → 650 Âmes).
- **Ce qui est stocké vs dérivé** : un décor de chapitre se **déduit** de
  `chapters_cleared` (le stocker ferait deux vérités, qui divergeraient à la
  première renumérotation) ; un décor **acheté** est stocké, c'est un choix.
- **Achat hors de l'Enclume** : l'Enclume vend de la puissance. Mêler un achat
  cosmétique à ses rangs forcerait un arbitrage « je tape plus fort » vs « c'est
  plus joli » dans la même liste. La galerie porte donc son propre prix.
- **Débit sûr** : re-contrôle de la bourse au moment d'agir (l'état dessiné a une
  frame d'âge) et débit **conditionné au déblocage réel** — `unlock_backdrop`
  rend `false` si on le possédait déjà, donc un double-clic ne paie pas deux fois.
  La dépense passe par `MetaSouls`, jamais par le miroir `souls_total`.
- **Sauvegarde bricolée** : `BackdropsConfig::resolve` revalide la possession au
  rendu — équiper à la main un décor non gagné retombe sur le repli.
- Nouveaux : `forgia-mode-roguelite/backdrops.rs` (8 tests), `MenuPage::Decors`,
  `IdentitySave::{equip_backdrop, unlock_backdrop}` (opérations d'intention : la
  persistance reste dans le module qui possède le fichier).

**Piège évité** : ni l'init ni le hot-reload du catalogue ne sont gatés sur
`GameMode::Roguelite` — le menu tourne en `GameMode::None`. C'est le gate déjà
payé deux fois (musique de hub, sons d'UI).

**Validé en jeu 2026-08-06** — capteur : `backdrops_owned: 6 / 14`,
`backdrop_shown` = `backdrop_wanted`, `backdrop_props: 17`. Changement de décor
vu à l'écran (Crypte → Forêt Profonde).

**3 défauts vus à l'inspection, corrigés dans la foulée** :
1. la page affichait **« Contenu à venir »** alors qu'elle fonctionne —
   `section_intro` est le helper des pages-GABARIT et termine par cette mention ;
   une interface qui ment sur elle-même. Intro écrite en clair.
2. cinq boutons « Afficher » en `cartoon_btn` s'étiraient sur toute la largeur et
   écrasaient la page → largeur fixe, colonnes alignées.
3. la barre d'avancement du chip forgeron faisait 8 px pour un texte de 9 px
   (libellé coupé), et son texte crème se noyait sur le remplissage doré.

### 🛍 MARKETPLACE — 4 familles, monnaie séparée (2026-08-06, demande Antoine)

`backdrops.rs` généralisé en **`cosmetics.rs`** : un catalogue, une règle de
possession, quatre familles (`decor` / `color` / `arm` / `music`). 27 articles
dans `roguelite_cosmetics.toml` (hot-reload). `MenuPage::Marketplace` avec un
onglet par famille, atteint depuis le Forgeron.

- **Monnaie ÉCLATS**, séparée des Âmes (choix d'Antoine). `MetaShopSave.shards_total`,
  gagnés en fin de run au barème du génome — **à la profondeur atteinte, jamais au
  temps passé**. ⚠ Barème jamais joué : 1/round + 10/chapitre bouclé, prix 40→120.
  À confronter au playtest.
- **Pas de miroir vif** pour les Éclats, contrairement aux Âmes : une seule
  vérité, donc le défaut « achat sans effet » ne peut pas s'y reproduire.
- **Un stock de possession par famille** — celui qui sert DÉJÀ à cette famille
  ailleurs (couleurs dans `unlocked_colors`, que le panneau Forgeron filtre et
  que le boot fait respecter). Une cinquième liste centralisée aurait fait deux
  vérités par famille, et le boot aurait réinitialisé une couleur achetée.
- **Application branchée** : décor (`arena_backdrop`), couleur (existant),
  bras (`sys_sync_arm_cosmetics`, miroir set-if-different — sans lui l'article
  serait payé et inerte), musique du hub (`hub_music_chapter_index`, avec le
  décalage 1-indexé/0-indexé nommé et testé).

**🚨 Erreur corrigée dans la foulée** — j'avais annoncé que les couleurs
`azur`/`emeraude`/`pourpre`/`or` étaient du **contenu mort**. FAUX :
`sys_init_identity` les débloque toutes au boot (« MVP : gratuites »).
Conséquence sur le livré : l'onglet Couleurs aurait affiché un prix pour ce que
le jeu donne. Corrigé par `color_is_governed` — une couleur listée au catalogue
avec une source autre que `start` cesse d'être offerte ; une couleur absente du
catalogue reste gratuite, donc rien ne disparaît par omission, et **rien n'est
retranché d'une sauvegarde existante**.

### 🐛 « Achat sans effet » — la cause, et la classe (2026-08-06)

Une dépense d'Âmes = **trois écritures indissociables** (solde vif, miroir
persisté, écriture disque). La galerie n'en faisait qu'une. Invisible parce que
l'autosave porte `if meta.current <= save.souls_total { return }` : **il ne pousse
que les GAINS**. La dépense n'était jamais persistée, le compteur (qui lit le
miroir) ne bougeait pas, et les Âmes revenaient au relancement.

Le geste était **déjà écrit deux fois** dans `apply_meta_purchase` ; la galerie
en aurait fait un troisième site. Nommé une fois — `meta_shop::spend_souls` —
et les trois l'appellent. Test `une_depense_bouge_le_solde_et_son_miroir_persiste`
sur la partie pure (`debit_souls`), pour ne pas écrire dans la vraie sauvegarde.

### 🔍 AUDIT UI complet au curseur + correctifs (2026-08-06 soir)

Tour de TOUTES les pages au curseur (main explicitement confiée) : 11 onglets,
4 sous-onglets Marketplace, un achat réel (Gants cyber −80 Éclats, débité +
porté + persisté, vérifié à l'écran ET sur disque). Constats et remèdes :

- 🔴 **Nav incliquable sous les pages hautes** (prouvé : clic Marketplace avalé
  depuis Options) — une `Area` trop haute est remontée par egui PAR-DESSUS la
  barre, qui mange les clics. Fix : `hub_section_panel` plafonne sa hauteur +
  `ScrollArea` interne. Systémique, réglé pour toutes les pages.
- 🔴 **Fiche Sac/Forgeron explosée** (sac écrasé en colonne de lettres) — un
  `vertical_centered` dans l'horizontal réclamait toute la largeur restante.
  Fix : colonnes à largeur FIXE, structure alignée sur la référence d'Antoine
  (« Classic RPG UI » : personnage encadré de cellules, grille à côté,
  **bloc CARACTÉRISTIQUES restauré** dessous).
- 🟠 **Couleur payable DEUX FOIS** — `has()` comparait l'id d'ARTICLE
  (`color_azur`) au stock qui contient l'id de COULEUR (`azur`) : toute couleur
  achetée restait « Débloquer ». Vu en jeu (azur/émeraude payées puis niées par
  l'UI), prouvé par le fichier. Fix + le test vérifie désormais `has()` APRÈS
  l'achat — c'est la vérification qui manquait.
- 🟠 **Aperçus RTT roses** — `color_grading::sys_apply` grade TOUTES les
  `Camera3d`, aperçus compris (l'hypothèse atmosphère est RÉFUTÉE : elle ne
  cible que `FpsCamera`). Fix : marqueur `UiStudioCamera` (forgia-core), les
  caméras d'aperçu s'excluent ; le fond d'arène, lui, reste gradé (il montre
  l'univers). Clear du portrait passé opaque studio (le transparent n'a jamais
  marché sous tonemapping).
- 🟡 Tofus (`←`, `🛍`) remplacés (chevrons, 💰) ; clic sur un emplacement de la
  poupée → sélectionne la pièce dans le sac ; « Ouvrir le sac » retiré (même
  écran) ; titre « TON SAC » quand on entre par l'onglet Sac.

### 🧨 INCIDENT — découpe par script trop large, et reconstruction

En réécrivant la fiche, un remplacement par script a matché un marqueur
(« chrome commun ») dans `draw_options_page` au lieu du forgeron : tout
`draw_options_page` → `sys_menu_forgeron` a été REMPLACÉ par le seul corps du
forgeron (~600 lignes avalées : `HubBadges`, `sys_hub_badges`, dashboard,
Livre, Enclume). Fichier non commité → aucun filet git ; l'historique local
VSCode n'avait qu'un instantané de mai.

**Reconstruit à l'identique** depuis les lectures de session (dashboard et
forgeron lus intégralement le jour même) et les fonctions PARTAGÉES intactes
(`chapter_select_content`, `draw_enclume_panel`/`apply_meta_purchase`,
`draw_settings_controls`/`save_user_settings`) — l'architecture « le contenu
vit dans des helpers partagés, les pages sont des enveloppes » est ce qui a
rendu la reconstruction possible. Rebuild vert, 0 warning, 15 tests.

Leçons : (1) un remplacement par script exige un marqueur UNIQUE vérifié
(`assert count==1`) et une recherche BORNÉE au span visé ; (2) ce chantier
travaille depuis 2 jours sans un seul commit — c'est le vrai risque, pas le
script. Copie de sûreté du fichier en scratchpad en attendant le commit.

### 🚧 Blocage build — hors périmètre (2026-08-06) — RÉSOLU depuis

⚠ Historique conservé : la validation en jeu a eu lieu **après** que l'autre
terminal a corrigé son import (exe bâti à 14:34, capteur à 14:35).

`cargo build -p forgia` échoue dans **`crates/forgia-stage/src/authored.rs`** :
`use bevy::bevy_camera::primitives::MeshAabb` — ce chemin n'existe pas en Bevy
0.18, tout le reste du workspace importe `bevy::camera::primitives::*`. Le
fichier a été modifié à 14:05, soit **après** mes propres écritures : c'est le
chantier en cours de l'autre terminal (branche `feat/arena-authored-shell-625`).

**Non patché volontairement** (`multi-terminal-coordination` §Règle 2 : on ne
corrige pas l'erreur d'autrui, c'est un conflit garanti à la sync). Conséquence
directe : **la Phase 5 re-livrée n'a pas pu être vue tourner**. Mes crates, elles,
compilent et lintent proprement (`forgia-ui`, `forgia-mode-roguelite`,
`forgia-ui-lib`, `forgia-audio` — 0 warning, 10 tests verts).

### Capteur — correction d'un signal menteur (2026-08-06)

`backdrop_ready` mesurait `CharacterPreviewRtt.is_some()`, c'est-à-dire le
**portrait qui avait remplacé le fond**. Le health check « BACKDROP MORT » ne
pouvait donc pas voir la panne qu'il surveillait : il restait vert alors qu'il
n'y avait plus aucun fond. Il mesure désormais `backdrop_props > 0` (le nombre de
props réellement posés), et l'avatar est publié à part (`avatar_ready`) — deux
symptômes distincts, deux champs distincts.

### Déviations vs plan (assumées)

- Pas de `roguelite_ui.toml` : les genes UI audio vivent dans
  `roguelite_audio.toml` (un seul genome audio, règle « doublons interdits ») ;
  le toggle motion vit dans `UserSettings` (persistance déjà câblée) — les
  durées motion sont des consts nommées (exception UI cosmétique).
- Pas de `UiSfxEvent` Bevy : file en mémoire egui drainée par `sys_ui_sfx` —
  moins de plomberie, et tous les boutons du jeu sonorisés d'un coup.

### Hors scope, fait au passage

- `barks.rs` : complété la migration `audio_path` de la session audio du matin
  (3 initialiseurs de tests + `#[allow(too_many_arguments)]` maison) — elle
  cassait `--all-targets` pour tout le monde.

### Dette notée (hooks hotspot)

- `forgia-mode-roguelite/audio.rs` (30 éditions) et `forgia-ui-lib/pause_menu.rs`
  (15) : candidats au split en modules — story dédiée, pas sous ce scope lock.

## Risques & garde-fous

Voir le plan (SCOPE LOCK). Points durs : hover-sfx en front montant (anti-spam 60 Hz),
`waves.rs` = seul contact avec le cycle validation en cours (diff à vérifier avant),
volume kira à l'INSTANCE jamais au canal, `RenderLayers` propagé aux enfants GLB.

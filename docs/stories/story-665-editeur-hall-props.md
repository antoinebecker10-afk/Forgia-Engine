# story-665 — Éditeur in-game du Hall (lot 1 : props)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_editor.json`, fichier `bindings.rs`, symbole `SceneEdits`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (implémentation livrée, validation runtime en attente)
**Niveau BMAD** : Enterprise (nouvelle crate + 4 fichiers de câblage)
**Date** : 2026-07-26
**Related** : story-661 (bras viewmodel), reference_castle_hub_walkable_glb_mode

---

## Demande

> « Uniquement dans "Hall forgia" pour l'instant, en appuyant sur `.` du pavé
> numérique, je veux pouvoir ouvrir un petit interface éditeur très simple,
> permettant de sélectionner des assets, les déplacer en X Y Z comme dans Blender,
> les faire pivoter, agrandir et rétrécir, en ajouter d'autres qui sont dans ma
> bibliothèque d'assets avec l'option "aimant" pour faciliter le placement des
> assets par rapport au sol etc.. et trier correctement ainsi que des pinceaux
> pour pouvoir lisser, ajouter ou retirer du sol, faire grossir ou réduire le
> pinceau, choisir une texture pour peindre soit tout un asset soit comme
> photoshop, peindre les éléments. »

**Découpage validé avec l'utilisateur** (question posée avant implémentation) :

| Lot | Contenu | Statut |
|---|---|---|
| **1** | Props : sélection, déplacer / tourner / redimensionner, bibliothèque, aimant, tri, persistance | **cette story** |
| 2 | Pinceaux de sol (élever / creuser / lisser, rayon variable) | à faire |
| 3 | Peinture : texture sur asset entier, puis peinture per-texel façon Photoshop | à faire |

Raison du découpage : la peinture per-texel coûte à elle seule plus que 1→6
réunis (texture de peinture par instance, UV uniques, persistance des calques).

---

## Concept-First

**Concept** = `édition de scène` (nouveau). **Couche** = framework (nouvelle
logique) + definition (le fichier d'édition est de la donnée).

- **Producteur** : `castle_hub_edits.json` (vérité persistée) → `SceneEdits`
  (vérité runtime, `crates/forgia-editor/src/persist.rs`), timing `on_enter` +
  autosave.
- **Consommateurs** : `library::spawn_prop_entity` (respawn des props au
  chargement), `persist::sys_apply_overrides` (réapplique aux cellules qui
  reviennent par streaming), l'inspecteur egui (`panel.rs`).
- **Observabilité** : `forgia2_editor.json` (1 Hz) + alerte `critical` si
  l'écriture échoue.
- **Hot** : oui pour `pick::sys_editor_ray` (balayage AABB sur les meshes
  visibles) → checklist §4 appliquée : query filtrée `With<Mesh3d>`, zéro
  allocation, rejet des invisibles d'abord, `run_if(editor_is_open)`.
- **Réseau** : local (outil de création). **Script** : interne.

### Ce que la cartographie a évité

Le décor du Hall **n'a aucun collider par pièce** (`castle_hub.rs` : un TriMesh
fusionné + 5 boîtes de la Grande Salle). Un rayon physique — le réflexe, et ce
que faisait V1 — ne peut donc pas désigner un tonneau ou une colonne : il ne
touche que le proxy de collision. D'où la **double visée** de `pick.rs` : rayon
physique pour le point du monde, test rayon/AABB pour la pièce.

---

## Réutilisation V1

V1 (`D:\Forgia\RUST\Forgia\Forgia`) avait un éditeur : `ui/editor/` (2 720 LOC :
gizmo, select, move, snap, grid, toolbar, undo), `persistence/placed_objects.rs`
(356 LOC) et `terrain/sculpting.rs` (1 199 LOC, 9 pinceaux — matière du lot 2).

Ce qui a été **repris comme idée** : raycast d'éditeur centralisé 1/frame
(LOCK L4), remontée de hiérarchie vers l'objet éditable, fichier de sauvegarde
versionné + autosave débouncé, re-snap au sol après modification du terrain.

Ce qui a été **refait** plutôt que porté :
- la sélection (V1 dépendait de colliders par objet — impossible ici) ;
- le déplacement (V1 : glisser sur un plan Y ; demandé : axes façon Blender) ;
- la bibliothèque (V1 n'en avait pas — palette codée en dur).

---

## Livré

Nouvelle crate `crates/forgia-editor` (2 968 LOC, 9 modules) :

| Fichier | Rôle |
|---|---|
| `lib.rs` | Plugin, session, bascule pavé num `.`, curseur / blockers, gating |
| `pick.rs` | Double visée : rayon physique + test rayon/AABB (5 tests) |
| `select.rs` | Sélection, clé stable de décor, surlignage boîte réelle, suppr / dupliquer |
| `transform_ops.rs` | G/R/T + axes 1/2/3, pas fixes, pivot, `Ctrl+Z`, **sûreté miroir** (8 tests) |
| `snap.rs` | Aimant Sol / Grille / Libre, pose au sol par boîte englobante (2 tests) |
| `library.rs` | Balayage disque de `assets/models*`, groupes triés, file de spawn |
| `persist.rs` | `castle_hub_edits.json` versionné, autosave, réapplication au streaming, restauration des pièces masquées (5 tests) |
| `panel.rs` | 3 panneaux egui : barre d'outils, bibliothèque filtrable, inspecteur |
| `sensor.rs` | `forgia2_editor.json` + sévérité/next-step (5 tests) |
| `history.rs` | Journal des modifications persisté + annulation entrée par entrée (7 tests) |

Câblage (4 fichiers hors crate) :
- `Cargo.toml` : member + dépendance workspace ;
- `crates/forgia-game/Cargo.toml` + `src/lib.rs` : plugin ajouté (bloc 7e-quater) ;
- `crates/forgia-game/src/castle_ground.rs` : `sys_nudge_ground` désarmé quand
  l'éditeur est ouvert (il occupe tout le pavé numérique) ;
- `ARCHITECTURE.md` : ligne de crate (gate `arch-drift`).

---

## Décisions de conception

### 1. Clavier — pourquoi pas G/R/S + X/Y/Z

`KeyCode` est **physique** (positions QWERTY). Sur AZERTY, le déplacement occupe
`KeyW/KeyA/KeyS/KeyD` = les touches **Z Q S D**. « S » (scale) tomberait sur
*reculer*, « Z » (axe) sur *avancer*. Retenu : **G** déplacer, **R** tourner,
**T** taille, axes **1 / 2 / 3**, légende affichée en permanence dans le panneau.

Conflit résiduel assumé : si l'overlay debug **F2** est ouvert, `1/2/3` y togglent
aussi une catégorie (`forgia-debug/src/bindings.rs`). Sans effet sur l'édition.

`ESC` n'est **pas** utilisé (anti-trap « 1 KeyCode = 1 handler ») : annuler un
geste = `Retour arrière`, fermer = re-appuyer sur `.`.

### 2. Modèle d'interaction

Éditeur ouvert → curseur libre, caméra figée (`block_look`), Z Q S D toujours
actifs pour aller placer un objet ailleurs. **Clic droit maintenu** = regarder
autour (curseur verrouillé le temps du clic). Le vol libre `\` reste disponible.

### 3. Persistance non destructive

Deux formes d'édition, jamais de réécriture d'un GLB livré :
- `props` : assets **ajoutés** (chemin + transform) ;
- `overrides` : pièces **préexistantes** déplacées ou masquées, clé
  `<scène>#<index de fratrie>:<nom de nœud>`.

L'index de fratrie discrimine les homonymes (les packs en ont beaucoup) et reste
stable, l'ordre des nœuds glTF étant déterministe. Le format de clé vit dans
**une seule** fonction (`select::decor_key_from_parts`) : deux formats divergents
entre l'écriture et la relecture perdraient les éditions en silence.

Les cellules du château entrent et sortent par streaming → les overrides sont
réappliqués **à chaque instanciation**, pas une fois au chargement.

### 4. Miroirs du château — pas de recomposition de matrice

Le château importé porte des transformations **miroir** (échelle négative), suite
de la conversion de repère depuis le pack Unity. Deux réflexes le détruiraient en
silence dès le premier geste :

- `Transform::from_matrix(...)` — la décomposition d'un déterminant négatif est
  arbitraire ;
- `scale.max(Vec3::splat(MIN))` — retourne la pièce.

Le geste est donc appliqué **composante par composante en espace local** (axe et
pivot convertis via l'inverse du parent) et l'échelle est planchée avec
`MIN.copysign(valeur)`. Un test dédié verrouille ce comportement.

### 5. Pose au sol — trois défauts corrigés après incident runtime

Signalé deux fois par l'utilisateur : « j'ai appuyé sur poser au sol et le sol
s'est mis au plafond ».

**Cause racine** : le rayon de sondage vertical touchait **l'objet qu'on est en
train de poser**. Le terrain a son propre collider ; son bas (−59 m) a donc été
posé sur son propre sommet (+57 m) → +116 m. Tout objet ayant un collider est
concerné, pas seulement le terrain — les props du lot 1 n'en ont pas, d'où le
fait que ça marchait sur eux.

Trois correctifs, du plus fondamental au plus défensif :

1. **Exclusion de soi** : le rayon ignore tout le sous-arbre de l'objet posé
   (`collect_subtree` + `QueryFilter::predicate`). C'est le vrai correctif.
2. **Refus des remontées aberrantes** : poser au sol *dépose* ; une remontée de
   plus de 5 m signifie qu'on a touché autre chose que le sol → refus + message.
   Descendre reste libre (déposer un objet en l'air est le cas nominal).
3. **Garde-fou de sélection** : une pièce dont la boîte dépasse 80 m n'est plus
   sélectionnable (terrain, tapis de végétation). Le panneau affiche pourquoi,
   au lieu de laisser croire à un bug de visée.

### 6. Le rayon part du curseur, pas du centre de l'écran

Signalé : « je veux pouvoir avec clic gauche sélectionner un asset » — la fonction
existait mais ne répondait pas. L'éditeur **libère la souris**, or `pick.rs` tirait
son rayon depuis l'axe de la caméra : le créateur visait avec son curseur pendant
que le moteur testait le centre de l'écran. Corrigé via `Camera::viewport_to_world`
sur la position du curseur, avec repli sur l'axe caméra pendant la navigation
(clic droit maintenu, curseur verrouillé). Le placement d'un nouvel asset en
profite aussi : il apparaît là où on pointe.

### 7. Bibliothèque = l'arborescence du disque

Balayage de `assets/models/` et `assets/models-v1/` (~1 000 GLB/glTF), groupés
par dossier. Le dossier EST le tri : dupliquer ce classement dans un TOML
créerait une deuxième vérité à synchroniser. Dossiers ignorés : `previews`,
`src`, et les 4×46 cellules du château (elles *sont* le Hall).

---

## Critères d'acceptation

- [x] `.` du pavé numérique ouvre/ferme l'éditeur, dans le Hall uniquement
- [x] Clic gauche sélectionne une pièce de décor **ou** un asset ajouté
- [x] Déplacer / tourner / redimensionner avec contrainte d'axe et pas fixes
- [x] Ajouter un asset depuis la bibliothèque, listée et triée
- [x] Aimant Sol (pose sur la surface via boîte englobante) / Grille / Libre
- [x] Supprimer (décor → masqué, **réversible** via « Restaurer N pièce(s) »), dupliquer, `Ctrl+Z`
- [x] Inspecteur numérique (position / rotation / taille)
- [x] Persistance versionnée + autosave + réapplication au streaming
- [x] Capteur `forgia2_editor.json` avec sévérité et action de remédiation
- [x] `cargo check` + `cargo clippy` : 0 warning
- [x] 34 tests unitaires verts (`RUSTUP_TOOLCHAIN=stable`, `target/` isolé)
- [x] Historique consultable des modifications, annulable à la main entrée par entrée
- [ ] **Validation runtime par l'utilisateur** (récap de test fourni)

---

## Reste à faire (hors lot 1)

| Sujet | Priorité | Note |
|---|---|---|
| Lot 2 — pinceaux de sol | 🟠 | Le sol du Hall est un GLB cuit : voie retenue = déplacer les sommets dans le rayon puis reconstruire le collider TriMesh au relâchement (préserve la peinture gazon/terre/pavé cuite) |
| Lot 3 — peinture | 🟡 | Asset entier = swap de matériau (simple) ; per-texel = texture par instance + UV uniques + persistance |
| Colliders sur les props ajoutés | 🟡 | Volontairement absents : une boîte fausse sur une torche est pire que rien. Demande un choix explicite de forme |
| ~~Garde-fou anti-sélection du terrain~~ | ✅ | Corrigé 2026-07-26 (cf §6) |
| Sélection multiple par rectangle | 🟢 | `Maj+clic` suffit pour l'instant |

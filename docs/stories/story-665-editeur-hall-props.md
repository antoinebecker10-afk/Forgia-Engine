# story-665 — Éditeur in-game du Hall (lot 1 : props)

**Statut** : IN_PROGRESS (implémentation livrée, validation runtime en attente)
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

Nouvelle crate `crates/forgia-editor` (~2 050 LOC, 8 modules) :

| Fichier | Rôle |
|---|---|
| `lib.rs` | Plugin, session, bascule pavé num `.`, curseur / blockers, gating |
| `pick.rs` | Double visée : rayon physique + test rayon/AABB (5 tests) |
| `select.rs` | Sélection, clé stable de décor, surlignage boîte réelle, suppr / dupliquer |
| `transform_ops.rs` | G/R/T + axes 1/2/3, pas fixes, pivot, `Ctrl+Z` (5 tests) |
| `snap.rs` | Aimant Sol / Grille / Libre, pose au sol par boîte englobante (2 tests) |
| `library.rs` | Balayage disque de `assets/models*`, groupes triés, file de spawn |
| `persist.rs` | `castle_hub_edits.json` versionné, autosave, réapplication au streaming (4 tests) |
| `panel.rs` | 3 panneaux egui : barre d'outils, bibliothèque filtrable, inspecteur |
| `sensor.rs` | `forgia2_editor.json` + sévérité/next-step (5 tests) |

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

### 4. Bibliothèque = l'arborescence du disque

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
- [x] Supprimer (décor → masqué), dupliquer, `Ctrl+Z`
- [x] Inspecteur numérique (position / rotation / taille)
- [x] Persistance versionnée + autosave + réapplication au streaming
- [x] Capteur `forgia2_editor.json` avec sévérité et action de remédiation
- [x] `cargo check` + `cargo clippy` : 0 warning
- [ ] 21 tests unitaires verts (passe en cours dans un `target/` isolé)
- [ ] **Validation runtime par l'utilisateur** (récap de test fourni)

---

## Reste à faire (hors lot 1)

| Sujet | Priorité | Note |
|---|---|---|
| Lot 2 — pinceaux de sol | 🟠 | Le sol du Hall est un GLB cuit : voie retenue = déplacer les sommets dans le rayon puis reconstruire le collider TriMesh au relâchement (préserve la peinture gazon/terre/pavé cuite) |
| Lot 3 — peinture | 🟡 | Asset entier = swap de matériau (simple) ; per-texel = texture par instance + UV uniques + persistance |
| Colliders sur les props ajoutés | 🟡 | Volontairement absents : une boîte fausse sur une torche est pire que rien. Demande un choix explicite de forme |
| Annulation de spawn / suppression | 🟡 | `Ctrl+Z` ne couvre que les transforms |
| Sélection multiple par rectangle | 🟢 | `Maj+clic` suffit pour l'instant |

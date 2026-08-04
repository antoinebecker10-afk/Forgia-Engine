# story-675 — Personnage Trooper + équipement loot par rareté

**Statut** : IN_PROGRESS (code livré, **runtime non validé**)
**Niveau BMAD** : Standard (7 fichiers)
**Date** : 2026-07-31
**Demande** : « `D:\ressources externes\Perso` — que ce soit le personnage par défaut,
que les autres skins correspondent à des loots, et en fonction de la couleur chaque
pièce apporte des avantages pour les combats. On doit pouvoir les équiper dans le
menu, là où on choisit la couleur des bras. Comme dans un roguelite. »

---

## 1. Ce que la source contenait réellement

`Perso/scifitroopermanv3.unitypackage` (73 Mo) → **un seul personnage**, pas de
skins multiples. Rig Unreal 68 os, 21 920 tris, 1,82 m, 2 jeux de textures 4096².

Trois sous-mesh, qui se sont révélés être une découpe corps/armure exploitable :

| Sous-mesh | Contenu | Tris |
|---|---|---|
| `Trooper_003` | corps nu (combinaison lisse) | 7 940 |
| `Trooper_000` | armure : casque, épaulières, brassards, gants, cuissards, bottes | 10 556 |
| `Trooper_002` | plastron | 3 424 |

**Conséquence de design** : il n'y avait aucun « autre skin » à looter. La rareté
(choix d'Antoine) fournit la variation : une même pièce en Commun ou en Mythique
est une pièce différente à l'œil, parce que sa **couleur** la distingue.

## 2. Découpe dérivée, pas déclarée

`tools/blender/build_trooper.py` sépare l'armure en **îlots de géométrie**, puis
classe chaque îlot par **l'os qui porte le plus de poids de skinning**. Aucun slot
n'est écrit à la main : la table `BONE_SLOT` associe des préfixes d'os aux 5 slots,
et la géométrie suit.

29 îlots → 5 slots, somme conservée exactement (13 980 tris avant = après) :

| Slot | Îlots | Tris | Stat | Visible en jeu |
|---|---|---|---|---|
| `chest` | 12 | 5 526 | Blindage | non |
| `legs` | 7 | 3 448 | Puissance | non |
| `helmet` | 4 | 2 626 | Visée | non |
| `gloves` | 4 | 1 164 | Cadence | **oui** (viewmodel) |
| `boots` | 2 | 1 216 | Critique | non |

Les 5 slots reprennent ceux du nain (`assets/models/characters/dwarf/`) pour que
l'UI d'équipement soit la même si le nain revient.

## 3. Pièges payés

- 🚨 **210 Mo au premier export.** Chaque GLB embarquait sa copie des textures
  4096². Corrigé en glTF à fichiers **externes partagés** (`export_keep_originals`)
  + descente à 2048² → **18 Mo**, un seul upload GPU pour les 5 pièces.
- 🚨 **Unity `_RGBA` n'est pas un ORM.** R=metallic, G=occlusion, A=**smoothness**.
  La rugosité glTF est l'**inverse** du smoothness ; l'oublier donne un personnage
  en plastique verni. Repack numpy dans le script.
- 🚨 **`colorspace_settings` après `scale()` relâche le buffer redimensionné** et
  `save()` échoue sur « pas de données d'image ». Fixer l'espace colorimétrique
  AVANT de toucher les pixels.
- 🚨 **L'importeur glTF ajoute une Icosphère de 80 tris par fichier importé.**
  Vérifiée absente des `.gltf` (`grep -i icosph` = 0) : c'est le **compteur de la
  planche** qui mentait, pas l'asset. Filtré.
- 🚨 **`ortho_scale` cadre la plus grande dimension du rendu** — en portrait, la
  hauteur. Piège déjà documenté sur le nain, refait quand même : personnage coupé.
- Le manifeste annonçait `trooper_body` pendant que le fichier disait
  `Maillage.001` : le glTF exporte le nom de la **donnée**, pas de l'objet.

## 4. Le moteur

Un seul endroit calcule les modificateurs du joueur. `EquipmentMods` est composé
dans `PlayerCombatMods` par `boons_apply::sys_recompute_boon_mods`, au même titre
que boons / méta / maîtrise / Trempe — **aucune seconde voie** n'a été ouverte.
Le clamp anti-cheese des 85 % de réduction est calculé après l'apport des pièces.

Butin : 1 pièce par étage atteint + 1 à la victoire, tirage **dérivé de `RunSeed`**
(deux runs de même graine donnent le même butin). Anti-doublon léger par re-tirage
borné ; un tirage ne rend jamais « rien » silencieusement.

Capteur `forgia2_equipment.json` 1 Hz, alerte `EQUIPMENT_NO_DROPS` — un joueur
sans pièce ressemble à un joueur qui débute, donc l'absence de butin ne se voit
pas sans signal explicite.

## 5. Fichiers

| Fichier | État |
|---|---|
| `tools/blender/build_trooper.py` | neuf — découpe + textures + export |
| `tools/blender/preview_trooper.py` | neuf — planche de contrôle depuis les `.gltf` |
| `assets/models/characters/trooper/` | neuf — 6 `.gltf` + 8 textures, 18 Mo |
| `assets/genomes/roguelite/roguelite_equipment.toml` | neuf — corps, raretés, slots, butin |
| `crates/forgia-mode-roguelite/src/equipment.rs` | neuf — config, save, mods, butin, contenu du panneau, capteur |
| `crates/forgia-mode-roguelite/src/lib.rs` | modifié — `mod` + `EquipmentPlugin` |
| `crates/forgia-mode-roguelite/src/boons_apply.rs` | modifié — composition d'`EquipmentMods` |
| `crates/forgia-ui/src/weapon_preview.rs` | modifié — `CharacterPreviewRtt` (aperçu 3D, layer 4) |
| `crates/forgia-ui/src/lib.rs` | modifié — `sys_menu_equipement` (panneau + aperçu) |

## 6. Critères d'acceptation

- [x] Le Trooper est découpé en corps + 5 pièces, somme de tris conservée
- [x] Poids disque maîtrisé (18 Mo, textures partagées entre les 5 pièces)
- [x] Raretés / gains / poids de tirage / chemins de modèles : **zéro valeur en
      dur** dans le code
- [x] Les bonus passent par le point de composition existant
- [x] `cargo clippy` : **0 warning ajouté** (2 warnings préexistants hors
      périmètre : `forgia-stage:819` et `mode-roguelite/lib.rs:821`, tous deux
      issus de story-676, terminal parallèle)
- [x] 10 tests verts, dont « chaque modèle déclaré existe sur disque »
- [x] Butin, bonus, save et capteur **vérifiés au runtime** (§7)
- [ ] Panneau + aperçu 3D au menu : **non vérifiés visuellement** (§7 bis)
- [ ] Story-gate (`cargo run -p xtask -- story-gate --story 675`)

## 7. Le panneau était branché au mauvais endroit (corrigé)

Antoine : « je ne vois rien dans l'onglet Forgeron au lobby ». Diagnostic mené
en lançant le jeu (`FORGIA_BOOT_MODE=roguelite`) puis en lisant les capteurs.

**Ce que le butin et les bonus disaient** — tout marchait :

```
[equipment] bonus — dégâts ×1.04 cadence ×1.00 blindage 3% critique 3% (3 pièces)
{"panel_shown":true,"damage_mul":1.040,"damage_reduction":0.030,"crit_chance":0.030}
```

**Ce que la capture d'écran disait** — « Préparation de la Forge… », plein écran.

🚨 **Le Lobby n'est plus un hub interactif.** `forgia_ui::sys_lobby_loading_overlay`
y peint un overlay `Order::Foreground` qui recouvre tout, et `sys_auto_start_when_warm`
lance la run dès le warmup fini. Le hub à onglets a déménagé **dans le menu**
(`MenuPage::Forgeron`) — c'est là que vit le choix de couleur des bras, donc
exactement l'endroit désigné par la demande. J'avais branché le panneau au Lobby.

🚨 **Et mon capteur mentait** : `panel_shown: true` pour un panneau dessiné sous
un overlay opaque. Le drapeau est maintenant posé par `sys_menu_equipement`, donc
il signifie « réellement visible ». Un capteur qui rapporte le dessin plutôt que
la visibilité fait exactement ce que la famille IV des patterns de carte interdit.

**Corrections** : panneau déplacé dans `forgia_ui::sys_menu_equipement` (menu,
section Forgeron, bande droite), `draw_equipment_panel` supprimé (code mort),
`[portrait] enabled = false` — le portrait 3D se construisait dans la scène du
Lobby, jamais visible ; l'afficher au menu demande le rendu hors-écran comme
`ArmPreviewRtt`.

**Leçon générale** : la première question sur « je ne vois rien » n'est pas
« est-ce que ça tourne ? » mais « est-ce que ça tourne **là où on regarde** ? ».
Les capteurs disaient vrai sur le calcul et faux sur la visibilité ; c'est la
capture d'écran qui a tranché en une image.

## 7 ter. Passe UX — aperçu 3D et lisibilité (2026-08-01)

Demande : « améliore l'UX, je veux une prévisualisation comme dans un roguelite ».

**Ce que dit l'état de l'art** (sources en §9) : la convention de couleur de rareté
gris → vert → bleu → violet → orange/or vient de Diablo II puis World of Warcraft,
et s'est imposée telle quelle (Destiny, Apex, Fortnite, Borderlands). Notre TOML y
était déjà aligné. Trois principes en découlent, et ils commandent l'implémentation :

1. **La couleur se lit sans texte** — donc elle doit se voir *sur le personnage*,
   pas seulement sur une pastille d'interface. D'où l'aperçu teinté.
2. **Le paperdoll se met à jour en direct** — équiper une pièce doit se voir
   immédiatement, sinon le lien entre le clic et l'objet n'est pas fait.
3. **On compare avant de choisir** — un survol doit dire ce qu'on *gagne*, pas
   seulement ce que la pièce vaut.

**Aperçu 3D** — `forgia_ui::weapon_preview::CharacterPreviewRtt`, layer 4, à côté
de ceux des bras (layer 3) et des armes (layer 2). Le menu n'ayant pas de scène 3D,
c'est le seul chemin possible : une caméra dédiée rend le personnage dans une image
hors écran, affichée comme image egui dans le panneau. Corps + pièces équipées,
chaque pièce teintée à sa rareté, plateau tournant, re-cadrage automatique sur
l'emprise réelle à chaque changement d'équipement.

🚨 **La teinte est NORMALISÉE** (`rarity_tint`) : `base_color` *multiplie* la
texture d'albédo, donc appliquer le gris Commun brut (0,62) assombrirait l'armure
de 40 % au lieu de la colorer. On ne garde que la teinte, jamais la luminosité.
Même raisonnement que `apply_arm_style_glb` pour les bras.

**Panneau** — liseré de la couleur portée en tête de ligne, gain aligné à droite
dans cette même couleur, compteur « n / 5 emplacements », bilan réduit aux
statistiques réellement modifiées (une colonne de « +0 % » noie les deux lignes qui
comptent), et **delta au survol** : « Épique — Visée +24 % / Visée −16 % en
l'équipant ».

**Le portrait dans la scène 3D du Lobby a été SUPPRIMÉ** (131 lignes), pas
désactivé. Il ne pouvait pas fonctionner là — et le garder aurait fait deux
implémentations du même concept, exactement ce que `concept-first` cherche à
éviter. La section `[portrait]` du TOML disparaît avec lui : le RTT se cadre seul
sur l'emprise mesurée, un réglage de distance/hauteur aurait été une valeur à
maintenir en double avec la géométrie.

## 7 quater. Le bilan mentait au menu (corrigé)

Antoine : « ya pas de casque ni de gants ? ». Réponse factuelle : ils n'étaient
pas tombés — mais le capteur lu au passage montrait `equipped_total: 3` avec
**tous les bonus à 1.000**.

🚨 **`sys_recompute_equipment_mods` était gaté sur `in_state(GameMode::Roguelite)`
alors qu'on équipe au MENU.** Le panneau affichait donc « Aucun bonus actif » avec
trois pièces portées. Même classe que les deux défauts précédents : l'écran
rapportait autre chose que l'état réel. Le gate ne concerne que le butin, qui lit
`RunState` (SubState absent hors Roguelite) ; le recompute ne lit que ses propres
Resources et doit tourner partout.

### Cadence du butin — mesurée, pas estimée

Simulation du tirage réel (slot uniforme, rareté pondérée 60/28/9/2,5/0,5,
re-tirage anti-doublon ×8), 20 000 essais :

| | butins nécessaires |
|---|---|
| couvrir les 5 emplacements | **médiane 7**, moyenne 7,5 |
| 90ᵉ centile | 10 |
| pire cas observé | 16 |

Et le remplissage : après 3 butins il reste **2,36 emplacements vides en moyenne**
— exactement l'état d'Antoine (casque + gants). Rien d'anormal.

Une run complète donne **6 butins** (`total_stages` = 5 → 1 par étage + 1 à la
victoire), donc le set complet demande un peu plus d'une run entière. À revoir
seulement si le playtest le juge trop lent — c'est une décision de design, pas un
défaut.

## 8 bis. Le Hall passe à la 3ᵉ personne (2026-08-01)

Demande : « dans le hall de Forgia, vue 3ᵉ personne, avec le personnage portant
l'armure débloquée ».

Le Hall est le lieu où l'on se regarde ; le reste du jeu reste en vue subjective.
Le montage reprend trait pour trait la référence 3P du projet
(`forgia_rpg::character::spawn_rex_character`) :

1. la `FpsCamera` est **désactivée**, pas détruite — réactivée en sortant ;
2. l'`OrbitCamera` est une entité **séparée**, jamais enfant du joueur : enfant,
   elle hériterait de son lacet et ce ne serait plus de la 3ᵉ personne ;
3. le personnage est **enfant** du joueur → il suit position et lacet sans une
   ligne de synchronisation.

**Deux valeurs dérivées, aucune choisie** (les deux ont un test) :

- pieds à **−1,0 m** de l'origine du joueur — `Collider::capsule_y(0.7, 0.3)`
  fait 2,0 m centrés, et le modèle a son origine aux pieds (AABB 0 → 1,82 m) ;
- demi-tour de **π** — le modèle regarde +Z (vérifié au rendu de contrôle),
  l'avant de Bevy est −Z.

🚨 **Le viewmodel a sa PROPRE caméra.** Couper la `FpsCamera` ne l'éteint pas :
les bras seraient restés collés à l'écran par-dessus la vue 3P. Le mode RPG n'a
pas eu à traiter ce cas (pas de viewmodel) — c'est le piège propre au Hall.

🚨 **Le curseur et le contrôleur se contredisaient.** `orbit_cursor_grab`
relâche le curseur dès qu'aucun bouton n'est tenu, alors que `mouse_look`
tournait le personnage en permanence en dehors du RPG : bouger une souris
pourtant libre aurait fait pivoter le perso. La condition du pattern WoW suit
désormais la **caméra** et non un mode : `Rpg | CastleHub`. Non observé, déduit
de la lecture des deux systèmes — à confirmer manette/souris en main.

### Deux erreurs de méthode, dans le même tour

🚨 **Lancer `forgia.exe` directement casse TOUT le chargement d'assets, en
silence.** La racine d'assets de Bevy est relative au binaire hors cargo, et
`target/<profil>/assets` n'existe pas. Le jeu démarre pourtant, le ciel
(procédural) s'affiche, les capteurs sont justes, et surtout `assets.load()` ne
crée qu'un *handle* : `[castle-avatar] avatar monté — 5 pièce(s)` s'écrit alors
que rien ne sera visible. J'ai conclu d'une capture noire que la feature ne
marchait pas. Trancher coûte 5 s : `grep -c "Path not found"`.
→ [[reference-forgia-asset-root-is-exe-relative]]

🚨 **Et j'ai annoncé « vérifié » sur un log de spawn.** Le log prouvait que le
système avait tourné, pas que le modèle s'affichait. Un log de spawn n'est pas
une observation du rendu.

### Le cadrage — cause dérivée, pas devinée

Relancé correctement, le Hall s'affiche (0 erreur, 46 cellules, joueur placé) —
mais toujours sans personnage, vue à hauteur d'œil.

`OrbitCamera::new` pose `height_offset = 1.8`, documenté « hauteur d'épaule »,
ce qui suppose une cible dont l'origine est aux **pieds**. Notre `Player` a la
sienne au **centre de sa capsule**, 1 m plus haut : la caméra visait 2,8 m au-
dessus du sol, un mètre **au-dessus de la tête**. Le rayon anti-clip
(`orbit_follow`, `dist = (toi − 0,3).max(0,2)`) part du plafond, touche
immédiatement, et le bras se rétracte à 20 cm — caméra dans le crâne.

Corrigé en visant l'épaule (82 % de la hauteur mesurée du modèle, ramenés dans le
repère du joueur), avec un test qui refuse toute visée hors de la silhouette. Le
défaut du défaut partagé `OrbitCamera::new` **n'est pas corrigé** : le RPG a
probablement le même décalage, mais ce n'était pas la demande.

### Vérifié à l'image (2026-08-01)

Capture du grand hall : le personnage est là, de dos, à ~7 m, debout au sol,
portant ses cinq pièces — casque **bleu** (Épique), plastron **gris** (Commun),
gants et jambières **verts** (Rares), bottes teintées. La rareté se lit d'un
coup d'œil, sans texte, ce qui était tout l'objet. 109 FPS.

Capteur au même instant : `mesh_count:6 · visible_meshes:6 ·
camera_distance_m:7.41 · severity:ok`.

Deux observations pour le playtest, pas des défauts :

- **L'émissif n'est pas teinté** (`base_color` ne multiplie que l'albédo) : les
  liserés sci-fi restent orange quelle que soit la rareté. Ça préserve l'identité
  du personnage, mais dilue un peu le signal couleur.
- **La teinte est franche.** Normalisée à max=1, un vert Rare donne
  `(0.43, 1.0, 0.51)` — l'armure lit « verte » plus que « acier à reflet vert ».
  Lisible, mais à juger à l'œil.

### Le capteur qui manquait

`forgia2_castle_avatar.json` — sur « je ne vois pas le personnage », trois causes
qu'aucune capture ne distingue, désormais mesurées : `AVATAR_NO_MESH` (le glTF
n'a pas chargé), `CAMERA_COLLAPSED` (bras à ressort sous 1 m), ou `ok` avec la
distance caméra. C'est exactement ce qui aurait évité les deux erreurs ci-dessus.

**Le montage de l'avatar est extrait** dans `forgia-mode-roguelite::avatar` :
l'aperçu du menu et le Hall en sont maintenant deux consommateurs, et dupliquer
le spawn + la teinte aurait garanti qu'ils divergent. C'est l'étape 5 de
`concept-first` appliquée avant que le doublon existe, pas après.

## 8 ter. Animations, contact au sol, inventaire touche I (2026-08-01)

### Le pack ne fournissait AUCUNE animation

🚨 Son unique clip « Take 001 » anime bien 69 os — **avec des valeurs
constantes**. Frames 0 et 53 rendues : identiques. C'est une A-pose tenue 3,37 s.
Un clip qui existe n'est pas un clip qui anime ; seul le rendu l'a montré.

`idle` (respiration répartie sur deux vertèbres) et `walk` (1 s bouclée, foulée
26°, genou plié vers l'arrière uniquement, bras à contre-temps des jambes) sont
donc construits dans `build_trooper.py`. Les deux partent **bras baissés** :
l'A-pose du pack lit « asset ».

🚨 **Les clips vont dans les SIX fichiers.** Chaque pièce embarque sa propre
armature : animer le seul corps l'aurait fait sortir de son armure restée figée.
Le runtime joue le même clip sur les six lecteurs. Alerte `AVATAR_ANIM_PARTIAL`
si l'un d'eux reste muet — c'est le mode de défaillance que cette structure crée.

Poses écrites en repère **armature** (`M⁻¹ · R · M`), jamais en repère local d'os
— le piège « bras derrière le dos » du nain.

### Contact au sol : mesuré, puis dérivé

`ground_gap_m: 0.045` — l'avatar flottait de 4,5 cm, soit exactement l'`offset`
que le contrôleur cinématique maintient entre capsule et sol. Le système **lit**
`KinematicCharacterController.offset` et compense d'autant, plutôt que de
recopier la constante. Après correctif : `-0.005`.

### Touche I

`KeyI` était libre (vérifié) : un seul gestionnaire, gaté sur le Hall — anti-trap
« 2 handlers ESC ». Le panneau reprend `draw_equipment_content`, curseur rendu à
l'ouverture et repris à la fermeture.

Pour « voir les armures disponibles pour les prochaines runs », **une seule vue**
plutôt que deux écrans : le panneau affiche toute l'échelle de raretés par
emplacement — pastilles pleines pour ce qu'on possède, **contours** pour ce qui
reste à trouver, avec le gain annoncé au survol. Le menu en hérite.

### Vérifié

```json
{"mesh_count":6,"visible_meshes":6,"anim_players":6,"anim_playing":6,
 "ground_gap_m":-0.005,"camera_distance_m":2.70,"severity":"ok"}
```

Capture : bras le long du corps, pieds au sol, ombre portée, armure teintée.

**Non vérifiable sans clavier** : la bascule idle ↔ walk (`walking` dans le
capteur) et le confort de la marche. L'amplitude (26° de cuisse) et la cadence
(1 s) sont des constantes nommées de `build_trooper.py` — 2 min de rebuild
d'assets pour les changer.

## 8 quater. Squelette partagé — la dislocation rendue impossible (2026-08-01)

Antoine, capture à l'appui : les brassards flottaient à l'écart des bras. Le
corps avait baissé les bras, l'armure était restée écartée.

**Ce que la mesure a écarté d'abord.** Les clips de `body.gltf` et `gloves.gltf`
étaient **identiques au bit près** sur `upperarm_l` et `lowerarm_l` — l'asset
n'était pas en cause. Et mon capteur annonçait `anim_playing: 6` : il **mentait
par imprécision**, comptant des lecteurs actifs et non des os qui bougent.

**La cause.** Chaque fichier glTF embarque l'armature complète — contrainte du
format, un maillage skinné ne peut pas référencer un squelette externe. Six
squelettes jouant six copies du même clip ne restent pas synchronisés.

**La correction n'est pas de les synchroniser, c'est de n'en garder qu'un.** Les
pièces conservent leur maillage et rebranchent leurs joints sur les os du corps ;
leurs propres os sont supprimés. Et les clips ne sont plus exportés que dans
`body.gltf` : une pièce sans animation **ne peut plus** diverger. Le défaut passe
de « surveillé par une alerte » à « structurellement impossible ».

**Condition de correction, vérifiée avant d'agir** : les six fichiers sortent de
la même armature au même repos — matrices de liaison inverses identiques (SHA1
`92aa24a0`) et même ordre de joints. Le rebranchement se fait quand même **par
nom** : un ré-export qui changerait l'ordre échouera bruyamment au lieu de tordre
le personnage en silence.

### Mesuré, avant → après

| | avant | après |
|---|---|---|
| `anim_players` | 6 | **1** |
| `bone_copies` (copies de `hand_l`) | 6 | **1** |
| `desync_m` | — | **0.000** |
| `visible_meshes` | 6 | 6 |

Confirmé à l'image : brassards sur les avant-bras, bras le long du corps.

Effet de bord bienvenu : 5 × 68 = **340 transforms d'os inertes** en moins à
propager chaque frame.

## 7 bis. Reste à valider en jeu

1. Le panneau ÉQUIPEMENT apparaît-il au menu, section Forgeron, sans chevaucher
   le hero « TON FORGERON » (ancré centre +100, large de 460) ?
2. Cliquer une pastille change-t-il les bonus affichés et le save ?
3. Le butin continue-t-il de tomber par étage (`drops_total` monte) ?

## 8. Hors périmètre, assumé

- **Les bras du viewmodel restent les bras procéduraux existants.** Remplacer
  `forgia-viewmodel::arms` par les bras du Trooper est un chantier distinct
  (804 LOC, calibration, poses) ; les gants équipés ne se voient donc pas encore
  en combat, seulement sur le portrait. C'est le principal écart avec la demande.
- Pas de LOD, pas de KTX2 : 8 textures 2048² restent ~536 Mo de VRAM non
  compressée si tout est résident. À traiter si le portrait coûte cher.
- Les autres personnages (`Dorin`, `Rex`, `Mira`…) ne deviennent pas des skins :
  ce sont des mesh fusionnés sans armature, inexploitables comme pièces modulaires.

## 9. Cross-refs

**Sources externes (passe UX)** — convention de couleur de rareté et comparaison
d'objets :

- [How Color Theory Helps Codify Item Quality in Video Games](https://medium.com/@ClaireFish/how-color-theory-codifies-item-quality-in-video-games-104d8118044) — origine WoW, pourquoi le violet = épique
- [Color-Coded Item Tiers — TV Tropes](https://tvtropes.org/pmwiki/pmwiki.php/Main/ColorCodedItemTiers) — la constance de l'échelle d'un jeu à l'autre
- [Color-Coded Loot — Giant Bomb](https://www.giantbomb.com/color-coded-loot/3015-4702/) — Diablo II puis WoW comme point d'origine
- [Game UI Database](https://www.gameuidatabase.com/index.php?tag=13) — captures d'écrans d'inventaires (Hades, Hades II, Dead Cells)
- [Devlog — Preparation Scene (UI + paperdoll)](https://tabulaforge.itch.io/chains-on-sand/devlog/946548/devlog-15-video-first-look-at-the-preparation-scene-ui-music-paperdoll) — le paperdoll qui se met à jour en direct
- [Game UI: design principles and best practices](https://www.justinmind.com/ui-design/game) — hiérarchie visuelle, la taille comme signal
- [RPG Game Design — Fundamentals](https://gamedesignskills.com/game-design/rpg/) — armure lisible au coup d'œil, rareté comme axe de design

- `.claude/rules/no-hardcode.md` — tout le chiffrage en couche definition
- `.claude/rules/observability-required.md` — capteur + alerte à next-step
- `reference_procedural_dwarf_modular_armor` (mémoire) — mêmes 5 slots, mêmes pièges
- `feedback_antoine_refuse_bases_externes_personnage` (mémoire) — la voie « base
  externe » avait été refusée deux fois ; c'est Antoine qui a désigné ce pack,
  l'exception prévue par ce feedback.

---

## 10. Passe « Puissance » — un nombre pour suivre sa progression (2026-08-02)

**Demande** : « J'aimerais que les avantages (dégâts, visée, etc.) te donnent un
montant qui te permette de suivre ton évolution. »

**Le problème que ça résout.** Le bilan affiche cinq pourcentages qui portent sur
des choses différentes (dégâts, cadence, réduction, critique, visée). Ils disent
le *profil* d'un build, mais rien ne dit si celui d'aujourd'hui vaut mieux que
celui d'hier — et deux pièces d'emplacements différents restent incomparables.

**La Puissance** = Σ des rangs de rareté portés, sur `slots × rarities` (25 ici).
`equipment::power_score()`, dérivé du **même** `equipped` que `compute_mods` — pas
de seconde vérité.

**Pourquoi le RANG et non le gain de statistique** — trois raisons, dans l'ordre :

1. c'est le rang que le joueur progresse : trouver plus rare est le geste du jeu,
   alors qu'un `per_tier` est une décision de game design ;
2. un score dérivé des pourcentages **bougerait à chaque rééquilibrage**, sans
   qu'aucune pièce n'ait changé — le record d'hier deviendrait faux ;
3. additionner cinq pourcentages hétérogènes ne veut rien dire physiquement.

Le dénominateur se **dérive** du genome (`EquipmentConfig::power_max()`) : ajouter
un emplacement ou une rareté déplace la cible tout seul.

**Livré**

| Endroit | Ce qu'on voit |
|---|---|
| Panneau Forgeron | `PUISSANCE  14 / 25  record 19` + jauge (liseré sombre = record) |
| Survol d'une pastille | `+3 Puissance` — rend comparables deux emplacements |
| Carte de butin | `Puissance 12 → 15`, ou `15 → 18 si équipée` quand l'emplacement est déjà rempli |
| `forgia2_equipment.json` | `power`, `power_max`, `power_record` |

`EquipmentSave.power_record` (scalaire placé **avant** les tables, cf. §Sauvegarde)
retient le pic : sans lui, retirer une pièce effacerait l'histoire.

## 11. Correction — l'armure ne suivait pas le personnage

**Symptôme rapporté** : « quand je change l'équipement dans le lobby, l'armure ne
suit pas le personnage » (il tourne, elle reste).

**Fausse piste écartée par lecture** : j'avais supposé qu'un ancien corps
coexistait une frame avec le neuf pendant la reconstruction. Faux —
`weapon_preview` despawne et respawne dans la **même file de commandes**, donc
aucun système n'observe les deux.

**Cause réelle** : `sys_share_body_skeleton` prenait `q_body.iter().next()`, *un*
corps arbitraire. Deux avatars peuvent vivre en même temps (aperçu du menu et
avatar du Hall) — les pièces de l'un se rebranchaient alors sur le squelette de
l'autre. Une fois `SkinnedMesh.joints` pointant ailleurs, la pièce suit un corps
qui n'est pas le sien : elle reste immobile pendant que le personnage tourne.

**Fix** : appariement par **parent commun** (`HashMap<parent, body>`), avec cache
des os par corps. Le garde `continue` posé la veille était insuffisant *et*
nuisible : il affamait le second avatar, dont les pièces n'auraient jamais trouvé
leur tour. C'est l'appariement qui devait être juste, pas le filtrage.

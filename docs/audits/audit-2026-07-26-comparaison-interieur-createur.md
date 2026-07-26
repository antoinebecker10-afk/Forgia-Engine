# Comparaison — notre intérieur de château vs celui du créateur (2026-07-26)

Comparaison de deux captures de **la même salle** (double escalier courbe, griffon
sur son socle entre les deux volées, voûtes d'arêtes, portes en arc sous la
plateforme) : la nôtre en jeu (84 FPS / 11,8 ms) et celle du créateur du pack.

Méthode : chaque écart constaté à l'image est **confirmé ou écarté par une mesure**
sur les fichiers exportés. Rien n'est conclu de l'œil seul.

---

## 0. Verdict

L'écart n'est **pas géométrique**. C'est un écart **d'éclairage**, et la cause
dominante n'était pas dans mon audit précédent :

| # | Cause | Poids | Statut |
|---|---|---|---|
| 1 | **Aucun lightmap** — l'UV2 est là sur 100 % des meshes, les textures cuites n'ont jamais été exportées | 🔴🔴 dominant | à faire |
| 2 | **Aucune lumière locale ni flamme** (particules et lights Unity perdues à l'import) | 🔴 fort | ✅ corrigé (§8) |
| 3 | **Aucun étalonnage couleur en `CastleHub`** — `grade_for()` renvoie `None` | 🟠 moyen | ✅ corrigé (§8) |
| 4 | **Ambiante à 900** qui écrase le peu de contraste restant | 🟠 moyen | ✅ corrigé (§8) |
| 5 | Tangentes absentes (micro-relief) | 🟡 | ✅ corrigé |

---

## 1. 🔴🔴 La découverte : la scène est faite pour être lightmappée

**Mesure** : `TEXCOORD_1` est présent sur **1321 primitives sur 1321**, soit 100 %.

Un deuxième jeu d'UV sur *toutes* les surfaces d'un décor, ça ne sert qu'à une
chose : recevoir un **lightmap** (éclairage global cuit hors-ligne). Le pack a donc
été authoré pour Unity avec GI bakée — et la capture du créateur en est la
démonstration : ombres de contact sous chaque marche, dégradé chaud autour de
chaque bougie, et surtout du **rebond coloré** (le tapis rouge repeint le dessous
des voûtes en rouge-brun, la pierre prend un bleu-vert d'ambiance).

**Ce que nous avons** : les UV2 sont exportés, mais **aucune texture de lightmap**,
et aucun composant `Lightmap` côté Bevy. Toute l'illumination indirecte cuite est
donc perdue. Il ne reste que 2 lumières directionnelles + une ambiante constante.

**Preuve par le plafond** — c'est le test qui tranche. Les voûtes du créateur sont
rouge-brun ; les nôtres blanches. Or les pièces de plafond utilisent
`M_MOD_ceiling_plaster_castle`, dont le `baseColorFactor` est **blanc** (aucune
teinte) et la couleur de base est un plâtre pâle. Aucune de nos 69 primitives de
plafond ne porte de `COLOR_0` (couleurs de sommets) : il n'existe donc **aucune
donnée dans notre export** capable de rougir ce plafond. Notre plafond blanc est
*correct* pour ce matériau. Le rouge du créateur ne peut venir que du **rebond
lumineux cuit** dans son lightmap.

Autrement dit : ce n'est pas une texture qui manque au plafond — c'est la lumière.

---

## 2. 🔴 Ni flammes ni lumières locales

Déjà établi dans `audit-2026-07-26-castle-hub-manques.md` : 0 nœud
flamme/feu/lumière, 0 matériau émissif sur 28, parce que la reconstruction n'a
gardé que les porteurs de mesh (les `ParticleSystem` et `Light` Unity sont partis).

L'écart visuel est frappant : chez le créateur **chaque applique et chaque
chandelier porte une flamme** qui projette une flaque de lumière chaude sur le mur
derrière. Nos 203 bougeoirs sont là, mèche nue, sans halo.

À noter aussi chez lui, et absents chez nous pour la même raison : des **poussières
en suspension** (particules) et des **vitraux qui rayonnent** (émissif). Nos vitraux
utilisent `M_MOD_glass_castle`, un masque gris à 50 % d'alpha sans émissif — il ne
peut que rester terne.

---

## 3. 🟠 Le Hall n'a aucun étalonnage couleur

**Mesure** : `crates/forgia-game/src/color_grading.rs` ne traite que
`GameMode::Roguelite`, `CyberCity` et `Rpg`. Pour `CastleHub`, `grade_for()` retourne
`None` → **aucun `ColorGrading` appliqué**.

La capture du créateur est franchement étalonnée : ombres poussées vers le
bleu-vert, lumières vers l'orange. C'est ce contraste chaud/froid qui fait « lire »
le volume. Notre image est neutre et désaturée — chaque surface tire vers le gris
clair, donc plus rien ne se détache.

C'est le correctif le moins cher du lot : une entrée de plus dans
`color_grading.toml` et un bras de plus dans le `match`.

---

## 4. 🟠 L'ambiante à 900 est une compensation qui coûte cher

`castle_hub.rs` : `AmbientLight { brightness: 900.0 }`, et l'historique du
commentaire dit tout — 700 (« trop sombre ») → 1600 (« cramait la roche ») → 900.

Ces allers-retours sont le symptôme de 1 et 2 : sans GI cuite ni lumières locales,
la seule façon de voir l'intérieur était de monter le remplissage ambiant. Mais une
ambiante n'a **pas de direction** : elle donne la même valeur en chaque point, donc
elle *supprime* le modelé au lieu de le révéler. C'est exactement l'aspect « décor
en plastique blanc » de notre capture.

Tant que 1 ou 2 n'est pas fait, la baisser rendrait le Hall inutilisable. Après,
c'est elle qui rendra la profondeur.

---

## 5. Ce qui n'est PAS en cause

- **La géométrie.** Même salle, mêmes escaliers, même griffon, mêmes voûtes, mêmes
  arcs, mêmes balustrades, mêmes chandeliers. Le capteur confirme 46/46 cellules
  chargées, 8 560 meshes.
- **Les textures.** 61 présentes, aucune URI cassée, un seul matériau sans couleur
  de base et c'est volontaire (falaise en couleurs de sommets).
- **Les tentures.** L'export contient 33 tentures + 10 bannières + 84 tapis + 22
  tableaux. Elles sont surtout groupées dans les cellules `x-1_*` et `x-2_*` ; notre
  cadrage coupe les murs latéraux, donc **je ne peux pas conclure de la capture
  qu'elles manquent**. À vérifier en jeu en pivotant sur place.
- **La performance.** 84 FPS / 11,8 ms : il reste du budget pour des lumières.

## 6. Le seul écart de contenu possible

Le créateur a un **grand motif circulaire incrusté au sol** de la salle. Aucun nœud
de l'inventaire n'y correspond (pas de `rosette`/`mosaic`/`deco_floor`). C'est
très probablement un **décalque Unity** (`Decal Projector`) — donc un objet non-mesh,
perdu par le même mécanisme que les flammes. À reproduire à la main si tu le veux.

---

## 7. Plan, réordonné

| # | Action | Gain | Effort | Risque |
|---|---|---|---|---|
| **C'** | **Étalonnage `CastleHub`** dans `color_grading.toml` (ombres froides, lumières chaudes) | Immédiat sur l'ambiance générale | **15 min** | Nul, donnée |
| **B** | **Flammes + lumières** sur les 203 bougeoirs : quad émissif + `PointLight` courte portée, **plafonné aux N plus proches du joueur** | Rend les bougies vivantes ET éclaire là où il faut. Débloque C | ~2 h | Perf si non plafonné |
| **C** | Baisser l'ambiante (900 → ~250) une fois B en place | Rend le modelé et la profondeur | 15 min | Nul, réglage |
| **F** | **Lightmaps** : extraire les textures cuites du pack Unity + l'index/scale-offset par renderer, et poser le composant `Lightmap` de Bevy | Le vrai rendu du créateur, rebond coloré compris | **1-2 j** | Élevé : mapping par renderer, format EXR, remise en cause du découpage en cellules |
| **D** | Cartes métal/rugosité manquantes (dont le plafond) | Variation de brillance | ~1 h | Faible |
| **E** | Diff avec le pack source (chemin du `.unitypackage` requis) | Chiffre les meshes réellement perdus | 30 min | Nul |

**Recommandation** : **C' → B → C** d'abord. Ces trois-là coûtent ~2 h 30 au total
et récupèrent l'essentiel du ressenti (contraste chaud/froid, bougies vivantes,
volume lisible). **F** est le seul chemin vers un rendu réellement identique, mais
c'est un chantier d'un à deux jours et il faut le décider en connaissance de cause —
pas le lancer par réflexe parce que « c'est ce que le créateur a fait ».

---

## 8. Correctifs C' + B + C — appliqués (2026-07-26)

### C' — Étalonnage du Hall

`color_grading.rs` traitait Roguelite / CyberCity / Rpg mais pas `CastleHub` :
`grade_for()` renvoyait `None`, donc aucun `ColorGrading`. Ajout d'un bras et d'une
section `[castle_hub]` dans `assets/genomes/color_grading.toml` (hot-reload) :
exposition −0,10 · température +0,12 (lumières chaudes) · tint −0,04 (ombres vers
le vert-bleu) · saturation et contraste relevés. C'est le contraste chaud/froid de
la référence du créateur.

### B — Bougies allumées

Nouveau module `crates/forgia-game/src/castle_flames.rs` (~560 LOC).

Chaque nœud dont le nom contient `_candle` (les 98 bougies **et** les 203 chandeliers
et appliques, dont la bougie est intégrée au mesh) reçoit une **flamme émissive**
posée au sommet de sa boîte englobante — donc automatiquement à la bonne hauteur,
qu'il s'agisse d'un chandelier au sol ou d'une applique murale.

Trois décisions structurantes :

1. **Flammes en entités racines, pas en enfants.** Une flamme enfant hériterait de
   l'échelle de son parent — or les pièces du pack ont des échelles variées, y
   compris **miroir** (négative) : la flamme prendrait une taille arbitraire. En
   racine, sa taille est celle demandée, et son cycle de vie est explicite (retirée
   quand son ancre disparaît avec le déchargement de sa cellule).
2. **Seules les N flammes les plus proches portent une `PointLight`** (24 par
   défaut). ~300 lumières dépasseraient le budget du rendu groupé de Bevy. Le
   composant est posé et retiré **aux seules transitions**, sur un reclassement à
   5 Hz — pas à chaque frame. Ombres portées désactivées : 24 lumières à ombres
   coûteraient bien plus que ce qu'elles apportent sur une bougie.
3. **Le placement attend la géométrie.** Au moment où le nœud d'une cellule
   apparaît, sa scène glTF n'est pas encore instanciée : sa boîte englobante — donc
   le sommet de la bougie — est inconnue. Le marqueur `NeedsFlame` persiste et
   réessaie, avec un plafond pour ne pas boucler sur un asset absent.

Vacillement : l'intensité de chaque flamme oscille avec une **phase dérivée de son
identifiant d'entité** — deux bougies voisines ne battent pas ensemble, et le
résultat est reproductible (pas d'aléatoire).

### C — Ambiante redescendue et rendue réglable

L'ambiante à 900 était codée en dur, après un historique 700 « trop sombre » →
1600 « cramait la roche » → 900. Ces allers-retours étaient le **symptôme** de
l'absence de lumières locales, pas un mauvais réglage.

Elle passe à **250** et vit désormais dans `assets/genomes/castle_hub_lighting.toml`
avec les réglages de flammes, relu à chaud (1 Hz) : ce réglage se juge à l'œil en
jeu, il appartient à la donnée. Teinte légèrement froide (`[0.72, 0.80, 0.92]`) pour
que le contraste avec les bougies chaudes fasse lire le volume.

### Observabilité

Capteur `forgia2_castle_flames.json` (1 Hz) : nombre de flammes, nombre allumées,
en attente de géométrie, plafond de lumières, ambiante, rechargements. Alerte
`critical` si le Hall est actif, les flammes activées, et **zéro flamme posée** —
avec l'action de remédiation (vérifier que les nœuds portent bien `_candle`).

### État de vérification

`cargo clippy -p forgia-game --all-targets` : **0 warning** (le code de test
compile). Les 6 tests unitaires de `castle_flames` (parsing TOML partiel, TOML
cassé, fichier livré, ambiante négative, sévérités) **n'ont pas été exécutés** :
lancer `cargo test -p forgia-game` demande un très gros rebuild. À faire au
prochain passage.


---

## 9. Le vrai défaut, trouvé sur capture runtime (2026-07-26, après G+H)

Constat à l'écran : deux murs **identiques côte à côte**, l'un blanc éclatant,
l'autre presque noir. Sol en damier clair/sombre. Ce n'était pas un manque
d'ambiance — c'était un défaut d'éclairage.

**Hypothèse écartée par la mesure** : normales inversées sur des pièces miroir.
Comptage sur les 46 cellules → **0 nœud à échelle négative sur 7 911**. Faux.

**Cause réelle**, lue dans `castle_hub.rs` :

```rust
DirectionalLight { illuminance: 7_000.0, shadows_enabled: false, .. }  // fill
```

Le remplissage ciel à **7 000 lux n'a pas d'ombres** : il traverse murs et toit et
éclaire l'intérieur comme si le château n'existait pas. Chaque surface est donc
éclairée par sa **seule orientation** vis-à-vis d'une lumière qui ignore toute la
géométrie — d'où deux murs jumeaux aux luminosités opposées.

Aggravant : le soleil principal porte bien des ombres, mais avec les **cascades
par défaut**, qui ne couvrent pas les 193 m d'emprise du château. Les intérieurs
lointains recevaient donc le soleil à pleine puissance *à travers le toit*.

Ces deux défauts existaient depuis toujours ; l'ambiante à 900 les masquait en
noyant tout. La baisser à 250 les a révélés — la baisse n'est pas la cause.

### Correctifs

| Réglage | Avant | Après |
|---|---|---|
| Remplissage (sans ombres, traversant) | 7 000 lux | **600 lux** |
| Soleil principal | 20 000 lux, cascades par défaut | **12 000 lux**, 4 cascades jusqu'à **420 m** |

Les deux vivent désormais dans `castle_hub_lighting.toml` (`key_lux`, `fill_lux`),
en hot-reload : ces valeurs se jugent à l'œil, en jeu.

### Ce qui reste, et pourquoi

L'aspect « pierre mouillée » du sol sur les captures vient des **31 sondes de
réflexion absentes** (correctif K) : une surface PBR avec une carte de rugosité et
aucun environnement à réfléchir tombe sur un reflet plat et uniforme.

Et l'écart de fond avec la référence reste les **lightmaps** (F) : sans rebond
cuit, tout ce qui n'est pas frappé directement par une lumière tombe au plancher
de l'ambiante. C'est structurel, pas un réglage.

# Audit — observabilite du combat 3P Expedition

> 2026-08-17. 11 agents, 15 defauts juges, contradiction adversariale sur
> chaque pretention de couverture. Question posee : « a-t-on l'observabilite
> pour comprendre tous les bugs crees avec les nouvelles features ? »

# Observabilité du combat 3P Expédition — verdict

## 1. La réponse

**Non.** Sur les 9 bugs réellement créés cette session et jugés pièce par pièce, **1 seul est diagnosticable depuis un capteur** (`forgia2_crash.previous.json::message`, id 8) — et encore, il a été détecté par `bevy_ecs-0.18.1/src/system/system_param.rs:851`, pas par un capteur Forgia, qui n'a fait que le transcrire (`crates/forgia-observability/src/qa_bridge.rs:44`). En ajoutant les 6 défauts trouvés pendant l'audit lui-même : **1 sur 15**. Cinq des neuf capteurs concernés ne sont pas muets, ils sont **menteurs** : ils publient `severity: "ok"` sur l'état exact du bug.

---

## 2. Les 15 bugs

Bugs 1 à 11 = les défauts de la session (ids 2 et 7 absents du matériel jugé). Bugs A à F = défauts constatés pendant l'audit, non encore rapportés en jeu.

| # | Bug | Verdict | Champ qui existe | Ce qui manque |
|---|---|---|---|---|
| 1 | L'arme pointait vers l'arrière | **PARTIEL** | `forgia2_expedition_arme.json::ecart_visee_deg` (`arme_main.rs:985`, calcul `:914-923`, seuils `:970`/`:975`) | Branche `AVEUGLE` avant le repli `("ok","")` `:981` ; `ecart_visee_mesure:bool` et `depuis_entree_s` (calculé `:884`, jamais publié) ; seuils 90.0/35.0 sortis du Rust vers `[visee]` de `expedition_arme_main.toml` |
| 3 | Aucune arme visible (corps sans `socket_*`) | **MENTEUR** | `ancrage`, `socket_trouve` (`arme_main.rs:987-988`) | `ANCRAGE_ABSENT` (`:945`) est **inatteignable** : `:452-458` retombe sur l'os `RightHand` et pose `SocketMain`, donc `socket_trouve:true` → `ok`. Manque `ancrage_est_repli:bool` + branche `ANCRAGE_DE_REPLI`, et `corps_modele` (aujourd'hui seulement dans `forgia2_castle_avatar.json::corps[].modele`, `castle_avatar.rs:120`) |
| 4 | Capteur qui accuse la mauvaise cause | **PARTIEL** | `next_step` (`arme_main.rs:985`) | Champ `cause` machine séparé de la prose ; extraire `:931-982` en fonction pure `juger(...)` + test par table. **Zéro test** ne touche la chaîne de sévérité (grep sur `:1129-1437` = 0 hit) |
| 5 | La pose s'accumulait (bras en hélice) | **MENTEUR** | `ecart_buste_deg` publié `visee.rs:1020`, jugé par personne ; `forgia2_animation.json::cibles_touchees` (62/83, `severity:"ok"`) | `pose_derive_max_deg` dans la boucle `visee.rs:611-638` ; `cibles_non_touchees[]` (`anim_sensor.rs:180-184` les a en main, `:210` n'en garde que le compte) ; `ecart_buste_deg` mesure un lacet sur l'axe long de l'os (`:967-980`) au lieu du tangage réellement appliqué (`:587-588`) |
| 6 | Les bras tournaient sur eux-mêmes | **MENTEUR** | `forgia2_expedition_visee.json::severity` = `"ok"` avec `ecart_main_droite_m:-1.000` | `ecart_visee_deg` est **saturé** : 145,1 / 145,6 / 148,2 / 144,4 / 145,0 / 147,4 / 147,7° en critical continu sur 40 s, joueur immobile — parce qu'il mesure contre la capsule (`arme_main.rs:879,919`) et non contre la caméra (`visee.rs:969-975`). Manque `ecart_pose_max_deg` + `ecart_pose_croissance_deg_s` |
| 8 | Panique B0002 au démarrage | **COUVERT** | `forgia2_crash.previous.json::message` = « ResMut\<Visee\> in system `capteur_visee` conflicts with a previous Res\<Visee\> » | Rien pour observer. Pour empêcher : le harnais `visee.rs:1062-1063` ne monte que 2 des 5 plugins du mode ; `avatar_vfx.rs:70`, `posture.rs:107`, `matiere.rs:109` ne sont montés par **aucun** `App::new()` du dépôt |
| 9 | Personnage 100× trop grand | **PARTIEL** | `forgia_bone_trace.json::personnages[].os[].monde` (`bone_trace.rs:391`) | **Aucun seuil** : `ecrire_sante` (`:407-434`) ne lit que l'`ecart_m` des maillages. Manque `os_le_plus_long_m`, `hauteur_squelette_m`, `os_surveilles_trouves` (les 12 noms Mixamo font 0 touche sur un rig UE, tous `surveille:false`) |
| 10 | Dérive de la racine jusqu'à 6,8 m | **MENTEUR** | `forgia2_castle_avatar.json::ground_gap_m` = 0.002, `desync_m` = 0.000, `severity:"ok"` | Les deux se mesurent sur la **racine de scène** (`castle_avatar.rs:478,492-501`), qui reste collée à la capsule pendant que le corps suit `Hips`. `DESYNC_PROBE_BONE="hand_l"` (`:288`) est absent du rig Mixamo → `bone_copies:0` lu comme « synchronisé ». Manque `derive_racine_m` |
| 11 | Correction de dérive sur le mauvais axe | **MENTEUR** | *aucun champ publié* | `mixamo_recolte.py` n'écrit que 3 fichiers (`:259`, `:509`, `:523`) — zéro JSON. Pire : `:585-589` imprime « ✓ dérive racine retirée X m » avec `pire` accumulé `:485` **avant** la correction `:486-491` → le chiffre ne peut pas baisser quand la correction échoue. `:467-468` rend `0.0` muet si aucun nœud `Hips` |
| A | Les 25 clips fusil viennent d'un **autre** personnage | **MENTEUR** | `SEUIL_OS_M = 2.0` (`mixamo_recolte.py:289`) | Mesuré : LeftArm ×1,529, Head ×0,370, LeftForeArm ×0,809 ; 61 os sur 62 divergent de >1 mm ; les 9 clips d'origine collent à 0,2 %. Le seuil mesure une taille **absolue** (1,06 < 2,0 → vert) et n'ouvre jamais le corps cible : le ratio, seule grandeur qui décide, n'est jamais calculé |
| B | 43 animations dans le corps, 9 noms en **double** | **MENTEUR** | *aucun* | `merge_gltf_anims.py:176-178` append sans consulter les noms existants — alors que le même contrôle existe pour les **nœuds** (`:84-93`). `bevy_gltf-0.18.1 loader/mod.rs:556-558` fait un `insert` nu : le dernier gagne. `jump_clip="jump"` (`roguelite_equipment.toml:248`) résout vers l'anim #17, jamais #5. Le génome affirme « 34 au total, 0 ignoré » (`:212-214`) : faux |
| C | Cheveux animés désactivés de fait | **AVEUGLE** | *aucun* | `merge_gltf_anims.py:152-153` mesure les os du **clip** absents du **corps** — l'inverse de la grandeur utile. Mesuré : 62/64 os couverts, les 2 manquants sont `cheveux_01` et `cheveux_02` |
| D | `jog_backward` n'anime que 49 os sur 64 | **MENTEUR** | ligne « (1 os absents du corps) » (`merge_gltf_anims.py:180-184`) | 13 manquants : Head, LeftToeBase, RightToeBase, 10 bouts de doigts. Le seul garde est `if not canaux` (`:160-163`), qui ne tire qu'à 0 os commun |
| E | `death` conserve 0,890 m de dérive racine | **AVEUGLE** | *aucun* | `convertir()` itère `dossier.glob("*.fbx")` (`mixamo_recolte.py:519`) ; 9 des 34 GLB n'ont pas de `.fbx` — dont `jump`, `run`, `walk`, `idle`. Ils ne passent aucune des trois mesures, et `merge_gltf_anims.py:114` les fusionne quand même |
| F | `accroupi_facteur_vitesse` n'a aucun consommateur | **AVEUGLE** | *aucun* | 3 occurrences dans tout `crates/` : déclaration `posture.rs:46`, défaut `:55`, test `:231`. `PlayerMovementTuning` (`forgia-player/src/lib.rs:727`) ne le lit pas : **s'accroupir ne ralentit pas**. Le test `:227-232` valide la plage d'un gène que personne n'applique |

**Bilan : 1 COUVERT · 3 PARTIEL · 8 MENTEUR · 3 AVEUGLE.**

---

## 3. Les trous par famille, ordonnés par coût à la prochaine occurrence

### (c) Le capteur dit `ok` alors que ça casse — 8 cas, coût le plus élevé

Ce sont ceux qui fabriquent de la fausse confiance : ils envoient chercher ailleurs, ou empêchent de chercher.

| Coût | Capteur | Ce qu'il affiche | Pourquoi c'est faux |
|---|---|---|---|
| bloquant | `forgia2_castle_avatar.json` | `ground_gap_m:0.002`, `desync_m:0.000`, `bone_copies:0`, `severity:"ok"` | Mesuré sur la racine de scène (`castle_avatar.rs:478`) ; sonde `hand_l` (`:288`) inexistante sur le rig Mixamo → branche `:523` structurellement morte |
| bloquant | `forgia2_expedition_arme.json` | `socket_trouve:true`, `ancrage:"RightHand"`, `severity:"ok"` sur un corps sans socket | Le repli `arme_main.rs:452-458` rend `ANCRAGE_ABSENT` (`:945`) inatteignable |
| bloquant | `forgia2_expedition_visee.json` | `ecart_main_droite_m:-1.000`, `severity:"ok"` | `[visee.bras] actif = false` (`expedition_arme_main.toml:102`) force la sentinelle (`visee.rs:776-777`) ; `MAIN_HORS_CIBLE` (`:991`, `> 0.12`) ne peut jamais tirer |
| bloquant | `tools/ai/forgia_digest.py` | « 135 lus, 8 en alerte » — **aucun** capteur de la tranche | `:205` classe `info` ET `severity` absente comme non-alerte ; grep `mtime\|getmtime` = **0 occurrence**, aucun contrôle de fraîcheur |
| majeur | `forgia_anim_layer.json` | `spring 0/0/0`, `ik 0/0/0`, `cloth 0/0/0`, `budget.severity:"ok"` | Severity **imbriquée** sous `budget` → invisible du digest ; c'est nommément le contre-exemple cité par `anim_sensor.rs:270-272` |
| majeur | `forgia_rig_bones.json` | `bone_count:0`, `bones:[]`, aucun champ `severity` | Écrit en `fs::write` **synchrone dans la frame** (`debug_gizmos.rs:275`) pour publier zéro os |
| majeur | `forgia2_input.json` | `keys_pressed_per_sec:0`, `severity:"ok"` | `severity_for_input` (`input_sensor.rs:26-35`) n'a qu'un seuil haut (>200/s). Aucune ventilation par `PlayerAction` : « la touche accroupi a-t-elle jamais été lue ? » est sans réponse |
| majeur | `forgia2_rex_bones*.json` | `severity:"ok"`, `timestamp_secs:23.5` | mtime 2026-08-10 contre 2026-08-17 pour les autres — **7 jours** de retard, rien ne le dit ; et ils décrivent le rig auto-rig (`hand_l`, `hip`), pas le rig Mixamo |
| majeur | `forgia2_run.log` | consommé par `forgia_digest.py all` | mtime 2026-08-14 16:44 contre 22:21:47 pour `forgia2_sensor_io.json` : **3,23 jours** de retard, mélangés sans un mot — alors que plusieurs `next_step` ordonnent de croiser avec lui (`castle_avatar.rs:549`) |
| majeur | test `les_clips_declares_existent_dans_le_glb_du_corps` | vert | `equipment.rs:1375-1377` saute en silence le corps trooper (`.gltf` externe, `roguelite_equipment.toml:19`) ; `assert!(mesures > 0)` (`:1395`) passe avec 1 corps sur 2 |
| majeur | `mixamo_recolte.py:585-590` | « ✓ dérive racine retirée 0.00 m » | Même sortie qu'un clip propre quand `Hips` est introuvable (`:466-467`) ; `faits += 1` (`:590`) sur un clip accusé « NE PAS FUSIONNER » (`:559-565`) ; `main()` sort en 0 (`:639-643`) |

### (b) Grandeur publiée, aucun seuil — un humain doit deviner

Cas §13 littéral : le nombre est là, personne ne le juge.

| Champ | Où il est publié | Aucune branche ne le lit |
|---|---|---|
| `ecart_buste_deg` | `visee.rs:983`, format `:1020` | Chaîne `:989-1017` ne teste que `facteur`, `droite_m`, `os_attendus`, `os_trouves`, `zoom_arme`, `fov_deg`. Oscille 7,6 → 33,8° joueur immobile |
| `ecart_main_gauche_m` | `visee.rs:1020` | Un seul bras sur deux est jugé (`:991`) |
| `fov_deg` | `visee.rs:954-961` | `:1011-1017` : les **deux** branches rendent `("ok","")`. Le contrôle annoncé en commentaire `:1012-1013` n'existe pas |
| `ancrage` | `arme_main.rs:988` | `etat.ancrage` n'apparaît qu'en `:458`, `:460`, `:464`, `:988` — aucune condition |
| `personnages[].os[].euler_deg` | `bone_trace.rs:311`, `:391` | Aucune branche du dépôt ne lit une rotation d'os. En direct : RightForeArm `[0.0,-0.0,-97.3]` |
| `personnages[].os[].monde` | `bone_trace.rs:391` | `ecrire_sante` (`:407-434`) ne lit que l'`ecart_m` des maillages |
| `cibles_touchees` | `anim_sensor.rs:210` | `ANIM_HORS_CIBLE` (`:283-285`) exige `== 0`. En direct : 62/83, `severity:"ok"` — 21 os déclarés non réécrits, et **aucun champ ne dit lesquels** |
| `lecteurs_total` vs `lecteurs_detailles` | `anim_sensor.rs:218` | `take(MAX_LECTEURS=12)` (`:163`) : le 13ᵉ lecteur en T-pose sort au vert ; l'écart entre les deux ne déclenche rien |
| `pire_ecart_m` | `bone_trace.rs:428-434` | Clé **absente** du JSON dans les branches `ok` (`:419`) et aveugle (`:412`) ; chroniquement en warn sur assets sains (0,69 sur `SM_Legs.Armor`) |

### (a) Grandeur jamais publiée — on ne peut pas savoir

| Grandeur | Où elle est calculée | Conséquence |
|---|---|---|
| **Toute la posture** : `accroupi`, `glisse`, `glisse_restant_s`, `ReposGlissade` | `posture.rs:125-170` | Aucun `sensor_io::enqueue` dans le module ; grep `accroupi\|glisse` sur **tous** les `.json` du dépôt = **0 occurrence**. « La glissade ne part pas » n'a aucune trace, et les 3 causes que `peut_glisser` (`:97`) distingue meurent dans la frame |
| 10 des 11 entrées d'`AvatarLocomotion` : `avant`, `lateral`, `au_sol`, `vitesse_verticale`, `accroupi`, `glisse`, `vise`, `tire`, `lacet_rad_s`, `depuis_atterrissage_s` | `avatar.rs:554-569`, consommées `:848` | Seul `speed_mps` est publié (`castle_avatar.rs:577`). Quand le mauvais clip joue, on voit **lequel**, jamais **pourquoi** |
| `derive_racine_m` (avatar ↔ capsule) | nulle part | Le symptôme de 6,8 m n'existe dans **aucun** des 135 capteurs |
| `pose_derive_max_deg` | boucle `visee.rs:611-638` | L'invariant anti-accumulation n'a aucun témoin ; sa récidive du 2026-08-17 (`visee.rs:659-661`, coude) a été trouvée à l'œil |
| `recul` / `avance` de prise, roulis de l'arme | `arme_main.rs:647`, `:651`, `correction_de_socket:1056,1070-1071` | Une arme qui vise juste mais **couchée sur le flanc** sort à `ecart_visee_deg` inchangé et `severity:"ok"` — le test `:1234` prouve que le cas compte |
| distance origine→bouche | `bouche_locale`, `arme_main.rs:644,1106` | Une bouche à la crosse sur le même axe rend le même angle |
| `dague_os`, `dague_materiau` | écrits `:735`, `:773` | **Absents du `format!` `:985`** — le commentaire `:344-348` affirme une publication qui n'existe pas |
| `pole_coude`, position du coude, angle de flexion | `visee.rs:862-863` | Seule sortie : un gizmo F3 (`:850-857`), pas une donnée machine |
| Tout le pipeline hors-ligne | `mixamo_recolte.py`, `merge_gltf_anims.py` | Zéro `json.dump`, zéro `forgia2_*`. `OS_LE_PLUS_LONG`, `COURSE_RACINE`, `OS_RENOMMES` vivent quelques ms dans un tube stdout |
| `os_le_plus_long_m`, `hauteur_squelette_m`, `os_surveilles_trouves` | `bone_trace.rs:285-337` a les `GlobalTransform` en main | Les 12 os Mixamo surveillés ont fait **0 touche** sur les 44 os d'un rig UE, et `os_ecartes=44` ne distingue pas « tronqués » de « les 12 surveillés sont dans le tas » |

---

## 4. Le constat structurel

**8 des 9 bugs de la session ont été trouvés par l'œil de l'utilisateur en jeu.** Le neuvième (id 8) a été trouvé par `bevy_ecs`, pas par Forgia : le capteur de crash (`qa_bridge.rs:44`) a transcrit un message que le moteur avait déjà écrit. **Aucun capteur Forgia n'a détecté un seul des bugs de cette tranche.**

Et le mécanisme qui a permis cette unique détection est bien un **désaccord** : `bevy_ecs` compare deux déclarations d'accès à la même ressource (`Res<Visee>` contre `ResMut<Visee>`) et refuse. Ce n'est pas un capteur d'état, c'est un comparateur.

Le déséquilibre est mesurable. Le dépôt compte **135 fichiers capteurs** (`forgia_digest.py sensors`), qui publient massivement des valeurs — « voici `speed_mps` », « voici `fov_deg` », « voici `ecart_buste_deg` ». Les comparateurs, eux, sont **quatre** :

1. `corps[].attendus` vs `presents` → `AVATAR_ANIM_PARTIAL` (`castle_avatar.rs:528-555`) — **le seul qui se déclenche sur un désaccord partiel**.
2. `os_attendus` vs `os_trouves` → `TENUE_PARTIELLE` (`visee.rs:1006`) — mais `os_attendus = 1` (le génome ne cite que `Spine2`) : l'échantillon est d'un os.
3. `cibles_declarees` vs `cibles_touchees` (`anim_sensor.rs:283-285`) — seuil au **zéro** seulement : 62/83 passe au vert.
4. `etat_demande` vs `clip_joue` (`avatar.rs:880-885`) — bien conçu, **aucune branche de sévérité**, et neutralisé par le garde `:858` qui n'écrit qu'au changement d'état.

Le dispositif **produit déjà les paires** dont le désaccord aurait nommé les bugs, et ne les compare jamais :

- `forgia2_player_state.json::velocity_planar_m_s` (`player_state_sensor.rs:68`, en `Update`, donc replié 0↔15) contre `forgia2_castle_avatar.json::speed_mps` (`castle_avatar.rs:566`, depuis `PlayerLocomotion` en `FixedUpdate`) — deux vitesses contradictoires publiées, rien ne dit laquelle pilote l'animation.
- `forgia2_player_state.json::grounded` (`:80-82`) contre `loco.au_sol` (`forgia-player/src/lib.rs:869`, avec un repli `unwrap_or(true)` qui lit « au sol » quand le KCC ne répond pas) — la seconde n'est même pas publiée.
- **Au même `timestamp_secs` 2932.6** : `forgia2_expedition_arme.json` dit `critical` / `ARME_A_L_ENVERS` / 145,1° pendant que `forgia2_expedition_visee.json` dit `ok`. Les deux mesurent l'orientation de la même arme, l'un contre la capsule (`arme_main.rs:879,919`), l'autre contre la caméra (`visee.rs:969-975`). **Personne ne relève la contradiction.**

Le dispositif sait produire ce genre de désaccord. Il ne sait pas le lire.

---

## 5. Le plan — 8 correctifs, par bugs attrapés / effort

### 1. `tools/assets/rapport_clips.json` — le pipeline d'animation cesse de parler en `print()`
- **Où** : écriture après `mixamo_recolte.py:509` et après `merge_gltf_anims.py:188`. Aucune compilation Rust, aucun runtime.
- **Champs, un objet par clip** : `os_le_plus_long_m`, `ratio_au_corps_max` + `os_le_plus_divergent`, `course_entree_m`, `derive_residuelle_m:[x,y,z]` (remesurée sur les octets **réécrits**, mêmes nœuds `Hips`, mêmes canaux `translation`), `os_racine_trouve:bool`, `os_couverts`/`os_du_corps`, `noms_en_double:[]`.
- **Seuils** : `ratio_au_corps_max` hors [0,95 ; 1,05] → critical · `max(|derive_residuelle_m|) > 0.05` → critical · `os_racine_trouve == false` → critical (et non le `0.0` muet de `:467`) · nom d'animation déjà présent dans le doc de sortie → critical · `os_couverts < os_du_corps` → critical. Et `main()` (`:639-643`) rend un code non nul.
- **Aurait attrapé** : 9, 11, A (×1,529 sur LeftArm), B (9 doublons), C (`cheveux_01/02`), D (49/64), E (`death` 0,890 m) — **7 bugs**.

### 2. `forgia2_expedition_posture.json` — la feature la plus récente n'a aucune sortie
- **Où** : `crates/forgia-mode-expedition/src/posture.rs`, plugin `:109-122`, un système `.in_set(GameSet::Sensors)` à 1 Hz.
- **Champs** : `accroupi`, `glisse`, `glisse_restant_s`, `repos_restant_s`, `vitesse_mps`, `cause_refus_glissade` ∈ {`DEJA`, `REPOS`, `TROP_LENT`, `AUCUNE`} (les 3 causes que `peut_glisser:97` distingue déjà), `facteur_vitesse_applique`, `config_chargee:bool`.
- **Seuils** : `GENE_SANS_CONSOMMATEUR` critical si `accroupi == true` et `facteur_vitesse_applique == 1.0` · `POSTURE_AVEUGLE` **info** (jamais `ok`) hors Expédition · `CONFIG_PAR_DEFAUT` warn si le repli muet `:90` a servi.
- **Aurait attrapé** : F. Fait surtout passer de zéro à couvert une feature dont **aucune** grandeur n'est lisible aujourd'hui.

### 3. `forgia_digest.py` honnête — 1 fichier Python, désarme 6 faux verts d'un coup
- **Où** : `tools/ai/forgia_digest.py:205`.
- **Trois changements** : `severity` absente → **alerte** (et non `"—"` non-alerte) ; remonter une severity imbriquée (`budget.severity`, `forgia_anim_layer.json`) ; comparer chaque mtime au plus frais des capteurs et alerter au-delà d'un écart (aujourd'hui : `grep mtime` = 0 occurrence).
- **Seuil** : capteur en retard de plus de 2× sa période déclarée, ou de plus de 5 min sur le plus frais du lot.
- **Aurait signalé** : `forgia_rig_bones.json`, `forgia_bone_trace.json`, les 4 `rex_bones` (7 j), `forgia2_run.log` (3,23 j), `forgia_anim_layer.json`. Sans lui, la commande de réflexe sur « regarde » répond « tout va bien » sur cette tranche entière.

### 4. Réparer le référentiel de `ecart_visee_deg`, puis lui donner ses deux branches manquantes
- **Où** : `arme_main.rs:914-923` (mesurer contre la caméra qui rend, que `visee.rs:969` sait déjà trouver, au lieu de `q_player`/`:879`), `:945-982`, `:985`.
- **Ajouts** : branche `ECART_NON_MESURABLE` en `info` quand `ecart_deg < 0.0 && installe` (patron déjà dans le dépôt : `anim_sensor.rs:274-280`) · `ancrage_est_repli:bool` + branche `ANCRAGE_DE_REPLI` en warn · `ecart_visee_mesure:bool` · `depuis_entree_s` · `corps_modele`.
- **Seuils** : les 90.0/35.0 (`:970`, `:975`) déplacés dans `[visee]` de `expedition_arme_main.toml`, déjà rechargé à chaud.
- **Aurait attrapé** : 1, 3, et **désature** le canal pour 6 (aujourd'hui épinglé à ~145° critical, donc incapable de nommer un défaut nouveau).

### 5. Publier les 11 entrées d'`AvatarLocomotion`
- **Où** : `castle_avatar.rs:566` (le `format!`), source `avatar.rs:554-569`. Et retirer le garde `:858` du bloc `:878-889` pour que `etat_demande`/`clip_joue` décrivent l'état courant, pas la dernière transition.
- **Champs** : `avant`, `lateral`, `au_sol`, `vitesse_verticale`, `accroupi`, `glisse`, `vise`, `tire`, `lacet_rad_s`, `depuis_atterrissage_s`.
- **Seuils** : `LOCO_INCOHERENTE` warn si `etat_demande == MarcheArriere && avant > 0` (une inversion de signe se nomme seule) · `AU_SOL_DIVERGENT` warn si `au_sol` ≠ `forgia2_player_state.json::grounded` — **le premier comparateur inter-capteurs du dépôt**.
- **Aurait attrapé** : toute la classe « le mauvais clip joue », aujourd'hui sans aucune cause lisible.

### 6. `derive_racine_m` + une sonde de désync lue sur le squelette présent
- **Où** : `castle_avatar.rs` — distance horizontale entre la position monde de `Hips` (atteignable par le même `iter_descendants` + `Name` que `:462-469`) et la capsule ; remplacer `DESYNC_PROBE_BONE` (`:288`) par un os lu, ou publier `sonde_trouvee:false` ; faire de `ground_gap_m` un raycast sous `LeftFoot`/`RightFoot` (déjà surveillés par `bone_trace`) plutôt que sous la racine de scène (`:492`).
- **Seuil** : `AVATAR_HORS_CAPSULE` critical à 0,5 m, `next_step` = « root motion non retirée du clip — le clip n'est pas passé par la porte `.fbx` ».
- **Aurait attrapé** : 10, 11.

### 7. `pose_derive_max_deg` dans la boucle qui tient déjà l'invariant
- **Où** : `visee.rs:611-638`, avant l'écriture `:629` ; publier dans `:1020`.
- **Champs** : `pose_derive_max_deg` (max de `base.angle_between(sortie)`), `os_le_plus_tourne`, `pose_os_base_memorisee`/`pose_os_base_rafraichie` (compteurs des branches `:618` et `:619`).
- **Seuil** : `POSE_ADDITIVE_ACCUMULEE` critical au-delà de 1° (0° par construction tant que le mémo tient) ; croissance non nulle sur 3 tics consécutifs → critical.
- **Aurait attrapé** : 5, 6 — et la récidive du 2026-08-17 sur le coude (`visee.rs:659-661`), trouvée en jeu.

### 8. `cibles_non_touchees[]` dans `forgia2_animation.json`
- **Où** : `anim_sensor.rs:180-184` a déjà la liste en main, `:210` n'en garde que le compte.
- **Seuil** : critical dès qu'un os ciblé par une couche additive (aujourd'hui `Spine2`) figure dans la liste — la précondition exacte du bug 5, aujourd'hui invisible derrière `62/83, severity:"ok"`.
- **Aurait attrapé** : 5. C'est aussi le seul moyen de voir C (`cheveux_01`/`cheveux_02` non couverts) au runtime.

---

## 6. Ce que cet audit n'a pas couvert

- **Aucun build, aucun run.** Tous les verdicts runtime reposent sur les JSON présents à la racine. Or le fichier le plus frais (`forgia2_expedition_arme.json`, 2026-08-17 22:51) porte `en_expedition:false` : **les capteurs d'Expédition n'ont jamais été observés en Expédition** pendant cet audit. Les relevés en direct cités (145,1° saturé, `ground_gap_m:0.002`, 62/83) viennent d'un jeu qui tournait, pas d'un scénario reproduit.
- **3 des 5 plugins du mode n'ont été exercés par rien** : `avatar_vfx.rs:70`, `posture.rs:107`, `matiere.rs:109` ne sont montés par aucun `App::new()` du dépôt (75 sites pour 62 crates). Leur comportement au montage est inconnu, y compris vis-à-vis de B0002.
- **Aucun test n'a été exécuté.** Les comptes de tests (13 dans `arme_main.rs:1129-1437`, 8 dans `capteur.rs`) viennent de la lecture des fichiers, pas d'un `cargo test`.
- **Les 15 clips déclarés au génome n'ont pas été vérifiés un par un au runtime.** Le seul test du produit final (`equipment.rs:1360-1398`) n'en couvre que 3 sur 15, et saute le corps trooper.
- **Hors périmètre** : son, réseau, physique Rapier, combat (dégâts, TTK, IA), VFX/braseros, streaming de cellules, mémoire/GPU.
- **Le coût des capteurs proposés n'est pas mesuré.** La file tient aujourd'hui (`forgia2_sensor_io.json` : 58 225 enqueued, `pending:25`, `dropped_full:0`), mais 6 écritures restent en `std::fs::write` bloquant dans la frame (`rounds.rs:630`, `forgia-skeleton-template/lib.rs:954`, `forgia-auto-rig/lib.rs:353`, `migration_baseline.rs:124`, `debug_gizmos.rs:275`) et aucun correctif ci-dessus ne les traite.
- **Ce que les capteurs ne trancheront jamais** : l'arme est-elle *bien* tenue, la glissade est-elle *agréable*, l'essaim est-il *lisible*. `map-design-intention.md` §5.3 — ça se décide manette en main. Le plan ci-dessus rend les 15 défauts **nommables**, il ne rend pas le jeu bon.
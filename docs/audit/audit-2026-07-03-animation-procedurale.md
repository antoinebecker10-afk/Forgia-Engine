# Audit — Système d'animation procédurale Forgia + comment rendre le gobelin vivant

> **Date** : 2026-07-03. Demande Antoine : auditer notre anim procédurale, croiser aux best
> practices industrie (recherche web), et savoir comment rendre le gobelin marchand vivant
> comme un vrai vendeur itinérant. Sources : balayage code (98 fichiers avec `sin/lerp/spring/
> Quat`) + 3 recherches web + 1 fetch technique. Sources listées en fin de doc.

---

## 1. TL;DR — le verdict

Forgia a **4 tiers d'animation** de qualité très inégale :

| Tier | Technique | Qui l'utilise | Verdict |
|---|---|---|---|
| **A. Skeletal baké** | GLB riggé + `AnimationPlayer`/`AnimationGraph`, 95 clips | **Ennemis** (`enemy_anim.rs`) | ✅ excellent, c'est LA vie |
| **B. Toolbench os** | spring bones (Verlet), IK 2-os, proc-walk, foot-IK | RPG (Rex), auto-rig NPC | 🟡 réel + branché mais **exige un rig** + mono-perso |
| **C. Procédural Transform** | `sin/cos/lerp` sur le Transform entier | **Gobelin** (`forge_shop.rs`), viewmodel | 🔴 tier le plus faible, brut |
| **D. Juice** | decay/trauma, FOV punch, recoil, shake | combat feel | ✅ bon (mais pas des ressorts) |

**Le gobelin est au tier C, le plus pauvre**, sur un mesh **sans rig** (vérifié : `Gobli.glb` =
1 node, 0 skin, 0 clip). Il « bouge » (sin brut : balancement + respiration + hop) mais reste
robotique : mouvement **symétrique** (l'œil lit la symétrie comme *uncanny*), **pas de
ressort**, et surtout **il ne regarde pas le joueur** (il fixe le point de spawn, orienté par le
yaw fixe du parent). Un vrai vendeur itinérant **suit son client des yeux**.

**La bonne nouvelle** : les 3 plus gros gains (look-at joueur, ressort amorti, asymétrie) sont
faisables **sans rig**, à coût quasi nul. La vie « complète » (gestes de bras, compter les
pièces) exige un rig — deux routes documentées en §6.

---

## 2. Inventaire — notre système, tier par tier

### Tier A — Skeletal baké (ennemis) ✅
- [`enemy_anim.rs`](../../crates/forgia-mode-roguelite/src/enemy_anim.rs) : les GLB KayKit sont
  riggés + contiennent 95 clips (`Idle`/`Walking_A`/`Running_A`/attaques/`Death`). On les joue
  via `AnimationPlayer` + `AnimationGraph`, pilotés par l'état IA (`ArenaBot.state`) + vitesse
  mesurée, crossfade 150 ms. **C'est comme ça que les ennemis sont vivants** : l'animateur a
  baké la vie, on la rejoue. Robuste multi-ennemis (chaque `SceneRoot` a son player).
- **Enseignement pour le gobelin** : si on veut la vraie vie (bras, mains), c'est CE pipeline
  qu'il faut — donc un gobelin **riggé**.

### Tier B — Toolbench os (FORGE track) 🟡 réel mais bloqué pour ce cas
- [`forgia-secondary-motion`](../../crates/forgia-secondary-motion/src/lib.rs) : spring bones
  Verlet + contraintes de distance, `PostUpdate` après les clips / avant propagate, scratch
  `Local`, `run_if(any_with_component)`, budget 500 µs/12 chaînes. **Branché globalement**
  ([forgia-game:146](../../crates/forgia-game/src/lib.rs#L146)) — mais n'agit que sur les
  entités **avec `SpringBoneChain`** (⇒ des **os**). Le gobelin n'en a pas.
- [`forgia-ik`](../../crates/forgia-ik/src/lib.rs) : 2-bone analytique + **look-at-target**
  (réf. Orange Duck), lib pure, O(1). Utilisé par `forgia-rpg::character`. **Exige un rig**.
- [`forgia-anim-locomotion`](../../crates/forgia-anim-locomotion/src/locomotion.rs) : proc-walk
  + foot-IK, **mono-personnage** (`.single()` — cf `npc_pose.rs` : ajouter un 2ᵉ
  `LocomotionTarget` « tue l'anim de Rex »). Réservé au héros RPG.
- [`forgia-rpg::npc_pose`](../../crates/forgia-rpg/src/npc_pose.rs) : les PNJ auto-riggés
  (Pinocchio) sont sortis de la T-pose vers une **pose STATIQUE** (bras le long) — vivants
  visuellement mais **immobiles** (le cœur locomotion est mono-perso).
- **Bilan** : un vrai moteur d'anim procédurale existe, mais il est **os-centré** et
  **mono-perso** ; inapplicable tel quel au gobelin rigless.

### Tier C — Procédural Transform (gobelin, viewmodel) 🔴 le maillon faible
- [`forge_shop::sys_animate_gobli`](../../crates/forgia-mode-roguelite/src/forge_shop.rs) :
  `rotation = yaw(sin) · X(sin respiration + hop)`. Rotation seule (pour ne pas casser la
  calibration scale/Y). **Ce que je viens de livrer** — honnêtement minimal.
- [`viewmodel::pose::apply_viewmodel_sway_bob`](../../crates/forgia-viewmodel/src/pose.rs) :
  plus mûr — sway par delta souris **lissé (lerp)**, bob de marche indexé sur
  `PlayerLocomotion` (phase wrappée à 2×TAU pour éviter l'à-coup), respiration idle. C'est le
  meilleur exemple de tier C existant, mais le lissage est un **lerp** (ressort du pauvre), pas
  un vrai spring-damper.

### Tier D — Juice ✅
- `camera_shake` (trauma decay), `fov_punch`, `recoil`, `knockback`, `hit_stop`. Bon game-feel,
  mais basé decay/trauma plutôt que ressorts normalisés — cf reco perf §7.

---

## 3. Audit ciblé du gobelin actuel (`sys_animate_gobli`)

| Aspect | État | Best practice | Écart |
|---|---|---|---|
| Vie de base (idle non figé) | ✅ bob + respiration | oui | OK |
| Réaction à l'ouverture | ✅ « hop » | oui (feedback) | OK |
| **Regard vers le joueur** | ❌ fixe (yaw parent = spawn) | « if AI looks at something, actually turn the head » | 🔴 gros manque |
| **Ressort (spring-damper)** | ❌ sin brut + decay linéaire du hop | springs normalisés 0→1 | 🟠 |
| **Asymétrie** | ❌ sin pur = symétrique | asymétrie obligatoire (symétrie = uncanny) | 🟠 |
| **Weight shift** (transfert de poids) | ❌ | lean latéral lent | 🟠 |
| Gestes contextuels (achat, salut) | ❌ | fidgets/réactions | 🟡 |
| Data-driven + sensor | ❌ consts en dur | genome + observabilité | 🟡 (dette projet) |

---

## 4. Best practices industrie (sourcées)

1. **Ressorts amortis > sin brut** (Ryan Juckett / *Game Developer*) : un `damped spring`
   (position+vélocité, params **damping 0-1** + **frequency**) donne un mouvement organique,
   stable, sans discontinuité. **Normaliser la valeur du ressort en 0→1** en fait une *API*
   réutilisable (position/scale/rotation/alpha branchent dessus). Idéal pour recoil, caméra,
   secondary motion, réactions UI.
2. **Idle croyable = 3 couches** (MoCap Online, garagefarm) : (a) **respiration** subtile
   (chest/épaules/poids), (b) **asymétrie naturelle** — « l'œil humain détecte la symétrie
   bilatérale comme *uncanny* », un pied porte plus de poids, une épaule tombe, (c) **micro
   head-float** qui suit la respiration. Boucle 2-4 s sans couture.
3. **Look-at / head-tracking** : « an idle that loops with a small head adjustment signals a
   living entity waiting to respond, not a frozen prop » ; « if your AI mentions looking at
   something, have them actually turn their head ». Pour un **vendeur**, suivre le client du
   regard = signal de vie n°1.
4. **Overgrowth / David Rosen (GDC 2014)** : générer beaucoup d'anim avec **peu de key-poses**
   + blend piloté par le code (distance→blend pose, IK locomotion, combat physique). Doctrine
   indé : **du code plutôt que 500 clips**. Aligne exactement avec la vision FORGE de Forgia.
5. **Secondary motion pas cher** : une **spring chain** sur une queue / oreille / bourse de
   pièces donne une vie « gratuite » — c'est EXACTEMENT ce que fait `forgia-secondary-motion`
   (déjà branché) — il ne manque que des **os** à animer.

---

## 5. Gap analysis — Forgia gobelin vs best practices

| Best practice | Industrie | Forgia gobelin | Action |
|---|---|---|---|
| Ressort amorti normalisé | standard | sin brut | **P0** : mini spring-damper (Juckett) réutilisable |
| Look-at client | signal de vie n°1 | fixe sur spawn | **P0** : yaw suit le joueur (spring) |
| Asymétrie + weight shift | obligatoire | sin symétrique | **P0** : 2ᵉ harmonique + lean + phase/instance |
| Gestes contextuels | fidgets | hop only | **P1** : réactions event-driven (achat/salut) |
| Secondary motion (os) | spring chain | absente (pas d'os) | **P2** : rig → spring bones (toolbench) |
| Bras / mains / compter | skeletal | impossible (rigless) | **P2** : gobelin riggé |
| Data-driven + sensor | — | consts en dur | **P1** : genome + `forgia2_vendor_anim.json` |

---

## 6. Recommandations — rendre le gobelin vivant

### 🥇 P0 — Sans rig, ~2 h, impact maximal (le vendeur « prend vie »)
1. **Look-at joueur** : le gobelin oriente son yaw (et un léger lean du torse) vers la
   position RÉELLE du joueur, **lissé par un ressort** (pas de snap). Compensé du yaw parent
   → on tourne le local de l'enfant, le stand ne bouge pas. *Un vendeur regarde son client.*
2. **Spring-damper réutilisable** (`fn damped_spring(pos,&mut vel,target,damping,freq,dt)`,
   réf. Juckett) : remplace le hop `sin` linéaire + pilote le look-at et les réactions. Valeur
   **normalisée 0→1** = API branchable (scale « pop », lean, nod).
3. **Asymétrie + weight shift** : casser le sin pur → lean latéral lent (transfert de poids) +
   2ᵉ harmonique déphasée + respiration en **scale-Y** subtil. Rendu organique, pas métronome.

### 🥈 P1 — Sans rig, ~2 h, gestes de marchand (event-driven, réutilise nos events)
4. **Réactions contextuelles** : `PurchaseRequest` → petit **bounce joyeux** ; passage de
   proximité `near=false→true` → **salut** (hop + lean vers le joueur) ; « pas assez d'Or » →
   micro-**shrug**. Tout via le spring 0→1 (P0-2). Les events existent déjà (story-659).
5. **Data-driven + observabilité** (dette projet, règles Forgia) : `roguelite_vendor_anim.toml`
   (amplitudes/fréquences/damping, hot-reload) + `forgia2_vendor_anim.json` (look-at actif,
   angle, réactions/s). Sort les consts du dur, respecte `no-hardcode` + `observability-required`.

### 🥉 P2 — La vraie vie de vendeur (gestes de bras) → exige un rig
Deux routes, à trancher :
- **Route A — asset riggé (recommandée pour le SHIP)** : importer un **gobelin riggé** (GLB
  avec squelette + clips `idle`/`talk`/`wave`/`count_coins`) et le jouer via le pipeline
  **Tier A** déjà éprouvé (`enemy_anim.rs` : `AnimationPlayer`/`AnimationGraph`). Qualité
  maximale, risque minimal, c'est la technique des ennemis.
- **Route B — dogfood FORGE (engine)** : **auto-rig** du gobelin statique
  (`forgia-auto-rig`/Pinocchio) → **spring bones** (secondary-motion, DÉJÀ branché) sur oreilles
  + bourse de pièces + **look-at IK** (`forgia-ik::two_bone_ik`). Valide le toolbench sur un
  asset de prod réel. **Bloqué aujourd'hui** par les blockers auto-rig humanoïde (story-601) et
  le locomotion mono-perso — mais **1 seul vendeur** contourne le multi-perso. C'est le pari
  « moteur IA-natif » de la vision, à faire quand le ship le permet, pas avant.

---

## 7. Best practices perf / optim (recherche + notre code)

- **Intégration stable** : spring-damper en **semi-implicite / analytique (Juckett)**, jamais
  Euler naïf (explose à bas fps). Notre viewmodel utilise un lerp `min(1.0)` — correct mais
  moins bon qu'un vrai ressort.
- **Normaliser les ressorts 0→1** = API réutilisable (cf §4.1) — évite de dupliquer la physique
  par propriété animée.
- **Gating** : `run_if` / `any_with_component` (le crate secondary-motion le fait déjà) →
  n'anime le vendeur que proche/visible (LOD). 1 entité ici = trivial, mais le **pattern** doit
  tenir si on généralise à N PNJ.
- **Zéro alloc hot path** : scratch `Local`, pas de `Vec::new()` par frame (secondary-motion
  respecte déjà). Les `sin/cos` par frame sur 1 entité = négligeable.
- **Ordering** : la secondary motion doit tourner **après** la pose de clip et **avant**
  `TransformPropagate` — notre crate le fait (`PostUpdate` + `before(TransformPropagate)`).
- **Budget** : viser < 200 µs pour tous les PNJ non-combat animés ; au-delà, batcher / réduire
  la fréquence des non-visibles.

---

## 8. Plan / stories candidates

| Story | Contenu | Effort | Rig ? |
|---|---|---|---|
| **P0** | `vendor_anim.rs` : spring-damper util + look-at joueur + asymétrie/weight-shift | ~2 h | non |
| **P1** | Réactions event-driven (achat/salut/shrug) + genome + sensor | ~2 h | non |
| **P2-A** | Import gobelin riggé + clips via pipeline `AnimationPlayer` (Tier A) | ~1 j | **asset** |
| **P2-B** | Dogfood : auto-rig + spring bones + look-at IK (débloque story-601) | épique | **auto-rig** |

**Reco** : faire **P0 maintenant** (2 h, le vendeur prend vie sans asset), P1 dans la foulée,
et garder P2 pour quand un asset riggé arrive (route A) ou quand le toolbench auto-rig est
débloqué (route B, FORGE track).

---

## Sources

- [Instant Game Feel — Springs Explained (Game Developer)](https://www.gamedeveloper.com/game-platforms/instant-game-feel---springs-explained) — spring-damper (Juckett), damping/frequency, normalisation 0→1.
- [Idle Animation Design Guide (MoCap Online)](https://mocaponline.com/blogs/mocap-news/idle-animation-game-dev-guide) — respiration, asymétrie, head-float, boucle 2-4 s.
- [Idle Animation — What Devs Need to Know (MoCap Online)](https://mocaponline.com/blogs/mocap-news/idle-animation-loop) — « living entity vs frozen prop », look-at.
- [Procedural Animation: Tips & Best Practices (Animost)](https://animost.com/ideas-inspirations/procedural-animation/) — hybride procédural + secondary motion.
- [An indie approach to procedural animation (David Rosen / Overgrowth, Game Developer)](https://www.gamedeveloper.com/design/video-an-indie-approach-to-procedural-animation) — pose-blending, IK locomotion, code>clips.
- [Idle Animation Tips (GarageFarm)](https://garagefarm.net/blog/idle-animation-tips-to-animate-your-characters) — weight shift, micro-fidgets.
- Réf. interne IK : [Orange Duck — Simple Two Joint](https://theorangeduck.com/page/simple-two-joint) (cité dans `forgia-ik`).

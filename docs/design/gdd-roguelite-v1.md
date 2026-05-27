# Forgia Roguelite — GDD V1

> Game Design Document V1 — source de vérité unique pour Forgia Roguelite shipping V1.
> Rédigé 2026-05-27 (senior GD review : Hadès × Cult of the Lamb × Borderlands).
> Tags complexité : 🟢 simple Bevy / 🟡 moyen / 🔴 complexe.
> Anti-canon strict : "s'endorment / s'envolent / éjectés", jamais "tué/mort/sang".
> Cible commerciale : enfants Roblox + femmes gameuses 20-35 + hommes casual. Vache à lait.

---

## Pitch canonique

> **Un méchant a volé les âmes des armes. Toi, tu es un apprenti gentil. Tu les libères une par une, elles deviennent tes amies, et elles n'arrêtent pas de parler.**

Ton : **Overwatch × Pixar × Cult of the Lamb**. Vocabulaire CE2 (8 ans).
Substituts cartoon : *"Crénom !", "Saperlipopette !", "Sapristi !"*.

Cast MVP : 🔫 Pépin (timide), 💨 Bourrasque (pétillante), 🎩 Madame Lenoir (snob), 🪓 Boucherie (brutal joyeux). + Apprenti héros doux, Forgeron Noir méchant ridicule (Bowser-energy), Maître Forgeron mentor.

Lieux : Royaume (free roam RPG hub), Cryptes de l'Enclume (Volcan polish target), Sanctuaire de la Forge (bonus).

---

# MISSION 1 — FPS Accessible

## 1.1 Projectiles et hitbox

### Vitesse projectiles par arme

| Arme | Type | Vitesse | Justification narrative | Complexité |
|---|---|---|---|---|
| 🔫 **Pépin** | Hitscan instant + tracer cyan visible 100ms | ∞ | Pépin est timide mais précis — il tire vite | 🟢 |
| 💨 **Bourrasque** | 7 pellets cone, projectile court 15 m/s | rapide visible | Bourrasque souffle, l'air emporte les pellets | 🟢 |
| 🎩 **Madame Lenoir** | Hitscan instant + tracer fin blanc 200ms | ∞ | Une dame ne perd pas son temps | 🟢 |
| 🪓 **Boucherie** | Roquette parabolique 12 m/s arc visible | lent voyez la roquette | Boucherie envoie ça avec amour | 🟡 |

**Règle accessibilité** : seuls Pépin et Lenoir hitscan (apprentissage facile). Bourrasque et Boucherie projectile lent = lisibles + skill ceiling.

### Hitbox ennemis

- **Capsule simple** 1.2× model size (Halo doctrine "generous"). `Collider::capsule()` 🟢.
- **Headshot zone** = top 25% capsule, multiplier ×2 (Lenoir) ×1.5 (Pépin) ×1.0 (autres).
- Pattern split parent/child collider déjà existant (`reference_los_exclude_rigid_body_split_parent_child.md`).

### Aim assist (cible casual/Roblox kids)

```
Query<Transform, With<Enemy>> → ennemi le plus proche du crosshair (angle < 6°)
SI distance < 15m ET angle < 6° :
  rotation_camera.slerp(rotation_vers_enemy, 0.04 * dt)
```
🟢 Bevy ~30 LOC, toggle `aim_assist_strength: f32 0.0..1.0` dans `fps_tuning.toml` hot-reloadable. Default `0.5`.

### Feedback hit (anti-sang)

| Élément | Description | Complexité |
|---|---|---|
| **Hitmarker BD** | 4 étoiles cartoon ✨ jaune émanent du point d'impact, 200ms scale + fade | 🟢 |
| **Son "TING"** | Cartoon metal ting, pas de gore squish | 🟢 |
| **Hit flash** | Material emissive enemy flash blanc 80ms → couleur normale | 🟢 |
| **Headshot** | "POW!" gros + son "DING" plus aigu + +20% knockback | 🟢 |
| **Kill** | Enemy ragdoll soft physics + "ZzzZ" floating text + petit ☁️ smoke poof | 🟡 |

## 1.2 Mouvement du joueur

### Vitesse et inertie

| Param | Valeur |
|---|---|
| Walk speed | **5.5 m/s** |
| Sprint speed | **7.5 m/s** (Shift) |
| Acceleration | **40 m/s²** (lerp ~0.15s) |
| Air control | **0.6×** ground accel |

### FOV + accessibilité

- **Default 90°**, slider **75-110°**
- **Motion blur OFF** default, **head bob OFF** default
- **View shake amplitude** slider 0-100%, défaut 60%

### Dash : "Pas de l'Apprenti" 🟢

> Lore : *L'Apprenti bondit comme un cabri.*

- Espace double-tap OU Shift+direction
- **4m en 0.25s**, cooldown **1.5s** (2 charges visibles UI cabriol stocks)
- Voiceline : *"Hop !"*

### Mort = "Fatigue" 🟢

> Pas de "mourir", pas de "Game Over".

- **Barre Énergie** cœurs cartoon ❤️
- À 0 énergie : Apprenti **"tombe dans les pommes"** 😴, voix Maître Forgeron *« Oh là là mon petit, viens à la forge te reposer »*, retour hub
- Aucun écran "YOU DIED"

## 1.3 Lisibilité visuelle

### Signalement ennemis dangereux

| Archetype | Outline | Sound cue (0.5s pre-attack) |
|---|---|---|
| Cage Marchante | bleu doux | "GRRR !" sourd |
| Cage Rapide | jaune éclatant | "Clink clink !" |
| Cage Tank | rouge profond | "BOUM BOUM BOUM" |
| Cage Sniper | violet + laser visible 1.5s | "Tch-tch !" |
| Cage Boomer | orange clignotant accélère | "Bip-bip-BIP !" |
| Cage Mage | cyan glow | "Hmmmmm !" |

### Projectiles ennemis

- Vitesse **lente 8-12 m/s** (player walk 5.5 → évitable)
- Couleur saturée + traînée hanabi
- Arc parabolique pour gros (Tank, Sniper)
- Télégraphe 1-2s wind-up + flash blanc 100ms

### Différenciation vue FPS 4 armes

| Arme | Couleur | Idle | Recoil | Reload |
|---|---|---|---|---|
| 🔫 Pépin | bleu cyan | Tremblote 0.5Hz | 2° pitch | Mag click rapide |
| 💨 Bourrasque | jaune chaud | Sautille 1Hz | 4° pitch + 2° yaw | Pump théâtral |
| 🎩 Lenoir | noir+blanc | Immobile | 0° | Cartouche 2 doigts |
| 🪓 Boucherie | rouge orangé | Respiration épaules | 8° pitch | Barillet rotatif |

Vue **1 main visible** (pas 2-mains Apex). Pattern Source SDK CBaseViewModel.

---

# MISSION 2 — Système de Boons

## 2.1 Architecture

| Param | Valeur V1 |
|---|---|
| **Boons par run cible** | **4** (1 commun + 2 rares + 1 légendaire) |
| **Boons V1 implémentés** | **24** (12 armes + 5 neutres + 7 légendaires/synergies) |
| **Apparition** | 1 après chaque wave clear + 1 récompense mid-boss |
| **Choix** | **3 cartes BD** dans "Coffre du Forgeron" UI |
| **Synergies V1** | Tags `fire`/`ricochet`/`knockback`/`chain`/`precision`/`chaos`. 3+ tags = légendaire caché |

**UI Coffre** 🟡 : 3 cartes 2D, Maître Forgeron sprite *« Choisis bien ! »*, hover = zoom + voiceline preview.

## 2.2 Catalogue par arme (12 boons)

### 🔫 Pépin — mécanique **confiance gauge**

> Jauge confiance 0-10 (HUD discret cœur clignote). +1/hit, -1/miss, reset si "pommes".

| Nom | Voiceline preview | Effet | Tags | Complexité |
|---|---|---|---|---|
| **"Premier vrai tir"** | *"Je commence à y prendre goût !"* | 5 kills consécutifs sans miss : damage +30% jusqu'à miss | precision | 🟢 |
| **"Pépin s'enhardit"** | *"Hop, regarde !"* | +5% mag size par stack confiance (max +50% à 10) | precision | 🟢 |
| **"Crénom de Pépin !"** | *"Crénom je vise vite !"* | Si confiance > 5, 30% chance shot ricochet 1× | ricochet, precision | 🟡 |

### 💨 Bourrasque — mécanique **chaos joyeux**

| Nom | Voiceline preview | Effet | Tags | Complexité |
|---|---|---|---|---|
| **"Vent de chaos"** | *"WOOSH ! Ils reviennent !"* | Pellets rebondissent 1× sur murs | ricochet, chaos | 🟡 |
| **"Souffle de Bourrasque"** | *"Pousse-toi !"* | RMB = tornade radius 3m, knockback 4m, 0 dmg | knockback, chaos | 🟢 |
| **"Saperlipopette ça pète !"** | *"BOUM-BOUM contagieux !"* | Ennemi touché émet onde 1.5m radius sur voisins (chain 50%) | chain, chaos | 🟢 |

### 🎩 Madame Lenoir — mécanique **précision/patience**

| Nom | Voiceline preview | Effet | Tags | Complexité |
|---|---|---|---|---|
| **"Œil de Lenoir"** | *"Enfin un peu de tenue."* | RMB >2s = no spread + 100% HS multiplier | precision | 🟢 |
| **"Mouchoir parfumé"** | *"Mes félicitations."* | 1er HS du run = +25% damage permanent run | precision | 🟢 |
| **"Une dame ne se précipite pas"** | *"On prend son temps."* | Charge LMB 1.5s = explosion impact 2m radius | precision, chaos | 🟡 |

### 🪓 Boucherie — mécanique **chaos physique**

| Nom | Voiceline preview | Effet | Tags | Complexité |
|---|---|---|---|---|
| **"BOUM extra-cuit"** | *"AHA chaîne !"* | Explosion 50% chance chain adjacent radius 4m | chain, chaos | 🟢 |
| **"Pirouette envoyée"** | *"Ils volent, ils volent !"* | Ennemis volent +5m haut, retombent ragdoll 1s | knockback, chaos | 🟡 |
| **"Boucherie joyeuse"** | *"Ça défile !"* | Si 3+ ennemis dans explosion → +20% mag size 5s | chaos | 🟢 |

## 2.3 Boons neutres (5)

| Nom | Voiceline Maître | Effet | Complexité |
|---|---|---|---|
| **"Éclat d'âme nourrissant"** | *"Tiens, un éclat pour toi"* | +5% chance loot éclat bonus par kill | 🟢 |
| **"Métal chaud"** | *"Frappe pendant que c'est chaud"* | Après dash, prochain tir +50% damage 3s | 🟢 |
| **"Bénédiction de l'Enclume"** | *"Mon enclume te protège"* | Max énergie +20 (+1 cœur UI) | 🟢 |
| **"Souffle du Maître"** | *"Je veille sur toi"* | Voicelines random quand low HP + boost +10% damage 5s | 🟢 |
| **"Petit Champignon Lumineux"** 🍄 | *"Un nouvel ami !"* | Champignon cyan suit, heal 1/5s (canon bible) | 🟡 |

## 2.4 Anti-boons (3 marchés Forgeron Noir)

> Cinematic ridicule, le Forgeron Noir : *« Hé hé hé, j'ai un petit marché... »*

| Nom | Lore | Malus | Avantage | Complexité |
|---|---|---|---|---|
| **"Marché de Mauvais Goût"** | *"Beurre dans les yeux !"* | Vision floue >15m | +50% damage all weapons run | 🟡 post-process |
| **"Talon de Ferraille"** | *"Donne un cœur, prends 3 jouets"* | -1 cœur permanent run | 3 boons au prochain coffre | 🟢 |
| **"Pacte Rigolo"** | *"TU VEUX DU CHAOS ?"* | Spawn × 3 ennemis 30s | Tous lâchent éclats × 5 | 🟡 |

---

# MISSION 3 — Movesets par arme

## 🔫 Pépin — l'arme accessible

| Action | Détail | Complexité |
|---|---|---|
| **LMB** | Hitscan instant, 15 dmg, mag 12, 4/s, tracer cyan 100ms | 🟢 |
| **RMB** | ADS zoom 1.5×, accuracy ×2 static | 🟢 |
| **Spé (Shift)** | "Petit cri" — burst 3 tirs rapide, cooldown 6s | 🟢 |
| **Voicelines tir** | *"PIOU !"* / *"Tac !"* / *"J'ai eu !"* | 🟢 |
| **Voicelines kill** | *"OH ! J'ai réussi !"* / *"Maître serait fier !"* | 🟢 |
| **Anim FPS** | Tremblote idle, recoil 2°, fume cyan, mag drop | 🟢 |
| **Pattern** | *Tir précis posé. Panique quand entouré. Gagne confiance avec kills consécutifs.* |

## 💨 Bourrasque — l'arme chaos proche

| Action | Détail | Complexité |
|---|---|---|
| **LMB** | Shotgun 7 pellets cone 20°, 8 dmg/pellet, range 10m, mag 5, 1.5/s | 🟢 |
| **RMB** | "Souffle" cone 4m knockback 0 dmg, interrupt 1s | 🟢 |
| **Spé** | "TORNADE !" vortex stationnaire 3m radius 2s pull ennemis | 🟡 |
| **Voicelines tir** | *"PROUT !"* / *"BAAM !"* / *"YAA !"* | 🟢 |
| **Voicelines kill** | *"WHOUUUU !"* / *"Sayonara !"* | 🟢 |
| **Anim FPS** | Sautille idle, pump-action théâtral reload, muzzle jaune | 🟢 |
| **Pattern** | *Engage close-range, joue avec knockback, danse autour des ennemis.* |

## 🎩 Madame Lenoir — l'arme précision

| Action | Détail | Complexité |
|---|---|---|
| **LMB** | Hitscan instant, 80 HS / 40 body, mag 4, reload 3s, tracer blanc fin | 🟢 |
| **RMB** | Scope 4×, no breath sway static >0.5s, crosshair monocle | 🟢 |
| **Spé** | "Coup d'œil" — silhouettes outline tous ennemis 5s through-walls, CD 15s | 🟡 |
| **Voicelines tir** | *"Tsk."* / *"Acceptable."* / *"Élégant."* | 🟢 |
| **Voicelines kill HS** | *"Précis."* / *"Mes félicitations."* (rare = mémorable) | 🟢 |
| **Voicelines miss** | *"Lamentable."* / *"On se ressaisit, voulez-vous ?"* | 🟢 |
| **Anim FPS** | Immobile élégante, recoil 0°, reload 2 doigts | 🟢 |
| **Pattern** | *Long-range, attend l'ouverture parfaite, juge le joueur quand il rate.* |

## 🪓 Boucherie — l'arme chaos pur

| Action | Détail | Complexité |
|---|---|---|
| **LMB** | Roquette parabolique 12 m/s, explosion 4m, 70 dmg + knockback 8m. Mag 3, reload shell-per-shell 4s | 🟡 |
| **RMB** | "Roquette douce" lobé fort knockback, 30 dmg, AOE 5m | 🟡 |
| **Spé** | "Salve festive" 3 roquettes spread cone, cooldown 12s | 🟡 |
| **Voicelines tir** | *"BOUM !"* / *"ÇA PÈTE !"* / *"AHAHA !"* | 🟢 |
| **Voicelines kill** | *"ENVOLE-TOI !"* / *"C'est la fête !"* | 🟢 |
| **Anim FPS** | Bouge épaules respiration, recoil 8° massive, barillet rotatif visible, smoke roux | 🟢 |
| **Pattern** | *AOE chaos, repousse ennemis, joue avec ragdoll physics.* |

---

# MISSION 4 — Structure des Runs

## 4.1 Macro-structure

| Param | Valeur |
|---|---|
| **Durée run cible** | **25-30 min** (Hadès reference) |
| **Stages combat** | **3** (Vestibule, Cœur, Sanctuaire) |
| **Stage boss final** | **1** (Trône du Forgeron Noir) |
| **Mid-boss** | **1 optionnel** (Cœur stage 2) |

### Rythme cible

```
[Vestibule 6min combat] → [Coffre 30s] → [Marché Forgeron Noir 30s OPTIONAL] →
[Cœur 6min combat + mid-boss 3min] → [Sanctuaire 2min repos + lore] →
[Coffre 30s] → [Trône Boss Final 5min] → [Récompense 1min]
```

**Anti-frustration** : fail 2× même stage → Maître Forgeron *« Coup de pouce »* → +1 cœur permanent run. 🟢

### Progression difficulté

| Stage | Waves | Composition |
|---|---|---|
| 1 Vestibule | 3 | 5/7/8 Cages standard + 1 Rapide W3 |
| 2 Cœur | 3 + mid-boss | 6/8/9 Cages mix + 1 Tank W2 + 2 Sniper W3 |
| 3 Sanctuaire | 3 | 8/10 Cages mix + 1 Mage + 1 Boomer chacune |
| 4 Trône | Boss | Forgeron Noir 2 phases + Cages summons |

## 4.2 Templates de salles (3 handcrafted)

### Template A — "Cour des Marteaux"

| | |
|---|---|
| Disposition | Arena ronde 80m, 6 piliers ColumnBig, melee_pit central |
| Vagues | W1 5 standard / W2 8 + 1 Tank / W3 10 mix + 1 Boomer |
| Durée | ~6 min |
| Gimmick 🟡 | Marteau géant tombe random 30s, télégraphe 2s |

### Template B — "Pont de l'Enclume"

| | |
|---|---|
| Disposition | Couloir 60×20m, 2 perches TowerBig, 4 columns broken |
| Vagues | W1 4 Snipers perches / W2 6 Cages rush / W3 mix tactical |
| Durée | ~6 min |
| Gimmick 🟢 | Pont s'affaisse extrémités (lava visible) |

### Template C — "Fosse aux Étincelles"

| | |
|---|---|
| Disposition | Arena circulaire 40m, lava extérieur, 3 plateformes flottantes centre |
| Vagues | W1 6 Cages rush close / W2 8 + 2 Boomers / W3 chaos + 1 Mage |
| Durée | ~5 min |
| Gimmick 🟡 | Éclats d'âme tombent 10s, sortir cover pour ramasser |

## 4.3 Catalogue ennemis V1 (6)

| Cage | Comportement | Attaque | Voice/SFX | Complexité |
|---|---|---|---|---|
| 🤖 **Marchante** | Walk → stop → tire → walk | Hitscan 10 dmg, télégraphe 0.5s | *"GRRR !"* | 🟢 |
| 🏃 **Rapide** | Court joueur, melee | Slash 15 dmg | *"Clink clink !"* | 🟢 |
| 🛡️ **Tank** | Slow walk, AOE ground pound | Slam 25 dmg radius 3m | *"BOUM BOUM !"* | 🟢 |
| 🎯 **Sniper** | Perché, charge laser, tire | Laser 30 dmg charge 1.5s | *"Tch-tch !"* | 🟡 |
| 💣 **Boomer** | Court, explose mort/proxy | AOE 40 dmg radius 4m | *"Bip-bip-BIP !"* | 🟢 |
| 🔮 **Mage** | Stationary, projectile homing slow | Boule 8 m/s 20 dmg | *"Hmmmmm !"* | 🟡 |

### Contre-strats par arme

| Cage | 🔫 Pépin | 💨 Bourrasque | 🎩 Lenoir | 🪓 Boucherie |
|---|---|---|---|---|
| Marchante | Tir HS simple | Rush close-up | 1-HS instant | 1 roquette OK |
| Rapide | Difficile | **Parfait** close | Hard | AOE écraser |
| Tank | Long snipe head | Difficile (range) | 2 HS placés | **Parfait** gros boum |
| Sniper | Engage cover | Difficile (range) | **Duel** | Roquette arc |
| Boomer | Tir + recule | Attention close | Snipe before close | AOE chain |
| Mage | Dodge + tir | Difficile slow proj | **Snipe parfait** | AOE knockback |

## 4.4 Boss

### Mid-boss : "Le Grand Forgeron Cage" 🟡

| Élément | Détail |
|---|---|
| Identité | Géant Cage barbu cartoon, marteau, voix grave-comique |
| Phase 1 | Marteau swing AOE 2s télégraphe, shrapnel cartoon |
| Phase 2 (<50%) | Colère : tire 3× + summon 3 Cages standard |
| Dialogues | *"JE SUIS LE PLUS GROS !"* / *"TU ES MINUSCULE !"* / *"AÏE MES MARTEAUX !"* / *"PFFF FATIGUÉ"* |
| Victoire | Brise en éclats dorés → âme arme bonus → choix 1 boon entre 3 légendaires |

### Boss final : "Le Forgeron Noir" 🔴

| Élément | Détail |
|---|---|
| Identité | Petit perso ridicule (Bowser-energy), chapeau trop grand, monté sur Machine de Forge |
| Phase 1 | Machine tire boulets télégraphe 2s, Forgeron rit *"AHAHA !"*. Summon 2 Cages |
| Phase 2 (<50%) | Machine se casse, Forgeron descend en **pyjama** (révélation comique), couine *"OUILLE !"* |
| Dialogues | *"TU N'AURAS JAMAIS LES ÂMES !"* / *"JE VAIS TE TRANSFORMER EN MARTEAU !"* / *"AÏE MA SOUPE !"* / *"POURQUOI MOIIII ?"* / *"MAMAN !"* (phase 2) |
| Victoire | Toutes les âmes-armes apparaissent autour de toi (cinematic 30s). Forgeron Noir s'endort dans pyjama, ronfle. Maître Forgeron *« Tu l'as fait mon petit. »* Retour hub avec **toutes les armes unlock permanently**. |

### Différenciation 4 armes vs bosses

| | Mid-boss Grand Forgeron Cage | Final Forgeron Noir |
|---|---|---|
| 🔫 Pépin | Snipe weakpoint torse joyau | Snipe machine P1, duel P2 |
| 💨 Bourrasque | Stun avec souffle, profite openings | Casse pièces machine + tornade |
| 🎩 Lenoir | Solo HS patient, joue phase 2 cooldowns | Sniper Forgeron direct, ignore machine |
| 🪓 Boucherie | Explode Cages summon ensemble | Détruit machine P1, fait voler Forgeron P2 |

## 4.5 Méta-progression

### Monnaie : **Éclats d'âme** 🟢

- Bible canon : *"éclats brillants tombent des ennemis"*
- Gain 5-15 par kill, +100 si victoire boss final
- UI HUD top-right compteur ✨ X100

### Hub évolution visuelle 🟡

| Palier | Visuel hub |
|---|---|
| Run 1 | Atelier vide, 1 enclume, Maître Forgeron seul |
| Run 3 | 🔫 Pépin sur étagère (dialogue spécial) |
| Run 5 | 💨 Bourrasque + 🎩 Lenoir libérées (cinematic accueil 30s) |
| Run 8 | 🪓 Boucherie + Petit Champignon ambient 🍄 |
| Run 10 | Forge complète, 4 armes flottent autour Apprenti |
| Run 15+ | Statues décoratives, Maître Forgeron prépare V2 hook |

### Débloquables V1

| Item | Coût | Complexité |
|---|---|---|
| Voicelines random unlock | Paliers éclats 250/500/1000/2500/5000 | 🟢 |
| Cosmétiques arme 3 skins/arme | 100/250/500 éclats par skin | 🟢 |
| +1 énergie max permanent (max +5) | 200/500/1000/2000/4000 éclats croissant | 🟢 |
| Starting boon (run 10 unlock) | 1000 éclats | 🟡 |

### Débloquables V2 post-MVP 🔴

- 5e arme secrète "Bébé Marteau" (hammer melee)
- Mode "Cauchemar du Forgeron Noir" (hard +leaderboard)
- Daily runs seeded + leaderboards online
- Hub interactif : dialoguer avec armes posées

---

## Pourquoi le joueur va relancer un run ?

| Hook | Source | Force |
|---|---|---|
| Voicelines random unlock | Hadès 21k lines collectionneur | ★★★★★ |
| Hub visuel évolue | Animal Crossing / Cult of the Lamb base | ★★★★★ |
| Build variety boons | Hadès 24 boons V1 = 100+ builds | ★★★★ |
| Cosmétiques skins | Fortnite vache-à-lait | ★★★ |
| Anti-boons curiosity | Forgeron Noir négocie | ★★★ |
| Maître Forgeron dialogue prog | Stardew Valley NPCs | ★★★ |

---

## Scope V1 réaliste (solo Bevy)

| Système | Estimé | Story |
|---|---|---|
| 4 armes movesets | ~3 sem | 531-534 |
| 24 boons + UI Coffre | ~2 sem | 529-530 |
| 6 ennemis FSM + 1 mid-boss + 1 final boss | ~3 sem | 535-536 |
| 3 templates salles handcrafted + remix seed | ~1 sem | (déjà 80%) |
| Méta-progression hub évolutif + éclats persist | ~1 sem | 537 |
| Polish VFX biome volcanique + voicelines BD popup | ~1 sem | 538 |
| **Total V1 shippable** | **~10-11 sem solo** | |

---

## Cross-refs

- [Bible v1](../lore/README.md)
- [Crypts of Anvil location](../lore/locations/crypts_of_anvil.md)
- [Personas (4 armes + Apprenti + Forgeron Noir)](../lore/personas/)
- [Voicelines in-run](../../assets/genomes/roguelite/roguelite_dialogue.toml)
- [Voicelines hub](../../assets/genomes/roguelite/roguelite_hub_dialogues.toml)
- [Modules palette](../../assets/genomes/level_modules.toml)
- [Roadmap Roguelite](../ROADMAP_ROGUELITE.md)
- Stories impl 528-538 : voir `docs/stories/`

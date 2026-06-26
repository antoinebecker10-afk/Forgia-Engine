# Proposition — Hub d'accueil Roguelite Forgia (audit + design d'interface)

**Date** : 2026-06-26 · **Type** : proposition de design (audit + maquettes), pré-stories
**Demande user** : vrai menu d'accueil (Nouvelle partie / Continuer / Options / Quitter) + wizard de
création (nom, style de dégâts Feu/Givre/Éclair/Poison, armes à débloquer), arbre de talents par
style avec combos, Enclume des Âmes + niveau. Bug signalé : « pas de souris au lancement ».
**Méthode** : audit code (scout) + recherche web 4 axes (hubs roguelite, sélection de style,
arbres de talents, UX menu/curseur). Sources en §9.

---

## 1. Audit de l'existant (scout code)

| Composant | État | Détail (file) |
|---|---|---|
| Écran titre | ✅ existe | `forgia-ui/src/lib.rs` — titre FORGIA + boutons « RPG / ROGUELITE RUN / CYBER CITY / QUITTER » + fond vidéo |
| Curseur libre en menu | ✅ par design | `forgia-ui/src/lib.rs` — `release_cursor` OnEnter(Menu/Lobby/Paused/Defeat) ; grab OnEnter(InGame) |
| Choix d'arme | ✅ wizard | `weapon_select.rs` — 4 armes, ← →, stats réelles + élément + matchup + aperçu 3D turntable |
| Identité (nom + couleur) | ✅ (livré ce jour) | `identity.rs` — panneau Lobby nom/couleur, save séparée |
| Enclume des Âmes | ✅ | `meta_shop.rs` — upgrades permanents (Vitalité/Puissance/Armure/Pactole) + déblocage armes + paliers boons, en Âmes |
| Éléments | ⚠️ par-arme | `elements.rs` — **Fire / Poison / Explosive / ArmorPierce** (≠ les 4 voulus). Liés à l'arme, pas au joueur. Combustion (Feu+Poison) existe |
| **Menu « Nouvelle partie / Continuer / Options / Quitter »** | ❌ manque | le menu actuel lance direct, pas de wizard ni de slots |
| **Wizard de création** (nom + style + récap) | ❌ manque | nom/couleur + arme sont 3 panneaux Lobby séparés, pas un parcours guidé |
| **Style de dégâts joueur** (spécialisation) | ❌ manque | les éléments sont sur les armes, pas un choix de build du joueur |
| **Arbre de talents** | ❌ manque | boons per-run au Coffre (liste, pas arbre) ; aucun skill tree |
| **Niveau / XP joueur** | ❌ manque | rien |
| **Enclume hors-run** | ❌ | l'Enclume n'existe qu'au Lobby (pré-run), pas dans un hub d'accueil |

**Conclusion** : on a les briques (menu, curseur, choix d'arme, identité, Enclume, 4 éléments) mais
elles vivent **en panneaux in-game au Lobby**, sans parcours d'accueil ni progression de build.

---

## 2. Le bug « pas de souris »

Le scout confirme que le curseur est **relâché au Lobby**. Le bug vient donc d'un de :

- **Ordre de transition** : `Menu → InGame (grab) → Lobby (release)` — si l'OnEnter(InGame) grab tire
  APRÈS l'OnEnter(Lobby) release la même frame, le curseur reste verrouillé.
- **Placement panneau** : le panneau identité (TOP-LEFT 24,120) chevauche peut-être un autre panneau.

**Fix structurel (et c'est ce que tu demandes)** : déplacer création/choix/Enclume dans un **HUB
d'accueil hors gameplay** (`AppMode::Menu`, curseur libre **par design**) AVANT d'entrer dans la run.
Plus de panneaux cliquables pendant un état « jeu ». À corriger en P1 (vérif ordre + bascule curseur
atomique `grab_mode` + `visible` ensemble, sur transition d'état — jamais par frame).

---

## 3. Ce qui marche (synthèse recherche)

1. **Hub diégétique** (Hades Maison, Gunfire lobby, Roboquest basecamp) : une « pièce » avec des
   stations ; la **porte = lancer la run**. Chaleureux, kid-friendly. Alternative plus simple : **menu
   à onglets** (RoR2/Slay the Spire) — *liste à gauche / aperçu au centre / détails à droite + gros
   bouton LANCER en bas*. **Reco MVP : onglets** (pas de scène 3D à construire), évolution « pièce » plus tard.
2. **Titre minimal 4 boutons**, **Continuer en focus par défaut** si une save existe. Le plus gros bouton = Jouer.
3. **Continuer = charge le profil → dépose dans le HUB** (pas une run aveugle). Si run en cours →
   bandeau « Reprendre (Étage N) ». **Nouvelle partie = wizard + confirmation** (efface la méta = piège kids).
4. **4 styles = 4 verbes clairs** (1 couleur + 1 icône + 1 slogan chacun, **zéro tableau de résistances**) :
   - 🔥 **Feu** (rouge) — *DoT + AOE* : « brûle tout le monde ».
   - ❄️ **Givre** (bleu clair) — *contrôle* : « ralentis et gèle ».
   - ⚡ **Éclair** (jaune) — *burst + chaîne* : « frappe fort, ça rebondit ».
   - ☠️ **Poison** (vert) — *stack DoT* : « empile, ça monte ».
5. **Combos = nommés + VFX flashy + stacks visibles** (Genshin = référence ; RoR2 a dû *afficher les
   stacks* car les joueurs ignoraient le cumul). Max 2-3 combos. Forgia a déjà **Feu+Poison = Combustion**.
6. **Arbre de talents = modèle Gunfire, PAS PoE** : 4 petits arbres élémentaires, **7 nœuds en losange**
   (tronc + 2 branches + **capstone combo**), **1 actif par run**, **1 point par level-up**, respec gratuit
   hors run. **Test du verbe** : chaque nœud décrit une ACTION (« Propage », « Explose »), pas un « +5 % ».
7. **Niveau = XP de participation** (vagues + boss + survie, **versée même en défaite**), ≠ Âmes. Donne
   les **points de talent** (le build) ; l'**Enclume** garde les **stats permanentes** (2 axes, modèle Gunfire).
8. **UX kids** : gros boutons (~2 cm), icônes > texte, **1 décision à la fois**, **impossible de se
   bloquer**, feedback son+anim instantané, wizard **3 étapes** avec stepper + retour arrière + bons défauts.
9. **Curseur** : libre+visible en menu, verrouillé+caché en jeu, basculé **atomiquement** sur transition
   d'état. Menu navigable **souris ET clavier/manette** (focus surligné).

---

## 4. Proposition d'interface

### 4.1 Écran titre

```
                         F O R G I A
                    (logo + fond animé)

                   ▶  CONTINUER            ← focus défaut si save
                      NOUVELLE PARTIE
                      OPTIONS
                      QUITTER

         [Discord]            v0.x        [Crédits]
```
- « Continuer » grisé si aucune save. « Nouvelle partie » → confirmation si une save existe (efface la méta).

### 4.2 « Nouvelle partie » — wizard 3 étapes (stepper + retour arrière)

**Étape 1 — Ton nom** (défaut pré-rempli, presets cliquables, texte optionnel — un kid clique « Suivant » sans rien taper).
**Étape 2 — Ton style de dégâts** :

```
   ÉTAPE 2/3 — CHOISIS TON STYLE              [● ● ○]

  ┌─ 🔥 FEU ──┐ ┌─ ❄️ GIVRE ─┐ ┌─ ⚡ ÉCLAIR ┐ ┌─ ☠️ POISON ┐
  │  (rouge)  │ │ (bleu)     │ │ (jaune)    │ │ (vert)     │
  │ Brûle     │ │ Ralentis   │ │ Frappe     │ │ Empile,    │
  │ tout le   │ │ et gèle    │ │ fort, ça   │ │ ça monte   │
  │ monde     │ │            │ │ rebondit   │ │            │
  └───────────┘ └────────────┘ └────────────┘ └────────────┘
        ▲ sélectionné (bordure + aperçu animé)

              ‹ Retour            Suivant ›
```
**Étape 3 — Récap** : nom + style + **arme de départ** + aperçu de **l'arbre de talents du style** (verrouillé,
« se débloque en jouant ») + **armes à débloquer** (avec coûts en Âmes). Bouton **« FORGER MON DESTIN »** → HUB.

### 4.3 Le HUB (« La Forge ») — après Continuer / création (curseur libre)

Modèle onglets, MVP. Currency + niveau toujours en haut. Porte = lancer.

```
┌─ FORGE ─┬─ ARMES ─┬─ TALENTS ─┬─ ENCLUME ─┐   onglets
│  ◇ Âmes 1240        Niv. 7  [▓▓▓▓░░] 320/500 XP   <Nom> 🔥 │  bandeau haut
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   (contenu de l'onglet actif — voir ci-dessous)            │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│   ⚑ Reprendre la run (Étage 3)        ▶  LANCER LA RUN      │  CTA bas
└─────────────────────────────────────────────────────────────┘
```
- **Onglet ARMES** = le `weapon_select` existant (liste + aperçu 3D + stats + déblocage Âmes).
- **Onglet ENCLUME** = le `meta_shop` existant (Vitalité/Puissance/Armure/Pactole en Âmes).
- **Onglet TALENTS** = l'arbre du style choisi (§4.4).
- **Onglet FORGE** (accueil) = perso + nom/couleur (identité) + résumé (style, niveau, prochain déblocage).

### 4.4 L'arbre de talents (1 par style, losange 7 nœuds — modèle Gunfire)

```
              [ TRONC ]   ← gratuit, identité du style
             /         \
       [Branche A]   [Branche B]
          |              |
       [Nœud A2]      [Nœud B2]
             \         /
            [ CAPSTONE COMBO ]   ← gros nœud doré (réaction signature)
```
- **1 point par level-up** (≈5-7/run) → on remplit son petit arbre en jouant.
- **Capstone = combo nommé** (ex. Feu+Givre = « Vapeur » → explosion), montré par **icône de paire**
  (🔥+❄️→💥) + prompt « il te faut aussi du GIVRE » (style Hades 2 Infusion), pas en %.
- **Respec gratuit hors run**, verrouillé pendant la run.

Exemple arbre **Feu** :

| Nœud | Effet (langage simple) | Valeur |
|---|---|---|
| Tronc | Tes tirs mettent le feu | Brûlure 5/s, 3 s |
| Fournaise (A) | Ennemis en feu prennent plus | +25 % sur cibles en feu |
| Traînée (B) | Le feu saute d'ennemi en ennemi | propagation 3 m |
| **Vapeur** (capstone) | Givré + en feu → **EXPLOSION** | AoE 150 %, rayon 4 m |

### 4.5 Niveau / XP

XP de participation (vague/boss/survie, **même en mourant**) → barre dans le bandeau. Le level-up donne
**1 point de talent** (+ pop-up juicy). Distinct des Âmes (Enclume). « Toute run paie. »

---

## 5. Réconciliation des éléments (décision à valider)

Tu veux **Feu / Givre / Éclair / Poison**. Le code a aujourd'hui **Fire / Poison / Explosive / ArmorPierce**
(liés aux armes). Deux options :

- **Option A (recommandée)** : le **style joueur** = Feu/Givre/Éclair/Poison (les 4 voulus) ; les éléments
  d'arme actuels restent des **traits d'arme** (Explosive/ArmorPierce = particularités de Pépin/Lenoir), et on
  **ajoute Givre + Éclair** comme styles+VFX (Feu et Poison existent déjà). Le style joueur ouvre son arbre.
- **Option B** : remapper les 4 éléments d'arme sur les 4 styles (drop Explosive/ArmorPierce) — plus simple
  mais on perd 2 identités d'arme existantes.

→ **Reco A** : garde l'existant, ajoute 2 éléments (Givre/Éclair) + le concept de style joueur.

---

## 6. Phasage (priorité SHIP — ne pas tout faire d'un coup)

| Phase | Contenu | Réutilise | Poids |
|---|---|---|---|
| **P1 — Menu + curseur** | Titre 4 boutons (Continuer/Nouvelle/Options/Quitter) + fix curseur + Options minimal (volume/FOV/plein écran) | menu + pause_menu existants | **S** |
| **P2 — Hub onglets** | Regrouper weapon_select + Enclume + identité en **onglets** d'un hub (curseur libre) + bandeau Âmes/niveau + bouton LANCER | weapon_select, meta_shop, identity | **M** |
| **P3 — Wizard Nouvelle partie** | parcours 3 étapes (nom → style → récap) + confirmation efface-méta | identity + nouveau « style » | **M** |
| **P4 — Niveau / XP** | XP participation + barre + point de talent par niveau (story-623 Phase F adaptée) | — | **M** |
| **P5 — Arbres de talents** | 4 arbres losange 7 nœuds, data-driven, points/level, respec | boons infra | **L** |
| **P6 — Givre + Éclair + combos** | 2 nouveaux éléments + VFX + 1-2 combos capstone | elements.rs, combustion | **L** |

**MVP « tu vois le parcours » = P1 + P2 + P3** (menu → wizard nom+style → hub onglets avec armes/Enclume/lancer).
Le **build profond** (niveau + talents + nouveaux éléments) = P4-P6, après.

### 6.1 Statut d'implémentation

- **P1 — Menu + curseur** : ✅ **LIVRÉ** (2026-06-26, non commité). `forgia-ui` + `forgia-ui-lib` uniquement (zéro `forgia-mode-roguelite` → aucun conflit terminal parallèle). Décisions appliquées : toutes mes recos (onglets, éléments option A, niveau→points, Continuer=hub).
  - Menu titre 4 boutons : `CONTINUER` (grisé si pas de méta), `NOUVELLE PARTIE`, `OPTIONS`, `QUITTER` ([forgia-ui/src/lib.rs](../../crates/forgia-ui/src/lib.rs) `main_menu_ui`, sous-page `MenuPage{Root,Options}` locale — PAS un AppMode). Démos dev RPG/Cyber City conservées en secondaire.
  - **Fix curseur (root cause nommé)** : course `OnEnter(InGame)→grab_cursor` (LOCK) vs `OnEnter(RunState::Lobby)→release_cursor` (FREE), 2 schedules sans ordre → grab gagnait, curseur verrouillé sous le wizard. Fix = réconciliateur par-frame `sys_force_lobby_cursor_free` (run_if InGame+Roguelite+Lobby, set-if-different) = source unique de vérité du curseur au Lobby. `grab_cursor`/`release_cursor` NON modifiés (ce qui marche est protégé).
  - **Options** : réutilise les contrôles du pause menu via `draw_settings_controls` (extrait, public, DRY) — volume/FOV/plein écran/MSAA/VSync/tonemapping. `cargo check` + clippy 0 warning sur les 2 crates.
- **P2 / P3** : à faire — touchent `forgia-mode-roguelite` (hub onglets regroupant weapon_select+meta_shop+identity, wizard) → **coordonner avec le terminal parallèle** (actif sur ce crate).

---

## 7. Décisions à valider (avant stories)

1. **Hub = onglets (reco MVP) ou pièce 3D diégétique** (plus tard) ?
2. **Éléments : Option A** (ajouter Givre/Éclair, garder l'existant) **ou B** (remapper) ?
3. **Niveau donne des points de talent** (validé par ta demande d'arbre) — OK ? (révise story-623 qui disait « niveau cosmétique pur »).
4. **Combos** : combien au lancement (1 = Combustion existante, ou viser 2-3) ?
5. **Continuer** : reprendre une run en cours OU juste recharger le hub (reco : hub ; run-en-cours = bandeau) ?

---

## 8. Lien avec l'existant / stories

- Réutilise : `weapon_select.rs` (612/613), `meta_shop.rs` (591/616), `identity.rs` (623 Phase E), `elements.rs` (582/588/589).
- Étend : story-623 (identité + niveau) — le niveau passe de « cosmétique » à « donne des points de talent ».
- Nouveau : hub à onglets, wizard, arbres de talents, Givre/Éclair, système de niveau.

## 9. Sources (recherche)

Hubs : [Hades Maison](https://www.pcinvasion.com/hades-guide-house-of-hades-house-contractor-upgrades/) · [Gunfire Talents](https://gunfirereborn.fandom.com/wiki/Talents) · [Roboquest classes](https://roboquest.wiki.gg/wiki/Classes) · [Slay the Spire UI](https://interfaceingame.com/games/slay-the-spire/) · [Game UX UXPin](https://www.uxpin.com/studio/blog/game-ux/).
Éléments/combos : [Genshin reactions](https://genshin-impact.fandom.com/wiki/Elemental_Reaction) · [Borderlands elements](https://www.gamesradar.com/games/borderlands/borderlands-4-elements-elemental-damage/) · [RoR2 status (afficher stacks)](https://riskofrain2.wiki.gg/wiki/Status_Effects) · [Color-coded elements](https://tvtropes.org/pmwiki/pmwiki.php/Main/ColorCodedElements).
Talents : [Skill trees beginner](https://gamedesigning.org/learn/skill-trees/) · [Meaningful skill trees (verb test)](https://gdkeys.com/keys-to-meaningful-skill-trees/) · [Hades 2 infusion](https://screenrant.com/hades-2-elemental-essence-types-benefits-uses/) · [DRG overclocks](https://deeprockgalactic.wiki.gg/wiki/Weapon_Overclocks).
UX/curseur : [Bevy mouse grab](https://bevy-cheatbook.github.io/window/mouse-grab.html) · [NN/g Wizards](https://www.nngroup.com/articles/wizards/) · [NN/g kids](https://www.nngroup.com/articles/children-ux-physical-development/) · [Game Accessibility Guidelines](https://gameaccessibilityguidelines.com/basic/).

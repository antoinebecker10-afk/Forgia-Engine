# Forgia — Vision & Plan

> **Document stratégique — pivot du 2026-06-04.**
> Supersède le funnel « YouTube du gaming / Play→Build→Edit→Monétise » (CLAUDE.md §1, à mettre à jour).
> Sert de source pour le positionnement, la roadmap, et la copy du site.

---

## 1. En une phrase

**Forgia est un moteur de jeu IA-natif : tu apportes ton idée et tes assets, l'IA construit ton jeu 3D.**

Pas du no-code (graphes visuels). De l'**IA-code** : tu décris ce que tu veux en langage naturel, un studio-dans-une-boîte piloté par IA assemble un vrai jeu jouable.

---

## 2. Le problème qu'on résout

Faire un jeu 3D demande aujourd'hui de **devenir ingénieur moteur** : Unity, Unreal, Godot, ou les éditeurs no-code (Roblox, Dreams) imposent tous d'apprendre *leur* outil, leur logique, leurs limites. Des mois avant le premier prototype jouable. C'est la barrière qui tue 99 % des idées de jeu.

Les éditeurs « no-code » ont promis de lever cette barrière — et ont échoué sur le même mur : soit trop limités (on ne fait rien de vrai), soit aussi complexes que du code déguisé.

**Le déblocage de 2026, c'est l'IA agentique.** Un agent qui comprend l'intention créative et la traduit en jeu réel. C'est ce que Forgia industrialise.

---

## 3. La proposition de valeur — le pivot

| | Ancien paradigme (no-code) | Forgia (IA-native) |
|---|---|---|
| Promesse | « Crée sans coder » | « Décris, l'IA construit » |
| Charge | **Toi** apprends l'outil | **L'outil** te comprend |
| Interface | Graphes, nodes, menus | Langage naturel + tes assets |
| Limite | Le no-code plafonne vite | L'IA écrit du vrai code moteur |
| Contrôle | Total mais lent | Rapide d'abord, fin **à terme** (édition) |

**La vraie percée de Forgia n'est pas un éditeur. C'est un codebase + une mémoire + des process conçus pour qu'une IA fasse *exactement* ce qu'on lui demande, de façon fiable.** C'est le moat : pas le moteur seul, mais le moteur *rendu pilotable par IA*.

---

## 4. Pour qui — les 3 audiences dans le temps

1. **Aujourd'hui — le studio (nous).** On prouve le modèle en **sortant un vrai jeu**. Tant qu'un jeu n'est pas sorti, rien d'autre ne compte.
2. **Demain — le créateur.** Quelqu'un qui veut faire son jeu 3D sans devenir programmeur moteur : il **importe ses assets**, décrit son jeu, l'IA le construit.
3. **Plus tard — le créateur exigeant.** Il **édite finement** ce que l'IA a généré (modes Build/Edit), assisté par l'IA. Contrôle chirurgical par-dessus la génération.

---

## 5. Les 3 piliers

**Pilier 1 — Une IA qui fait exactement ce qu'on demande.**
Fiabilité par ingénierie : protocole Concept-First, observabilité totale (sensors JSON sur chaque système), Quality Gates mesurables, règles anti-régression (no-speculative-fix), mémoire persistante du projet. *C'est l'apparatus qui transforme « demander à une IA » en résultat fiable.*

**Pilier 2 — Création asset-first.**
Le créateur apporte ses assets (packs CC0, ses propres modèles). Le moteur les ingère et les vérifie (pipeline d'import + lockfile reproductible). Le jeu se construit *autour de son univers visuel*, pas autour de primitives génériques.

**Pilier 3 — Des vrais jeux, un vrai moteur.**
Bevy / Rust, 3D, physique, terrain procédural, combat, systèmes RPG. Pas un jouet : un moteur de production capable de jeux finis et performants.

---

## 6. Le plan — séquençage honnête

> Règle d'or : **on gagne le droit à la plateforme en sortant un jeu d'abord.** Amazon a été une librairie avant AWS.

### Phase 0 — SORTIR LE PREMIER JEU *(maintenant, priorité absolue)*
Prouver le couple moteur + process IA en livrant **un jeu complet, jouable, poli, sorti**. Un seul. Définition de « fini » dure et écrite (genre, durée, condition de victoire, barre de polish, plateforme cible). **C'est l'unique priorité.** Tout ce qui ne débloque pas ce ship est différé.

**Premier jeu = le Roguelite** — un **FPS roguelite type *Gunfire Reborn*** (runs, ascensions/boons, loot, armes multiples, boss, progression par vagues). Boucle courte, proche d'un « fini » shippable. Référence de genre claire = scope cadré.

#### Modèle d'exécution — deux tracks couplés

Le développement se mène sur deux pistes parallèles, mais **subordonnées au ship** :

- **Track SHIP — Roguelite** : le véhicule de sortie. Tout y converge.
- **Track FORGE — RPG** : banc d'essai où l'on construit les **outils durs** (animation/rig procédural, skinning, locomotion…) qui, une fois prouvés, **refluent dans le Roguelite**. Ex. concret : le pipeline d'animation procédurale développé côté RPG accélère la création de personnages **~×2** — bénéfice directement réinjecté dans les ennemis/persos du Roguelite.

**Test d'autorisation du Track FORGE** (garde-fou anti-second-océan) :
> *« Ce travail RPG construit-il un outil réutilisable qui accélère le ship du Roguelite ? »*
> - ✅ Oui (pipeline anim/rig, skinning, locomotion) → greenlight.
> - ❌ Non (contenu RPG pour le RPG : quêtes, dialogues, lore) → Phase 1+, pas maintenant.

Le Track FORGE est légitime **tant qu'il est un multiplicateur d'outillage pour le ship**, jamais comme fin en soi.

### Phase 1 — Ouvrir la création aux autres
Généraliser la boucle « importe assets + dirige l'IA → jeu » au-delà du studio. Un créateur externe fait son jeu via l'IA.

### Phase 2 — Édition & raffinement
Le créateur édite ce dont il a besoin (modes Build/Edit), assisté IA. Contrôle fin par-dessus la génération.

### Phase 3 — Écosystème *(destination, pas promesse)*
Partage et distribution de créations. **Volontairement pas cadré comme « monétise/publie » aujourd'hui** — c'est une destination qui se mérite, une fois les phases 0→2 prouvées.

---

## 7. État actuel — honnête (audit 2026-06-04)

- **Moteur : sain.** 123 crates, Bevy 0.18, compile 0 erreur / 0 warning, tourne à **141 FPS** (4.8 ms/frame).
- **Deux fondations de jeu jouables :**
  - **Forgia RPG** — monde ouvert, quêtes, PNJ, dialogues, inventaire, biomes procéduraux.
  - **Roguelite Arène** *(FPS roguelite type Gunfire Reborn — le ship)* — vagues, boons/ascensions, économie (or/âmes), armes multiples, boss, HUD loadout TAB, feedback de combat.
- **Infrastructure IA-execution :** observabilité étendue (60+ sensors), règles de process, système de mémoire — l'apparatus qui rend le dev piloté-IA fiable.
- **Pipeline d'assets :** import + vérification de packs externes (18 packs KayKit, lockfile SHA256).

> Ce qui *manque* pour le ship n'est pas de la santé moteur — c'est de la **complétude de jeu** (boucle début→fin sans trou bloquant). À cadrer en Phase 0.

---

## 8. Copy prête pour le site

### Hero — taglines candidates
1. **« Décris ton jeu. Importe tes assets. L'IA le construit. »**
2. **« Le premier moteur qui te comprend, au lieu de te faire apprendre. »**
3. **« Ton idée. Tes assets. Ton jeu — construit par l'IA. »**

### Sous-titre
*Forgia est un moteur de jeu 3D piloté par IA. Tu apportes ton univers et ta vision ; une IA studio-dans-une-boîte assemble un vrai jeu jouable. Pas de graphes no-code. Pas de mois d'apprentissage. Du langage naturel et tes assets.*

### Bloc 3 features
- **🤖 L'IA fait le travail** — Tu décris, elle construit. Combat, niveaux, IA ennemie, systèmes RPG : assemblés à partir de ton intention.
- **🎨 Tes assets, ton univers** — Importe tes modèles 3D ou des packs CC0. Le jeu se bâtit autour de *ton* style.
- **⚙️ Un vrai moteur** — Bevy/Rust, 3D, physique, terrain procédural. Des jeux finis et performants, pas des prototypes jouets.

### Roadmap publique (simplifiée)
1. **Aujourd'hui** — Notre premier jeu, construit avec Forgia. *(la preuve par l'exemple)*
2. **Bientôt** — Crée le tien : importe tes assets, dirige l'IA.
3. **Ensuite** — Édite et affine ce que l'IA a généré.

### CTA
- Principal : **« Voir le jeu »** *(quand Phase 0 livrée — la démo EST l'argument)*
- Secondaire : **« Rejoindre la liste d'attente créateurs »**

> ⚠️ **Discipline copy** : ne rien promettre publiquement au-delà de ce qui est livré. Tant que Phase 0 n'est pas sortie, le site vend **le jeu** (réel), pas la plateforme (future). La crédibilité = montrer un jeu fini, pas une vision.

---

## 9. Garde-fous internes *(NE PAS mettre sur le site)*

1. **Scoper le MVG brutalement.** « Sortir un jeu » sans définition dure devient un tapis roulant infini — le même piège que « bâtir une plateforme », en plus petit. Écrire noir sur blanc : quel jeu, quelle durée, quelle fin, quelle barre de polish, quelle plateforme.
2. **Anti-jardinage moteur.** 123 crates + sensors + rules, c'est séduisant à polir indéfiniment. Chaque tâche se juge à : *« ça débloque le ship du jeu, oui ou non ? »* (discipline Friction Log / sub-agent game-maker).
3. **Pas de généralité prématurée.** Ne pas construire « pour des créateurs » avant d'être soi-même le créateur qui a shippé. Cf. le pack registry runtime : construit, complet, **dormant, zéro consommateur** — symptôme exact à ne pas reproduire.
4. **L'IA-fiabilité sert le ship, pas l'inverse.** Optimiser les process pour l'exécution IA est un moyen, pas une fin. Le livrable reste le jeu.

---

## 10. Décisions ouvertes

- **Quel jeu sort en premier — RPG ou Roguelite ?** Cadre tout le reste de la Phase 0. *(Le Roguelite a une boucle plus courte et plus proche d'un « fini » → candidat naturel au premier ship ; le RPG est plus ambitieux/long.)*
- **Mise à jour `CLAUDE.md §1`** (constitution, Lock §6) — requiert ton go explicite pour acter cette vision dans le contrat IA.

---

*Rédigé 2026-06-04. Base : audit codebase du jour + clarifications vision (Antoine).*

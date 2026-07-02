# Rapport — Forgia « Gunfire-like » : état réel, identité & recommandations fun/addictif

> **Date** : 2026-07-02. Sources : balayage code 3 axes (level design/parcours, armes qui
> parlent/juice, boucle méta/rétention) + état frais des sessions P0-2→P0-4 + HUD (story-640/641/642/644,
> commits `fdaec7d`→`018ded8`). Croisé avec `direction-forgia-gunfire.md` (verrouillée),
> `forgia-gunfire-masterplan-2026-07-01.md`, `gunfire-reborn-parity-audit-2026-07-01.md`.
> ⚠️ Corrige des lectures périmées : la défense tri-couche + réactions + HUD segmenté SONT livrés.

---

## 1. TL;DR — le verdict

**Le moteur de combat « Gunfire » est FAIT et frais (Phase 0 ≈ 95 %). Le jeu, lui, n'est pas
encore fun en boucle** : il rejoue la même arène, ses armes parlent **en silence**, et gagner
ne déclenche **aucun écran de victoire**. Les 3 chantiers qui transforment la base technique
en jeu addictif :

1. **Donner une VOIX aux armes** (l'identité n°1 est à 60 % — moteur + 90 répliques + personas,
   mais **0 % audio** : les armes écrivent, elles ne parlent pas).
2. **Consommer le stage-graph** (générateur 7 types de salles complet + testé, **jamais lu** —
   le joueur voit 2 arènes en alternance codée en dur) et **intégrer le parcours** comme tissu
   conjonctif entre salles (l'identité « niveaux uniques » est à 40 %, déconnectée de la run).
3. **Sceller la boucle** : l'écran de VICTOIRE ne se déclenche jamais (story-603), la maîtrise
   d'arme a une structure mais 0 consommateur, pas de « best run ». Une boucle addictive doit
   se fermer avec une célébration.

---

## 2. Ce qui est FAIT (état frais, vérifié)

### 2.1 Combat & défenses — ✅ ~95 % (Phase 0 quasi close, cette semaine)
- **Défense tri-couche** Vie/Bouclier/Armure (`DefenseLayer`, forgia-damage) : ennemis par
  archétype (Tank=armure, Runner/Sniper=bouclier, Boss=les deux) + joueur (bouclier 50 régén).
  Genome + sensor + validé runtime. (story-640, `fdaec7d`)
- **4 éléments + moteur de réactions générique** : Feu/Poison/Électrique/Perforant ;
  Combustion (Feu+Poison), Miasma (Élec+Poison, DoT %PV max), Surcharge (Feu+Élec) —
  `ReactionTable` data-driven, validé en jeu. (story-641)
- **Affinité élément↔couche** : Feu→Vie, Électrique→Bouclier, Perforant/Poison→Armure
  (×1.5 / ×0.75) sur bonus + réactions + arc + Miasma ; vulnérabilité électrique (+10 %)
  jusque sur le hit de base ; affinité du hit de base prête derrière toggle genome (OFF).
  (story-642, 4 commits)
- **HUD défensif segmenté** : barre Bouclier/Armure joueur (egui) + mini-barres sur nameplate
  ennemi (3D, ancrées à la tête — fix boss géant). (story-644 Inc.1+2, validé user)
- Reste Phase 0 : réaction **Manipulation** (déférée — conflit de paire avec Surcharge +
  collision arena-bot terminal perf), icônes de statut (HUD Inc.3).

### 2.2 Armes qui parlent — ⚠️ 60 % (le corps existe, la voix manque)
- **Moteur de barks complet** (728 LOC, pattern Hadès) : pools pondérées, cooldown par ligne,
  lock global 2,5 s, plafond 12/min. **90 répliques** : ~20 par arme × 4 personas distinctes
  (Pépin timide, Bourrasque pétillante, Lenoir snob — seule à juger les ratés —, Boucherie
  boucher joyeux) + 12 lignes Forgeron Noir (boss). 11 déclencheurs (kill, miss, low-HP,
  idle, reload, pickup, swap, stage clear, boss intro/combat/defeat). Bulle UI couleur persona.
  35 tests verts.
- **Gimmicks mécaniques par arme** : jauge de confiance Pépin (+2 %/stack, HUD cœurs, validé),
  Bourrasque SMG rafales, Lenoir one-shot tête (>40 % HS = métrique), Boucherie roquette
  balistique + knockback. 4 Ultimes (F) distincts.
- **🚨 AUDIO : 0 %.** Les OGG existent (`assets/audio/roguelite/weapons/*.ogg`) mais ne sont
  **jamais joués**. `forgia-audio-voicelines` = scaffold 16 LOC. Pas de TTS, pas de ducking.
- Bloqué : tirs alternatifs AC2/AC3 des 4 armes (décision keybind Q/RMB jamais tranchée).

### 2.3 Level design & parcours — ⚠️ 35-40 % (infrastructure dormante)
- **Stage-graph générateur COMPLET et jamais consommé** : 7 `StageKind` (Combat/Elite/Shop/
  Event/Treasure/Rest/Boss), pondération Slay-the-Spire, déterministe, 50+ tests… et
  `graph.stages[depth].kind` n'est lu **nulle part**. Le dispatch réel = `stage_id_for_depth`
  codé en dur : Crypts ↔ Forge en alternance, boss toujours Crypts.
- **2 arènes** seulement (solver de placement story-485 solide : covers, sniper perch, melee
  pit, sight-lines). 3 templates handcrafted du GDD : **0 implémenté**.
- **Boss = spawn + enrage à 50 %**, aucune mécanique (pas de phases, pas de patterns, pas de
  dialogues wired alors que 12 lignes Forgeron Noir existent).
- **Parcours platformer** (l'atout unique) : kit 1200 instances, 3 zones, obstacles animés
  (marteaux pendules, croix rotatives, blocs coulissants — genome hot-reload), collectibles
  (Couronne rétrécissante, Cœur +PV, Or, Âmes), checkpoints. **MAIS** : pas de tracking de
  progression, déconnecté de la boucle (annexe post-boss), et le portail retour n'émet
  jamais la victoire.
- **Traversal** : dash 4 m / 2 charges seulement (choix assumé GDD, audience accessible).
- Vagues : 3 + boss, 4/6 archétypes (Boomer/Mage manquent), budget de difficulté généré
  mais **pas consommé** par le spawner (pas d'escalade réelle).

### 2.4 Juice — ✅ 80 %
Killfeed AAA (top-right + bannières multi-kill), damage numbers 3D par zone, hitstop 50 ms,
screenshake + chromatic + FOV punch + camera kick, muzzle 5 couches, tracers par arme,
headshot flash. Manque : **audio spatial** (impacts silencieux), VFX d'ultimes placeholder,
télégraphes ennemis (P1-3), ragdoll réel.

### 2.5 Boucle méta — ✅ 70 % (saine mais peu profonde)
- Double monnaie (Or perdu à la mort / **Âmes toujours conservées** — excellent pour la
  rétention douce), Enclume 4 upgrades permanents, déblocage armes (60/150/250 Âmes) +
  paliers de boons (80/200/400), save atomique versionnée, écran de défaite kid-friendly
  avec FTUE 1ʳᵉ mort (très bon).
- **18 boons** (cible LITE = ~40), 6 tags, légendaire à 3 tags. Zéro trade-off, empilement
  additif (pas multiplicatif), tirage non pondéré par rareté.
- **🚨 Écran de VICTOIRE ne se déclenche jamais** (`EndRunEvent(Victory)` jamais émis,
  story-603). `weapon_levels` (maîtrise) : structure présente, **0 consommateur**.
- Pas d'élites, pas d'ascension, pas de best-run/stats.

---

## 3. Les 3 trous qui empêchent « fun & addictif »

| # | Trou | Pourquoi ça tue le fun | Preuve |
|---|---|---|---|
| 1 | **Armes muettes** | L'identité n°1 (« un FPS où tes armes sont des créatures vivantes ») n'est PAS perçue : une bulle de texte en pleine fusillade ne se lit pas. Hadès sans voix ne serait pas Hadès. | 90 répliques écrites, 0 jouée en audio |
| 2 | **La run rejoue la même arène** | La dopamine roguelite = « qu'est-ce qui m'attend derrière la porte ? ». Ici : la même porte. Le générateur de variété existe, débranché. | `stage_id_for_depth` hardcodé ; `graph.stages[].kind` jamais lu |
| 3 | **La boucle ne se ferme pas** | Gagner ne produit RIEN (pas d'écran, pas de récap, pas de récompense de victoire). La maîtrise d'arme n'avance pas. Aucune raison mécanique de « une dernière run ». | Victory jamais fired ; `weapon_levels` sans consumer |

---

## 4. Recommandations priorisées

### 🥇 R1 — « Les armes parlent VRAIMENT » (identité, ~1 sem, ROI max)
1. **Voix gibberish par persona** (recommandé) : synthèse type Animal Crossing/Banjo-Kazooie —
   syllabes pitchées par personnalité (Pépin aigu hésitant, Bourrasque rapide, Lenoir trainant
   hautain, Boucherie grave tonitruant). **Zéro coût VA, zéro localisation, 100 % cartoon**,
   et ça rend chaque bulle *audible* sans enregistrer 90 lignes. Les OGG par arme existent déjà
   comme base de timbre.
2. Brancher `bevy_kira_audio` dans `sys_trigger_combat_barks` (+ ducking musique −6 dB, le
   squelette story-478 existe) + petit « blip » d'apparition de bulle a minima dès cette semaine.
3. **Portrait/wiggle de la bulle** : la bulle pulse/rebondit à l'émission (2-3 h, egui).
4. Wired les 12 lignes du **Forgeron Noir** sur le boss (elles existent, triggers existent).
→ *Critère : les yeux fermés, on sait quelle arme vient de tuer.*

### 🥈 R2 — « Chaque salle est une promesse » (level design, ~2-3 sem — Phase 2 du masterplan)
1. **P2-1 Consommer le RunGraph** : lire `stages[depth].kind`, enchaîner 3 salles Combat +
   1 Boss en clear-to-progress (les briques existent : portes, stage dispatch, layouts).
2. **P2-2 Portail de choix** : 2 portes typées après clear (`draw_portal_overlay` est déjà
   en dead_code !) — LE moment de décision roguelite.
3. **Intégrer le PARCOURS dans la boucle** (l'identité « niveaux uniques ») : les segments
   platformer deviennent les **couloirs entre salles** — 60-90 s de traversal avec
   collectibles risk/reward (Âmes en hauteur, Cœur au bout d'un défi de marteaux) au lieu
   d'une annexe déconnectée. C'est le différenciateur vs Gunfire (qui n'a que des couloirs
   morts) et c'est déjà construit à 40 % (obstacles animés + checkpoints + collectibles).
4. **Oser les obstacles du parcours DANS les arènes** : un marteau pendule au milieu d'une
   salle de combat = signature Forgia (les genomes obstacles sont hot-reload).
→ *Critère : deux runs consécutives ne se ressemblent pas.*

### 🥉 R3 — « Sceller la boucle » (rétention, ~1 sem, quick-wins)
1. **Fix Victory** : émettre `EndRunEvent(Victory)` au portail retour (story-603) + écran de
   célébration avec récap (Âmes, boons, kills, temps, **les 4 armes commentent la victoire** —
   les lignes stage-clear existent).
2. **Wirer la maîtrise d'arme** : `weapon_levels` → +X %/niveau (consumer 1 journée) + affichage
   Enclume. Chaque run fait progresser QUELQUE CHOSE de visible même en cas de défaite.
3. **Best run / stats** persistantes (temps, vague max, kills) sur l'écran d'accueil.
4. **Boons** : passer l'empilement en **multiplicatif** + tirage pondéré par rareté +
   3-4 boons à **trade-off** (la tension du choix est le sel du roguelite). P3-3 du masterplan.

### R4 — Combat spice (après R1-R3, Phase 1/4 du masterplan)
- **Télégraphes ennemis** (P1-3, windup 0,25 s) — l'anti-frustration n°1.
- **Boss avec 2 phases réelles** (patterns + adds + dialogue Forgeron Noir).
- **Élites** (P4-1) en dernière vague + un affixe visuel.
- Trancher les **keybinds Q/RMB** (débloque 6 tirs alternatifs déjà spécifiés).
- Manipulation : trancher Surcharge-vs-Manipulation sur la paire Feu+Élec (après coordination
  terminal perf sur arena-bot).

### Ce qu'il ne faut PAS faire (discipline de scope, direction verrouillée)
- ❌ Courir après 71 armes / 148 scrolls (cible LITE : 6-8 armes, ~40 boons).
- ❌ Traversal avancé (wall-run/grapple) — le dash + parcours suffit à l'audience.
- ❌ Ascension R1-R8 avant le premier playtest externe (1 palier suffit au ship).
- ❌ Co-op avant le ship.

---

## 5. Plan d'attaque suggéré (6 semaines)

| Sem. | Chantier | Livrable jouable |
|---|---|---|
| 1 | R3 (Victory + maîtrise + best-run) + R1.2/R1.3 (blip + bulle animée + boss barks) | La boucle se ferme, le boss parle |
| 2 | R1.1 (gibberish par persona + ducking) | **Les armes parlent à voix haute** |
| 3-4 | R2.1/R2.2 (RunGraph consommé + portails de choix) | Une run = 3 salles + boss, choix de porte |
| 5 | R2.3 (parcours intégré entre salles) | **L'identité « niveaux à parcours » existe** |
| 6 | R4 (télégraphes + boss 2 phases + boons multiplicatifs/trade-offs) → **PLAYTEST #1 externe** | Le jeu est montrable et testable |

**Après le playtest** : Phase 3 (2 armes swappables + inscriptions) et le contenu (40 boons,
6-8 armes, 3 actes) — guidés par les retours, pas par la parité.

---

*Rapport généré le 2026-07-02 (balayage 3 axes + état sessions P0/HUD). Le combat est prêt ;
le fun se joue maintenant sur la voix, la variété des salles, et la fermeture de la boucle.*

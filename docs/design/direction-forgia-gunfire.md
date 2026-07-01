# Direction créative — Forgia Roguelite (Gunfire-lite)

> **Statut : VERROUILLÉE 2026-07-01.** C'est la boussole avant d'attaquer P0. Toute
> nouvelle idée passe le test « est-ce que ça sert cette direction ? » — sinon, backlog gelé.
> Sources : les 3 documents `docs/audit/gunfire-reborn-parity-audit-2026-07-01.md`,
> `docs/audit/gunfire-reborn-level-design-audit-2026-07-01.md`,
> `docs/audit/forgia-gunfire-masterplan-2026-07-01.md`.

---

## 0. Le pitch en une phrase

**Un FPS roguelite où tes armes sont des créatures vivantes qui parlent.** Un Forgeron
descend dans les royaumes-forge, chaque flingue ayant sa personnalité et ses répliques
réactives. **Gunfire-like** dans la structure (salles → boss, éléments, boons, inscriptions),
**« armes qui parlent »** dans l'âme.

---

## 1. Scope verrouillé — la ligne d'arrivée (« Gunfire-LITE »)

**On ne vise PAS la parité 100 % Gunfire** (71 armes / 148 scrolls / 4 actes / 7 boss / R1-R8
= 5 ans + une équipe). Cible réaliste solo, ~5-7 mois :

| Élément | Cible LITE | (Gunfire réf.) |
|---|:---:|:---:|
| Armes | **6-8** | 71 |
| Boons / scrolls | **~40** | 148 |
| Actes | **3** | 4 |
| Salles par acte | **~4** | 4-6 |
| Boss | **3-4** | 7 |
| Difficulté | **1 mode + 1 palier d'ascension** | R1-R8 |

**Règle d'or** : backlog **gelé** (les idées vont dans une liste, jamais direct dans le code).
Contenu supplémentaire = **post-ship**. « Done » = les chiffres ci-dessus, pas « comme Gunfire ».

---

## 2. Identité — le pilier central : les armes qui parlent

C'est **LE différenciateur** (un clone ne se vend pas) et ça colle à l'ADN IA-natif de Forgia.
**Les armes SONT les personnages.**

- **Pépin** (pistolet) — apprenti timide · **Bourrasque** (SMG) — vent extraverti ·
  **Mme Lenoir** (sniper) — aristo snob · **Pompe** (ex-Boucherie, shotgun givrant) — brutal.
- **Barks réactives** : kill, miss, reload, low-ammo, élément déclenché, réaction élémentaire,
  entrée de boss. Le HUD montre l'arme « qui parle ».
- Tout renforce ce pilier : les compétences F/Q ont la **saveur de la personnalité** de l'arme,
  les inscriptions ont une voix, la mort a une réplique.

> Règle : à chaque décision, demander « est-ce que ça rend les armes plus vivantes ? ».

---

## 3. Le monde & les 3 actes (fiction)

**Fantasy de forge.** Tu es un **Forgeron** ; tes armes sont des créations vivantes. La run =
descente/traversée des royaumes-forge. Réutilise les biomes existants (crypts volcanique +
forge plaines) et en ajoute un.

| Acte | Titre | Ambiance | Réutilise |
|---|---|---|---|
| **I** | **Les Cryptes de l'Enclume** | cryptes sombres, forge-nécropole | `crypts_of_anvil.md` (bible existante) ✅ |
| **II** | **La Fonderie** | fonderie en fusion, lave, machinerie | biome volcanique |
| **III** | **Le Sanctuaire du Forgeron** | maître-forge, plaines célestes, boss final | stage forge plaines |

Chaque acte : palette + roster d'ennemis + faiblesse élémentaire du boss distincts.

---

## 4. Modèle de loadout — HYBRIDE (décision #4 affinée)

- **Au menu d'accueil** : tu **construis ta paire de départ** (2 armes parmi celles débloquées)
  + tu **échanges les inscriptions** (toutes combinaisons possibles). C'est ta « build meta ».
- **En run** : tu **trouves et échanges** d'autres armes/inscriptions (découverte roguelite).
- → Tu **démarres** avec ton loadout choisi, mais la run peut le faire évoluer. Le meilleur des
  deux mondes : identité de build (Forgia-original) + chaos de découverte (Gunfire).

---

## 5. Combat & systèmes — décisions verrouillées

- **Compétences** : **F = primaire instantanée + cooldown** (par arme, saveur personnalité),
  **Q = secondaire** (+ charges/ressource). L'**Ultime 10 s** n'est **pas jeté** → il devient un
  **talent méta débloquable** sur une **3e touche**, plus tard (post-P1).
- **Défense tri-couche** (ennemis **et** joueur) : **Vie (rouge) / Bouclier (bleu, régénère
  hors combat) / Armure (jaune)**. Le Bouclier s'empile par-dessus la `combat::Health`.
- **4 éléments** : **Feu / Poison / Électrique / Perforant**. Couplage défense :
  **Feu → Vie · Électrique → Bouclier · Perforant → Armure · Poison → DoT pur** (pas de couche dédiée).
- **Table de réactions** (2 statuts co-présents) :
  - **Combustion** (Feu + Poison) — burst AoE *(existe déjà)*
  - **Miasma** (Électrique + Poison) — DoT % PV, stackant
  - **Surcharge** (Feu + Électrique) — décharge/AoE
  - **Perforant** = brise-armure (pas de réaction, interagit avec la couche Armure)

---

## 6. Portée technique

- **Solo au launch. Co-op (netcode lightyear) = post-ship** (double la complexité).
- **Rust → 1.96.1 maintenant.** **Bevy 0.19 = différé** (bloqué par `bevy_rapier3d`, sans ETA) —
  surveiller mensuellement, ne rien bloquer dessus.

---

## 7. L'ADN Forgia — à ne jamais lâcher

- **Chaque système livré = genome TOML (data-driven, hot-reload) + sensor `forgia2_*.json`.**
- **No-hardcode** (valeurs gameplay en genome), **observable** (diagnostic sans deviner).
- **Discipline de scope** + **playtest externe tous les 15 jours** dès que la boucle est fun.
- Une phase à la fois, `cargo check` après chaque story, story-done-gate avant `DONE`.

---

## 8. Prochaine étape

Direction verrouillée → on attaque **P0** (voir le masterplan §5) :
1. `rustup update` → Rust 1.96.1.
2. **P0-1** : extraire les stats ennemis hardcodées (`enemies.rs`) → `roguelite_enemies.toml`.
3. **P0-2** : `DefenseLayer{Health,Shield,Armor}` (le pilier structurant).

*Direction créative verrouillée le 2026-07-01. Modifiable, mais tout changement = décision
consciente, pas dérive.*

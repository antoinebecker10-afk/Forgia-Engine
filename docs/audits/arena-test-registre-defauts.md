# Registre des défauts — banc Arena Test (story-667)

> **À quoi sert ce fichier.** Chaque défaut rencontré en construisant cette carte,
> avec sa cause profonde. C'est la matière première de la méthode : une règle de
> construction ne vaut que si elle aurait **empêché** une de ces lignes.
>
> Il existe parce que ces 35 défauts n'étaient consignés nulle part de façon
> exploitable — dispersés dans 700 lignes de story, donc réappris à chaque passe.
> **Toute nouvelle ligne s'ajoute ici avant d'être corrigée.**

Dernière mesure : 2026-07-29 · carte `arena_test_crypte_vertical.toml` 120 × 60 m,
7 salles, 8 routes, 80 blocs, 6 rampes, 30 lumières.

---

## Les six classes de cause

| Classe | Cause profonde | Défauts |
|---|---|---|
| **A** | **Forme de mesure fausse** — on mesure la bonne grandeur avec la mauvaise géométrie | 1, 2, 4, 15, 16, 17, 35 |
| **B** | **Discrétisation** — la trame déforme ce qu'on croit poser dessus | 13, 19, 20 |
| **C** | **Littéral qui dérive de l'intention** — une coordonnée écrite à la main cesse de respecter ce que le plan déclare | 11, 12, 21, 22, 23, 24, 33 |
| **D** | **Géométrie non dérivée** — une valeur est déclarée deux fois au lieu d'être déduite une fois | 7, 10, 25, 26, 27, 28, 30, 31 |
| **E** | **Source auto-référente ou périmée** — on lit sa propre sortie, ou un état qui n'existe plus | 6, 29, 32, 34 |
| **F** | **Contrôle qui passe à vide** — le vert ne prouve rien | 14, 18, 26, 34, 35 |
| — | Hors classe (bugs ponctuels) | 3, 5, 8, 9 |

Classement **provisoire** : il est en cours de validation par une passe dédiée.
Ce qui est établi, ce sont les faits ci-dessous, pas la taxonomie.

---

## Le registre

### A — Forme de mesure

1. **Sol découpé sur l'apothème d'un hexagone** au lieu du rayon circonscrit :
   sommets à 30 m, murs à 26 m, sol de rayon 26 → **6 coins vides où l'on tombe**.
2. **Spawn noyé dans le sol** : `spawn_pos.y = 0.5` alors que la demi-hauteur de
   capsule vaut 1,0.
4. **Hauteur de marche confondue avec hauteur de volume** : une caisse de 2 m
   atteinte depuis une caisse de 1 m est une marche de **1 m**, pas de 2.
15. Contrôle de dégagement testé à **exactement `largeur/2`** → les murs qui
    *définissent* le couloir déclenchent.
16. Le même contrôle **ignorait Y** : les piles sous un tablier faisaient échouer
    la route qui passe **au-dessus**.
17. Le même contrôle **gonflait l'AABB du rayon joueur** alors que la capsule est
    un **disque** : coin du pavé gonflé à `r·√2`, soit **+41 %** dans les
    diagonales → 3 faux positifs sur 15 obstructions annoncées.
35. « 52 positions hors trame de 2 m » : le test juge la trame sur le **centre**
    du pavé, or un pavé de 2 m posé sur une trame de 2 m a forcément un centre
    **impair**. Le contrôle produit des faux positifs — la trame se juge aux
    **arêtes**.

### B — Discrétisation

13. Une route de 5 m ne passait pas **en diagonale** dans un jeu de 5 m.
19. **`snap()` déplaçait la roche de 1 m une bande sur deux.** Une bande de `n`
    cellules a pour centre `2·i₀ + n`, impair si `n` est impair, et
    `round(1.5) == 2` l'arrondit à l'entier pair voisin. La fusion produisait
    déjà un centre exact. *Preuve dans la donnée* : tous les pavés
    `size = [2, 12, 2]` étaient à une position **paire**.
20. Le creusement dilatait de **1,0 m (demi-côté)** sur une trame de 2,0 m au lieu
    de la **demi-diagonale 1,414** → les coins de cellule mordaient 0,41 m dans
    les couloirs.

### C — Littéral qui dérive de l'intention

11. Route `entree_tunnel` déclarée **large de 14 m**, soit toute la section du
    tunnel : un couloir dont le trajet occupe toute la largeur n'est pas un
    couloir.
12. Le trajet serpentant **traversait une couverture** posée à la main.
21. **Passage de chicane à 4,0 m** pour une route déclarée à 5.
22. 4ᵉ mur de chicane **flottant en îlot à 1,0 m** de l'axe de `tunnel_cour` →
    3,5 m utiles sur 5.
23. Couverture **tangente à exactement 2,0 m** de la médiane de `ruines`, que la
    route suit.
24. Pilier de colonnade à **1,92 m** de l'axe de `entree_terrasse` — mordait de
    8 cm.
33. **Aucune ligne de vue au-delà de 30 m dans aucune des 7 salles** (max 29 m) :
    la bande longue portée demandée n'existe plus depuis le shrink, alors que
    l'arsenal contient un sniper 300 m sans chute de dégâts et un lance-roquettes
    60 m.

### D — Géométrie non dérivée

> **Classe fermée par construction le 2026-07-30**, en suivant les normes du
> marché : emprises de salles **disjointes** (deux sols à deux altitudes sur le
> même XZ est une contradiction), la **transition occupe son propre espace**, et
> un pas de plus de **45 cm** — `MaxStepHeight` d'Unreal, contre 0,1–0,4 m
> recommandé par Unity pour un gabarit de 2 m — exige une rampe, pas un saut.
> La chaîne est est **dérivée** (`_lay_out_east_chain`), le sol est **dérivé** de
> la forme creusée, et `ROOM_CARVE_MARGIN` est la **source unique** lue par le
> creusement ET par le sol. Coût : monde 120 → 152 m en X ; aucune taille de
> salle modifiée. Bords de chute **82 → 61 m²**. Reste ouvert : `27`.

7. **Dalle de plafond ne touchant pas les murs** : le creusement ouvre salle + 1 m,
   la dalle couvrait la salle seule → fente de 1 m sur ~200 m par salle couverte.
10. **350 m² de bords de chute.** Première tentative (élargir les plateformes) a
    **englouti les atterrissages de rampe** → annulée, creusement resserré à la
    place.
25. **Interpénétration de plateformes 144 m³** (`pont × chapelle`) et 48 m³
    (`cour × pont`) : les emprises de salles adjacentes se chevauchent
    volontairement, et chaque socle étant plein depuis `y=0`, le socle le plus bas
    était entièrement noyé dans le plus haut.
26. **Les deux rampes entre salles en hauteur sont enterrées** dans les socles :
    `rampe_pont` (x 22→26, 2→3 m) et `rampe_chapelle` (x 38→42, 3→4 m). Les vraies
    transitions sont des **marches de 1 m**.
27. **Le pont n'est pas un pont** : `plateforme_pont` est un massif plein, et les
    3 piles ajoutées pour corriger « pont sans appui » sont enterrées dedans.
28. **82 m² de bords sans garde-corps** (terrasse 16, cour 19, pont 22, chapelle
    25 cellules). Cause : plateforme = emprise **nominale** de la salle, creusement
    = emprise + 1 m → anneau de 1 m sans sol sur chaque salle en hauteur.
    **Élargir le creusement aggrave.**
30. **Marches au plafond** aux raccords salle/couloir : entrée 6 m contre 4 m,
    chapelle 7 m contre 4 m.
31. **Puits de roche morts** au-dessus des dalles : entrée 6 m, tunnel 8 m,
    chapelle 1 m (enceinte 12 m, plafonds 4-7 m). Géométrie invisible et payée.

### E — Source auto-référente ou périmée

6. **Boucle de rétroaction du générateur** : le script **lisait le TOML qu'il
   écrivait** → clé de palette dupliquée (TOML cassé), élargissement de plateforme
   cumulatif, puis carte réduite de 1742 à 1175 lignes avec des routes sortant de
   l'arène. **3 occurrences.**
29. Le correctif « 19 coudes à nu → 29 dalles d'angle » de la passe finitions a été
    **perdu à la régénération** du shrink : 3 coudes à nu, 0 dalle d'angle.
32. Le dossier passé à l'audit multi-agents décrivait la carte **d'avant le
    shrink** (220 × 144 m, 165 blocs, 64 lumières) : les 78 constats portent sur
    une géométrie 4× plus grande qui n'existe plus. Et ses **5 chiffres codés en
    dur étaient tous faux** (49 m² au lieu de 82, 2 chevauchements au lieu de 15,
    un correctif noté CORRIGÉ qui ne l'était plus).
34. La story a enregistré « `route_contract_errors` vert » comme une **preuve**,
    alors que ce vert venait du contrôle cassé du défaut 14.

### F — Contrôle qui passe à vide

14. Le contrôle de dégagement testait un **fil de 0,6 m** (rayon joueur) au milieu
    de couloirs de 5 m : il ne voyait rien et passait au vert sur n'importe quelle
    géométrie.
18. `tallest_traversal_step_m` **au vert par construction** : 0 bloc de rôle
    `traversal` dans la carte, donc il rapportait `0.0`, lu comme « vérifié ».
26. `route_contract_errors` vérifie qu'une rampe est **déclarée**, pas qu'elle est
    **dégagée** → il ne voit pas les deux rampes enterrées.
35. Voir classe A : un contrôle qui produit des faux positifs finit ignoré, donc
    aveugle.

### Hors classe

3. Double construction à l'entrée : un `Local<u32>` à 0 alors que `OnEnter` avait
   déjà bâti la révision 1.
5. Rampes à **48,8°** : course trop courte pour la dénivelée. Corrigé à 23-27°.
8. Dalle de la chapelle **hors de la roche** : sol +4 plus plafond 9 = 13 m contre
   12 m de roche.
9. **Dalles d'angle coplanaires** : 22 nouveaux chevauchements en z-fighting,
   corrigés par un décalage de 2 cm.

---

## Ce que le registre dit de la méthode

Deux constats qui comptent plus que les 35 lignes :

1. **Les classes C et D pèsent 15 défauts sur 35.** Ce sont exactement celles que
   produit une **table de boîtes écrite à la main** : un littéral ne peut pas
   porter un invariant, et une valeur déclarée deux fois finit par divorcer. Tant
   que la géométrie est écrite plutôt que dérivée, la boucle de correction ne se
   ferme jamais.
2. **La classe F est la plus dangereuse.** Un contrôle qui passe à vide ne coûte
   pas seulement son propre défaut : il **cache** tous ceux qu'il devait attraper,
   et il fait consigner de faux « verts » comme des preuves (34). Un contrôle doit
   dire *combien* il a mesuré, pas seulement s'il est content.

## Cross-refs

- [story-667](../stories/story-667-arena-test-blockout-bench.md) — l'historique complet des passes
- `tools/shrink_crypte_vertical.py` — le générateur (la carte est **générée**, ne jamais l'éditer à la main)
- `crates/forgia-game/src/arena_test.rs` — les contrôles et les 24 tests
- `.claude/rules/no-hardcode.md` · `.claude/rules/concept-first.md`

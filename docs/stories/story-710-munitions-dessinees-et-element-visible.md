# story-710 — Les munitions se voient, et elles disent leur élément

**Statut** : DRAFT (2026-08-13)
**Épic** : game feel / lisibilité · **Scale** : Standard
**Décision source** : session design 2026-08-13 (Antoine) · [GDD §4 réactions](../design/gdd-forgia-the-spared.md)

## Ce qu'on livre

Le bloc bas-droite (munitions + slots d'armes) passe du **chiffre** au **visuel** :

1. **Les balles se dessinent** et disparaissent une par une en tirant.
2. **Leur couleur dit l'élément** de l'arme.
3. **Les slots d'armes portent une icône**, pas un libellé.

## Pourquoi l'élément visible est le vrai gain

Les réactions élémentaires sont l'**USP n°1** du GDD (« je pose, tu détones ») et le constat qui
revient à chaque run est qu'elles sont **invisibles**. Aujourd'hui rien à l'écran ne dit qu'une arme
porte le choc et l'autre le feu.

Colorer les munitions par élément rend cette identité lisible **en permanence, sans ajouter un
widget** — sur un écran déjà occupé à 8 zones sur 9, c'est la surface la moins chère disponible.

⚠ Le GDD réserve explicitement les couleurs d'éléments : *« les couleurs d'éléments restent
**sacrées**, réservées à la lisibilité gameplay FPS »*. Cet usage est donc exactement dans la doctrine.

## Le seuil, parce que 30 pastilles est du bruit

| Arme | Chargeur | Rechargement | Traitement |
| --- | --- | --- | --- |
| **Boucherie** (lance-roquettes) | **3** | `shell_per_shell`, 1,33 s l'unité | balles dessinées — **le cas vitrine** |
| **Madame Lenoir** (sniper) | **5** | bolt, 2,5 s | balles dessinées |
| **Pépin** (pistolet) | **12** | mag, 1,2 s | balles dessinées |
| **Bourrasque** (SMG) | **30** | mag, 1,6 s | **barre segmentée** + chiffre |

**Boucherie est le cas vitrine** : elle recharge déjà roquette par roquette. Les trois logements qui
se remplissent un par un ne décorent pas — ils **racontent une mécanique qui existe**.

Le basculement dessin ↔ barre est un **gène** (`ammo_draw_max_rounds`), pas un `if` : le seuil se
règle manette en main.

## 🚨 Piège de nommage — vérifier avant de coder

Trois vocabulaires coexistent pour les mêmes quatre armes :

- `WeaponType::Shotgun` **est le sniper** (Madame Lenoir)
- la clé capteur `pompe` **est le lance-roquettes** (Boucherie)

Toujours croiser `viewmodel_arena.toml` et `roguelite_elements.toml` avant de conclure sur une arme.
Un mapping fait « au nom » se trompera de cible.

## Le socle existe

- `crates/forgia-ui-lib/src/hud_ammo/` — compteur bas-droite, **déjà data-driven**
  (`assets/genomes/hud_ammo_tuning.toml` + `tuning.rs`)
- `crates/forgia-mode-roguelite/src/hud.rs:1482` — slots d'armes bas-droite
- Pipeline pixel art par arme (`tools/art/light.py`, 4 vues, quad `sprite.rs`) — **les icônes se
  dérivent de la même source**, ce n'est pas une nouvelle production
- `ReloadState::progress` — l'animation de rechargement lit déjà l'état, pas une durée codée

## Critères d'acceptation (falsifiables)

- [ ] **Test** : `mag_size` donné + N tirés ⟹ le nombre de balles dessinées est déterministe
      (fonction pure, testable headless — même contrat que `run_progress_label`)
- [ ] Sous le seuil, chaque tir retire **une** balle visible ; le rechargement les remet — et sur
      Boucherie, **une par une**, au rythme de `ReloadState::progress`
- [ ] Au-dessus du seuil, barre segmentée + chiffre ; **aucune** pastille dessinée
- [ ] La couleur de balle vient de `roguelite_elements.toml`, jamais d'un littéral — un élément
      changé en génome change le HUD sans recompiler
- [ ] Les 4 slots portent l'icône de leur arme, et l'arme active se distingue **sans lire le texte**
- [ ] Aucun nouveau littéral : seuil, tailles, espacements, couleurs vivent dans
      `hud_ammo_tuning.toml`
- [ ] 0 warning clippy · tests verts

## Contrainte de lisibilité

À 1920×1080 en pleine fusillade, ce qui se lit est une **silhouette et une couleur**, jamais une
texture. Formes franches, pas de balles détaillées.

## Test runtime

1. **Action** : lancer une run, tirer 3 coups à Pépin, puis passer à Boucherie et vider les 3 roquettes
2. **Rechargement** : rebuild (`cargo run -p forgia`) — le HUD n'est pas hot-reloadable, seul le génome l'est
3. **Effet attendu** : 12 balles bas-droite qui passent à 9 · sur Boucherie, 3 logements qui se
   vident puis se remplissent **un par un** sur ~4 s
4. **Où observer** : à l'écran ; et `forgia2_hud_ammo.json` doit refléter le même compte
5. **Variantes si KO** : rien de dessiné → vérifier `ammo_draw_max_rounds` dans le génome ·
   couleurs fausses → croiser `roguelite_elements.toml` (piège de nommage ci-dessus) ·
   aucune animation de recharge → vérifier que `ReloadState::progress` est bien lu

## Dépendances

Aucune. Autonome, livrable immédiatement.

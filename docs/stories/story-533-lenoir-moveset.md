# Story-533 — 🎩 Madame Lenoir Moveset Distinctive (Mission 3 GDD)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_barks.json`, fichier `lenoir.rs`, symbole `LenoirStats`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : EN COURS — incrément 1 livré 2026-06-11 (AC7+AC10, AC6-kill via moteur barks)
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Lenoir](../design/gdd-roguelite-v1.md#-madame-lenoir--larme-précision)
> **Prérequis** : story-528 (FPS feel)

## Incrément 1 (2026-06-11) — Lenoir juge + sensor précision

- **AC7 ✅ code** : barks de RATÉ — moteur barks généralisé (`try_play` +
  event `miss` lisant `ShotResolved.hit_enemy=false`), gène
  `bark_chance_on_miss` (0.35 hot-reload) + pool `lenoir/miss` (*« Lamentable. »*,
  *« On se ressaisit, voulez-vous ? »*). **Seule Lenoir a un pool miss** → elle
  seule juge ; les autres armes restent silencieuses sur un raté (NoPool, ne
  consomme pas le lock anti-overlap).
- **AC10 ✅ code** : `forgia-fps/lenoir.rs` — `LenoirStats` (shots/hits/
  **headshots** via `CombatHitEvent.is_headshot`/kills, observe-only multicast),
  reset OnEnter(Roguelite), sensor `forgia2_lenoir.json` 1Hz avec `hs_ratio`
  (cible GDD : >40 % gamers) + registry. 41 tests verts (2 crates), clippy 0.
- **AC1 — divergence délibérée** : stats actuelles CONSERVÉES (50 body / ×2
  head = one-shot tête, mag 5, reload 2.5 s) au lieu du GDD littéral (80 HS /
  40 body, mag 4, reload 3 s) — le one-shot tête est la récompense « ouverture
  parfaite » et le tuning actuel est validé en jeu. Leçon Bourrasque v1 (GDD
  littéral ≠ feel). Re-discuter seulement si l'équilibrage en pâtit.
- **Restent** : AC2 scope 4×/monocle (scope existe, monocle+sway = anims), AC3
  « Coup d'œil » through-walls (⚠ Shift=Sprint + F déjà pris par Tir Perçant
  story-572 — keybind à trancher), AC4 anims, AC5 barks de tir, AC6 raffinement
  HS-only 5 %, AC8 bark reload, AC9 couleurs.

### Test in-game (incrément 1)

1. **Action** : run Roguelite → Digit3 (Lenoir) → rater exprès quelques tirs
   (mur/vide), puis enchaîner des tirs précis dont des têtes.
2. **Effet attendu** : sur ~1 raté sur 3, bulle sombre bas-droite au liseré
   Lenoir : *« Lamentable. »* ou *« On se ressaisit, voulez-vous ? »* —
   uniquement avec Lenoir en main (rate avec Pépin : silence).
3. **Sensor** : `forgia2_lenoir.json` → hs_ratio/accuracy ;
   `forgia2_barks.json` → `misses_seen` monte, `chance_on_miss: 0.35`.
4. **Variantes si KO** : jamais de jugement → `bark_chance_on_miss` 0.35→1.0
   (hot-reload) pour valider ; trop bavarde → 0.35→0.15 ou monter les
   `cooldown_sec` du pool.

## Pourquoi

Lenoir = arme précision/patience. Pattern de jeu distinctif : long-range, attend l'ouverture parfaite, juge le joueur quand il rate. Pour joueurs patients qui aiment prendre leur temps.

## Acceptance Criteria

- [ ] AC1 — LMB hitscan instant, 80 HS / 40 body, mag 4, reload long 3s, tracer fin blanc 200ms
- [ ] AC2 — RMB scope 4×, no breath sway si static >0.5s, crosshair = monocle élégant
- [ ] AC3 — Spé (Shift) "Coup d'œil" silhouettes outline tous ennemis 5s à travers murs (through-walls shader). Cooldown 15s. Voiceline *"Une dame voit tout."*
- [ ] AC4 — Animation FPS : Lenoir parfaitement immobile idle, recoil 0° (impeccable), reload manipule cartouche dorée avec deux doigts comme un mouchoir
- [ ] AC5 — Voicelines tir : *"Tsk."*, *"Acceptable."*, *"Élégant."*
- [ ] AC6 — Voicelines HS (rare = mémorable) : *"Précis."*, *"Mes félicitations."* (5% chance, ne se déclenche pas si <2 HS d'affilée)
- [ ] AC7 — Voicelines miss : *"Lamentable."*, *"On se ressaisit, voulez-vous ?"*
- [ ] AC8 — Voicelines reload : *"Patientez..."*
- [ ] AC9 — Couleur dominante noir + blanc (smoking style), accent doré cartouche
- [ ] AC10 — Sensor `forgia2_lenoir.json` : HS ratio (cible >40% gamers, <20% débutants), scope time avg, coup d'œil uses

## Files
- `crates/forgia-weapon-hitscan/src/lenoir.rs` NEW
- `crates/forgia-viewmodel/src/lenoir_anim.rs` NEW
- `crates/forgia-effects/src/coup_doeil_shader.rs` NEW (outline through-walls)
- `assets/genomes/roguelite/weapons/lenoir.toml`

## Anti-canon
- "Adversaire" pas "target"
- "Élégance" terme récurrent voicelines

## Cross-refs
- GDD V1 Mission 3 Madame Lenoir
- Bible v1 persona Lenoir
- story-530 boons precision Lenoir

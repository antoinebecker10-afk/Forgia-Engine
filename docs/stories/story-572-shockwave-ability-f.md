# Story-572 — Sort F « Onde de choc » (AOE)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_roguelite_state.json`, fichier `shockwave.rs`, symbole `DamageEvent`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-06-02)
> **Scale** : Standard (~4 fichiers : shockwave.rs NEW + lib + hud + sensor)
> **Décision** : user 2026-06-02 — sort F = Onde de choc AOE (vs buff/heal/dash)
> **Bible** : v1 cartoon family-friendly

---

## 1. Vision

Première **compétence active** du Roguelite (modèle Gunfire « skill »). Touche **F** :
onde de choc en zone autour du joueur → dégâts à tous les ennemis proches + punch
caméra + VFX anneau au sol. Bouton panique satisfaisant quand on est encerclé.

**V1 = sort universel** (même pour toutes les armes). Le **per-arme** (lié aux armes
parlantes Pépin/Bourrasque/…) = évolution future.

## 2. Acceptance Criteria

- **AC1** : touche **F** déclenche l'onde si cooldown prêt (gate InGame + Roguelite + InRun/Boss)
- **AC2** : dégâts en zone via `DamageEvent` (kind Explosion) → passe par `apply_damage`
  (donc kills comptent : souls, killfeed, death) à tous `ArenaBot` dans `SHOCKWAVE_RADIUS` (XZ)
- **AC3** : **cooldown** `SHOCKWAVE_COOLDOWN` (8s V1) ; re-cast bloqué tant que > 0
- **AC4** : **VFX** anneau/disque au sol qui s'étend (0.5→rayon) + fade + despawn (~0.4s)
- **AC5** : **punch caméra** `CameraTrauma.add` au cast (juice Vlambeer)
- **AC6** : **HUD** indicateur bas-centre : « F » + radial cooldown (prêt = doré, en CD = arc + secs)
- **AC7** : **Observability** : `forgia2_roguelite_state.json` → `shockwave_casts`, `shockwave_cd`

## 3. Hot path
- [ ] Input = `just_pressed` (event-like, pas chaque frame de travail)
- [ ] AOE itère `ArenaBot` une fois au cast seulement (pas par frame)
- [ ] VFX = 1 entité/cast, cooldown ~8s → fréquence basse

## 4. Constantes V1 (→ genome story-566 balance)
`SHOCKWAVE_COOLDOWN=8.0`, `SHOCKWAVE_RADIUS=6.0`, `SHOCKWAVE_DAMAGE=45.0`.

## 5. Coupes assumées
- ❌ Knockback/repoussement (fight le KCC/AI — V2)
- ❌ Per-arme (sort propre à chaque arme parlante — V2)
- ❌ Externalisation genome des constantes (→ story-566)
- ❌ Respect fin InputBlockers (cast pendant Coffre/break possible — mineur)

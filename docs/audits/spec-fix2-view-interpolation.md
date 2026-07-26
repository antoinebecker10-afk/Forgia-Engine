# Spec — Interpolation de vue (Fix #2 de l'audit 2026-07-24)

> **Statut** : spec prête à coder (aucun code écrit — arbre contesté par un autre terminal au 2026-07-24).
> **Objectif** : supprimer le judder de translation du joueur/caméra sans toucher au tick physique déterministe.
> **Effort** : ~1 crate (`forgia-player`), 1 composant + 3 systèmes, ~80 LOC. **Risque** : Moyen (ordering physique).

---

## 1. Problème & root cause (preuves)

- Physique + mouvement joueur en **`FixedUpdate` 64 Hz** : [forgia-game/src/lib.rs:70-83](../../crates/forgia-game/src/lib.rs#L70-L83) (`Time::<Fixed>::from_hz(64)`, `RapierPhysicsPlugin::default().in_fixed_schedule()`, `TimestepMode::Fixed`).
- Le joueur écrit sa translation via KCC en FixedUpdate : [forgia-player/src/lib.rs:697](../../crates/forgia-player/src/lib.rs#L697) (`kcc.translation = Some(move_vec)`), appliquée par `PhysicsSet::SyncBackend`.
- **La `FpsCamera` est un enfant du joueur** ([forgia-player/src/lib.rs:408-427](../../crates/forgia-player/src/lib.rs#L408-L427), offset local `(0, 0.7, 0)`) → sa position monde = translation joueur (64 Hz) composée avec l'offset.
- La rotation (look) est écrite **chaque frame** en `Update` : yaw sur `player_tf.rotation`, pitch sur `cam_tf.rotation` ([forgia-player/src/lib.rs:522-527](../../crates/forgia-player/src/lib.rs#L522-L527)).
- **Aucune interpolation** dans tout le workspace (grep `interpolat` → 0 hit pertinent).

**Effet** : la *translation* rendue ne change que 64×/s alors que le rendu tourne plus vite (ou à 60 Hz → battement 64-vs-60). La *rotation* est fluide. Résultat perçu : **le monde glisse par pas pendant qu'on tourne fluide** — le défaut de feel #1.

## 2. Invariant à protéger (no-speculative-fix + déterminisme)

- **NE PAS changer le tick 64 Hz** (Keystone story-634, déterminisme sim). Le problème est la *présentation*, pas le pas de simulation.
- **NE PAS laisser la physique lire une position interpolée.** `player_movement`/KCC lisent `Transform.translation` en FixedUpdate ; s'ils lisent la valeur visuelle lerpée → dérive physique. La valeur *autoritaire* doit être restaurée avant chaque step.
- **NE PAS interpoler la rotation** (déjà per-frame, fluide). On interpole **uniquement la translation**.

## 3. Design

### Composant (sur le Player)
```rust
/// Interpolation render-only de la translation entre deux ticks fixes.
/// `curr` = position autoritaire (fin du dernier step physique).
/// `prev` = position autoritaire du step précédent.
/// Le rendu affiche lerp(prev, curr, overstep). La physique lit toujours `curr`.
#[derive(Component)]
pub struct RenderInterp {
    prev: Vec3,
    curr: Vec3,
    /// true = téléport/spawn : on colle prev=curr pour ne pas lerp à travers le saut.
    snap: bool,
}
```
Ajouté au spawn du Player (`prev = curr = spawn_pos`, `snap = true`).

### 3 systèmes + schedules

| # | Système | Schedule / set | Rôle |
|---|---------|----------------|------|
| A | `interp_restore_authoritative` | `FixedFirst` (**avant** `GameSet::Movement`) | `tf.translation = interp.curr` — la physique repart de la position autoritaire, jamais de la visuelle |
| B | `interp_capture` | `FixedLast` (**après** `PhysicsSet::Writeback`) | `if snap { prev = curr = tf } else { prev = curr; curr = tf }` ; `snap = false` |
| C | `interp_apply_render` | `PostUpdate` **avant** `TransformSystem::TransformPropagate` | `tf.translation = prev.lerp(curr, Time::<Fixed>::overstep_fraction())` |

**Pourquoi ça marche** : chaque frame de rendu, C écrit une position visuelle interpolée sur `Transform.translation` ; la `FpsCamera` enfant l'hérite → caméra fluide. Au step fixe suivant, A **restaure** la valeur autoritaire avant que le mouvement ne tourne → la physique est **identique** à aujourd'hui (déterminisme intact). B recapture après physique.

### Pseudocode
```rust
// A — FixedFirst, avant GameSet::Movement
fn interp_restore_authoritative(mut q: Query<(&mut Transform, &RenderInterp)>) {
    for (mut tf, i) in &mut q { tf.translation = i.curr; }
}
// B — FixedLast, après PhysicsSet::Writeback
fn interp_capture(mut q: Query<(&Transform, &mut RenderInterp)>) {
    for (tf, mut i) in &mut q {
        if i.snap { i.prev = tf.translation; i.curr = tf.translation; i.snap = false; }
        else { i.prev = i.curr; i.curr = tf.translation; }
    }
}
// C — PostUpdate, avant TransformSystem::TransformPropagate
fn interp_apply_render(fixed: Res<Time<Fixed>>, mut q: Query<(&mut Transform, &RenderInterp)>) {
    let a = fixed.overstep_fraction();               // [0,1], fourni par Bevy — pas de magic number
    for (mut tf, i) in &mut q { tf.translation = i.prev.lerp(i.curr, a); }
}
```

### Câblage
```rust
app.add_systems(FixedFirst, interp_restore_authoritative
        .before(GameSet::Movement).run_if(in_state(AppMode::InGame)));
app.add_systems(FixedLast, interp_capture
        .after(PhysicsSet::Writeback).run_if(in_state(AppMode::InGame)));
app.add_systems(PostUpdate, interp_apply_render
        .before(TransformSystem::TransformPropagate).run_if(in_state(AppMode::InGame)));
```
> `PhysicsSet::Writeback` est le set Rapier qui écrit les transforms après solve ; vérifier le nom exact en 0.33 (sinon `PhysicsSet::StepSimulation` puis writeback). `FixedFirst`/`FixedLast` existent en Bevy 0.18.

## 4. Snap sur téléport (sinon slide visible à travers le saut)

Tout hard-set de translation doit poser `snap = true` (ou faire `prev = curr = new_pos`). Sites connus :
- Spawn / respawn ([forgia-player/src/lib.rs:365-430](../../crates/forgia-player/src/lib.rs#L365-L430)).
- `player_floor_safety_net` (téléport Y<-1 → Y=2, [forgia-player/src/lib.rs:730](../../crates/forgia-player/src/lib.rs#L730)).
- Récupération de chute du Hall (`recover_fallen...`, castle_hub, `GREAT_HALL_FALL_RECOVERY_Y`).
- Toute future téléportation (portail, boss arena).

Pattern : après avoir écrit `tf.translation = target`, faire `if let Ok(mut i) = q_interp.get_mut(e) { i.snap = true; }`. À défaut d'accès, un `RequestInterpSnap` event lu par B.

## 5. Cas limites

- **Plusieurs steps fixes/frame** (frame lente) : A→physique→B tournent à chaque step → `prev`/`curr` avancent correctement ; C lerp sur le dernier couple. OK.
- **0 step fixe dans une frame** (fps > 64) : `overstep_fraction` croît vers 1, C lerp vers `curr`. Fluide. OK.
- **Pause** (`AppMode::Paused`) : les 3 systèmes gated `InGame` → figés, `curr` stable, aucune dérive. OK.
- **Ennemis / projectiles** : même judder (relevé par l'audit feel). **Follow-up** : composant `RenderInterp` générique appliqué aux corps physiques visibles répliqués (hors scope de ce fix, qui cible l'œil du joueur).
- **`track_player_speed`** ([forgia-player/src/lib.rs:704](../../crates/forgia-player/src/lib.rs#L704)) mesure le delta en FixedUpdate → lit la position **autoritaire** (restaurée par A) → signal bob du viewmodel inchangé. OK (vérifier que `track_player_speed` tourne après A, i.e. en `GameSet::Movement` ou après).

## 6. Alternative évaluée

- **`bevy_rapier3d` 0.33 built-in** : l'interpolation Rapier vise historiquement les corps *dynamiques* ; pour `KinematicPositionBased` + KCC il n'y a pas de chemin propre → l'approche manuelle ci-dessus est la voie robuste ET compatible déterminisme. *(À confirmer d'un coup d'œil sur la doc 0.33 avant de coder ; si un `TransformInterpolation` kinematic existe, le préférer.)*
- **Rig caméra découplé** (caméra plus enfant du player, driven par pos interpolée) : plus invasif (re-parenting + le yaw est sur le transform player hérité). Rejeté pour ce fix — l'approche restore garde la hiérarchie actuelle.

## 7. Observabilité (règle observability-required)

Étendre le capteur feel existant (`forgia2_fps_feel.json` / `fps_feel_sensor.rs`) avec :
```json
{ "view_interp_active": true, "overstep_fraction": 0.42, "interp_snaps_last_30s": 0 }
```
Health-check : `interp_active == false && in_game` (le fix ne tourne pas alors qu'il le devrait) → `warn`.

## 8. Récap de test runtime (règle in-game-test-recap)

1. **Action** : lancer une run, avancer/strafe en ligne droite le long d'un mur texturé, ~5 s, à fps non capé (ou 144 Hz).
2. **Rechargement** : rebuild (`cargo run -p forgia`) — pas hot-reloadable (code).
3. **Effet attendu** : le décor **ne glisse plus par pas** en translation ; le mouvement latéral est lisse comme la rotation l'est déjà.
4. **Où observer** : `forgia2_fps_feel.json::view_interp_active=true` + `overstep_fraction` ∈ ]0,1[ qui varie ; visuellement, plus de micro-saccade de translation.
5. **Variantes si KO** :
   - Judder inversé / doublé → l'ordre de A vs `GameSet::Movement` est faux (A doit être **avant**).
   - Le perso « traîne » / mou → C lerp `curr→prev` (arguments inversés) ou overstep mal lu.
   - Slide au respawn/chute → `snap` non posé sur le site de téléport (§4).
   - Aucun effet → systèmes non gated `InGame` actifs ? `TransformPropagate` tourne-t-il après C ? artefact rebuild à jour ?

## 9. Ce qui NE doit PAS bouger

Aim brut 1:1 (mouse_look Update), juice `dt`-based, tick 64 Hz, KCC tuning (offset 0.05, autostep 0.3, snap_to_ground 0.5). Ce fix n'ajoute **aucune** constante gameplay (overstep = moteur) → conforme no-hardcode.

---

*Spec dérivée de l'audit `docs/audits/audit-2026-07-24.md` §7 (Movement & Feel). À implémenter dans `forgia-player` quand l'arbre sera décontesté. Story candidate : Standard (1 crate, ~80 LOC, risque ordering).*

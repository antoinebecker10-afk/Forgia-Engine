//! navmesh_build.rs — `ArenaGeometry` → maillage de navigation (story-700 inc.2).
//!
//! Le sens de la dépendance est délibéré : c'est **`forgia-stage` qui appelle
//! `forgia-navmesh`**, jamais l'inverse. La crate de navigation ne connaît ni les arènes
//! ni leur cycle de vie — c'est ce qui lui permettra de servir aussi le terrain
//! d'expédition (Phase 2 de la refonte) sans être réécrite.

use crate::ArenaGeometry;
use bevy::prelude::*;
use forgia_navmesh::{build, hexagon_edge, ActiveNavMesh, NavmeshBuild};
use web_time::Instant;

/// Mémoire du système entre deux frames.
#[derive(Default)]
pub struct RebuildState {
    /// La géométrie a bougé et n'a pas encore été rebâtie.
    dirty: bool,
}

/// Rebâtit le maillage quand la géométrie de l'arène a **fini** de bouger.
///
/// # Les deux gardes, et pourquoi aucune n'est optionnelle
///
/// 1. **Debounce d'une frame.** Les solides sont déposés par une demi-douzaine de
///    producteurs (murs de pièces, modules, pièces autorées, décor du mode roguelite).
///    Rebâtir à chaque dépôt trianguleraient N fois la même arène pour un seul résultat
///    utile. On attend donc la première frame *stable* après la dernière modification.
///
/// 2. **`authored_pending == 0`.** Les pièces autorées arrivent avec leur GLB, en
///    asynchrone. Un maillage bâti pendant leur chargement ignorerait leurs emprises et
///    laisserait des agents traverser des murs bien réels — le défaut serait invisible,
///    puisque la construction « réussit ». `ArenaGeometry` documente exactement ce piège
///    pour son propre capteur ; il vaut ici à l'identique.
pub fn sys_rebuild_navmesh_from_arena(
    geo: Res<ArenaGeometry>,
    cfg: Res<NavmeshBuild>,
    mut active: ResMut<ActiveNavMesh>,
    mut state: Local<RebuildState>,
) {
    if geo.is_changed() {
        state.dirty = true;
        return;
    }
    if !state.dirty {
        return;
    }
    if geo.authored_pending > 0 {
        // Échantillon incomplet : on reste `dirty` et on retentera.
        return;
    }
    state.dirty = false;

    // Une arène dont le rayon jouable ne dépasse pas l'emprise de l'agent n'a pas de
    // surface navigable. C'est le cas au `reset()`, entre deux stages : on oublie le
    // maillage précédent plutôt que d'en bâtir un vide, sinon les agents continueraient
    // de router sur une géométrie démontée.
    if geo.playable_radius_m <= cfg.agent_radius_m {
        if active.is_built() {
            info!("[navmesh] arene demontee — maillage oublie");
            active.clear();
        }
        return;
    }

    let t0 = Instant::now();
    let edge = hexagon_edge(geo.playable_radius_m, cfg.agent_radius_m);
    let (mesh, report) = build(edge, &geo.discs, &geo.segs, &cfg);
    let build_ms = t0.elapsed().as_secs_f32() * 1000.0;

    info!(
        "[navmesh] {} (seed {}) — {} disques + {} troncons soumis, {} obstacles retenus, {:.1} ms",
        geo.stage_id, geo.seed, report.discs_seen, report.segs_seen, report.obstacles_kept, build_ms
    );
    active.set(mesh, report, &geo.stage_id, geo.seed, build_ms);
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_core::layout::SolidDisc;

    /// Monte le minimum : les deux ressources, le système, rien d'autre.
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ArenaGeometry>()
            .insert_resource(NavmeshBuild::default())
            .init_resource::<ActiveNavMesh>()
            .add_systems(Update, sys_rebuild_navmesh_from_arena);
        app
    }

    fn peuple(app: &mut App, radius: f32, pending: u32) {
        let mut geo = app.world_mut().resource_mut::<ArenaGeometry>();
        geo.reset("test_stage", 7, radius);
        geo.discs.push(SolidDisc {
            x: 0.0,
            z: 0.0,
            r: 3.0,
            h: 4.0,
        });
        geo.authored_pending = pending;
    }

    #[test]
    fn rien_n_est_bati_tant_que_la_geometrie_bouge() {
        let mut app = app();
        peuple(&mut app, 20.0, 0);
        // Une seule frame : la géométrie vient de changer, donc on attend.
        app.update();
        assert!(
            !app.world().resource::<ActiveNavMesh>().is_built(),
            "la premiere frame doit seulement marquer dirty"
        );
    }

    #[test]
    fn le_maillage_est_bati_a_la_premiere_frame_stable() {
        let mut app = app();
        peuple(&mut app, 20.0, 0);
        app.update(); // change → dirty
        app.update(); // stable → bâtit
        let active = app.world().resource::<ActiveNavMesh>();
        assert!(active.is_built());
        assert_eq!(active.source, "test_stage");
        assert_eq!(active.seed, 7);
        assert_eq!(active.report.obstacles_kept, 1);
    }

    #[test]
    fn les_pieces_autorees_en_vol_bloquent_la_construction() {
        // LE piège que cette garde existe pour éviter : batir pendant le chargement
        // produit un maillage qui ignore des murs bien reels, sans lever d'erreur.
        let mut app = app();
        peuple(&mut app, 20.0, 2); // 2 GLB pas encore arrivés
        app.update();
        app.update();
        assert!(
            !app.world().resource::<ActiveNavMesh>().is_built(),
            "authored_pending > 0 : l'echantillon est incomplet, on ne batit pas"
        );

        // Les pièces arrivent → la construction se débloque toute seule.
        app.world_mut().resource_mut::<ArenaGeometry>().authored_pending = 0;
        app.update(); // change → dirty
        app.update(); // stable → bâtit
        assert!(app.world().resource::<ActiveNavMesh>().is_built());
    }

    #[test]
    fn demonter_l_arene_oublie_le_maillage() {
        let mut app = app();
        peuple(&mut app, 20.0, 0);
        app.update();
        app.update();
        assert!(app.world().resource::<ActiveNavMesh>().is_built());

        // `reset` à rayon nul = plus d'arène. Router sur une géométrie démontée
        // enverrait les agents à travers des murs qui n'existent plus.
        app.world_mut()
            .resource_mut::<ArenaGeometry>()
            .reset("", 0, 0.0);
        app.update();
        app.update();
        assert!(!app.world().resource::<ActiveNavMesh>().is_built());
    }
}

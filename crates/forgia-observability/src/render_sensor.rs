//! render_sensor.rs — Producteur `forgia2_render.json` (1Hz, cross-mode).
//!
//! Comble le trou d'observabilité du rendu (audit 2026-06-17) : aucun capteur ne
//! disait « le monde 3D rend-il ? ». Diagnostic de l'écran brun / monde invisible
//! sans triangulation log+capture.
//!
//! Signal décisif : `mesh3d_total` vs `mesh3d_visible`.
//! - total > 0 mais visible == 0 → meshes présents NON rendus (culling B0004 /
//!   passe opaque cassée / pas de caméra active) = signature écran brun.
//!
//! Énumère TOUTES les caméras (`With<Camera>`, pas `With<Camera3d>` — robuste si
//! une 3P ne porte pas le marker comme attendu), et marque `is_3d` par caméra :
//! is_active, order, Tonemapping, Hdr, Bloom, Msaa, Skybox, DistanceFog, fps_camera.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_pipeline::Skybox;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::Hdr;

use forgia_player::FpsCamera;

/// Pur — extrait pour tests headless. Sévérité orientée diagnostic « écran brun ».
pub fn severity_for_render(
    active_3d_cams: u64,
    mesh_total: u64,
    mesh_visible: u64,
    hdr_or_bloom_on_active: bool,
) -> (&'static str, &'static str) {
    if active_3d_cams == 0 {
        (
            "critical",
            "AUCUNE Camera3d active — rien ne rend en 3D (skybox/clear seuls). Vérifier is_active (résidu mode 3P ?).",
        )
    } else if mesh_total > 50 && mesh_visible == 0 {
        (
            "critical",
            "Meshes présents mais 0 visible — passe opaque cassée OU culling (B0004 InheritedVisibility) OU frustum. SIGNATURE ECRAN BRUN.",
        )
    } else if hdr_or_bloom_on_active {
        (
            "warn",
            "Hdr/Bloom sur une Camera3d active — risque passe principale cassée (écran nu). À configurer à la création, jamais post-hoc.",
        )
    } else if mesh_total > 100 && mesh_visible.saturating_mul(2) < mesh_total {
        (
            "warn",
            "Plus de la moitié des Mesh3d non visibles — culling agressif ou position/frustum caméra suspect.",
        )
    } else {
        ("ok", "")
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn sys_write_render_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    clear: Res<ClearColor>,
    q_cams: Query<(
        Entity,
        Option<&Name>,
        &Camera,
        Has<Camera3d>,
        Has<FpsCamera>,
        Option<&Tonemapping>,
        Has<Hdr>,
        Has<Bloom>,
        Option<&Msaa>,
        Has<Skybox>,
        Has<DistanceFog>,
    )>,
    q_meshes: Query<&ViewVisibility, With<Mesh3d>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let mut mesh_total = 0u64;
    let mut mesh_visible = 0u64;
    for vv in q_meshes.iter() {
        mesh_total += 1;
        if vv.get() {
            mesh_visible += 1;
        }
    }

    let mut cams_json = String::new();
    let mut cams_total = 0u64;
    let mut n3d = 0u64;
    let mut n3d_active = 0u64;
    let mut hdr_or_bloom_on_active = false;
    for (e, name, cam, is_3d, is_fps, tm, hdr, bloom, msaa, sky, fog) in q_cams.iter() {
        cams_total += 1;
        if is_3d {
            n3d += 1;
            if cam.is_active {
                n3d_active += 1;
                if hdr || bloom {
                    hdr_or_bloom_on_active = true;
                }
            }
        }
        let nm = name
            .map(|n| n.as_str().replace('"', "'"))
            .unwrap_or_else(|| format!("Entity{e:?}"));
        let tm_s = tm
            .map(|t| format!("{t:?}"))
            .unwrap_or_else(|| "none".to_string());
        let msaa_s = msaa
            .map(|m| format!("{m:?}"))
            .unwrap_or_else(|| "none".to_string());
        if !cams_json.is_empty() {
            cams_json.push(',');
        }
        cams_json.push_str(&format!(
            r#"{{"name":"{nm}","is_3d":{is_3d},"fps_camera":{is_fps},"active":{active},"order":{order},"tonemapping":"{tm_s}","hdr":{hdr},"bloom":{bloom},"msaa":"{msaa_s}","skybox":{sky},"distance_fog":{fog}}}"#,
            active = cam.is_active,
            order = cam.order,
        ));
    }

    let cc = clear.0.to_srgba();
    let (severity, next_step) =
        severity_for_render(n3d_active, mesh_total, mesh_visible, hdr_or_bloom_on_active);

    let json = format!(
        r#"{{"id":"render","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"clear_color":[{:.3},{:.3},{:.3}],"mesh3d_total":{mesh_total},"mesh3d_visible":{mesh_visible},"cameras_total":{cams_total},"cameras_3d":{n3d},"cameras_3d_active":{n3d_active},"cameras":[{cams_json}]}}"#,
        time.elapsed_secs(),
        cc.red,
        cc.green,
        cc.blue,
    );

    if let Err(e) = std::fs::write("forgia2_render.json", &json) {
        warn!("[forgia-observability] render sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_active_camera_is_critical() {
        let (sev, next) = severity_for_render(0, 2000, 0, false);
        assert_eq!(sev, "critical");
        assert!(next.contains("AUCUNE"));
    }

    #[test]
    fn meshes_present_none_visible_is_brown_screen() {
        let (sev, next) = severity_for_render(1, 2059, 0, false);
        assert_eq!(sev, "critical");
        assert!(next.contains("ECRAN BRUN"));
    }

    #[test]
    fn hdr_bloom_on_active_warns() {
        let (sev, next) = severity_for_render(1, 2059, 2059, true);
        assert_eq!(sev, "warn");
        assert!(next.contains("Hdr/Bloom"));
    }

    #[test]
    fn healthy_render_is_ok() {
        let (sev, next) = severity_for_render(1, 2059, 2059, false);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn heavy_culling_warns() {
        let (sev, _) = severity_for_render(1, 2000, 500, false);
        assert_eq!(sev, "warn");
    }
}

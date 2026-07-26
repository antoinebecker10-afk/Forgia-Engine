//! perf_sensor.rs — Producteur `forgia2_perf.json` (1Hz, cross-mode).
//!
//! Lit `FrameTimeDiagnosticsPlugin` (Bevy 0.18) :
//! - `avg_ms` : moyenne ring-buffer (~120 samples)
//! - `min_ms` / `max_ms` : fold sur `values()` (la struct `Diagnostic` n'expose
//!   pas de min/max natifs — voir bevy-specialist research 2026-05-19)
//! - `fps_smoothed` : EMA depuis le diagnostic dédié FPS
//! - `gpu_time_sum_ms` / `bound_hint` (audit 2026-07-01) : somme des passes GPU
//!   (`RenderDiagnosticsPlugin`, Vulkan/DX12 uniquement) → verdict CPU-bound vs
//!   GPU-bound. Bevy 0.18 n'expose PAS de path GPU "total" : on itère les paths
//!   `render/*` et on somme ceux finissant en `elapsed_gpu`/`gpu_time` (suffixe
//!   incertain → couvre les deux). `gpu_paths_sample` dumpe les top passes réelles
//!   au 1er run pour lever le doute sur le naming (cf bevy-specialist 2026-07-01).
//!   ⚠️ décalage 1 frame : le GPU de la frame N remonte au main-world en N+1.
//!
//! Severity heuristic :
//! - `warn`     : avg > 25 ms (~40 FPS)
//! - `critical` : avg > 50 ms (~20 FPS)
//!
//! Story-467 — Vague 5 Phase 5b Session B.

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;

/// Pur — extrait pour tests headless sans App Bevy.
pub fn severity_for_perf(avg_ms: f64) -> (&'static str, &'static str) {
    if avg_ms > 50.0 {
        (
            "critical",
            "frame_time avg > 50ms — investigate hot systems (Tracy, forgia2_entities)",
        )
    } else if avg_ms > 25.0 {
        ("warn", "frame_time avg > 25ms — perf budget approaching")
    } else {
        ("ok", "")
    }
}

/// Verdict heuristique CPU-bound vs GPU-bound. Pur → testable.
/// - `gpu_passes == 0` : aucun timing GPU dispo (backend non Vulkan/DX12, plugin
///   absent, ou suffixe de path non matché) → on ne tranche PAS.
/// - ratio `gpu/frame` élevé : le GPU est le long pole → GPU-bound.
/// - ratio bas : le CPU (ou le vsync) domine → CPU-bound.
pub fn bound_hint(frame_ms: f64, gpu_ms: f64, gpu_passes: usize) -> &'static str {
    // Sous ~14 ms (≈71 fps) : largement sous le budget 16.6 ms → AUCUN goulot à
    // signaler, peu importe le ratio GPU. Évite un faux "cpu_bound" affiché à 241 fps
    // (le ratio GPU est bas simplement parce que la frame est triviale, pas bloquée).
    if frame_ms < 14.0 {
        return "headroom";
    }
    if gpu_passes == 0 {
        return "gpu_timing_unavailable";
    }
    let ratio = if frame_ms > 0.001 {
        gpu_ms / frame_ms
    } else {
        0.0
    };
    if ratio >= 0.80 {
        "gpu_bound"
    } else if ratio <= 0.50 {
        "cpu_bound"
    } else {
        "balanced_or_vsync"
    }
}

/// Somme le temps GPU par frame + dumpe les top passes `render/*` (nom + ms).
/// Retour : (somme_gpu_ms, passes_gpu_matchées, paths_render_total, json_top_passes).
/// Le dump top-12 révèle le naming réel des paths (suffixe incertain en 0.18) et
/// où part le temps GPU — à lire au 1er run avant tout hardcode de nom de passe.
fn render_frame_stats(diagnostics: &DiagnosticsStore) -> (f64, f64, usize, usize, String) {
    let mut gpu_sum_ms = 0.0_f64;
    let mut cpu_sum_ms = 0.0_f64;
    let mut gpu_passes = 0usize;
    let mut render_total = 0usize;
    let mut entries: Vec<(String, f64)> = Vec::new();
    for diag in diagnostics.iter() {
        let path = diag.path().as_str();
        if !path.starts_with("render/") {
            continue;
        }
        render_total += 1;
        let ms = diag.smoothed().unwrap_or(0.0);
        let is_gpu = path.ends_with("elapsed_gpu") || path.ends_with("gpu_time");
        let is_cpu = path.ends_with("elapsed_cpu");
        if is_gpu {
            gpu_sum_ms += ms;
            gpu_passes += 1;
        }
        if is_cpu {
            cpu_sum_ms += ms;
        }
        // Sample : UNIQUEMENT les paths de TEMPS (elapsed_gpu/cpu). On EXCLUT les
        // compteurs (`*_invocations`, `*_primitives_out`, `*clipper*`) dont la valeur
        // est un nombre d'appels, pas des ms (sinon ils polluent le top par magnitude).
        if is_gpu || is_cpu {
            let esc = path.replace('\\', "\\\\").replace('"', "\\\"");
            entries.push((esc, ms));
        }
    }
    // Top 12 par ms décroissant → passes lourdes + naming réel des paths.
    entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    entries.truncate(12);
    let arr = entries
        .iter()
        .map(|(p, ms)| format!(r#"{{"p":"{p}","ms":{ms:.3}}}"#))
        .collect::<Vec<_>>()
        .join(",");
    (
        gpu_sum_ms,
        cpu_sum_ms,
        gpu_passes,
        render_total,
        format!("[{arr}]"),
    )
}

pub fn sys_write_perf_sensor(
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (avg_ms, min_ms, max_ms, samples) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .map(|ft| {
            let avg = ft.average().unwrap_or(0.0);
            let (mn, mx, n) = ft
                .values()
                .fold((f64::MAX, 0.0_f64, 0usize), |(mn, mx, n), v| {
                    (mn.min(*v), mx.max(*v), n + 1)
                });
            (avg, if n > 0 { mn } else { 0.0 }, mx, n)
        })
        .unwrap_or((0.0, 0.0, 0.0, 0));

    let fps_smoothed = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .unwrap_or(0.0);

    // Audit 2026-07-01 — temps GPU par frame (RenderDiagnosticsPlugin) pour trancher
    // CPU-bound vs GPU-bound. Décalage 1 frame (GPU frame N lu en N+1) — sans effet à 1Hz.
    let (gpu_sum_ms, render_cpu_sum_ms, gpu_passes, render_total, gpu_paths) =
        render_frame_stats(&diagnostics);
    let gpu_ratio = if avg_ms > 0.001 {
        gpu_sum_ms / avg_ms
    } else {
        0.0
    };
    // render_cpu_ratio : part du frame passée en encodage CPU des passes de rendu.
    // Élevé → le coût est la soumission/préparation du rendu (meshes/draw calls) ;
    // bas alors que cpu_bound → le coût est dans les systèmes gameplay (main world).
    let render_cpu_ratio = if avg_ms > 0.001 {
        render_cpu_sum_ms / avg_ms
    } else {
        0.0
    };
    let hint = bound_hint(avg_ms, gpu_sum_ms, gpu_passes);

    let (severity, next_step) = severity_for_perf(avg_ms);

    let json = format!(
        r#"{{"id":"perf","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"frame_time_avg_ms":{:.3},"frame_time_min_ms":{:.3},"frame_time_max_ms":{:.3},"fps_smoothed":{:.1},"samples":{},"gpu_time_sum_ms":{:.3},"gpu_pass_samples":{},"render_paths_total":{},"gpu_frame_ratio":{:.3},"render_cpu_sum_ms":{:.3},"render_cpu_ratio":{:.3},"bound_hint":"{hint}","gpu_paths_sample":{gpu_paths}}}"#,
        time.elapsed_secs(),
        avg_ms,
        min_ms,
        max_ms,
        fps_smoothed,
        samples,
        gpu_sum_ms,
        gpu_passes,
        render_total,
        gpu_ratio,
        render_cpu_sum_ms,
        render_cpu_ratio,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_perf.json", json) {
        warn!("[forgia-observability] perf sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_25ms() {
        let (sev, next) = severity_for_perf(16.6);
        assert_eq!(sev, "ok");
        assert!(next.is_empty());
    }

    #[test]
    fn severity_warn_25_to_50ms() {
        let (sev, next) = severity_for_perf(33.3);
        assert_eq!(sev, "warn");
        assert!(next.contains("perf budget"));
    }

    #[test]
    fn severity_critical_above_50ms() {
        let (sev, next) = severity_for_perf(60.0);
        assert_eq!(sev, "critical");
        assert!(next.contains("Tracy"));
    }

    #[test]
    fn severity_boundary_25_is_ok() {
        assert_eq!(severity_for_perf(25.0).0, "ok");
        assert_eq!(severity_for_perf(25.001).0, "warn");
    }

    #[test]
    fn bound_hint_headroom_when_frame_fast() {
        // 4.17ms / 241fps : aucun goulot, même si le ratio GPU est bas.
        assert_eq!(bound_hint(4.17, 0.88, 14), "headroom");
        assert_eq!(bound_hint(13.9, 0.5, 14), "headroom");
        assert_eq!(bound_hint(14.0, 0.5, 14), "cpu_bound"); // borne : 14ms = on classe
    }

    #[test]
    fn bound_hint_unavailable_when_no_gpu_passes() {
        assert_eq!(bound_hint(16.6, 0.0, 0), "gpu_timing_unavailable");
        // 0 passe matchée = pas de verdict, même si une somme traîne.
        assert_eq!(bound_hint(16.6, 5.0, 0), "gpu_timing_unavailable");
    }

    #[test]
    fn bound_hint_gpu_bound_high_ratio() {
        assert_eq!(bound_hint(16.0, 14.0, 8), "gpu_bound"); // ratio 0.875
    }

    #[test]
    fn bound_hint_cpu_bound_low_ratio() {
        assert_eq!(bound_hint(20.0, 6.0, 8), "cpu_bound"); // ratio 0.30
    }

    #[test]
    fn bound_hint_balanced_mid_ratio() {
        assert_eq!(bound_hint(16.0, 10.0, 8), "balanced_or_vsync"); // ratio 0.625
    }
}

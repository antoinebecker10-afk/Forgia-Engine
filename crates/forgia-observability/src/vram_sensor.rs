//! vram_sensor.rs — Producteur `forgia2_vram.json` (~5s, cross-mode).
//!
//! Port V2 de `forgia-game/src/debug/sensors/vram_breakdown.rs` (V1, même
//! Bevy 0.18.1). Comble l'angle mort `forgia2_memory.json::vram_status = "N/A"` :
//! wgpu 0.18 n'expose pas de budget mémoire GPU cross-backend, MAIS on peut
//! **estimer** la VRAM côté CPU en itérant `Assets<Image>` + `Assets<Mesh>` et
//! en sommant la taille estimée de chaque asset (texture descriptor + mipmaps,
//! vertex count × stride). Ne requête PAS le driver → portable, déterministe.
//!
//! Bonus vs un simple total : exporte les **top offenders** (textures/meshes
//! les plus lourds) avec leur path → après une alerte VRAM, on sait QUI blâmer.
//!
//! Coût : itération O(assets) toutes les ~5s (pas hot path). `severity` heuristique
//! sur le total estimé (consts, pattern identique à `memory_sensor`/`perf_sensor`).

use bevy::asset::AssetServer;
use bevy::prelude::*;
use serde::Serialize;

/// Poids de la chaîne de mipmaps (miroir `vram_breakdown::MIPMAP_FACTOR` V1).
const MIPMAP_FACTOR: f64 = 1.33;
/// Stride approx par vertex (position+normal+uv+tangent ~ 32 o). Miroir V1.
const MESH_VERTEX_STRIDE: u64 = 32;
const MESH_INDEX_FUDGE: f64 = 1.10;
/// Top offenders exportés par type d'asset (compact pour l'overlay).
const TOP_K: usize = 10;
/// Intervalle de refresh — VRAM bouge lentement, pas besoin de 1Hz.
const REFRESH_SECS: f32 = 5.0;

/// Severity sur la VRAM estimée (MB). Tunable — démarrage heuristique cohérent
/// avec `memory_sensor` (RAM 4/8 GB) : la VRAM estimée est typiquement < RAM.
pub fn severity_for_vram(total_mb: f64) -> (&'static str, &'static str) {
    if total_mb > 4096.0 {
        (
            "critical",
            "VRAM estimée > 4GB — lire forgia2_vram.json top_images/top_meshes pour la source",
        )
    } else if total_mb > 2048.0 {
        ("warn", "VRAM estimée > 2GB — surveiller top offenders")
    } else {
        ("ok", "")
    }
}

#[derive(Serialize, Clone, Debug)]
struct AssetEntry {
    path: String,
    size_mb: f64,
    detail: String,
}

#[derive(Serialize)]
struct VramPayload<'a> {
    id: &'a str,
    severity: &'a str,
    next_step: &'a str,
    timestamp_secs: f32,
    total_estimated_mb: f64,
    images_total_mb: f64,
    meshes_total_mb: f64,
    images_count: usize,
    meshes_count: usize,
    /// Part du plus gros asset dans le total (0..1). > 0.20 = un asset domine.
    top1_share: f64,
    /// Part cumulée des top-K.
    topk_share: f64,
    top_images: &'a [AssetEntry],
    top_meshes: &'a [AssetEntry],
}

/// Octets VRAM estimés pour une image (base × mipmaps), via le texture descriptor.
fn image_bytes(img: &Image) -> u64 {
    let desc = &img.texture_descriptor;
    let w = u64::from(desc.size.width);
    let h = u64::from(desc.size.height);
    let layers = u64::from(desc.size.depth_or_array_layers);
    let (block_w, block_h) = desc.format.block_dimensions();
    let block_size = u64::from(desc.format.block_copy_size(None).unwrap_or(4));
    let blocks_x = w.div_ceil(u64::from(block_w));
    let blocks_y = h.div_ceil(u64::from(block_h));
    let base = blocks_x * blocks_y * layers * block_size;
    (base as f64 * MIPMAP_FACTOR) as u64
}

/// Octets VRAM estimés pour un mesh (vertex buffer + fudge index).
fn mesh_bytes(mesh: &Mesh) -> u64 {
    let raw = mesh.count_vertices() as u64 * MESH_VERTEX_STRIDE;
    (raw as f64 * MESH_INDEX_FUDGE) as u64
}

pub fn sys_write_vram_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    images: Res<Assets<Image>>,
    meshes: Res<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
) {
    // Accumulateur delta (pattern memory_sensor/perf_sensor) — immunisé contre
    // elapsed_secs==0.0 au 1er tick (cf BUG-581-01). 1er write après REFRESH_SECS,
    // assets encore en cours de chargement avant donc lecture plus complète.
    *accum += time.delta_secs();
    if *accum < REFRESH_SECS {
        return;
    }
    *accum = 0.0;
    let now = time.elapsed_secs();

    let mut img_entries: Vec<AssetEntry> = Vec::with_capacity(images.len());
    let mut img_total: u64 = 0;
    for (id, img) in images.iter() {
        let bytes = image_bytes(img);
        img_total += bytes;
        let path = asset_server
            .get_path(id.untyped())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "<runtime>".to_string());
        let desc = &img.texture_descriptor;
        let detail = format!(
            "{}x{}x{} {:?}",
            desc.size.width, desc.size.height, desc.size.depth_or_array_layers, desc.format
        );
        img_entries.push(AssetEntry {
            path,
            size_mb: bytes as f64 / (1024.0 * 1024.0),
            detail,
        });
    }
    img_entries.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut mesh_entries: Vec<AssetEntry> = Vec::with_capacity(meshes.len());
    let mut mesh_total: u64 = 0;
    for (id, mesh) in meshes.iter() {
        let bytes = mesh_bytes(mesh);
        mesh_total += bytes;
        let path = asset_server
            .get_path(id.untyped())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "<runtime>".to_string());
        let detail = format!("{} verts", mesh.count_vertices());
        mesh_entries.push(AssetEntry {
            path,
            size_mb: bytes as f64 / (1024.0 * 1024.0),
            detail,
        });
    }
    mesh_entries.sort_by(|a, b| {
        b.size_mb
            .partial_cmp(&a.size_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let images_total_mb = img_total as f64 / (1024.0 * 1024.0);
    let meshes_total_mb = mesh_total as f64 / (1024.0 * 1024.0);
    let total_mb = images_total_mb + meshes_total_mb;

    let top_images: Vec<AssetEntry> = img_entries.iter().take(TOP_K).cloned().collect();
    let top_meshes: Vec<AssetEntry> = mesh_entries.iter().take(TOP_K).cloned().collect();

    let top1_share = if total_mb > 0.0 {
        top_images
            .first()
            .map(|e| e.size_mb)
            .unwrap_or(0.0)
            .max(top_meshes.first().map(|e| e.size_mb).unwrap_or(0.0))
            / total_mb
    } else {
        0.0
    };
    let topk_share = if total_mb > 0.0 {
        let img_sum: f64 = top_images.iter().map(|e| e.size_mb).sum();
        let mesh_sum: f64 = top_meshes.iter().map(|e| e.size_mb).sum();
        (img_sum + mesh_sum) / total_mb
    } else {
        0.0
    };

    let (severity, next_step) = severity_for_vram(total_mb);

    let payload = VramPayload {
        id: "vram",
        severity,
        next_step,
        timestamp_secs: now,
        total_estimated_mb: total_mb,
        images_total_mb,
        meshes_total_mb,
        images_count: images.len(),
        meshes_count: meshes.len(),
        top1_share,
        topk_share,
        top_images: &top_images,
        top_meshes: &top_meshes,
    };

    match serde_json::to_string(&payload) {
        Ok(json) => {
            if let Err(e) = std::fs::write("forgia2_vram.json", &json) {
                warn!("[forgia-observability] vram sensor write failed: {e}");
            }
        }
        Err(e) => warn!("[forgia-observability] vram sensor serialize failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_below_2gb() {
        assert_eq!(severity_for_vram(1024.0).0, "ok");
    }

    #[test]
    fn severity_warn_between_2_4gb() {
        let (sev, next) = severity_for_vram(3000.0);
        assert_eq!(sev, "warn");
        assert!(next.contains("top offenders"));
    }

    #[test]
    fn severity_critical_above_4gb() {
        let (sev, next) = severity_for_vram(5000.0);
        assert_eq!(sev, "critical");
        assert!(next.contains("forgia2_vram.json"));
    }

    #[test]
    fn severity_boundaries() {
        assert_eq!(severity_for_vram(2048.0).0, "ok");
        assert_eq!(severity_for_vram(2048.001).0, "warn");
        assert_eq!(severity_for_vram(4096.0).0, "warn");
        assert_eq!(severity_for_vram(4096.001).0, "critical");
    }

    #[test]
    fn mesh_bytes_zero_vertices() {
        let mut m = Mesh::new(
            bevy::mesh::PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        m.insert_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new());
        assert_eq!(mesh_bytes(&m), 0);
    }

    #[test]
    fn mipmap_factor_stable() {
        assert!((MIPMAP_FACTOR - 1.33).abs() < 1e-9);
    }
}

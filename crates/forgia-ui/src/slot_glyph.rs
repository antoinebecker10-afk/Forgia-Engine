//! slot_glyph.rs — **les cinq silhouettes d'équipement**, dessinées (story-678).
//!
//! Le pack d'icônes du projet (Layer Lab Casual) ne contient aucune pièce
//! d'armure : ni casque, ni plastron, ni gants. Plutôt que d'importer un second
//! pack — donc une seconde licence à suivre et un second style à accorder — les
//! cinq types sont **tracés en egui**.
//!
//! Ça coûte moins qu'un asset et ça tient mieux : la silhouette prend la couleur
//! de la rareté portée, donc elle dit **quoi** et **de quelle qualité** d'un seul
//! coup d'œil, ce qu'une image fixe ne saurait pas faire sans cinq variantes par
//! rareté (soit vingt-cinq fichiers).
//!
//! Le trait est cerné et les formes pleines, comme la DA cartoon du projet.

use bevy_egui::egui;

/// Dessine la silhouette du type d'emplacement dans `rect`.
///
/// `slot_id` vient du génome (`helmet` / `chest` / `gloves` / `legs` / `boots`).
/// Un id inconnu tombe sur un carré neutre plutôt que sur rien : une pièce
/// ajoutée au génome doit rester visible en attendant sa silhouette.
pub fn draw(painter: &egui::Painter, rect: egui::Rect, slot_id: &str, col: egui::Color32) {
    // Tout est tracé en fractions du cadre : la silhouette suit la taille qu'on
    // lui donne, du portrait (62 px) à la case du sac (56 px), sans réglage.
    let w = rect.width();
    let h = rect.height();
    let c = rect.center();
    let cerne = egui::Stroke::new((w * 0.055).max(1.5), egui::Color32::from_rgb(18, 12, 22));
    let plein = col;

    let rond = |centre: egui::Pos2, r: f32| {
        painter.circle_filled(centre, r, plein);
        painter.circle_stroke(centre, r, cerne);
    };
    let boite = |min: egui::Pos2, taille: egui::Vec2, arrondi: u8| {
        let r = egui::Rect::from_min_size(min, taille);
        painter.rect_filled(r, egui::CornerRadius::same(arrondi), plein);
        painter.rect_stroke(
            r,
            egui::CornerRadius::same(arrondi),
            cerne,
            egui::StrokeKind::Inside,
        );
    };

    match slot_id {
        // Casque : une calotte et sa visière.
        "helmet" => {
            rond(c - egui::vec2(0.0, h * 0.06), w * 0.30);
            boite(
                egui::pos2(c.x - w * 0.32, c.y + h * 0.04),
                egui::vec2(w * 0.64, h * 0.16),
                4,
            );
        }
        // Plastron : un tronc qui s'évase aux épaules.
        "chest" => {
            boite(
                egui::pos2(c.x - w * 0.30, c.y - h * 0.28),
                egui::vec2(w * 0.60, h * 0.20),
                5,
            );
            boite(
                egui::pos2(c.x - w * 0.22, c.y - h * 0.12),
                egui::vec2(w * 0.44, h * 0.42),
                4,
            );
        }
        // Gants : la paume et le pouce.
        "gloves" => {
            boite(
                egui::pos2(c.x - w * 0.22, c.y - h * 0.26),
                egui::vec2(w * 0.40, h * 0.46),
                6,
            );
            boite(
                egui::pos2(c.x + w * 0.16, c.y - h * 0.10),
                egui::vec2(w * 0.16, h * 0.22),
                5,
            );
        }
        // Jambières : deux fuseaux.
        "legs" => {
            for signe in [-1.0_f32, 1.0] {
                boite(
                    egui::pos2(c.x + signe * w * 0.20 - w * 0.13, c.y - h * 0.28),
                    egui::vec2(w * 0.26, h * 0.56),
                    5,
                );
            }
        }
        // Bottes : deux L, tige et semelle.
        "boots" => {
            for signe in [-1.0_f32, 1.0] {
                let x = c.x + signe * w * 0.19;
                boite(
                    egui::pos2(x - w * 0.10, c.y - h * 0.26),
                    egui::vec2(w * 0.20, h * 0.36),
                    4,
                );
                boite(
                    egui::pos2(x - w * 0.10, c.y + h * 0.10),
                    egui::vec2(w * 0.28, h * 0.16),
                    4,
                );
            }
        }
        // Type inconnu : une forme neutre, pour qu'un ajout au génome se voie.
        _ => boite(
            egui::pos2(c.x - w * 0.26, c.y - h * 0.26),
            egui::vec2(w * 0.52, h * 0.52),
            6,
        ),
    }
}

#[cfg(test)]
mod tests {
    /// Les cinq types du génome ont chacun leur silhouette — un `_ =>` silencieux
    /// laisserait passer un type sans dessin, et ça ne se verrait qu'à l'écran.
    #[test]
    fn les_cinq_types_du_genome_sont_couverts() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/genomes/roguelite/roguelite_equipment.toml");
        let Ok(src) = std::fs::read_to_string(root) else {
            eprintln!("génome absent (checkout partiel), contrôle non exécuté");
            return;
        };
        // Les ids d'emplacement sont ceux qui suivent un `[[slots]]`.
        let mut ids = Vec::new();
        let mut dans_slot = false;
        for l in src.lines() {
            let l = l.trim();
            if l == "[[slots]]" {
                dans_slot = true;
            } else if dans_slot && l.starts_with("id = ") {
                ids.push(l.trim_start_matches("id = ").trim_matches('"').to_string());
                dans_slot = false;
            }
        }
        assert!(!ids.is_empty(), "aucun emplacement lu dans le génome");
        const DESSINES: [&str; 5] = ["helmet", "chest", "gloves", "legs", "boots"];
        for id in &ids {
            assert!(
                DESSINES.contains(&id.as_str()),
                "l'emplacement « {id} » n'a pas de silhouette — il tomberait sur la forme neutre"
            );
        }
    }
}

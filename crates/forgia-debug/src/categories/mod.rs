//! Pattern catégories — inspiration Unreal GameplayDebugger.
//!
//! Chaque catégorie expose `draw(ui, snapshot)`. La registry route le rendu
//! depuis `CategoryId`. Extensible runtime via `CategoryRegistry::register()`.

use crate::snapshot::SensorSnapshot;
use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashMap;

pub mod anim;
pub mod audio;
pub mod combat;
pub mod physics;
pub mod player;
pub mod system;
pub mod terrain;

/// Identifiant typé de catégorie. Étendre l'enum + `ALL` + `name`/`numpad_digit`
/// pour ajouter une catégorie built-in.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum CategoryId {
    System,
    Combat,
    Player,
    Terrain,
    Anim,
    Audio,
    Physics,
}

impl CategoryId {
    pub const ALL: &'static [CategoryId] = &[
        CategoryId::System,
        CategoryId::Combat,
        CategoryId::Player,
        CategoryId::Terrain,
        CategoryId::Anim,
        CategoryId::Audio,
        CategoryId::Physics,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            CategoryId::System => "System",
            CategoryId::Combat => "Combat",
            CategoryId::Player => "Player",
            CategoryId::Terrain => "Terrain",
            CategoryId::Anim => "Anim",
            CategoryId::Audio => "Audio",
            CategoryId::Physics => "Physics",
        }
    }

    pub fn numpad_digit(&self) -> u8 {
        match self {
            CategoryId::System => 1,
            CategoryId::Combat => 2,
            CategoryId::Player => 3,
            CategoryId::Terrain => 4,
            CategoryId::Anim => 5,
            CategoryId::Audio => 6,
            CategoryId::Physics => 7,
        }
    }
}

/// Trait implémenté par chaque catégorie. `draw` lit le snapshot et rend
/// l'UI egui. Pas d'allocation requise — egui::Ui gère le widget tree.
pub trait DebugCategory: Send + Sync {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot);
}

/// Registry des catégories. Built-ins enregistrées au `default()`, mais
/// `register_custom()` permet à n'importe quelle crate métier d'ajouter
/// sa propre catégorie sans toucher forgia-debug.
#[derive(Resource)]
pub struct CategoryRegistry {
    categories: HashMap<CategoryId, Box<dyn DebugCategory>>,
}

impl Default for CategoryRegistry {
    fn default() -> Self {
        let mut categories: HashMap<CategoryId, Box<dyn DebugCategory>> = HashMap::new();
        categories.insert(CategoryId::System, Box::new(system::SystemCategory));
        categories.insert(CategoryId::Combat, Box::new(combat::CombatCategory));
        categories.insert(CategoryId::Player, Box::new(player::PlayerCategory));
        categories.insert(CategoryId::Terrain, Box::new(terrain::TerrainCategory));
        categories.insert(CategoryId::Anim, Box::new(anim::AnimCategory));
        categories.insert(CategoryId::Audio, Box::new(audio::AudioCategory));
        categories.insert(CategoryId::Physics, Box::new(physics::PhysicsCategory));
        Self { categories }
    }
}

impl CategoryRegistry {
    pub fn register_custom(&mut self, id: CategoryId, cat: Box<dyn DebugCategory>) {
        self.categories.insert(id, cat);
    }

    pub fn draw_category(&self, id: CategoryId, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        if let Some(cat) = self.categories.get(&id) {
            cat.draw(ui, snap);
        } else {
            ui.label(format!("(category {} not registered)", id.name()));
        }
    }
}

/// Helper interne — formatte `Option<T>` en string ou "n/a".
pub(crate) fn fmt_opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map_or("n/a".into(), |x| format!("{x}"))
}

pub(crate) fn fmt_opt_str(v: Option<&str>) -> String {
    v.unwrap_or("n/a").to_string()
}

pub(crate) fn fmt_opt_f64(v: Option<f64>, decimals: usize) -> String {
    v.map_or("n/a".into(), |x| format!("{x:.*}", decimals))
}

pub(crate) fn fmt_opt_vec3(v: Option<[f64; 3]>) -> String {
    v.map_or("n/a".into(), |[x, y, z]| {
        format!("[{x:.2}, {y:.2}, {z:.2}]")
    })
}

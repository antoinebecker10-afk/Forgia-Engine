//! pipeline_ready.rs — détecteur générique « tous les pipelines de rendu sont
//! compilés » (anti-freeze de specialization au 1er affichage).
//!
//! Symptôme visé (audit 2026-07-20) : freezes 45-146 ms **quand le joueur tourne
//! la caméra** = compilation de pipeline PBR au 1er affichage d'un mesh/matériau
//! de décor entrant dans le frustum. Bevy ne fournit AUCUN warmup natif (issue
//! bevyengine/bevy#10871 ouverte) — seulement les primitives. Ce module expose la
//! resource [`PipelinesReady`] alimentée depuis le `RenderApp` via
//! `PipelineCache::waiting_pipelines()`, pattern exact de l'exemple officiel
//! Bevy 0.18 `examples/games/loading_screen.rs`.
//!
//! ⚠️ Piège confirmé (bevy-specialist 2026-07-20) : un pipeline PBR ne se compile
//! QUE si son entité est **réellement dans le frustum** d'une caméra
//! (`ViewVisibility==true`). `Visibility::Hidden` / hors-champ (le pattern des
//! dummies Hanabi à Y=-10000) NE compile PAS un `StandardMaterial`. Le warmup qui
//! consomme ce détecteur (`forgia-mode-roguelite::pipeline_warmup`) rend donc ses
//! entités visibles (minuscules, occluses par le viewmodel), jamais cachées.

use bevy::prelude::*;
use bevy::render::render_resource::PipelineCache;
use bevy::render::{ExtractSchedule, MainWorld, RenderApp};

/// `true` dès qu'aucun pipeline de rendu n'est plus en attente de compilation.
/// Alimentée chaque frame depuis le `RenderApp` ; lue par les passes de warmup
/// (gate « le diorama a fini de compiler, on peut le despawn »).
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PipelinesReady(pub bool);

/// Instantané du cache de pipelines rendu, copié chaque frame vers le monde
/// principal. Ce compteur est une preuve plus fiable qu'une attribution par
/// défaut : un hitch sans spawn n'est pas forcément une compilation shader.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct PipelineCacheStats {
    /// Pipelines encore en attente de compilation/validation par le backend.
    pub waiting: usize,
}

/// Branche le suivi de `PipelineCache` (sous-app rendu) sur une resource lisible
/// depuis le monde principal. Idempotent-safe : ne fait rien si le `RenderApp`
/// n'existe pas (mode headless/test sans backend rendu).
pub struct PipelinesReadyPlugin;

impl Plugin for PipelinesReadyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PipelinesReady>()
            .init_resource::<PipelineCacheStats>();
        // Le `RenderApp` est absent en headless (tests, CI sans GPU) → skip propre.
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(ExtractSchedule, extract_pipelines_ready);
        }
    }
}

/// Tourne dans `ExtractSchedule` du `RenderApp` : compte les pipelines encore en
/// attente et écrit le booléen dans le monde principal (`MainWorld`).
fn extract_pipelines_ready(mut main_world: ResMut<MainWorld>, cache: Res<PipelineCache>) {
    let waiting = cache.waiting_pipelines().count();
    let ready = waiting == 0;
    if let Some(mut flag) = main_world.get_resource_mut::<PipelinesReady>() {
        flag.0 = ready;
    }
    if let Some(mut stats) = main_world.get_resource_mut::<PipelineCacheStats>() {
        stats.waiting = waiting;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_not_ready() {
        // Avant tout render, le gate doit être « pas prêt » (ne pas despawn un
        // diorama de warmup qui n'a encore rien compilé).
        assert!(!PipelinesReady::default().0);
    }
}

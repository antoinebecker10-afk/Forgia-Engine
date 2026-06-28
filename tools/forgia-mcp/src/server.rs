use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        Annotated, ListResourcesResult, PaginatedRequestParams, RawResource, ReadResourceRequestParams,
        ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── paths ───────────────────────────────────────────────────────────────────

/// Workspace root (post story-100 split: forgia-engine + forgia-game)
const FORGIA_ROOT: &str = "D:/Forgia/RUST/Forgia/Forgia";

fn forgia_path(rel: &str) -> String {
    format!("{FORGIA_ROOT}/{rel}")
}

// ─── param types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EmptyParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenomeQueryParams {
    #[schemars(description = "Nom du genome (ex: combat_default, biome_forest, weapon_modern_ar)")]
    pub genome_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GeneSearchParams {
    #[schemars(description = "Terme de recherche (nom de gene, chromosome, ou mot-cle). Case-insensitive.")]
    pub query: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetFpsTuningParams {
    #[schemars(description = "Nom du parametre FpsTuning (ex: run_speed, fireball_damage)")]
    pub param: String,
    #[schemars(description = "Nouvelle valeur f32")]
    pub value: f32,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BridgeCommand {
    SetFpsTuning { param: String, value: f32 },
}

const BRIDGE_DIR: &str = "D:/Forgia/mcp_bridge";

// ─── server ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ForgiaMcp {
    #[allow(dead_code)] // used by #[tool_router] macro
    tool_router: ToolRouter<Self>,
}

impl ForgiaMcp {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

// ─── tools ───────────────────────────────────────────────────────────────────

#[tool_router]
impl ForgiaMcp {
    #[tool(description = "Lance `cargo check -p forgia-game` sur le workspace. Retourne les erreurs de compilation.")]
    async fn cargo_check(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        run_cargo(&["check", "-p", "forgia-game", "--message-format=human"], FORGIA_ROOT).await
    }

    #[tool(description = "Lance `cargo clippy -p forgia-game -- -W warnings`. Retourne les warnings clippy.")]
    async fn cargo_clippy(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        run_cargo(&["clippy", "-p", "forgia-game", "--", "-W", "warnings"], FORGIA_ROOT).await
    }

    #[tool(description = "Verifie le Stability Lock L1 : detecte les appels asset_server.load() illicites hors exceptions autorisees.")]
    async fn check_stability_locks(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let violations = find_illegal_asset_loads(&forgia_path("forgia-game/src")).await?;
        Ok(if violations.is_empty() {
            "OK: aucune violation L1 detectee (asset_server.load() hors exceptions).".to_string()
        } else {
            format!("VIOLATIONS L1 ({}):\n{}", violations.len(), violations.join("\n"))
        })
    }

    #[tool(description = "Parse forgia-game/src/assets.rs et compte les GameAssets (scenes, anims, audio, textures).")]
    async fn list_game_assets(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = forgia_path("forgia-game/src/assets.rs");
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(format!("Lecture assets.rs echouee: {e}"), None))?;
        let scenes = content.matches("Handle<Scene>").count();
        let anims  = content.matches("Handle<AnimationClip>").count();
        let audio  = content.matches("Handle<AudioSource>").count();
        let images = content.matches("Handle<Image>").count();
        Ok(format!(
            "GameAssets (forgia-game/src/assets.rs):\n  Handle<Scene>:         {scenes}\n  Handle<AnimationClip>: {anims}\n  Handle<AudioSource>:   {audio}\n  Handle<Image>:         {images}\n  TOTAL:                 {}",
            scenes + anims + audio + images
        ))
    }

    // ─── genome tools ─────────────────────────────────────────────────────

    #[tool(description = "Liste tous les genomes TOML avec leur nombre de genes, domaine et categorie.")]
    async fn list_genomes(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let dir = forgia_path("config/genomes");
        let summaries = scan_genomes(&dir).await?;
        if summaries.is_empty() {
            return Ok("Aucun genome trouve dans config/genomes/".to_string());
        }
        let mut lines = vec![format!("{} genomes:\n", summaries.len())];
        for s in &summaries {
            let extends = s.extends.as_deref().unwrap_or("-");
            lines.push(format!(
                "  {:40} {:12} {:10} genes={:3} constraints={:2} extends={}",
                s.id, s.domain, s.category, s.gene_count, s.constraint_count, extends
            ));
        }
        let total_genes: usize = summaries.iter().map(|s| s.gene_count).sum();
        let total_constraints: usize = summaries.iter().map(|s| s.constraint_count).sum();
        lines.push(format!("\nTotal: {} genes, {} constraints", total_genes, total_constraints));
        Ok(lines.join("\n"))
    }

    #[tool(description = "Retourne le detail complet d'un genome : genes (id, chromosome, min/max/default), constraints, extends.")]
    async fn query_genome(
        &self,
        Parameters(p): Parameters<GenomeQueryParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = forgia_path(&format!("config/genomes/{}.toml", p.genome_id));
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| rmcp::ErrorData::invalid_params(
                format!("Genome '{}' introuvable ({})", p.genome_id, path),
                None,
            ))?;
        let doc: toml::Value = content.parse()
            .map_err(|e: toml::de::Error| rmcp::ErrorData::internal_error(format!("Parse TOML: {e}"), None))?;
        let table = doc.as_table().unwrap();

        let id = table.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let name = table.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        let domain = table.get("domain").and_then(|v| v.as_str()).unwrap_or("?");
        let extends = table.get("extends").and_then(|v| v.as_str());

        let mut out = format!("# {} ({})\nDomain: {} | Extends: {}\n\n", id, name, domain, extends.unwrap_or("none"));

        // Genes grouped by chromosome
        if let Some(genes) = table.get("genes").and_then(|v| v.as_array()) {
            let mut by_chrom: HashMap<String, Vec<String>> = HashMap::new();
            for g in genes {
                let t = g.as_table().unwrap();
                let gid = t.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                let chrom = t.get("chromosome").and_then(|v| v.as_str()).unwrap_or("Default");
                let min = t.get("min").and_then(|v| v.as_float()).unwrap_or(0.0);
                let max = t.get("max").and_then(|v| v.as_float()).unwrap_or(1.0);
                let default = t.get("default").and_then(|v| v.as_float()).unwrap_or(0.0);
                let label = t.get("label").and_then(|v| v.as_str()).unwrap_or("");
                by_chrom.entry(chrom.to_string()).or_default()
                    .push(format!("    {:40} [{:8.2} .. {:8.2}] default={:8.2}  {}", gid, min, max, default, label));
            }
            let mut chroms: Vec<_> = by_chrom.keys().cloned().collect();
            chroms.sort();
            for chrom in chroms {
                let genes = &by_chrom[&chrom];
                out.push_str(&format!("## {} ({} genes)\n", chrom, genes.len()));
                for line in genes {
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
        }

        // Constraints
        if let Some(constraints) = table.get("constraints").and_then(|v| v.as_array()) {
            out.push_str(&format!("## Constraints ({})\n", constraints.len()));
            for c in constraints {
                let t = c.as_table().unwrap();
                let src = t.get("source").and_then(|v| v.as_str()).unwrap_or("?");
                let tgt = t.get("target").and_then(|v| v.as_str()).unwrap_or("?");
                let ratio = t.get("ratio").and_then(|v| v.as_float()).unwrap_or(0.0);
                out.push_str(&format!("    {} → {} (ratio {:.2})\n", src, tgt, ratio));
            }
        }

        Ok(out)
    }

    #[tool(description = "Cherche un gene par nom/mot-cle dans tous les genomes. Retourne genome, chromosome, valeur, range.")]
    async fn search_gene(
        &self,
        Parameters(p): Parameters<GeneSearchParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let dir = forgia_path("config/genomes");
        let query = p.query.to_lowercase();
        let mut results = Vec::new();

        let mut entries = tokio::fs::read_dir(&dir).await
            .map_err(|e| rmcp::ErrorData::internal_error(format!("read_dir: {e}"), None))?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };
            let doc: toml::Value = match content.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let table = doc.as_table().unwrap();
            let genome_id = table.get("id").and_then(|v| v.as_str()).unwrap_or("?");

            if let Some(genes) = table.get("genes").and_then(|v| v.as_array()) {
                for g in genes {
                    let t = g.as_table().unwrap();
                    let gid = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let label = t.get("label").and_then(|v| v.as_str()).unwrap_or("");
                    let chrom = t.get("chromosome").and_then(|v| v.as_str()).unwrap_or("");
                    let layer = t.get("layer").and_then(|v| v.as_str()).unwrap_or("");

                    if gid.to_lowercase().contains(&query)
                        || label.to_lowercase().contains(&query)
                        || chrom.to_lowercase().contains(&query)
                        || layer.to_lowercase().contains(&query)
                    {
                        let min = t.get("min").and_then(|v| v.as_float()).unwrap_or(0.0);
                        let max = t.get("max").and_then(|v| v.as_float()).unwrap_or(1.0);
                        let default = t.get("default").and_then(|v| v.as_float()).unwrap_or(0.0);
                        results.push(format!(
                            "  {:30} {:30} {:15} [{:.2}..{:.2}] default={:.2}  {}",
                            genome_id, gid, chrom, min, max, default, label
                        ));
                    }
                }
            }
        }

        if results.is_empty() {
            Ok(format!("Aucun gene trouvé pour '{}'", p.query))
        } else {
            results.sort();
            Ok(format!("{} resultats pour '{}':\n{}", results.len(), p.query, results.join("\n")))
        }
    }

    // ─── ECS bridge tools ───────────────────────────────────────────────

    #[tool(description = "Lit l'etat ECS Forgia en live (FpsTuning, timestamp). Necessite FORGIA_MCP_BRIDGE=1 et le jeu en cours.")]
    async fn get_fps_tuning(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = format!("{BRIDGE_DIR}/state.json");
        tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| rmcp::ErrorData::internal_error(
                "state.json introuvable. Le jeu tourne-t-il avec FORGIA_MCP_BRIDGE=1 ?",
                None,
            ))
    }

    // ─── AI Autonomy: always-on diagnostic exports (story-313) ──────────

    #[tool(description = "Lit le dump JSON complet de FpsTuning (~150 params combat/movement/camera/AI/VFX). Always-on, refresh 10s post-load.")]
    async fn read_fps_tuning(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_fps_tuning.json").await
    }

    #[tool(description = "Lit GraphicsSettings (preset, entity_budget, vegetation_density, shadow_distance, fog, ssao, ssgi, etc.). Always-on, refresh 10s.")]
    async fn read_graphics_settings(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_graphics_settings.json").await
    }

    #[tool(description = "Lit MapGenConfig (octaves, warp_strength, features, biome_cell_size, vegetation_density). Always-on, refresh 10s.")]
    async fn read_mapgen_config(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_mapgen_config.json").await
    }

    #[tool(description = "Lit GameSession (phase Setup/Countdown/Playing/Results, countdown_timer, go_timer). Always-on, refresh 10s.")]
    async fn read_gameplay_state(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_gameplay_state.json").await
    }

    #[tool(description = "Lit forgia_diagnostic_360.json (10 categories Performance/Physics/Terrain/Memory/Entities/Player/Rendering/Tuning/Health/Visual avec status Ok/Warning/Critical). Refresh F9 + ~10s.")]
    async fn read_diagnostic_report(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_diagnostic_360.json").await
    }

    #[tool(description = "Lit forgia_diagnostics.json (world snapshot live: FPS, entity_count, player_pos, chunks, vegetation, buildings). Refresh 10s.")]
    async fn read_world_snapshot(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_diagnostics.json").await
    }

    // ─── Tier 2 (story-314) ──────────────────────────────────────────────

    #[tool(description = "Lit forgia_entities_snapshot.json (player pos/hp, total_entities, enemies alive/dead). Refresh 10s.")]
    async fn read_entities_snapshot(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_entities_snapshot.json").await
    }

    #[tool(description = "Lit forgia_chunks_snapshot.json (chunks_with_sdf/mesh/dirty/modified, last_player_chunk, managers vegetation/grass/villages/lampposts/roads/enemies). Refresh 10s.")]
    async fn read_chunks_snapshot(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_chunks_snapshot.json").await
    }

    #[tool(description = "Lit forgia_npcs_snapshot.json (total NPCs, breakdown par AI state Idle/Patrol/Alert/Chase/Attack/RangedAttack/Charge, top 50 details name/level/hp/state/pos). Refresh 10s.")]
    async fn read_npcs_snapshot(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_npcs_snapshot.json").await
    }

    #[tool(description = "Lit forgia_panic.json (rapport de crash : message, location file:line, thread, backtrace, timestamp). Disponible si le jeu a crashe au moins une fois.")]
    async fn read_crash_dump(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_panic.json").await
    }

    // ─── Tier 3 (story-315) ──────────────────────────────────────────────

    #[tool(description = "Lit forgia_input_log.json (rolling 500 derniers events keyboard + mouse: key_pressed/key_released, mouse_pressed/released, mouse_wheel). Refresh 1s.")]
    async fn read_input_log(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_input_log.json").await
    }

    #[tool(description = "Lit forgia_perf_history.json (rolling 600 frames de metrics: fps, frame_time_ms, entities, chunks_loaded, gen_pending, mesh_pending, veg_total). Refresh 1s. Permet de detecter spikes/stutters dans les ~10 dernieres secondes.")]
    async fn read_perf_history(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_perf_history.json").await
    }

    #[tool(description = "Lit forgia_memory_breakdown.json (process_memory_mb, growth_rate, asset counts par type Image/Mesh/Material/Scene/AnimationClip/AudioSource, archetype count, entities par categorie). Refresh 5s.")]
    async fn read_memory_breakdown(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_memory_breakdown.json").await
    }

    #[tool(description = "Lit forgia_events_log.json (rolling 500 events gameplay: combat_hit, damage_player, level_up, destruction, terrain_crater, game_mode victory/defeat, biome_discovered, trigger_activated). Refresh 1s.")]
    async fn read_events_log(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_events_log.json").await
    }

    #[tool(description = "Lit forgia_npcs_extended.json (shopkeepers detailles avec role/has_shop/interact_radius/pos + behavior counts followers/wanderers/fleeing/merchants/passive/bosses). Refresh 10s.")]
    async fn read_npcs_extended(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_npcs_extended.json").await
    }

    // ─── Story-316 Crash & Performance Forensics ─────────────────────────

    #[tool(description = "Lit forgia_lag_events.json (rolling 200 frames > 50ms : severity lag/freeze/critical, frame_time_ms, entity_count, chunks state, recent inputs, recent gameplay events). Detection des spikes de performance avec contexte.")]
    async fn read_lag_events(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_lag_events.json").await
    }

    #[tool(description = "Lit forgia_watchdog_heartbeat.json (timestamp_unix_ms, last_main_heartbeat_ms, gap_ms, main_thread_alive). Refresh 1s par un thread separe — permet de verifier que le main thread Bevy est vivant meme s'il est freeze.")]
    async fn read_watchdog_heartbeat(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_watchdog_heartbeat.json").await
    }

    #[tool(description = "Lit forgia_watchdog_alert.json (alert_at_unix_ms, gap_ms, threshold_ms, note). Existe seulement si le main thread a freeze > 5s. Indique deadlock ou boucle infinie.")]
    async fn read_watchdog_alert(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_watchdog_alert.json").await
    }

    #[tool(description = "Lit forgia_memory_leaks.json (timeline 20 snapshots toutes les 30s + alertes si croissance memoire sustained > 10MB/30s pendant 3 cycles consecutifs). Identifie la categorie suspect (images/meshes/scenes/etc).")]
    async fn read_memory_leaks(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_memory_leaks.json").await
    }

    #[tool(description = "Lit forgia_vram_guardian.json (story-317 auto-protection VRAM) : gpu_estimated_mb live, thresholds warn/danger, consecutive counts, current settings (streaming_radius/veg_density/view_distance/entity_budget/ssgi), action log si emergency declenchee.")]
    async fn read_vram_guardian(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_vram_guardian.json").await
    }

    #[tool(description = "Lit forgia_map_composition.json (story-318) : 10 categories de la map en 1 fichier — sky (time_of_day, sun, illuminance, fog), water (planes/rivers/lakes/waterfalls), lighting (point/dir/spot), particles (hanabi count), caves (worm params), castles (spawned/entities), paths (cell_size, grid, player_influence), biome_map (player_biome + 8 voisins), weather (current type + emitters), audio (music/sinks). Refresh 5s.")]
    async fn read_map_composition(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_map_composition.json").await
    }

    #[tool(description = "Lit forgia_ui_state.json (story-320) : etat complet UI/modes. modes (app_mode Play/Build/Edit, world_mode Player/Editor, loading, pause, dashboard, dead), session (exists, phase, blocked), input_blockers (look/move/zoom), panels_open (inventory/grimoire/wheel/ai_chat/console/calibrator/designer/inspector/hud_editor/sculpting/shop/dialogue), overlays (build_grid_visible, editor_gizmos), hud_visibility (predicte SI chaque widget DEVRAIT etre visible : crosshair_expected, hotbar_expected, viewmodel_expected, quest_tracker_expected, minimap_expected) + verdict explicatif. Refresh 1s. ESSENTIEL pour disambiguer 'le HUD est cache' (Build mode normal) vs 'le HUD est casse' (Play mode = bug).")]
    async fn read_ui_state(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        read_export("forgia_ui_state.json").await
    }

    #[tool(description = "Modifie un parametre FpsTuning en live dans le jeu. Necessite FORGIA_MCP_BRIDGE=1 et le jeu en cours.")]
    async fn set_fps_tuning(
        &self,
        Parameters(SetFpsTuningParams { param, value }): Parameters<SetFpsTuningParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let cmd = BridgeCommand::SetFpsTuning { param: param.clone(), value };
        let json = serde_json::to_string(&cmd)
            .map_err(|e: serde_json::Error| rmcp::ErrorData::internal_error(e.to_string(), None))?;

        let path = format!("{BRIDGE_DIR}/cmd.json");
        tokio::fs::write(&path, json.as_bytes())
            .await
            .map_err(|e: std::io::Error| rmcp::ErrorData::internal_error(
                format!("Impossible d'ecrire cmd.json (jeu en cours?): {e}"),
                None,
            ))?;

        // Give the game 2.5s to process the command, then confirm
        tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;
        let state_path = format!("{BRIDGE_DIR}/state.json");
        let state = tokio::fs::read_to_string(&state_path).await.unwrap_or_default();
        Ok(format!("Commande envoyee: FpsTuning.{param} = {value}\n\nEtat actuel:\n{state}"))
    }

    #[tool(description = "Enregistre cette session Claude Code dans le registre. Appele automatiquement au demarrage via hook.")]
    async fn register_session(
        &self,
        Parameters(p): Parameters<SessionParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let mut sessions = load_sessions().await;
        // Purge stale sessions (older than 2h)
        let now = epoch_secs();
        sessions.retain(|s| now.saturating_sub(s.started_at) < 7200);
        // Check for conflict on same project
        let conflict = sessions.iter().find(|s| s.project == p.project && s.pid != p.pid);
        let warning = conflict.map(|c| format!(
            "\n⚠️  CONFLIT: session '{}' (PID {}) travaille deja sur ce projet depuis {}s!",
            c.name, c.pid, now.saturating_sub(c.started_at)
        )).unwrap_or_default();
        sessions.retain(|s| s.pid != p.pid); // remove old entry for same pid
        sessions.push(SessionEntry { pid: p.pid, name: p.name.clone(), project: p.project.clone(), started_at: now });
        save_sessions(&sessions).await;
        Ok(format!("Session '{}' (PID {}) enregistree sur {}{}", p.name, p.pid, p.project, warning))
    }

    #[tool(description = "Liste toutes les sessions Claude Code actives sur les projets Forgia.")]
    async fn list_sessions(
        &self,
        Parameters(_): Parameters<EmptyParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let sessions = load_sessions().await;
        if sessions.is_empty() {
            return Ok("Aucune session active.".to_string());
        }
        let now = epoch_secs();
        let lines: Vec<String> = sessions.iter().map(|s| {
            format!("  PID {:6} | {:20} | {} | {}s", s.pid, s.name, s.project, now.saturating_sub(s.started_at))
        }).collect();
        Ok(format!("{} session(s) active(s):\n{}", sessions.len(), lines.join("\n")))
    }

    #[tool(description = "Libere la session Claude Code (appele a la fin via hook Stop).")]
    async fn release_session(
        &self,
        Parameters(p): Parameters<ReleaseParams>,
    ) -> Result<String, rmcp::ErrorData> {
        let mut sessions = load_sessions().await;
        let before = sessions.len();
        sessions.retain(|s| s.pid != p.pid);
        save_sessions(&sessions).await;
        let removed = before - sessions.len();
        Ok(format!("{removed} session(s) liberee(s) pour PID {}", p.pid))
    }
}

// ─── session structs & params ────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SessionParams {
    #[schemars(description = "Nom de la session (ex: terminal-1, vscode-main)")]
    pub name: String,
    #[schemars(description = "PID du processus Claude Code")]
    pub pid: u32,
    #[schemars(description = "Chemin du projet (cwd)")]
    pub project: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReleaseParams {
    #[schemars(description = "PID de la session a liberer")]
    pub pid: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionEntry {
    pid: u32,
    name: String,
    project: String,
    started_at: u64,
}

const SESSIONS_FILE: &str = "D:/Forgia/mcp_bridge/sessions.json";

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn load_sessions() -> Vec<SessionEntry> {
    tokio::fs::read_to_string(SESSIONS_FILE)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

async fn save_sessions(sessions: &[SessionEntry]) {
    let _ = tokio::fs::create_dir_all(BRIDGE_DIR).await;
    if let Ok(json) = serde_json::to_string_pretty(sessions) {
        let _ = tokio::fs::write(SESSIONS_FILE, json.as_bytes()).await;
    }
}

// ─── ServerHandler (resources + server info) ─────────────────────────────────

impl ServerHandler for ForgiaMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_instructions(
            "MCP server Forgia. Tools: cargo_check, cargo_clippy, check_stability_locks, list_game_assets, \
             list_genomes, query_genome, search_gene, get_fps_tuning, set_fps_tuning, \
             read_fps_tuning, read_graphics_settings, read_mapgen_config, read_gameplay_state, \
             read_diagnostic_report, read_world_snapshot, \
             read_entities_snapshot, read_chunks_snapshot, read_npcs_snapshot, read_crash_dump, \
             read_input_log, read_perf_history, read_memory_breakdown, read_events_log, read_npcs_extended, \
             read_lag_events, read_watchdog_heartbeat, read_watchdog_alert, read_memory_leaks, read_vram_guardian, read_map_composition, read_ui_state. \
             Resources: forgia://lexique, forgia://context, forgia://ecs-types, forgia://stories-active.",
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        Ok(ListResourcesResult::with_all_items(vec![
            make_resource("forgia://lexique",        "Lexique Forgia — prefabs, params, controles, historique (archive)"),
            make_resource("forgia://context",        "CONTEXT.md — specifications techniques"),
            make_resource("forgia://ecs-types",      "ECS_REFERENCE.md — types ECS du projet"),
            make_resource("forgia://stories-active", "docs/stories/_index.md — toutes les stories"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ReadResourceResult, rmcp::ErrorData> {
        let uri = request.uri.as_str();
        let path = match uri {
            "forgia://lexique"        => forgia_path("docs/archive/lexique.md"),
            "forgia://context"        => forgia_path("CONTEXT.md"),
            "forgia://ecs-types"      => forgia_path("AI_CONTEXT/ECS_REFERENCE.md"),
            "forgia://stories-active" => forgia_path("docs/stories/_index.md"),
            _ => return Err(rmcp::ErrorData::invalid_params(format!("Resource inconnue: {uri}"), None)),
        };
        let text = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| rmcp::ErrorData::internal_error(format!("Lecture {path} echouee: {e}"), None))?;
        Ok(ReadResourceResult::new(vec![ResourceContents::text(text, uri)]))
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn make_resource(uri: &str, description: &str) -> rmcp::model::Resource {
    Annotated {
        raw: RawResource {
            uri: uri.to_string(),
            name: uri.trim_start_matches("forgia://").to_string(),
            title: None,
            description: Some(description.to_string()),
            mime_type: Some("text/plain".to_string()),
            size: None,
            icons: None,
            meta: None,
        },
        annotations: None,
    }
}

/// Read an always-on AI autonomy export file from the workspace root.
/// Returns a friendly error if the file does not exist (game not running, or never loaded).
async fn read_export(filename: &str) -> Result<String, rmcp::ErrorData> {
    let path = forgia_path(filename);
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => Ok(content),
        Err(_) => Ok(format!(
            "{filename} introuvable a {path}. Le jeu n'est pas lance, n'a pas fini le chargement (4s post-load), ou ce fichier n'est pas encore genere par cette version de forgia-game (story-313).",
        )),
    }
}

async fn run_cargo(args: &[&str], cwd: &str) -> Result<String, rmcp::ErrorData> {
    let output = tokio::process::Command::new("cargo")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("cargo introuvable: {e}"), None))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let ok = output.status.success();
    Ok(format!(
        "exit: {} ({})\n{}{}",
        output.status.code().unwrap_or(-1),
        if ok { "OK" } else { "ERREURS" },
        if !stdout.is_empty() { format!("stdout:\n{stdout}\n") } else { String::new() },
        if !stderr.is_empty() { format!("stderr:\n{stderr}") } else { String::new() },
    ))
}

// ─── genome helpers ─────────────────────────────────────────────────────

struct GenomeSummary {
    id: String,
    domain: String,
    category: String,
    gene_count: usize,
    constraint_count: usize,
    extends: Option<String>,
}

async fn scan_genomes(dir: &str) -> Result<Vec<GenomeSummary>, rmcp::ErrorData> {
    let mut summaries = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("read_dir genomes: {e}"), None))?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let doc: toml::Value = match content.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let table = doc.as_table().unwrap();
        summaries.push(GenomeSummary {
            id: table.get("id").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
            domain: table.get("domain").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
            category: table.get("category").and_then(|v| v.as_str()).unwrap_or("?").to_string(),
            gene_count: table.get("genes").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            constraint_count: table.get("constraints").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
            extends: table.get("extends").and_then(|v| v.as_str()).map(|s| s.to_string()),
        });
    }
    summaries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(summaries)
}

// ─── L1 helpers ─────────────────────────────────────────────────────────

async fn find_illegal_asset_loads(src_dir: &str) -> Result<Vec<String>, rmcp::ErrorData> {
    // L1 exceptions autorisees (CLAUDE.md)
    let exceptions = ["audio_registry.rs", "nexus_forge", "vegetation", "village", "scene_cache"];
    let mut violations = Vec::new();
    scan_rs_files(src_dir, &mut violations, &exceptions)
        .await
        .map_err(|e| rmcp::ErrorData::internal_error(format!("Scan echoue: {e}"), None))?;
    Ok(violations)
}

async fn scan_rs_files(
    dir: &str,
    violations: &mut Vec<String>,
    exceptions: &[&str],
) -> anyhow::Result<()> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(scan_rs_files(
                path.to_str().unwrap_or(""),
                violations,
                exceptions,
            ))
            .await?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let path_str = path.to_str().unwrap_or("").replace('\\', "/");
            let is_exception = exceptions.iter().any(|e| path_str.contains(e));
            if is_exception {
                continue;
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                for (i, line) in content.lines().enumerate() {
                    if line.contains("asset_server.load(") {
                        violations.push(format!("{}:{} | {}", path_str, i + 1, line.trim()));
                    }
                }
            }
        }
    }
    Ok(())
}

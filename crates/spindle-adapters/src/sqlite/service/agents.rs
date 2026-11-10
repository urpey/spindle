use super::*;

impl SqliteSpindleService {
    // =========================================================================
    // Agent / config tools (delegate to model_router; no DB work)
    // =========================================================================

    pub fn list_agents(&self) -> Result<ListAgentsOutput> {
        self.repository.model_router().list_agents()
    }

    pub fn agent_config_error(&self) -> Result<Option<String>> {
        self.repository.model_router().config_error()
    }

    pub fn agent_routing_config(&self) -> Result<spindle_core::models::AgentRoutingConfigOutput> {
        self.repository.model_router().routing_config()
    }

    pub fn model_routes(&self) -> Result<Vec<spindle_core::models::ModelRouteSummary>> {
        use crate::ai::adapter_pulls_canon_via_mcp;
        Ok(self
            .repository
            .model_router()
            .list_route_bindings()?
            .into_iter()
            .map(|binding| {
                let caller_should_send_brief =
                    adapter_pulls_canon_via_mcp(&binding.route.adapter_kind);
                spindle_core::models::ModelRouteSummary {
                    route_name: binding.route.route_name,
                    adapter_kind: binding.route.adapter_kind,
                    model_name: binding.route.model_name,
                    purpose: binding.route.purpose,
                    rating: binding.rating,
                    caller_should_send_brief,
                }
            })
            .collect())
    }

    /// Read-only model configuration for the operator console: agents, resolved
    /// routes and the features each route powers. Carries API-key variable
    /// names and set/unset flags only, never key values.
    pub fn console_settings(&self) -> Result<spindle_core::models::ConsoleSettingsOutput> {
        use spindle_core::models::{
            ConsoleAgentSummary, ConsoleRouteSummary, ConsoleSettingsOutput,
        };
        let agents = self.list_agents()?;
        let routing = self.agent_routing_config()?;
        // The public agent summary is frozen (its tool schema has byte budgets),
        // so the console joins the loaded config file for the key variable name.
        let configured = agents
            .source_path
            .as_deref()
            .and_then(|path| crate::agent_config::load_agent_config(Some(path)).ok())
            .map(|loaded| loaded.agents)
            .unwrap_or_default();
        let revision = agents
            .source_path
            .as_deref()
            .map(|path| crate::agent_config::config_revision(std::path::Path::new(path)))
            .transpose()?;
        let routes = self
            .model_routes()?
            .into_iter()
            .map(|route| {
                let agent_id = routing
                    .rules
                    .iter()
                    .find(|rule| rule.route_name == route.route_name && rule.rating == route.rating)
                    .map(|rule| rule.agent_id.clone());
                ConsoleRouteSummary {
                    is_local_stand_in: route.adapter_kind == "local",
                    features: route_feature_keys(&route.route_name)
                        .iter()
                        .map(|key| (*key).to_string())
                        .collect(),
                    route_name: route.route_name,
                    adapter_kind: route.adapter_kind,
                    model_name: route.model_name,
                    purpose: route.purpose,
                    rating: route.rating,
                    agent_id,
                }
            })
            .collect();
        Ok(ConsoleSettingsOutput {
            source_path: agents.source_path,
            revision,
            config_error: agents.config_error,
            health_checks_enabled: agents.health_checks_enabled,
            agents: agents
                .agents
                .into_iter()
                .map(|agent| {
                    let api_key_env = configured
                        .iter()
                        .find(|configured| configured.id == agent.id)
                        .and_then(|configured| configured.api_key_env.clone());
                    let api_key_set = api_key_env
                        .as_deref()
                        .is_some_and(|variable| std::env::var_os(variable).is_some());
                    ConsoleAgentSummary {
                        id: agent.id,
                        name: agent.name,
                        provider: agent.provider,
                        model: agent.model,
                        endpoint: agent.endpoint,
                        ratings: agent.ratings,
                        quality_tier: agent.quality_tier,
                        capabilities: agent.capabilities,
                        notes: agent.notes,
                        status: agent.status,
                        api_key_env,
                        api_key_set,
                        route_names: agent.route_names,
                    }
                })
                .collect(),
            routes,
        })
    }

    pub fn configure_agents(&self, input: ConfigureAgentsInput) -> Result<ConfigureAgentsOutput> {
        self.repository
            .model_router()
            .configure(input.config_path.as_deref())
    }

    pub async fn test_agent(&self, input: TestAgentInput) -> Result<TestAgentOutput> {
        let context = crate::ai::RequestContext {
            project_id: input.project_id.clone(),
            book_id: input.book_id.clone(),
            chapter_id: input.chapter_id.clone(),
            scene_id: input.scene_id.clone(),
            run_id: input.run_id.clone(),
            prompt_template_id: input.prompt_template_id.clone(),
            prompt_version: input.prompt_version.clone(),
            context_packet_hash: input.context_packet_hash.clone(),
        };
        let context = (!context.is_empty()).then_some(context);
        let (mut output, call_id) = self
            .repository
            .model_router()
            .test_agent_with_context_and_call_id(
                &input.agent_id,
                input.test_prompt.as_deref(),
                input.route.as_deref(),
                input.rating.as_deref(),
                context,
            )
            .await?;
        if output.route_name == "draft" && !output.output.trim().is_empty() {
            let receipt = self
                .register_generation_receipt_with_calls(
                    &output.route_name,
                    input.rating.as_deref(),
                    &output.model_name,
                    &output.output,
                    call_id.into_iter().collect(),
                )
                .await?;
            output.generation_id = Some(receipt.id);
            output.generation_agent_id = Some(receipt.agent_id);
            output.generation_output_sha256 = Some(receipt.output_sha256);
        }
        Ok(output)
    }

    pub async fn set_arc_pacing_constraints(
        &self,
        input: SetArcPacingConstraintsInput,
    ) -> Result<SetArcPacingConstraintsOutput> {
        let arc = self
            .repository
            .get_character_arc(&input.character_arc_id)
            .await?;
        if arc.project_id != input.project_id {
            anyhow::bail!("character arc does not belong to the requested project");
        }
        let tracker = self
            .repository
            .get_pacing_tracker_by_arc(&input.character_arc_id)
            .await?;
        let updated = self
            .repository
            .set_arc_pacing_constraints(
                &tracker.id,
                input.per_book_budget,
                input.max_progress_per_chapter,
                input.milestone_spacing,
                input.sprint_allowance,
                input.regression_budget,
            )
            .await?;
        Ok(SetArcPacingConstraintsOutput {
            pacing_tracker_id: updated.id,
        })
    }

    /// UPSERT a character's voice profile + clear voice_drift validator
    /// findings on the active branch + log a session_activity. Mirrors the
    /// SurrealDB service flow, simplified by skipping the
    /// voice_profile_affected_scene_ids helper (which isn't ported) — the
    /// findings clear runs branch-wide for the voice_drift validator instead.
    pub async fn set_character_voice_profile(
        &self,
        input: SetCharacterVoiceProfileInput,
    ) -> Result<SetCharacterVoiceProfileOutput> {
        if input.profile.updated_at.is_some() {
            anyhow::bail!("voice profile updated_at is server-managed and must be omitted");
        }

        let character = self.repository.get_character(&input.character_id).await?;
        if character.project_id != input.project_id {
            anyhow::bail!("character does not belong to the requested project");
        }
        let active_branch_id = self
            .repository
            .active_branch_id_public(&input.project_id)
            .await?;
        // Profiles are keyed only by character_id; inherited writes would
        // mutate the parent and every sibling that reads this character.
        if character.branch_id != active_branch_id {
            anyhow::bail!("character must belong to the active branch");
        }

        // Validate established_in_scene_id if present.
        if let Some(scene_id) = input.profile.established_in_scene_id.as_deref() {
            let scene = self.repository.get_scene(scene_id).await?;
            if scene.project_id != input.project_id {
                anyhow::bail!("established_in_scene_id must belong to the requested project");
            }
            if !self
                .repository
                .branch_inherits_from(&input.project_id, &active_branch_id, &scene.branch_id)
                .await?
            {
                anyhow::bail!("established_in_scene_id must belong to the active branch");
            }
        }

        let stored = self
            .repository
            .set_character_voice_profile(&character.id, &input.profile)
            .await?;
        // Branch-wide voice_drift resolution as a simplification of the
        // SurrealDB voice_profile_affected_scene_ids helper.
        self.resolve_phase_four_caches(
            &input.project_id,
            &active_branch_id,
            &[PhaseFourCacheId::VoiceDrift],
        )
        .await?;
        let _ = self
            .repository
            .append_session_activity(AppendSessionActivityParams {
                project_id: input.project_id.clone(),
                branch_id: active_branch_id.clone(),
                kind: "voice_profile_set".to_string(),
                subject_table: Some("character".to_string()),
                subject_id: Some(character.id.clone()),
                summary: format!("updated voice profile for {}", character.name),
                details_json: Some(serde_json::json!({
                    "character_id": character.id,
                    "character_name": character.name,
                    "tone": stored.tone,
                    "vocabulary_count": stored.vocabulary.len(),
                    "sentence_structure_count": stored.sentence_structure.len(),
                    "tics_count": stored.tics.len(),
                    "forbidden_words_count": stored.forbidden_words.len(),
                    "example_lines_count": stored.example_lines.len(),
                    "established_in_scene_id": stored.established_in_scene_id,
                })),
            })
            .await;

        Ok(SetCharacterVoiceProfileOutput {
            character_id: input.character_id,
            branch_id: active_branch_id,
            profile: CharacterVoiceProfileData {
                tone: stored.tone,
                vocabulary: stored.vocabulary,
                sentence_structure: stored.sentence_structure,
                tics: stored.tics,
                forbidden_words: stored.forbidden_words,
                example_lines: stored.example_lines,
                established_in_scene_id: stored.established_in_scene_id,
                updated_at: stored.updated_at.map(|t| t.to_rfc3339()),
            },
            activity_id: None,
        })
    }
}

/// Catalog keys for the features a model route powers.
fn route_feature_keys(route: &str) -> &'static [&'static str] {
    match route {
        "draft" => &["route.feature.drafting"],
        "review" => &[
            "route.feature.reader_review",
            "route.feature.dual_persona_review",
            "route.feature.replan",
            "route.feature.canon_mining",
        ],
        "research" => &["route.feature.research_recall"],
        "embedding" => &["route.feature.semantic_search"],
        "import_extract" | "import_synthesize" | "import_validate" => {
            &["route.feature.manuscript_import"]
        }
        "style_analyze" | "style_revise" => &["route.feature.style_profile"],
        _ => &[],
    }
}

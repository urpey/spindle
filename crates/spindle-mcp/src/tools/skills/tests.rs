use super::*;

#[tokio::test(flavor = "current_thread")]
async fn list_skills_returns_embedded_skills_and_references() {
    let (router, _tmp_guard) = router().await;

    let result = router
        .call_tool("list_skills", None)
        .await
        .expect("tool call should return result");

    assert_eq!(result.is_error, Some(false));
    let content = result
        .structured_content
        .expect("list_skills should use structured content");
    let skill_names: Vec<&str> = content["skills"]
        .as_array()
        .expect("skills array")
        .iter()
        .filter_map(|skill| skill["name"].as_str())
        .collect();
    for expected in [
        "spindle",
        "scene-writer",
        "character-creator",
        "worldbuilder",
        "authoring-supervisor",
    ] {
        assert!(
            skill_names.contains(&expected),
            "missing skill {expected} in: {skill_names:?}"
        );
    }
    let reference_names: Vec<&str> = content["references"]
        .as_array()
        .expect("references array")
        .iter()
        .filter_map(|reference| reference["name"].as_str())
        .collect();
    assert!(
        reference_names.contains(&"anti-slop"),
        "missing reference anti-slop in: {reference_names:?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn get_skill_returns_markdown_and_unknown_skill_errors() {
    let (router, _tmp_guard) = router().await;
    let mut args = serde_json::Map::new();
    args.insert("name".to_string(), "scene-writer".into());

    let result = router
        .call_tool("get_skill", Some(&args))
        .await
        .expect("tool call should return result");

    assert_eq!(result.is_error, Some(false));
    let content = result
        .structured_content
        .expect("get_skill should use structured content");
    assert_eq!(content["name"].as_str(), Some("scene-writer"));
    assert!(
        content["markdown"]
            .as_str()
            .is_some_and(|markdown| !markdown.is_empty()),
        "markdown should be non-empty"
    );

    args.insert("name".to_string(), "not-a-skill".into());
    let result = router
        .call_tool("get_skill", Some(&args))
        .await
        .expect("tool call should return result");
    assert_eq!(result.is_error, Some(true));
    let text = format!("{:?}", result.content.first().expect("error content"));
    assert!(
        text.contains("unknown skill: not-a-skill"),
        "expected error text in: {text}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn get_reference_returns_markdown_and_unknown_reference_errors() {
    let (router, _tmp_guard) = router().await;
    let mut args = serde_json::Map::new();
    args.insert("name".to_string(), "anti-slop".into());

    let result = router
        .call_tool("get_reference", Some(&args))
        .await
        .expect("tool call should return result");

    assert_eq!(result.is_error, Some(false));
    let content = result
        .structured_content
        .expect("get_reference should use structured content");
    assert_eq!(content["name"].as_str(), Some("anti-slop"));
    assert!(
        content["markdown"]
            .as_str()
            .is_some_and(|markdown| !markdown.is_empty()),
        "markdown should be non-empty"
    );

    args.insert("name".to_string(), "not-a-reference".into());
    let result = router
        .call_tool("get_reference", Some(&args))
        .await
        .expect("tool call should return result");
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test(flavor = "current_thread")]
async fn nested_skill_references_load_through_tools_and_resources() {
    let (router, _tmp_guard) = router().await;
    for name in ["spindle/setup", "editor/style"] {
        let args = serde_json::json!({"name": name});
        let result = structured_json(
            router
                .call_tool("get_reference", args.as_object())
                .await
                .unwrap(),
        );
        assert_eq!(result["name"], name);
        let reference = spindle_adapters::get_reference(name).unwrap();
        assert_eq!(result["markdown"], reference.markdown);
        let resources = crate::resources::ResourceRouter::new(router.service().clone());
        let resource = resources
            .read_resource(&format!("bible://references/{name}"))
            .await
            .unwrap();
        let rmcp::model::ResourceContents::TextResourceContents { text, .. } =
            &resource.contents[0]
        else {
            panic!("expected Markdown text")
        };
        assert_eq!(text, &reference.markdown);
        assert!(
            spindle_adapters::list_references()
                .iter()
                .any(|r| r.name == name)
        );
    }
    for name in ["../spindle/setup", "spindle/../setup", "setup"] {
        let args = serde_json::json!({"name": name});
        let result = router
            .call_tool("get_reference", args.as_object())
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn get_skill_and_get_reference_enforce_profile_coverage_without_mutating_profile() {
    let (mut router, _tmp_guard) = router().await;
    router.tool_profile = Some("workflow-write".to_owned());

    // Covered skill in workflow-write returns full workflow instructions.
    let writer_skill = structured_json(
        router
            .call_tool(
                "get_skill",
                serde_json::json!({"name": "scene-writer"}).as_object(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(writer_skill["covered"], true);
    assert_eq!(writer_skill["required_profile"], "workflow-write");
    assert_eq!(writer_skill["missing_capabilities"], serde_json::json!([]));
    assert_eq!(
        writer_skill["markdown"],
        spindle_adapters::get_skill("scene-writer")
            .unwrap()
            .markdown
    );

    // Covered reference in workflow-write returns full reference markdown.
    let craft_ref = structured_json(
        router
            .call_tool(
                "get_reference",
                serde_json::json!({"name": "scene-writer/craft-and-voice"}).as_object(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(craft_ref["covered"], true);
    assert_eq!(craft_ref["required_profile"], "workflow-write");
    assert_eq!(craft_ref["missing_capabilities"], serde_json::json!([]));
    assert_eq!(
        craft_ref["markdown"],
        spindle_adapters::get_reference("scene-writer/craft-and-voice")
            .unwrap()
            .markdown
    );

    // Uncovered reference in workflow-write returns profile-selection message and metadata.
    let offloaded_ref = structured_json(
        router
            .call_tool(
                "get_reference",
                serde_json::json!({"name": "scene-writer/offloaded-generation"}).as_object(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(offloaded_ref["covered"], false);
    assert_eq!(offloaded_ref["required_profile"], "workflow-authoring");
    let missing: Vec<&str> = offloaded_ref["missing_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        missing,
        vec!["continue_generation", "list_agents", "revise_generation",]
    );
    let available: Vec<&str> = offloaded_ref["available_units"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(available.contains(&"scene-writer"));
    assert!(available.contains(&"scene-writer/craft-and-voice"));
    assert!(!available.contains(&"scene-writer/offloaded-generation"));
    let msg = offloaded_ref["markdown"].as_str().unwrap();
    assert!(msg.contains("SPINDLE_TOOL_PROFILE=workflow-authoring"));
    assert!(
        !msg.contains("## Step 3: Revise Offloaded Output"),
        "uncovered reference must not return actionable offloaded instructions"
    );
    assert_eq!(router.tool_profile.as_deref(), Some("workflow-write"));
    assert!(!router.has_tool("continue_generation"));

    // Minimal profile covers spindle entry and global craft references, but not spindle/setup or scene-writer.
    router.tool_profile = Some("minimal".to_owned());
    let spindle_entry = structured_json(
        router
            .call_tool(
                "get_skill",
                serde_json::json!({"name": "spindle"}).as_object(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(spindle_entry["covered"], true);
    assert_eq!(spindle_entry["required_profile"], "minimal");

    let setup_ref = structured_json(
        router
            .call_tool(
                "get_reference",
                serde_json::json!({"name": "spindle/setup"}).as_object(),
            )
            .await
            .unwrap(),
    );
    assert_eq!(setup_ref["covered"], false);
    assert_eq!(setup_ref["required_profile"], "setup");
    assert_eq!(
        setup_ref["missing_capabilities"],
        serde_json::json!(["create_book", "create_chapter"])
    );
    assert_eq!(router.tool_profile.as_deref(), Some("minimal"));
    assert!(!router.has_tool("create_book"));
}

/// Issue #35: every entry skill names the `story-structure` references for its
/// steps, and **each reference loads in the profile that skill actually runs
/// under** — not only in the unset (full-catalog) profile. This is the
/// regression the first wiring missed: a reference-only profile unit must not
/// inherit the owning core's planning/get_entity tools.
#[tokio::test(flavor = "current_thread")]
async fn entry_skills_load_their_references_in_their_own_profiles() {
    let (mut router, _tmp_guard) = router().await;
    let pointer = regex::Regex::new(r#"get_reference\(name="(story-structure/[a-z-]+)"\)"#)
        .expect("reference pointer regex");

    for (skill_name, profiles) in [
        ("new-book", vec!["new-book"]),
        ("plot-architect", vec!["planning"]),
        ("character-creator", vec!["worldbuilding"]),
        ("scene-writer", vec!["workflow-write", "workflow-authoring"]),
    ] {
        let skill = spindle_adapters::get_skill(skill_name).expect("entry skill");
        let references: Vec<String> = pointer
            .captures_iter(&skill.markdown)
            .map(|capture| capture[1].to_string())
            .collect();
        assert!(
            !references.is_empty(),
            "{skill_name} must name at least one story-structure reference"
        );

        for profile in profiles {
            router.tool_profile = Some(profile.to_string());

            // The skill itself is covered by the profile it runs under.
            let skill_result = structured_json(
                router
                    .call_tool(
                        "get_skill",
                        serde_json::json!({ "name": skill_name }).as_object(),
                    )
                    .await
                    .unwrap(),
            );
            assert_eq!(
                skill_result["covered"], true,
                "{skill_name} must be loadable under {profile}: {skill_result:?}"
            );
            assert_eq!(skill_result["markdown"], skill.markdown);

            for reference in &references {
                let result = structured_json(
                    router
                        .call_tool(
                            "get_reference",
                            serde_json::json!({ "name": reference }).as_object(),
                        )
                        .await
                        .unwrap(),
                );
                assert_eq!(
                    result["covered"], true,
                    "{skill_name} runs under {profile}, so {reference} must load there: {result:?}"
                );
                assert!(
                    result["markdown"]
                        .as_str()
                        .is_some_and(|text| text.len() > 500),
                    "{reference} must return its full guidance"
                );
                assert_eq!(
                    result["markdown"],
                    spindle_adapters::get_reference(reference).unwrap().markdown
                );
            }

            // Reading the owning core is still refused: composing a reference
            // never grants the core skill or its capabilities.
            let core = structured_json(
                router
                    .call_tool(
                        "get_skill",
                        serde_json::json!({ "name": "story-structure" }).as_object(),
                    )
                    .await
                    .unwrap(),
            );
            assert_eq!(
                core["covered"], false,
                "{profile} must not gain the story-structure core: {core:?}"
            );
        }
    }
}

/// The reference-only composition must not leak the owning core's tools into
/// writer or worldbuilding profiles (issue #35 review).
#[test]
fn story_structure_reference_units_do_not_grant_core_tools() {
    use spindle_adapters::guidance::list_profiles;
    let profile = |name: &str| {
        list_profiles()
            .iter()
            .find(|profile| profile.name == name)
            .unwrap_or_else(|| panic!("missing generated profile {name}"))
    };

    for name in ["new-book", "planning"] {
        let tools = profile(name).tools;
        assert!(
            !tools.contains(&"get_entity"),
            "{name} must not gain get_entity from a story-structure reference: {tools:?}"
        );
    }
    for name in ["worldbuilding", "workflow-write", "workflow-authoring"] {
        let tools = profile(name).tools;
        for forbidden in [
            "planning_begin",
            "planning_advance",
            "planning_decide",
            "planning_status",
        ] {
            assert!(
                !tools.contains(&forbidden),
                "{name} must not gain {forbidden} from a story-structure reference: {tools:?}"
            );
        }
    }
    for name in [
        "new-book",
        "planning",
        "worldbuilding",
        "workflow-write",
        "workflow-authoring",
    ] {
        assert!(
            !profile(name).units.contains(&"story-structure"),
            "{name} must compose references only, never the story-structure core"
        );
    }
}

/// ADR 0007: the Swain / MRU reference is paragraph-level rendering craft for a
/// planned McKee Beat, never a competing structural framework.
#[tokio::test(flavor = "current_thread")]
async fn the_swain_reference_is_paragraph_craft_not_a_structure() {
    let reference = spindle_adapters::get_reference("scene-writer/craft-and-voice")
        .expect("craft-and-voice reference");
    let text = reference.markdown;
    assert!(
        text.contains("paragraph-level") && text.contains("McKee"),
        "the reference must frame itself as paragraph-level craft under McKee"
    );
    for structural_promise in [
        "## 1. Swain's Scene / Sequel Framework",
        "SCENE (Action: Goal -> Conflict -> Disaster)",
        "SEQUEL (Reaction: Reaction -> Dilemma -> Decision)",
        "Outline beats",
    ] {
        assert!(
            !text.contains(structural_promise),
            "the reference must not present Swain as a structure: found {structural_promise:?}"
        );
    }
    assert!(
        text.contains("`turning_beat`") && text.contains("`swain_label`"),
        "the reference must point structure at the plan and keep Swain as an optional label"
    );
}

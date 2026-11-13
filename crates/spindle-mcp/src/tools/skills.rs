use super::*;

/// Public entry point for both the MCP tool and the CLI.
/// Writes the Grok skill adapter(s) for Spindle into the target directory
/// (or into ~/.grok/skills/ when `global` is true).
pub fn run_init_grok_skills(
    target_dir: Option<String>,
    global: bool,
) -> anyhow::Result<InitGrokSkillsOutput> {
    use std::fs;
    use std::path::PathBuf;

    let base_skills_dir: PathBuf = if global {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;
        home.join(".grok").join("skills")
    } else {
        let target = match target_dir {
            Some(p) if !p.trim().is_empty() => PathBuf::from(p),
            _ => std::env::current_dir().map_err(|e| anyhow::anyhow!("could not get cwd: {e}"))?,
        };
        target.join(".grok").join("skills")
    };

    fs::create_dir_all(&base_skills_dir)
        .map_err(|e| anyhow::anyhow!("failed to create skills directory: {e}"))?;

    let mut files_written = Vec::new();

    // 1. Write the main meta skill
    let meta_dir = base_skills_dir.join("spindle");
    fs::create_dir_all(&meta_dir)?;
    let meta_path = meta_dir.join("SKILL.md");
    fs::write(&meta_path, SPINDLE_GROK_SKILL_MD)?;
    files_written.push(meta_path.display().to_string());

    // 2. Write thin adapters for every individual Spindle skill
    let spindle_skills = [
        "scene-writer",
        "character-creator",
        "worldbuilder",
        "revision-manager",
        "continuity-editor",
        "editor",
        "manuscript-importer",
        "bible-librarian",
        "plot-architect",
    ];

    for skill in spindle_skills {
        let adapter_name = format!("spindle-{}", skill);
        let adapter_dir = base_skills_dir.join(&adapter_name);
        fs::create_dir_all(&adapter_dir)?;

        let content = generate_spindle_skill_adapter(skill);
        let path = adapter_dir.join("SKILL.md");
        fs::write(&path, content)?;
        files_written.push(path.display().to_string());
    }

    let location = if global {
        "~/.grok/skills/ (global)"
    } else {
        "repo-scoped .grok/skills/"
    };

    Ok(InitGrokSkillsOutput {
        target_dir: if global {
            "~/.grok/skills".to_string()
        } else {
            // best effort
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        },
        files_written,
        message: format!(
            "Grok Spindle skills initialized ({location}). You now have the 'spindle' meta skill plus individual adapters (spindle-scene-writer, spindle-character-creator, etc.). They will be available globally in Grok."
        ),
    })
}

/// Generates a thin Grok adapter for a specific Spindle bible skill.
pub(super) fn generate_spindle_skill_adapter(skill_name: &str) -> String {
    let title = skill_name.replace('-', " ");
    let title = title
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r#"---
name: spindle-{skill_name}
description: Use when doing {title} work inside Spindle book projects. This is the Grok adapter for the official Spindle {skill_name} skill.
---

# Spindle {title} (Grok Adapter)

This is a thin Grok-specific wrapper around Spindle's canonical skill.

**Authoritative instructions (always read these):** `bible://skills/{skill_name}`

## When to activate
- The user asks to write, plan, revise, or manage anything related to **{title}** in a Spindle project.

## Grok-specific notes
- Spindle already provides strong guardrails via the Bible, continuity, and voice profiles.
- Prefer natural, readable, webnovel-style prose.
- For explicit content, always route through the proper `continue_generation` + `rating: "explicit"` path.
- Re-anchor with `get_writer_state` + the relevant briefing tools before major work.

Use the live `bible://skills/{skill_name}` resource for the detailed step-by-step procedure.
"#
    )
}

impl ToolRouter {
    pub(crate) fn covered_units(&self) -> Vec<String> {
        let skills = spindle_adapters::list_skills();
        let mut units = std::collections::BTreeSet::new();
        for skill in skills {
            let required =
                skill_required_tools(skill, skills, &mut std::collections::BTreeSet::new());
            if self.unit_is_covered(skill.name, &required) {
                units.insert(skill.name.to_string());
            }
            for reference in skill.manifest.references {
                let required = reference_required_tools(reference.tools, skill, skills);
                if self.unit_is_covered(reference.name, &required) {
                    units.insert(reference.name.to_string());
                }
            }
        }
        units.into_iter().collect()
    }

    /// Whether the active profile covers `unit`.
    ///
    /// A generated profile declares its units explicitly, so membership is the
    /// gate. That is what lets a writer or worldbuilding profile compose a
    /// `story-structure` reference **without** the owning core: the reference
    /// itself is covered while the core's planning/get_entity tools are not
    /// granted. Compatibility profiles (unset/`legacy-all`, `minimal`, `write`,
    /// `import`, `authoring`) have no composition, so tool availability stays
    /// their rule.
    fn unit_is_covered(&self, unit: &str, required: &std::collections::BTreeSet<&str>) -> bool {
        if let Some(active) = self.tool_profile.as_deref()
            && let Some(generated) = spindle_adapters::guidance::list_profiles()
                .iter()
                .find(|generated| generated.name == active)
        {
            return generated.units.contains(&unit);
        }
        required.iter().all(|tool| self.has_tool(tool))
    }

    pub(super) fn resolve_skill_output(&self, text: spindle_adapters::SkillText) -> GetSkillOutput {
        let skill = text.skill;
        let skills = spindle_adapters::list_skills();
        let required = skill_required_tools(&skill, skills, &mut std::collections::BTreeSet::new());
        let missing_capabilities: Vec<String> = required
            .iter()
            .filter(|tool| !self.has_tool(tool))
            .map(|tool| (*tool).to_string())
            .collect();
        let covered = self.unit_is_covered(skill.name, &required);
        let required_profile = smallest_covering_profile(skill.name, &required);
        let available_units = self.covered_units();
        let markdown = if covered {
            text.markdown
        } else {
            uncovered_unit_message(
                skill.name,
                self.tool_profile.as_deref().unwrap_or("default"),
                required_profile.as_deref().unwrap_or("default"),
                &missing_capabilities,
                &available_units,
            )
        };
        GetSkillOutput {
            name: skill.name.to_string(),
            markdown,
            covered,
            required_profile,
            available_units,
            missing_capabilities,
        }
    }

    pub(super) fn resolve_reference_output(
        &self,
        text: spindle_adapters::ReferenceText,
    ) -> GetReferenceOutput {
        let reference = text.name;
        let required = reference_required_tools_by_name(reference);
        let missing_capabilities: Vec<String> = required
            .iter()
            .filter(|tool| !self.has_tool(tool))
            .map(|tool| (*tool).to_string())
            .collect();
        let covered = self.unit_is_covered(reference, &required);
        let required_profile = smallest_covering_profile(reference, &required);
        let available_units = self.covered_units();
        let markdown = if covered {
            text.markdown
        } else {
            uncovered_unit_message(
                reference,
                self.tool_profile.as_deref().unwrap_or("default"),
                required_profile.as_deref().unwrap_or("default"),
                &missing_capabilities,
                &available_units,
            )
        };
        GetReferenceOutput {
            name: reference.to_string(),
            markdown,
            covered,
            required_profile,
            available_units,
            missing_capabilities,
        }
    }
}

fn skill_required_tools<'a>(
    skill: &'a spindle_adapters::EmbeddedSkill,
    skills: &'a [spindle_adapters::EmbeddedSkill],
    seen: &mut std::collections::BTreeSet<&'a str>,
) -> std::collections::BTreeSet<&'a str> {
    assert!(seen.insert(skill.name), "dependency cycle: {}", skill.name);
    let mut tools: std::collections::BTreeSet<_> = skill.manifest.tools.iter().copied().collect();
    for dependency in skill.manifest.requires_skills {
        if let Some(parent) = skills.iter().find(|s| s.name == *dependency) {
            tools.extend(skill_required_tools(parent, skills, seen));
        }
    }
    seen.remove(skill.name);
    tools
}

/// Tools contributed by a skill's `requires_skills` closure **only**.
///
/// A reference unit applies this closure but not the owning core's own tools, so
/// composing `story-structure/story-spine` never demands the core's
/// `get_entity` / `planning_*` capabilities from the profile.
fn dependency_required_tools<'a>(
    skill: &'a spindle_adapters::EmbeddedSkill,
    skills: &'a [spindle_adapters::EmbeddedSkill],
) -> std::collections::BTreeSet<&'a str> {
    let mut seen = std::collections::BTreeSet::new();
    let mut tools = std::collections::BTreeSet::new();
    for dependency in skill.manifest.requires_skills {
        if let Some(parent) = skills.iter().find(|s| s.name == *dependency) {
            tools.extend(skill_required_tools(parent, skills, &mut seen));
        }
    }
    tools
}

/// What a reference unit needs to load: its own tools plus the owning core's
/// dependency closure (never the owning core's own tools).
fn reference_required_tools<'a>(
    reference_tools: &'a [&'a str],
    owner: &'a spindle_adapters::EmbeddedSkill,
    skills: &'a [spindle_adapters::EmbeddedSkill],
) -> std::collections::BTreeSet<&'a str> {
    let mut required = dependency_required_tools(owner, skills);
    required.extend(reference_tools.iter().copied());
    required
}

fn reference_required_tools_by_name(name: &str) -> std::collections::BTreeSet<&'static str> {
    let skills = spindle_adapters::list_skills();
    for skill in skills {
        for reference in skill.manifest.references {
            if reference.name == name {
                return reference_required_tools(reference.tools, skill, skills);
            }
        }
    }
    std::collections::BTreeSet::new()
}

fn smallest_covering_profile(
    unit_name: &str,
    required_tools: &std::collections::BTreeSet<&str>,
) -> Option<String> {
    // A reference unit is loadable only under a generated profile that declares
    // it, so name the smallest such profile rather than a profile that merely
    // shares tools.
    if unit_name.contains('/') {
        let declared = spindle_adapters::guidance::list_profiles()
            .iter()
            .filter(|profile| profile.units.contains(&unit_name));
        if let Some(profile) = declared.min_by_key(|profile| (profile.tools.len(), profile.name)) {
            return Some(profile.name.to_string());
        }
    }
    if required_tools
        .iter()
        .all(|tool| super::profile_has_tool(Some("minimal"), tool))
    {
        return Some("minimal".to_string());
    }
    spindle_adapters::guidance::list_profiles()
        .iter()
        .filter(|profile| {
            profile.units.contains(&unit_name)
                || required_tools
                    .iter()
                    .all(|tool| profile.tools.contains(tool))
        })
        .min_by_key(|profile| (profile.tools.len(), profile.name))
        .map(|profile| profile.name.to_string())
        .or_else(|| Some("default".to_string()))
}

fn uncovered_unit_message(
    unit_name: &str,
    active_profile: &str,
    required_profile: &str,
    missing_capabilities: &[String],
    available_units: &[String],
) -> String {
    let available = if available_units.is_empty() {
        "none".to_string()
    } else {
        available_units.join(", ")
    };
    format!(
        "# Profile coverage required for `{unit_name}`\n\n\
         The active tool profile (`{active_profile}`) does not cover `{unit_name}`.\n\
         - Required profile: `{required_profile}`\n\
         - Missing capabilities: {}\n\
         - Available units in active profile: {available}\n\n\
         Reconnect with `SPINDLE_TOOL_PROFILE={required_profile}` (or unset `SPINDLE_TOOL_PROFILE` for the full catalog) to load and run this workflow.",
        missing_capabilities.join(", ")
    )
}

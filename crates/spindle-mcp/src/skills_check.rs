//! `spindle-mcp skills check`: validate the skills directory from text alone, so
//! a skill edit gets feedback in seconds instead of a full rebuild.
//!
//! The build already enforces these rules on the compiled tree; this repeats the
//! text-only part against the directory a running server reads, and reports
//! every problem with its file and line instead of stopping at the first one.

use std::collections::BTreeSet;
use std::path::Path;

use spindle_adapters::{EmbeddedSkill, list_references, list_skills};

use crate::tools::{is_operator_only, static_tool_names};

/// One problem found in the skills directory.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Problem {
    /// File the problem is in, as addressed by the check.
    pub file: String,
    /// 1-based line; 1 for a whole-file problem such as a missing file.
    pub line: usize,
    /// Stable machine-readable category.
    pub code: &'static str,
    /// Operator-facing explanation.
    pub message: String,
}

/// Result of checking one skills directory.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Report {
    pub skills_dir: String,
    pub references_dir: String,
    pub files_checked: usize,
    pub problems: Vec<Problem>,
}

impl Report {
    /// `true` when nothing is wrong, which is what the exit code follows.
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Check every compiled skill and reference against `skills_dir` and
/// `references_dir`. Text-only: no database, no model, no rebuild.
pub fn check(skills_dir: &Path, references_dir: &Path) -> Report {
    let mut report = Report {
        skills_dir: skills_dir.display().to_string(),
        references_dir: references_dir.display().to_string(),
        files_checked: 0,
        problems: Vec::new(),
    };
    if !skills_dir.is_dir() {
        report.problems.push(Problem {
            file: report.skills_dir.clone(),
            line: 1,
            code: "missing-directory",
            message: "skills directory not found; point SPINDLE_SKILLS_DIR at it".to_string(),
        });
    }
    let skills = list_skills();
    for skill in skills {
        let declared = tool_closure(skill, skills, &mut BTreeSet::new());
        let skill_file = skills_dir.join(skill.name).join("SKILL.md");
        check_file(&mut report, &skill_file, &declared);
        for reference in skill.manifest.references {
            let mut declared = declared.clone();
            declared.extend(reference.tools.iter().copied());
            let reference_file = skills_dir.join(skill.name).join(reference.markdown_path);
            check_file(&mut report, &reference_file, &declared);
        }
    }
    // Craft references have no manifest of their own: only their files matter.
    let craft_files: Vec<_> = list_references()
        .iter()
        .filter(|reference| reference.skill.is_none())
        .map(|reference| references_dir.join(reference.file))
        .collect();
    for path in craft_files {
        check_file(&mut report, &path, &BTreeSet::new());
    }
    if !references_dir.is_dir() {
        report.problems.push(Problem {
            file: report.references_dir.clone(),
            line: 1,
            code: "missing-directory",
            message: "craft reference directory not found; point SPINDLE_REFERENCES_DIR at it"
                .to_string(),
        });
    }
    report.problems.sort_by(|left, right| {
        (&left.file, left.line, left.code).cmp(&(&right.file, right.line, right.code))
    });
    report.problems.dedup();
    report
}

/// Every tool a unit may name: its own declarations plus its skill's closure.
fn tool_closure<'a>(
    skill: &'a EmbeddedSkill,
    skills: &'a [EmbeddedSkill],
    seen: &mut BTreeSet<&'a str>,
) -> BTreeSet<&'a str> {
    assert!(seen.insert(skill.name), "dependency cycle: {}", skill.name);
    let mut tools: BTreeSet<&'a str> = skill.manifest.tools.iter().copied().collect();
    for dependency in skill.manifest.requires_skills {
        if let Some(parent) = skills
            .iter()
            .find(|candidate| candidate.name == *dependency)
        {
            tools.extend(tool_closure(parent, skills, seen));
        }
    }
    seen.remove(skill.name);
    tools
}

fn check_file(report: &mut Report, path: &Path, declared: &BTreeSet<&str>) {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            report.problems.push(Problem {
                file: path.display().to_string(),
                line: 1,
                code: "missing-file",
                message: format!("{error}; the compiled copy is served instead"),
            });
            return;
        }
    };
    report.files_checked += 1;
    if declared.is_empty() && !path.ends_with("SKILL.md") {
        return; // craft references carry no manifest, so no tool declarations
    }
    let registry = static_tool_names();
    let mut marked: BTreeSet<&str> = BTreeSet::new();
    for (index, line) in text.lines().enumerate() {
        for name in markers_on_line(line) {
            marked.insert(name);
            let line_number = index + 1;
            if !registry.contains(name) {
                report.problems.push(problem(
                    path,
                    line_number,
                    "unknown-tool-marker",
                    format!("`{name}` is not a Spindle tool"),
                ));
            } else if is_operator_only(name) {
                report.problems.push(problem(
                    path,
                    line_number,
                    "operator-tool-marker",
                    format!("`{name}` is operator-only and cannot be a skill dependency"),
                ));
            } else if !declared.contains(name) {
                report.problems.push(problem(
                    path,
                    line_number,
                    "undeclared-tool-marker",
                    format!(
                        "`{name}` is not declared in this unit's manifest, so its profiles hide it"
                    ),
                ));
            }
        }
    }
    for (index, line) in text.lines().enumerate() {
        for name in words_on_line(line)
            .into_iter()
            .filter(|word| registry.contains(*word))
        {
            let line_number = index + 1;
            if !declared.contains(name) {
                report.problems.push(problem(
                    path,
                    line_number,
                    "undeclared-tool-mention",
                    format!("`{name}` is mentioned but not declared in this unit's manifest"),
                ));
            } else if !marked.contains(name) {
                report.problems.push(problem(
                    path,
                    line_number,
                    "missing-tool-marker",
                    format!("`{name}` needs an `<!-- tool: {name} -->` marker"),
                ));
            }
        }
    }
}

fn problem(path: &Path, line: usize, code: &'static str, message: String) -> Problem {
    Problem {
        file: path.display().to_string(),
        line,
        code,
        message,
    }
}

/// `<!-- tool: name -->` markers on one line.
fn markers_on_line(line: &str) -> Vec<&str> {
    const OPEN: &str = "<!--";
    let mut names = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = line[cursor..].find(OPEN) {
        let start = cursor + offset + OPEN.len();
        let rest = line[start..].trim_start();
        if let Some(body) = rest.strip_prefix("tool:")
            && let Some(end) = body.find("-->")
        {
            names.push(body[..end].trim());
        }
        cursor = start;
    }
    names
}

/// Identifier-shaped words on one line, matching the build's prose scanner.
fn words_on_line(line: &str) -> Vec<&str> {
    let bytes = line.as_bytes();
    let mut words = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_lowercase() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_lowercase()
                    || bytes[index].is_ascii_digit()
                    || bytes[index] == b'_')
            {
                index += 1;
            }
            if index - start >= 2 {
                words.push(&line[start..index]);
            }
        } else {
            index += 1;
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn temp_skills(markdown: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        let skill = root.path().join("scene-writer");
        std::fs::create_dir_all(&skill).expect("skill dir");
        std::fs::write(skill.join("SKILL.md"), markdown).expect("write");
        root
    }

    #[test]
    fn the_repo_skills_directory_passes() {
        let root = repo_root();
        let report = check(&root.join("skills"), &root.join("references"));
        assert!(report.ok(), "{:#?}", report.problems);
        assert!(report.files_checked >= 14, "{}", report.files_checked);
    }

    #[test]
    fn an_unknown_tool_marker_is_reported_with_file_and_line() {
        let root = temp_skills("# edited\n\n<!-- tool: continnu_generation -->\n");
        let report = check(root.path(), root.path());
        let problem = report
            .problems
            .iter()
            .find(|problem| problem.code == "unknown-tool-marker")
            .expect("unknown marker problem");
        assert!(
            problem.file.ends_with("scene-writer/SKILL.md"),
            "{}",
            problem.file
        );
        assert_eq!(problem.line, 3);
        assert!(
            problem.message.contains("continnu_generation"),
            "{}",
            problem.message
        );
    }

    #[test]
    fn a_marker_for_an_undeclared_tool_is_reported() {
        let root = temp_skills("<!-- tool: get_model_usage -->\n");
        let report = check(root.path(), root.path());
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.code == "undeclared-tool-marker"
                    && problem.message.contains("get_model_usage")),
            "{:#?}",
            report.problems
        );
    }

    #[test]
    fn a_missing_skill_file_is_reported() {
        let empty = tempfile::tempdir().expect("skills dir");
        let report = check(empty.path(), empty.path());
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.code == "missing-file"
                    && problem.file.ends_with("scene-writer/SKILL.md")),
            "{:#?}",
            report.problems
        );
    }

    #[test]
    fn a_missing_reference_file_is_reported() {
        let repo = repo_root();
        let markdown =
            std::fs::read_to_string(repo.join("skills/scene-writer/SKILL.md")).expect("repo skill");
        let root = temp_skills(&markdown);
        let report = check(root.path(), root.path());
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.code == "missing-file"
                    && problem.file.contains("references/")),
            "{:#?}",
            report.problems
        );
    }

    #[test]
    fn an_undeclared_mention_without_a_marker_is_reported() {
        let root = temp_skills("Call get_model_usage before drafting.\n");
        let report = check(root.path(), root.path());
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.code == "undeclared-tool-mention" && problem.line == 1),
            "{:#?}",
            report.problems
        );
    }

    #[test]
    fn the_report_serializes_for_scripts() {
        let root = temp_skills("<!-- tool: continnu_generation -->\n");
        let report = check(root.path(), root.path());
        let json = serde_json::to_value(&report).expect("json");
        assert!(json["files_checked"].is_number());
        assert!(json["skills_dir"].is_string());
        assert!(
            json["problems"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        assert!(json["problems"][0]["line"].is_number());
        assert!(json["problems"][0]["code"].is_string());
    }
}

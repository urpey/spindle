use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub use spindle_skills::{EmbeddedSkill, GeneratedProfile, list_profiles};
use spindle_skills::{get_skill as embedded_get_skill, list_skills as embedded_list_skills};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedReference {
    pub name: &'static str,
    pub markdown: &'static str,
    /// File path relative to this reference's root directory.
    pub file: &'static str,
    /// `Some(skill)` for a skill's own reference (root is that skill's
    /// directory); `None` for a craft reference (root is the references
    /// directory).
    pub skill: Option<&'static str>,
}

const EMBEDDED_REFERENCES: &[EmbeddedReference] = &[
    EmbeddedReference {
        name: "anti-slop",
        markdown: include_str!("../../../references/anti-slop.md"),
        file: "anti-slop.md",
        skill: None,
    },
    EmbeddedReference {
        name: "mru-guide",
        markdown: include_str!("../../../references/mru-guide.md"),
        file: "mru-guide.md",
        skill: None,
    },
    EmbeddedReference {
        name: "swain-scene-sequel",
        markdown: include_str!("../../../references/swain-scene-sequel.md"),
        file: "swain-scene-sequel.md",
        skill: None,
    },
    EmbeddedReference {
        name: "voice-differentiation",
        markdown: include_str!("../../../references/voice-differentiation.md"),
        file: "voice-differentiation.md",
        skill: None,
    },
];

/// Where a unit's markdown came from for this read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSource {
    /// The text compiled into the binary at build time.
    Embedded,
    /// The text read from the configured skills directory on this call.
    Disk,
}

/// One skill: its compiled metadata plus the markdown read for this call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillText {
    pub skill: EmbeddedSkill,
    pub markdown: String,
    pub source: TextSource,
}

/// One reference: its name plus the markdown read for this call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceText {
    pub name: &'static str,
    pub markdown: String,
    pub source: TextSource,
}

/// What the running server is reading its skill text from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillSourceStatus {
    pub skills_dir: Option<String>,
    pub references_dir: Option<String>,
    pub warnings: Vec<String>,
}

impl SkillSourceStatus {
    /// `true` when markdown is read from disk rather than the compiled copy.
    pub fn reads_from_disk(&self) -> bool {
        self.skills_dir.is_some() || self.references_dir.is_some()
    }
}

/// Markdown reader for the compiled skill set: the names, manifests and
/// profiles stay compiled, only the text is refreshed from disk per read.
pub struct SkillContent {
    skills_dir: Option<PathBuf>,
    references_dir: Option<PathBuf>,
    warnings: Mutex<BTreeSet<String>>,
}

impl SkillContent {
    /// Serve the compiled markdown only (no directory configured).
    pub fn embedded() -> Self {
        Self::at(None, None)
    }

    /// Serve markdown from these directories when present; `None` means the
    /// compiled copy for that root.
    pub fn at(skills_dir: Option<PathBuf>, references_dir: Option<PathBuf>) -> Self {
        Self {
            skills_dir,
            references_dir,
            warnings: Mutex::new(BTreeSet::new()),
        }
    }

    /// Read `SPINDLE_SKILLS_DIR` / `SPINDLE_REFERENCES_DIR`, defaulting to the
    /// repo layout (`skills/`, `references/`) relative to the working directory.
    /// A directory that is not there means the compiled copy, silently when it
    /// is the default and with a recorded warning when it was configured.
    pub fn from_env() -> Self {
        let (skills_dir, skills_warning) = configured_dir(
            "SPINDLE_SKILLS_DIR",
            std::env::var_os("SPINDLE_SKILLS_DIR"),
            "skills",
        );
        let (references_dir, references_warning) = configured_dir(
            "SPINDLE_REFERENCES_DIR",
            std::env::var_os("SPINDLE_REFERENCES_DIR"),
            "references",
        );
        let content = Self::at(skills_dir, references_dir);
        for warning in skills_warning.into_iter().chain(references_warning) {
            content.record(warning);
        }
        content
    }

    /// Read one skill's markdown for this call.
    pub fn read_skill(&self, skill: &EmbeddedSkill) -> SkillText {
        let relative = format!("{}/SKILL.md", skill.name);
        let (markdown, source) =
            self.read_markdown(self.skills_dir.as_deref(), &relative, skill.markdown);
        SkillText {
            skill: *skill,
            markdown,
            source,
        }
    }

    /// Read one reference's markdown for this call.
    pub fn read_reference(&self, reference: &EmbeddedReference) -> ReferenceText {
        let (markdown, source) = match (reference.skill, self.skills_dir.as_ref()) {
            (Some(skill), Some(dir)) => {
                let root = dir.join(skill);
                self.read_markdown(Some(&root), reference.file, reference.markdown)
            }
            (Some(_), None) => (reference.markdown.to_string(), TextSource::Embedded),
            (None, _) => self.read_markdown(
                self.references_dir.as_deref(),
                reference.file,
                reference.markdown,
            ),
        };
        ReferenceText {
            name: reference.name,
            markdown,
            source,
        }
    }

    /// What this reader is reading from, and which files it could not read.
    pub fn status(&self) -> SkillSourceStatus {
        SkillSourceStatus {
            skills_dir: self
                .skills_dir
                .as_ref()
                .map(|dir| dir.display().to_string()),
            references_dir: self
                .references_dir
                .as_ref()
                .map(|dir| dir.display().to_string()),
            warnings: self
                .warnings
                .lock()
                .map(|warnings| warnings.iter().cloned().collect())
                .unwrap_or_default(),
        }
    }

    fn read_markdown(
        &self,
        root: Option<&Path>,
        relative: &str,
        embedded: &'static str,
    ) -> (String, TextSource) {
        let Some(root) = root else {
            return (embedded.to_string(), TextSource::Embedded);
        };
        match read_confined(root, relative) {
            Ok(markdown) => {
                self.clear_warning(&warning_key(root, relative));
                (markdown, TextSource::Disk)
            }
            Err(error) => {
                self.record(format!(
                    "{}: {error}; serving the compiled copy",
                    root.join(relative).display()
                ));
                (embedded.to_string(), TextSource::Embedded)
            }
        }
    }

    fn record(&self, warning: String) {
        let mut warnings = self
            .warnings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if warnings.insert(warning.clone()) {
            tracing::warn!("skill content: {warning}");
        }
    }

    fn clear_warning(&self, key: &str) {
        let mut warnings = self
            .warnings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stale: Vec<String> = warnings
            .iter()
            .filter(|warning| warning.starts_with(key))
            .cloned()
            .collect();
        for warning in stale {
            warnings.remove(&warning);
        }
    }
}

fn warning_key(root: &Path, relative: &str) -> String {
    root.join(relative).display().to_string()
}

/// Resolve a configured directory: present means read from it, absent means the
/// compiled copy. Only an explicitly configured but missing directory warns.
fn configured_dir(
    variable: &str,
    setting: Option<std::ffi::OsString>,
    default: &str,
) -> (Option<PathBuf>, Option<String>) {
    match setting {
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_dir() {
                (Some(path), None)
            } else {
                (
                    None,
                    Some(format!(
                        "{variable}: {} is not a directory; serving the compiled copy",
                        path.display()
                    )),
                )
            }
        }
        None => {
            let path = PathBuf::from(default);
            if path.is_dir() {
                (Some(path), None)
            } else {
                (None, None)
            }
        }
    }
}

/// Read `relative` below `root`, refusing anything that is not a plain file name
/// chain inside it (no `..`, no absolute path, no symlink leaving the root).
fn read_confined(root: &Path, relative: &str) -> std::io::Result<String> {
    let relative = Path::new(relative);
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "path escapes the skills directory",
        ));
    }
    let root = root.canonicalize()?;
    let resolved = root.join(relative).canonicalize()?;
    if !resolved.starts_with(&root) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "path escapes the skills directory",
        ));
    }
    std::fs::read_to_string(resolved)
}

static CONTENT: OnceLock<SkillContent> = OnceLock::new();

/// Set the process-wide content source. Returns `false` when one is already set.
pub fn configure(content: SkillContent) -> bool {
    CONTENT.set(content).is_ok()
}

fn content() -> &'static SkillContent {
    CONTENT.get_or_init(SkillContent::from_env)
}

/// What the running server reads skill markdown from.
pub fn status() -> SkillSourceStatus {
    content().status()
}

pub fn list_skills() -> &'static [EmbeddedSkill] {
    embedded_list_skills()
}

/// Read one skill through the process-wide content source.
pub fn get_skill(name: &str) -> Option<SkillText> {
    embedded_get_skill(name).map(|skill| content().read_skill(&skill))
}

pub fn standards_text() -> String {
    get_skill("scene-writer")
        .map(|skill| skill.markdown)
        .unwrap_or_default()
}

pub fn list_references() -> &'static [EmbeddedReference] {
    static REFERENCES: std::sync::OnceLock<Vec<EmbeddedReference>> = std::sync::OnceLock::new();
    REFERENCES.get_or_init(|| {
        let mut references = EMBEDDED_REFERENCES.to_vec();
        references.extend(embedded_list_skills().iter().flat_map(|skill| {
            skill
                .manifest
                .references
                .iter()
                .map(move |reference| EmbeddedReference {
                    name: reference.name,
                    markdown: reference.markdown,
                    file: reference.markdown_path,
                    skill: Some(skill.name),
                })
        }));
        references
    })
}

pub fn get_reference(name: &str) -> Option<ReferenceText> {
    list_references()
        .iter()
        .find(|reference| reference.name == name)
        .map(|reference| content().read_reference(reference))
}

#[cfg(test)]
mod runtime_markdown_tests {
    use super::*;

    fn edited_skill_dir(marker: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temp dir");
        let skill = root.path().join("scene-writer");
        std::fs::create_dir_all(&skill).expect("skill dir");
        std::fs::write(skill.join("SKILL.md"), format!("# edited\n\n{marker}\n")).expect("write");
        root
    }

    #[test]
    fn disk_markdown_is_read_at_call_time_not_at_build_time() {
        let root = edited_skill_dir("MARKER-ONE");
        let content = SkillContent::at(Some(root.path().to_path_buf()), None);
        let skill = spindle_skills::get_skill("scene-writer").expect("embedded scene-writer");

        let first = content.read_skill(&skill);
        assert!(first.markdown.contains("MARKER-ONE"), "{}", first.markdown);
        assert_eq!(first.source, TextSource::Disk);

        // The edit is visible on the next read, in the same process.
        std::fs::write(
            root.path().join("scene-writer").join("SKILL.md"),
            "# edited\n\nMARKER-TWO\n",
        )
        .expect("rewrite");
        let second = content.read_skill(&skill);
        assert!(
            second.markdown.contains("MARKER-TWO"),
            "{}",
            second.markdown
        );
        assert!(!second.markdown.contains("MARKER-ONE"));
    }

    #[test]
    fn missing_file_falls_back_to_the_compiled_copy_with_a_warning() {
        let empty = tempfile::tempdir().expect("skills dir");
        let content = SkillContent::at(Some(empty.path().to_path_buf()), None);
        let skill = spindle_skills::get_skill("scene-writer").expect("embedded scene-writer");

        let text = content.read_skill(&skill);
        assert_eq!(text.source, TextSource::Embedded);
        assert_eq!(text.markdown, skill.markdown);
        let status = content.status();
        assert!(
            status
                .warnings
                .iter()
                .any(|warning| warning.contains("scene-writer")),
            "{:?}",
            status.warnings
        );
    }

    #[test]
    fn unreadable_file_falls_back_to_the_compiled_copy() {
        let root = tempfile::tempdir().expect("skills dir");
        let skill_dir = root.path().join("scene-writer");
        std::fs::create_dir_all(&skill_dir).expect("skill dir");
        std::fs::write(skill_dir.join("SKILL.md"), [0xff, 0xfe, 0x00]).expect("write");
        let content = SkillContent::at(Some(root.path().to_path_buf()), None);
        let skill = spindle_skills::get_skill("scene-writer").expect("embedded scene-writer");

        let text = content.read_skill(&skill);
        assert_eq!(text.source, TextSource::Embedded);
        assert!(!content.status().warnings.is_empty());
    }

    #[test]
    fn a_warning_clears_once_the_file_reads_again() {
        let root = tempfile::tempdir().expect("skills dir");
        let skill_dir = root.path().join("scene-writer");
        std::fs::create_dir_all(&skill_dir).expect("skill dir");
        let content = SkillContent::at(Some(root.path().to_path_buf()), None);
        let skill = spindle_skills::get_skill("scene-writer").expect("embedded scene-writer");

        assert_eq!(content.read_skill(&skill).source, TextSource::Embedded);
        std::fs::write(skill_dir.join("SKILL.md"), "MARKER-BACK\n").expect("write");
        assert_eq!(content.read_skill(&skill).source, TextSource::Disk);
        assert!(
            content.status().warnings.is_empty(),
            "{:?}",
            content.status()
        );
    }

    #[test]
    fn files_that_are_not_compiled_units_are_ignored() {
        let root = tempfile::tempdir().expect("skills dir");
        let rogue = root.path().join("rogue-skill");
        std::fs::create_dir_all(&rogue).expect("rogue dir");
        std::fs::write(rogue.join("SKILL.md"), "MARKER-ROGUE\n").expect("write");
        std::fs::write(root.path().join("notes.md"), "MARKER-NOTES\n").expect("write");
        let content = SkillContent::at(Some(root.path().to_path_buf()), None);

        // The compiled set decides what exists; a file cannot add a skill.
        assert!(
            list_skills()
                .iter()
                .all(|skill| skill.name != "rogue-skill")
        );
        let skill = spindle_skills::get_skill("scene-writer").expect("embedded scene-writer");
        assert_eq!(content.read_skill(&skill).source, TextSource::Embedded);
        assert_eq!(content.read_skill(&skill).markdown, skill.markdown);
    }

    #[test]
    fn reads_cannot_escape_the_configured_directory() {
        let root = tempfile::tempdir().expect("skills dir");
        let outside = root.path().parent().expect("parent").join("secret.md");
        std::fs::write(&outside, "MARKER-OUTSIDE\n").expect("write");
        let skills = root.path().to_path_buf();

        for relative in [
            "../secret.md",
            "scene-writer/../../secret.md",
            "/etc/passwd",
        ] {
            assert!(
                read_confined(&skills, relative).is_err(),
                "{relative} must be refused"
            );
        }
        assert!(read_confined(&skills, "scene-writer/SKILL.md").is_err());
    }

    #[test]
    fn status_reports_the_configured_directories() {
        let skills = tempfile::tempdir().expect("skills dir");
        let references = tempfile::tempdir().expect("references dir");
        let content = SkillContent::at(
            Some(skills.path().to_path_buf()),
            Some(references.path().to_path_buf()),
        );
        let status = content.status();
        assert!(status.reads_from_disk());
        assert_eq!(
            status.skills_dir.as_deref(),
            Some(skills.path().to_str().unwrap())
        );
        assert_eq!(
            status.references_dir.as_deref(),
            Some(references.path().to_str().unwrap())
        );
        assert!(status.warnings.is_empty());
    }

    #[test]
    fn reference_markdown_is_read_from_disk_too() {
        let skills = tempfile::tempdir().expect("skills dir");
        let references = tempfile::tempdir().expect("references dir");
        let nested = skills.path().join("spindle").join("references");
        std::fs::create_dir_all(&nested).expect("nested dir");
        std::fs::write(nested.join("setup.md"), "MARKER-SKILL-REF\n").expect("write");
        std::fs::write(references.path().join("anti-slop.md"), "MARKER-CRAFT-REF\n")
            .expect("write");
        let content = SkillContent::at(
            Some(skills.path().to_path_buf()),
            Some(references.path().to_path_buf()),
        );

        let skill_reference = content.read_reference(&compiled_reference("spindle/setup"));
        assert!(skill_reference.markdown.contains("MARKER-SKILL-REF"));
        assert_eq!(skill_reference.source, TextSource::Disk);

        let craft_reference = content.read_reference(&compiled_reference("anti-slop"));
        assert!(craft_reference.markdown.contains("MARKER-CRAFT-REF"));
        assert_eq!(craft_reference.source, TextSource::Disk);
    }

    fn compiled_reference(name: &str) -> EmbeddedReference {
        *list_references()
            .iter()
            .find(|reference| reference.name == name)
            .expect("compiled reference")
    }
}

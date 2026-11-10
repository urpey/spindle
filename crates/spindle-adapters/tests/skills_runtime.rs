//! The public skill read path: markdown is read from the configured directory on
//! every call, with the compiled copy as the fallback, so an edit needs no
//! rebuild and no restart. This file is its own test process, which is what
//! makes configuring the process-wide source safe.

use std::fs;

use spindle_adapters::guidance::{SkillContent, TextSource};

#[test]
fn editing_markdown_on_disk_changes_the_next_public_read() {
    let skills = tempfile::tempdir().expect("skills dir");
    let references = tempfile::tempdir().expect("references dir");
    let skill_dir = skills.path().join("scene-writer");
    fs::create_dir_all(&skill_dir).expect("skill dir");
    fs::write(
        skill_dir.join("SKILL.md"),
        "# edited scene writer\n\nMARKER-LIVE\n",
    )
    .expect("write skill");
    let nested = skills.path().join("spindle").join("references");
    fs::create_dir_all(&nested).expect("nested references");
    fs::write(nested.join("setup.md"), "MARKER-SETUP\n").expect("write reference");

    assert!(spindle_adapters::configure_skill_content(SkillContent::at(
        Some(skills.path().to_path_buf()),
        Some(references.path().to_path_buf()),
    )));

    let first = spindle_adapters::get_skill("scene-writer").expect("scene-writer");
    assert!(first.markdown.contains("MARKER-LIVE"), "{}", first.markdown);
    assert_eq!(first.source, TextSource::Disk);

    // The next read sees a later edit: no rebuild, no restart.
    fs::write(
        skill_dir.join("SKILL.md"),
        "# edited again\n\nMARKER-LATER\n",
    )
    .expect("rewrite skill");
    let second = spindle_adapters::get_skill("scene-writer").expect("scene-writer");
    assert!(
        second.markdown.contains("MARKER-LATER"),
        "{}",
        second.markdown
    );

    // References go through the same source, tools and resources alike.
    let reference = spindle_adapters::get_reference("spindle/setup").expect("spindle/setup");
    assert!(
        reference.markdown.contains("MARKER-SETUP"),
        "{}",
        reference.markdown
    );

    // A deleted file falls back to the compiled copy and is reported.
    fs::remove_file(skill_dir.join("SKILL.md")).expect("remove skill");
    let fallback = spindle_adapters::get_skill("scene-writer").expect("scene-writer");
    assert_eq!(fallback.source, TextSource::Embedded);
    assert_eq!(fallback.markdown, fallback.skill.markdown);

    let status = spindle_adapters::skill_content_status();
    assert!(status.reads_from_disk());
    assert!(
        status
            .warnings
            .iter()
            .any(|warning| warning.contains("scene-writer")),
        "{:?}",
        status.warnings
    );
}

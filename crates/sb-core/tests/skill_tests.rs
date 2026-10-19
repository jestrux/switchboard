//! The agent skill ships copies of pages in docs/. Keep them identical.

#[test]
fn skill_reference_pages_match_the_docs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skill = root.join("skills/switchboard");
    assert!(skill.join("SKILL.md").is_file());
    for page in ["projects.md", "setting-up-a-project.md", "theming.md"] {
        let doc = std::fs::read_to_string(root.join("docs").join(page)).unwrap();
        let copy = std::fs::read_to_string(skill.join("reference").join(page)).unwrap_or_default();
        assert!(doc == copy, "skills/switchboard/reference/{page} differs from docs/{page}; run scripts/sync-skill.sh");
    }
}

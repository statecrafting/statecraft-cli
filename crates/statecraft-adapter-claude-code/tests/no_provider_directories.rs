//! Spec 030 sections 3.1 and 3.2, the adapter half: the Claude Code adapter
//! declares no path inside a provider-specific directory in either form, and
//! its one possible path is a root pointer that imports the managed project
//! instructions directly.

use statecraft_adapter_claude_code::environment::{
    MANAGED_INSTRUCTIONS, POINTER, RETIRED_INSTRUCTIONS, UNEXPRESSIBLE, declaration,
    declaration_for, unexpressible_markdown,
};
use statecraft_environment::adapter::{provider_directory, provider_paths};

#[test]
fn neither_form_of_the_declaration_names_a_provider_directory_path() {
    for d in [declaration(), declaration_for(true), declaration_for(false)] {
        assert!(provider_paths(std::slice::from_ref(&d)).is_empty(), "{d:?}");
        assert!(d.paths().all(|p| provider_directory(p).is_none()), "{d:?}");
    }
}

#[test]
fn the_pointer_is_one_root_file_importing_the_managed_instructions() {
    let d = declaration_for(true);
    assert_eq!(d.files.len(), 1, "{d:?}");
    let pointer = &d.files[0];
    assert_eq!(pointer.path, POINTER);
    assert!(!pointer.path.contains('/'), "a root file, not a directory");
    assert!(pointer.pointer, "written only where nothing exists");
    assert_eq!(
        String::from_utf8(pointer.contents.clone()).unwrap(),
        format!("@{MANAGED_INSTRUCTIONS}\n")
    );
    assert_eq!(MANAGED_INSTRUCTIONS, ".statecraft/AGENTS.md");
}

#[test]
fn without_the_pointer_the_adapter_declares_no_path_and_keeps_its_facts() {
    let d = declaration_for(false);
    assert_eq!(d.paths().count(), 0, "{d:?}");
    // The facts this harness cannot express are still stated: they live in
    // the global harness, not in a project file.
    assert_eq!(d.unexpressible.len(), UNEXPRESSIBLE.len());
    assert_eq!(
        unexpressible_markdown().lines().count(),
        UNEXPRESSIBLE.len()
    );
    assert_eq!(d.prerequisites, declaration().prerequisites);
}

#[test]
fn the_file_an_earlier_build_owned_is_the_one_inside_a_provider_directory() {
    assert_eq!(provider_directory(RETIRED_INSTRUCTIONS), Some(".claude"));
    assert!(declaration().paths().all(|p| p != RETIRED_INSTRUCTIONS));
}

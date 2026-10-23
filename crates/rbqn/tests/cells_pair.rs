//! `⋈˘` and `≍˘` on atoms, lists and empties produce the shapes CBQN does (#88).

use std::process::Command;

fn run(expr: &str) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rbqn")).args(["-p", expr]).output().unwrap();
    assert!(out.status.success(), "{expr}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

#[test]
fn pair_cells_of_an_atom_is_a_one_element_list() {
    // Before: an enclosed ⟨ 0 ⟩ (rank 0).
    assert_eq!(run("⋈˘ 0"), "⟨ 0 ⟩");
    assert_eq!(run("≍˘ 'a'"), "\"a\"");
    assert_eq!(run("≢ ⋈˘ 2.5"), "⟨ 1 ⟩");
}

#[test]
fn pair_cells_of_a_list_encloses_each_rank0_cell() {
    // Before: the plain atoms, not enclosed.
    assert_eq!(run("≢ ⋈˘ 1‿2‿3"), "⟨ 3 1 ⟩");
    assert_eq!(run("≡ ⋈˘ 1‿2‿3"), "2");
    assert_eq!(run("(⋈˘ 1‿2‿3) ≡ ⋈˘¨ 1‿2‿3"), "0");
}

#[test]
fn pair_cells_of_an_empty_list_has_shape_0_1() {
    // Before: ⟨⟩.
    assert_eq!(run("≢ ⋈˘ ⟨⟩"), "⟨ 0 1 ⟩");
    assert_eq!(run("≢ ≍˘ ↕0"), "⟨ 0 1 ⟩");
}

#[test]
fn left_cells_of_a_nested_list_is_identity() {
    // Before: an error.
    assert_eq!(run("⊣˘ ⟨1,\"a\",⟨2⟩⟩"), "⟨ 1 \"a\" ⟨ 2 ⟩ ⟩");
}

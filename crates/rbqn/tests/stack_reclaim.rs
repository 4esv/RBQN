//! Stage 2b-i of #124 (#160): temporaries on the VM value stack are freed, and the
//! ownership rules around them (moves, leaks, boxed results) keep every value readable.
//! One test: the live-array counter is process-global.
use rbqn::Bqn;
use rbqn_core::live_arrays;

fn fmt(b: &Bqn, src: &str) -> String {
    b.eval(src).unwrap().fmt()
}

#[test]
fn stack_temporaries_are_reclaimed_and_survivors_stay_readable() {
    let b = Bqn::new().unwrap();

    // The #124 shape: table -> take -> first, each consumed by the next primitive.
    // Compiling the program leaks a fixed amount, so compare two sizes.
    let grown = |n: usize| {
        let before = live_arrays();
        assert_eq!(fmt(&b, &format!("{{𝕊: ⊑ 1↑ +⌜˜ ↕100}}¨ ↕{n} ⋄ 0")), "0");
        live_arrays() - before
    };
    grown(10);
    let (small, big) = (grown(1000), grown(5000));
    // Without reclamation each call leaks 3 arrays (↕100, the table, the take).
    assert!(big - small < 400, "4000 more calls grew live arrays by {}", big - small);

    // Aliasing results move ownership instead of dropping.
    assert_eq!(fmt(&b, "⊢ ↕3"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "1 ⊣ ↕3"), "1");
    assert_eq!(fmt(&b, "(↕3) ⊢ ↕4"), "⟨ 0 1 2 3 ⟩");
    assert_eq!(fmt(&b, "≢ ⊢ ⊢ ↕3"), "⟨ 3 ⟩");
    // Results holding their arguments (boxed) keep them alive.
    assert_eq!(fmt(&b, "⋈˜ ↕2"), "⟨ ⟨ 0 1 ⟩ ⟨ 0 1 ⟩ ⟩");
    assert_eq!(fmt(&b, "⊑ < ↕3"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "⊑ (↕3) ⋈ ↕2"), "⟨ 0 1 2 ⟩");
    // Stored values are leaked, never freed.
    assert_eq!(fmt(&b, "a ← ↕4 ⋄ b ← 1↓a ⋄ a‿b"), "⟨ ⟨ 0 1 2 3 ⟩ ⟨ 1 2 3 ⟩ ⟩");
    assert_eq!(fmt(&b, "{t ← ↕3 ⋄ F ← {t∾𝕩} ⋄ F 3}"), "⟨ 0 1 2 3 ⟩");
    // Errors unwind with Owned slots on the stack.
    assert_eq!(fmt(&b, "{𝕊: (↕3) + ↕4}⎊{𝕊: \"err\"} 0"), "\"err\"");
}

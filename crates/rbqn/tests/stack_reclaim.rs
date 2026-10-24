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

    // Block and function results travel as Owned slots (#161): the result of a block
    // call is consumed by the next primitive and freed. Without it each call leaks one array.
    let grown_blocks = |n: usize| {
        let before = live_arrays();
        assert_eq!(fmt(&b, &format!("{{𝕊: ⊑ {{1↑ +⌜˜ ↕𝕩}} 100}}¨ ↕{n} ⋄ 0")), "0");
        live_arrays() - before
    };
    grown_blocks(10);
    let (small, big) = (grown_blocks(1000), grown_blocks(5000));
    assert!(big - small < 400, "4000 more block calls grew live arrays by {}", big - small);
    let grown_mod = |n: usize| {
        let before = live_arrays();
        assert_eq!(fmt(&b, &format!("_m ← {{𝔽 1↑ +⌜˜ ↕𝕩}} ⋄ {{𝕊: ⊑ ⊢ _m 100}}¨ ↕{n} ⋄ 0")), "0");
        live_arrays() - before
    };
    grown_mod(10);
    let (small, big) = (grown_mod(1000), grown_mod(5000));
    assert!(big - small < 400, "4000 more modifier calls grew live arrays by {}", big - small);
    // Scope slots own their arrays (#161): a local variable and a block argument are
    // freed when the block returns, if nothing captured the scope.
    let grown_local = |n: usize| {
        let before = live_arrays();
        assert_eq!(fmt(&b, &format!("{{𝕊: a ← 1↑ +⌜˜ ↕100 ⋄ b ← a ⋄ ⊑ b}}¨ ↕{n} ⋄ 0")), "0");
        live_arrays() - before
    };
    grown_local(10);
    let (small, big) = (grown_local(1000), grown_local(5000));
    assert!(big - small < 400, "4000 more locals grew live arrays by {}", big - small);
    // The result list holds one array per call; the argument `{𝕩+1}` receives is freed.
    let grown_arg = |n: usize| {
        let before = live_arrays();
        assert_eq!(fmt(&b, &format!("{{𝕊: {{𝕩+1}} 1↑ +⌜˜ ↕100}}¨ ↕{n} ⋄ 0")), "0");
        live_arrays() - before
    };
    grown_arg(10);
    let (small, big) = (grown_arg(1000), grown_arg(5000));
    assert!(big - small < 4400, "4000 more argument calls grew live arrays by {}", big - small);
    // A scope that a closure or namespace captured keeps its slots (leaked, still readable).
    assert_eq!(fmt(&b, "G ← {𝕩 ⋄ a ← ↕3 ⋄ {𝕩 ⋄ a}} ⋄ ({𝕏 0} G 0) ≡ {𝕏 1} G 0"), "1");
    assert_eq!(fmt(&b, "n ← {x ⇐ 1↓↕4 ⋄ y ⇐ x}  ⋄ n.x ≡ n.y"), "1");
    assert_eq!(fmt(&b, "o ← 0 ⋄ {𝕩 ⋄ a ← ↕3 ⋄ {𝕩 ⋄ o ↩ a}0 ⋄ 0}0 ⋄ o"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{𝕩 ⋄ a ← ↕3 ⋄ a}0"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{a ← 𝕩 ⋄ a} ↕3"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{𝕩 ⋄ a ← ↕3 ⋄ ⟨a, a⟩}0"), "⟨ ⟨ 0 1 2 ⟩ ⟨ 0 1 2 ⟩ ⟩");
    assert_eq!(fmt(&b, "v ← {𝕩 ⋄ a ← ↕3 ⋄ a ⋈ a}0 ⋄ v"), "⟨ ⟨ 0 1 2 ⟩ ⟨ 0 1 2 ⟩ ⟩");
    // A returned array is the only reference: reading it after the call stays valid,
    // also when it came through a variable, an argument or a closure.
    assert_eq!(fmt(&b, "{𝕩 ⋄ ↕3} 0"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "(↕2) {𝕨∾𝕩} ↕3"), "⟨ 0 1 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{𝕩 ⋄ a←↕3 ⋄ a} 0"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{𝕩} ↕3"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "{⊢𝕩} {1↓𝕩} ↕4"), "⟨ 1 2 3 ⟩");
    assert_eq!(fmt(&b, "{𝕩 ⋄ t←↕3 ⋄ F ← {t∾𝕩} ⋄ (F 1)≡F 1} 0"), "1");
    assert_eq!(fmt(&b, "≢ {1‿2⥊𝕩} 2↑ ↕5"), "⟨ 1 2 ⟩");

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

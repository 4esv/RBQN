//! Stage 2b of #161: boxed arrays own their elements (`ArrData::Boxed` invariant) and
//! release them when freed. Runs under the quarantine, so a freed element that something
//! still reads panics ("use after free") instead of silently passing. One test: the
//! live-array counter is process-global.
use rbqn::Bqn;
use rbqn_core::live_arrays;

fn fmt(b: &Bqn, src: &str) -> String {
    b.eval(src).unwrap().fmt()
}

#[test]
fn nested_elements_are_owned_released_and_stay_readable() {
    // SAFETY: first statement of the only test in this binary; no other thread exists yet.
    unsafe { std::env::set_var("RBQN_QUARANTINE", "1") };
    let b = Bqn::new().unwrap();
    assert!(rbqn_core::arrstore::quarantine_enabled());

    let growth = |src: &dyn Fn(usize) -> String| {
        let run = |n: usize| {
            let before = live_arrays();
            assert_eq!(fmt(&b, &src(n)), "0");
            live_arrays() - before
        };
        run(10);
        let (small, big) = (run(1000), run(5000));
        big as i64 - small as i64
    };
    // The `¨` result list owns one array per call and frees them with itself.
    let g = growth(&|n| format!("{{𝕊: {{𝕩+1}} 1↑ +⌜˜ ↕100}}¨ ↕{n} ⋄ 0"));
    assert!(g < 400, "result list kept {g} arrays over 4000 calls");
    // An array literal owns its elements: dropped as a statement, they go with it.
    let g = growth(&|n| format!("{{𝕊: ⟨↕100, 1↑ +⌜˜ ↕3⟩ ⋄ 0}}¨ ↕{n} ⋄ 0"));
    assert!(g < 400, "array literal kept {g} arrays over 4000 calls");
    // ... or consumed by a primitive with a numeric result.
    let g = growth(&|n| format!("{{𝕊: ≠ ⟨↕100, 1↑ +⌜˜ ↕3⟩ ⋄ 0}}¨ ↕{n} ⋄ 0"));
    assert!(g < 400, "consumed literal kept {g} arrays over 4000 calls");
    // Enclose and pair adopt their arguments instead of leaking them. The fill prototype
    // they create stays leaked (derived arrays copy fill handles raw): one array per call.
    let g = growth(&|n| format!("{{𝕊: ≠ ⊑ <↕100 ⋄ 0}}¨ ↕{n} ⋄ 0"));
    assert!((3600..4400).contains(&g), "enclose kept {g} arrays over 4000 calls (want only the fill prototype)");
    let g = growth(&|n| format!("{{𝕊: ≠ (↕3) ⋈ ↕100 ⋄ 0}}¨ ↕{n} ⋄ 0"));
    assert!((3600..4400).contains(&g), "pair kept {g} arrays over 4000 calls (want only the fill prototype)");

    // A constant in a literal is shared, not stolen: the literal freed on every iteration
    // must leave the program's own copy of the constant alone.
    assert_eq!(fmt(&b, "{𝕊: ⟨\"abc\", 1⟩ ⋄ 0}¨ ↕3 ⋄ {𝕩 ⋄ ⟨\"abc\", 1⟩}¨ ↕2"), "⟨ ⟨ \"abc\" 1 ⟩ ⟨ \"abc\" 1 ⟩ ⟩");
    // Elements read out of an array outlive it.
    assert_eq!(fmt(&b, "l ← ⟨↕2, ↕3⟩ ⋄ e ← ⊑ l ⋄ l ↩ 0 ⋄ e"), "⟨ 0 1 ⟩");
    assert_eq!(fmt(&b, "e ← ⊑ {𝕩 ⋈ 1}¨ ↕3 ⋄ e"), "⟨ 0 1 ⟩");
    assert_eq!(fmt(&b, "e ← ⊑ 1 ⊑ ⟨↕2, ⟨↕3⟩⟩ ⋄ e"), "⟨ 0 1 2 ⟩");
    assert_eq!(fmt(&b, "e ← ⊑ ⟨⟨↕3⟩⟩ ⋄ ≠ e"), "1");
    // The same element in two arrays and in a variable.
    assert_eq!(fmt(&b, "a ← ↕3 ⋄ l ← ⟨a, a⟩ ⋄ m ← ⟨a, l⟩ ⋄ a ↩ 0 ⋄ l ↩ 0 ⋄ ⌽¨ m"), "┌─                                   \n· ⟨ 2 1 0 ⟩ ⟨ ⟨ 0 1 2 ⟩ ⟨ 0 1 2 ⟩ ⟩  \n                                    ┘");
    assert_eq!(fmt(&b, "a ← ↕3 ⋄ l ← ⟨a, a⟩ ⋄ r ← ⌽ l ⋄ l ↩ 0 ⋄ r"), "⟨ ⟨ 0 1 2 ⟩ ⟨ 0 1 2 ⟩ ⟩");
    // `¨` over a boxed array returning its own elements.
    assert_eq!(fmt(&b, "l ← {𝕩 ⋈ 𝕩}¨ ↕2 ⋄ r ← ⊢¨ l ⋄ l ↩ 0 ⋄ r"), "⟨ ⟨ 0 0 ⟩ ⟨ 1 1 ⟩ ⟩");
    assert_eq!(fmt(&b, "l ← {𝕩 ⋈ 𝕩}¨ ↕2 ⋄ r ← {𝕩}¨ l ⋄ l ↩ 0 ⋄ r"), "⟨ ⟨ 0 0 ⟩ ⟨ 1 1 ⟩ ⟩");
    assert_eq!(fmt(&b, "l ← {↕𝕩}¨ 1+↕3 ⋄ r ← ⌽¨ l ⋄ l ↩ 0 ⋄ r"), "⟨ ⟨ 0 ⟩ ⟨ 1 0 ⟩ ⟨ 2 1 0 ⟩ ⟩");
    // Merge, join, transpose, reverse of boxed arrays.
    assert_eq!(fmt(&b, "≢ > {↕𝕩}¨ 2‿2"), "⟨ 2 2 ⟩");
    assert_eq!(fmt(&b, "≠ ∾ {⟨𝕩⟩}¨ ↕4"), "4");
    assert_eq!(fmt(&b, "∾ ⟨⟨↕2⟩, ⟨<↕3⟩⟩"), "┌─                       \n· ⟨ 0 1 ⟩ ┌·             \n          · ⟨ 0 1 2 ⟩    \n                      ┘  \n                        ┘");
    assert_eq!(fmt(&b, "l ← {⟨𝕩, ↕𝕩⟩}¨ 1+↕2 ⋄ t ← ⌽ l ⋄ l ↩ 0 ⋄ t"), "┌─                           \n· ⟨ 2 ⟨ 0 1 ⟩ ⟩ ⟨ 1 ⟨ 0 ⟩ ⟩  \n                            ┘");
    assert_eq!(fmt(&b, "l ← 2‿2 ⥊ ⟨↕1, ↕2, ↕3, ↕4⟩ ⋄ t ← ⍉ l ⋄ l ↩ 0 ⋄ ≠¨ ⥊ t"), "⟨ 1 3 2 4 ⟩");
    // Closures and namespaces keep the lists they captured.
    assert_eq!(fmt(&b, "F ← {𝕩 ⋄ l ← ⟨↕2, ↕3⟩ ⋄ {𝕩 ⋄ l}} ⋄ c ← F 0 ⋄ c {𝕨 𝕎 𝕩} 0"), "⟨ ⟨ 0 1 ⟩ ⟨ 0 1 2 ⟩ ⟩");
    assert_eq!(fmt(&b, "n ← {l ⇐ ⟨↕2, ↕3⟩} ⋄ n.l"), "⟨ ⟨ 0 1 ⟩ ⟨ 0 1 2 ⟩ ⟩");
    // HashMap values and error results holding lists.
    assert_eq!(fmt(&b, "m ← ⟨\"k\"⟩ •HashMap ⟨⟨↕2, ↕3⟩⟩ ⋄ ≠¨ m.Get \"k\""), "⟨ 2 3 ⟩");
    assert_eq!(fmt(&b, "{𝕩 ⋄ l ← ⟨↕2, ↕3⟩ ⋄ 1+\"a\" ⋄ l}⎊{𝕩 ⋄ ⟨↕4⟩} 0"), "⟨ ⟨ 0 1 ⟩ ⟨ 0 1 2 ⟩ ⟩");
}

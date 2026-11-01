//! #174: trains and modifier applications with stable parts are hash-consed, so
//! re-evaluating them (here: the self-hosted compiler on every `Bqn::eval`) does not
//! allocate a new derived object each time. One test: the derived-object counter is
//! process-global.
use rbqn::Bqn;
use rbqn_vm::derive::derived_allocated;

fn fmt(b: &Bqn, src: &str) -> String {
    b.eval(src).unwrap().fmt()
}

#[test]
fn structural_derived_objects_are_shared() {
    let b = Bqn::new().unwrap();

    // A train and modifier applications evaluated 1000 times inside a block: with
    // sharing they are built once. Compare two sizes so the compile cost cancels.
    let grown = |n: usize| {
        let before = derived_allocated();
        assert_eq!(fmt(&b, &format!("{{𝕊: (- + ×˜∘⌽∘⋈) 𝕩}}¨ ↕{n} ⋄ 0")), "0");
        derived_allocated() - before
    };
    grown(10);
    let (small, big) = (grown(10), grown(1010));
    assert!(big - small < 100, "1000 more calls built {} more derived objects", big - small);

    // Compiling `1` through the self-hosted compiler: 1309 derived objects per call
    // before #174, 684 after (the rest have array operands or fresh closures).
    fmt(&b, "1");
    let before = derived_allocated();
    for _ in 0..20 {
        fmt(&b, "1");
    }
    let per = (derived_allocated() - before) / 20;
    assert!(per < 900, "one eval of `1` built {per} derived objects");

    // Sharing keeps the semantics: equal parts are equal, different parts are not,
    // and an array operand is never a key (a freed array's address can be reused).
    assert_eq!(fmt(&b, "⟨⟨+¨⟩ ≡ ⟨+¨⟩, (⊑⟨+¨⟩) = ⊑⟨+¨⟩⟩"), "⟨ 1 1 ⟩");
    assert_eq!(fmt(&b, "⟨+¨⟩ ≡ ⟨-¨⟩"), "0");
    assert_eq!(fmt(&b, "⟨⟨+-×⟩ ≡ ⟨+-÷⟩, ⟨+-×⟩ ≡ ⟨+-×⟩⟩"), "⟨ 0 1 ⟩");
    assert_eq!(fmt(&b, "{(𝕩⊸+) 1}¨ ⟨⟨1,2⟩, ⟨3,4⟩⟩"), "⟨ ⟨ 2 3 ⟩ ⟨ 4 5 ⟩ ⟩");
    assert_eq!(fmt(&b, "F ← {𝕩+1} ⋄ G ← {𝕩+1} ⋄ ⟨⟨F¨⟩ ≡ ⟨F¨⟩, ⟨F¨⟩ ≡ ⟨G¨⟩⟩"), "⟨ 1 0 ⟩");
}

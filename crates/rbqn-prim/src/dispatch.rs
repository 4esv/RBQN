use rbqn_core::value::{B, BFn, RT_LEN};

pub struct PrimitiveRegistry {
    pub fns: Vec<B>,
    pub names: Vec<&'static str>,
    pub glyphs: Vec<char>,
}

pub const FN_GLYPHS: &str = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!";
pub const MD1_GLYPHS: &str = "˙˜˘¨⌜⁼´˝`";
pub const MD2_GLYPHS: &str = "∘○⊸⟜⌾⊘◶⎉⚇⍟⎊";

impl PrimitiveRegistry {
    pub fn new() -> Self {
        let mut fns = Vec::with_capacity(RT_LEN);
        for i in 0..RT_LEN {
            fns.push(B::Fn(BFn {
                name: format!("(prim {i})"),
                prim_id: Some(i as u8),
                c1: None,
                c2: None,
            }));
        }
        PrimitiveRegistry {
            fns,
            names: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    pub fn get(&self, idx: usize) -> &B {
        &self.fns[idx]
    }
}

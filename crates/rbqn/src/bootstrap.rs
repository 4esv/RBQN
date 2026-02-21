use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_prim::Primitive;

use crate::embedded;

const RT_LEN: usize = 64;

#[allow(dead_code)]
pub struct Runtime {
    pub prims: Vec<Primitive>,
    pub fruntime: Vec<B>,
    pub runtime: Vec<B>,
    pub compiler: B,
    pub formatter: Option<(B, B)>,
    pub glyphs: Vec<Vec<u32>>,
}

// Indices into the 64-element fruntime, matching builtins.h PrimNumbers
#[allow(dead_code)]
mod n {
    pub const ADD: usize = 0;
    pub const SUB: usize = 1;
    pub const MUL: usize = 2;
    pub const DIV: usize = 3;
    pub const POW: usize = 4;
    pub const ROOT: usize = 5;
    pub const FLOOR: usize = 6;
    pub const CEIL: usize = 7;
    pub const STILE: usize = 8;
    pub const NOT: usize = 9;
    pub const AND: usize = 10;
    pub const OR: usize = 11;
    pub const LT: usize = 12;
    pub const GT: usize = 13;
    pub const NE: usize = 14;
    pub const EQ: usize = 15;
    pub const LE: usize = 16;
    pub const GE: usize = 17;
    pub const FEQ: usize = 18;
    pub const FNE: usize = 19;
    pub const LTACK: usize = 20;
    pub const RTACK: usize = 21;
    pub const SHAPE: usize = 22;
    pub const JOIN: usize = 23;
    pub const COUPLE: usize = 24;
    pub const PAIR: usize = 25;
    pub const TAKE: usize = 26;
    pub const DROP: usize = 27;
    pub const UD: usize = 28;
    pub const SHIFTA: usize = 29;
    pub const SHIFTB: usize = 30;
    pub const REVERSE: usize = 31;
    pub const TRANSP: usize = 32;
    pub const SLASH: usize = 33;
    pub const GRADE_UP: usize = 34;
    pub const GRADE_DOWN: usize = 35;
    pub const SELECT: usize = 36;
    pub const PICK: usize = 37;
    pub const INDEX_OF: usize = 38;
    pub const COUNT: usize = 39;
    pub const MEMBER_OF: usize = 40;
    pub const FIND: usize = 41;
    pub const GROUP: usize = 42;
    pub const ASRT: usize = 43;
}

pub fn bootstrap() -> Result<Runtime, BqnError> {
    let prims = rbqn_prim::get_runtime();
    assert_eq!(prims.len(), RT_LEN, "primitive registry must have exactly {RT_LEN} entries");

    // Build fruntime: 64 B values, one per primitive.
    // Each is tagged with the primitive index so the VM can dispatch later.
    // For now, use sentinel values — real dispatch will wire through Derived.
    let fruntime: Vec<B> = (0..RT_LEN).map(|i| B::m_i32(i as i32)).collect();

    // Build glyphs as char arrays
    let fn_glyphs: Vec<u32> = "+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!".chars().map(|c| c as u32).collect();
    let md1_glyphs: Vec<u32> = "˙˜˘¨⌜⁼´˝`".chars().map(|c| c as u32).collect();
    let md2_glyphs: Vec<u32> = "∘○⊸⟜⌾⊘◶⎉⚇⍟⎊".chars().map(|c| c as u32).collect();
    let glyphs = vec![fn_glyphs, md1_glyphs, md2_glyphs];

    // Runtime starts as fruntime; bytecode evaluation will override
    let runtime = fruntime.clone();

    // Compiler and formatter are NYI until bytecode eval is wired
    let compiler = B::SENTINEL;
    let formatter = None;

    // Attempt bytecode bootstrap if embedded bytecode is available
    if !embedded::RUNTIME0.is_empty() {
        eprintln!("rbqn: bytecode bootstrap not yet wired to VM");
    }

    Ok(Runtime {
        prims,
        fruntime,
        runtime,
        compiler,
        formatter,
        glyphs,
    })
}

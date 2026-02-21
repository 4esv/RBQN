use rbqn_core::error::{BqnError, Result};
use rbqn_core::value::{B, RT_LEN};
use rbqn_prim::dispatch::PrimitiveRegistry;

use crate::embedded;

#[allow(dead_code)]
pub struct Runtime {
    pub fruntime: Vec<B>,
    pub runtime: Vec<B>,
    pub compiler: B,
    pub formatter: Option<(B, B)>,
    pub glyphs: B,
    pub def_re: RuntimeEnv,
}

#[allow(dead_code)]
pub struct RuntimeEnv {
    pub comp_fn: B,
    pub rt: B,
    pub glyphs: B,
    pub sys_names: B,
    pub sys_vals: B,
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
    pub const CONST: usize = 44;
    pub const SWAP: usize = 45;
    pub const CELL: usize = 46;
    pub const EACH: usize = 47;
    pub const TBL: usize = 48;
    pub const UNDO: usize = 49;
    pub const FOLD: usize = 50;
    pub const INSERT: usize = 51;
    pub const SCAN: usize = 52;
    pub const ATOP: usize = 53;
    pub const OVER: usize = 54;
    pub const BEFORE: usize = 55;
    pub const AFTER: usize = 56;
    pub const UNDER: usize = 57;
    pub const VAL: usize = 58;
    pub const COND: usize = 59;
    pub const RANK: usize = 60;
    pub const DEPTH: usize = 61;
    pub const REPEAT: usize = 62;
    pub const CATCH: usize = 63;
}

pub fn bootstrap(prims: &PrimitiveRegistry) -> Result<Runtime> {
    // Step 1: Build fruntime — 64 native primitives in PrimNumbers order
    // +-×÷⋆√⌊⌈|¬ ∧∨<>≠=≤≥≡≢ ⊣⊢⥊∾≍⋈↑↓↕« »⌽⍉/⍋⍒⊏⊑⊐⊒ ∊⍷⊔!˙˜˘¨⌜⁼ ´˝`∘○⊸⟜⌾⊘◶ ⎉⚇⍟⎊
    assert_eq!(prims.fns.len(), RT_LEN, "primitive registry must have exactly {RT_LEN} entries");
    let fruntime: Vec<B> = prims.fns.clone();

    // Step 2: Build provide — 23 core primitives + 17 extended for runtime0
    // Indices from load.c:459-464
    let provide = build_provide(&fruntime);

    // Step 3: Evaluate runtime0 bytecode with provide
    // runtime0 produces 24 derived operations:
    //   ⌊ ⌈ | < > ≠ ≥ ⊢ ⊣ ∾ ⋈ ↑ ↓ ⊏ ˙ ˜ ¨ ´ ∘ ○ ⊸ ⟜ ◶ ⍟
    let runtime_0 = if !embedded::RUNTIME0.is_empty() {
        let r0 = eval_precompiled(&embedded::RUNTIME0, &provide)?;
        match r0 {
            B::Arr(items, _) => items,
            _ => return Err(BqnError::Internal("runtime0 must return an array".into())),
        }
    } else {
        build_native_runtime0(&fruntime)
    };

    // Step 4: Evaluate runtime1 bytecode with provide+runtime0
    // Returns [rtObj, setPrims, setInv]
    let runtime = if !embedded::RUNTIME1.is_empty() {
        let mut rt1_arg = provide.clone();
        rt1_arg.extend(runtime_0.iter().cloned());
        let rt_res = eval_precompiled(&embedded::RUNTIME1, &rt1_arg)?;
        process_runtime1(rt_res, &fruntime)?
    } else {
        // Without bytecode, use native primitives directly
        fruntime.clone()
    };

    // Step 5: Build glyphs
    let glyphs = build_glyphs();

    // Step 6: Evaluate compiler bytecode
    let compiler = if !embedded::COMPILER.is_empty() {
        let comp_gen = eval_precompiled(&embedded::COMPILER, &runtime)?;
        // c1(comp_gen, glyphs) → compiler function
        call1(&comp_gen, glyphs.clone())?
    } else {
        B::nothing()
    };

    // Step 7: Evaluate formatter bytecode (optional)
    let formatter = if !embedded::FORMATTER.is_empty() {
        match eval_precompiled(&embedded::FORMATTER, &runtime) {
            Ok(fmt_mod) => {
                // The formatter module expects [type, decompose, glyph, repr]
                let fmt_arg = B::list(vec![
                    fruntime[n::ASRT].clone(), // placeholder for type
                    B::nothing(),              // placeholder for decompose
                    B::nothing(),              // placeholder for glyph
                    B::nothing(),              // placeholder for repr
                ]);
                match call1(&fmt_mod, fmt_arg) {
                    Ok(result) => {
                        if let B::Arr(items, _) = &result {
                            if items.len() >= 2 {
                                Some((items[0].clone(), items[1].clone()))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    } else {
        None
    };

    // Step 8: Assemble runtime environment
    let rt_obj = B::list(runtime.clone());
    let def_re = RuntimeEnv {
        comp_fn: compiler.clone(),
        rt: rt_obj,
        glyphs: glyphs.clone(),
        sys_names: B::empty_list(),
        sys_vals: B::empty_list(),
    };

    Ok(Runtime {
        fruntime,
        runtime,
        compiler,
        formatter,
        glyphs,
        def_re,
    })
}

fn build_provide(fruntime: &[B]) -> Vec<B> {
    // From load.c:459-463, provide is built from bi_* globals:
    // bi_type, bi_fill, bi_log, bi_grLen, bi_grOrd, bi_asrt,
    // bi_add, bi_sub, bi_mul, bi_div, bi_pow, bi_floor,
    // bi_eq, bi_le, bi_fne, bi_shape, bi_pick, bi_ud,
    // bi_tbl, bi_scan, bi_fillBy, bi_val, bi_catch,
    // (extended:)
    // bi_root, bi_not, bi_and, bi_or, bi_feq, bi_couple,
    // bi_shifta, bi_shiftb, bi_reverse, bi_transp,
    // bi_gradeUp, bi_gradeDown, bi_indexOf, bi_count,
    // bi_memberOf, bi_cell, bi_rank

    let mut provide = Vec::new();

    // Core provide (23 entries)
    // type (system fn) — provide a stub
    provide.push(B::Fn(rbqn_core::value::BFn {
        name: "•Type".into(),
        prim_id: None,
        c1: None,
        c2: None,
    }));
    // fill (system fn)
    provide.push(B::Fn(rbqn_core::value::BFn {
        name: "•FillFn".into(),
        prim_id: None,
        c1: None,
        c2: None,
    }));
    // log (⋆⁼)
    provide.push(B::Fn(rbqn_core::value::BFn {
        name: "⋆⁼".into(),
        prim_id: None,
        c1: None,
        c2: None,
    }));
    // grLen (•GroupLen)
    provide.push(B::Fn(rbqn_core::value::BFn {
        name: "•GroupLen".into(),
        prim_id: None,
        c1: None,
        c2: None,
    }));
    // grOrd (•GroupOrd)
    provide.push(B::Fn(rbqn_core::value::BFn {
        name: "•GroupOrd".into(),
        prim_id: None,
        c1: None,
        c2: None,
    }));
    provide.push(fruntime[n::ASRT].clone());     // !
    provide.push(fruntime[n::ADD].clone());       // +
    provide.push(fruntime[n::SUB].clone());       // -
    provide.push(fruntime[n::MUL].clone());       // ×
    provide.push(fruntime[n::DIV].clone());       // ÷
    provide.push(fruntime[n::POW].clone());       // ⋆
    provide.push(fruntime[n::FLOOR].clone());     // ⌊
    provide.push(fruntime[n::EQ].clone());        // =
    provide.push(fruntime[n::LE].clone());        // ≤
    provide.push(fruntime[n::FNE].clone());       // ≢
    provide.push(fruntime[n::SHAPE].clone());     // ⥊
    provide.push(fruntime[n::PICK].clone());      // ⊑
    provide.push(fruntime[n::UD].clone());        // ↕
    provide.push(fruntime[n::TBL].clone());       // ⌜
    provide.push(fruntime[n::SCAN].clone());      // `
    // fillBy (•_fillBy_)
    provide.push(B::Md2(rbqn_core::value::BMd2 {
        name: "•_fillBy_".into(),
        prim_id: None,
    }));
    provide.push(fruntime[n::VAL].clone());       // ⊘
    provide.push(fruntime[n::CATCH].clone());     // ⎊

    // Extended provide (17 entries) from load.c:462-463
    provide.push(fruntime[n::ROOT].clone());       // √
    provide.push(fruntime[n::NOT].clone());        // ¬
    provide.push(fruntime[n::AND].clone());        // ∧
    provide.push(fruntime[n::OR].clone());         // ∨
    provide.push(fruntime[n::FEQ].clone());        // ≡
    provide.push(fruntime[n::COUPLE].clone());     // ≍
    provide.push(fruntime[n::SHIFTA].clone());     // «
    provide.push(fruntime[n::SHIFTB].clone());     // »
    provide.push(fruntime[n::REVERSE].clone());    // ⌽
    provide.push(fruntime[n::TRANSP].clone());     // ⍉
    provide.push(fruntime[n::GRADE_UP].clone());   // ⍋
    provide.push(fruntime[n::GRADE_DOWN].clone()); // ⍒
    provide.push(fruntime[n::INDEX_OF].clone());   // ⊐
    provide.push(fruntime[n::COUNT].clone());      // ⊒
    provide.push(fruntime[n::MEMBER_OF].clone());  // ∊
    provide.push(fruntime[n::CELL].clone());       // ˘
    provide.push(fruntime[n::RANK].clone());       // ⎉

    provide
}

fn build_native_runtime0(fruntime: &[B]) -> Vec<B> {
    // When ALL_R0=0 (no precompiled runtime0), CBQN uses native implementations
    // for these 24 derived operations (load.c:467):
    // ⌊ ⌈ | < > ≠ ≥ ⊢ ⊣ ∾ ⋈ ↑ ↓ ⊏ ˙ ˜ ¨ ´ ∘ ○ ⊸ ⟜ ◶ ⍟
    vec![
        fruntime[n::FLOOR].clone(),     // ⌊
        fruntime[n::CEIL].clone(),      // ⌈
        fruntime[n::STILE].clone(),     // |
        fruntime[n::LT].clone(),       // <
        fruntime[n::GT].clone(),       // >
        fruntime[n::NE].clone(),       // ≠
        fruntime[n::GE].clone(),       // ≥
        fruntime[n::RTACK].clone(),    // ⊢
        fruntime[n::LTACK].clone(),    // ⊣
        fruntime[n::JOIN].clone(),     // ∾
        fruntime[n::PAIR].clone(),     // ⋈
        fruntime[n::TAKE].clone(),     // ↑
        fruntime[n::DROP].clone(),     // ↓
        fruntime[n::SELECT].clone(),   // ⊏
        fruntime[n::CONST].clone(),    // ˙
        fruntime[n::SWAP].clone(),     // ˜
        fruntime[n::EACH].clone(),     // ¨
        fruntime[n::FOLD].clone(),     // ´
        fruntime[n::ATOP].clone(),     // ∘
        fruntime[n::OVER].clone(),     // ○
        fruntime[n::BEFORE].clone(),   // ⊸
        fruntime[n::AFTER].clone(),    // ⟜
        fruntime[n::COND].clone(),     // ◶
        fruntime[n::REPEAT].clone(),   // ⍟
    ]
}

fn build_glyphs() -> B {
    // From load.c:573
    // ⟨fn_glyphs, md1_glyphs, md2_glyphs⟩
    let fn_glyphs = B::c32_vec("+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!");
    let md1_glyphs = B::c32_vec("˙˜˘¨⌜⁼´˝`");
    let md2_glyphs = B::c32_vec("∘○⊸⟜⌾⊘◶⎉⚇⍟⎊");
    B::list(vec![fn_glyphs, md1_glyphs, md2_glyphs])
}

fn eval_precompiled(bc: &embedded::EmbeddedBytecode, _provide: &[B]) -> Result<B> {
    if bc.is_empty() {
        return Err(BqnError::Internal("empty bytecode".into()));
    }
    // TODO: Wire to VM once rbqn-vm is implemented
    // This will:
    // 1. Convert bc.bc/bc.objs/bc.blocks/bc.bodies into CompiledBlock
    // 2. Build a scope with provide as the argument
    // 3. Execute the block and return the result
    Err(BqnError::Nyi("bytecode evaluation not yet implemented".into()))
}

fn process_runtime1(_rt_res: B, fruntime: &[B]) -> Result<Vec<B>> {
    // runtime1 returns [rtObj, setPrims, setInv]
    // rtObj has RT_LEN elements
    // setPrims(⟨decompose, primInd⟩) wires up primitive decomposition
    // setInv(invSwap, invReg) wires up inverse system

    // For now, fall back to native
    Ok(fruntime.to_vec())
}

fn call1(_f: &B, _x: B) -> Result<B> {
    // TODO: Wire to VM
    Err(BqnError::Nyi("call1 not yet implemented".into()))
}

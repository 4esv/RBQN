use rbqn_core::{B, BqnArr, Result};

pub type MonadFn = fn(B, Option<&BqnArr>) -> Result<PrimResult>;
pub type DyadFn = fn(B, Option<&BqnArr>, B, Option<&BqnArr>) -> Result<PrimResult>;

#[derive(Debug)]
pub enum PrimResult {
    Scalar(B),
    Array(BqnArr),
}

impl PrimResult {
    pub fn to_b(&self) -> B {
        match self {
            PrimResult::Scalar(b) => *b,
            PrimResult::Array(_) => {
                // In the full implementation this would tag an allocated array.
                // For now, return a sentinel indicating array result.
                B::SENTINEL
            }
        }
    }

    pub fn into_arr(self) -> Option<BqnArr> {
        match self {
            PrimResult::Array(a) => Some(a),
            PrimResult::Scalar(_) => None,
        }
    }
}

pub struct Primitive {
    pub name: &'static str,
    pub glyph: &'static str,
    pub c1: Option<MonadFn>,
    pub c2: Option<DyadFn>,
}

pub fn get_runtime() -> Vec<Primitive> {
    use crate::arith_dyad;
    use crate::arith_monad;
    use crate::compare;
    use crate::group;
    use crate::search;
    use crate::select;
    use crate::slash;
    use crate::sort;
    use crate::structural;
    use crate::sysfn;

    // RT_LEN = 64 in order: +-×÷⋆√⌊⌈|¬ ∧∨<>≠=≤≥≡≢ ⊣⊢⥊∾≍⋈↑↓↕« »⌽⍉/⍋⍒⊏⊑⊐⊒ ∊⍷⊔!˙˜˘¨⌜⁼ ´˝`∘○⊸⟜⌾⊘◶ ⎉⚇⍟⎊
    vec![
        // Row 0: +-×÷⋆√⌊⌈|¬
        Primitive { name: "add",   glyph: "+", c1: Some(arith_monad::add_c1),   c2: Some(arith_dyad::add_c2) },
        Primitive { name: "sub",   glyph: "-", c1: Some(arith_monad::sub_c1),   c2: Some(arith_dyad::sub_c2) },
        Primitive { name: "mul",   glyph: "×", c1: Some(arith_monad::mul_c1),   c2: Some(arith_dyad::mul_c2) },
        Primitive { name: "div",   glyph: "÷", c1: Some(arith_monad::div_c1),   c2: Some(arith_dyad::div_c2) },
        Primitive { name: "pow",   glyph: "⋆", c1: Some(arith_monad::pow_c1),   c2: Some(arith_dyad::pow_c2) },
        Primitive { name: "root",  glyph: "√", c1: Some(arith_monad::root_c1),  c2: Some(arith_dyad::root_c2) },
        Primitive { name: "floor", glyph: "⌊", c1: Some(arith_monad::floor_c1), c2: Some(arith_dyad::floor_c2) },
        Primitive { name: "ceil",  glyph: "⌈", c1: Some(arith_monad::ceil_c1),  c2: Some(arith_dyad::ceil_c2) },
        Primitive { name: "stile", glyph: "|", c1: Some(arith_monad::stile_c1), c2: Some(arith_dyad::stile_c2) },
        Primitive { name: "not",   glyph: "¬", c1: Some(arith_monad::not_c1),   c2: Some(arith_dyad::not_c2) },

        // Row 1: ∧∨<>≠=≤≥≡≢
        Primitive { name: "and",   glyph: "∧", c1: Some(sort::grade_up_c1),    c2: Some(arith_dyad::and_c2) },
        Primitive { name: "or",    glyph: "∨", c1: Some(sort::grade_down_c1),  c2: Some(arith_dyad::or_c2) },
        Primitive { name: "lt",    glyph: "<", c1: Some(structural::enclose_c1), c2: Some(compare::lt_c2) },
        Primitive { name: "gt",    glyph: ">", c1: Some(structural::merge_c1),   c2: Some(compare::gt_c2) },
        Primitive { name: "ne",    glyph: "≠", c1: Some(structural::length_c1),  c2: Some(compare::ne_c2) },
        Primitive { name: "eq",    glyph: "=", c1: Some(structural::rank_c1),    c2: Some(compare::eq_c2) },
        Primitive { name: "le",    glyph: "≤", c1: None,                         c2: Some(compare::le_c2) },
        Primitive { name: "ge",    glyph: "≥", c1: None,                         c2: Some(compare::ge_c2) },
        Primitive { name: "feq",   glyph: "≡", c1: Some(structural::depth_c1),   c2: Some(compare::feq_c2) },
        Primitive { name: "fne",   glyph: "≢", c1: Some(structural::shape_c1),   c2: Some(compare::fne_c2) },

        // Row 2: ⊣⊢⥊∾≍⋈↑↓↕«
        Primitive { name: "ltack",   glyph: "⊣", c1: Some(structural::identity_c1), c2: Some(structural::ltack_c2) },
        Primitive { name: "rtack",   glyph: "⊢", c1: Some(structural::identity_c1), c2: Some(structural::rtack_c2) },
        Primitive { name: "shape",   glyph: "⥊", c1: Some(structural::deshape_c1),  c2: Some(structural::reshape_c2) },
        Primitive { name: "join",    glyph: "∾", c1: Some(structural::join_c1),      c2: Some(structural::join_to_c2) },
        Primitive { name: "couple",  glyph: "≍", c1: Some(structural::solo_c1),      c2: Some(structural::couple_c2) },
        Primitive { name: "pair",    glyph: "⋈", c1: Some(structural::solo_c1),      c2: Some(structural::pair_c2) },
        Primitive { name: "take",    glyph: "↑", c1: Some(structural::prefixes_c1),  c2: Some(structural::take_c2) },
        Primitive { name: "drop",    glyph: "↓", c1: Some(structural::suffixes_c1),  c2: Some(structural::drop_c2) },
        Primitive { name: "ud",      glyph: "↕", c1: Some(structural::range_c1),     c2: Some(structural::windows_c2) },
        Primitive { name: "shifta",  glyph: "«", c1: None,                           c2: Some(structural::shifta_c2) },

        // Row 3: »⌽⍉/⍋⍒⊏⊑⊐⊒
        Primitive { name: "shiftb",    glyph: "»", c1: None,                          c2: Some(structural::shiftb_c2) },
        Primitive { name: "reverse",   glyph: "⌽", c1: Some(structural::reverse_c1),  c2: Some(structural::rotate_c2) },
        Primitive { name: "transp",    glyph: "⍉", c1: Some(structural::transpose_c1),c2: Some(structural::reorder_c2) },
        Primitive { name: "slash",     glyph: "/", c1: Some(slash::indices_c1),        c2: Some(slash::replicate_c2) },
        Primitive { name: "gradeUp",   glyph: "⍋", c1: Some(sort::grade_up_c1),       c2: Some(sort::bins_up_c2) },
        Primitive { name: "gradeDown", glyph: "⍒", c1: Some(sort::grade_down_c1),     c2: Some(sort::bins_down_c2) },
        Primitive { name: "select",    glyph: "⊏", c1: Some(select::first_cell_c1),   c2: Some(select::select_c2) },
        Primitive { name: "pick",      glyph: "⊑", c1: Some(select::first_c1),        c2: Some(select::pick_c2) },
        Primitive { name: "indexOf",   glyph: "⊐", c1: Some(search::self_indexOf_c1), c2: Some(search::indexOf_c2) },
        Primitive { name: "count",     glyph: "⊒", c1: Some(search::self_count_c1),   c2: Some(search::count_c2) },

        // Row 4: ∊⍷⊔!˙˜˘¨⌜⁼
        Primitive { name: "memberOf", glyph: "∊", c1: Some(search::mark_firsts_c1), c2: Some(search::member_of_c2) },
        Primitive { name: "find",     glyph: "⍷", c1: Some(search::deduplicate_c1), c2: Some(search::find_c2) },
        Primitive { name: "group",    glyph: "⊔", c1: Some(group::group_indices_c1),c2: Some(group::group_c2) },
        Primitive { name: "asrt",     glyph: "!", c1: Some(sysfn::assert_c1),        c2: Some(sysfn::assert_msg_c2) },
        Primitive { name: "const",    glyph: "˙", c1: None, c2: None },
        Primitive { name: "swap",     glyph: "˜", c1: None, c2: None },
        Primitive { name: "cell",     glyph: "˘", c1: None, c2: None },
        Primitive { name: "each",     glyph: "¨", c1: None, c2: None },
        Primitive { name: "tbl",      glyph: "⌜", c1: None, c2: None },
        Primitive { name: "undo",     glyph: "⁼", c1: None, c2: None },

        // Row 5: ´˝`∘○⊸⟜⌾⊘◶
        Primitive { name: "fold",    glyph: "´", c1: None, c2: None },
        Primitive { name: "insert",  glyph: "˝", c1: None, c2: None },
        Primitive { name: "scan",    glyph: "`", c1: None, c2: None },
        Primitive { name: "atop",    glyph: "∘", c1: None, c2: None },
        Primitive { name: "over",    glyph: "○", c1: None, c2: None },
        Primitive { name: "before",  glyph: "⊸", c1: None, c2: None },
        Primitive { name: "after",   glyph: "⟜", c1: None, c2: None },
        Primitive { name: "under",   glyph: "⌾", c1: None, c2: None },
        Primitive { name: "val",     glyph: "⊘", c1: None, c2: None },
        Primitive { name: "cond",    glyph: "◶", c1: None, c2: None },

        // Row 6: ⎉⚇⍟⎊
        Primitive { name: "rank",   glyph: "⎉", c1: None, c2: None },
        Primitive { name: "depth",  glyph: "⚇", c1: None, c2: None },
        Primitive { name: "repeat", glyph: "⍟", c1: None, c2: None },
        Primitive { name: "catch",  glyph: "⎊", c1: None, c2: None },
    ]
}

// Provide-23 bootstrap primitives (indices into the provide array in load.c)
pub const PROVIDE_NAMES: &[&str] = &[
    "type", "fill", "log", "grLen", "grOrd", "asrt",
    "add", "sub", "mul", "div", "pow", "floor",
    "eq", "le", "fne", "shape", "pick", "ud",
    "tbl", "scan", "fillBy", "val", "catch",
    // Extended provide (14 more):
    "root", "not", "and", "or", "feq", "couple",
    "shifta", "shiftb", "reverse", "transp",
    "gradeUp", "gradeDown", "indexOf", "count",
    "memberOf", "cell", "rank",
];

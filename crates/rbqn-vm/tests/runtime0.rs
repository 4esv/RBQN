// Integration tests verifying runtime0 core functions work correctly.
// These tests directly exercise derive::c1/c2 with native primitives,
// verifying the two bugs fixed in plan 01-01:
//   1. Fork monadic dispatch: (f g h) x = (f x) g (h x)
//   2. Each/scan/cells return typed numeric arrays, not Boxed

use rbqn_core::{B, BqnArr};
use rbqn_vm::derive::{c1, c2, m1_d, prim_to_b, m_fork};
use rbqn_vm::vm::{get_arr, tag_arr};

// Primitive indices (matching bootstrap.rs fruntime layout)
const IDX_ADD: usize = 0;    // +
const IDX_SUB: usize = 1;    // -
const IDX_FLOOR: usize = 6;  // ⌊
const IDX_ID_R: usize = 21;  // ⊢ (right identity)
const IDX_FOLD: usize = 50;  // ´
const IDX_EACH: usize = 47;  // ¨
const IDX_SCAN: usize = 52;  // `

fn make_f64_arr(vals: &[f64]) -> B {
    tag_arr(BqnArr::new_vec_f64(vals.to_vec()))
}

/// Helper: verify result is a numeric scalar with given value
fn assert_num(result: B, expected: f64, name: &str) {
    assert!(result.is_f64(), "{name}: result should be numeric, got {result:?}");
    let got = result.o2f();
    assert!(
        (got - expected).abs() < 1e-10,
        "{name}: expected {expected}, got {got}"
    );
}

/// Helper: verify result is a non-Boxed numeric array with given values
fn assert_numeric_arr(result: B, expected: &[f64], name: &str) {
    let arr = get_arr(result).unwrap_or_else(|| panic!("{name}: result is not an array"));
    assert!(
        !matches!(arr.data, rbqn_core::ArrData::Boxed(_)),
        "{name}: result should be typed numeric array, not Boxed. data={:?}", arr.data
    );
    let vals = arr.f64_iter()
        .unwrap_or_else(|e| panic!("{name}: cannot extract f64 values: {e}"));
    assert_eq!(vals.len(), expected.len(), "{name}: length mismatch");
    for (i, (&got, &exp)) in vals.iter().zip(expected.iter()).enumerate() {
        assert!(
            (got - exp).abs() < 1e-10,
            "{name}[{i}]: expected {exp}, got {got}"
        );
    }
}

// ============================================================
// Bug 1: Fork monadic dispatch
// ============================================================

#[test]
fn runtime0_fork_monadic_identity() {
    // (⊢ + ⊢) 3 = (⊢ 3) + (⊢ 3) = 3 + 3 = 6
    let fn_id = prim_to_b(IDX_ID_R);    // ⊢
    let fn_add = prim_to_b(IDX_ADD);    // +
    let fork = m_fork(fn_id, fn_add, fn_id);
    let x = B::m_f64(3.0);
    let result = c1(fork, x);
    assert_num(result, 6.0, "(⊢ + ⊢) 3");
}

#[test]
fn runtime0_fork_monadic_floor_add() {
    // (⌊ + ⊢) 3.7 = (⌊ 3.7) + (⊢ 3.7) = 3 + 3.7 = 6.7
    let fn_floor = prim_to_b(IDX_FLOOR); // ⌊
    let fn_add = prim_to_b(IDX_ADD);     // +
    let fn_id = prim_to_b(IDX_ID_R);    // ⊢
    let fork = m_fork(fn_floor, fn_add, fn_id);
    let x = B::m_f64(3.7);
    let result = c1(fork, x);
    assert_num(result, 6.7, "(⌊ + ⊢) 3.7");
}

// ============================================================
// Bug 2: +´ fold returns correct result
// ============================================================

#[test]
fn runtime0_fold_add() {
    // +´ ⟨1, 2, 3⟩ = 6
    let fn_add = prim_to_b(IDX_ADD);
    let md_fold = prim_to_b(IDX_FOLD);
    let fold_add = m1_d(md_fold, fn_add);

    let x = make_f64_arr(&[1.0, 2.0, 3.0]);
    let result = c1(fold_add, x);
    assert_num(result, 6.0, "+´ ⟨1,2,3⟩");
}

#[test]
fn runtime0_fold_add_longer() {
    // +´ ⟨1, 2, 3, 4, 5⟩ = 15
    let fn_add = prim_to_b(IDX_ADD);
    let md_fold = prim_to_b(IDX_FOLD);
    let fold_add = m1_d(md_fold, fn_add);

    let x = make_f64_arr(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let result = c1(fold_add, x);
    assert_num(result, 15.0, "+´ ⟨1,2,3,4,5⟩");
}

#[test]
fn runtime0_fold_sub() {
    // -´ ⟨5, 3, 1⟩ = 5 - (3 - 1) = 5 - 2 = 3
    let fn_sub = prim_to_b(IDX_SUB);
    let md_fold = prim_to_b(IDX_FOLD);
    let fold_sub = m1_d(md_fold, fn_sub);

    let x = make_f64_arr(&[5.0, 3.0, 1.0]);
    let result = c1(fold_sub, x);
    assert_num(result, 3.0, "-´ ⟨5,3,1⟩");
}

// ============================================================
// Bug 2: +¨ each returns typed numeric array
// ============================================================

#[test]
fn runtime0_each_add_monadic_typed() {
    // +¨ ⟨1, 2, 3⟩ = ⟨1, 2, 3⟩ (monadic + is identity)
    // Result must be a typed numeric array, not Boxed
    let fn_add = prim_to_b(IDX_ADD);
    let md_each = prim_to_b(IDX_EACH);
    let each_add = m1_d(md_each, fn_add);

    let x = make_f64_arr(&[1.0, 2.0, 3.0]);
    let result = c1(each_add, x);
    assert_numeric_arr(result, &[1.0, 2.0, 3.0], "+¨ ⟨1,2,3⟩");
}

#[test]
fn runtime0_each_floor_typed() {
    // ⌊¨ ⟨1.5, 2.7, 3.1⟩ = ⟨1, 2, 3⟩
    let fn_floor = prim_to_b(IDX_FLOOR);
    let md_each = prim_to_b(IDX_EACH);
    let each_floor = m1_d(md_each, fn_floor);

    let x = make_f64_arr(&[1.5, 2.7, 3.1]);
    let result = c1(each_floor, x);
    assert_numeric_arr(result, &[1.0, 2.0, 3.0], "⌊¨ ⟨1.5,2.7,3.1⟩");
}

#[test]
fn runtime0_each_add_dyadic_typed() {
    // ⟨1,2,3⟩ +¨ ⟨10,20,30⟩ = ⟨11,22,33⟩
    let fn_add = prim_to_b(IDX_ADD);
    let md_each = prim_to_b(IDX_EACH);
    let each_add = m1_d(md_each, fn_add);

    let w = make_f64_arr(&[1.0, 2.0, 3.0]);
    let x = make_f64_arr(&[10.0, 20.0, 30.0]);
    let result = c2(each_add, w, x);
    assert_numeric_arr(result, &[11.0, 22.0, 33.0], "⟨1,2,3⟩ +¨ ⟨10,20,30⟩");
}

// ============================================================
// Scan returns typed numeric array
// ============================================================

#[test]
fn runtime0_scan_add_typed() {
    // +` ⟨1, 2, 3, 4⟩ = ⟨1, 3, 6, 10⟩ (running sums)
    let fn_add = prim_to_b(IDX_ADD);
    let md_scan = prim_to_b(IDX_SCAN);
    let scan_add = m1_d(md_scan, fn_add);

    let x = make_f64_arr(&[1.0, 2.0, 3.0, 4.0]);
    let result = c1(scan_add, x);
    assert_numeric_arr(result, &[1.0, 3.0, 6.0, 10.0], "+` ⟨1,2,3,4⟩");
}

// ============================================================
// Basic primitives that should still work (regression guard)
// ============================================================

#[test]
fn runtime0_identity_right() {
    // ⊢ 42 = 42
    let fn_id = prim_to_b(IDX_ID_R);
    let result = c1(fn_id, B::m_f64(42.0));
    assert_num(result, 42.0, "⊢ 42");
}

#[test]
fn runtime0_floor_scalar() {
    // ⌊ 3.7 = 3
    let fn_floor = prim_to_b(IDX_FLOOR);
    let result = c1(fn_floor, B::m_f64(3.7));
    assert_num(result, 3.0, "⌊ 3.7");
}

#[test]
fn runtime0_add_dyadic() {
    // 3 + 4 = 7
    let fn_add = prim_to_b(IDX_ADD);
    let result = c2(fn_add, B::m_f64(3.0), B::m_f64(4.0));
    assert_num(result, 7.0, "3 + 4");
}

// ============================================================
// Fork dyadic dispatch (should already work, regression check)
// ============================================================

#[test]
fn runtime0_fork_dyadic() {
    // w (⊢ + ⊢) x = (w ⊢ x) + (w ⊢ x) = x + x = 2*x
    // With ⊢ dyadic = right argument, so fork(⊢, +, ⊢) w x = (w⊢x) + (w⊢x) = x+x
    let fn_id = prim_to_b(IDX_ID_R);
    let fn_add = prim_to_b(IDX_ADD);
    let fork = m_fork(fn_id, fn_add, fn_id);
    let w = B::m_f64(10.0);
    let x = B::m_f64(5.0);
    let result = c2(fork, w, x);
    // ⊢ dyadic returns right arg, so (w⊢x)=x=5, (w⊢x)=x=5, 5+5=10
    assert_num(result, 10.0, "w (⊢ + ⊢) x = x+x");
}

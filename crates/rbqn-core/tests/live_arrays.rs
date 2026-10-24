//! The live-array counter is process-global, so these checks run in one test
//! (own process, sequential) instead of racing the unit tests.
use rbqn_core::{get_arr, live_arrays, tag_arr, tag_arr_owned, BqnArr, Val, B};
use std::sync::Arc;

fn nums(n: usize) -> BqnArr {
    BqnArr::from_b_vec((0..n).map(B::m_usz).collect())
}

#[test]
fn counter_tracks_owned_handles() {
    let base = live_arrays();

    // frees exactly when the last Val drops
    let v = tag_arr_owned(nums(2));
    assert_eq!(live_arrays(), base + 1);
    let w = v.clone();
    drop(v);
    assert_eq!(live_arrays(), base + 1, "one count left");
    assert_eq!(get_arr(w.b()).unwrap().shape, vec![2], "still readable");
    drop(w);
    assert_eq!(live_arrays(), base);

    // leak() keeps it alive; get_arr stays valid
    let b = tag_arr_owned(nums(3)).leak();
    assert_eq!(live_arrays(), base + 1);
    assert_eq!(get_arr(b).unwrap().shape, vec![3]);
    // SAFETY: `b` was leaked from a Val above and is not read after this drop.
    drop(unsafe { Val::from_b_owned(b) });
    assert_eq!(live_arrays(), base);

    // plain tag_arr is never freed
    let _ = tag_arr(nums(1));
    assert_eq!(live_arrays(), base + 1);

    // a transient Arc outliving the last Val: the Val is not last, no decrement,
    // and the array is freed later by the Arc (counter stays high: conservative)
    let u = tag_arr_owned(nums(2));
    let held = get_arr(u.b()).unwrap();
    drop(u);
    assert_eq!(live_arrays(), base + 2);
    assert_eq!(Arc::strong_count(&held), 1);
}

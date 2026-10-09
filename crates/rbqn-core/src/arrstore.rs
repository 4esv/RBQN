use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use crate::array::BqnArr;
use crate::value::{B, ARR_TAG, tagu64};

static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);

static ARR_STORE: std::sync::LazyLock<Mutex<HashMap<u64, Arc<BqnArr>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn tag_arr(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ARR_STORE.lock().unwrap().insert(id, Arc::new(arr));
    tagu64(id << 3, ARR_TAG)
}

/// Tag an array as a merge target (ARMM opcode).
/// Uses bit 0 of the payload to distinguish from plain arrays (LSTM).
/// v_set checks this bit to destructure along the first axis (major cells).
pub fn tag_arr_merge(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ARR_STORE.lock().unwrap().insert(id, Arc::new(arr));
    tagu64((id << 3) | 1, ARR_TAG)
}

/// Check if a B value is an array tagged as a merge target (ARMM).
pub fn is_arr_merge(b: B) -> bool {
    b.is_arr() && (b.0 & 0x7) == 1
}

/// Returns a shared handle; clone the inner BqnArr explicitly to mutate.
pub fn get_arr(b: B) -> Option<Arc<BqnArr>> {
    if !b.is_arr() {
        return None;
    }
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    ARR_STORE.lock().unwrap().get(&id).cloned()
}

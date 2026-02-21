use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use crate::array::BqnArr;
use crate::value::{B, ARR_TAG, tagu64};

static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);

static ARR_STORE: std::sync::LazyLock<Mutex<HashMap<u64, BqnArr>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn tag_arr(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    ARR_STORE.lock().unwrap().insert(id, arr);
    tagu64(id << 3, ARR_TAG)
}

pub fn get_arr(b: B) -> Option<BqnArr> {
    if !b.is_arr() {
        return None;
    }
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    ARR_STORE.lock().unwrap().get(&id).cloned()
}

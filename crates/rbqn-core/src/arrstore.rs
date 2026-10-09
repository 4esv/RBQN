use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use crate::array::BqnArr;
use crate::value::{B, ARR_TAG, tagu64};

/// Hasher for sequential u64 ids: one multiply (Fibonacci hashing) instead of SipHash.
/// The multiply spreads ids into the high bits hashbrown uses for its control bytes.
#[derive(Default, Clone, Copy)]
pub struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(8) ^ b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = n.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

pub type IdMap<V> = HashMap<u64, V, std::hash::BuildHasherDefault<IdHasher>>;

static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);

static ARR_STORE: std::sync::LazyLock<Mutex<IdMap<Arc<BqnArr>>>> =
    std::sync::LazyLock::new(|| Mutex::new(IdMap::default()));

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

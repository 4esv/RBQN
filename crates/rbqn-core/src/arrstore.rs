use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use crate::array::BqnArr;
use crate::value::{B, ARR_TAG, tagu64};

/// Fast hasher for u64/u128 keys: a splitmix64 finalizer instead of SipHash.
/// Mixes every input bit into both the low (bucket) and high (control byte) bits,
/// so it works for sequential ids and for f64 bit patterns alike.
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
        self.0 = mix64(self.0 ^ n);
    }
    fn write_u128(&mut self, n: u128) {
        self.0 = mix64(mix64(self.0 ^ n as u64) ^ (n >> 64) as u64);
    }
}

#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub type IdMap<V> = HashMap<u64, V, std::hash::BuildHasherDefault<IdHasher>>;

static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);

// NOTE: Stores are thread-local: the interpreter runs on one thread (no spawn, no
// rayon, GPU and FFI calls are synchronous on the caller's thread). Every borrow
// below is a single insert or get+Arc clone that runs no user code and cannot
// panic, so catch_unwind (⎊) can never observe a live borrow.
// ManuallyDrop: values are never freed today, so skip a teardown walk at exit.
std::thread_local! {
    static ARR_STORE: std::mem::ManuallyDrop<std::cell::RefCell<IdMap<Arc<BqnArr>>>> = std::mem::ManuallyDrop::new(std::cell::RefCell::new(IdMap::default()));
}

pub fn tag_arr(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let arr = Arc::new(arr);
    ARR_STORE.with(|s| s.borrow_mut().insert(id, arr));
    tagu64(id << 3, ARR_TAG)
}

/// Tag an array as a merge target (ARMM opcode).
/// Uses bit 0 of the payload to distinguish from plain arrays (LSTM).
/// v_set checks this bit to destructure along the first axis (major cells).
pub fn tag_arr_merge(arr: BqnArr) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let arr = Arc::new(arr);
    ARR_STORE.with(|s| s.borrow_mut().insert(id, arr));
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
    ARR_STORE.with(|s| s.borrow().get(&id).cloned())
}

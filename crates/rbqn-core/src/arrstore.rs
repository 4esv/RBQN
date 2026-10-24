use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

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

/// Ids for device-tagged values only (PENDING / RESIDENT / DEVICE_HOST keys).
/// Host arrays carry an `Arc` pointer instead of an id.
static ARR_COUNTER: AtomicU64 = AtomicU64::new(1);

// Handle layout (payload of ARR_TAG, 48 bits):
//   host array:   `Arc::into_raw(..)` address, 8-byte aligned; bit 0 = merge flag, bit 1 = 0.
//   device value: `(id << 3) | 2`; id keys the thread-local tables below.
// A host handle is valid on every thread: `get_arr` needs no table.
// NOTE: the strong count taken by `tag_arr` is never released, so arrays are still
// never freed (#124) and an address is never reused while a `B` can name it.
//
// NOTE: The device tables are thread-local: device values (PENDING, RESIDENT and
// their host-side cache DEVICE_HOST) do not cross threads yet; GPU and FFI calls
// run synchronously on the interpreter thread. Every borrow is a single insert or
// get+Arc clone that runs no user code and cannot panic, so catch_unwind (⎊) can
// never observe a live borrow.
std::thread_local! {
    static DEVICE_HOST: std::cell::RefCell<IdMap<Arc<BqnArr>>> = std::cell::RefCell::new(IdMap::default());
}

const PTR_MASK: u64 = 0xFFFF_FFFF_FFF8;

// A host handle stores the allocation address, so the array must be Send + Sync
// for a `B` to be readable on any thread.
const _: fn() = || {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BqnArr>();
};

/// Element count from which a freshly tagged array means "this program does
/// array work": the first such array fires the hook registered with
/// `register_large_arr_hook` (once per process). The GPU runtime uses it to
/// start device creation only in programs that may need a device.
pub const LARGE_ARR_HINT: usize = 1 << 16;
static LARGE_ARR_HOOK: OnceLock<fn()> = OnceLock::new();
static LARGE_ARR_FIRED: AtomicBool = AtomicBool::new(false);

pub fn register_large_arr_hook(f: fn()) {
    let _ = LARGE_ARR_HOOK.set(f);
}

#[inline]
fn large_arr_hint(ia: usize) {
    if ia >= LARGE_ARR_HINT
        && let Some(f) = LARGE_ARR_HOOK.get()
        && !LARGE_ARR_FIRED.swap(true, Ordering::Relaxed)
    {
        f();
    }
}

#[inline]
fn tag_host(arr: BqnArr, flags: u64) -> B {
    large_arr_hint(arr.ia());
    let addr = Arc::into_raw(Arc::new(arr)).expose_provenance() as u64;
    assert!(
        addr & !PTR_MASK == 0,
        "array allocation {addr:#x} is not 8-byte aligned or does not fit in 48 bits"
    );
    tagu64(addr | flags, ARR_TAG)
}

pub fn tag_arr(arr: BqnArr) -> B {
    tag_host(arr, 0)
}

/// Tag an array as a merge target (ARMM opcode).
/// Uses bit 0 of the payload to distinguish from plain arrays (LSTM).
/// v_set checks this bit to destructure along the first axis (major cells).
pub fn tag_arr_merge(arr: BqnArr) -> B {
    tag_host(arr, 1)
}

/// Check if a B value is an array tagged as a merge target (ARMM).
pub fn is_arr_merge(b: B) -> bool {
    b.is_arr() && (b.0 & 0x7) == 1
}

/// Returns a shared handle; clone the inner BqnArr explicitly to mutate.
/// A device-tagged value still pending on the GPU is materialized here (one
/// readback), cached in DEVICE_HOST, and dropped from PENDING.
pub fn get_arr(b: B) -> Option<Arc<BqnArr>> {
    if !b.is_arr() {
        return None;
    }
    if b.0 & 2 == 0 {
        let ptr = std::ptr::with_exposed_provenance::<BqnArr>((b.0 & PTR_MASK) as usize);
        // SAFETY: a host handle is built only by `tag_host` from `Arc::into_raw`, and
        // that strong count is never released, so the allocation is live and `ptr`
        // carries its (exposed) provenance. The increment gives the returned Arc its
        // own count, which `from_raw` then adopts.
        return Some(unsafe {
            Arc::increment_strong_count(ptr);
            Arc::from_raw(ptr)
        });
    }
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    if let Some(a) = DEVICE_HOST.with(|s| s.borrow().get(&id).cloned()) {
        return Some(a);
    }
    if let Some(r) = peek_resident(b) {
        return Some(download_resident(id, &r));
    }
    // NOTE: materialize outside any borrow: it runs GPU code.
    let dv = PENDING.with(|p| p.borrow().get(&id).cloned())?;
    let arr = Arc::new(dv.materialize());
    DEVICE_HOST.with(|s| s.borrow_mut().insert(id, arr.clone()));
    PENDING.with(|p| p.borrow_mut().remove(&id));
    dv.materialized(b.0);
    Some(arr)
}

/// A lazily materialized array living off-host (GPU). Shape and fill are
/// host-side so shape queries never force a readback.
pub trait DeviceValue: std::any::Any {
    fn shape(&self) -> &[usize];
    fn fill(&self) -> Option<B>;
    /// Read back and build the host array (called at most once per value).
    fn materialize(&self) -> BqnArr;
    fn as_any(&self) -> &dyn std::any::Any;
    /// Called once after `materialize`, with the value's `B` bits.
    fn materialized(&self, _bits: u64) {}
}

std::thread_local! {
    static PENDING: std::cell::RefCell<IdMap<Arc<dyn DeviceValue>>> = std::cell::RefCell::new(IdMap::default());
}

/// Register a pending device value. Payload bit 1 marks it device-tagged.
pub fn tag_device(v: Arc<dyn DeviceValue>) -> B {
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    PENDING.with(|p| p.borrow_mut().insert(id, v));
    tagu64((id << 3) | 2, ARR_TAG)
}

/// Bit test only: true if `b` was created by `tag_device` (pending or not).
#[inline]
pub fn is_device_tagged(b: B) -> bool {
    b.is_arr() && (b.0 & 2) != 0
}

/// The pending device value behind `b`, without materializing. None once materialized.
pub fn peek_device(b: B) -> Option<Arc<dyn DeviceValue>> {
    if !is_device_tagged(b) {
        return None;
    }
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    PENDING.with(|p| p.borrow().get(&id).cloned())
}

// Device-resident arrays (•gpu.Tensor and •gpu results): a BqnArr holding
// ArrData::Gpu, tagged like a pending device value (payload bit 1). The entry
// stays after the first download, so GPU consumers keep using the buffer while
// host consumers read the copy cached in DEVICE_HOST.
std::thread_local! {
    static RESIDENT: std::cell::RefCell<IdMap<Arc<BqnArr>>> = std::cell::RefCell::new(IdMap::default());
}
static RESIDENT_DOWNLOAD_HOOK: OnceLock<fn(&BqnArr) -> BqnArr> = OnceLock::new();

/// Set by the GPU runtime: read a resident array back into host data.
pub fn register_resident_download(f: fn(&BqnArr) -> BqnArr) {
    let _ = RESIDENT_DOWNLOAD_HOOK.set(f);
}

/// Register a resident array (`arr.data` is `ArrData::Gpu`).
pub fn tag_resident(arr: BqnArr) -> B {
    debug_assert!(matches!(arr.data, crate::array::ArrData::Gpu(_)));
    let id = ARR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    RESIDENT.with(|r| r.borrow_mut().insert(id, Arc::new(arr)));
    tagu64((id << 3) | 2, ARR_TAG)
}

/// The resident array behind `b` (data still `ArrData::Gpu`), without downloading.
pub fn peek_resident(b: B) -> Option<Arc<BqnArr>> {
    if !is_device_tagged(b) {
        return None;
    }
    let id = (b.0 & 0xFFFFFFFFFFFF) >> 3;
    RESIDENT.with(|r| r.borrow().get(&id).cloned())
}

/// First host touch of a resident value: one download, cached under the same id.
fn download_resident(id: u64, r: &BqnArr) -> Arc<BqnArr> {
    let f = RESIDENT_DOWNLOAD_HOOK.get().expect("resident array without a GPU runtime");
    // NOTE: download outside any borrow: it runs GPU code.
    let arr = Arc::new(f(r));
    DEVICE_HOST.with(|s| s.borrow_mut().insert(id, arr.clone()));
    arr
}

/// Shape of an array value without forcing a pending device value.
pub fn arr_shape(b: B) -> Option<Vec<usize>> {
    if let Some(d) = peek_device(b) {
        return Some(d.shape().to_vec());
    }
    if let Some(r) = peek_resident(b) {
        return Some(r.shape.clone());
    }
    get_arr(b).map(|a| a.shape.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nums(n: usize) -> BqnArr {
        BqnArr::from_b_vec((0..n).map(B::m_usz).collect())
    }

    #[test]
    fn handle_is_aligned_pointer_and_flags_survive() {
        let a = tag_arr(nums(3));
        let m = tag_arr_merge(nums(2));
        assert!(a.is_arr() && m.is_arr());
        assert!(!is_arr_merge(a) && is_arr_merge(m));
        assert!(!is_device_tagged(a) && !is_device_tagged(m));
        assert_eq!(get_arr(m).unwrap().shape, vec![2]);
        assert_eq!(arr_shape(a), Some(vec![3]));
    }

    #[test]
    fn same_handle_same_allocation() {
        let a = tag_arr(nums(4));
        let x = get_arr(a).unwrap();
        let y = get_arr(a).unwrap();
        assert!(Arc::ptr_eq(&x, &y));
    }

    // The property slice 3 of #81 needs: a B made on one thread is readable on another.
    #[test]
    fn b_readable_on_another_thread() {
        let inner = tag_arr(nums(3));
        let outer = tag_arr(BqnArr::from_b_vec(vec![inner, B::m_f64(7.0)]));
        let bits = outer.0;
        let (shape, firsts, nested) = std::thread::spawn(move || {
            let o = B(bits);
            let arr = get_arr(o).expect("get_arr on a worker thread");
            let n = get_arr(arr.get(0).unwrap()).expect("nested array");
            (
                arr.shape.clone(),
                arr.get(1).unwrap().o2f(),
                (n.shape.clone(), (0..n.ia()).map(|i| n.get(i).unwrap().o2f()).collect::<Vec<_>>()),
            )
        })
        .join()
        .unwrap();
        assert_eq!(shape, vec![2]);
        assert_eq!(firsts, 7.0);
        assert_eq!(nested, (vec![3], vec![0.0, 1.0, 2.0]));
        // and the spawning thread still sees the same data
        assert_eq!(get_arr(outer).unwrap().shape, vec![2]);
    }
}

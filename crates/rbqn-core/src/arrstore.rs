use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
// NOTE: the strong count taken by `tag_arr` is released only by dropping a `Val` that
// owns it (#145, stage 1). Every existing caller holds a plain `B` and so never frees
// (#124): an address is never reused while a `B` can name it, unless its owner dropped
// a `Val` after proving the array unreachable.
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

/// Host arrays currently allocated through `tag_*` (created minus freed by `Val`).
static LIVE_ARRAYS: AtomicUsize = AtomicUsize::new(0);

/// Number of host arrays tagged and not yet freed. Arrays held as plain `B` are
/// never freed, so this only goes down when a `Val` drops the last strong count.
pub fn live_arrays() -> usize {
    LIVE_ARRAYS.load(Ordering::Relaxed)
}

std::thread_local! {
    static BIRTHS: std::cell::RefCell<Option<Vec<u64>>> = const { std::cell::RefCell::new(None) };
}

/// Start recording the handles `tag_*` creates on this thread (replaces any
/// earlier recording). Used to prove an array is fresh: made after this call.
pub fn births_begin() {
    BIRTHS.with(|b| *b.borrow_mut() = Some(Vec::new()));
}

/// Stop recording and return the host handles (as `B` bits, flags included)
/// created since `births_begin`.
pub fn births_end() -> Vec<u64> {
    BIRTHS.with(|b| b.borrow_mut().take()).unwrap_or_default()
}

#[inline]
fn tag_host(arr: BqnArr, flags: u64) -> B {
    large_arr_hint(arr.ia());
    let addr = Arc::into_raw(Arc::new(arr)).expose_provenance() as u64;
    LIVE_ARRAYS.fetch_add(1, Ordering::Relaxed);
    assert!(
        addr & !PTR_MASK == 0,
        "array allocation {addr:#x} is not 8-byte aligned or does not fit in 48 bits"
    );
    let b = tagu64(addr | flags, ARR_TAG);
    BIRTHS.with(|v| {
        if let Some(v) = v.borrow_mut().as_mut() {
            v.push(b.0);
        }
    });
    b
}

pub fn tag_arr(arr: BqnArr) -> B {
    tag_host(arr, 0)
}

/// Like `tag_arr`, but the caller owns the array's strong count.
pub fn tag_arr_owned(arr: BqnArr) -> Val {
    Val(tag_host(arr, 0))
}

/// An owned value. For a host array it owns one strong count: `Clone` adds one,
/// `Drop` releases one and frees the array when it was the last. Numbers, chars,
/// functions and device-tagged values carry no count here (Clone/Drop do nothing).
///
/// Not `Copy`, so a double drop does not type-check. `b()` is a `Copy` view that
/// must not outlive the `Val`; `leak()` gives up ownership (what every caller
/// holding a plain `B` does implicitly today).
pub struct Val(B);

#[inline]
fn is_host_arr(b: B) -> bool {
    b.is_arr() && b.0 & 2 == 0
}

#[inline]
fn host_ptr(b: B) -> *const BqnArr {
    std::ptr::with_exposed_provenance::<BqnArr>((b.0 & PTR_MASK) as usize)
}

impl Val {
    /// Wrap a non-array value (number, char, function). Arrays need
    /// `tag_arr_owned` or `from_b_owned`; this panics on one to keep ownership explicit.
    pub fn plain(b: B) -> Val {
        assert!(!b.is_arr(), "Val::plain on an array handle");
        Val(b)
    }

    /// Adopt the strong count of a handle that was leaked (created by `tag_arr`
    /// and held as a plain `B`).
    ///
    /// # Safety
    /// The caller must own that count: no other `Val` may adopt the same count,
    /// and no copy of `b` may be read after the returned `Val` is dropped (unless
    /// another count keeps the array alive). Device-tagged handles are fine (no-op).
    pub unsafe fn from_b_owned(b: B) -> Val {
        Val(b)
    }

    /// Copy view for read-only use that does not outlive this `Val`.
    #[inline]
    pub fn b(&self) -> B {
        self.0
    }

    /// Give up ownership without releasing the count: the array stays alive.
    pub fn leak(self) -> B {
        let b = self.0;
        std::mem::forget(self);
        b
    }
}

impl Clone for Val {
    fn clone(&self) -> Val {
        if is_host_arr(self.0) {
            // SAFETY: `self` owns a strong count of the live allocation behind this
            // host handle (built by `tag_host` from `Arc::into_raw`), so the count
            // is at least 1 and the pointer is valid; the increment gives the clone
            // its own count.
            unsafe { Arc::increment_strong_count(host_ptr(self.0)) };
        }
        Val(self.0)
    }
}

impl Drop for Val {
    fn drop(&mut self) {
        if is_host_arr(self.0) {
            // SAFETY: `self` owns one strong count (see `Clone`/`from_b_owned`) and
            // is being dropped, so adopting that count into an `Arc` is exactly
            // releasing it. `into_inner` returns the array only when this was the
            // last count (atomically), so the counter drops exactly when it frees.
            let arc = unsafe { Arc::from_raw(host_ptr(self.0)) };
            if let Some(arr) = Arc::into_inner(arc) {
                LIVE_ARRAYS.fetch_sub(1, Ordering::Relaxed);
                drop(arr);
            }
        }
    }
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
        let ptr = host_ptr(b);
        // SAFETY: a host handle is built only by `tag_host` from `Arc::into_raw`. Its
        // strong count is released only when a `Val` owning it drops, and that owner
        // promises no `B` copy is read afterwards (`from_b_owned`), so the allocation
        // is live and `ptr` carries its (exposed) provenance. The increment gives the returned Arc its
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

    #[test]
    fn val_clone_drop_round_trip_frees_at_zero() {
        // NOTE: the counter is global and tests run in parallel, so assert on a
        // value-local signal too: the Arc strong count seen through get_arr.
        let v = tag_arr_owned(nums(5));
        let w = v.clone();
        let a = get_arr(v.b()).unwrap();
        assert_eq!(Arc::strong_count(&a), 3); // v, w, a
        drop(v);
        assert_eq!(Arc::strong_count(&a), 2);
        assert_eq!(get_arr(w.b()).unwrap().shape, vec![5]);
        drop(a);
        drop(w); // last count: frees
    }

    #[test]
    fn non_arrays_are_noops() {
        let n = Val::plain(B::m_f64(3.0));
        let m = n.clone();
        drop(n);
        assert_eq!(m.b().o2f(), 3.0);
    }

    #[test]
    fn births_record_new_handles() {
        births_begin();
        let a = tag_arr(nums(1));
        let v = tag_arr_owned(nums(1));
        let born = births_end();
        assert!(born.contains(&a.0) && born.contains(&v.b().0));
        assert!(births_end().is_empty());
    }
}

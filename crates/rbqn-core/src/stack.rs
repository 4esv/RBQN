//! Native stack guard for recursive paths that do not pass through a block
//! (structural primitives, comparison, hashing, formatting on deep arrays).
//! `guard()` throws "Stack overflow" before the thread's stack runs out, so a
//! deeply nested value raises a BQN error instead of aborting the process.

use std::cell::Cell;

thread_local! {
    /// Lowest stack address the thread may reach; 0 = not yet computed,
    /// 1 = bounds unknown on this platform (guard disabled).
    static LIMIT: Cell<usize> = const { Cell::new(0) };
}

/// Stack kept free below the limit for the frames between two checks and for
/// unwinding the BQN error: a quarter of the stack, at most 1 MB.
fn margin(size: usize) -> usize {
    (size / 4).min(1 << 20)
}

#[cfg(all(target_os = "macos", not(miri)))]
fn bounds() -> Option<(usize, usize)> {
    // SAFETY: these are the Darwin libpthread prototypes (pthread_t is a pointer-sized
    // opaque handle, passed here as usize; both getters take it by value and return
    // by value), declared once and called only below.
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_get_stackaddr_np(t: usize) -> *mut u8;
        fn pthread_get_stacksize_np(t: usize) -> usize;
    }
    // SAFETY: pthread_self() is the live calling thread, so the get_*_np calls (which
    // only read that thread's attributes) are valid for it; no memory is written.
    unsafe {
        let t = pthread_self();
        let top = pthread_get_stackaddr_np(t) as usize;
        let size = pthread_get_stacksize_np(t);
        Some((top - size, size))
    }
}

#[cfg(all(target_os = "linux", not(miri)))]
fn bounds() -> Option<(usize, usize)> {
    // SAFETY: an all-zero pthread_attr_t is a valid value to pass as the out-parameter
    // of pthread_getattr_np, which fully initialises it on success (checked == 0
    // before use). attr_getstack then reads it and writes only addr/size; attr is
    // destroyed exactly once, on the success path. The calling thread is live.
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
            return None;
        }
        let mut addr: *mut libc::c_void = std::ptr::null_mut();
        let mut size: libc::size_t = 0;
        let r = libc::pthread_attr_getstack(&attr, &mut addr, &mut size);
        libc::pthread_attr_destroy(&mut attr);
        if r != 0 { None } else { Some((addr as usize, size)) }
    }
}

// NOTE: under Miri the pthread stack queries are foreign calls it cannot execute (and the
// interpreter's stack is not the real one), so the guard is disabled there like on
// unknown platforms. The two unsafe `bounds` bodies above are therefore not Miri-checked.
#[cfg(any(miri, not(any(target_os = "macos", target_os = "linux"))))]
fn bounds() -> Option<(usize, usize)> {
    None
}

#[cold]
fn init() -> usize {
    let lim = match bounds() {
        Some((low, size)) => low + margin(size),
        None => 1,
    };
    LIMIT.with(|l| l.set(lim));
    lim
}

/// Throw "Stack overflow" if the native stack is nearly exhausted.
#[inline]
pub fn guard() {
    let marker = 0u8;
    let sp = std::ptr::addr_of!(marker) as usize;
    let mut lim = LIMIT.with(|l| l.get());
    if lim == 0 {
        lim = init();
    }
    if sp < lim {
        crate::error::throw("Stack overflow");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn guard_passes_on_a_shallow_stack() {
        super::guard();
        super::guard();
    }

    #[test]
    #[cfg(not(miri))] // pthread stack queries are foreign calls Miri cannot run
    fn bounds_contain_the_current_frame() {
        let (low, size) = super::bounds().expect("stack bounds on this platform");
        let here = 0u8;
        let sp = std::ptr::addr_of!(here) as usize;
        assert!(low < sp && sp < low + size);
    }
}

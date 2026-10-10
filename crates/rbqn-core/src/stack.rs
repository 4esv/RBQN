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

#[cfg(target_os = "macos")]
fn bounds() -> Option<(usize, usize)> {
    unsafe extern "C" {
        fn pthread_self() -> usize;
        fn pthread_get_stackaddr_np(t: usize) -> *mut u8;
        fn pthread_get_stacksize_np(t: usize) -> usize;
    }
    // SAFETY: querying the calling thread's own attributes.
    unsafe {
        let t = pthread_self();
        let top = pthread_get_stackaddr_np(t) as usize;
        let size = pthread_get_stacksize_np(t);
        Some((top - size, size))
    }
}

#[cfg(target_os = "linux")]
fn bounds() -> Option<(usize, usize)> {
    // SAFETY: attr is initialised by pthread_getattr_np and destroyed after use.
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
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

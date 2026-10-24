//! Safe Rust API for evaluating BQN over host data.
//!
//! ```
//! use rbqn::{Bqn, Error};
//!
//! # fn main() -> Result<(), Error> {
//! let bqn = Bqn::new()?;
//! let data: Vec<f64> = (0..1_000_000).map(|i| i as f64).collect();
//! let mean = bqn.eval("+´÷≠")?; // a function value
//! let x = bqn.list_f64(&data); // one copy into the BQN store
//! let m = bqn.call1(&mean, &x)?;
//! assert_eq!(m.to_f64(), Some(499_999.5));
//! # Ok(())
//! # }
//! ```
//!
//! # Threads
//!
//! BQN values live in a thread-local store, so a [`Bqn`] is `!Send` and
//! `!Sync`: make one per thread. A [`Value`] borrows its [`Bqn`] and cannot
//! outlive it:
//!
//! ```compile_fail
//! use rbqn::Bqn;
//! let v = {
//!     let bqn = Bqn::new().unwrap();
//!     bqn.list_f64(&[1.0, 2.0])
//! }; // error: `bqn` does not live long enough
//! drop(v);
//! ```
//!
//! Every host to BQN transfer is one copy; nothing is shared with host memory.

use std::marker::PhantomData;

use rbqn_core::array::{ArrData, BqnArr};
use rbqn_core::error::BqnError;
use rbqn_core::B;
use rbqn_vm::derive::{c1, c2};
use rbqn_vm::vm::{clear_error_site, get_arr, tag_arr, take_error_site_lines};

use crate::bootstrap::{self, Runtime};
use crate::exec::{exec_string, panic_to_bqn_error};

/// Source position of an error, as the CLI prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// 1-based line of the failing token.
    pub line: usize,
    /// 1-based column (in characters) of the first caret.
    pub col: usize,
    /// The source line.
    pub src_line: String,
    /// Padding plus carets, aligned under `src_line`.
    pub caret: String,
}

/// A BQN error: the message, and where in the source it happened if known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub site: Option<Site>,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(s) = &self.site {
            write!(f, "\n{}:\n  {}\n  {}", s.line, s.src_line, s.caret)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

impl Error {
    fn msg(m: impl Into<String>) -> Error {
        Error { message: m.into(), site: None }
    }
}

fn parse_site(rendered: &str) -> Option<Site> {
    // Format (vm.rs render_site): "name:LINE:\n  src\n  pad^^^"
    let mut it = rendered.lines();
    let head = it.next()?;
    let line = head.strip_suffix(':')?.rsplit(':').next()?.parse().ok()?;
    let src_line = it.next()?.strip_prefix("  ")?.to_string();
    let caret = it.next()?.strip_prefix("  ")?.to_string();
    let col = caret.chars().position(|c| c == '^')? + 1;
    Some(Site { line, col, src_line, caret })
}

fn to_error(e: BqnError, with_site: bool) -> Error {
    let site = if with_site {
        take_error_site_lines("(embed)").and_then(|s| parse_site(&s))
    } else {
        None
    };
    Error { message: e.to_string(), site }
}

thread_local! {
    /// True while this thread runs BQN code under `guarded`.
    static INSIDE_RBQN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Install, once per process, a hook that stays quiet while the current thread
/// is inside `guarded` (BQN errors travel as panics) and otherwise delegates
/// to whichever hook was installed before it. The hook is never swapped back.
fn install_quiet_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !INSIDE_RBQN.with(|f| f.get()) {
                prev(info);
            }
        }));
    });
}

/// Clears the inside-rbqn flag (restoring the outer value) on drop.
struct InsideGuard(bool);
impl InsideGuard {
    fn enter() -> Self {
        Self(INSIDE_RBQN.with(|f| f.replace(true)))
    }
}
impl Drop for InsideGuard {
    fn drop(&mut self) {
        INSIDE_RBQN.with(|f| f.set(self.0));
    }
}

/// Run `f`, turning BQN's panic-based errors into `Error` without printing them.
fn guarded<T>(f: impl FnOnce() -> Result<T, BqnError>) -> Result<T, Error> {
    clear_error_site();
    install_quiet_hook();
    let r = {
        let _inside = InsideGuard::enter();
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
    };
    match r {
        Ok(Ok(v)) => Ok(v),
        Ok(Err(e)) => Err(to_error(e, true)),
        Err(p) => Err(to_error(panic_to_bqn_error(p), true)),
    }
}

/// A BQN interpreter. One per thread: `!Send` and `!Sync` because the array
/// store is thread-local.
pub struct Bqn {
    rt: Runtime,
    _not_send: PhantomData<*const ()>,
}

impl Bqn {
    /// Bootstrap the runtime (self-hosted compiler included).
    pub fn new() -> Result<Bqn, Error> {
        let rt = bootstrap::bootstrap().map_err(|e| to_error(e, false))?;
        rbqn_vm::derive::set_sys_runtime(rt.compiler, rt.runtime.clone(), rt.formatter);
        rbqn_vm::derive::set_sys_compgen(rt.compgen);
        rbqn_vm::derive::register_derived_equality();
        rbqn_vm::derive::set_sys_args(&[]);
        rbqn_vm::derive::set_sys_path("");
        Ok(Bqn { rt, _not_send: PhantomData })
    }

    fn wrap(&self, b: B) -> Value<'_> {
        Value { b, formatter: self.rt.formatter, _bqn: PhantomData }
    }

    /// Evaluate BQN source and return the value of its last expression.
    /// Nothing is printed.
    ///
    /// ```
    /// let bqn = rbqn::Bqn::new().unwrap();
    /// assert_eq!(bqn.eval("1+1").unwrap().to_f64(), Some(2.0));
    /// let err = bqn.eval("\"a\"+\"b\"").unwrap_err();
    /// assert!(err.site.is_some());
    /// ```
    pub fn eval(&self, src: &str) -> Result<Value<'_>, Error> {
        guarded(|| exec_string(&self.rt, src, 1)).map(|b| self.wrap(b))
    }

    /// Apply function `f` to `x`.
    pub fn call1(&self, f: &Value<'_>, x: &Value<'_>) -> Result<Value<'_>, Error> {
        if !f.b.is_callable() {
            return Err(Error::msg("call1: f is not a function"));
        }
        guarded(|| Ok(c1(f.b, x.b))).map(|b| self.wrap(b))
    }

    /// Apply function `f` to `w` and `x`.
    pub fn call2(&self, f: &Value<'_>, w: &Value<'_>, x: &Value<'_>) -> Result<Value<'_>, Error> {
        if !f.b.is_callable() {
            return Err(Error::msg("call2: f is not a function"));
        }
        guarded(|| Ok(c2(f.b, w.b, x.b))).map(|b| self.wrap(b))
    }

    /// A list of numbers. Copies `data` once into the store.
    pub fn list_f64(&self, data: &[f64]) -> Value<'_> {
        self.wrap(tag_arr(BqnArr::new_vec_f64(data.to_vec())))
    }

    /// A list of integers. Copies `data` once into the store.
    pub fn list_i32(&self, data: &[i32]) -> Value<'_> {
        self.wrap(tag_arr(BqnArr::new_vec_i32(data.to_vec())))
    }

    /// A row-major array of numbers; `data.len()` must equal the product of `shape`.
    pub fn array_f64(&self, shape: &[usize], data: &[f64]) -> Result<Value<'_>, Error> {
        let n: usize = shape.iter().product();
        if n != data.len() {
            return Err(Error::msg(format!(
                "array_f64: shape {shape:?} needs {n} elements, got {}",
                data.len()
            )));
        }
        Ok(self.wrap(tag_arr(BqnArr::new_vec_f64(data.to_vec()).with_shape(shape.to_vec()))))
    }

    /// A string (list of characters).
    pub fn string(&self, s: &str) -> Value<'_> {
        self.wrap(tag_arr(BqnArr::new_vec_c32(s.chars().map(|c| c as u32).collect())))
    }
}

/// A BQN value. Borrows its [`Bqn`], so it cannot outlive it.
#[derive(Clone, Copy)]
pub struct Value<'a> {
    b: B,
    formatter: Option<(B, B)>,
    _bqn: PhantomData<&'a Bqn>,
}

impl<'a> Value<'a> {
    fn arr(&self) -> Option<std::sync::Arc<BqnArr>> {
        if self.b.is_arr() { get_arr(self.b) } else { None }
    }

    /// Shape; empty for an atom.
    pub fn shape(&self) -> Vec<usize> {
        self.arr().map(|a| a.shape.clone()).unwrap_or_default()
    }

    /// The number, if this is a number atom.
    pub fn to_f64(&self) -> Option<f64> {
        if self.b.is_f64() { Some(self.b.o2f()) } else { None }
    }

    /// All elements of a numeric array, row-major.
    ///
    /// ```
    /// let bqn = rbqn::Bqn::new().unwrap();
    /// let v = bqn.eval("2‿3⥊↕6").unwrap();
    /// assert_eq!(v.shape(), vec![2, 3]);
    /// assert_eq!(v.to_vec_f64().unwrap(), vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    /// ```
    pub fn to_vec_f64(&self) -> Result<Vec<f64>, Error> {
        let a = self.arr().ok_or_else(|| Error::msg("to_vec_f64: not an array"))?;
        if !a.is_num_arr() {
            return Err(Error::msg("to_vec_f64: not a numeric array"));
        }
        a.f64_iter().map_err(|e| to_error(e, false))
    }

    /// All elements of an integer-valued array, row-major. Errors on
    /// non-integral or out-of-range elements.
    pub fn to_vec_i32(&self) -> Result<Vec<i32>, Error> {
        self.to_vec_f64()?
            .into_iter()
            .map(|x| {
                if x.fract() == 0.0 && x >= i32::MIN as f64 && x <= i32::MAX as f64 {
                    Ok(x as i32)
                } else {
                    Err(Error::msg(format!("to_vec_i32: {x} is not an i32")))
                }
            })
            .collect()
    }

    /// The text of a character array.
    pub fn to_string_lossy(&self) -> Option<String> {
        let a = self.arr()?;
        if !matches!(a.data, ArrData::C8(_) | ArrData::C16(_) | ArrData::C32(_)) && !a.is_char_arr() {
            return None;
        }
        let cs = a.c32_iter().ok()?;
        Some(cs.into_iter().map(|c| char::from_u32(c).unwrap_or('\u{fffd}')).collect())
    }

    /// An exported field of a namespace.
    pub fn field(&self, name: &str) -> Result<Value<'a>, Error> {
        if !self.b.is_nsp() {
            return Err(Error::msg("field: not a namespace"));
        }
        let ns = rbqn_vm::namespace::get_ns(self.b);
        let gid = rbqn_vm::namespace::str2gid(name);
        ns.get_by_gid(gid)
            .map(|b| Value { b, formatter: self.formatter, _bqn: PhantomData })
            .ok_or_else(|| Error::msg(format!("field: no field '{name}'")))
    }

    /// What the REPL would print for this value.
    pub fn fmt(&self) -> String {
        if let Some(s) = rbqn_vm::derive::fast_fmt_ints(self.b, false) {
            return s;
        }
        if let Some((f, _)) = self.formatter {
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c1(f, self.b)));
            if let Ok(r) = r
                && let Some(a) = get_arr(r)
                && let Ok(cs) = a.c32_iter()
            {
                return cs.into_iter().filter_map(char::from_u32).collect();
            }
        }
        format!("(B:{:#x})", self.b.0)
    }
}

impl std::fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Value({})", Value::fmt(self))
    }
}

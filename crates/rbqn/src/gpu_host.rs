//! Host evaluation of lazy elementwise trees (Step 6): the CPU side of a
//! dispatch decision. The tree is compiled once into a list of block
//! instructions (one slot per unique node, CSE by pointer) and run over
//! blocks of `BLK` elements, so every op is a tight loop the compiler
//! vectorizes and nothing but the result is ever written to memory. Host leaves
//! are read straight from their `BqnArr`, never uploaded.
use std::rc::Rc;
use std::sync::Arc;

use rbqn_core::array::{ArrData, BqnArr};
use rbqn_gpu::buffer::GpuBuffer;
use rbqn_gpu::expr::{Expr, Op};

const BLK: usize = 2048;

/// A host array referenced by a lazy tree (`Expr::Host`), with its exact
/// max |element| and the `B` bits it came from (0 = none, upload not cached).
pub struct HostLeaf {
    pub arr: Arc<BqnArr>,
    pub key: u64,
    pub bound: f64,
}

/// Element data of one leaf, as the evaluator reads it.
pub enum Source {
    Host(Arc<BqnArr>),
    I32(Vec<i32>),
    I64(Vec<i64>),
}

pub trait Num: Copy + Default + Ord + 'static {
    fn of_i64(v: i64) -> Self;
    fn of_f64(v: f64) -> Self;
    fn add(self, b: Self) -> Self;
    fn sub(self, b: Self) -> Self;
    fn mul(self, b: Self) -> Self;
}
macro_rules! num {
    ($t:ty) => {
        impl Num for $t {
            #[inline(always)]
            fn of_i64(v: i64) -> Self { v as $t }
            #[inline(always)]
            fn of_f64(v: f64) -> Self { v as $t }
            #[inline(always)]
            fn add(self, b: Self) -> Self { self.wrapping_add(b) }
            #[inline(always)]
            fn sub(self, b: Self) -> Self { self.wrapping_sub(b) }
            #[inline(always)]
            fn mul(self, b: Self) -> Self { self.wrapping_mul(b) }
        }
    };
}
num!(i32);
num!(i64);

#[derive(Clone, Copy)]
enum R {
    Slot(usize),
    Const(i64),
}

enum Ins {
    Iota(usize),
    Load(usize, usize),
    Bin(Op, usize, R, R),
}

/// A compiled tree: instructions in dependency order and their sources.
pub struct Prog {
    ins: Vec<Ins>,
    slots: usize,
    root: R,
    srcs: Vec<Source>,
}

/// Compile `e`. `dev` turns a device leaf into host data (a download).
pub fn compile(e: &Rc<Expr>, dev: &mut dyn FnMut(&Arc<GpuBuffer>) -> Source) -> Prog {
    struct C<'a> {
        ins: Vec<Ins>,
        slots: usize,
        srcs: Vec<Source>,
        src_keys: Vec<*const ()>,
        memo: Vec<(*const Expr, R)>,
        dev: &'a mut dyn FnMut(&Arc<GpuBuffer>) -> Source,
    }
    impl C<'_> {
        fn src(&mut self, key: *const (), mk: &mut dyn FnMut(&mut Self) -> Source) -> usize {
            if let Some(i) = self.src_keys.iter().position(|&k| k == key) {
                return i;
            }
            let s = mk(self);
            self.srcs.push(s);
            self.src_keys.push(key);
            self.srcs.len() - 1
        }
        fn slot(&mut self) -> usize {
            self.slots += 1;
            self.slots - 1
        }
        fn go(&mut self, e: &Rc<Expr>) -> R {
            let p = Rc::as_ptr(e);
            if let Some((_, r)) = self.memo.iter().find(|(q, _)| *q == p) {
                return *r;
            }
            let r = match &**e {
                Expr::Scalar(s) => R::Const(*s as i64),
                Expr::Iota => {
                    let k = self.slot();
                    self.ins.push(Ins::Iota(k));
                    R::Slot(k)
                }
                Expr::Leaf(b) => {
                    let b = b.clone();
                    let si = self.src(Arc::as_ptr(&b) as *const (), &mut |c| (c.dev)(&b));
                    let k = self.slot();
                    self.ins.push(Ins::Load(k, si));
                    R::Slot(k)
                }
                Expr::Host(h) => {
                    let leaf = h.downcast_ref::<HostLeaf>().expect("host leaf type");
                    let arr = leaf.arr.clone();
                    let si = self.src(Rc::as_ptr(h) as *const (), &mut |_| Source::Host(arr.clone()));
                    let k = self.slot();
                    self.ins.push(Ins::Load(k, si));
                    R::Slot(k)
                }
                Expr::Bin(op, l, r) => {
                    let (a, b) = (self.go(l), self.go(r));
                    let k = self.slot();
                    self.ins.push(Ins::Bin(*op, k, a, b));
                    R::Slot(k)
                }
            };
            self.memo.push((p, r));
            r
        }
    }
    let mut c = C { ins: Vec::new(), slots: 0, srcs: Vec::new(), src_keys: Vec::new(), memo: Vec::new(), dev };
    let root = c.go(e);
    Prog { ins: c.ins, slots: c.slots, root, srcs: c.srcs }
}

#[inline]
fn load<T: Num>(src: &Source, s: usize, out: &mut [T]) {
    let e = s + out.len();
    match src {
        Source::I32(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_i64(x as i64)),
        Source::I64(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_i64(x)),
        Source::Host(a) => match &a.data {
            ArrData::I32(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_i64(x as i64)),
            ArrData::I16(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_i64(x as i64)),
            ArrData::I8(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_i64(x as i64)),
            ArrData::F64(v) => out.iter_mut().zip(&v[s..e]).for_each(|(o, &x)| *o = T::of_f64(x)),
            ArrData::Bit(v) => {
                for (j, o) in out.iter_mut().enumerate() {
                    let i = s + j;
                    *o = T::of_i64(((v[i / 64] >> (i % 64)) & 1) as i64);
                }
            }
            _ => panic!("host leaf: non-numeric data"),
        },
    }
}

#[inline(always)]
fn ap<T: Num>(op: Op, a: T, b: T) -> T {
    match op {
        Op::Add => a.add(b),
        Op::Sub => a.sub(b),
        Op::Mul => a.mul(b),
        Op::Min => a.min(b),
        Op::Max => a.max(b),
    }
}

/// out = a op b, one loop per (op, operand shape) so each vectorizes.
fn bin<T: Num>(op: Op, a: Option<&[T]>, ca: T, b: Option<&[T]>, cb: T, out: &mut [T]) {
    macro_rules! lanes {
        ($o:expr) => {
            match (a, b) {
                (Some(a), Some(b)) => out.iter_mut().zip(a).zip(b).for_each(|((o, &x), &y)| *o = ap($o, x, y)),
                (Some(a), None) => out.iter_mut().zip(a).for_each(|(o, &x)| *o = ap($o, x, cb)),
                (None, Some(b)) => out.iter_mut().zip(b).for_each(|(o, &y)| *o = ap($o, ca, y)),
                (None, None) => out.iter_mut().for_each(|o| *o = ap($o, ca, cb)),
            }
        };
    }
    match op {
        Op::Add => lanes!(Op::Add),
        Op::Sub => lanes!(Op::Sub),
        Op::Mul => lanes!(Op::Mul),
        Op::Min => lanes!(Op::Min),
        Op::Max => lanes!(Op::Max),
    }
}

impl Prog {
    /// Run every block, handing the root block (start, values) to `sink`.
    fn run<T: Num>(&self, n: usize, sink: &mut dyn FnMut(usize, &[T])) {
        let mut slots: Vec<Vec<T>> = (0..self.slots).map(|_| vec![T::default(); BLK]).collect();
        let mut s = 0;
        while s < n {
            let len = BLK.min(n - s);
            for ins in &self.ins {
                match *ins {
                    Ins::Iota(k) => {
                        for (j, o) in slots[k][..len].iter_mut().enumerate() {
                            *o = T::of_i64((s + j) as i64);
                        }
                    }
                    Ins::Load(k, si) => load(&self.srcs[si], s, &mut slots[k][..len]),
                    Ins::Bin(op, k, a, b) => {
                        let mut out = std::mem::take(&mut slots[k]);
                        let get = |r: R| match r {
                            R::Slot(i) => (Some(&slots[i][..len]), T::default()),
                            R::Const(c) => (None, T::of_i64(c)),
                        };
                        let ((sa, ca), (sb, cb)) = (get(a), get(b));
                        bin(op, sa, ca, sb, cb, &mut out[..len]);
                        slots[k] = out;
                    }
                }
            }
            match self.root {
                R::Slot(k) => sink(s, &slots[k][..len]),
                R::Const(c) => sink(s, &vec![T::of_i64(c); len]),
            }
            s += len;
        }
    }

    /// Every element, as i32 (caller guarantees every node fits) or i64.
    pub fn eval_i32(&self, n: usize) -> Vec<i32> {
        let mut v = Vec::with_capacity(n);
        self.run::<i32>(n, &mut |_, b| v.extend_from_slice(b));
        v
    }
    pub fn eval_i64(&self, n: usize) -> Vec<i64> {
        let mut v = Vec::with_capacity(n);
        self.run::<i64>(n, &mut |_, b| v.extend_from_slice(b));
        v
    }

    /// Fold of every element with `op` (add/mul/min/max), exact in i64.
    pub fn fold(&self, n: usize, op: &str, wide: bool) -> i64 {
        let (mut acc, f): (i64, fn(i64, i64) -> i64) = match op {
            "add" => (0, |a, b| a.wrapping_add(b)),
            "mul" => (1, |a, b| a.wrapping_mul(b)),
            "min" => (i64::MAX, |a, b| a.min(b)),
            _ => (i64::MIN, |a, b| a.max(b)),
        };
        macro_rules! blocks {
            ($t:ty) => {
                self.run::<$t>(n, &mut |_, b| {
                    let part = match op {
                        "add" => b.iter().map(|&x| x as i64).sum::<i64>(),
                        "mul" => b.iter().fold(1i64, |p, &x| p.wrapping_mul(x as i64)),
                        "min" => b.iter().copied().min().map_or(i64::MAX, |x| x as i64),
                        _ => b.iter().copied().max().map_or(i64::MIN, |x| x as i64),
                    };
                    acc = f(acc, part);
                })
            };
        }
        if wide { blocks!(i64) } else { blocks!(i32) }
        acc
    }
}

/// Exact max |element| of a numeric host array (Bit: 1). None for chars/boxes.
pub fn max_abs(a: &BqnArr) -> Option<f64> {
    Some(match &a.data {
        ArrData::Bit(_) => 1.0,
        ArrData::I8(v) => v.iter().map(|&x| (x as i32).unsigned_abs()).max().unwrap_or(0) as f64,
        ArrData::I16(v) => v.iter().map(|&x| (x as i32).unsigned_abs()).max().unwrap_or(0) as f64,
        ArrData::I32(v) => v.iter().map(|&x| x.unsigned_abs()).max().unwrap_or(0) as f64,
        ArrData::F64(v) => v.iter().fold(0.0f64, |m, &x| m.max(x.abs())),
        _ => return None,
    })
}

// •FFI — Foreign Function Interface for calling C functions from BQN.
//
// `path •FFI spec` returns a callable BQN function. spec = "result"‿"funcname"‿"arg1"‿...
// Types follow the grammar in the BQN spec (system.md, "Foreign Function Interface"):
//   conv = type (":" bqn)?   type = [iuf]nat | a | (*|&) type? | [nat] type | { conv,... }
// Calls go through libffi on the platform C ABI; libloading only resolves symbols.
// Every C value lives in a byte buffer laid out by `size_align`/`field_offsets`, so
// numbers, arrays, structs and pointers share one write/read path.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use rbqn_core::value::B;
use rbqn_core::error::throw;

/// Cached loaded libraries (resolved path -> Library)
static FFI_LIBS: LazyLock<Mutex<HashMap<String, Arc<libloading::Library>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static FFI_SPECS: LazyLock<Mutex<Vec<Arc<FfiSpec>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

static PTR_OBJS: LazyLock<Mutex<Vec<PtrObj>>> = LazyLock::new(|| Mutex::new(Vec::new()));
/// Namespace value bits -> index into PTR_OBJS.
static PTR_NS: LazyLock<Mutex<HashMap<u64, usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------- types

type Bqn = (char, u32);

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    Num(char, u32),
    Any,
    /// Pointee type (None = untyped) and mutability.
    Ptr(Option<Box<Ty>>, bool),
    Arr(usize, Box<Conv>),
    Struct(Vec<Conv>),
}

#[derive(Debug, Clone, PartialEq)]
struct Conv {
    ty: Ty,
    bqn: Option<Bqn>,
}

struct Parser {
    s: Vec<char>,
    pos: usize,
    allow_amp: bool,
}

impl Parser {
    fn err(&self, msg: &str) -> ! {
        throw(format!("•FFI: {} in type \"{}\"", msg, self.s.iter().collect::<String>()))
    }
    fn peek(&self) -> Option<char> { self.s.get(self.pos).copied() }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) { self.pos += 1; true } else { false }
    }
    fn nat(&mut self) -> usize {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) { self.pos += 1; }
        if start == self.pos { self.err("expected a number"); }
        self.s[start..self.pos].iter().collect::<String>().parse()
            .unwrap_or_else(|_| self.err("number too large"))
    }
    fn num_type(&mut self, k: char) -> (char, u32) {
        let bits = self.nat() as u32;
        let ok = match k { 'f' => bits == 32 || bits == 64, _ => matches!(bits, 8 | 16 | 32 | 64) };
        if !ok { self.err("unsupported width"); }
        (k, bits)
    }
    fn conv(&mut self, depth: usize) -> Conv {
        let mut ty = self.ty(depth);
        let mut bqn = None;
        if self.eat(':') {
            let k = self.peek().filter(|c| matches!(c, 'i' | 'u' | 'f' | 'c'))
                .unwrap_or_else(|| self.err("expected i, u, f or c after \":\""));
            self.pos += 1;
            let bits = self.nat() as u32;
            let ok = match k { 'f' => matches!(bits, 32 | 64), 'c' => matches!(bits, 8 | 16 | 32), _ => matches!(bits, 8 | 16 | 32 | 64) };
            if !ok { self.err("unsupported BQN element width"); }
            bqn = Some((k, bits));
            // NOTE: a bare pointer with a BQN element type is a pointer to raw data of that width.
            if let Ty::Ptr(e @ None, _) = &mut ty {
                *e = Some(Box::new(Ty::Num('u', bits)));
            }
        }
        Conv { ty, bqn }
    }
    fn starts_type(&self) -> bool {
        matches!(self.peek(), Some('i' | 'u' | 'f' | 'a' | '*' | '&' | '[' | '{'))
    }
    fn ty(&mut self, depth: usize) -> Ty {
        let c = self.peek().unwrap_or_else(|| self.err("unexpected end"));
        self.pos += 1;
        match c {
            'i' | 'u' | 'f' => { let (k, b) = self.num_type(c); Ty::Num(k, b) }
            'a' => Ty::Any,
            '*' | '&' => {
                if c == '&' && !(depth == 0 && self.allow_amp) {
                    self.err("\"&\" is only valid at the top level of an argument");
                }
                let elem = if self.starts_type() { Some(Box::new(self.ty(depth + 1))) } else { None };
                Ty::Ptr(elem, c == '&')
            }
            '[' => {
                let n = self.nat();
                if !self.eat(']') { self.err("expected \"]\""); }
                Ty::Arr(n, Box::new(Conv { ty: self.ty(depth + 1), bqn: None }))
            }
            '{' => {
                let mut fields = Vec::new();
                if !self.eat('}') {
                    loop {
                        fields.push(self.conv(depth + 1));
                        if self.eat('}') { break; }
                        if !self.eat(',') { self.err("expected \",\" or \"}\""); }
                    }
                }
                Ty::Struct(fields)
            }
            other => self.err(&format!("unexpected \"{}\"", other)),
        }
    }
}

fn parse_conv(s: &str, allow_amp: bool) -> Conv {
    let mut p = Parser { s: s.chars().collect(), pos: 0, allow_amp };
    let c = p.conv(0);
    if p.pos != p.s.len() { p.err("trailing characters"); }
    c
}

fn size_align(t: &Ty) -> (usize, usize) {
    match t {
        Ty::Num(_, b) => { let n = (*b / 8) as usize; (n, n) }
        Ty::Any | Ty::Ptr(..) => (8, 8),
        Ty::Arr(n, e) => { let (s, a) = size_align(&e.ty); (s * n, a) }
        Ty::Struct(f) => { let (_, s, a) = field_offsets(f); (s, a) }
    }
}

fn field_offsets(fields: &[Conv]) -> (Vec<usize>, usize, usize) {
    let (mut off, mut al) = (0usize, 1usize);
    let mut offs = Vec::with_capacity(fields.len());
    for f in fields {
        let (s, a) = size_align(&f.ty);
        off = off.div_ceil(a) * a;
        offs.push(off);
        off += s;
        al = al.max(a);
    }
    (offs, off.div_ceil(al) * al, al)
}

fn size_of(t: &Ty) -> usize { size_align(t).0 }

/// Types are compatible if identical, or if either is an untyped pointer.
fn compat(a: &Ty, b: &Ty) -> bool {
    match (a, b) {
        (Ty::Ptr(None, _), Ty::Ptr(..)) | (Ty::Ptr(..), Ty::Ptr(None, _)) => true,
        (Ty::Ptr(Some(x), _), Ty::Ptr(Some(y), _)) => compat(x, y),
        (Ty::Arr(n, x), Ty::Arr(m, y)) => n == m && compat(&x.ty, &y.ty),
        (Ty::Struct(x), Ty::Struct(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| compat(&p.ty, &q.ty)),
        _ => a == b,
    }
}

fn to_ffi_type(t: &Ty) -> libffi::middle::Type {
    use libffi::middle::Type;
    match t {
        Ty::Num('i', 8) => Type::i8(),
        Ty::Num('i', 16) => Type::i16(),
        Ty::Num('i', 32) => Type::i32(),
        Ty::Num('i', _) => Type::i64(),
        Ty::Num('u', 8) => Type::u8(),
        Ty::Num('u', 16) => Type::u16(),
        Ty::Num('u', 32) => Type::u32(),
        Ty::Num('u', _) => Type::u64(),
        Ty::Num(_, 32) => Type::f32(),
        Ty::Num(..) => Type::f64(),
        Ty::Any | Ty::Ptr(..) => Type::pointer(),
        Ty::Arr(n, e) => {
            if *n == 0 { throw("•FFI: zero-length array can't be passed by value"); }
            Type::structure((0..*n).map(|_| to_ffi_type(&e.ty)).collect::<Vec<_>>())
        }
        Ty::Struct(f) => {
            if f.is_empty() { throw("•FFI: empty struct can't be passed by value"); }
            Type::structure(f.iter().map(|c| to_ffi_type(&c.ty)).collect::<Vec<_>>())
        }
    }
}

// ---------------------------------------------------------------- BQN <-> bytes

/// 8-byte-aligned heap buffer; address is stable for its lifetime.
struct Buf(Vec<u64>, usize);

impl Buf {
    fn new(len: usize) -> Buf { Buf(vec![0u64; len.div_ceil(8).max(2) + 1], len) }
    fn bytes(&mut self) -> &mut [u8] {
        let len = self.1;
        // SAFETY: self.0 holds at least len.div_ceil(8).max(2)+1 u64 words (see `new`), i.e.
        // >= len initialised bytes; u8 has alignment 1; the &mut self borrow makes the
        // returned slice the only live view of the allocation.
        unsafe { std::slice::from_raw_parts_mut(self.0.as_mut_ptr() as *mut u8, len) }
    }
    fn ptr(&mut self) -> *mut u8 { self.0.as_mut_ptr() as *mut u8 }
}

const MAX_EXACT: f64 = 9007199254740992.0; // 2⋆53

fn num_write(k: char, bits: u32, v: B, out: &mut [u8]) {
    if !v.is_f64() { throw("•FFI: expected a number"); }
    let x = v.o2f();
    match (k, bits) {
        ('f', 32) => out.copy_from_slice(&(x as f32).to_le_bytes()),
        ('f', _) => out.copy_from_slice(&x.to_le_bytes()),
        _ => {
            let half = 2f64.powi(bits as i32 - 1);
            let (lo, hi) = if k == 'i' { (-half, half) } else { (0.0, half * 2.0) };
            if !x.is_finite() || x.fract() != 0.0 || x.abs() > MAX_EXACT || x < lo || x >= hi {
                throw(format!("•FFI: {} doesn't fit in {}{}", x, k, bits));
            }
            let raw = (x as i64) as u64;
            out.copy_from_slice(&raw.to_le_bytes()[..out.len()]);
        }
    }
}

fn num_read(k: char, bits: u32, buf: &[u8]) -> B {
    let mut raw = [0u8; 8];
    raw[..buf.len()].copy_from_slice(buf);
    let u = u64::from_le_bytes(raw);
    let x = match (k, bits) {
        ('f', 32) => f32::from_bits(u as u32) as f64,
        ('f', _) => f64::from_bits(u),
        ('i', _) => { let sh = 64 - bits; (((u << sh) as i64) >> sh) as f64 }
        _ => u as f64,
    };
    if k != 'f' && x.abs() > MAX_EXACT {
        throw(format!("•FFI: result {} is outside the exactly representable integer range", x));
    }
    B::m_f64(x)
}

fn list_items(v: B) -> Vec<B> {
    let arr = crate::vm::get_arr(v).unwrap_or_else(|| throw("•FFI: expected a list"));
    (0..arr.ia()).map(|i| arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))).collect()
}

/// BQN list of `bqn`-typed elements -> raw bytes.
fn list_to_bytes(bqn: Bqn, v: B) -> Vec<u8> {
    let (k, bits) = bqn;
    let w = (bits / 8) as usize;
    let items = list_items(v);
    let mut out = vec![0u8; items.len() * w];
    for (it, slot) in items.iter().zip(out.chunks_mut(w)) {
        if k == 'c' {
            if !it.is_c32() { throw("•FFI: expected a character"); }
            let c = it.0 as u32;
            if bits < 32 && c >= 1u32 << bits { throw(format!("•FFI: character doesn't fit in c{}", bits)); }
            slot.copy_from_slice(&c.to_le_bytes()[..w]);
        } else {
            num_write(k, bits, *it, slot);
        }
    }
    out
}

fn bytes_to_list(bqn: Bqn, bytes: &[u8]) -> B {
    let (k, bits) = bqn;
    let w = (bits / 8) as usize;
    if !bytes.len().is_multiple_of(w) { throw("•FFI: data length isn't a multiple of the BQN element width"); }
    let chunks = bytes.chunks(w);
    if k == 'c' {
        let v: Vec<u32> = chunks.map(|c| { let mut r = [0u8; 4]; r[..w].copy_from_slice(c); u32::from_le_bytes(r) }).collect();
        return crate::vm::tag_arr(rbqn_core::array::BqnArr::new_vec_c32(v));
    }
    let v: Vec<f64> = chunks.map(|c| num_read(k, bits, c).o2f()).collect();
    crate::vm::tag_arr(rbqn_core::array::squeeze_num(rbqn_core::array::BqnArr::new_vec_f64(v)))
}

fn mk_list(items: Vec<B>) -> B { crate::vm::b_vec_to_arr(items) }

/// Mutable pointer whose buffer is returned after the call.
struct MutPtr {
    /// Index into Marsh::keep, or None when a pointer object was passed (returned unchanged).
    keep: Option<usize>,
    len: usize,
    elem: Conv,
    bqn: Option<Bqn>,
    obj: B,
}

#[derive(Default)]
struct Marsh {
    keep: Vec<Buf>,
    muts: Vec<MutPtr>,
}

fn ptr_obj_id(v: B) -> Option<usize> {
    if v.is_nsp() { lock(&PTR_NS).get(&v.0).copied() } else { None }
}

impl Marsh {
    fn write(&mut self, c: &Conv, v: B, out: &mut [u8]) {
        if let Some(bq) = c.bqn {
            match &c.ty {
                Ty::Num(..) | Ty::Struct(_) | Ty::Arr(..) => {
                    let bytes = list_to_bytes(bq, v);
                    if bytes.len() != out.len() {
                        throw(format!("•FFI: expected {} bytes of data, got {}", out.len(), bytes.len()));
                    }
                    out.copy_from_slice(&bytes);
                    return;
                }
                Ty::Ptr(Some(e), m) => {
                    if ptr_obj_id(v).is_none() {
                        let bytes = list_to_bytes(bq, v);
                        let es = size_of(e);
                        if es == 0 || !bytes.len().is_multiple_of(es) { throw("•FFI: data length isn't a multiple of the pointee size"); }
                        let len = bytes.len() / es;
                        let mut buf = Buf::new(bytes.len());
                        buf.bytes().copy_from_slice(&bytes);
                        let elem = Conv { ty: (**e).clone(), bqn: Some(bq) };
                        self.finish_ptr(buf, len, *m, elem, Some(bq), v, out);
                        return;
                    }
                }
                _ => throw("•FFI: \":\" conversion isn't valid for this type"),
            }
        }
        match &c.ty {
            Ty::Num(k, b) => num_write(*k, *b, v, out),
            Ty::Any => out.copy_from_slice(&v.0.to_le_bytes()),
            Ty::Ptr(elem, m) => {
                if let Some(id) = ptr_obj_id(v) {
                    let p = lock(&PTR_OBJS)[id].clone();
                    if let (Some(pe), Some(ae)) = (&p.elem, elem)
                        && !compat(&pe.ty, ae) {
                            throw("•FFI: pointer object type isn't compatible with the argument type");
                        }
                    out.copy_from_slice(&(p.addr as u64).to_le_bytes());
                    if *m {
                        let e = elem.as_deref().cloned().map(|ty| Conv { ty, bqn: None })
                            .unwrap_or(Conv { ty: Ty::Any, bqn: None });
                        self.muts.push(MutPtr { keep: None, len: 0, elem: e, bqn: None, obj: v });
                    }
                    return;
                }
                let Some(e) = elem else { throw("•FFI: an untyped pointer needs a pointer object") };
                let items = list_items(v);
                let es = size_of(e);
                let mut buf = Buf::new(items.len() * es);
                let ec = Conv { ty: (**e).clone(), bqn: None };
                for (it, slot) in items.iter().zip(buf.bytes().chunks_mut(es.max(1))) {
                    self.write(&ec, *it, slot);
                }
                self.finish_ptr(buf, items.len(), *m, ec, None, v, out);
            }
            Ty::Arr(n, e) => {
                let items = list_items(v);
                if items.len() != *n { throw(format!("•FFI: expected a list of {} elements, got {}", n, items.len())); }
                let es = size_of(&e.ty);
                for (it, slot) in items.iter().zip(out.chunks_mut(es.max(1))) { self.write(e, *it, slot); }
            }
            Ty::Struct(fs) => {
                let items = list_items(v);
                if items.len() != fs.len() { throw(format!("•FFI: expected a list of {} fields, got {}", fs.len(), items.len())); }
                let (offs, _, _) = field_offsets(fs);
                for ((f, it), off) in fs.iter().zip(&items).zip(offs) {
                    let end = off + size_of(&f.ty);
                    self.write(f, *it, &mut out[off..end]);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_ptr(&mut self, mut buf: Buf, len: usize, mutable: bool, elem: Conv, bqn: Option<Bqn>, orig: B, out: &mut [u8]) {
        out.copy_from_slice(&(buf.ptr() as u64).to_le_bytes());
        self.keep.push(buf);
        if mutable {
            self.muts.push(MutPtr { keep: Some(self.keep.len() - 1), len, elem, bqn, obj: orig });
        }
    }
}

fn read(c: &Conv, buf: &[u8]) -> B {
    if let Some(bq) = c.bqn {
        match &c.ty {
            Ty::Num(..) | Ty::Struct(_) | Ty::Arr(..) => return bytes_to_list(bq, buf),
            Ty::Ptr(Some(e), _) if matches!(**e, Ty::Num(..)) => {
                // NOTE: a pointer result with a BQN element type is read up to a zero element (C string).
                let addr = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
                if addr == 0 { return bytes_to_list(bq, &[]); }
                let w = size_of(e);
                let mut n = 0usize;
                // SAFETY: NOT sound for arbitrary input, by design of •FFI: `addr` is a raw
                // address the BQN program obtained from C (a pointer result) and the scan
                // reads until a zero element, as C code would for a C string. Validity is
                // the caller's contract, same as CBQN's •FFI; see #128.
                unsafe {
                    while (0..w).any(|i| *((addr + n * w + i) as *const u8) != 0) { n += 1; }
                    return bytes_to_list(bq, std::slice::from_raw_parts(addr as *const u8, n * w));
                }
            }
            _ => throw("•FFI: \":\" conversion isn't valid for this type"),
        }
    }
    match &c.ty {
        Ty::Num(k, b) => num_read(*k, *b, &buf[..(*b / 8) as usize]),
        Ty::Any => B(u64::from_le_bytes(buf[..8].try_into().unwrap())),
        Ty::Ptr(elem, _) => {
            let addr = u64::from_le_bytes(buf[..8].try_into().unwrap()) as usize;
            let elem = elem.as_deref().map(|t| Conv { ty: t.clone(), bqn: None });
            let stride = elem.as_ref().map_or(0, |e| size_of(&e.ty));
            make_ptr_obj(PtrObj { addr, elem, stride })
        }
        Ty::Arr(n, e) => {
            let es = size_of(&e.ty);
            mk_list((0..*n).map(|i| read(e, &buf[i * es..(i + 1) * es])).collect())
        }
        Ty::Struct(fs) => {
            let (offs, _, _) = field_offsets(fs);
            mk_list(fs.iter().zip(offs).map(|(f, o)| read(f, &buf[o..o + size_of(&f.ty)])).collect())
        }
    }
}

// ---------------------------------------------------------------- pointer objects

#[derive(Debug, Clone)]
struct PtrObj {
    addr: usize,
    elem: Option<Conv>,
    stride: usize,
}

fn make_ptr_obj(p: PtrObj) -> B {
    use crate::namespace::{str2gid, NSDesc, NS};
    let id = { let mut v = lock(&PTR_OBJS); v.push(p); v.len() - 1 };
    let names = ["read", "write", "add", "sub", "cast", "field"];
    let desc = Arc::new(NSDesc { var_am: 6, exp_gids: names.iter().map(|n| str2gid(n)).collect() });
    let body = Arc::new(crate::block::Body::new(6, 0, 0, 0));
    let fns: Vec<B> = (0..6).map(|op| crate::derive::m_ptr_fn(id, op)).collect();
    let sc = std::rc::Rc::new(crate::scope::Scope::new(body, None, 6, &fns));
    let b = crate::namespace::store_ns(NS { desc, sc });
    lock(&PTR_NS).insert(b.0, id);
    b
}

fn int_arg(v: B, what: &str) -> i64 {
    if !v.is_f64() || v.o2f().fract() != 0.0 || v.o2f().abs() > MAX_EXACT {
        throw(format!("•FFI: {} must be an integer", what));
    }
    v.o2f() as i64
}

fn offset_addr(p: &PtrObj, off: i64) -> usize {
    (p.addr as i64).wrapping_add(off.wrapping_mul(p.stride as i64)) as usize
}

/// Pointer-object methods: 0 Read, 1 Write, 2 Add, 3 Sub, 4 Cast, 5 Field.
pub fn ptr_call(id: usize, op: u8, w: Option<B>, x: B) -> B {
    let p = lock(&PTR_OBJS)[id].clone();
    let typed = |name: &str| -> Conv {
        p.elem.clone().unwrap_or_else(|| throw(format!("•FFI: {} on an untyped pointer", name)))
    };
    match op {
        0 => {
            let e = typed("Read");
            let addr = offset_addr(&p, int_arg(x, "offset"));
            // SAFETY: NOT sound for arbitrary input, by design of •FFI: `addr` is the pointer
            // object's address plus a BQN-supplied offset, and the caller must ensure
            // size_of(elem) readable bytes there (as in CBQN's •FFI); see #128.
            let bytes = unsafe { std::slice::from_raw_parts(addr as *const u8, size_of(&e.ty)) };
            read(&e, bytes)
        }
        1 => {
            let e = typed("Write");
            let off = w.map_or(0, |w| int_arg(w, "offset"));
            let addr = offset_addr(&p, off);
            let mut m = Marsh::default();
            let mut tmp = vec![0u8; size_of(&e.ty)];
            m.write(&e, x, &mut tmp);
            // NOTE: pointers written into C memory must outlive this call, so leak their buffers.
            std::mem::forget(m);
            // SAFETY: `tmp` is a live local Vec of tmp.len() bytes. The destination `addr` is
            // NOT checked: the BQN caller must ensure tmp.len() writable bytes there that
            // do not overlap `tmp` (a fresh heap allocation, so overlap is impossible for
            // honest addresses), as in CBQN's •FFI; see #128.
            unsafe { std::ptr::copy_nonoverlapping(tmp.as_ptr(), addr as *mut u8, tmp.len()); }
            x
        }
        2 => {
            typed("Add");
            make_ptr_obj(PtrObj { addr: offset_addr(&p, int_arg(x, "offset")), ..p })
        }
        3 => {
            let e = typed("Sub");
            if let Some(oid) = ptr_obj_id(x) {
                let o = lock(&PTR_OBJS)[oid].clone();
                let Some(oe) = &o.elem else { throw("•FFI: Sub needs a typed pointer") };
                if !compat(&e.ty, &oe.ty) || o.stride != p.stride {
                    throw("•FFI: Sub needs pointers of compatible types and the same stride");
                }
                let d = p.addr as i64 - o.addr as i64;
                if p.stride == 0 || d % p.stride as i64 != 0 { throw("•FFI: Sub: pointers differ by a non-multiple of the stride"); }
                B::m_f64((d / p.stride as i64) as f64)
            } else {
                make_ptr_obj(PtrObj { addr: offset_addr(&p, -int_arg(x, "offset")), ..p })
            }
        }
        4 => {
            let s = b_to_rust_string(x);
            if s.is_empty() {
                make_ptr_obj(PtrObj { addr: p.addr, elem: None, stride: 0 })
            } else {
                let c = parse_conv(&s, false);
                let stride = size_of(&c.ty);
                make_ptr_obj(PtrObj { addr: p.addr, elem: Some(c), stride })
            }
        }
        _ => {
            let e = typed("Field");
            let n = int_arg(x, "field index");
            let (offs, fields): (Vec<usize>, Vec<Conv>) = match &e.ty {
                Ty::Struct(fs) => (field_offsets(fs).0, fs.clone()),
                Ty::Arr(len, el) => {
                    let es = size_of(&el.ty);
                    ((0..*len).map(|i| i * es).collect(), vec![(**el).clone(); *len])
                }
                _ => throw("•FFI: Field needs a pointer to a struct or array"),
            };
            if n < 0 || n as usize >= fields.len() { throw("•FFI: Field index out of range"); }
            let i = n as usize;
            make_ptr_obj(PtrObj { addr: p.addr + offs[i], elem: Some(fields[i].clone()), stride: p.stride })
        }
    }
}

// ---------------------------------------------------------------- specs and loading

#[derive(Debug)]
struct ArgSpec {
    conv: Conv,
    /// `>`: the whole BQN argument is the value, not a list of values.
    whole: bool,
    from_w: bool,
}

#[derive(Debug)]
struct FfiSpec {
    ret: Option<Conv>,
    /// Result type "&": the single mutable pointer is the result.
    ret_amp: bool,
    func_name: String,
    func: usize,
    args: Vec<ArgSpec>,
}

fn load_library(path: &str) -> Arc<libloading::Library> {
    let mut cache = lock(&FFI_LIBS);
    if let Some(lib) = cache.get(path) {
        return Arc::clone(lib);
    }
    let lib = if path == "@" {
        // @ = current process handle (includes libc on all platforms)
        #[cfg(unix)]
        { libloading::os::unix::Library::this().into() }
        #[cfg(not(unix))]
        { throw("•FFI: @ (libc) not supported on this platform") }
    } else {
        // SAFETY: loading a library runs its initialisers, which is the point of
        // •FFI; the caller of •FFI chose `path`. Library handles are cached in an Arc
        // for the process lifetime, so symbols resolved from it never dangle.
        unsafe { libloading::Library::new(path) }
            .unwrap_or_else(|e| throw(format!("•FFI: couldn't load library \"{}\": {}", path, e)))
    };
    let arc = Arc::new(lib);
    cache.insert(path.to_string(), Arc::clone(&arc));
    arc
}

/// Relative library paths resolve against the directory of `•path`, like `•file`;
/// a name not found there is left for the system loader to search.
fn resolve_lib_path(p: &str) -> String {
    if p == "@" || p.is_empty() || std::path::Path::new(p).is_absolute() {
        return p.to_string();
    }
    let base = b_to_rust_string(crate::derive::dispatch_sys_env(38));
    if let Some(dir) = std::path::Path::new(&base).parent().filter(|d| !base.is_empty() && !d.as_os_str().is_empty()) {
        let cand = dir.join(p);
        if cand.exists() { return cand.to_string_lossy().into_owned(); }
    }
    p.to_string()
}

fn str_at(arr: &rbqn_core::array::BqnArr, i: usize) -> String {
    b_to_rust_string(arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e)))
}

fn parse_arg(s: &str) -> ArgSpec {
    let (mut whole, mut w, mut x) = (false, false, false);
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        match c {
            '>' if !whole => whole = true,
            '𝕨' if !w && !x => w = true,
            '𝕩' if !w && !x => x = true,
            '>' | '𝕨' | '𝕩' => throw(format!("•FFI: repeated or conflicting markers in \"{}\"", s)),
            _ => break,
        }
        rest = &rest[c.len_utf8()..];
    }
    ArgSpec { conv: parse_conv(rest, true), whole, from_w: w }
}

fn parse_spec(spec: B) -> FfiSpec {
    let arr = crate::vm::get_arr(spec).unwrap_or_else(|| throw("•FFI: 𝕩 must be a list"));
    if arr.ia() < 2 {
        throw("•FFI: 𝕩 must have at least 2 elements (result type and function name)");
    }
    let ret_str = str_at(&arr, 0);
    let func_name = str_at(&arr, 1);
    let (ret, ret_amp) = match ret_str.as_str() {
        "" => (None, false),
        "&" => (None, true),
        s if s.contains('&') => throw("•FFI: result type can't contain \"&\" except as exactly \"&\""),
        s => (Some(parse_conv(s, false)), false),
    };
    let args: Vec<ArgSpec> = (2..arr.ia()).map(|i| parse_arg(&str_at(&arr, i))).collect();
    for side in [false, true] {
        let on: Vec<&ArgSpec> = args.iter().filter(|a| a.from_w == side).collect();
        if on.iter().any(|a| a.whole) && on.len() != 1 {
            throw("•FFI: an argument marked \">\" must be the only value from its BQN argument");
        }
    }
    let nmut = args.iter().filter(|a| matches!(a.conv.ty, Ty::Ptr(_, true))).count();
    if ret_amp && nmut != 1 {
        throw("•FFI: result type \"&\" needs exactly one mutable pointer argument");
    }
    FfiSpec { ret, ret_amp, func_name, func: 0, args }
}

fn b_to_rust_string(b: B) -> String {
    if let Some(arr) = crate::vm::get_arr(b) {
        (0..arr.ia()).map(|i| {
            let v = arr.get(i).unwrap_or_else(|e| rbqn_core::error::throw_bqn(e));
            if v.is_c32() {
                char::from_u32(v.o2c().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))).unwrap_or('?')
            } else {
                '?'
            }
        }).collect()
    } else if b.is_c32() {
        char::from_u32(b.o2c().unwrap_or_else(|e| rbqn_core::error::throw_bqn(e))).map(|c| c.to_string()).unwrap_or_default()
    } else {
        throw("•FFI: expected string element")
    }
}

/// Main entry: `path •FFI spec` → returns a callable BQN function.
pub fn ffi_load(path: B, spec: B) -> B {
    let path_str = resolve_lib_path(&b_to_rust_string(path));
    let lib = load_library(&path_str);
    let mut ffi_spec = parse_spec(spec);
    // SAFETY: `lib` (an Arc held in FFI_LIBS for the process lifetime) stays loaded, so
    // the symbol address stays valid. The symbol is read only as an address (*const ());
    // its real signature is checked, if at all, only by the caller's spec (see ffi_call).
    ffi_spec.func = unsafe {
        let sym: libloading::Symbol<*const ()> = lib.get(ffi_spec.func_name.as_bytes())
            .unwrap_or_else(|e| throw(format!(
                "•FFI: couldn't find symbol \"{}\" in \"{}\": {}", ffi_spec.func_name, path_str, e
            )));
        *sym as usize
    };
    let mut specs = lock(&FFI_SPECS);
    specs.push(Arc::new(ffi_spec));
    crate::derive::m_ffi_fn(specs.len() - 1)
}

/// Split a BQN argument (`𝕨` or `𝕩`) into the values for `n` C arguments.
fn split_args(v: B, n: usize, whole: bool) -> Vec<B> {
    if whole { return vec![v]; }
    match crate::vm::get_arr(v) {
        Some(arr) => {
            if arr.ia() != n { throw(format!("•FFI: expected {} argument(s), got {}", n, arr.ia())); }
            list_items(v)
        }
        None if n == 1 => vec![v],
        None => throw(format!("•FFI: expected a list of {} arguments", n)),
    }
}

/// Call an FFI function: `x` supplies the unmarked arguments, `w` those marked 𝕨.
pub fn ffi_call(spec_id: usize, w: Option<B>, x: B) -> B {
    let spec = Arc::clone(&lock(&FFI_SPECS)[spec_id]);
    let nw = spec.args.iter().filter(|a| a.from_w).count();
    let nx = spec.args.len() - nw;
    let whole_of = |side: bool| spec.args.iter().any(|a| a.from_w == side && a.whole);

    let wvals = match w {
        Some(w) if nw > 0 => split_args(w, nw, whole_of(true)),
        Some(w) if crate::vm::get_arr(w).is_some_and(|a| a.ia() == 0) => vec![],
        Some(_) => throw(format!("•FFI {}: 𝕨 must be an empty list (no 𝕨 arguments)", spec.func_name)),
        None if nw > 0 => throw(format!("•FFI {}: needs 𝕨 for {} argument(s)", spec.func_name, nw)),
        None => vec![],
    };
    let xvals = if nx == 0 { vec![] } else { split_args(x, nx, whole_of(false)) };
    let (mut wi, mut xi) = (wvals.into_iter(), xvals.into_iter());

    let mut m = Marsh::default();
    let mut bufs: Vec<Buf> = Vec::with_capacity(spec.args.len());
    for a in &spec.args {
        let v = if a.from_w { wi.next() } else { xi.next() }.unwrap();
        let mut b = Buf::new(size_of(&a.conv.ty));
        m.write(&a.conv, v, b.bytes());
        bufs.push(b);
    }

    let cif = libffi::middle::Cif::new(
        spec.args.iter().map(|a| to_ffi_type(&a.conv.ty)),
        spec.ret.as_ref().map_or(libffi::middle::Type::void(), |c| to_ffi_type(&c.ty)),
    );
    let mut ret = Buf::new(spec.ret.as_ref().map_or(8, |c| size_of(&c.ty)).max(16));
    let mut ptrs: Vec<*mut std::ffi::c_void> = bufs.iter_mut().map(|b| b.ptr() as *mut _).collect();
    // SAFETY: `spec.func` is the address of a symbol in a library kept loaded by
    // FFI_LIBS (see ffi_load). The Cif was built from the same spec as `ptrs`/`ret`:
    // `ptrs` holds one pointer per argument to a Buf of size_of(arg type) bytes, 8-byte
    // aligned, and `ret` is >= max(size_of(ret), 16) bytes (libffi's minimum), all
    // alive across the call. That the C function really has the declared signature is
    // the BQN caller's contract (as in CBQN's •FFI); see #128.
    unsafe {
        let f: unsafe extern "C" fn() = std::mem::transmute(spec.func);
        libffi::raw::ffi_call(cif.as_raw_ptr(), Some(f), ret.ptr() as *mut _, ptrs.as_mut_ptr());
    }

    let mut parts = Vec::new();
    if let Some(rc) = &spec.ret {
        let rsz = size_of(&rc.ty);
        parts.push(read(rc, &ret.bytes()[..rsz]));
    }
    let n_ret = parts.len();
    for mp in &m.muts {
        parts.push(match mp.keep {
            None => mp.obj,
            Some(k) => {
                let bytes = &m.keep[k].0;
                let es = size_of(&mp.elem.ty);
                // SAFETY: this buffer was created in `Marsh::write` as Buf::new(len * es) with
                // the same len/es (mp.len = items.len(), es = size_of(elem)), so it holds
                // >= mp.len * es initialised bytes; u8 alignment is 1; `m.keep` is not
                // mutated while `raw` is used.
                let raw = unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const u8, mp.len * es) };
                match mp.bqn {
                    Some(bq) => bytes_to_list(bq, raw),
                    None => mk_list((0..mp.len).map(|i| read(&mp.elem, &raw[i * es..(i + 1) * es])).collect()),
                }
            }
        });
    }
    if spec.ret_amp {
        return parts[0];
    }
    if m.muts.is_empty() {
        return if n_ret == 0 { B::m_c32(0) } else { parts[0] };
    }
    mk_list(parts)
}

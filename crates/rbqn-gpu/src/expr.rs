//! Lazy elementwise expression trees compiled to one WGSL kernel (Step 5).
//!
//! A tree is a DAG of `Expr` nodes shared through `Rc`. Codegen walks it once,
//! numbering unique nodes (CSE by pointer) and unique leaf buffers, so `a+a`
//! binds `a` once. Scalars are slots in a small storage buffer (`sc[1..]`,
//! `sc[0]` is n), never literals: `1+2×3+4×a` and `5+6×7+8×a` produce the same
//! source and share a pipeline. Pipelines are cached by the generated source.

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use crate::buffer::{ElementKind, GpuBuffer};
use crate::dispatch::{WORKGROUP_SIZE, grid_for};
use crate::pipeline::{PassBatch, PipelineCache, PipelineKey};

/// Unique nodes per tree; a larger tree is evaluated to a buffer first.
pub const MAX_NODES: usize = 32;
/// Distinct leaf buffers per tree (storage-binding budget: 8 leaves + sc + out
/// + partials stays within Metal's per-stage limit).
pub const MAX_LEAVES: usize = 8;
/// Workgroup cap of the fused map+reduce kernel, so partials fit one reduce pass.
pub const MAX_REDUCE_GROUPS: usize = 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Min,
    Max,
}

impl Op {
    pub fn from_name(s: &str) -> Option<Op> {
        Some(match s {
            "add" => Op::Add,
            "sub" => Op::Sub,
            "mul" => Op::Mul,
            "min" => Op::Min,
            "max" => Op::Max,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
            Op::Min => "min",
            Op::Max => "max",
        }
    }
    fn wgsl(self, a: &str, b: &str) -> String {
        match self {
            Op::Add => format!("{a} + {b}"),
            Op::Sub => format!("{a} - {b}"),
            Op::Mul => format!("{a} * {b}"),
            Op::Min => format!("min({a}, {b})"),
            Op::Max => format!("max({a}, {b})"),
        }
    }
    pub fn apply_i64(self, a: i64, b: i64) -> i64 {
        match self {
            Op::Add => a.wrapping_add(b),
            Op::Sub => a.wrapping_sub(b),
            Op::Mul => a.wrapping_mul(b),
            Op::Min => a.min(b),
            Op::Max => a.max(b),
        }
    }
}

pub enum Expr {
    /// A device buffer (I32 or I64), read at the element index.
    Leaf(Arc<GpuBuffer>),
    /// An i32 scalar, bound through a storage slot.
    Scalar(i32),
    /// The element index itself (`↕n` never uploaded).
    Iota,
    Bin(Op, Rc<Expr>, Rc<Expr>),
    /// A host array not uploaded yet (opaque to this crate). The runtime
    /// replaces it with a `Leaf` before `run`, or evaluates the tree on the
    /// host without ever uploading it.
    Host(Rc<dyn std::any::Any>),
}

/// Unique node and leaf counts of a tree.
pub fn counts(e: &Rc<Expr>) -> (usize, usize) {
    let mut seen: Vec<*const Expr> = Vec::new();
    let mut leaves: Vec<*const GpuBuffer> = Vec::new();
    fn go(e: &Rc<Expr>, seen: &mut Vec<*const Expr>, leaves: &mut Vec<*const GpuBuffer>) {
        let p = Rc::as_ptr(e);
        if seen.contains(&p) {
            return;
        }
        seen.push(p);
        match &**e {
            Expr::Leaf(b) => {
                let bp = Arc::as_ptr(b);
                if !leaves.contains(&bp) {
                    leaves.push(bp);
                }
            }
            Expr::Host(h) => {
                let bp = Rc::as_ptr(h) as *const GpuBuffer;
                if !leaves.contains(&bp) {
                    leaves.push(bp);
                }
            }
            Expr::Bin(_, l, r) => {
                go(l, seen, leaves);
                go(r, seen, leaves);
            }
            _ => {}
        }
    }
    go(e, &mut seen, &mut leaves);
    (seen.len(), leaves.len())
}

/// A tree flattened for codegen: the `ev` body, its bindings and a readable op string.
struct Flat {
    body: String,
    result: String,
    leaves: Vec<Arc<GpuBuffer>>,
    scalars: Vec<i32>,
    desc: String,
}

fn flatten(root: &Rc<Expr>, ty: &str) -> Flat {
    struct W<'a> {
        ty: &'a str,
        memo: HashMap<*const Expr, (String, String)>,
        leaves: Vec<Arc<GpuBuffer>>,
        scalars: Vec<i32>,
        body: String,
        k: usize,
    }
    impl W<'_> {
        fn go(&mut self, e: &Rc<Expr>) -> (String, String) {
            let p = Rc::as_ptr(e);
            if let Some(v) = self.memo.get(&p) {
                return v.clone();
            }
            let ty = self.ty;
            let (rhs, desc) = match &**e {
                Expr::Leaf(b) => {
                    let li = match self.leaves.iter().position(|x| Arc::ptr_eq(x, b)) {
                        Some(i) => i,
                        None => {
                            self.leaves.push(b.clone());
                            self.leaves.len() - 1
                        }
                    };
                    (format!("{ty}(lf{li}[i])"), format!("L{li}"))
                }
                Expr::Scalar(s) => {
                    self.scalars.push(*s);
                    let k = self.scalars.len();
                    (format!("{ty}(sc[{k}])"), format!("S{}", k - 1))
                }
                Expr::Iota => (format!("{ty}(i32(i))"), "iota".to_string()),
                Expr::Host(_) => panic!("rbqn-gpu: host leaf reached codegen (resolve it first)"),
                Expr::Bin(op, l, r) => {
                    let (lv, ld) = self.go(l);
                    let (rv, rd) = self.go(r);
                    (op.wgsl(&lv, &rv), format!("{}({ld},{rd})", op.name()))
                }
            };
            let v = format!("t{}", self.k);
            self.k += 1;
            self.body.push_str(&format!("    let {v}: {ty} = {rhs};\n"));
            self.memo.insert(p, (v.clone(), desc.clone()));
            (v, desc)
        }
    }
    let mut w = W { ty, memo: HashMap::new(), leaves: Vec::new(), scalars: Vec::new(), body: String::new(), k: 0 };
    let (result, desc) = w.go(root);
    Flat { body: w.body, result, leaves: w.leaves, scalars: w.scalars, desc }
}

fn ty_name(k: ElementKind) -> &'static str {
    match k {
        ElementKind::I64 => "i64",
        _ => "i32",
    }
}

fn identity(op: &str, acc: ElementKind) -> &'static str {
    match (op, acc) {
        ("add", ElementKind::I64) => "0li",
        ("mul", ElementKind::I64) => "1li",
        ("min", ElementKind::I64) => "9223372036854775807li",
        ("max", ElementKind::I64) => "(-9223372036854775807li - 1li)",
        ("add", _) => "0i",
        ("mul", _) => "1i",
        ("min", _) => "2147483647i",
        ("max", _) => "(-2147483647i - 1i)",
        _ => panic!("rbqn-gpu: fused reduce op {op:?}"),
    }
}

fn generate(f: &Flat, ty: &str, reduce: Option<(&str, ElementKind)>) -> String {
    let mut s = String::new();
    s.push_str("@group(0) @binding(0) var<storage, read> sc: array<i32>;\n");
    for (i, b) in f.leaves.iter().enumerate() {
        s.push_str(&format!(
            "@group(0) @binding({}) var<storage, read> lf{i}: array<{}>;\n",
            i + 1,
            ty_name(b.element_type())
        ));
    }
    let ob = f.leaves.len() + 1;
    s.push_str(&format!("@group(0) @binding({ob}) var<storage, read_write> out: array<{ty}>;\n"));
    if let Some((op, acc)) = reduce {
        let at = ty_name(acc);
        s.push_str(&format!("@group(0) @binding({}) var<storage, read_write> part: array<{at}>;\n", ob + 1));
        s.push_str(&format!("var<workgroup> sh: array<{at}, 256>;\n"));
        let comb = Op::from_name(op).unwrap().wgsl("a", "b");
        s.push_str(&format!("fn comb(a: {at}, b: {at}) -> {at} {{ return {comb}; }}\n"));
    }
    s.push_str(&format!("fn ev(i: u32) -> {ty} {{\n{}    return {};\n}}\n", f.body, f.result));
    s.push_str(
        "@compute @workgroup_size(256)\nfn main(@builtin(local_invocation_id) lid: vec3<u32>, \
         @builtin(workgroup_id) wid: vec3<u32>, @builtin(num_workgroups) nwg: vec3<u32>) {\n    let n = u32(sc[0]);\n",
    );
    match reduce {
        None => s.push_str(
            "    let i = (wid.y * nwg.x + wid.x) * 256u + lid.x;\n    if (i < n) { out[i] = ev(i); }\n}\n",
        ),
        Some((op, acc)) => {
            let at = ty_name(acc);
            s.push_str(&format!(
                "    let stride = nwg.x * 256u;\n    var acc: {at} = {id};\n\
                 \x20   for (var i = wid.x * 256u + lid.x; i < n; i = i + stride) {{\n\
                 \x20       let v = ev(i);\n        out[i] = v;\n        acc = comb(acc, {at}(v));\n    }}\n\
                 \x20   sh[lid.x] = acc;\n    workgroupBarrier();\n\
                 \x20   for (var st = 128u; st > 0u; st = st >> 1u) {{\n\
                 \x20       if (lid.x < st) {{ sh[lid.x] = comb(sh[lid.x], sh[lid.x + st]); }}\n\
                 \x20       workgroupBarrier();\n    }}\n\
                 \x20   if (lid.x == 0u) {{ part[wid.x] = sh[0]; }}\n}}\n",
                id = identity(op, acc)
            ));
        }
    }
    s
}

/// Interns generated sources as `&'static str` shader ids (the cache keys modules by id).
fn shader_id(src: &str) -> (&'static str, bool) {
    static IDS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let mut m = IDS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(id) = m.get(src) {
        return (id, false);
    }
    let id: &'static str = Box::leak(format!("fused_{}", m.len()).into_boxed_str());
    m.insert(src.to_string(), id);
    (id, true)
}

/// What a fused dispatch did, for logging.
pub struct FusedRun {
    pub desc: String,
    /// The WGSL source when this tree shape was compiled for the first time.
    pub new_source: Option<String>,
    /// Per-workgroup partials (reduce mode only).
    pub partials: Option<GpuBuffer>,
}

/// Evaluate `root` over n elements into `out` (I32 or I64, length n), in one
/// dispatch. With `reduce = Some((op, acc_kind))` the same kernel also writes
/// per-workgroup partials (≤ MAX_REDUCE_GROUPS) for a final reduce.
pub fn run(
    device: &Arc<wgpu::Device>,
    queue: &wgpu::Queue,
    cache: &mut PipelineCache,
    root: &Rc<Expr>,
    n: usize,
    out: &GpuBuffer,
    reduce: Option<(&str, ElementKind)>,
) -> FusedRun {
    assert!(n > 0 && n < i32::MAX as usize, "fused: bad length {n}");
    assert_eq!(out.len(), n);
    let ty = ty_name(out.element_type());
    let f = flatten(root, ty);
    let src = generate(&f, ty, reduce);
    let (id, fresh) = shader_id(&src);
    let pipeline = cache.get_or_create(&PipelineKey::raw(id, "main"), &src);

    let mut words: Vec<i32> = Vec::with_capacity(f.scalars.len() + 1);
    words.push(n as i32);
    words.extend_from_slice(&f.scalars);
    let sc = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("fused_sc"),
        size: (words.len() * 4) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&sc, 0, bytemuck::cast_slice(&words));

    let (groups, grid) = match reduce {
        None => (0, grid_for(n, WORKGROUP_SIZE)),
        Some(_) => {
            let g = n.div_ceil(WORKGROUP_SIZE as usize * 4).clamp(1, MAX_REDUCE_GROUPS);
            (g, (g as u32, 1))
        }
    };
    let partials = reduce.map(|(_, acc)| GpuBuffer::storage(device, acc, groups));

    let mut entries = vec![wgpu::BindGroupEntry { binding: 0, resource: sc.as_entire_binding() }];
    for (i, b) in f.leaves.iter().enumerate() {
        entries.push(wgpu::BindGroupEntry { binding: i as u32 + 1, resource: b.inner().as_entire_binding() });
    }
    let ob = f.leaves.len() as u32 + 1;
    entries.push(wgpu::BindGroupEntry { binding: ob, resource: out.inner().as_entire_binding() });
    if let Some(p) = &partials {
        entries.push(wgpu::BindGroupEntry { binding: ob + 1, resource: p.inner().as_entire_binding() });
    }
    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });
    let mut batch = PassBatch::new(device);
    batch.push(pipeline, &bg, grid);
    batch.submit(queue);
    FusedRun { desc: f.desc, new_source: fresh.then_some(src), partials }
}

/// Host evaluation of element `i` (fallback when the device path fails).
pub fn eval_host(e: &Expr, i: usize, leaf: &dyn Fn(&Arc<GpuBuffer>) -> i64) -> i64 {
    match e {
        Expr::Leaf(b) => leaf(b),
        Expr::Scalar(s) => *s as i64,
        Expr::Iota => i as i64,
        Expr::Bin(op, l, r) => op.apply_i64(eval_host(l, i, leaf), eval_host(r, i, leaf)),
        Expr::Host(_) => panic!("rbqn-gpu: eval_host on a host leaf"),
    }
}

/// Leaf buffers of a tree, deduplicated.
pub fn leaves(root: &Rc<Expr>) -> Vec<Arc<GpuBuffer>> {
    flatten(root, "i64").leaves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_are_slots_not_literals() {
        let t = |a: i32, b: i32| {
            Rc::new(Expr::Bin(Op::Add, Rc::new(Expr::Scalar(a)), Rc::new(Expr::Bin(Op::Mul, Rc::new(Expr::Scalar(b)), Rc::new(Expr::Iota)))))
        };
        let (f1, f2) = (flatten(&t(1, 2), "i32"), flatten(&t(5, 6), "i32"));
        assert_eq!(generate(&f1, "i32", None), generate(&f2, "i32", None));
        assert_eq!(f1.scalars, vec![1, 2]);
        assert_eq!(f1.desc, "add(S0,mul(S1,iota))");
    }

    #[test]
    fn shared_nodes_emit_once() {
        let a = Rc::new(Expr::Iota);
        let s = Rc::new(Expr::Bin(Op::Add, a.clone(), a));
        let t = Rc::new(Expr::Bin(Op::Mul, s.clone(), s));
        assert_eq!(counts(&t), (3, 0));
        let f = flatten(&t, "i32");
        assert_eq!(f.body.matches("let ").count(), 3);
    }
}

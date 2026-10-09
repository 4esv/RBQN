pub mod arith;
pub mod reduce;
pub mod scan;
pub mod sort;
pub mod select;
pub mod unary;
pub mod matmul;
pub mod softmax;
pub mod int64;
pub mod arith_i64;
pub mod reduce_i64;
pub mod scan_i64;
pub mod minmax;

use crate::buffer::ElementKind;
use crate::pipeline::PipelineKey;

/// Pipelines the runtime compiles ahead of time, in the order the integer
/// paths reach them: i32 scan, reduce, arith and scalar arith; with
/// SHADER_INT64 the i64 promotions (fold `+´` past 2^31 runs reduce_i64 add
/// on I32 then I64 levels; `+`` past 2^31 runs scan_i64 add; scalar `+`/`×`
/// past 2^31 runs arith_scalar_i64_r); the radix sort passes; minmax
/// (bound refinement). Rarer i64 variants (min/max folds, array-array i64
/// arith, scalar-left i64) stay lazy: compiling every variant cost ~1 ms of
/// wall time through contention with the main thread.
pub fn precompile_set(int64_ok: bool) -> Vec<(PipelineKey, String)> {
    let mut v: Vec<(PipelineKey, String)> = Vec::new();
    let mut raw = |id: &'static str, entry: &str, src: &str| v.push((PipelineKey::raw(id, entry), src.to_string()));
    let (i32k, i64k) = (ElementKind::I32, ElementKind::I64);
    raw("scan_i32", "block_sum", scan::SHADER_I32);
    raw("scan_i32", "scan_incl", scan::SHADER_I32);
    for op in ["add", "max", "min"] {
        raw("reduce_i32", &format!("reduce_{op}_i32"), reduce::SHADER_I32);
    }
    for op in ["add", "sub", "mul"] {
        raw("arith_i32", &format!("{op}_i32"), arith::SHADER_I32);
    }
    for op in ["add", "sub", "mul", "rsub", "min", "max"] {
        raw("arith_scalar_i32", &format!("scalar_{op}_i32"), arith::SHADER_SCALAR_I32);
    }
    if int64_ok {
        for k in [i32k, i64k] {
            let id = int64::shader_id("reduce_i64", "add", k);
            raw(id, "main", &int64::expand(reduce_i64::TEMPLATE, "add", k));
        }
    }
    for e in ["hist", "scan", "scatter"] {
        raw("sort", e, sort::SHADER);
    }
    if int64_ok {
        for k in [i32k, i64k] {
            let id = int64::shader_id("scan_i64", "add", k);
            let src = int64::expand(scan_i64::TEMPLATE, "add", k)
                .replace("@EPT@", &scan_i64::EPT.to_string())
                .replace("@TILE@", &scan_i64::BLOCK.to_string());
            raw(id, "block_sum", &src);
            raw(id, "scan_block", &src);
        }
        for op in ["add", "mul"] {
            let id = int64::shader_id("arith_scalar_i64_r", op, i32k);
            let src = int64::expand(arith_i64::SCALAR_TEMPLATE, op, i32k).replace("@ARGS@", "v, s");
            raw(id, "main", &src);
        }
    }
    raw("minmax_i32", "minmax_first", minmax::SOURCE);
    raw("minmax_i32", "minmax_pairs", minmax::SOURCE);
    v
}

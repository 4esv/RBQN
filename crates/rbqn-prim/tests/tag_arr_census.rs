//! Census of leaked array handles in rbqn-prim (#157, stage 2a of #150).
//!
//! Every array a primitive returns goes out as `PrimResult::Array(BqnArr)` and is
//! turned into an owned `Val` at the one conversion point (`PrimResult::into_out`).
//! The `tag_arr(` calls left here build *nested elements* (a `B` stored inside a
//! parent array, or a recursion helper whose `B` result becomes such an element),
//! so their owner is the parent array, not the primitive. Stage 2b/2c retire them.
//! Adding a new `tag_arr(` to rbqn-prim fails this test until it is listed with a reason.

use std::fs;
use std::path::Path;

/// (file, count, reason)
const ALLOWED: &[(&str, usize, &str)] = &[
    ("arith_dyad.rs", 2, "pervasive recursion over boxed elements: results become elements of the parent array"),
    ("arith_monad.rs", 2, "pervasive_monad_b recursion over boxed elements: results become elements of the parent array"),
    ("compare.rs", 4, "cmp_pervasive_b recursion over boxed elements (Result<B> helper)"),
    ("group.rs", 13, "group cells and fill prototypes are stored as elements of the result array"),
    ("select.rs", 3, "pick_elem (Result<B> helper for boxed-index picks) builds elements of the result"),
    ("structural.rs", 4, "box_arr / prototype_of_arr build fill prototypes and enclosed elements (B-typed helpers)"),
];

fn count(src: &str) -> usize {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .map(|l| l.matches("tag_arr(").count())
        .sum::<usize>()
}

#[test]
#[cfg_attr(miri, ignore = "reads source files")]
fn tag_arr_only_where_allowlisted() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut seen = 0;
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let n = count(&fs::read_to_string(&path).unwrap());
        let allowed = ALLOWED.iter().find(|a| a.0 == name).map(|a| a.1).unwrap_or(0);
        assert_eq!(n, allowed, "{name}: {n} `tag_arr(` sites, allowlist says {allowed}; convert to an owned result or list it with a reason");
        seen += n;
    }
    assert_eq!(seen, ALLOWED.iter().map(|a| a.1).sum::<usize>());
}

#[test]
#[cfg_attr(miri, ignore = "reads source files")]
fn dispatch_owns_created_arrays() {
    let src = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dispatch.rs")).unwrap();
    assert!(src.contains("tag_arr_owned(a)"), "PrimResult::Array must become an owned Val");
}

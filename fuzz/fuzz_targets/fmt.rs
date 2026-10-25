#![no_main]
//! Numeric and character arrays through `•Fmt`, `•Repr` and `•BQN`.
use libfuzzer_sys::fuzz_target;
use rbqn_fuzz::*;

fuzz_target!(|data: &[u8]| {
    rbqn_fuzz::tick();
    let mut c = Cursor(data, 0);
    let b = bqn();
    let rank = c.next() % 3;
    let shape: Vec<usize> = (0..rank).map(|_| (c.next() % 5) as usize).collect();
    let n: usize = shape.iter().product();
    let v = if c.next() % 2 == 0 {
        // Numbers: 8 raw bytes each as f64 (NaN has no BQN literal, so it becomes 0).
        let xs: Vec<f64> = (0..n)
            .map(|_| {
                let mut a = [0u8; 8];
                for x in &mut a { *x = c.next(); }
                let f = f64::from_le_bytes(a);
                if f.is_nan() { 0.0 } else { f }
            })
            .collect();
        b.array_f64(&shape, &xs).unwrap()
    } else {
        // Characters: build the source of a reshaped string from arbitrary scalar values.
        let s: String = (0..n)
            .map(|_| {
                let u = u32::from_le_bytes([c.next(), c.next(), c.next(), c.next() & 0x1]) % 0x11_0000;
                char::from_u32(u).unwrap_or('a')
            })
            .collect();
        let f = func("{𝕨⥊𝕩}");
        let sh = b.list_f64(&shape.iter().map(|&d| d as f64).collect::<Vec<_>>());
        let sh = if shape.is_empty() { b.list_f64(&[]) } else { sh };
        match b.call2(&f, &sh, &b.string(&s)) { Ok(v) => v, Err(_) => return }
    };
    let _ = v.fmt();
    let fmt = func("•Fmt");
    let repr = func("•Repr");
    let ev = func("•BQN");
    let _ = b.call1(&fmt, &v);
    if let Ok(r) = b.call1(&repr, &v) {
        match b.call1(&ev, &r) {
            Ok(back) => {
                let m = func("≡");
                let ok = b.call2(&m, &v, &back).map(|t| t.to_f64() == Some(1.0)).unwrap_or(false);
                assert!(ok, "•BQN•Repr round trip differs: {}", r.to_string_lossy().unwrap_or_default());
            }
            Err(e) => panic!("•BQN of •Repr output failed: {e}"),
        }
    }
});

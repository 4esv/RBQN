//! Shared setup for the rbqn fuzz targets.
//!
//! BQN errors travel as `panic_any(BqnError)` and the embed API catches them.
//! Anything else that unwinds (`panic!`, `unwrap`, index out of bounds, ...) is
//! a bug in a primitive: the embed API would turn it into a BQN error too, which
//! would hide it from the fuzzer, so a hook installed after warm-up aborts on it.

use rbqn::Bqn;
use rbqn_core::error::BqnError;

thread_local! {
    static BQN: &'static Bqn = {
        let b: &'static Bqn = Box::leak(Box::new(Bqn::new().expect("bootstrap")));
        // Warm-up eval installs the embed API's quiet hook first; ours goes on top.
        let _ = b.eval("1+1");
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let p = info.payload();
            if p.downcast_ref::<BqnError>().is_none() {
                let _ = &prev;
                let msg = p
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| p.downcast_ref::<&str>().copied())
                    .unwrap_or("<non-string payload>");
                eprintln!("rbqn-fuzz: Rust panic inside rbqn (not a BQN error): {msg} at {:?}", info.location());
                std::process::abort();
            }
        }));
        b
    };
}

/// The per-process interpreter (`Bqn` is `!Send`, so one per thread; libFuzzer is single-threaded).
pub fn bqn() -> &'static Bqn {
    BQN.with(|b| *b)
}

/// A compiled BQN function, evaluated once per process (each `eval` costs a compiler run).
pub fn func(src: &str) -> rbqn::Value<'static> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, rbqn::Value<'static>>> = Default::default();
    }
    CACHE.with(|c| {
        *c.borrow_mut().entry(src.to_string()).or_insert_with(|| bqn().eval(src).expect(src))
    })
}

/// Byte stream cursor: reads wrap to a zero byte once the input runs out.
pub struct Cursor<'a>(pub &'a [u8], pub usize);

impl Cursor<'_> {
    pub fn next(&mut self) -> u8 {
        let b = self.0.get(self.1).copied().unwrap_or(0);
        self.1 += 1;
        b
    }
    pub fn pick<'t, T>(&mut self, xs: &'t [T]) -> &'t T {
        &xs[self.next() as usize % xs.len()]
    }
    pub fn done(&self) -> bool {
        self.1 >= self.0.len()
    }
}

// Same lists as bench/stranger/grid.py (kept in sync by hand; fuzz/check_lists.sh diffs them).
pub const FNS: &str = "+-×÷⋆√⌊⌈∧∨¬|≤<>≥=≠≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!";
pub const ARGS: &[&str] = &[
    "0", "1", "¯2", "2.5", "'a'", "\"abc\"", "⟨⟩", "1‿2‿3", "⟨1,\"a\",⟨2⟩⟩",
    "2‿3⥊↕6", "<5", "@", "↕0", "0‿1‿1", "1‿0⥊0",
];
pub const MODS: &[&str] = &["´", "˝", "`", "¨", "˘", "⌜", "⁼", "˜"];
pub const MOD_FNS: &str = "+-×⌊∾⋈≍⊣";
pub const MODS2: &[&str] = &["∘", "○", "⊸", "⟜", "⌾"];
pub const PAIRS: &[(&str, &str)] = &[("+", "-"), ("∾", "⌽"), ("⌽", "↕")];

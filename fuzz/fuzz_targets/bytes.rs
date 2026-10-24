#![no_main]
//! Arbitrary bytes through the tokenizer, parser and compiler (`•BQN`): only BQN errors allowed.
use libfuzzer_sys::fuzz_target;
use rbqn_fuzz::{bqn, func};

fuzz_target!(|data: &[u8]| {
    rbqn_fuzz::tick();
    let s = String::from_utf8_lossy(data);
    // System values are the one thing random text must not reach: •Exit, •file.* and friends.
    if s.contains('•') { return; }
    let b = bqn();
    let f = func("•BQN");
    let _ = b.call1(&f, &b.string(&s));
});

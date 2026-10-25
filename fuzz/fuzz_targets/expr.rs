#![no_main]
//! Grid-shaped expressions: the byte stream picks primitive, modifier and argument shapes
//! from the lists in bench/stranger/grid.py. Wrapped in `⎊`, formatted with `•Fmt`.
use libfuzzer_sys::fuzz_target;
use rbqn_fuzz::*;

fn arg(c: &mut Cursor, depth: u8) -> String {
    // A quarter of the time (and not too deep) the argument is itself a parenthesised expression.
    if depth < 2 && c.next() % 4 == 0 {
        return format!("({})", expr(c, depth + 1));
    }
    c.pick(ARGS).to_string()
}

fn prim(c: &mut Cursor) -> char {
    *c.pick(&FNS.chars().collect::<Vec<_>>())
}

fn expr(c: &mut Cursor, depth: u8) -> String {
    let kind = c.next() % 6;
    let w = if c.next() % 2 == 0 { Some(arg(c, depth)) } else { None };
    let f = match kind {
        0 | 1 => prim(c).to_string(),
        2 | 3 => format!("{}{}", c.pick(&MOD_FNS.chars().collect::<Vec<_>>()), c.pick(MODS)),
        _ => {
            let (f, g) = *c.pick(PAIRS);
            format!("{f}{}{g}", c.pick(MODS2))
        }
    };
    let x = arg(c, depth);
    match w {
        Some(w) => format!("{w} {f} {x}"),
        None => format!("{f} {x}"),
    }
}

fuzz_target!(|data: &[u8]| {
    rbqn_fuzz::tick();
    let mut c = Cursor(data, 0);
    let mut stmts = Vec::new();
    // One to three statements, so state from earlier ones can matter.
    for _ in 0..=(c.next() % 3) {
        stmts.push(expr(&mut c, 0));
        if c.done() { break; }
    }
    let last = stmts.pop().unwrap();
    let mut src = String::new();
    for s in &stmts {
        src.push_str(&format!("{{𝕊: {s}}}⎊{{𝕊: 0}} 0\n"));
    }
    src.push_str(&format!("{{𝕊: •Fmt {last}}}⎊{{𝕊: \"ERR\"}} 0"));
    let _ = bqn().eval(&src);
});

use rbqn::embedded::{decode_bytecode, BlockEntry, ObjectEntry};

fn dump_summary(path: &str) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("Can't read {path}: {e}"));
    let obc = decode_bytecode(&bytes);
    println!("{path}:");
    println!("  source_tag={:?}, bc={}, iarrs={}, objs={}, blocks={}, bodies={}",
        obc.source_tag, obc.bc.len(), obc.iarrs.len(), obc.objs.len(), obc.blocks.len(), obc.bodies.len());

    println!("  iarrs lengths (first 15): {:?}",
        obc.iarrs.iter().take(15).map(|a| a.len()).collect::<Vec<_>>());

    println!("  blocks (first 15):");
    for (i, b) in obc.blocks.iter().enumerate().take(15) {
        match b {
            BlockEntry::IArr(n) => {
                let data = &obc.iarrs[*n];
                println!("    [{i}] IArr({n}): {:?}", data);
            }
            BlockEntry::Info { typ, iarrs0_idx, data_idx } => {
                let mono = &obc.iarrs[*iarrs0_idx];
                let dyadic = &obc.iarrs[*data_idx];
                println!("    [{i}] Info(typ={typ}, mono={iarrs0_idx}={:?}, dyadic={data_idx}={:?})",
                    mono, dyadic);
            }
        }
    }

    println!("  bodies iarrs (first 15): {:?}",
        obc.bodies.iter().take(15).map(|&i| {
            format!("iarrs[{i}]={:?}", obc.iarrs.get(i).map(|a| a.as_slice()))
        }).collect::<Vec<_>>());

    let type0 = obc.objs.iter().filter(|e| matches!(e, ObjectEntry::Provide(_))).count();
    let type1 = obc.objs.iter().filter(|e| matches!(e, ObjectEntry::Runtime(_))).count();
    let type2 = obc.objs.iter().filter(|e| matches!(e, ObjectEntry::RuntimePrev(_))).count();
    let type3 = obc.objs.iter().filter(|e| matches!(e, ObjectEntry::Float(_))).count();
    let type6 = obc.objs.iter().filter(|e| matches!(e, ObjectEntry::IArr(_))).count();
    println!("  obj counts: Provide={type0} Runtime={type1} RuntimePrev={type2} Float={type3} IArr={type6}");

    // Show first few objs
    println!("  objs (first 10):");
    for (i, obj) in obc.objs.iter().enumerate().take(10) {
        match obj {
            ObjectEntry::Provide(n) => println!("    [{i}] Provide({n})"),
            ObjectEntry::Runtime(n) => println!("    [{i}] Runtime({n})"),
            ObjectEntry::RuntimePrev(n) => println!("    [{i}] RuntimePrev({n})"),
            ObjectEntry::Float(v) => println!("    [{i}] Float({v})"),
            ObjectEntry::IArr(n) => println!("    [{i}] IArr({n}) = {:?}", obc.iarrs.get(*n).map(|a| &a[..a.len().min(5)])),
            ObjectEntry::Char(c) => println!("    [{i}] Char({c})"),
            ObjectEntry::Str(s) => println!("    [{i}] Str(len={})", s.len()),
        }
    }
    println!();
}

fn main() {
    dump_summary("$HOME/Code/forks/RBQN/crates/rbqn/src/embedded/compiler.bin");
    dump_summary("$HOME/Code/forks/RBQN/crates/rbqn/src/embedded/self-compiled/compiler.bin");
}

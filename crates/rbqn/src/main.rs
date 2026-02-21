mod bootstrap;
mod embedded;

use rbqn_prim::dispatch::PrimitiveRegistry;

fn main() {
    let prims = PrimitiveRegistry::new();
    match bootstrap::bootstrap(&prims) {
        Ok(rt) => {
            if rt.compiler.is_nothing() {
                eprintln!("rbqn: bootstrap completed with native-only runtime (no compiler bytecode)");
            } else {
                eprintln!("rbqn: bootstrap completed with self-hosted compiler");
            }
        }
        Err(e) => {
            eprintln!("rbqn: bootstrap failed: {e}");
            std::process::exit(1);
        }
    }
}

// NOTE: This build script only checks that the embedded .bin bytecode files exist.
// To regenerate them from CBQN source:
//   CBQN_PATH=/path/to/cbqn cargo run --bin rbqn-gen --features gen-tools

use std::env;
use std::path::Path;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let embedded_dir = Path::new(&manifest_dir).join("src").join("embedded");

    let bin_files = ["runtime0.bin", "runtime1x.bin", "compiler.bin", "formatter.bin"];

    for name in bin_files {
        let path = embedded_dir.join(name);
        if !path.is_file() {
            panic!(
                "Missing embedded bytecode: src/embedded/{name}\n\
                 Run: CBQN_PATH=/path/to/cbqn cargo run --bin rbqn-gen --features gen-tools"
            );
        }
        println!("cargo:rerun-if-changed=src/embedded/{name}");
    }
}

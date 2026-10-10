// NOTE: This build script checks that the committed .bin bytecode files exist in bins/.
// To regenerate them using RBQN's own compiler:
//   BQN_SRC=/path/to/BQN/src cargo run --bin rbqn-gen --features gen-tools -- --self

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let bins_dir = PathBuf::from(&manifest_dir).join("../../bins");

    // CBQN_PATH deprecation warning
    if env::var("CBQN_PATH").is_ok() {
        println!("cargo:warning=CBQN_PATH is no longer needed — use `rbqn-gen --self` (with gen-tools feature) to regenerate bins");
    }
    println!("cargo:rerun-if-env-changed=CBQN_PATH");

    let bin_files = ["runtime0.bin", "runtime1x.bin", "compiler.bin", "formatter.bin"];

    for name in bin_files {
        let path = bins_dir.join(name);
        if !path.is_file() {
            panic!(
                "Missing bins/{name}\nRun: BQN_SRC=/path/to/BQN/src cargo run -p rbqn --features gen-tools --bin rbqn-gen -- --self"
            );
        }
        println!("cargo:rerun-if-changed=../../bins/{name}");
    }

    // Dev profile: also watch BQN_SRC for source changes
    let profile = env::var("PROFILE").unwrap_or_default();
    if profile != "release"
        && let Ok(bqn_src) = env::var("BQN_SRC")
    {
        println!("cargo:rerun-if-changed={bqn_src}");
    }
    println!("cargo:rerun-if-env-changed=BQN_SRC");
}

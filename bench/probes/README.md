# GPU probes

Standalone crate (own workspace, not built by the root `cargo build`).
Numbers from these are recorded in `../gpu-plan-2026-10.md`.

    cargo run --release --bin monkey -- 10000000   # exact i64 chain, end to end
    cargo run --release --bin floor                # sync and upload floors
    cargo run --release --bin dispatchbench -- 1000000
    cargo run --release --bin par                  # rayon reductions
    cargo run --release --bin sortb ; cargo run --release --bin sortd   # existing radix sort
    cargo run --release --bin feat                 # adapter features

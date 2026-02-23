# Stack Research

**Domain:** BQN interpreter — runtime bypass, GPU integration, self-hosting compiler
**Researched:** 2026-02-23
**Confidence:** HIGH (existing crates verified from Cargo.lock; new additions verified from docs.rs/crates.io)

---

## What This Covers

This milestone adds three capabilities on top of the existing working foundation:

1. **Runtime bypass (Option B)** — already partially implemented in `bootstrap.rs` (the `runtime_0` native mapping exists). The remaining work is purely in existing Rust, no new crates needed.
2. **GPU dispatch integration** — wiring `rbqn-gpu` into `rbqn-prim` primitive dispatch. The infrastructure crates (`wgpu`, `pollster`, `bytemuck`) are already in place.
3. **Self-hosting compiler** — embedding pre-compiled CBQN bytecode so `CBQN_PATH` is not required at build time. Purely a `build.rs` + `include_bytes!` + file management problem.

---

## Existing Stack (Verified, No Changes Needed)

| Technology | Version | Purpose | Status |
|------------|---------|---------|--------|
| `wgpu` | 24.0.5 | GPU compute via Metal/Vulkan/DX12 | In workspace, working |
| `bytemuck` | ^1 | Safe f32/i32 byte casting for GPU buffers | In workspace |
| `pollster` | 0.4 | `block_on` to bridge async wgpu init into sync VM | In `rbqn-gpu` |
| `rustyline` | 15.0.0 | REPL readline support | In `rbqn` |

These are correct choices. Do not change or upgrade them for this milestone.

---

## New Dependencies Needed

### For GPU Integration (rbqn-prim)

No new crates. The GPU dispatch path needs only what's already there:

- `rbqn-gpu` is already a dependency of `rbqn-prim` in `Cargo.toml`
- `std::sync::OnceLock` (stable since Rust 1.70, no crate needed) to hold the `GpuContext` singleton
- `pollster::block_on` (already in `rbqn-gpu`) to initialize the GPU context once at startup

The integration layer (array transfer `ArrData::F64` → `GpuBuffer<f32>` → `ArrData::F64`) is pure Rust using the existing `bytemuck` + `wgpu::device.poll(wgpu::Maintain::Wait)` blocking pattern already demonstrated in `rbqn-gpu/src/buffer.rs`.

### For Testing (dev dependencies only)

| Library | Version | Purpose | Crate |
|---------|---------|---------|-------|
| `criterion` | `0.8` | Statistical GPU vs CPU benchmarks; measure threshold decisions | `rbqn` |
| `insta` | `1` | Snapshot tests for primitive output and BQN expression results | `rbqn-vm`, `rbqn` |

**Why criterion over built-in bench:** Criterion gives statistical confidence intervals, which matters for the GPU threshold decision (currently hardcoded at 100K elements). It proves whether the threshold is correct.

**Why insta over assert_eq:** The BQN official test suite produces string output. Snapshot testing makes it easy to capture expected output once (from CBQN) and detect regressions. `assert_eq!` on 13 test files would require manually maintaining expected strings.

```toml
# Add to workspace [dev-dependencies] or per-crate:
criterion = { version = "0.8", features = ["html_reports"] }
insta = "1"
```

### For Self-Hosting (build.rs only)

No new crates. Self-hosting uses `include_bytes!` (standard library) with pre-generated `.bc` files committed to the repo. The existing `build.rs` structure handles this — the change is switching from requiring `CBQN_PATH` at build time to shipping committed bytecode files.

Concrete approach: generate bytecode once using CBQN, commit the `.bc` files to `crates/rbqn/src/bytecode/`, change `build.rs` to `include_bytes!` from those files when `CBQN_PATH` is not set. No new build tooling needed.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `std::sync::OnceLock` for GPU singleton | `once_cell` crate | `OnceLock` is stable stdlib since Rust 1.70; no extra dep needed |
| `pollster::block_on` (already present) | `tokio::runtime::Runtime::block_on` | Tokio is 450K lines; pollster is ~150 lines. The VM is sync and doesn't need a full async runtime |
| `criterion` 0.8 | `divan` (newer, faster compile) | criterion has better HTML report output for threshold analysis; divan is newer but less ecosystem support |
| `insta` snapshots | Shell script harness (`test_cbqn_compat.sh`) | insta integrates with `cargo test`; the existing shell harness is a good prototype but doesn't gate CI |
| Committed bytecode files | Re-generating bytecode in CI | Re-generation requires CBQN to be installed in CI; committed files make `cargo install rbqn` possible without CBQN |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `tokio` | Massive dep for a sync VM that only needs async for GPU init | `pollster::block_on` (already in use) |
| `async-std` | Same problem as tokio, wrong fit | `pollster::block_on` |
| `wgpu::Features::SHADER_F64` | Not universally supported (fails on many Metal devices) | f32 GPU path + f64 CPU fallback (already the project decision) |
| `rayon` for parallelism | Global Mutex stores (`ARR_STORE`, `DERIVED_STORE`) prevent threading | Deferred to future milestone after GC replaces stores |
| `build.rs` exec of CBQN at build time | Fails for `cargo install rbqn` users without CBQN | Commit generated bytecode files to repo |
| `lazy_static` crate | Deprecated in favor of stdlib `OnceLock` / `LazyLock` since Rust 1.80 | `std::sync::OnceLock` |

---

## GPU Integration Pattern

The integration requires exactly one new abstraction in `rbqn-prim`: a global GPU state holder. Use `std::sync::OnceLock`:

```rust
// In rbqn-prim/src/lib.rs or gpu_dispatch.rs:
use std::sync::OnceLock;
use rbqn_gpu::context::GpuContext;

static GPU_CTX: OnceLock<Option<GpuContext>> = OnceLock::new();

pub fn gpu_ctx() -> Option<&'static GpuContext> {
    GPU_CTX.get_or_init(|| {
        pollster::block_on(GpuContext::new())
    }).as_ref()
}
```

Then in hot primitive dispatch paths (`+`, `×`, `÷`, fold, scan):

```rust
// Example: dyadic add in arith_dyad.rs
pub fn add_c2(w: B, wa: Option<&BqnArr>, x: B, xa: Option<&BqnArr>) -> Result<PrimResult> {
    let len = xa.map_or(0, |a| a.ia());
    if len >= rbqn_gpu::dispatch::GPU_THRESHOLD {
        if let Some(ctx) = crate::gpu_ctx() {
            // GPU path: upload, dispatch, readback
            return gpu_add(ctx, w, wa, x, xa);
        }
    }
    // CPU fallback (existing impl)
    ...
}
```

The async download functions in `rbqn-gpu/src/buffer.rs` already use `device.poll(wgpu::Maintain::Wait)` for synchronous blocking — this is the correct pattern for a sync VM. No async executor needed in `rbqn-prim`.

---

## Self-Hosting Build Pattern

Current state: `build.rs` reads CBQN `gen/` directory and generates `embedded_bytecode.rs`.

Target state: Ship `crates/rbqn/src/bytecode/` directory with committed `.bc` files (generated once from CBQN). Update `build.rs` fallback:

```rust
// build.rs: when CBQN_PATH not set, use committed bytecode
if let Some(dir) = bc_dir {
    write_bytecode_from_gen(&mut code, &dir);  // existing
} else {
    // NEW: fall back to committed bytecode in src/bytecode/
    let committed = manifest_dir.join("src/bytecode");
    if committed.exists() {
        write_bytecode_from_committed(&mut code, &committed);
    } else {
        write_empty_bytecode(&mut code);  // existing
    }
}
```

The bytecode files are binary data embedded via `include_bytes!`. No runtime parsing. This is how all other BQN implementations distribute their self-hosted compiler (JS BQN, CBQN's bytecodeSubmodule build).

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `wgpu` 24.0.5 | Rust edition 2024, `bytemuck` ^1, `pollster` 0.4 | Already resolved in Cargo.lock, no conflicts |
| `criterion` 0.8 | Rust 1.88+ | Milestone targets current stable; verify MSRV if needed |
| `insta` 1.x | Rust 1.70+ | No conflicts with existing deps |
| `std::sync::OnceLock` | Rust 1.70+ (stable) | Edition 2024 in workspace satisfies this |

---

## Installation

```toml
# No new workspace dependencies needed for runtime functionality.
# Dev-only additions to workspace Cargo.toml:

[workspace.dev-dependencies]
criterion = { version = "0.8", features = ["html_reports"] }
insta = "1"
```

```toml
# Per-crate: add to [[bench]] targets in rbqn/Cargo.toml
[dev-dependencies]
criterion.workspace = true
insta.workspace = true

[[bench]]
name = "gpu_vs_cpu"
harness = false
```

---

## Sources

- Cargo.lock (verified): wgpu 24.0.5, pollster 0.4, bytemuck ^1, rustyline 15.0.0
- [criterion 0.8.2 on docs.rs](https://docs.rs/criterion/latest/criterion/) — latest version confirmed
- [pollster on GitHub](https://github.com/zesterer/pollster) — `block_on` pattern for wgpu sync bridge
- [wgpu device poll docs](https://docs.rs/wgpu/latest/wgpu/struct.Device.html) — `Maintain::Wait` for sync readback
- [BQN test/README.txt](https://github.com/mlochbaum/BQN/blob/master/test/README.txt) — test runner requires `•file.List`, `•file.Lines`, `•args`, `•Out`, `•BQN`, `•Repr`
- [CBQN src/README.md](https://github.com/dzaima/CBQN/blob/master/src/README.md) — self-hosting via embedded bytecode
- [OnceLock RFC stabilization](https://rust-lang.github.io/rfcs/2788-standard-lazy-types.html) — stable since Rust 1.70
- [insta snapshot testing](https://insta.rs/) — cargo-insta workflow
- proptest 1.9.0 (LOW confidence, not recommended here — property testing useful for future math validation work, not blocking)

---

*Stack research for: RBQN v2.0 — runtime bypass, GPU integration, self-hosting*
*Researched: 2026-02-23*

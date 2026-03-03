# Technology Stack

**Analysis Date:** 2026-02-21

## Languages

**Primary:**
- Rust (Edition 2024) - All application code across 5 workspace crates

**Secondary:**
- WGSL (WebGPU Shading Language) - GPU compute shaders in `crates/rbqn-gpu/src/shaders/*.wgsl`
- C (parsed, not compiled) - Build script parses CBQN-generated C source files to extract embedded bytecode

## Runtime

**Environment:**
- Rust 1.90.0+ (Edition 2024 requires nightly or recent stable; no `rust-toolchain.toml` present)
- Binary target: single CLI executable `rbqn`

**Package Manager:**
- Cargo (workspace)
- Lockfile: present but `.gitignore`d (library-style; reproducibility depends on Cargo.toml version bounds)

## Frameworks

**Core:**
- No web/application framework. This is a language interpreter implemented from scratch.

**Testing:**
- No test framework configured. Zero `#[test]` or `#[cfg(test)]` blocks exist in the codebase.

**Build/Dev:**
- Cargo workspace with resolver v2
- Custom `build.rs` in `crates/rbqn/` for compile-time bytecode embedding

## Workspace Structure

```
Cargo.toml (workspace root)
├── crates/rbqn/          # Main binary crate (CLI, REPL, bootstrap)
├── crates/rbqn-core/     # Core types: B (NaN-boxed value), BqnArr, error types
├── crates/rbqn-vm/       # VM: bytecode interpreter, blocks, scopes, compiler bridge
├── crates/rbqn-prim/     # Primitive function implementations (64 BQN primitives)
└── crates/rbqn-gpu/      # GPU compute acceleration via wgpu
```

**Dependency graph (internal crates):**
```
rbqn → rbqn-core, rbqn-vm, rbqn-prim, rbqn-gpu
rbqn-vm → rbqn-core, rbqn-prim
rbqn-prim → rbqn-core, rbqn-gpu
rbqn-gpu → rbqn-core
rbqn-core → (no internal deps)
```

## Key Dependencies

**Critical (direct):**
- `rustyline` 15.0.0 - REPL line editing with history (`crates/rbqn/src/repl.rs`)
- `wgpu` 24.0.5 - WebGPU API for GPU compute dispatch (`crates/rbqn-gpu/`)
- `bytemuck` 1.25.0 (with `derive` feature) - Safe transmutes for GPU buffer data
- `pollster` 0.4.0 - Async runtime bridge for wgpu's async device creation

**Infrastructure (transitive via wgpu):**
- `ash` 0.38.0 - Vulkan bindings (Linux/Windows backend)
- `metal` - Metal bindings (macOS backend, preferred via `cfg!(target_os = "macos")`)
- `naga` - WGSL shader compilation

**Zero external dependencies in core crates:**
- `rbqn-core` has no external dependencies (pure Rust)
- `rbqn-vm` has no external dependencies (depends only on internal crates)

## Build System

**Build Script:** `crates/rbqn/build.rs`
- Parses CBQN's pre-compiled C source files (`runtime0`, `runtime1`, `compiles`, `formatter`)
- Extracts integer arrays, object tables, block definitions, and body layouts
- Generates `embedded_bytecode.rs` with static data included via `include!()` at `crates/rbqn/src/embedded/mod.rs`
- Searches for CBQN source in this order:
  1. `$CBQN_PATH` environment variable
  2. `../cbqn-ref` or `../cbqn` relative to manifest dir
  3. `../../cbqn-ref` or `../../cbqn` relative to manifest dir
- Within CBQN, searches for gen files in:
  1. `build/bytecodeLocal/gen/`
  2. `build/bytecodeSubmodule/gen/`
  3. `build/obj/*/gen/`
- If no CBQN found, generates empty placeholders (bootstrap will not work)

**Generated components:**
- `RUNTIME0` - Core runtime (24 override functions)
- `RUNTIME1` - Full runtime (64 BQN runtime functions)
- `COMPILER` - Self-hosted BQN compiler
- `FORMATTER` - Self-hosted BQN formatter (optional)

## Configuration

**Environment Variables:**
- `CBQN_PATH` - Path to CBQN source tree (required for build if not adjacent). Used by build script only.
- `XDG_DATA_HOME` / `HOME` - REPL history file location (`$XDG_DATA_HOME/.rbqn_repl_history` or `$HOME/.rbqn_repl_history`)

**Build Configuration:**
- `Cargo.toml` (workspace) - Single configuration file, no features/profiles defined
- No `.rustfmt.toml`, `clippy.toml`, or `rust-toolchain.toml`
- No CI/CD configuration (`.github/workflows/` does not exist)

## Platform Requirements

**Development:**
- Rust toolchain with Edition 2024 support (1.85.0+ for stable, or nightly)
- CBQN source tree built with `make bytecodeLocal` (for bootstrap to function)
- GPU-capable system with wgpu-compatible driver (Metal on macOS, Vulkan on Linux/Windows) for GPU crate

**Production:**
- Single statically-linked binary (`rbqn`)
- No runtime dependencies beyond system libraries (GPU drivers optional)
- CLI modes: REPL (`-r`), eval (`-e`), print (`-p`), output (`-o`), file execution, stdin (`-`)

## Build Commands

```bash
# Standard build (requires CBQN_PATH or adjacent CBQN source)
CBQN_PATH=/path/to/CBQN cargo build

# Run with eval
CBQN_PATH=/path/to/CBQN cargo run -- -p "2+3"

# Build without bootstrap (compiles but runtime won't work)
cargo build
```

---

*Stack analysis: 2026-02-21*

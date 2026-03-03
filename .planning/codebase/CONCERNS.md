# Codebase Concerns

**Analysis Date:** 2026-02-21

## Tech Debt

**Global Mutable Stores with No GC (Memory Leak by Design):**
- Issue: Three global `LazyLock<Mutex<HashMap>>` stores hold all heap-allocated objects forever. Arrays (`ARR_STORE`), derived functions (`DERIVED_STORE`), and namespaces (`NS_STORE`) are inserted but never removed. Every `tag_arr()`, `store_derived()`, and `store_ns()` call monotonically increases memory usage.
- Files: `crates/rbqn-core/src/arrstore.rs` (lines 8-11), `crates/rbqn-vm/src/derive.rs` (lines 35-56), `crates/rbqn-vm/src/namespace.rs` (lines 22-31)
- Impact: Any non-trivial BQN program or REPL session will eventually exhaust memory. The bootstrap alone creates thousands of objects that are never freed.
- Fix approach: Implement reference counting or a tracing GC. The NaN-boxing scheme stores object IDs in the lower 48 bits of tagged values; a mark-and-sweep collector could scan all reachable `B` values (scopes, stacks, stored arrays) and remove unreferenced entries from the global maps. Alternatively, switch to `Arc`-based direct pointers stored in the NaN-box payload (requires careful bit-width management).

**Error Handling via `panic!` Instead of `Result`:**
- Issue: `rbqn_core::error::throw()` and friends call `panic!()` to signal BQN errors. The VM and primitive dispatch layers use `unwrap_or_else(|| throw(...))` pervasively (~139 `panic`/`unwrap`/`throw` sites across 16 files). The `⎊` (Catch) modifier relies on `std::panic::catch_unwind(AssertUnwindSafe(...))` to catch these panics.
- Files: `crates/rbqn-core/src/error.rs` (lines 30-48), `crates/rbqn-vm/src/modifiers.rs` (lines 471-483), `crates/rbqn/src/bootstrap.rs` (line 422), `crates/rbqn/src/main.rs` (line 159)
- Impact: Unwinding through FFI boundaries is undefined behavior. `AssertUnwindSafe` suppresses the compiler's unwind safety checks but does not actually make the code unwind-safe -- mutexes may be poisoned, Arc reference counts may leak, and the global stores may be left in inconsistent states after a caught panic. Performance cost of unwinding is also significant vs. `Result` propagation.
- Fix approach: Convert the VM evaluation loop and `c1`/`c2` dispatch to return `Result<B, BqnError>`. This is a large refactor touching every primitive and modifier call site. A phased approach: first wrap `c1`/`c2` to return `Result`, then propagate outward. Replace `catch_unwind` in `⎊` with `Result`-based error handling.

**Fork Monadic Dispatch Bug:**
- Issue: `c1` for `DerivedKind::Fork` incorrectly calls `c2(d.f, B::m_f64(0.0), gx)` instead of the correct `c2(d.f, c1(d.f, x), c1(d.g, c1(d.h, x)))` pattern. The code has a `// TODO: proper fork c1` comment. A monadic fork `(f g h) x` should compute `(f x) g (h x)`.
- Files: `crates/rbqn-vm/src/derive.rs` (line 205)
- Impact: Any BQN expression using monadic forks (extremely common pattern) produces wrong results.
- Fix approach: Replace line 205 with the correct fork evaluation: `let hx = c1(d.h, x); let fx = c1(d.f, x); c2(d.g, fx, hx)`.

**Heap Limit Not Enforced:**
- Issue: CLI accepts `-M` flag for heap limit but the value is discarded with `let _ = args.heap_max; // TODO: enforce heap limit`.
- Files: `crates/rbqn/src/main.rs` (line 33)
- Impact: No memory protection for runaway programs.
- Fix approach: Track total allocation size in `ARR_STORE`/`DERIVED_STORE` and check against limit before each insertion.

**`get_arr` Clones Entire Array on Every Access:**
- Issue: `get_arr(b)` locks the global `ARR_STORE` mutex, looks up the ID, and calls `.cloned()` to return `Option<BqnArr>`. Every array access -- including reading a single element -- clones the full `BqnArr` (which includes `Vec<T>` data). The VM does this constantly: `get_arr(x)` appears in every primitive call path.
- Files: `crates/rbqn-core/src/arrstore.rs` (lines 19-25)
- Impact: O(n) memory allocation and copy for every O(1) element access. Quadratic blowup for operations like `¨` (Each) that iterate over array elements. This is the single largest performance bottleneck.
- Fix approach: Return `Arc<BqnArr>` instead of cloning. Store `Arc<BqnArr>` in the global map, and return cloned `Arc` references. Alternatively, store arrays directly behind tagged pointers (like CBQN) to avoid the HashMap lookup entirely.

**REPL Scope Extension Not Implemented:**
- Issue: The compiler has a placeholder for REPL scope extension (allowing new variables to be defined across REPL lines) with `// TODO: implement scope extension for REPL`.
- Files: `crates/rbqn-vm/src/compiler.rs` (line 193)
- Impact: REPL sessions cannot share state between lines. Each line is evaluated in isolation.
- Fix approach: Implement the scope extension mechanism matching CBQN's `ScopeExt` approach, where the REPL scope grows to accommodate new variables.

## Known Bugs

**`ArrData::Bit` Length Calculation is Incorrect:**
- Symptoms: `ArrData::len()` for `Bit` variant returns `v.len() * 64` which overestimates the actual element count. The comment says "overestimate; real length from shape" but code paths that call `data.len()` directly will get wrong values.
- Files: `crates/rbqn-core/src/array.rs` (lines 46-49)
- Trigger: Any operation that checks `data.len()` instead of using `BqnArr::ia()` (which correctly computes from shape).
- Workaround: Always use `BqnArr::ia()` for element count.

**DYNM Opcode Returns SENTINEL:**
- Symptoms: The `DYNM` (dynamic mutable reference) opcode pushes `B::SENTINEL` as a placeholder with the comment "for now treat as read-only". Any BQN code that tries to assign to a dynamic variable will silently fail.
- Files: `crates/rbqn-vm/src/vm.rs` (lines 371-374)
- Trigger: Assignment to system-value-like variables in the bootstrap or user code.
- Workaround: None.

**ALIM Opcode Silently Discards Operand:**
- Symptoms: The `ALIM` (array limit) opcode pops one value and reads a GID but does nothing. Comment says "currently just consume the operand and pass through."
- Files: `crates/rbqn-vm/src/vm.rs` (lines 599-603)
- Trigger: Namespace field limiting syntax in BQN.
- Workaround: None -- functionality is missing.

## Security Considerations

**`unsafe` Transmute for Opcode Parsing:**
- Risk: `Op::from_u32` uses `unsafe { std::mem::transmute(v) }` to convert a `u32` to the `Op` enum. The guard `v < Op::BC_SIZE as u32` prevents out-of-range values, but any gap in the enum discriminant values between `ALIM` (0x42) and `EXTO` (0x50) means values 0x43-0x4F would transmute to invalid enum variants, triggering undefined behavior.
- Files: `crates/rbqn-vm/src/bytecode.rs` (line 83)
- Current mitigation: The check `v < Op::BC_SIZE as u32` catches values above the enum range. However, gap values within the range are not caught.
- Recommendations: Replace transmute with an exhaustive `match` statement or use a crate like `num_enum` with `TryFromPrimitive`. Alternatively, use a const lookup table `[Option<Op>; BC_SIZE]`.

**`unsafe` Pointer Dereference in Compiler:**
- Risk: The compiler uses `unsafe { &*Arc::as_ptr(p) }` to traverse parent scope chains without incrementing the reference count. This is correct only if the `Arc` is guaranteed to live long enough, but scope lifetimes in BQN with closures can be complex.
- Files: `crates/rbqn-vm/src/compiler.rs` (line 321)
- Current mitigation: The scope chain is only traversed during compilation, not at runtime, which limits the exposure.
- Recommendations: Replace with safe `Arc::clone()` and dereference. The performance cost of cloning an `Arc` is negligible compared to the compilation cost.

**Panic Safety with Global Mutex State:**
- Risk: If a panic occurs while a global store mutex is locked (e.g., during `tag_arr()` inside a primitive that also panics), the mutex becomes poisoned. Subsequent operations on the same store will also panic, causing cascading failures.
- Files: All files using `ARR_STORE.lock().unwrap()`, `DERIVED_STORE.lock().unwrap()`, `NS_STORE.lock().unwrap()`
- Current mitigation: None.
- Recommendations: Use `lock().unwrap_or_else(|e| e.into_inner())` to recover from poisoned mutexes, or switch to the `parking_lot` crate which does not poison mutexes.

## Performance Bottlenecks

**HashMap Lookup on Every Value Access:**
- Problem: Every array read, derived function call, or namespace access requires: lock global mutex, HashMap lookup by ID, clone the value, unlock mutex. For hot paths like `c1`/`c2` dispatch and `¨` (Each) iteration, this dominates runtime.
- Files: `crates/rbqn-core/src/arrstore.rs`, `crates/rbqn-vm/src/derive.rs` (lines 47-50)
- Cause: NaN-boxing stores an integer ID in the payload rather than a pointer. This requires an indirection through a global registry.
- Improvement path: Store raw `Arc<T>` pointers directly in the NaN-box payload. The lower 48 bits of a tagged pointer on x86-64 are sufficient for heap addresses. This eliminates the HashMap entirely and matches CBQN's approach where tagged values contain direct pointers.

**Scope Cloning on Every Block Entry:**
- Problem: `exec_block` and `exec_block_with_args` create `Arc::new(Scope::new(..., Some(Arc::new(psc.clone()))))` on every function call. This clones the parent scope (including its `vars` Vec) even though the child scope only needs a reference to it.
- Files: `crates/rbqn-vm/src/vm.rs` (lines 12-22)
- Cause: The `Scope` struct is `Clone`-derived and contains `Vec<B>` which does a full allocation + copy.
- Improvement path: Pass `Arc<Scope>` directly to child scopes without cloning the inner `Scope`. The current `Some(Arc::new(psc.clone()))` should be `Some(psc_arc.clone())` where `psc_arc` is already an `Arc<Scope>`.

**`Arc::make_mut` on Every Variable Write:**
- Problem: `v_set` and the `SETNi`/`SETUi` opcodes call `Arc::make_mut(&mut pscs[d])` which clones the entire scope if the reference count is >1. In deeply nested closures this causes O(n) copies per variable assignment.
- Files: `crates/rbqn-vm/src/vm.rs` (lines 329, 412-467), `crates/rbqn-vm/src/scope.rs` (lines 91-139)
- Cause: Using `Arc` for scopes means shared references require COW semantics.
- Improvement path: Use `Rc<RefCell<Scope>>` for single-threaded execution (which is the only supported mode). Eliminates atomic operations and enables mutation without cloning.

**No Typed Array Dispatch in Primitives:**
- Problem: Many structural operations (join, couple, take, drop, reverse, rotate, transpose) always produce `ArrData::Boxed(Vec<B>)` output regardless of input type. An operation on an `I32` array creates a boxed array where every element is individually NaN-boxed, losing the compact typed representation.
- Files: `crates/rbqn-prim/src/structural.rs` (throughout -- every function returns `ArrData::Boxed`)
- Cause: Implementing typed dispatch for all 9 `ArrData` variants is tedious. The current approach boxes everything for simplicity.
- Improvement path: Add typed fast-paths for common types (at minimum `I32` and `F64`). Use `squeeze_num` on output to re-compact results. Consider a macro to generate typed variants.

## Fragile Areas

**Bootstrap Provide Array Mapping:**
- Files: `crates/rbqn/src/bootstrap.rs` (lines 80-200)
- Why fragile: The `build_provide()` function maps 40 named provide slots to specific `fruntime` indices and system functions. The mapping was manually derived from CBQN's `load.c` and the comment block at lines 120-128 shows a previous incorrect mapping that was caught and corrected mid-function (the code has `provide.pop()` and `provide.clear()` from an iterative debugging session). Any change to CBQN's provide ordering or fruntime layout will silently produce wrong behavior.
- Safe modification: When updating to a new CBQN bytecode version, cross-reference each provide index against CBQN's `builtins.h` and `load.c`. Add automated verification tests that check each provide entry produces the correct behavior.
- Test coverage: No automated tests exist for the provide mapping.

**Build Script C Parser:**
- Files: `crates/rbqn/build.rs` (686 lines)
- Why fragile: The build script parses CBQN's C-generated bytecode files (`runtime0`, `runtime1`, `compiles`, `formatter`) using string matching and regex-like patterns. It relies on specific formatting: `a0p[N] = incG(provide[M])`, `m_blockinfo(TYPE, iarrs[M], iarrs[N])`, etc. Any change in CBQN's code generation output format (e.g., different whitespace, new object types, changed function names) will silently produce incorrect bytecode or fail to parse.
- Safe modification: Test against multiple CBQN versions. Add validation that checks parsed component counts match expectations.
- Test coverage: None. Failures only manifest at runtime as wrong bootstrap results.

**NaN-Boxing Bit Patterns:**
- Files: `crates/rbqn-core/src/value.rs` (lines 1-12, 28-33)
- Why fragile: Tag values (`FUN_TAG=0xFFF4`, `ARR_TAG=0xFFF7`, `MD1_TAG=0xFFF2`, etc.) and special sentinel patterns (`SENTINEL`, `NO_VAR`, `OPT_OUT`, `NO_FILL`) must exactly match CBQN's bit patterns for the bootstrap bytecode to work correctly. The `is_f64()` check (line 44) is a bitwise port from CBQN. Any discrepancy causes silent misclassification of values.
- Safe modification: Never change tag values without verifying against CBQN's `h.h` definitions. Add compile-time assertions.
- Test coverage: Only the runtime0 verification tests exercise a subset of these patterns.

**Compiler Body/Block Processing:**
- Files: `crates/rbqn-vm/src/compiler.rs` (lines 69-492)
- Why fragile: The `compile_block` function is a 420-line monolithic function that processes body pairs, performs bytecode rewriting, handles predicate remapping, and manages SETH/PRED fixups. The body-swap logic (lines 457-473) silently remaps bytecode body references after a swap. Off-by-one errors in the fixup pass would produce incorrect control flow that only manifests with specific BQN patterns (headers, predicates).
- Safe modification: Add tracing/debug output that can be enabled to dump pre/post-swap body layouts. Write targeted tests for header-matching and predicate evaluation.
- Test coverage: None for the compilation path itself.

## Scaling Limits

**HashMap-Based Object Stores:**
- Current capacity: Unbounded (monotonically growing).
- Limit: System memory. With each array clone on access, effective limit is much lower.
- Scaling path: GC implementation (see Tech Debt section above).

**Stack Overflow Protection:**
- Current capacity: `EnvStack` limits to 10,000 frames in `crates/rbqn-vm/src/env.rs` (line 23), but the actual recursion happens through `c1`/`c2` → `eval_bc` → block dispatch on the Rust call stack, which is not bounded by `EnvStack`.
- Limit: Rust's default thread stack size (~8MB). Deep BQN recursion (e.g., naive fibonacci) will overflow the Rust stack.
- Scaling path: Convert the VM to a trampoline or CPS style to bound stack usage. Or increase stack size via `RUST_MIN_STACK` env var as a stopgap.

**Single-Threaded Execution:**
- Current capacity: All global stores use `Mutex` but the VM is fundamentally single-threaded.
- Limit: No parallelism for array operations.
- Scaling path: The `rbqn-gpu` crate exists as a placeholder for GPU-accelerated operations. For CPU parallelism, consider `rayon` for data-parallel primitives.

## Dependencies at Risk

**CBQN Build-Time Dependency:**
- Risk: RBQN requires a pre-built CBQN source tree at build time to extract bytecode for `runtime0`, `runtime1`, `compiles`, and `formatter`. Without this, the binary compiles but cannot execute any BQN code (the compiler B value is `SENTINEL`).
- Impact: Cannot build a self-contained RBQN binary without CBQN. The `build.rs` searches for CBQN at `$CBQN_PATH`, `../cbqn-ref`, `../cbqn`, `../../cbqn-ref`, `../../cbqn`, looking for `build/*/gen/` directories containing the pre-compiled bytecode.
- Migration plan: Long-term, RBQN should self-host: write a BQN compiler in Rust that doesn't require CBQN's bootstrap bytecode. Medium-term, embed the parsed bytecode as checked-in Rust source files so the CBQN dependency is only needed when updating the bootstrap, not for every build.

**`rustyline` for REPL:**
- Risk: Low. Standard crate, well-maintained.
- Impact: REPL functionality only.
- Migration plan: Not needed.

## Missing Critical Features

**Inverse System (⁼ and ⌾):**
- Problem: The `⁼` (Undo) modifier and `⌾` (Under) modifier are completely unimplemented. Both throw "not yet implemented" errors. `crates/rbqn-prim/src/inverse.rs` contains only a 3-line comment stub.
- Files: `crates/rbqn-prim/src/inverse.rs` (3 lines), `crates/rbqn-vm/src/modifiers.rs` (lines 44, 61, 83, 112)
- Blocks: Many BQN idioms rely on `⁼` (e.g., `⊐⁼` for inverse index-of, `+⁼` for subtraction). The bootstrap's `setPrims` and `setInv` callbacks (skipped at `crates/rbqn/src/bootstrap.rs` lines 373-375) are needed to register inverse functions.

**Rank (⎉) and Depth (⚇) Modifiers:**
- Problem: Both throw "not yet implemented". These are essential for multi-dimensional array processing.
- Files: `crates/rbqn-vm/src/modifiers.rs` (lines 86-87, 115-116)
- Blocks: Any BQN code using rank/depth-based operations.

**System Functions (•-prefixed):**
- Problem: Only 6 system values are implemented: `•Type` (0), `•Decompose` (1), `•Glyph` (4), `•Fill` (7), `•GroupLen` (22), `•GroupOrd` (23). CBQN provides ~50+ system functions including `•BQN`, `•Import`, `•SH`, `•FChars`, `•FLines`, `•FBytes`, `•Fmt`, `•Show`, `•Out`, `•Exit`, `•math.*`, `•wasm.*`, etc.
- Files: `crates/rbqn-vm/src/derive.rs` (lines 51-59, 427-486)
- Blocks: File I/O, process control, math library, formatting, module imports -- essentially anything beyond pure array computation.

**Monadic Shift (« and »):**
- Problem: `c1` for shift-after and shift-before are listed as `None` in the dispatch table.
- Files: `crates/rbqn-prim/src/dispatch.rs` (lines 87, 90)
- Blocks: Monadic shift is commonly used in BQN for accessing previous/next elements.

**Multi-Rank Primitive Gaps:**
- Problem: Several primitives have only vector-level implementations and throw errors for rank>1 inputs: `∾` monadic join for rank>1, `∾` dyadic join-to for atom+non-vector.
- Files: `crates/rbqn-prim/src/structural.rs` (lines 218, 324, 336)
- Blocks: Higher-rank array processing.

**Fill Tracking:**
- Problem: `dispatch_sys_c2` for fill-by (sys_idx 7) returns `x` unchanged with comment "fill tracking is a future enhancement."
- Files: `crates/rbqn-vm/src/derive.rs` (lines 473-476)
- Blocks: BQN programs that depend on fill elements for take/shift operations with proper default values.

**REPL State Persistence:**
- Problem: Each REPL line is evaluated independently via `exec_string`. There is no persistent scope between lines, so `a←5` in one line is not visible in the next.
- Files: `crates/rbqn/src/repl.rs` (line 60), `crates/rbqn-vm/src/compiler.rs` (line 193)
- Blocks: Interactive development workflow.

## Test Coverage Gaps

**No Automated Tests Exist:**
- What's not tested: The entire codebase has zero `#[test]` functions and zero `#[cfg(test)]` modules. The `tests/` directory exists but is empty.
- Files: Every file in `crates/*/src/*.rs`
- Risk: Any refactoring or bug fix can silently break existing functionality. The only validation is manual testing via `cargo run`.
- Priority: High. At minimum, need: (1) value representation tests (NaN-boxing round-trips), (2) primitive correctness tests (each glyph, monad and dyad), (3) VM opcode tests, (4) bootstrap stage verification, (5) end-to-end BQN expression tests.

**Bootstrap Verification Incomplete:**
- What's not tested: Per MEMORY.md, runtime0 verification has 7/9 tests passing (`+´` and `+¨` still fail). Runtime1, compiler, and formatter stages have no verification tests.
- Files: No test files exist
- Risk: Bootstrap produces subtly wrong runtime functions that cause cascading failures in user code.
- Priority: High. The bootstrap is the foundation of correctness.

---

*Concerns audit: 2026-02-21*

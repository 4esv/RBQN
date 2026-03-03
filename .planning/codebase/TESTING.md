# Testing Patterns

**Analysis Date:** 2026-02-21

## Test Framework

**Runner:**
- No test framework configured
- No `#[test]` annotations exist anywhere in the codebase
- No `#[cfg(test)]` modules exist anywhere in the codebase
- The `tests/` directory at project root is empty

**Assertion Library:**
- N/A (no tests)

**Run Commands:**
```bash
cargo test                  # Would run tests (currently finds none)
cargo test -p rbqn-core     # Test specific crate
```

## Test File Organization

**Location:**
- No test files exist
- No co-located test modules
- No separate test directory content

**Convention to follow for new tests:**
- Use co-located `#[cfg(test)] mod tests {}` blocks for unit tests
- Place integration tests in `tests/` at workspace root
- Name test files matching the module they test: `tests/bootstrap.rs`, `tests/primitives.rs`

## Test Structure

**Proposed pattern (no existing tests to reference):**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_scalar_scalar() {
        let result = add_c2(B::m_f64(2.0), None, B::m_f64(3.0), None).unwrap();
        match result {
            PrimResult::Scalar(b) => assert_eq!(b.o2f(), 5.0),
            _ => panic!("Expected scalar"),
        }
    }
}
```

## Mocking

**Framework:** N/A

**What would need mocking:**
- Array store (`ARR_STORE`) for isolated testing - currently global mutable state
- Derived store (`DERIVED_STORE`) - also global mutable state
- The global stores make unit testing difficult because tests share state

**What NOT to mock:**
- The `B` value type (it's a simple `u64` wrapper, test directly)
- Primitive functions (test end-to-end through their public API)

## Fixtures and Factories

**No test data or fixtures exist.**

**Proposed test data approach:**
```rust
// Helper to make a simple i32 array
fn test_arr(data: &[i32]) -> (B, BqnArr) {
    let arr = BqnArr::new_vec_i32(data.to_vec());
    let b = tag_arr(arr.clone());
    (b, arr)
}

// Helper to make a string
fn test_str(s: &str) -> B {
    let chars: Vec<u32> = s.chars().map(|c| c as u32).collect();
    tag_arr(BqnArr::new_vec_c32(chars))
}
```

## Coverage

**Requirements:** None enforced. No coverage tooling configured.

**View Coverage:**
```bash
cargo install cargo-tarpaulin   # if not installed
cargo tarpaulin --workspace     # generate coverage report
```

## Test Types

**Unit Tests:**
- Do not exist. Would be most valuable for:
  - `crates/rbqn-core/src/value.rs` - NaN-boxing tag/untag roundtrips
  - `crates/rbqn-core/src/array.rs` - Array creation, element access, squeeze
  - `crates/rbqn-prim/src/arith_dyad.rs` - All arithmetic primitives
  - `crates/rbqn-prim/src/structural.rs` - Structural operations
  - `crates/rbqn-vm/src/bytecode.rs` - Op encoding/decoding

**Integration Tests:**
- Do not exist. Would be most valuable for:
  - Bootstrap pipeline: `bootstrap()` succeeds and produces valid runtime
  - End-to-end BQN evaluation: `exec_string(rt, "1+1")` returns `B::m_f64(2.0)`
  - Compiler roundtrip: compile and execute simple BQN expressions

**E2E Tests:**
- Not present
- The BQN spec includes a comprehensive test suite at https://github.com/mlochbaum/BQN/tree/master/test
- Running the BQN test suite through `rbqn -e` would serve as E2E testing

## Existing Verification

**Manual verification approach documented in MEMORY.md:**
- Runtime0 verification: 7/9 tests pass (as of 2026-02-21)
- Verification done by running BQN expressions and comparing output
- No automated regression testing

**Build script verification:**
- `crates/rbqn/build.rs` parses CBQN gen files at build time
- Build fails if CBQN source files not found (requires `CBQN_PATH` env var)
- `assert_eq!` in `crates/rbqn/src/bootstrap.rs:321` verifies primitive count

## What to Test First (Priority Order)

1. **`crates/rbqn-core/src/value.rs`** - NaN-boxing is the foundation. Verify:
   - `B::m_f64(v).is_f64()` and `B::m_f64(v).o2f() == v` roundtrips
   - Tag checks: `is_arr()`, `is_fun()`, `is_md1()`, etc.
   - Special values: `SENTINEL`, `NO_VAR`, `OPT_OUT`
   - Edge cases: NaN, infinity, negative zero

2. **`crates/rbqn-prim/src/arith_dyad.rs`** - Pervasive arithmetic is heavily used:
   - Scalar-scalar, scalar-array, array-scalar, array-array paths
   - Character arithmetic (`char + num`, `char - char`)
   - Shape mismatch errors

3. **`crates/rbqn-prim/src/structural.rs`** - Most complex primitive file (1077 lines):
   - Reshape, join, take, drop, reverse, rotate, transpose
   - Multi-rank behavior vs rank-1 fast paths

4. **`crates/rbqn-vm/src/derive.rs`** - Modifier dispatch (`c1`/`c2`):
   - Fork, Atop, Md1D, Md2D dispatch paths
   - Native modifier dispatch

5. **Bootstrap integration test:**
   ```rust
   #[test]
   fn bootstrap_succeeds() {
       let rt = bootstrap::bootstrap().expect("bootstrap should succeed");
       assert_eq!(rt.runtime.len(), 64);
       assert!(!rt.compiler.q_n());
   }
   ```

## Challenges for Testing

**Global mutable state:** The `ARR_STORE`, `DERIVED_STORE`, and `NS_STORE` are global singletons. Tests running in parallel will share these stores. This is generally okay (IDs are unique) but means tests cannot verify store cleanup.

**Bootstrap dependency:** Integration tests require `CBQN_PATH` to be set and CBQN source to be available. Tests should be gated with `#[ignore]` or a feature flag for CI environments without CBQN.

**Panic-based errors:** The VM uses `panic!` for errors, making error-path testing require `#[should_panic]` or `catch_unwind`. The prim crate's `Result` return type is more testable.

---

*Testing analysis: 2026-02-21*

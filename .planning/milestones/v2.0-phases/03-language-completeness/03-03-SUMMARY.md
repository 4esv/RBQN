---
phase: 03-language-completeness
plan: 03
subsystem: system-functions
tags: [math, rand, platform, time, shell, modifiers, bqn]
dependency_graph:
  requires: [03-02]
  provides: [SYS-16, SYS-17, SYS-18, SYS-19, SYS-20, SYS-21]
  affects: [derive.rs, modifiers.rs]
tech_stack:
  added: [wyrand-prng, lanczos-lgamma, abramowitz-stegun-erf]
  patterns: [namespace-builder, sys-fn-dispatch, native-md2-modifier]
key_files:
  modified:
    - crates/rbqn-vm/src/derive.rs
    - crates/rbqn-vm/src/modifiers.rs
decisions:
  - CBQN compiler strips underscores from modifier names: "_while_" → "while", "_fillBy_" → "fillby"
  - Math functions use sys indices 1100-1130 to avoid collision with existing sys 100 (system resolver)
  - Erf uses Abramowitz & Stegun polynomial approximation (max error ~1.5e-7)
  - Fact uses exact integer multiplication for n≤20, Lanczos lgamma for non-integers
  - PRNG: wyrand algorithm seeded from SystemTime nanoseconds
  - platform.pi field stored as lowercase "pi" — BQN compiler lowercases all namespace fields
metrics:
  duration: 35min
  completed: "2026-02-24"
  tasks: 2
  files: 2
---

# Phase 3 Plan 3: System Functions — Math, Rand, Platform, Time, Shell, While Summary

Math, rand, and platform namespaces plus time functions, shell execution, and the •_while_ 2-modifier implemented, completing all Phase 3 system function requirements (SYS-16 through SYS-21).

## Tasks Completed

### Task 1: math, rand, and platform namespaces (commit 8a58e04)

**•math namespace (SYS-16):**
- `Sin`, `Cos`, `Tan`, `Asin`, `Acos`, `Atan` (mono atan/dyadic atan2)
- `Log` (monadic: ln, dyadic: log_w(x))
- `Cbrt`, `Hypot`, `Erf` (Abramowitz-Stegun approximation)
- `Comb` (binomial coefficient, iterative), `Fact` (integer exact / Lanczos lgamma)
- `GCD`, `LCM` (Euclidean algorithm)
- `pi` constant (immediate f64 value in namespace)
- Sys indices: 1100-1113 (monadic) + 1105/1106/1108/1110/1112/1113 (dyadic)

**•rand namespace (SYS-18):**
- `Range` (mono: random int [0,n); dyadic: shaped array of random ints)
- `Deal` (Fisher-Yates shuffle, full permutation of ↕n)
- `Subset` (k random distinct indices from ↕n)
- PRNG: wyrand algorithm, static `Mutex<u64>` state seeded from SystemTime

**•platform namespace (SYS-19):**
- `os`: immediate string ("macos", "linux", "windows" via std::env::consts::OS)
- `arch`: immediate string (std::env::consts::ARCH)
- `environment`: callable function (sys 1130) for env var lookup
- `impl`: immediate "RBQN" string

All namespaces follow `make_file_namespace()` pattern: `LazyLock<Mutex<Option<B>>>` cache, NSDesc with str2gid keys, Scope with field values.

### Task 2: Time functions, shell execution, •_while_ (commit 4bf40c1)

**Time functions (SYS-17):**
- `•UnixTime` (sys 140): SystemTime::now().duration_since(UNIX_EPOCH).as_secs_f64()
- `•MonoTime` (sys 141): static LazyLock<Instant> for elapsed seconds
- `•Delay` (sys 142): thread::sleep(Duration::from_secs_f64(x))

**Shell execution (SYS-20):**
- `•SH` (sys 145): Command::new("sh").arg("-c").arg(&cmd).output()
- Returns 3-element array: ⟨exit_code, stdout_string, stderr_string⟩
- Dyadic form: if x is string array, uses first element as command with rest as args

**•_while_ modifier (SYS-21):**
- Registered as `m_native_md2(MD2_WHILE)` where `MD2_WHILE = 65`
- Monadic c1: loop while `c1(g, acc) == 1`, apply `c1(f, acc)`
- Dyadic c2: loop while `c2(g, w, acc) == 1`, apply `c2(f, w, acc)`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] CBQN compiler strips underscores from modifier names**
- **Found during:** Task 2 verification (•_while_ returned SENTINEL)
- **Issue:** sys_name_to_b registered "_while_" but CBQN's compiler sends "while" (strips underscores, lowercases)
- **Fix:** Added "while" | "_while_" and "fillby" | "_fillBy_" patterns to sys_name_to_b
- **Files modified:** crates/rbqn-vm/src/derive.rs
- **Commit:** 4bf40c1

**2. [Rule 1 - Bug] Math sys indices conflicted with existing sys 100**
- **Found during:** Task 1 implementation
- **Issue:** Plan used sys 100-113 for math, but sys 100 is already the system value resolver
- **Fix:** Used sys indices 1100-1130 for math/rand/platform functions to avoid collision
- **Files modified:** crates/rbqn-vm/src/derive.rs
- **Commit:** 8a58e04

## Verification Results

All plan verifications pass:
1. `•math.Sin •math.pi÷2` → `1` (sin(π/2) = 1)
2. `≠•rand.Deal 10` → `10` (permutation of ↕10 has length 10)
3. `•UnixTime@` → `1771938816.80829` (current epoch seconds)
4. `•SH "echo hi"` → `⟨ 0 "hi\n" "" ⟩` (exit 0, stdout, empty stderr)
5. `{𝕩+1} •_while_ {𝕩<10} 0` → `10` (while loop to 10)

Test suite: no regressions (8 pass, rest ignored pending CBQN_PATH integration tests).

## Self-Check: PASSED

All files exist, both commits verified in git history.

# Phase 9: CBQN Behavioral Parity - Research

**Researched:** 2026-03-03
**Domain:** BQN system functions, FFI, bitwise ops, terminal I/O, error handling
**Confidence:** HIGH (verified against CBQN source code on disk)

## Summary

Phase 9 requires implementing six groups of system function features to match CBQN behavior: `•bit` namespace, `•term` namespace, `•math.Comb`/`•math.LCM` fixes, `•state`, `•CurrentError`, and `•FFI`. All features currently exist as stubs (throwing "not implemented") or returning SENTINEL.

The CBQN source code at `/Users/axel/Code/forks/CBQN/src/` was used as the primary reference for all behavioral details. Critical finding: the `•bit` namespace in CBQN does NOT have `_shift` -- it has `_cast`, `_not`, `_neg`, `_and`, `_or`, `_xor`, `_add`, `_sub`, `_mul`. Another critical finding: `•math.Comb` and `•math.LCM` are dyadic-only in CBQN -- monadic calls should error, not return dummy values.

**Primary recommendation:** Implement in order of complexity: math fixes (trivial) -> `•state` (trivial) -> `•bit` namespace (moderate) -> `•term` namespace (moderate) -> `•CurrentError` (moderate, needs VM plumbing) -> `•FFI` (complex, budget-capped at 2 plans).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **FFI scope:** Full CBQN type string support -- i8, i16, i32, i64, f32, f64, u8, *u8 (strings). No structs, no callbacks.
- **FFI returns:** Typed returns required (i32->BQN int, f64->BQN float, *u8->BQN string). Not void-only.
- **FFI errors:** Match CBQN exactly -- BQN errors for library-not-found, symbol-not-found (catchable with catch)
- **FFI budget:** Maximum 2 plans. If not working after 2 plans, defer remaining work to future milestone.
- **FFI tests:** Custom test .so for development validation + libm smoke test (call sin/cos/sqrt)
- **FFI deferral:** If basic load+call with typed args doesn't work after 2 plans, defer entirely
- **term behavior:** Match CBQN platform support exactly
- **term no-TTY:** Match CBQN behavior
- **term cleanup:** Match CBQN cleanup behavior
- **term REPL:** Match CBQN REPL vs script mode interaction
- **state/CurrentError:** Research required for exact behavior (done -- see below)
- **CurrentError priority:** Nice-to-have. Stub returning error message string acceptable if VM plumbing is complex.
- **math fixes:** Match CBQN exact monadic semantics
- **bit namespace:** Match CBQN And, Or, Xor, Not, Shift behavior for integer arrays

### Claude's Discretion
- Choice of FFI crate (libloading, dlopen, etc.)
- Choice of terminal crate (crossterm, nix, raw termios, etc.)
- Internal architecture for error context threading (thread-local, context param, etc.)
- Plan ordering and grouping within the 5 planned items
- Exact test structure and verification approach

### Deferred Ideas (OUT OF SCOPE)
- FFI struct mapping and callbacks -- future milestone if basic FFI ships
- FFI full type coverage beyond the basic CBQN set -- future milestone
- Windows-specific term implementation if CBQN doesn't support Windows term
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PAR3-01 | FFI loads shared libraries and calls C functions from BQN | FFI section: type grammar, libloading+libffi crates, CBQN calling convention |
| PAR3-02 | bit namespace fully implemented: And, Or, Xor, Not, Shift matching CBQN | bit namespace section: CBQN has 9 ops (no Shift), operand format, bit stream model |
| PAR3-03 | term namespace: RawMode, CharB, Flush working | term section: CBQN uses termios directly, 6 fields total |
| PAR3-04 | math.Comb monadic and math.LCM monadic correct | math section: both are DYADIC-ONLY in CBQN -- monadic should error |
| PAR3-05 | state returns meaningful value rather than SENTINEL | state section: returns `(path, name, args)` triple |
| PAR3-06 | CurrentError returns current error inside catch blocks | CurrentError section: thread-local `lastErrMsg`, set during catch handler |
</phase_requirements>

## Standard Stack

### Core (already in workspace)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rbqn-vm | workspace | VM dispatch, derive.rs sys_fn dispatch | Where all changes land |
| rbqn-core | workspace | B type, BqnArr, error types | Error propagation for CurrentError |
| rbqn-prim | workspace | Primitive dispatch table | Bit operations may use similar patterns |

### New Dependencies Needed
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| libloading | 0.8 | Load .so/.dylib/.dll at runtime | FFI: dlopen/dlsym equivalent |
| libffi | 3.2 | Call C functions with dynamic type signatures | FFI: construct call frames at runtime |
| nix | 0.29 | POSIX termios API (raw mode, non-blocking reads) | term: RawMode, CharN |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| libloading | dlopen-rs | libloading is more mature, widely used, cross-platform |
| libffi | Raw dlsym + transmute | Unsafe, no dynamic type marshalling; libffi handles calling conventions |
| nix (termios) | crossterm | crossterm is higher-level but heavier; CBQN uses raw termios directly |
| nix (termios) | Raw libc::tcgetattr | nix provides safe wrappers; same underlying calls |

**Recommendation:** Use `libloading` for library loading (simple, cross-platform) and `libffi` for dynamic calls (handles calling conventions properly). Use `nix` crate for termios (safe wrapper over POSIX, matches CBQN's direct termios approach).

**Installation:**
```toml
# In crates/rbqn-vm/Cargo.toml (or crates/rbqn/Cargo.toml)
libloading = "0.8"
libffi = "3.2"
nix = { version = "0.29", features = ["term", "fs"] }
```

## Architecture Patterns

### Key Files That Need Changes

```
crates/rbqn-vm/src/derive.rs     # Main file: sys_fn dispatch, namespace creation
                                   # - bit namespace: replace stubs with real ops
                                   # - term namespace: replace stubs, add OutRaw/ErrRaw
                                   # - math: fix Comb/LCM monadic to error
                                   # - state: return (path, name, args) triple
                                   # - CurrentError: read thread-local error msg
                                   # - FFI: new dispatch entries

crates/rbqn-vm/src/modifiers.rs  # catch_c1/catch_c2: set error context for CurrentError

crates/rbqn-core/src/error.rs    # May need BqnError variants for FFI errors
                                   # Thread-local for current error message

crates/rbqn/Cargo.toml           # Add libloading, libffi deps (or rbqn-vm/Cargo.toml)
```

### Pattern: Adding a New System Function

All system functions are dispatched in `crates/rbqn-vm/src/derive.rs` via two match blocks:

1. **Monadic dispatch** (`dispatch_sys_c1`): `match idx { ... 161 => ..., ... }`
2. **Dyadic dispatch** (`dispatch_sys_c2`): `match idx { ... 160 => ..., ... }`
3. **Name resolution** (`sys_name_to_b`): maps `"foo" => m_sys_fn(N)` or `"foo" => make_foo_namespace()`

To add a new function:
```rust
// 1. In sys_name_to_b:
"mynewfn" => m_sys_fn(190),

// 2. In dispatch_sys_c1 match:
190 => my_new_fn_c1(x),

// 3. In dispatch_sys_c2 match (if dyadic):
190 => my_new_fn_c2(w, x),

// 4. In FN_NAMES match (for error messages):
190 => "•MyNewFn",
```

### Pattern: Creating a Namespace

Namespaces use a cached LazyLock pattern (see `make_bit_namespace`, `make_term_namespace`):
```rust
static MY_NS: std::sync::LazyLock<Mutex<Option<B>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

fn make_my_namespace() -> B {
    let mut guard = MY_NS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(ns_b) = *guard {
        return ns_b;
    }
    use crate::namespace::{str2gid, NSDesc, NS};
    let gids = vec![str2gid("field1"), str2gid("field2")];
    let var_am = gids.len() as i32;
    let desc = Arc::new(NSDesc { var_am, exp_gids: gids });
    let body = Arc::new(crate::block::Body::new(var_am as u16, 0, 0, 0));
    let sc = Arc::new(crate::scope::Scope::new(
        body, None, var_am as u16,
        &[m_sys_fn(N1), m_sys_fn(N2)],
    ));
    let ns = NS { desc, sc };
    let ns_b = crate::namespace::store_ns(ns);
    *guard = Some(ns_b);
    ns_b
}
```

### Anti-Patterns to Avoid
- **Monadic stubs returning dummy values**: CBQN errors on monadic Comb/LCM/GCD -- RBQN currently returns `1.0`/`0.0`. Must throw instead.
- **Assuming Shift exists**: The requirement mentions `•bit.Shift` but CBQN does not have it. Match CBQN's actual namespace.
- **Ignoring argument order**: CBQN's `w •math.Comb x = C(w, x)` (w choose x), not C(x, w). RBQN currently has `C(x, w)` which is backwards.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Dynamic library loading | Raw dlopen/dlsym FFI | `libloading` crate | Cross-platform, handles symbol name mangling |
| Dynamic C function calls | Unsafe transmute with fixed signatures | `libffi` crate | Handles calling conventions, stack alignment, variadic |
| Terminal raw mode | Raw libc termios calls | `nix` crate termios module | Safe wrappers, error handling, cross-platform POSIX |
| Bit stream manipulation | Custom bit packing/unpacking | Standard Rust bitwise ops on u64 words | CBQN's model: arrays are bit streams, split by width |

## Common Pitfalls

### Pitfall 1: Comb Argument Order
**What goes wrong:** RBQN currently computes `C(x, w)` but CBQN computes `C(w, x)` for `w •math.Comb x`.
**Why it happens:** The code comment says "w •math.Comb x = C(x, w)" but CBQN actually does "w choose x" = C(w, x).
**How to avoid:** Test against CBQN: `5 •math.Comb 2` should return 10 (C(5,2) = 10).
**Warning signs:** Wrong binomial coefficients, especially when w < x.
**Verified:** Running CBQN confirms `5 •math.Comb 2` = 10, `5 •math.Comb 3` = 10, `10 •math.Comb 3` = 120.

### Pitfall 2: Monadic Math Functions Should Error
**What goes wrong:** RBQN returns dummy values for monadic Comb (1.0) and LCM (0.0). CBQN errors.
**Why it happens:** Original implementation assumed these had monadic forms.
**How to avoid:** Monadic dispatch for sys 1110 (Comb), 1112 (GCD), 1113 (LCM) should throw "This function can't be called monadically".
**Verified:** CBQN throws "This function can't be called monadically" for `•math.Comb 5`, `•math.LCM 6`, `•math.GCD 12`.

### Pitfall 3: bit Namespace Has No _shift
**What goes wrong:** Implementing `_shift` that doesn't exist in CBQN.
**Why it happens:** The requirement text mentions "Shift" but CBQN's actual namespace is: `_cast`, `_not`, `_neg`, `_and`, `_or`, `_xor`, `_add`, `_sub`, `_mul`.
**How to avoid:** Match CBQN exactly: 9 fields, no Shift.
**Warning signs:** Programs using `•bit._shift` should get "field not found", same as CBQN.

### Pitfall 4: bit Operations Are 1-Modifiers, Not Functions
**What goes wrong:** Implementing `•bit._and` as a regular function instead of a 1-modifier.
**Why it happens:** Namespace fields are typically functions, but `•bit` fields are 1-modifiers.
**How to avoid:** Each field must be a 1-modifier: `𝕨 N •bit._and 𝕩` where N is the operand (width spec).
**Implication:** The namespace stores MD1 values, not FUN values. Need `m_native_md1()` instead of `m_sys_fn()`.

### Pitfall 5: CurrentError Needs VM Plumbing
**What goes wrong:** Trying to implement `•CurrentError` without connecting it to the catch mechanism.
**Why it happens:** CBQN uses a global `lastErrMsg` that `pushRe()` sets when entering catch handler.
**How to avoid:** Use a thread-local `CURRENT_ERROR: RefCell<Option<String>>` that catch_c1/catch_c2 set before calling G, and clear after G returns.
**Warning signs:** `•CurrentError` always returning "not in catch" even inside catch handlers.

### Pitfall 6: FFI Type String Parsing Complexity
**What goes wrong:** Underestimating the grammar for FFI type strings.
**Why it happens:** The type grammar supports pointers, arrays, structs, explicit conversions.
**How to avoid:** Start with the minimal subset: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`, `""` (void), `*u8` (string pointer). Skip structs, callbacks, nested pointers per CONTEXT.md.

## Feature Details

### 1. •bit Namespace (PAR3-02)

**CBQN Reference** (from `/Users/axel/Code/forks/CBQN/src/builtins/bit.c`):

**Fields (9 total):**
| Field | Type | Behavior |
|-------|------|----------|
| `_cast` | 1-modifier | Type conversion: `from‿to •bit._cast arr` changes element interpretation |
| `_not` | 1-modifier | Bitwise NOT: `N •bit._not arr` |
| `_neg` | 1-modifier | Two's complement negation: `N •bit._neg arr` |
| `_and` | 1-modifier (dyadic) | Bitwise AND: `w N •bit._and x` |
| `_or` | 1-modifier (dyadic) | Bitwise OR: `w N •bit._or x` |
| `_xor` | 1-modifier (dyadic) | Bitwise XOR: `w N •bit._xor x` |
| `_add` | 1-modifier (dyadic) | Integer addition: `w N •bit._add x` |
| `_sub` | 1-modifier (dyadic) | Integer subtraction: `w N •bit._sub x` |
| `_mul` | 1-modifier (dyadic) | Integer multiplication: `w N •bit._mul x` |

**Operand Format:**
- Scalar N: all widths = N (operation, result, args all same width)
- Array of 1-4 elements: `[opWidth, resultWidth, xWidth, wWidth]`
- Extend by repeating last element to fill 3 (monadic) or 4 (dyadic) positions
- All widths must be powers of 2

**Bit Stream Model:**
1. Convert argument array to its bit representation based on element width
2. Split bit stream into units of operation width
3. Apply operation to units
4. Reassemble result as bit stream, split into result width elements
5. Return array with result element type

**Element Width -> Type mapping:**
- 1 bit -> boolean (bit array)
- 8 bits -> i8
- 16 bits -> i16
- 32 bits -> i32
- 64 bits -> f64

**Implementation approach:**
- Store 1-modifiers (MD1 values) in the namespace, not functions
- Each modifier creates a derived that captures the operand
- On call, parse operand, convert arrays to raw bytes, apply op, convert back
- For MVP: support common widths (1, 8, 16, 32, 64) on the bitwise ops (_and, _or, _xor, _not)
- Integer ops (_add, _sub, _mul, _neg) and _cast can follow same pattern

### 2. •term Namespace (PAR3-03)

**CBQN Reference** (from `/Users/axel/Code/forks/CBQN/src/builtins/sysfn.c:1284-1358`):

**Fields (6 total in CBQN):**
| Field | Behavior |
|-------|----------|
| `Flush` | `fflush(stdout); fflush(stderr); return x;` |
| `RawMode` | Unix: `tcgetattr/tcsetattr` with `ICANON \| ECHO` flags. Windows: `SetConsoleMode`. Arg 1=enable, 0=disable. |
| `CharB` | `fgetc(stdin)` blocking. Returns character or 0 if EOF. |
| `CharN` | Non-blocking: Unix uses `fcntl(O_NONBLOCK)` + `fgetc`. Windows uses `_kbhit/_getch`. Returns char or 0. |
| `OutRaw` | Write bytes directly to stdout without newline. `x` must be a list. Returns `x`. |
| `ErrRaw` | Write bytes directly to stderr without newline. `x` must be a list. Returns `x`. |

**Current RBQN state:** Namespace has 3 fields (RawMode, CharB, Flush), all throw "not implemented".

**What needs to change:**
1. Add `CharN`, `OutRaw`, `ErrRaw` fields to the namespace (6 total)
2. Implement `Flush`: trivial, just flush stdout/stderr
3. Implement `RawMode`: use `nix::sys::termios` on Unix
4. Implement `CharB`: `std::io::stdin().bytes().next()`
5. Implement `CharN`: non-blocking read with termios `O_NONBLOCK`
6. Implement `OutRaw`/`ErrRaw`: write raw bytes to stdout/stderr

**Platform behavior:** CBQN only supports Unix (termios) and Windows (conio). No-termios platforms get "not available" errors. Match this.

### 3. •math.Comb and •math.LCM Fixes (PAR3-04)

**CBQN Reference:**
- Both are **dyadic-only** functions (registered with `D()` macro in builtins.h)
- Monadic calls throw: "This function can't be called monadically"
- `•math.GCD` is also dyadic-only (same fix needed)

**Current RBQN state:**
- `math_comb_c1` returns `B::m_f64(1.0)` -- WRONG, should error
- `math_lcm_c1` returns `B::m_f64(0.0)` -- WRONG, should error
- `math_gcd_c1` returns `B::m_f64(x.o2f().abs())` -- WRONG, should error
- `math_comb_c2` computes `C(x, w)` -- **WRONG ORDER**, should be `C(w, x)`

**Fix required:**
```rust
// Monadic: error (matching CBQN)
fn math_comb_c1(_x: B) -> B {
    rbqn_core::error::throw("This function can't be called monadically")
}
fn math_lcm_c1(_x: B) -> B {
    rbqn_core::error::throw("This function can't be called monadically")
}
fn math_gcd_c1(_x: B) -> B {
    rbqn_core::error::throw("This function can't be called monadically")
}

// Dyadic: fix argument order
fn math_comb_c2(w: B, x: B) -> B {
    // w •math.Comb x = C(w, x) = w choose x
    let n = w.o2f().round() as i64;  // was: x.o2f()
    let k = x.o2f().round() as i64;  // was: w.o2f()
    // ... rest same
}
```

### 4. •state (PAR3-05)

**CBQN Reference** (from sysfn.c:1647-1652):
```c
case 15: { // •state
    if (q_N(args)) thrM("No arguments present for •state");
    if (q_N(name)) thrM("No name present for •state");
    if (q_N(path0)) thrM("No path present for •state");
    cr = m_hvec3(inc(REQ_PATH), inc(name), inc(args));
    break;
}
```

**Semantics:** `•state` returns `(•path, •name, •args)` -- a 3-element list.

**Current RBQN state:** `dispatch_sys_env(41)` returns `B::SENTINEL`.

**Fix required:**
```rust
41 => {
    // •state = ⟨•path, •name, •args⟩
    let path = dispatch_sys_env(38); // •path
    let name = dispatch_sys_env(39); // •name
    let args = dispatch_sys_env(37); // •args
    crate::vm::b_vec_to_arr(vec![path, name, args])
}
```

### 5. •CurrentError (PAR3-06)

**CBQN Reference** (from sysfn.c:715-725 and md2.c:36-53):

**Mechanism:**
1. Global `lastErrMsg` initialized to `bi_N` (Nothing)
2. When `F⎊G` catches an error: `pushRe()` saves old `lastErrMsg`, sets `lastErrMsg = thrownMsg`
3. G is called. If G calls `•CurrentError`, it reads `lastErrMsg` and returns it.
4. When G returns: `re_freeO()` restores old `lastErrMsg` (stack discipline)
5. If `•CurrentError` is called outside any catch: "Not currently within any ⎊"
6. Argument must not be a namespace (reserved for future)

**Current RBQN state:** `dispatch_sys_c1` for idx 80 returns `B::SENTINEL`.

**Implementation approach:**
```rust
// Thread-local to store current error message
thread_local! {
    static CURRENT_ERROR: RefCell<Option<B>> = RefCell::new(None);
}

// In •CurrentError dispatch:
80 => {
    if x.is_nsp() {
        rbqn_core::error::throw("•CurrentError 𝕩: Namespace 𝕩 is reserved");
    }
    CURRENT_ERROR.with(|ce| {
        match ce.borrow().as_ref() {
            Some(msg) => *msg,  // return copy of error message
            None => rbqn_core::error::throw("•CurrentError 𝕩: Not currently within any ⎊"),
        }
    })
}

// In catch_c1/catch_c2 (modifiers.rs):
fn catch_c1(f: B, g: B, x: B) -> B {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| c1(f, x)));
    match result {
        Ok(v) => v,
        Err(panic) => {
            let msg = extract_error_message(&panic);
            let old = CURRENT_ERROR.with(|ce| ce.borrow_mut().replace(msg));
            let r = c1(g, x);
            CURRENT_ERROR.with(|ce| *ce.borrow_mut() = old); // restore
            r
        }
    }
}
```

**Complexity assessment:** MEDIUM. The thread-local approach is clean and matches CBQN's global. The tricky part is extracting the error message from the panic payload (BqnError or String) and converting it to a B value (char array). The `exec.rs::panic_to_bqn_error` function already does the panic-to-BqnError extraction, so the pattern exists.

### 6. •FFI (PAR3-01)

**CBQN Reference** (from ffi.c and BQN spec):

**Calling convention:** `path •FFI spec` where:
- `path` = library path string (`.so`/`.dylib`/`.dll`), or `@` for libc
- `spec` = `"result"‿"funcname"‿"arg1"‿"arg2"‿...`

**Type strings (subset for this phase):**
| Type | C equivalent | BQN representation |
|------|-------------|-------------------|
| `i8` | `int8_t` | number |
| `i16` | `int16_t` | number |
| `i32` | `int32_t` | number |
| `i64` | `int64_t` | number |
| `u8` | `uint8_t` | number |
| `u16` | `uint16_t` | number |
| `u32` | `uint32_t` | number |
| `u64` | `uint64_t` | number |
| `f32` | `float` | number |
| `f64` | `double` | number |
| `""` | `void` | returns `@` (null character) |
| `*u8` | `uint8_t*` | pointer object |
| `*u8:c8` | `uint8_t*` (as string) | character list |

**Implementation approach:**
1. `path •FFI spec` returns a callable BQN function
2. The function, when called, uses `libloading` to load the library and `libffi` to call the symbol
3. Cache loaded libraries for reuse (per path)
4. Type string parsing: simple prefix matching for the basic types
5. Argument conversion: BQN number -> C type via truncation/casting
6. Return conversion: C type -> BQN number/string

**Key implementation details:**
```rust
// Library cache
static FFI_LIBS: LazyLock<Mutex<HashMap<String, Arc<libloading::Library>>>> = ...;

// •FFI dyadic: path •FFI spec -> returns a callable function
fn ffi_load(path: B, spec: B) -> B {
    let path_str = b_to_string(path);
    let spec_arr = get_arr(spec).expect("•FFI: 𝕩 must be a list");
    let result_type = extract_string(spec_arr.get(0));
    let func_name = extract_string(spec_arr.get(1));
    let arg_types: Vec<String> = (2..spec_arr.ia())
        .map(|i| extract_string(spec_arr.get(i)))
        .collect();
    // Create a derived function that captures path, func_name, types
    // On call, load library (cached), look up symbol, build libffi CIF, call
}
```

**Risk areas:**
- libffi may require system libffi installed (check if libffi-sys bundles it)
- macOS .dylib vs Linux .so path handling
- String conversion: BQN char arrays to/from C strings (null termination)
- Memory management: who owns returned string pointers?

## Code Examples

### Example: Bitwise AND on Integer Arrays
```rust
// w 32 •bit._and x
// Both w and x are i32 arrays; result is i32 array
// For each element pair: result[i] = w[i] & x[i]
fn bit_and_c2(operand: &[usize], w: &BqnArr, x: &BqnArr) -> BqnArr {
    let (ow, rw, xw, ww) = parse_widths(operand, false);
    let w_bytes = arr_to_bytes(w, ww);
    let x_bytes = arr_to_bytes(x, xw);
    let n = x_bytes.len();
    let result: Vec<u8> = (0..n).map(|i| w_bytes[i] & x_bytes[i]).collect();
    bytes_to_arr(&result, rw, x.shape.clone())
}
```

### Example: Terminal Raw Mode
```rust
// Source: CBQN sysfn.c:1287-1293
fn term_raw_mode(x: B) -> B {
    use nix::sys::termios;
    let fd = std::io::stdin().as_raw_fd();
    let mut term = termios::tcgetattr(fd).unwrap();
    if x.o2f() == 1.0 {
        term.local_flags.remove(termios::LocalFlags::ICANON | termios::LocalFlags::ECHO);
    } else {
        term.local_flags.insert(termios::LocalFlags::ICANON | termios::LocalFlags::ECHO);
    }
    termios::tcsetattr(fd, termios::SetArg::TCSAFLUSH, &term).unwrap();
    x
}
```

## State of the Art

| Old Approach (RBQN) | Current Approach (CBQN) | Impact |
|---------------------|------------------------|--------|
| •bit stubs throw | •bit has 9 1-modifier operations on bit streams | Must implement full bit stream model |
| •term stubs throw | •term has 6 functions using termios/conio | Must use platform-specific terminal APIs |
| •math.Comb returns 1.0 monadically | Comb is dyadic-only, errors monadically | Simple fix: throw instead of return |
| •math.Comb uses C(x,w) order | CBQN uses C(w,x) = "w choose x" | Fix argument order in dyadic |
| •state returns SENTINEL | •state returns (path, name, args) | Simple fix: compose from existing env values |
| •CurrentError returns SENTINEL | Uses thread-local set during catch | Needs VM plumbing in catch handlers |
| •FFI throws always | Loads .so/.dylib, calls C functions via libffi | Complex: type parsing, marshalling, calling |

## Open Questions

1. **libffi system dependency**
   - What we know: `libffi` crate wraps C libffi; `libffi-sys` can build from source
   - What's unclear: Whether libffi is pre-installed on macOS/Linux or needs bundling
   - Recommendation: Use `libffi` crate with `libffi-sys` which bundles a vendored copy. Test on macOS.

2. **•bit 1-modifier architecture**
   - What we know: CBQN stores modifiers in the namespace. RBQN's namespace stores B values.
   - What's unclear: How to create native 1-modifier B values that capture operands
   - Recommendation: Use `m_native_md1()` pattern (already exists in RBQN for `•_while_` etc.). The modifier's operand becomes the width spec. When the derived is called, parse widths and apply operation.

3. **•CurrentError message format**
   - What we know: CBQN returns the error message as a BQN value (typically a string)
   - What's unclear: Whether it returns the raw thrown value or a formatted string
   - Recommendation: Return the error message string (same as what `!msg` would throw). Extract from BqnError variant.

## Sources

### Primary (HIGH confidence)
- `/Users/axel/Code/forks/CBQN/src/builtins/bit.c` -- complete •bit implementation
- `/Users/axel/Code/forks/CBQN/src/builtins/sysfn.c` -- •term, •state, •CurrentError, •FFI registration
- `/Users/axel/Code/forks/CBQN/src/builtins/md2.c` -- catch/pushRe mechanism for •CurrentError
- `/Users/axel/Code/forks/CBQN/src/ffi.c` -- •FFI implementation (complex, 1000+ lines)
- CBQN live testing: verified •math.Comb/LCM/GCD monadic errors and Comb argument order

### Secondary (MEDIUM confidence)
- [BQN system specification](https://mlochbaum.github.io/BQN/spec/system.html) -- •bit operand format, •term spec, •state definition
- [BQN FFI documentation](https://mlochbaum.github.io/BQN/doc/ffi.html) -- type grammar, calling convention
- [CBQN docs/system.md](https://github.com/dzaima/CBQN/blob/master/docs/system.md) -- •CurrentError: "dynamically-scoped, within catch side of ⎊"

### Tertiary (LOW confidence)
- libffi crate docs (need to verify version compatibility and bundled libffi support)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - verified against CBQN source code
- Architecture: HIGH - existing patterns well understood from RBQN codebase
- Pitfalls: HIGH - critical bugs found (Comb order, monadic errors, missing Shift) all verified
- FFI: MEDIUM - complex feature, 2-plan budget appropriate for uncertainty

**Research date:** 2026-03-03
**Valid until:** 2026-04-03 (stable -- BQN spec changes slowly)

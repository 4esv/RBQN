# CBQN Parity Scope: Complete Requirements for RBQN

**Researched:** 2026-02-21
**Confidence:** HIGH (sourced from official BQN spec, CBQN docs, BQN repo)

---

## Table of Contents

1. [BQN Language Primitives](#1-bqn-language-primitives)
2. [System Functions](#2-system-functions)
3. [Bootstrap Chain](#3-bootstrap-chain)
4. [VM Specification](#4-vm-specification)
5. [Block Types & Headers](#5-block-types--headers)
6. [Namespaces](#6-namespaces)
7. [Inverse System](#7-inverse-system)
8. [FFI / System Interface](#8-ffi--system-interface)
9. [Test Suite](#9-test-suite)
10. [RBQN Current State vs Requirements](#10-rbqn-current-state-vs-requirements)

---

## 1. BQN Language Primitives

**Total: 64 runtime values** (44 functions + 9 one-modifiers + 11 two-modifiers)

The runtime output is ordered by these glyph strings:
- Functions: `+-×÷⋆√⌊⌈|¬∧∨<>≠=≤≥≡≢⊣⊢⥊∾≍⋈↑↓↕«»⌽⍉/⍋⍒⊏⊑⊐⊒∊⍷⊔!`
- 1-modifiers: `` ˙˜˘¨⌜⁼´˝` ``
- 2-modifiers: `∘○⊸⟜⌾⊘◶⎉⚇⍟⎊`

### 1.1 Functions (indices 0-43)

| Idx | Glyph | Monadic | Dyadic |
|-----|-------|---------|--------|
| 0 | `+` | Conjugate (identity for reals) | Add |
| 1 | `-` | Negate | Subtract |
| 2 | `×` | Sign | Multiply |
| 3 | `÷` | Reciprocal | Divide |
| 4 | `⋆` | Exponential (e^x) | Power |
| 5 | `√` | Square Root | Root |
| 6 | `⌊` | Floor | Minimum |
| 7 | `⌈` | Ceiling | Maximum |
| 8 | `\|` | Absolute Value | Modulus |
| 9 | `¬` | Not (1-x) | Span (1+w-x) |
| 10 | `∧` | Sort Up | And (logical) |
| 11 | `∨` | Sort Down | Or (logical) |
| 12 | `<` | Enclose | Less Than |
| 13 | `>` | Merge | Greater Than |
| 14 | `≠` | Length | Not Equals |
| 15 | `=` | Rank | Equals |
| 16 | `≤` | -- | Less or Equal |
| 17 | `≥` | -- | Greater or Equal |
| 18 | `≡` | Depth | Match |
| 19 | `≢` | Shape | Not Match |
| 20 | `⊣` | Identity | Left |
| 21 | `⊢` | Identity | Right |
| 22 | `⥊` | Deshape | Reshape |
| 23 | `∾` | Join | Join To |
| 24 | `≍` | Solo | Couple |
| 25 | `⋈` | Enlist | Pair |
| 26 | `↑` | Prefixes | Take |
| 27 | `↓` | Suffixes | Drop |
| 28 | `↕` | Range | Windows |
| 29 | `«` | Nudge Back | Shift After |
| 30 | `»` | Nudge | Shift Before |
| 31 | `⌽` | Reverse | Rotate |
| 32 | `⍉` | Transpose | Reorder Axes |
| 33 | `/` | Indices | Replicate |
| 34 | `⍋` | Grade Up | Bins Up |
| 35 | `⍒` | Grade Down | Bins Down |
| 36 | `⊏` | First Cell | Select |
| 37 | `⊑` | First | Pick |
| 38 | `⊐` | Classify | Index Of |
| 39 | `⊒` | Occurrence Count | Progressive Index Of |
| 40 | `∊` | Mark Firsts | Member Of |
| 41 | `⍷` | Deduplicate | Find |
| 42 | `⊔` | Group Indices | Group |
| 43 | `!` | Assert | Assert with Message |

### 1.2 One-Modifiers (indices 44-52)

| Idx | Glyph | Name | Behavior |
|-----|-------|------|----------|
| 44 | `˙` | Constant | Returns operand regardless of arguments |
| 45 | `˜` | Self/Swap | Monadic: F x x; Dyadic: F x w |
| 46 | `˘` | Cells | Apply to major cells (rank -1) |
| 47 | `¨` | Each | Apply element-wise |
| 48 | `⌜` | Table | Cartesian product application |
| 49 | `⁼` | Undo | Apply inverse of operand |
| 50 | `´` | Fold | Right fold over list |
| 51 | `˝` | Insert | Fold along first axis |
| 52 | `` ` `` | Scan | Running fold |

### 1.3 Two-Modifiers (indices 53-63)

| Idx | Glyph | Name | Behavior |
|-----|-------|------|----------|
| 53 | `∘` | Atop | F G x = F(Gx); w F∘G x = F(w G x) |
| 54 | `○` | Over | F○G x = F(Gx); w F○G x = (Gw) F (Gx) |
| 55 | `⊸` | Before/Bind | F⊸G x = (Fx) G x; w F⊸G x = (Fw) G x |
| 56 | `⟜` | After/Bind | F⟜G x = x F (Gx); w F⟜G x = w F (Gx) |
| 57 | `⌾` | Under | Apply F under structural transformation G |
| 58 | `⊘` | Valences | Monadic: F x; Dyadic: w G x |
| 59 | `◶` | Choose | Apply function at index returned by F |
| 60 | `⎉` | Rank | Apply F at specified rank |
| 61 | `⚇` | Depth | Apply F at specified depth |
| 62 | `⍟` | Repeat | Apply F n times (G computes n) |
| 63 | `⎊` | Catch | Try F, catch errors with G |

### 1.4 Pervasive Operations

Arithmetic and comparison functions are **pervasive**: they descend into nested arrays element-by-element. This applies to:
- All of `+-×÷⋆√⌊⌈|¬` (monadic and dyadic)
- Dyadic `∧∨`
- All comparison: `<>≠=≤≥`

Pervasiveness requires implementing `_perv` (a modifier that recursively applies to array elements).

---

## 2. System Functions

### 2.1 BQN Specification System Values (Required)

#### Execution
| Name | Type | Description |
|------|------|-------------|
| `•BQN` | Function | Evaluate BQN string in isolated scope |
| `•ReBQN` | Function | Create BQN evaluator with options namespace |
| `•primitives` | Value | List of glyph-value pairs for all primitives |

#### Control
| Name | Type | Description |
|------|------|-------------|
| `•_while_` | 2-modifier | Loop: run F while G returns 1 |

#### Scripts & State
| Name | Type | Description |
|------|------|-------------|
| `•Import` | Function | Load and cache a BQN script file |
| `•args` | Value | CLI arguments for current file |
| `•path` | Value | Directory path of current file |
| `•name` | Value | Filename (with extension) of current file |
| `•wdpath` | Value | Working directory path |
| `•state` | Value | `⟨•path, •name, •args⟩` |
| `•Exit` | Function | Terminate program with exit code |

#### I/O
| Name | Type | Description |
|------|------|-------------|
| `•Out` | Function | Print string to stdout with newline |
| `•Show` | Function | Display BQN value (uses •Fmt) |
| `•Repr` | Function | String representation (parseable) |
| `•Fmt` | Function | Human-readable format |
| `•ParseFloat` | Function | Convert string to float |
| `•GetLine` | Function | Read one line from stdin |

#### File Operations (`•file` namespace)
| Field | Type | Description |
|-------|------|-------------|
| `path` | Value | •path |
| `At` | Function | Join path components |
| `Name` | Function | Extract filename |
| `Parent` | Function | Extract parent directory |
| `BaseName` | Function | Filename without extension |
| `Extension` | Function | File extension |
| `Parts` | Function | Split into dir/name/ext |
| `Exists` | Function | Check file existence |
| `Type` | Function | File type (file/dir/link) |
| `Created` | Function | Creation timestamp |
| `Accessed` | Function | Access timestamp |
| `Modified` | Function | Modification timestamp |
| `Size` | Function | File size in bytes |
| `Permissions` | Function | Unix permissions |
| `Owner` | Function | Owner user/group |
| `RealPath` | Function | Resolve symlinks |
| `Open` | Function | Open file handle |
| `Rename` | Function | Rename/move file |
| `Copy` | Function | Copy file |
| `CreateDir` | Function | Create directory |
| `Remove` | Function | Remove file |
| `RemoveDir` | Function | Remove directory |
| `List` | Function | List directory contents |
| `Chars` | Function | Read/write file as string |
| `Lines` | Function | Read/write file as line list |
| `Bytes` | Function | Read/write file as byte list |
| `MapBytes` | Function | Memory-map file |

#### File Shorthands
| Name | Description |
|------|-------------|
| `•FChars` | `•file.Chars` with path resolution |
| `•FLines` | `•file.Lines` with path resolution |
| `•FBytes` | `•file.Bytes` with path resolution |

#### Operation Properties
| Name | Type | Description |
|------|------|-------------|
| `•Type` | Function | Type code: 0=array, 1=number, 2=character, 3=function, 4=1-modifier, 5=2-modifier, 6=namespace |
| `•Glyph` | Function | Glyph string for primitive |
| `•Source` | Function | Source code of block |
| `•Decompose` | Function | Decompose compound expression |

#### Namespace Operations (`•ns` namespace)
| Field | Type | Description |
|-------|------|-------------|
| `Keys` | Function | Get field names |
| `Has` | Function | Check field existence |
| `Get` | Function | Get field value |

#### Time
| Name | Type | Description |
|------|------|-------------|
| `•UnixTime` | Function | Seconds since epoch |
| `•MonoTime` | Function | Monotonic clock |
| `•Delay` | Function | Sleep n seconds |
| `•_timed` | 1-modifier | Measure execution time |
| `•_maxTime_` | 2-modifier | Execute with time limit |

#### Math (`•math` namespace)
| Field | Type | Description |
|-------|------|-------------|
| `Cbrt` | Function | Cube root |
| `Log2` | Function | Base-2 logarithm |
| `Log10` | Function | Base-10 logarithm |
| `Log1p` | Function | log(1+x) |
| `Expm1` | Function | e^x - 1 |
| `Hypot` | Function (dyadic) | Hypotenuse |
| `Sin`, `Cos`, `Tan` | Function | Trigonometric |
| `Sinh`, `Cosh`, `Tanh` | Function | Hyperbolic |
| `ASin`, `ACos`, `ATan` | Function | Inverse trig |
| `ATan2` | Function (dyadic) | Two-argument arctangent |
| `Fact` | Function | Factorial |
| `LogFact` | Function | Log factorial |
| `Comb` | Function (dyadic) | Binomial coefficient |
| `Erf`, `ErfC` | Function | Error function |
| `GCD`, `LCM` | Function (dyadic) | GCD/LCM |
| `Sum` | Function | Sum of list |

#### Random (`•MakeRand`, `•rand`)
| Field | Type | Description |
|-------|------|-------------|
| `Range` | Function | Random int in [0, n) |
| `Deal` | Function | Random permutation of n |
| `Subset` | Function | Random subset |

#### Data Structures
| Name | Type | Description |
|------|------|-------------|
| `•HashMap` | Function | Create mutable hash map |

HashMap fields: `Count`, `Keys`, `Values`, `Has`, `Get`, `Set`, `Delete`

#### Bitwise Operations (`•bit` namespace)
| Field | Type | Description |
|-------|------|-------------|
| `_not` | 1-modifier | Bitwise NOT |
| `_and` | 2-modifier | Bitwise AND |
| `_or` | 2-modifier | Bitwise OR |
| `_xor` | 2-modifier | Bitwise XOR |
| `_neg` | 1-modifier | Integer negation |
| `_add`, `_sub`, `_mul` | 2-modifier | Integer arithmetic |
| `_cast` | Modifier | Type reinterpretation |

#### Platform Info (`•platform` namespace)
| Field | Description |
|-------|-------------|
| `os` | "linux", "macos", "windows", etc. |
| `environment` | Execution environment |
| `cpu.arch` | CPU architecture string |
| `bqn.impl` | Implementation name ("CBQN") |
| `bqn.implVersion` | Version string |

#### Terminal I/O (`•term` namespace)
| Field | Type | Description |
|-------|------|-------------|
| `Flush` | Function | Flush stdout |
| `RawMode` | Function | Set terminal raw mode |
| `CharB` | Function | Read char (blocking) |
| `CharN` | Function | Read char (non-blocking) |

### 2.2 CBQN Extensions (Non-Spec, but Expected)

| Name | Type | Description |
|------|------|-------------|
| `•SH` | Function | Execute shell command |
| `•FFI` | Function | Foreign function interface |
| `•Hash` | Function | Hash a BQN value |
| `•Cmp` | Function | Compare two values |
| `•FromUTF8` | Function | Bytes to string |
| `•ToUTF8` | Function | String to bytes |
| `•CurrentError` | Function | Get error in catch handler |
| `•term.OutRaw` | Function | Raw stdout bytes |
| `•term.ErrRaw` | Function | Raw stderr bytes |
| `•internal.*` | Namespace | Debugging/internals (GC, HeapDump, etc.) |

---

## 3. Bootstrap Chain

### 3.1 Loading Sequence

The CBQN bootstrap proceeds in this exact order:

```
1. VM + Native primitives (C/Rust code)
   |
   v
2. Build provide array (23 basic + 17 extended = 40 native functions)
   |
   v
3. Execute runtime0 (r0.bqn, pre-compiled bytecode)
   -> Produces ~24 core runtime functions (limited implementations)
   -> Provides: ⊢⊣˙˜∘○⊸⟜◶⍟ ≥<>≠ ⌊⌈| ⋈∾↑↓ ¨´ ⊏
   |
   v
4. Execute runtime1 (r1.bqn, pre-compiled bytecode)
   -> Receives runtime0 output + provide array
   -> Produces FULL 64-value runtime (all primitives implemented)
   -> Handles: pervasive extension, sorting, searching, grouping,
               transpose, under/inverse, fills, identities
   |
   v
5. Call SetPrims(⟨Decompose, PrimInd⟩) on runtime1
   -> Enables primitive identification for •Decompose, •Glyph
   |
   v
6. Optionally call SetInv on runtime1
   -> Sets up inverse tables for ⁼ and ⌾
   |
   v
7. Execute compiler (c.bqn, pre-compiled bytecode)
   -> Receives runtime as environment
   -> Returns: [compiled_result, SetPrims_fn, SetInv_fn]
   -> The compiler is now a BQN function that compiles source strings
   |
   v
8. Execute formatter (f.bqn, pre-compiled bytecode)
   -> Receives: ⟨Type, Decompose, Glyph, FmtNum⟩
   -> Returns: ⟨Fmt, Repr⟩
   |
   v
9. System is ready to evaluate user BQN code
```

### 3.2 Provide Array Structure (40 entries)

**Basic provide (23 entries):**

| Index | Name | Maps To |
|-------|------|---------|
| 0 | type | `•Type` system function |
| 1 | fill | Fill element get/set |
| 2 | log | Natural logarithm (atom only) |
| 3 | grLen | `•GroupLen` - group lengths |
| 4 | grOrd | `•GroupOrd` - group ordering |
| 5 | asrt | `!` assertion |
| 6 | add | `+` on atoms |
| 7 | sub | `-` on atoms |
| 8 | mul | `×` on atoms |
| 9 | div | `÷` on atoms |
| 10 | pow | `⋆` on atoms |
| 11 | floor | `⌊` on single atom |
| 12 | eq | `=` comparison |
| 13 | le | `≤` comparison |
| 14 | fne | `≢` shape for arrays |
| 15 | shape | `⥊` reshape for arrays |
| 16 | pick | `⊑` index atom into list |
| 17 | ud | `↕` range for natural numbers |
| 18 | tbl | `⌜` table on arrays |
| 19 | scan | `` ` `` scan |
| 20 | fillBy | `_fillBy_` fill computation |
| 21 | val | `⊘` valences |
| 22 | catch | `⎊` try-catch |

**Extended provide (17 entries):**

| Index | Name | Maps To |
|-------|------|---------|
| 23 | root | `√` |
| 24 | not | `¬` |
| 25 | and | `∧` (dyadic) |
| 26 | or | `∨` (dyadic) |
| 27 | feq | `≡` match |
| 28 | couple | `≍` |
| 29 | shifta | `«` |
| 30 | shiftb | `»` |
| 31 | reverse | `⌽` |
| 32 | transp | `⍉` |
| 33 | gradeUp | `⍋` (monadic) |
| 34 | gradeDown | `⍒` (monadic) |
| 35 | indexOf | `⊐` (dyadic) |
| 36 | count | `⊒` (dyadic) |
| 37 | memberOf | `∊` (dyadic) |
| 38 | cell | `˘` cells modifier |
| 39 | rank | `⎉` rank modifier |

### 3.3 Compiler Interface

The compiler is called as: `env BQN_compile source_string`

Where `env` (left argument) contains up to 4 elements:
1. **Runtime** - the 64-value primitive list
2. **System function resolver** - maps normalized names to system values
3. **Variable names** - for REPL continuation
4. **Variable depths** - scope info for REPL

The compiler returns bytecode in the standard format:
`⟨bytecode, constants, blocks, bodies, indices, token_info, source⟩`

### 3.4 Formatter Interface

The formatter receives `⟨Type, Decompose, Glyph, FmtNum⟩` and returns `⟨Fmt, Repr⟩`.

---

## 4. VM Specification

### 4.1 Complete Opcode Table

**Stack/Constants:**
| Code | Name | Args | Description |
|------|------|------|-------------|
| 0x00 | PUSH | I | Push constant object[I] |
| 0x01 | DFND | I | Define and push block[I] |
| 0x02 | SYSV | I | Push system value[I] |
| 0x06 | POPS | - | Pop and discard |
| 0x07 | RETN | - | Return top of stack |
| 0x08 | RETD | - | Return scope as namespace |

**List/Array Construction:**
| Code | Name | Args | Description |
|------|------|------|-------------|
| 0x0B | LSTO | N | Create N-element list |
| 0x0C | LSTM | N | Create N-element reference list |
| 0x0D | ARMO | N | Create merged array (for `[a,b,c]` syntax) |
| 0x0E | ARMM | N | Create merged reference array |

**Function Calls:**
| Code | Name | Description |
|------|------|-------------|
| 0x10 | FN1C | Monadic call: F x |
| 0x11 | FN2C | Dyadic call: w F x |
| 0x12 | FN1O | FN1C but check for Nothing |
| 0x13 | FN2O | FN2C but check for Nothing |

**Trains:**
| Code | Name | Description |
|------|------|-------------|
| 0x14 | TR2D | 2-train: (G H) |
| 0x15 | TR3D | 3-train: (F G H) |
| 0x16 | CHKV | Error if top is Nothing |
| 0x17 | TR3O | 3-train checking for Nothing in F position |

**Modifiers:**
| Code | Name | Description |
|------|------|-------------|
| 0x1A | MD1C | Apply 1-modifier: F_m |
| 0x1B | MD2C | Apply 2-modifier: F_m_G |
| 0x1C | MD2L | Partial bind left operand to 2-modifier |
| 0x1D | MD2R | Partial bind right operand to 2-modifier |

**Variables:**
| Code | Name | Args | Description |
|------|------|------|-------------|
| 0x20 | VARO | D,I | Push var[I] from depth D |
| 0x21 | VARM | D,I | Push var reference for assignment |
| 0x22 | VARU | D,I | Push and clear var (last use) |
| 0x26 | DYNO | I | Push dynamic/named variable |
| 0x27 | DYNM | I | Push dynamic variable reference |

**Headers & Predicates:**
| Code | Name | Description |
|------|------|-------------|
| 0x2A | PRED | Check predicate condition |
| 0x2B | VFYM | Convert constant to match pattern |
| 0x2C | NOTM | Push "nothing" matcher (for `·`) |
| 0x2F | SETH | Set header (match and bind) |

**Assignment:**
| Code | Name | Description |
|------|------|-------------|
| 0x30 | SETN | Define new variable (`←`) |
| 0x31 | SETU | Update variable (`↩`) |
| 0x32 | SETM | Modify variable (`F↩`) |
| 0x33 | SETC | Monadic modify (`F↩ v`) |

**Namespaces:**
| Code | Name | Description |
|------|------|-------------|
| 0x40 | FLDO | Read field from namespace |
| 0x41 | FLDM | Push mutable field reference |
| 0x42 | ALIM | Alias field for mutable access |

### 4.2 Block Type Encoding

Block metadata: `[type, immediateness, ...body_indices]`
- **Type 0**: Function or immediate block
- **Type 1**: 1-modifier
- **Type 2**: 2-modifier
- **Immediateness 0**: Deferred (creates closure)
- **Immediateness 1**: Immediate (executes now)

Body metadata: `[bc_offset, var_count, var_names, export_mask]`

### 4.3 Internal/Optimized Opcodes (RBQN already has these)

RBQN already extends the base opcodes with compiled-down optimized variants:
- `EXTO/EXTM/EXTU` - External variable access
- `ADDI/ADDU` - Inline immediate values
- `FN1Ci/FN2Ci/FN1Oi/FN2Oi` - Inline function calls
- `SETNi/SETUi/SETMi/SETCi` - Inline assignments
- `SETH1/SETH2` - Optimized header match
- `PRED1/PRED2` - Optimized predicates
- `DFND0/DFND1/DFND2` - Specialized block definition
- `FAIL` - Explicit failure

---

## 5. Block Types & Headers

### 5.1 Block Types

| Type | Contains | Behavior |
|------|----------|----------|
| **Immediate** | No special names | Executes immediately when encountered |
| **Function** | `𝕨`/`𝕩`/`𝕤` | Creates closure, executes on call |
| **1-Modifier** (immediate) | `𝔽` only | Executes when operand bound |
| **1-Modifier** (deferred) | `𝔽` + `𝕨`/`𝕩`/`𝕤` | Creates derived function |
| **2-Modifier** (immediate) | `𝔽`/`𝔾` only | Executes when operands bound |
| **2-Modifier** (deferred) | `𝔽`/`𝔾` + `𝕨`/`𝕩`/`𝕤` | Creates derived function |

### 5.2 Header Syntax (Complete Forms)

**Function headers:**
```
{ 𝕊 𝕩: body }               -- Monadic with self-ref
{ 𝕨 𝕊 𝕩: body }             -- Dyadic with self-ref
{ 𝕩: body }                  -- Monadic (implicit 𝕊)
{ 𝕨 F 𝕩: body }             -- Dyadic with named function
```

**Modifier headers:**
```
{ F _𝕣: body }               -- Immediate 1-mod
{ F _𝕣_ G: body }            -- Immediate 2-mod
{ F _𝕣 𝕩: body }             -- Deferred 1-mod monadic
{ 𝕨 F _𝕣 𝕩: body }          -- Deferred 1-mod dyadic
```

**Case headers (pattern matching):**
```
{ 𝕊 0: "zero" ; 𝕊 1: "one" ; 𝕩: "other" }
```

**Inverse headers:**
```
{ 𝕊⁼𝕩: inverse_body ; 𝕊 𝕩: forward_body }
{ 𝕨𝕊⁼𝕩: inverse ; 𝕨 𝕊 𝕩: forward }
{ 𝕨𝕊˜⁼𝕩: swap_inverse }
```

### 5.3 Destructuring in Headers

Headers support pattern matching with:
- **List destructuring**: `{ 𝕊 ⟨a,b,c⟩: ... }`
- **Array destructuring**: `{ 𝕊 [a,b,c]: ... }`
- **Constant matching**: `{ 𝕊 ⟨a,1,⟨b,·,2⟩⟩: ... }`
- **Nothing/discard**: `{ 𝕊 ⟨a,·,b⟩: ... }`

### 5.4 Predicates

Syntax: `condition ? body`

- Predicates appear after `;` in multi-body blocks
- Variables defined before `?` are scoped to that predicate check
- If predicate fails, next body is tried

### 5.5 Multiple Bodies

Bodies separated by `;`:
- Headerless: body 1 = monadic, body 2 = dyadic
- With headers: tried in order, first matching header executes
- All bodies must agree on block type

---

## 6. Namespaces

### 6.1 Creation

A block becomes a namespace when it contains export arrows (`⇐`):
```bqn
ns ← {
  field ⇐ 42
  Method ⇐ {𝕩+field}
  private ← "hidden"
}
```

The RETD opcode returns the running scope as a namespace object.

### 6.2 Access

- **Dot notation**: `ns.field`, `ns.Method`
- **Destructuring**: `⟨a, b⟩ ← ns`
- **Aliased destructuring**: `⟨myA⇐a, myB⇐b⟩ ← ns`
- **Chained access**: `a.b.c`

### 6.3 Mutability

Namespaces are mutable if their source uses `↩` to modify exported fields:
```bqn
counter ← { n ← 0 ⋄ n⇐ ⋄ Inc ⇐ { n +↩ 1 } }
counter.Inc @
counter.n  # 1
```

### 6.4 File-Level Namespaces

A file with `⇐` exports becomes a namespace when loaded with `•Import`.

---

## 7. Inverse System

### 7.1 Undo (`⁼`)

Given `F⁼ x`, finds `y` such that `x ≡ F y`.

**Required arithmetic inverses:**

| Function | Monadic Inverse | Dyadic Inverse |
|----------|----------------|----------------|
| `+` | `+` (identity) | `-˜` |
| `-` | `-` (negate) | `-` (subtract) |
| `×` | -- | `÷` |
| `÷` | `÷` (reciprocal) | `√` |
| `⋆` | Log | `÷˜○Log` |
| `√` | `×˜` (square) | `⋆˜` |
| `∧` | `÷˜` | -- |
| `¬` | `¬` | `¬` |

**Required structural inverses:**

| Function | Monadic Inverse | Dyadic Inverse |
|----------|----------------|----------------|
| `⊢` | `⊢` | `⊢` |
| `⊣` | `⊢` | Assert match, return |
| `<` | Disclose (rank-0 check) | -- |
| `⌽` | `⌽` | Negate-rotate |
| `⍉` | Cycle-transpose | Permutation-transpose |
| `/` | `≠¨∘⊔` | -- |

**Self/Swap inverses (`˜`):**

| Expression | Monadic Inverse | Dyadic Inverse |
|-----------|----------------|----------------|
| `+˜` | `÷⟜2` | `+⁼` |
| `-˜` | `+` | -- |
| `×˜` | `√` | `×⁼` |
| `÷˜` | `×` | -- |
| `⋆˜` | `√` | `√˜` |
| `∧˜` | `√` | `∧⁼` |
| `∨˜` | `√⌾¬` | `∨⁼` |

**Modifier inverses:**

| Modifier | Inverse Rule |
|----------|-------------|
| `¨` | `F⁼¨` (element-wise) |
| `⌜` | `F⁼⌜` (monadic only) |
| `˘` | `F⁼˘` (cells) |
| `∘` | `G⁼ F⁼` (Atop) |
| `○` | `G⁼ (G𝕨) F⁼` (Over) |
| `` ` `` | Complex scan inverse |
| `⍟n` | `⍟(-n)` |
| `⊘` | `(F⁼)⊘(G⁼)` |
| `⊸` | `F⊸(G⁼)` |
| `⟜` | `G⁼ F⁼` |

### 7.2 Under (`⌾`)

`𝕨 F⌾G 𝕩` satisfies: `(𝕨 F○G 𝕩) ≡ G z` where z is the result.

Three cases:
1. **Invertible Under**: G is uniquely invertible, compute `G⁼ v`
2. **Structural Under**: G is a structural function, insert transformed values back
3. **Computational Under**: Fall back to `G⁼ v` if G is provably non-structural

**Required structural functions for Under:**

Monadic structural: `⊣ ⊢ < > ∾ ⥊ ≍ ↑ ↓ ⌽ ⍉ ⊏ ⊑`

Dyadic structural: `⊢ ⥊ ↑ ↓ ↕ ⌽ ⍉ / ⊏ ⊑ ⊔`

### 7.3 User-Defined Inverses

Block headers can specify custom inverses:
```bqn
Double ← { 𝕊⁼𝕩: 𝕩÷2 ; 𝕊 𝕩: 𝕩×2 }
```

Three header forms: `𝕊⁼𝕩:`, `𝕨𝕊⁼𝕩:`, `𝕨𝕊˜⁼𝕩:`

### 7.4 Fill Elements

Fill values are default elements used by operations like Take and Group:
- Numbers fill with `0`
- Characters fill with `' '`
- Arrays fill recursively
- Arithmetic: fill computed by applying function to fill elements
- Selection: fill from argument (`⊢`): `∧ ∨ ⥊ ≍ » « ⌽ ⍉ ⊏ ⍷`
- Zero fill: `≢ / ⍋ ⍒ ∊ ⊐ ⊒`
- Intersection fill: for `> ∾` (monadic), `∾ ≍ » «` (dyadic)

### 7.5 Identity Elements (for Fold on Empty)

| Identity | Functions |
|----------|-----------|
| `0` | `+ - ≠ = > ≥ ∨` |
| `1` | `× ÷ ⋆ ¬ ∧` |
| `∞` | `⌊` |
| `¯∞` | `⌈` |

---

## 8. FFI / System Interface

### 8.1 `•FFI` (Foreign Function Interface)

**Calling convention:**
```bqn
"libpath" •FFI "result_type"‿"function_name"‿"arg_type1"‿"arg_type2"
```

**Supported numeric types:**
- Signed: `i8`, `i16`, `i32`, `i64`
- Unsigned: `u8`, `u16`, `u32`, `u64`
- Float: `f32`, `f64`

**Compound types:**
- Typed pointer: `*i32` (read-only), `&i32` (mutable/copy-out)
- Untyped pointer: `*`, `&`
- Fixed array: `[25]i32`
- Struct: `{i32,f64,*u8}`
- BQN value: `a` (implementation-specific BQNV)

**Type conversion:** `type:representation` (e.g., `*i64:i32` - BQN i32 list to C i64 array)

**Pointer object methods:** `Read`, `Write`, `Add`, `Sub`, `Cast`, `Field`

**Void result:** Empty string `""` returns `@`

### 8.2 `•SH` (Shell Execution)

```bqn
•SH 𝕩   # 𝕩 is command string or ⟨cmd, args...⟩
```

Returns `⟨exit_code, stdout, stderr⟩` as strings.

Optional namespace left argument with fields:
- `stdin`: string to feed to stdin
- `raw`: boolean, skip UTF-8 decoding

### 8.3 C/Rust API (for embedding)

CBQN provides a C API for embedding BQN in other programs:
- `bqn_init()` - Initialize runtime
- `bqn_eval(str)` - Evaluate BQN string
- `bqn_call1(f, x)` / `bqn_call2(f, w, x)` - Call BQN function
- Value creation/extraction functions
- Type checking functions

RBQN equivalent: expose `pub fn` Rust API for the same operations.

---

## 9. Test Suite

### 9.1 Test File Structure (`mlochbaum/BQN/test/cases/`)

| File | What It Tests | Priority for RBQN |
|------|--------------|-------------------|
| `simple.bqn` | Basic arithmetic, simple operations | **Critical** - first to pass |
| `literal.bqn` | Number/string/character literals | **Critical** - parsing |
| `token.bqn` | Tokenizer edge cases | High - compiler correctness |
| `syntax.bqn` | Assignment, trains, blocks, lists | **Critical** - language core |
| `bytecode.bqn` | VM opcode behavior | **Critical** - VM correctness |
| `prim.bqn` | All primitive operations | **Critical** - completeness |
| `header.bqn` | Block headers, pattern matching | High - advanced blocks |
| `unhead.bqn` | Headerless multi-body blocks | High - block dispatch |
| `namespace.bqn` | Namespace creation, access, mutation | High - namespaces |
| `fill.bqn` | Fill element propagation | Medium - correctness |
| `identity.bqn` | Fold/insert identity elements | Medium - edge cases |
| `under.bqn` | Under (`⌾`) operation | Medium - inverse system |
| `undo.bqn` | Undo (`⁼`) operation | Medium - inverse system |

### 9.2 Test Runners

| Script | Purpose |
|--------|---------|
| `test/this.bqn` | Self-test using BQN compiler |
| `test/unit.bqn` | Unit tests with configurable components |
| `test/js` | JavaScript/WASM testing |
| `test/dzaima` | Test with dzaima/BQN |

### 9.3 Test Flags

- Default: test compiler (`src/c.bqn`)
- `-nocomp`: test native execution only
- `-rt`: test compiler + runtime
- `-ref`: test against spec reference implementation

---

## 10. RBQN Current State vs Requirements

### 10.1 What RBQN Has

**VM (crates/rbqn-vm):**
- All 43 standard opcodes + optimized internal opcodes
- Block definition and execution (DFND, FN1C, FN2C, etc.)
- Scope/variable management (VARO, VARM, VARU, SETN, SETU, SETM, SETC)
- Namespace support (FLDO, FLDM, ALIM, RETD)
- Train construction (TR2D, TR3D, TR3O)
- Modifier application (MD1C, MD2C, MD2L, MD2R)
- Header matching (SETH, SETH1, SETH2)
- Predicate checking (PRED, PRED1, PRED2)
- Array merging (ARMO, ARMM)
- Bytecode compiler (compiler.rs) for loading pre-compiled bytecode

**Primitives (crates/rbqn-prim):**
- All 44 functions have monadic/dyadic implementations (exceptions: monadic `≤` and `≥` correctly have none; monadic `«` and `»` are missing)
- System functions: Type, Decompose, Glyph, Fill, GroupLen, GroupOrd
- Dispatch table for all 64 runtime entries

**Modifiers (rbqn-vm/modifiers.rs):**
- 1-modifiers: Const, Swap, Cells, Each, Table, Fold, Insert, Scan
- 2-modifiers: Atop, Over, Before, After, Valences, Choose, Repeat, Catch
- NOT implemented: Undo (`⁼`), Under (`⌾`), Rank (`⎉`), Depth (`⚇`)

**Bootstrap:**
- Build script loads pre-compiled bytecode from CBQN gen files
- Provide array construction (40 entries)
- Runtime0 execution (partially working - 7/9 tests pass)
- Runtime1/compiler/formatter NOT yet working

**Core (crates/rbqn-core):**
- NaN-boxed value type (B)
- Array type with typed storage (BqnArr)
- Element type system
- Squeeze optimization
- Fill element tracking
- Value comparison
- Value formatting (partial)

### 10.2 Gaps (What Needs Building)

**Critical (blocks bootstrap):**
1. Fix remaining runtime0 failures (`+´` and `+¨` tests)
2. Complete runtime1 execution
3. Load and execute the self-hosted compiler
4. Load and execute the formatter
5. Implement `•BQN` / `•ReBQN` (eval)

**High Priority (language completeness):**
6. Undo (`⁼`) - inverse resolution for all required functions/modifiers
7. Under (`⌾`) - structural and invertible under
8. Rank modifier (`⎉`) - apply at specified rank
9. Depth modifier (`⚇`) - apply at specified depth
10. Fill element propagation (complete rules)
11. Identity elements for fold on empty
12. Monadic `«` / `»` (nudge back / nudge)
13. Pervasive extension (deep array arithmetic)

**Medium Priority (system functions):**
14. `•Import` (file loading with caching)
15. `•FChars` / `•FLines` / `•FBytes` (file I/O)
16. `•file` namespace (full file operations)
17. `•Out` / `•Show` / `•Repr` / `•Fmt`
18. `•math` namespace (trig, combinatorial, etc.)
19. `•SH` (shell execution)
20. `•args` / `•path` / `•name` / `•wdpath` / `•state`
21. `•UnixTime` / `•MonoTime` / `•Delay`
22. `•MakeRand` / `•rand` (random number generation)
23. `•HashMap`
24. `•platform` namespace

**Lower Priority (advanced features):**
25. `•FFI` (foreign function interface)
26. `•bit` namespace (bitwise operations)
27. `•term` namespace (terminal I/O)
28. `•_while_` (loop modifier)
29. `•ns` namespace (namespace introspection)
30. User-defined inverses (`⁼` in headers)
31. `•Source` / `•Decompose` (full implementation)
32. `•ParseFloat`
33. `•Hash` / `•Cmp` (CBQN extensions)
34. `•FromUTF8` / `•ToUTF8`
35. `•CurrentError`
36. `•primitives`
37. `•Exit`

### 10.3 Primitive Implementation Gaps (Detail)

Functions with `c1: None` or incomplete implementations:
- `≤`: No monadic form (correct per spec)
- `≥`: No monadic form (correct per spec)
- `«` (idx 29): Missing monadic (Nudge Back)
- `»` (idx 30): Missing monadic (Nudge)

Modifiers with no native implementation yet:
- Undo (`⁼`, idx 49): Throws "not yet implemented"
- Under (`⌾`, idx 57): Throws "not yet implemented"
- Rank (`⎉`, idx 60): Throws "not yet implemented"
- Depth (`⚇`, idx 61): Throws "not yet implemented"

### 10.4 Estimated Scope Summary

| Category | Items | Status |
|----------|-------|--------|
| Functions (44) | 42/44 | Missing 2 monadic forms (`«` `»`) |
| 1-Modifiers (9) | 8/9 | Missing `⁼` |
| 2-Modifiers (11) | 7/11 | Missing `⌾` `⎉` `⚇` (+ `⁼` from above) |
| VM Opcodes (43+) | 43/43 | Complete + internal opcodes |
| Bootstrap phases | 1/4 | runtime0 partial, runtime1/compiler/formatter pending |
| System functions | 6/50+ | Only bootstrap-required ones |
| Inverse system | 0% | Empty file |
| Test suite compat | 0/13 | Requires working compiler |

---

## Sources

- [BQN Specification: Primitives](https://mlochbaum.github.io/BQN/spec/primitive.html) - HIGH confidence
- [BQN Specification: System Values](https://mlochbaum.github.io/BQN/spec/system.html) - HIGH confidence
- [BQN Specification: Inferred Properties](https://mlochbaum.github.io/BQN/spec/inferred.html) - HIGH confidence
- [BQN VM and Runtime](https://mlochbaum.github.io/BQN/implementation/vm.html) - HIGH confidence
- [BQN Documentation: Primitives](https://mlochbaum.github.io/BQN/doc/primitive.html) - HIGH confidence
- [BQN Documentation: Blocks](https://mlochbaum.github.io/BQN/doc/block.html) - HIGH confidence
- [BQN Documentation: Namespaces](https://mlochbaum.github.io/BQN/doc/namespace.html) - HIGH confidence
- [BQN Documentation: FFI](https://mlochbaum.github.io/BQN/doc/ffi.html) - HIGH confidence
- [BQN Documentation: Undo](https://mlochbaum.github.io/BQN/doc/undo.html) - HIGH confidence
- [BQN Documentation: Under](https://mlochbaum.github.io/BQN/doc/under.html) - HIGH confidence
- [BQN Grammar Specification](https://mlochbaum.github.io/BQN/spec/grammar.html) - HIGH confidence
- [BQN Source: glyphs.bqn](https://github.com/mlochbaum/BQN/blob/master/src/glyphs.bqn) - HIGH confidence
- [BQN Test README](https://github.com/mlochbaum/BQN/blob/master/test/README.txt) - HIGH confidence
- [CBQN System Functions](https://github.com/dzaima/CBQN/blob/master/docs/system.md) - HIGH confidence
- [CBQN Source README](https://github.com/dzaima/CBQN/blob/master/src/README.md) - HIGH confidence

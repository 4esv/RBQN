# Deferred Items from 04-01

## header.bqn Remaining Failures (7 + 1 stack overflow)

These failures are NOT namespace-related and are deferred to plan 04-05.

### 1. Predicate in immediate block
`"{2 ? 3;4}"` — should fail but received 4.
In an immediate block with no retry body, `?` predicate should throw "This block cannot be called with these arguments". Our PRED1 dispatch returns the predicate value instead of failing.

### 2. Header with literal number matching
`"({2:4;𝕩}¨≡⊢+2×2⊸=)↕4"` — received 0.
Block `{2:4;𝕩}` should match header `2:` (value=2) and return 4. When value doesn't match, falls to `𝕩`. Our literal header matching may not correctly handle numeric equality.

### 3. Destructuring in function header
`"⟨0‿1,2⟩≡{1‿b:b;𝕊:𝕩}¨+⟜<↕2"` — evaluation failed.
Block `{1‿b:b;𝕊:𝕩}` should match `1‿b:` when input is `1‿x`.

### 4. Strand type matching in header
`"0‿1‿'c'‿3 ≡ {S a‿2:a;S a‿b:b}¨"abcd"∾¨↕4"` — received 0.
Headers matching specific element values in strands.

### 5. Multi-header dyadic block
`"4‿2‿4‿3 ≡ {𝕨𝕊0:4;1+𝕩;𝕩×𝕨}¨{𝔽∾3𝔽⊢}↕2"` — received 0.
Block with `𝕨𝕊0:4` header (matches when dyadic 𝕨=𝕊 and 𝕩=0).

### 6. Modifier header with operand args
`"4 3{4 3 _𝕣_ 2 1:𝕨÷𝕘}2 1"` — evaluation failed.
2-modifier with specific operand matching `4 3 _𝕣_ 2 1:`.

### 7. Recursive modifier with multi-header (STACK OVERFLOW)
`"2{⟨a,b⟩_r:a+b;a _r: a‿a _r}"` — expected 4.
Root cause: when `a _r:` matches (scalar left operand) and runs `a‿a _r` (recursive self-call), the dispatch re-enters the SAME header selection but with array operand. Our dispatch incorrectly retries with the original scalar operand instead of the new array, causing infinite recursion.
Fix: ensure that recursive modifier calls use the new operand (not the cached original) in SETH dispatch.

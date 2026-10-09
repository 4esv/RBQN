#!/bin/bash
# Default mode (and force) vs RBQN_GPU=off on lazy-tree, fold, scan and sort cases.
# Usage: bench/thresholds/loop.sh RBQN_BIN [modes...]   (modes default: default)
# Prints PASS/FAIL per case and size; exit status 1 on any FAIL.
B=${1:?rbqn binary}; shift
MODES=${*:-default}
# S summarises an array without printing it: two picks (materialize) and folds.
P='S←{⟨37⊑𝕩, ¯1⊑𝕩, +´𝕩, ⌈´𝕩, ⌊´𝕩⟩} ⋄ a←↕N ⋄ b←N⥊¯3‿5‿7 ⋄ c←N⥊0‿1‿1 ⋄ d←2×N⥊1.5‿¯2 ⋄ '
CASES=(
  'S 1+a'
  'S a-3'
  'S 3-a'
  'S 2×a'
  'S a⌊5'
  'S 5⌈a'
  'S a+b'
  'S a×b'
  'S a-b'
  'S b+c'
  'S d+a'
  'S c×b'
  '+´ 1+2×3+4×a'
  '+´ (a×a)+(a×a)'
  '+´ (a+a)+(a+a)'
  '⌈´ a-b'
  '⌊´ b×3'
  '×´ c⌊1'
  '+´ +` b'
  '⌈´ +` a'
  'S +` c+b'
  'x←1+a ⋄ ⟨+´x, S x, +´ x+x⟩'
  'x←a×b ⋄ y←x+1 ⋄ ⟨+´y, +´x, S y⟩'
  'S 1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+1+a'
  'S (a+b)+(a-b)+(b+c)+(c+d)+(d+a)+(a×c)+(b×c)+(b-d)+(c-a)'
  'S a×a×a'
  '≠ 1+a'
  '+´ a'
)
fail=0
for N in 100000 10000000; do
  for x in "${CASES[@]}"; do
    e="${P//N/$N}$x"
    ref=$(RBQN_GPU=off "$B" -p "$e" 2>&1)
    for m in $MODES; do
      got=$(RBQN_GPU=$m "$B" -p "$e" 2>&1)
      if [ "$got" == "$ref" ]; then r=PASS; else r=FAIL; fail=1; fi
      printf '%s %-7s n=%-8s %s\n' "$r" "$m" "$N" "$x"
      [ $r == FAIL ] && printf '   off: %s\n   %s: %s\n' "$ref" "$m" "$got"
    done
  done
done
for N in 100000 1000000 10000000; do
  e="s←∧ $N⥊↕1000 ⋄ ⟨+´s, +´ s×1+7|↕≠s, ∧´ (1↓s) ≥ ¯1↓s, ⊑s, ¯1⊑s⟩"
  ref=$(RBQN_GPU=off "$B" -p "$e" 2>&1)
  for m in $MODES; do
    got=$(RBQN_GPU=$m "$B" -p "$e" 2>&1)
    if [ "$got" == "$ref" ]; then r=PASS; else r=FAIL; fail=1; fi
    printf '%s %-7s n=%-8s %s\n' "$r" "$m" "$N" "∧ N⥊↕1000"
  done
done
exit $fail

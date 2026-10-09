#!/bin/bash
# Compare RBQN vs CBQN wall time on a fixed expression set.
# Usage: bench/compare.sh [RBQN_BIN] [CBQN_BIN]   (defaults: target/release/rbqn, bqn)
# Prints a markdown table: expression | cbqn s | rbqn s | ratio
set -u
RBQN="${1:-$(dirname "$0")/../target/release/rbqn}"
CBQN="${2:-bqn}"
EXPRS=(
  '1'
  '+´↕10000000'
  '+´×˜↕10000000'
  '+´ {𝕩+1}¨ ↕1000000'
  '{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 25'
  '≠ ⊐ 3000000⥊↕1000'
  '+´⥊ (↕3000) ×⌜ ↕3000'
  '+´ (⊢ ⍋⊸⊏ ⊢) 1000000⥊3‿1‿2'
  '≠ •Fmt ↕100000'
  '+´ +` ↕5000000'
  '+´ 2 × ↕5000000'
  '≠ ∾ 100000⥊⟨"ab","cde"⟩'
  '+´ 5000000⥊1'
)
t() { local s e; s=$(python3 -c 'import time;print(time.time())'); "$1" -p "$2" >/dev/null 2>&1; e=$(python3 -c 'import time;print(time.time())'); python3 -c "print(f'{$e-$s:.3f}')"; }
printf '| expression | cbqn s | rbqn s | ratio |\n|---|---:|---:|---:|\n'
for x in "${EXPRS[@]}"; do
  c=$(t "$CBQN" "$x"); r=$(t "$RBQN" "$x")
  ratio=$(python3 -c "c=$c;r=$r;print(f'{r/c:.0f}x' if c>0.002 else f'{r/0.002:.0f}x+')")
  printf '| `%s` | %s | %s | %s |\n' "$x" "$c" "$r" "$ratio"
done

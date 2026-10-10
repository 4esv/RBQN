#!/bin/bash
# Compare RBQN vs CBQN wall time on a fixed expression set, using hyperfine.
# Usage: bench/compare.sh [RBQN_BIN] [CBQN_BIN]   (defaults: target/release/rbqn, bqn)
# Prints a markdown table: expression | cbqn ms | rbqn ms | ratio  (mean of 5 runs, 2 warmups)
set -u
RBQN="${1:-$(dirname "$0")/../target/release/rbqn}"
CBQN="${2:-bqn}"
HF="${HYPERFINE:-/opt/homebrew/bin/hyperfine}"
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
  '≠ •Fmt ↕1000000'
  '{𝕩<2 ? 𝕩 ; (𝕊 𝕩-1)+𝕊 𝕩-2} 27'
  '+´ {𝕩×𝕩}¨ ↕1000000'
)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
# Mean wall time in ms for one binary on one expression.
# -N runs without a shell; hyperfine splits the command shell-style, so expressions must not contain '.
t() {
  "$HF" -N -w 2 -r 5 --export-json "$TMP/r.json" -- "'$1' -p '$2'" >/dev/null 2>&1 \
    || { echo "NaN"; return; }
  python3 -c 'import json,sys;print(f"{json.load(open(sys.argv[1]))["results"][0]["mean"]*1000:.1f}")' "$TMP/r.json"
}
printf '| expression | cbqn ms | rbqn ms | ratio |\n|---|---:|---:|---:|\n'
for x in "${EXPRS[@]}"; do
  c=$(t "$CBQN" "$x"); r=$(t "$RBQN" "$x")
  ratio=$(python3 -c "c=float('$c');r=float('$r');print(f'{r/c:.1f}x')")
  printf '| `%s` | %s | %s | %s |\n' "$x" "$c" "$r" "$ratio"
done

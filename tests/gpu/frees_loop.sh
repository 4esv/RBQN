#!/bin/bash
# Frees between dispatches (#163): every corpus row runs as one process that
# interleaves the row with allocation churn that frees arrays, so addresses get
# reused while the GPU upload caches are live. Output must match --no-gpu.
# Usage: ./tests/gpu/frees_loop.sh   (RBQN_QUARANTINE=1 is set)
set -uo pipefail
RBQN_BIN="${RBQN_BIN:-target/release/rbqn}"
CORPUS="$(cd "$(dirname "$0")" && pwd)/corpus.txt"
pass=0; fail=0
while IFS= read -r line; do
    [[ -z "$line" || "$line" == \#* ]] && continue
    expr="$line"
    while [[ "$expr" =~ ^[@~] ]]; do expr="${expr:1}"; expr="${expr# }"; done
    prog="Row ← {𝕊: ($expr)}
Churn ← {𝕊: ⊑ 1↑ +⌜˜ ↕100}
{𝕊: Churn@ ⋄ Row@}¨ ↕1
Churn¨ ↕3000
first ← Row@
Churn¨ ↕3000
•Out •Repr first ≡ Row@
•Out •Repr first"
    prog_file=$(mktemp); printf '%s\n' "$prog" > "$prog_file"
    g=$(RBQN_QUARANTINE=1 RBQN_GPU=force "$RBQN_BIN" "$prog_file" 2>&1)
    c=$(RBQN_QUARANTINE=1 "$RBQN_BIN" --no-gpu "$prog_file" 2>&1)
    rm -f "$prog_file"
    if # NOTE: a row that errors in the corpus (Softmax ⟨⟩) errors identically in both legs.
    [[ "$g" == "$c" && ( "$g" == 1* || "$g" == "Error: •math."* ) ]]; then pass=$((pass+1)); else fail=$((fail+1)); echo "FAIL: $expr"; echo "  gpu: $g" | head -3; echo "  cpu: $c" | head -3; fi
done < "$CORPUS"
echo "frees loop: $pass pass, $fail fail"
[[ $fail -eq 0 ]]

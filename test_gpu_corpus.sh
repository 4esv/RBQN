#!/bin/bash
# GPU kernel correctness corpus: each row runs three ways and must agree.
#   GPU:  RBQN_GPU=force rbqn -p EXPR   (must report dispatches>0)
#   CPU:  rbqn --no-gpu -p EXPR
#   CBQN: $CBQN_BIN -p EXPR             (skipped if CBQN_BIN unset)
#
# Row prefixes in tests/gpu/corpus.txt:
#   @  RBQN-only (•math.MatMul, •math.Softmax): no CBQN leg
#   ~  GPU may decline (empty input): dispatch not required
#
# Usage: [CBQN_BIN=/path/to/BQN] ./test_gpu_corpus.sh [--verbose]

set -uo pipefail

RBQN_BIN="${RBQN_BIN:-target/release/rbqn}"
CBQN_BIN="${CBQN_BIN:-${CBQN_PATH:+$CBQN_PATH/BQN}}"
CORPUS="${CORPUS:-$(cd "$(dirname "$0")" && pwd)/tests/gpu/corpus.txt}"
VERBOSE=0
[[ "${1:-}" == "--verbose" ]] && VERBOSE=1

if [[ ! -x "$RBQN_BIN" ]]; then
    echo "Error: RBQN_BIN=$RBQN_BIN not found. Run 'cargo build --release' first." >&2
    exit 1
fi
if [[ -n "${CBQN_BIN:-}" && ! -x "$CBQN_BIN" ]]; then
    echo "Error: CBQN_BIN=$CBQN_BIN is not executable" >&2
    exit 1
fi
[[ -z "${CBQN_BIN:-}" ]] && echo "NOTE: CBQN_BIN unset, CBQN leg skipped"

# The GPU must exist, or every row silently compares CPU with CPU.
probe=$(RBQN_GPU=force RBQN_GPU_DEBUG=1 "$RBQN_BIN" -p '+´↕100' 2>&1 >/dev/null)
if ! grep -q 'dispatches=[1-9]' <<<"$probe"; then
    echo "Error: no GPU dispatch under RBQN_GPU=force; is a GPU adapter available?" >&2
    echo "$probe" >&2
    exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
pass=0 fail=0 rows=0

show() { head -c 300 "$1"; [[ $(wc -c <"$1") -gt 300 ]] && echo " …"; }

while IFS= read -r line; do
    [[ -z "$line" || "$line" =~ ^# ]] && continue
    expr="$line" rbqn_only=0 may_decline=0
    while [[ "$expr" =~ ^[@~] ]]; do
        [[ "${expr:0:1}" == "@" ]] && rbqn_only=1
        [[ "${expr:0:1}" == "~" ]] && may_decline=1
        expr="${expr:1}"
        expr="${expr# }"
    done
    rows=$((rows + 1))

    RBQN_GPU=force RBQN_GPU_DEBUG=1 "$RBQN_BIN" -p "$expr" >"$tmp/gpu" 2>"$tmp/gpu.err"
    grep -v '^\[gpu\]\|^gpu: ' "$tmp/gpu.err" >>"$tmp/gpu"
    "$RBQN_BIN" --no-gpu -p "$expr" >"$tmp/cpu" 2>&1

    why=""
    if ! cmp -s "$tmp/gpu" "$tmp/cpu"; then
        why="GPU != CPU"
    elif [[ $may_decline -eq 0 ]] && ! grep -q 'dispatches=[1-9]' "$tmp/gpu.err"; then
        why="no GPU dispatch"
    elif [[ $rbqn_only -eq 0 && -n "${CBQN_BIN:-}" ]]; then
        "$CBQN_BIN" -p "$expr" >"$tmp/cbqn" 2>&1
        cmp -s "$tmp/cbqn" "$tmp/cpu" || why="CBQN != RBQN"
    fi

    if [[ -z "$why" ]]; then
        pass=$((pass + 1))
        if [[ $VERBOSE -eq 1 ]]; then
            echo "  PASS: $expr  ($(grep -o 'dispatches=[0-9]*' "$tmp/gpu.err"))"
        fi
    else
        fail=$((fail + 1))
        echo "  FAIL ($why): $expr"
        echo "    GPU:  $(show "$tmp/gpu")"
        echo "    CPU:  $(show "$tmp/cpu")"
        [[ "$why" == "CBQN != RBQN" ]] && echo "    CBQN: $(show "$tmp/cbqn")"
    fi
done <"$CORPUS"

echo "GPU corpus: $pass/$rows pass, $fail fail"
[[ $fail -eq 0 ]]

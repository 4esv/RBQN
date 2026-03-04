#!/bin/bash
# RBQN vs CBQN Parity Validation
# Runs a BQN expression corpus through both implementations and diffs output.
#
# Usage:
#   CBQN_BIN=/path/to/BQN ./test_parity.sh
#   CBQN_BIN=/path/to/BQN ./test_parity.sh --verbose

set -euo pipefail

RBQN_BIN="${RBQN_BIN:-target/release/rbqn}"
CBQN_BIN="${CBQN_BIN:-${CBQN_PATH:+$CBQN_PATH/BQN}}"
CORPUS_DIR="$(cd "$(dirname "$0")" && pwd)/tests/parity"
VERBOSE=0

for arg in "$@"; do
    case "$arg" in
        --verbose) VERBOSE=1 ;;
    esac
done

# Colors
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'
else
    RED=''; GREEN=''; BOLD=''; DIM=''; NC=''
fi

# Validation
if [[ -z "${CBQN_BIN:-}" ]]; then
    echo -e "${RED}Error: CBQN_BIN not set. Point it to the CBQN binary.${NC}" >&2
    echo "  Example: CBQN_BIN=/path/to/CBQN/BQN ./test_parity.sh" >&2
    echo "  Or set CBQN_PATH: CBQN_PATH=/path/to/CBQN ./test_parity.sh" >&2
    exit 1
fi
if [[ ! -x "$CBQN_BIN" ]]; then
    echo -e "${RED}Error: CBQN_BIN=$CBQN_BIN is not executable${NC}" >&2
    exit 1
fi
if [[ ! -x "$RBQN_BIN" ]]; then
    echo -e "${RED}Error: RBQN_BIN=$RBQN_BIN not found. Run 'cargo build --release' first.${NC}" >&2
    exit 1
fi
if [[ ! -d "$CORPUS_DIR" ]]; then
    echo -e "${RED}Error: Corpus directory $CORPUS_DIR not found${NC}" >&2
    exit 1
fi

TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0

run_category() {
    local file="$1"
    local category
    category=$(basename "$file" .txt)
    local pass=0 fail=0 skip=0

    while IFS= read -r expr; do
        # Skip comments and empty lines
        [[ -z "$expr" ]] && continue
        [[ "$expr" =~ ^# ]] && continue

        # Optional tests (prefixed with ?)
        local optional=0
        if [[ "$expr" =~ ^\? ]]; then
            optional=1
            expr="${expr:1}"
            expr="${expr# }"
        fi

        cbqn_out=$("$CBQN_BIN" -p "$expr" 2>&1 || true)
        rbqn_out=$("$RBQN_BIN" -p "$expr" 2>&1 || true)

        if [[ "$cbqn_out" == "$rbqn_out" ]]; then
            ((pass++))
            [[ $VERBOSE -eq 1 ]] && echo -e "  ${GREEN}PASS${NC}: $expr"
        elif [[ $optional -eq 1 ]]; then
            ((skip++))
            [[ $VERBOSE -eq 1 ]] && echo -e "  ${DIM}SKIP${NC}: $expr"
        else
            ((fail++))
            echo -e "  ${RED}FAIL${NC} [$category]: $expr"
            echo "    CBQN: $cbqn_out"
            echo "    RBQN: $rbqn_out"
        fi
    done < "$file"

    local total=$((pass + fail))
    if [[ $fail -eq 0 ]]; then
        printf "  ${GREEN}%-14s %d/%d pass${NC}\n" "$category" "$pass" "$total"
    else
        printf "  ${RED}%-14s %d/%d pass, %d fail${NC}\n" "$category" "$pass" "$total" "$fail"
    fi

    TOTAL_PASS=$((TOTAL_PASS + pass))
    TOTAL_FAIL=$((TOTAL_FAIL + fail))
    TOTAL_SKIP=$((TOTAL_SKIP + skip))
}

echo -e "${BOLD}=== RBQN Parity Report ===${NC}"
echo ""

for file in "$CORPUS_DIR"/*.txt; do
    [[ -f "$file" ]] || continue
    run_category "$file"
done

echo "──────────────────────────────"
total=$((TOTAL_PASS + TOTAL_FAIL))
if [[ $TOTAL_SKIP -gt 0 ]]; then
    skip_msg=", $TOTAL_SKIP skip"
else
    skip_msg=""
fi

if [[ $TOTAL_FAIL -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}Total: $TOTAL_PASS/$total pass, 0 fail${skip_msg}${NC}"
else
    echo -e "${RED}${BOLD}Total: $TOTAL_PASS/$total pass, $TOTAL_FAIL fail${skip_msg}${NC}"
fi

if [[ $TOTAL_FAIL -gt 0 ]]; then
    exit 1
fi

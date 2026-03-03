#!/bin/bash

# RBQN vs CBQN Compatibility Test Harness
# Compares output across 5 tiers of BQN expression complexity

CBQN_BIN="/Users/axel/Code/forks/CBQN/BQN"
RBQN_BIN="target/release/rbqn"
TIER="${1:-all}"

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

PASS=0
FAIL=0

# Test a single expression
test_expr() {
  local tier=$1
  local expr=$2

  # Get CBQN output
  cbqn_out=$("$CBQN_BIN" -p "$expr" 2>&1 || true)
  cbqn_exit=$?

  # Get RBQN output
  rbqn_out=$("$RBQN_BIN" -p "$expr" 2>&1 || true)
  rbqn_exit=$?

  # Compare outputs and exit codes
  if [[ "$cbqn_out" == "$rbqn_out" ]] && [[ $cbqn_exit -eq $rbqn_exit ]]; then
    echo -e "${GREEN}✓ PASS${NC} [T$tier] $expr"
    ((PASS++))
  else
    echo -e "${RED}✗ FAIL${NC} [T$tier] $expr"
    echo "  CBQN: $cbqn_out (exit: $cbqn_exit)"
    echo "  RBQN: $rbqn_out (exit: $rbqn_exit)"
    ((FAIL++))
  fi
}

# Tier 1: Arithmetic
if [[ "$TIER" == "all" ]] || [[ "$TIER" == "1" ]]; then
  echo "=== Tier 1: Arithmetic ==="
  test_expr 1 "1+1"
  test_expr 1 "3×4"
  test_expr 1 "2⋆10"
  test_expr 1 "÷4"
  test_expr 1 "|¯5"
  test_expr 1 "⌊3.7"
  test_expr 1 "⌈3.2"
  echo
fi

# Tier 2: Arrays
if [[ "$TIER" == "all" ]] || [[ "$TIER" == "2" ]]; then
  echo "=== Tier 2: Arrays ==="
  test_expr 2 "↕5"
  test_expr 2 "3‿4‿5"
  test_expr 2 "≢3‿4‿5"
  test_expr 2 "⌽\"abc\""
  test_expr 2 "3⥊1"
  echo
fi

# Tier 3: Modifiers
if [[ "$TIER" == "all" ]] || [[ "$TIER" == "3" ]]; then
  echo "=== Tier 3: Modifiers ==="
  test_expr 3 "+´↕10"
  test_expr 3 "×´1+↕5"
  test_expr 3 "+\`↕5"
  test_expr 3 "+˜3"
  test_expr 3 "+¨1‿2‿3"
  echo
fi

# Tier 4: Compound
if [[ "$TIER" == "all" ]] || [[ "$TIER" == "4" ]]; then
  echo "=== Tier 4: Compound ==="
  test_expr 4 "+´∘×˜ 3‿4"
  test_expr 4 "\"hello\"∾\" world\""
  echo
fi

# Tier 5: Blocks
if [[ "$TIER" == "all" ]] || [[ "$TIER" == "5" ]]; then
  echo "=== Tier 5: Blocks ==="
  test_expr 5 "{𝕩+1}5"
  test_expr 5 "2{𝕨×𝕩}3"
  echo
fi

# Summary
echo "=== Summary ==="
echo -e "${GREEN}Pass: $PASS${NC}"
echo -e "${RED}Fail: $FAIL${NC}"
total=$((PASS + FAIL))
if [[ $FAIL -eq 0 ]]; then
  exit 0
else
  exit 1
fi

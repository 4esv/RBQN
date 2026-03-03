#!/bin/bash
# RBQN Test Suite Runner & Regression Detector
# Usage:
#   ./test_suite.sh              Run all tests, save baseline
#   ./test_suite.sh --diff       Run tests and diff against last baseline
#   ./test_suite.sh --save TAG   Run tests and save as named snapshot
#   ./test_suite.sh --compare A B  Compare two saved snapshots
#   ./test_suite.sh --bisect     Git bisect mode (exit 0=good, 1=bad, 125=skip)
#   ./test_suite.sh --quick      Only run the 5 failing files (fast iteration)
#   ./test_suite.sh FILE...      Run specific test files only

set -euo pipefail

RBQN_ROOT="$(cd "$(dirname "$0")" && pwd)"
RBQN="$RBQN_ROOT/target/release/rbqn"
BQN_TEST_DIR="/Users/axel/Code/forks/BQN/test"
RESULTS_DIR="$RBQN_ROOT/.test-results"
CBQN_PATH="${CBQN_PATH:-/Users/axel/Code/forks/CBQN}"

ALL_FILES=(simple literal syntax bytecode token namespace identity unhead prim fill header under undo)
QUICK_FILES=(prim fill header under undo)

# Colors (skip if not a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[0;33m'
    CYAN='\033[0;36m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'
else
    RED=''; GREEN=''; YELLOW=''; CYAN=''; BOLD=''; DIM=''; NC=''
fi

mkdir -p "$RESULTS_DIR"

# --- Helpers ---

build_rbqn() {
    echo -e "${DIM}Building RBQN (release)...${NC}"
    if ! CBQN_PATH="$CBQN_PATH" cargo build --release --manifest-path="$RBQN_ROOT/Cargo.toml" 2>&1 | tail -1; then
        echo -e "${RED}Build failed${NC}"
        return 1
    fi
    if [[ ! -x "$RBQN" ]]; then
        echo -e "${RED}Build failed — binary not found${NC}"
        return 1
    fi
}

# Run a single test file, return summary line
run_test_file() {
    local file="$1"
    local output exit_code=0

    output=$(cd "$BQN_TEST_DIR" && "$RBQN" this.bqn "$file" 2>&1) || exit_code=$?

    local last_line
    last_line=$(echo "$output" | tail -1)

    if [[ "$last_line" == "All passed!" ]]; then
        # Extract test count from "Running N tests: file"
        local count
        count=$(echo "$output" | sed -n 's/^Running \([0-9]*\) tests:.*/\1/p')
        [[ -z "$count" ]] && count="?"
        echo "PASS:$count"
    elif [[ "$last_line" =~ ^([0-9]+)\ failed! ]]; then
        echo "FAIL:${BASH_REMATCH[1]}"
    elif [[ $exit_code -ne 0 ]]; then
        echo "CRASH:$exit_code"
    else
        echo "UNKNOWN"
    fi
}

# Run a single test file, return full detail (header + failure lines)
run_test_file_detail() {
    local file="$1"
    local output exit_code=0

    output=$(cd "$BQN_TEST_DIR" && "$RBQN" this.bqn "$file" 2>&1) || exit_code=$?

    local last_line
    last_line=$(echo "$output" | tail -1)

    if [[ "$last_line" == "All passed!" ]]; then
        local count
        count=$(echo "$output" | sed -n 's/^Running \([0-9]*\) tests:.*/\1/p')
        [[ -z "$count" ]] && count="0"
        echo "PASS|$count|0"
    elif [[ "$last_line" =~ ^([0-9]+)\ failed! ]]; then
        local failed="${BASH_REMATCH[1]}"
        echo "FAIL|0|$failed"
        # Extract individual failure lines (start with ")
        echo "$output" | grep '^"' || true
    elif [[ $exit_code -ne 0 ]]; then
        echo "CRASH|0|$exit_code"
        echo "$output" | tail -5
    else
        echo "UNKNOWN|0|0"
    fi
}

save_results() {
    local tag="$1"
    local results_file="$RESULTS_DIR/$tag.txt"
    local detail_file="$RESULTS_DIR/$tag.detail.txt"
    local files=("${@:2}")

    local total_pass=0 total_fail=0
    local timestamp commit
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    commit=$(git -C "$RBQN_ROOT" rev-parse --short HEAD 2>/dev/null || echo "unknown")

    echo "# RBQN Test Results — $tag" > "$results_file"
    echo "# commit: $commit" >> "$results_file"
    echo "# date: $timestamp" >> "$results_file"
    echo "" >> "$results_file"

    : > "$detail_file"

    for file in "${files[@]}"; do
        local detail header status passed failed
        detail=$(run_test_file_detail "$file")
        header=$(echo "$detail" | head -1)
        status=$(echo "$header" | cut -d'|' -f1)
        passed=$(echo "$header" | cut -d'|' -f2)
        failed=$(echo "$header" | cut -d'|' -f3)

        printf "%-12s %s\n" "$file" "$header" >> "$results_file"

        echo "=== $file ===" >> "$detail_file"
        echo "$detail" >> "$detail_file"
        echo "" >> "$detail_file"

        case "$status" in
            PASS) total_pass=$((total_pass + passed)) ;;
            FAIL) total_fail=$((total_fail + failed)) ;;
        esac
    done

    echo "" >> "$results_file"
    echo "# total_fail: $total_fail" >> "$results_file"

    echo "$results_file"
}

print_results() {
    local files=("$@")
    local total_pass=0 total_fail=0 green_files=0
    local total_files=${#files[@]}

    printf "\n${BOLD}%-12s  %s${NC}\n" "FILE" "RESULT"
    printf "%-12s  %s\n" "────────────" "──────────────────"

    for file in "${files[@]}"; do
        local result status count
        result=$(run_test_file "$file")
        status="${result%%:*}"
        count="${result#*:}"

        case "$status" in
            PASS)
                printf "${GREEN}%-12s  pass %s${NC}\n" "$file" "$count"
                green_files=$((green_files + 1))
                total_pass=$((total_pass + count))
                ;;
            FAIL)
                printf "${RED}%-12s  FAIL %s${NC}\n" "$file" "$count"
                total_fail=$((total_fail + count))
                ;;
            CRASH)
                printf "${RED}%-12s  CRASH (exit %s)${NC}\n" "$file" "$count"
                total_fail=$((total_fail + 1))
                ;;
            TIMEOUT)
                printf "${YELLOW}%-12s  TIMEOUT${NC}\n" "$file"
                total_fail=$((total_fail + 1))
                ;;
            *)
                printf "${YELLOW}%-12s  ? %s${NC}\n" "$file" "$result"
                ;;
        esac
    done

    printf "%-12s  %s\n" "────────────" "──────────────────"
    printf "${BOLD}%-12s  ${NC}" "TOTAL"

    if [[ $total_fail -eq 0 ]]; then
        printf "${GREEN}${BOLD}ALL GREEN (%d/%d files)${NC}\n" "$green_files" "$total_files"
    else
        printf "${RED}%d failed${NC} (%d/%d files green)\n" \
            "$total_fail" "$green_files" "$total_files"
    fi

    echo ""
    return $total_fail
}

diff_results() {
    local file_a="$1" file_b="$2"

    if [[ ! -f "$file_a" ]] || [[ ! -f "$file_b" ]]; then
        echo -e "${RED}Missing result file(s)${NC}"
        return 1
    fi

    local commit_a commit_b date_a date_b
    commit_a=$(grep '^# commit:' "$file_a" | cut -d' ' -f3)
    commit_b=$(grep '^# commit:' "$file_b" | cut -d' ' -f3)
    date_a=$(grep '^# date:' "$file_a" | cut -d' ' -f3)
    date_b=$(grep '^# date:' "$file_b" | cut -d' ' -f3)

    printf "\n${BOLD}Comparing:${NC}\n"
    printf "  A: %s (%s @ %s)\n" "$(basename "$file_a" .txt)" "$commit_a" "$date_a"
    printf "  B: %s (%s @ %s)\n" "$(basename "$file_b" .txt)" "$commit_b" "$date_b"
    printf "\n${BOLD}%-12s  %-20s  %-20s  %s${NC}\n" "FILE" "A" "B" "DELTA"
    printf "%-12s  %-20s  %-20s  %s\n" "────────────" "────────────────────" "────────────────────" "─────"

    local has_regression=0

    while IFS= read -r line; do
        [[ "$line" =~ ^#.*$ ]] && continue
        [[ -z "$line" ]] && continue

        local file_name status_a
        file_name=$(echo "$line" | awk '{print $1}')
        status_a=$(echo "$line" | awk '{print $2}')

        local line_b status_b
        line_b=$(grep "^$file_name " "$file_b" 2>/dev/null || echo "")
        status_b=$(echo "$line_b" | awk '{print $2}')

        if [[ -z "$status_b" ]]; then
            printf "%-12s  %-20s  ${DIM}%-20s${NC}  %s\n" "$file_name" "$status_a" "(missing)" ""
            continue
        fi

        local fail_a fail_b
        fail_a=$(echo "$status_a" | cut -d'|' -f3)
        fail_b=$(echo "$status_b" | cut -d'|' -f3)

        local delta=""
        if [[ "$fail_a" =~ ^[0-9]+$ ]] && [[ "$fail_b" =~ ^[0-9]+$ ]]; then
            local d=$((fail_b - fail_a))
            if [[ $d -gt 0 ]]; then
                delta="${RED}+${d} regression${NC}"
                has_regression=1
            elif [[ $d -lt 0 ]]; then
                delta="${GREEN}${d} fixed${NC}"
            else
                delta="${DIM}no change${NC}"
            fi
        elif [[ "$status_a" != "$status_b" ]]; then
            delta="${YELLOW}changed${NC}"
        else
            delta="${DIM}no change${NC}"
        fi

        printf "%-12s  %-20s  %-20s  " "$file_name" "$status_a" "$status_b"
        echo -e "$delta"
    done < <(grep -v '^#' "$file_a" | grep -v '^$')

    echo ""

    # Diff the detail files to show individual test regressions/fixes
    local detail_a="${file_a%.txt}.detail.txt"
    local detail_b="${file_b%.txt}.detail.txt"
    if [[ -f "$detail_a" ]] && [[ -f "$detail_b" ]]; then
        local new_failures fixed_failures
        new_failures=$(comm -13 <(grep '^"' "$detail_a" | sort) <(grep '^"' "$detail_b" | sort) || true)
        fixed_failures=$(comm -23 <(grep '^"' "$detail_a" | sort) <(grep '^"' "$detail_b" | sort) || true)

        if [[ -n "$new_failures" ]]; then
            echo -e "${RED}${BOLD}New regressions:${NC}"
            while IFS= read -r line; do
                echo -e "  ${RED}+ $line${NC}"
            done <<< "$new_failures"
            echo ""
        fi
        if [[ -n "$fixed_failures" ]]; then
            echo -e "${GREEN}${BOLD}Fixed:${NC}"
            while IFS= read -r line; do
                echo -e "  ${GREEN}- $line${NC}"
            done <<< "$fixed_failures"
            echo ""
        fi
    fi

    return $has_regression
}

# --- Main ---

MODE=""
TAG=""
COMPARE_A=""
COMPARE_B=""
FILES=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --diff)    MODE="diff"; shift ;;
        --save)    MODE="save"; TAG="$2"; shift 2 ;;
        --compare) MODE="compare"; COMPARE_A="$2"; COMPARE_B="$3"; shift 3 ;;
        --bisect)  MODE="bisect"; shift ;;
        --quick)   FILES=("${QUICK_FILES[@]}"); shift ;;
        --help|-h)
            echo "Usage: $0 [options] [files...]"
            echo ""
            echo "Options:"
            echo "  --diff          Run and diff against last baseline"
            echo "  --save TAG      Save results as named snapshot"
            echo "  --compare A B   Compare two saved snapshots"
            echo "  --bisect        Git bisect mode (uses .test-results/bisect-target.txt)"
            echo "  --quick         Only test failing files (prim fill header under undo)"
            echo "  --help          Show this help"
            echo ""
            echo "Files: ${ALL_FILES[*]}"
            echo ""
            echo "Snapshots stored in: $RESULTS_DIR/"
            exit 0
            ;;
        *)
            FILES+=("$1"); shift ;;
    esac
done

if [[ ${#FILES[@]} -eq 0 ]]; then
    FILES=("${ALL_FILES[@]}")
fi

case "$MODE" in
    compare)
        diff_results "$RESULTS_DIR/$COMPARE_A.txt" "$RESULTS_DIR/$COMPARE_B.txt"
        ;;
    bisect)
        # Git bisect mode: build, run, check against target
        build_rbqn || exit 125  # skip if build fails

        TARGET_FILE="$RESULTS_DIR/bisect-target.txt"
        if [[ ! -f "$TARGET_FILE" ]]; then
            echo -e "${YELLOW}No bisect target found. Creating default (current failures)...${NC}"
            echo "# Bisect target: max allowed failures per file" > "$TARGET_FILE"
            echo "# A commit is 'good' if all files have <= this many failures" >> "$TARGET_FILE"
            for file in "${FILES[@]}"; do
                result=$(run_test_file "$file")
                status="${result%%:*}"
                count="${result#*:}"
                case "$status" in
                    PASS) echo "$file 0" >> "$TARGET_FILE" ;;
                    FAIL) echo "$file $count" >> "$TARGET_FILE" ;;
                    *) echo "$file 999" >> "$TARGET_FILE" ;;
                esac
            done
            echo -e "Created $TARGET_FILE — edit thresholds, then re-run bisect."
            exit 125
        fi

        is_good=0
        while IFS=' ' read -r tfile tmax; do
            [[ "$tfile" =~ ^#.*$ ]] && continue
            [[ -z "$tfile" ]] && continue
            result=$(run_test_file "$tfile")
            status="${result%%:*}"
            count="${result#*:}"
            case "$status" in
                PASS) actual=0 ;;
                FAIL) actual="$count" ;;
                *) actual=999 ;;
            esac
            if [[ $actual -gt $tmax ]]; then
                echo -e "${RED}REGRESSION: $tfile has $actual failures (max: $tmax)${NC}"
                is_good=1
            fi
        done < "$TARGET_FILE"

        exit $is_good
        ;;
    diff)
        build_rbqn || exit 1

        LATEST=$(ls -t "$RESULTS_DIR"/baseline-latest.txt 2>/dev/null | head -1 || true)

        NEW_TAG="run-$(date +%Y%m%d-%H%M%S)"
        echo -e "${CYAN}Running tests...${NC}"
        new_file=$(save_results "$NEW_TAG" "${FILES[@]}")

        if [[ -z "$LATEST" ]]; then
            echo -e "${YELLOW}No baseline found. Saving this run as baseline.${NC}"
            cp "$new_file" "$RESULTS_DIR/baseline-latest.txt"
            cp "${new_file%.txt}.detail.txt" "$RESULTS_DIR/baseline-latest.detail.txt"
            print_results "${FILES[@]}" || true
        else
            diff_results "$LATEST" "$new_file" || true
        fi
        ;;
    save)
        build_rbqn || exit 1
        echo -e "${CYAN}Running tests and saving as '${TAG}'...${NC}"
        save_results "$TAG" "${FILES[@]}"
        print_results "${FILES[@]}" || true

        cp "$RESULTS_DIR/$TAG.txt" "$RESULTS_DIR/baseline-latest.txt"
        cp "$RESULTS_DIR/$TAG.detail.txt" "$RESULTS_DIR/baseline-latest.detail.txt"
        echo -e "${DIM}Saved to $RESULTS_DIR/$TAG.txt${NC}"
        ;;
    *)
        # Default: run and print, save as baseline
        build_rbqn || exit 1
        echo -e "${CYAN}Running RBQN test suite...${NC}"

        save_results "baseline-latest" "${FILES[@]}" > /dev/null
        total_fail=0
        print_results "${FILES[@]}" || total_fail=$?

        commit=$(git -C "$RBQN_ROOT" rev-parse --short HEAD 2>/dev/null || echo "?")
        echo -e "${DIM}Baseline saved. Use --diff to compare after changes. (commit: $commit)${NC}"
        ;;
esac

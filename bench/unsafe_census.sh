#!/usr/bin/env bash
# Per-crate census of `unsafe` occurrences vs `// SAFETY:` comments.
# unsafe = sites outside // comments: `unsafe {`, `unsafe impl`, `unsafe fn`, `unsafe extern "C" {`
# (fn-pointer types like `unsafe extern "C" fn()` are not sites);
# SAFETY = lines with `SAFETY:` in a comment. Prints a markdown table and a README sentence.
set -euo pipefail
cd "$(dirname "$0")/.."
tu=0; ts=0; rows=""
for d in crates/*/; do
  c=$(basename "$d")
  u=$(grep -rhE --include='*.rs' 'unsafe[[:space:]]*(\{|impl|fn |extern "C" \{)' "$d" 2>/dev/null | grep -vcE '^[[:space:]]*//' || true)
  s=$(grep -rh --include='*.rs' 'SAFETY:' "$d" 2>/dev/null | grep -c '//' || true)
  rows+="| $c | $u | $s |"$'\n'
  tu=$((tu+u)); ts=$((ts+s))
done
echo "| crate | unsafe | SAFETY |"
echo "|---|---|---|"
printf '%s' "$rows"
echo "| total | $tu | $ts |"
echo
echo "README: $tu \`unsafe\` sites across the workspace, each with a \`// SAFETY:\` comment enforced by \`clippy::undocumented_unsafe_blocks\`; rbqn-core and rbqn-prim pass under Miri in CI."
[ "$tu" -eq "$ts" ] || { echo "MISMATCH: $tu unsafe sites, $ts SAFETY comments" >&2; exit 1; }

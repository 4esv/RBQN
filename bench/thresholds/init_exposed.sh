#!/bin/bash
# How much device init is still exposed at the first GPU dispatch.
# Usage: bench/thresholds/init_exposed.sh RBQN_BIN [runs]
# Prints, per program, the "[gpu] init wait" lines (time the interpreter
# blocked in get(), and when that was relative to the init thread's start)
# and the device-init time from the summary, under RBQN_GPU=force.
B=${1:?rbqn binary}; R=${2:-10}
for e in 'a←↕1e6 ⋄ +´ 1+a' 'a←↕1e7 ⋄ +´ 1+a' 'a←1e6⥊1‿2 ⋄ +´ 1+a' 'a←1e7⥊1‿2 ⋄ +´ 1+a'; do
  echo "== $e"
  for _ in $(seq "$R"); do
    RBQN_GPU=force RBQN_GPU_DEBUG=1 "$B" -p "$e" 2>&1 >/dev/null | grep -E 'init wait|^gpu:' | tr '\n' ' ' | sed -E 's/dispatches.*init=/init=/; s/ compiles=.*//'
    echo
  done
done

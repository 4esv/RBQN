# Multicore bench (#104): three workloads a BQN user would write.
# Usage: <bqn> bench/wow/multicore.bqn sum|fma|max
# Prints the result, then "t=<seconds>" spent on the operation alone (inputs excluded).
n ← 1e8
Rep ← { •Show 𝕨 ⋄ •Out "t=" ∾ •Repr (•MonoTime@) - 𝕩 }
w ← ⊑•args
{ w≡"sum" ?                                  # +´ over 1e8 i32
  a ← n ⥊ 100000‿2‿3
  t ← •MonoTime@ ⋄ (+´ a) Rep t
; w≡"fma" ?                                  # (a×b)+c over 1e8 f64; ⌈´ forces and checks it
  a ← n ⥊ 0.5‿1.5‿2.25 ⋄ b ← n ⥊ 3.5‿0.25 ⋄ c ← n ⥊ 1.125‿7.5‿2‿0.75
  t ← •MonoTime@ ⋄ (⌈´ (a×b)+c) Rep t
; w≡"max" ?                                  # ⌈´ over 1e8 f64
  a ← n ⥊ 0.5‿1.5‿2.25‿¯7.5
  t ← •MonoTime@ ⋄ (⌈´ a) Rep t
; •Out "usage: sum|fma|max" }

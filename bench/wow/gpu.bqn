# GPU bench (#125): the pipeline +´ (x×x) + 2×x over 1e8 i32 elements.
# Usage: <bqn> bench/wow/gpu.bqn auto|tensor   (tensor needs rbqn)
# auto:   plain arrays; rbqn decides CPU or GPU per operation (RBQN_GPU=off / unset).
# tensor: explicit •gpu: one upload, fused f32 kernel, one download (f32, so the sum may round).
# Prints the result, then "t=<seconds>" for the pipeline alone (x built outside the timer;
# for tensor the upload is inside the timer).
n ← 1e8
Rep ← { •Show 𝕨 ⋄ •Out "t=" ∾ •Repr (•MonoTime@) - 𝕩 }
w ← ⊑•args
# NOTE: •gpu is looked up at run time so CBQN, which lacks it, can still run "auto".
x ← n ⥊ 1‿2‿3‿4
{ w≡"auto" ?
  t ← •MonoTime@ ⋄ (+´ (x×x) + 2×x) Rep t
; w≡"tensor" ?
  tensor ← •BQN "{•gpu.Tensor 𝕩}" ⋄ fetch ← •BQN "{•gpu.Fetch 𝕩}"
  t ← •MonoTime@ ⋄ g ← tensor {𝕎𝕩} x
  r ← fetch {𝕎𝕩} +´ (g×g) + 2×g
  r Rep t
; •Out "usage: auto|tensor" }

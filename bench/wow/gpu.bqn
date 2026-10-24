# GPU bench (#125): the pipeline +´ (x×x) + 2×x over 1e8 i32 elements.
# Usage: <bqn> bench/wow/gpu.bqn auto|tensor|fold|fold1|fold2|scan|map|sort|matmul|rows [size]   (tensor needs rbqn)
# auto, tensor, fold: x is size (default 1e8) elements of 1‿2‿3‿4 repeated; fold is plain +´x, fold1 is +´x+1, fold2 is +´x+y with a second host array, scan is +`x read at its last element, map is x+1 read at its last element (calibration).
# auto:   plain arrays; rbqn decides CPU or GPU per operation (RBQN_GPU=off / unset).
# tensor: explicit •gpu: one upload, fused f32 kernel, one download (f32, so the sum may round).
# sort:   ∧ of N random i32 (•MakeRand 7, values below 2⋆31). N = second argument, default 1e8.
# matmul: w •math.MatMul x, both 𝕊×𝕊 with entries below 10 (exact in f32, so the exact kernel may run); 𝕊 = second argument, default 4096.
#         CBQN has no •math.MatMul, so it runs the equal BQN expression w +˝∘× ⎉1‿∞ x.
# rows:   +´˘ over an S×S table of random i32 below 1000 (S = second argument, default 1e4).
# Prints the result, then "t=<seconds>" for the pipeline alone (x built outside the timer;
# for tensor the upload is inside the timer).
Rep ← { •Show 𝕨 ⋄ •Out "t=" ∾ •Repr (•MonoTime@) - 𝕩 }
w ← ⊑•args
# NOTE: •gpu is looked up at run time so CBQN, which lacks it, can still run "auto".
na ← ≠•args
Arg ← {na>1 ? •BQN 1⊑•args ; 𝕩}
# NOTE: summary of a sorted list that agrees across rbqn runs (CBQN's •rand stream differs, so its value differs).
Summ ← {(⊑𝕩)‿(¯1⊑𝕩)‿((⌊2÷˜≠𝕩)⊑𝕩)}
_ap ← {𝕨 𝔽 𝕩}
Mk ← {𝕩 ⥊ 1‿2‿3‿4}
{ w≡"sort" ?
  m ← Arg 1e8 ⋄ x ← m (•MakeRand 7).Range 2⋆31
  t ← •MonoTime@ ⋄ y ← ∧x ⋄ d ← (•MonoTime@) - t
  •Show Summ y ⋄ •Out "t=" ∾ •Repr d
; w≡"matmul" ?
  m ← Arg 4096 ⋄ r ← •MakeRand 7
  a ← m‿m r.Range 10 ⋄ b ← m‿m r.Range 10
  mmf ← •BQN⎊{𝕊: +˝∘×⎉1‿∞} "•math.MatMul"
  MM ← mmf _ap
  t ← •MonoTime@ ⋄ c ← a MM b ⋄ d ← (•MonoTime@) - t
  •Show (0‿0⊑c)‿(¯1‿¯1⊑c)‿(+´⥊c) ⋄ •Out "t=" ∾ •Repr d
; w≡"rows" ?
  m ← Arg 1e4 ⋄ x ← m‿m (•MakeRand 7).Range 1000
  t ← •MonoTime@ ⋄ r ← +´˘ x ⋄ d ← (•MonoTime@) - t
  •Show (⊑r)‿(¯1⊑r)‿(+´r) ⋄ •Out "t=" ∾ •Repr d
;
w≡"fold" ?
  x ← Mk Arg 1e8
  t ← •MonoTime@ ⋄ r ← +´x ⋄ d ← (•MonoTime@) - t
  •Show r ⋄ •Out "t=" ∾ •Repr d
; w≡"fold1" ?
  x ← Mk Arg 1e8
  t ← •MonoTime@ ⋄ r ← +´x+1 ⋄ d ← (•MonoTime@) - t
  •Show r ⋄ •Out "t=" ∾ •Repr d
; w≡"fold2" ?
  x ← Mk Arg 1e8 ⋄ y ← (≠x) ⥊ 4‿3‿2‿1
  t ← •MonoTime@ ⋄ r ← +´x+y ⋄ d ← (•MonoTime@) - t
  •Show r ⋄ •Out "t=" ∾ •Repr d
; w≡"scan" ?
  x ← Mk Arg 1e8
  t ← •MonoTime@ ⋄ r ← +`x ⋄ v ← ¯1⊑r ⋄ d ← (•MonoTime@) - t
  •Show v ⋄ •Out "t=" ∾ •Repr d
; w≡"map" ?
  x ← Mk Arg 1e8
  t ← •MonoTime@ ⋄ r ← x+1 ⋄ v ← (¯1⊑r) ⋄ d ← (•MonoTime@) - t
  •Show v ⋄ •Out "t=" ∾ •Repr d
; w≡"auto" ?
  x ← Mk Arg 1e8
  t ← •MonoTime@ ⋄ (+´ (x×x) + 2×x) Rep t
; w≡"tensor" ?
  x ← Mk Arg 1e8
  tensor ← •BQN "{•gpu.Tensor 𝕩}" ⋄ fetch ← •BQN "{•gpu.Fetch 𝕩}"
  t ← •MonoTime@ ⋄ g ← tensor {𝕎𝕩} x
  r ← fetch {𝕎𝕩} +´ (g×g) + 2×g
  r Rep t
; •Out "usage: auto|tensor|sort|matmul|rows" }

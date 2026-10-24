# bench/wow/run.sh after the blocked matmul kernel (#166), Intel i7-9750H 12 threads, Radeon Pro 5300M

WARNING: the machine was shared and heavily loaded (load1 16 at start, 23-27 in the minutes before, max foreign CPU 533%), so every cell, CBQN's included, is slower than an idle host; ratios are the usable part.

uptime (start): 15:04  up 2 days, 22:40, 3 users, load averages: 16.49 23.96 26.77
load1 before run: 16.49
| workload | CBQN | rbqn -t 1 | rbqn -t 12 | speedup vs -t 1 | result agrees |
|---|---|---|---|---|---|
| `+´` 1e8 i32 | 0.054 s (0.39 s) | 0.068 s (0.43 s) | 0.051 s (0.43 s) | 1.34x | yes |
| `(a×b)+c` 1e8 f64 | 1.309 s (3.62 s) | 2.034 s (4.67 s) | 0.425 s (3.10 s) | 4.79x | yes |
| `⌈´` 1e8 f64 | 0.116 s (0.79 s) | 0.268 s (1.02 s) | 0.122 s (0.87 s) | 2.19x | yes |

Cells: median in-process seconds for the operation alone (median whole-process wall seconds).

rbqn 0.1.0; bytecode: rbqn-self; threads: 12; gpu: AMD Radeon Pro 5300M

| pipeline `+´ (x×x)+2×x`, x 1e8 i32 | in-process s (wall s) | result | uploads | download lines | summary line |
|---|---|---|---|---|---|
| CBQN | 0.210 (0.31) | 1250000000 | n/a | n/a | n/a |
| rbqn CPU (`RBQN_GPU=off`) | 0.140 (0.26) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=0.0ms compiles=0` |
| rbqn GPU (automatic) | 0.134 (0.25) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=27.8ms compiles=0` |
| rbqn `•gpu` explicit (f32) | 2.395 (2.57) | 1250000000 | 1 | 0 | `gpu: dispatches=1 submits=1 readbacks=1 up=400000000B down=4096B init=28.1ms compiles=1` |

Explicit row: exactly one `[gpu] upload` line; the sum is read back as one scalar (readbacks=1 in the summary), so there is no `[gpu] download` line. The f32 sum is not guaranteed equal to the exact one.

| workload | CBQN | rbqn CPU `-t 1` (`RBQN_GPU=off`) | rbqn CPU (`RBQN_GPU=off`) | rbqn GPU (`RBQN_GPU=force`) | rbqn default | default ran on | model gpu / cpu ms | rbqn modes agree |
|---|---|---|---|---|---|---|---|---|
| sort `∧` 1e6 i32 | 0.016 s | 0.033 s | 0.035 s | 0.046 s | 0.035 s | CPU | - | yes |
| sort `∧` 1e7 i32 | 0.173 s | 0.474 s | 0.510 s | 0.472 s | 0.479 s | CPU | 342.00 / 220.00 | yes |
| sort `∧` 1e8 i32 | 2.462 s | 6.003 s | 5.583 s | 5.310 s | 5.568 s | CPU | 3402.00 / 2200.00 | yes |
| `•math.MatMul` 1000² (entries below 10) | 0.490 s | 0.198 s | 0.054 s | 0.107 s | 0.056 s | CPU | 69.00 / 17.53 | yes |
| `•math.MatMul` 2000² | 5.575 s | 1.869 s | 0.295 s | 0.805 s | 0.331 s | CPU | 538.00 / 140.27 | yes |
| `•math.MatMul` 4096² | 42.774 s | 15.590 s | 2.569 s | 5.834 s | 2.919 s | CPU | 4606.20 / 1204.88 | yes |
| `+´˘` 1e3×1e3 i32 | 0.000 s | 0.003 s | 0.003 s | 1.584 s | 0.003 s | CPU | - | yes |
| `+´˘` 3e3×3e3 i32 | 0.003 s | 0.022 s | 0.019 s | 5.289 s | 0.024 s | CPU | - | yes |
| `+´˘` 1e4×1e4 i32 | 0.028 s | 0.250 s | 0.254 s | 19.261 s | 0.268 s | CPU | - | yes |

Cells: median in-process seconds of the operation alone, 3 runs unless marked. Model column: the cost model's predicted GPU / CPU milliseconds from the default run's `RBQN_GPU_DEBUG` line (`-`: no model decision, the operation is below the array-size floor).

max foreign CPU% during run: 532.9
uptime (end): 15:16  up 2 days, 22:52, 3 users, load averages: 15.57 17.11 20.93

uptime (start): 11:49  up 2 days, 19:26, 3 users, load averages: 3.04 3.04 3.05
load1 before run: 3.04
| workload | CBQN | rbqn -t 1 | rbqn -t 12 | speedup vs -t 1 | result agrees |
|---|---|---|---|---|---|
| `+´` 1e8 i32 | 0.035 s (0.25 s) | 0.040 s (0.30 s) | 0.019 s (0.27 s) | 2.13x | yes |
| `(a×b)+c` 1e8 f64 | 0.692 s (2.03 s) | 1.053 s (2.65 s) | 0.325 s (1.91 s) | 3.24x | yes |
| `⌈´` 1e8 f64 | 0.065 s (0.49 s) | 0.147 s (0.63 s) | 0.065 s (0.54 s) | 2.26x | yes |

Cells: median in-process seconds for the operation alone (median whole-process wall seconds).

rbqn 0.1.0; bytecode: rbqn-self; threads: 12; gpu: AMD Radeon Pro 5300M

| pipeline `+´ (x×x)+2×x`, x 1e8 i32 | in-process s (wall s) | result | uploads | download lines | summary line |
|---|---|---|---|---|---|
| CBQN | 0.137 (0.20) | 1250000000 | n/a | n/a | n/a |
| rbqn CPU (`RBQN_GPU=off`) | 0.071 (0.14) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=0.0ms compiles=0` |
| rbqn GPU (automatic) | 0.073 (0.15) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=18.1ms compiles=0` |
| rbqn `•gpu` explicit (f32) | 1.424 (1.53) | 1250000000 | 1 | 0 | `gpu: dispatches=1 submits=1 readbacks=1 up=400000000B down=4096B init=16.5ms compiles=1` |

Explicit row: exactly one `[gpu] upload` line; the sum is read back as one scalar (readbacks=1 in the summary), so there is no `[gpu] download` line. The f32 sum is not guaranteed equal to the exact one.

| workload | CBQN | rbqn CPU (`RBQN_GPU=off`) | rbqn GPU (`RBQN_GPU=force`) | rbqn default | default ran on | model gpu / cpu ms | rbqn modes agree |
|---|---|---|---|---|---|---|---|
| sort `∧` 1e6 i32 | 0.010 s | 0.020 s | 0.035 s | 0.021 s | CPU | - | yes |
| sort `∧` 1e7 i32 | 0.106 s | 0.241 s | 0.303 s | 0.228 s | CPU | 345.43 / 220.00 | yes |
| sort `∧` 1e8 i32 | 1.071 s | 2.783 s | 3.401 s | 2.553 s | CPU | 3402.00 / 2200.00 | yes |
| `•math.MatMul` 1000² (entries below 10) | 0.197 s | 1.906 s | 0.073 s | 0.075 s | GPU | 69.00 / 1500.00 | yes |
| `•math.MatMul` 2000² | 1.689 s | 36.224 s | 0.580 s | 0.665 s | GPU | 538.00 / 36000.00 | yes |
| `•math.MatMul` 4096² | 23.537 s | 704.351 s (1 run) | 4.431 s | 4.448 s | GPU | 4606.20 / 742170.35 | yes |
| `+´˘` 1e3×1e3 i32 | 0.000 s | 0.001 s | 0.337 s | 0.002 s | CPU | - | yes |
| `+´˘` 3e3×3e3 i32 | 0.001 s | 0.011 s | 1.362 s | 0.013 s | CPU | - | yes |
| `+´˘` 1e4×1e4 i32 | 0.014 s | 0.143 s | 5.119 s | 0.144 s | CPU | - | yes |

Cells: median in-process seconds of the operation alone, 3 runs unless marked. Model column: the cost model's predicted GPU / CPU milliseconds from the default run's `RBQN_GPU_DEBUG` line (`-`: no model decision, the operation is below the array-size floor).

max foreign CPU% during run: 974.4
uptime (end): 12:08  up 2 days, 19:44, 3 users, load averages: 2.94 4.01 4.24

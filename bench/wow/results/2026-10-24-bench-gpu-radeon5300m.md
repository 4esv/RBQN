uptime (start):  6:59  up 2 days, 14:36, 3 users, load averages: 4.24 3.87 4.05
load1 before run: 4.24
| workload | CBQN | rbqn -t 1 | rbqn -t 12 | speedup vs -t 1 | result agrees |
|---|---|---|---|---|---|
| `+´` 1e8 i32 | 0.030 s (0.24 s) | 0.031 s (0.27 s) | 0.018 s (0.25 s) | 1.78x | yes |
| `(a×b)+c` 1e8 f64 | 0.637 s (1.90 s) | 1.015 s (2.50 s) | 0.299 s (1.82 s) | 3.40x | yes |
| `⌈´` 1e8 f64 | 0.054 s (0.47 s) | 0.147 s (0.66 s) | 0.055 s (0.58 s) | 2.69x | yes |

Cells: median in-process seconds for the operation alone (median whole-process wall seconds).

rbqn 0.1.0; bytecode: rbqn-self; threads: 12; gpu: AMD Radeon Pro 5300M

| pipeline `+´ (x×x)+2×x`, x 1e8 i32 | in-process s (wall s) | result | uploads | download lines | summary line |
|---|---|---|---|---|---|
| CBQN | 0.158 (0.23) | 1250000000 | n/a | n/a | n/a |
| rbqn CPU (`RBQN_GPU=off`) | 3.863 (3.95) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=0.0ms compiles=0` |
| rbqn GPU (automatic) | 0.071 (0.14) | 1250000000 | 0 | 0 | `gpu: dispatches=0 submits=0 readbacks=0 up=0B down=0B init=16.8ms compiles=0` |
| rbqn `•gpu` explicit (f32) | 1.317 (1.41) | 1250000000 | 1 | 0 | `gpu: dispatches=1 submits=1 readbacks=1 up=400000000B down=4096B init=16.5ms compiles=1` |

Explicit row: exactly one `[gpu] upload` line; the sum is read back as one scalar (readbacks=1 in the summary), so there is no `[gpu] download` line. The f32 sum is not guaranteed equal to the exact one.

max foreign CPU% during run: 109.4
uptime (end):  7:00  up 2 days, 14:37, 3 users, load averages: 3.68 3.76 4.00

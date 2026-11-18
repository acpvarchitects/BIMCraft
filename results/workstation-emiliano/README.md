# Workstation bench on Emiliano's laptop (MSVC), 2026-11-18

The #499 bench on an ordinary machine: `geo_bench` (BIMCraft's mesh path) and arris's `panels` (variants b and c) at 1, 8,
16 and 24 threads, the same calls as ADE's `harness/suites`. Same CPU as `results/workstation-vincenzo/`, but built with the
**MSVC** toolchain instead of windows-gnu, so the two laptops differ in toolchain (linker, C runtime, allocator) only. Raw
logs: `logs/<call>.out` and `.err`; one line per call: `runs.csv`.

## Machine

| | |
|---|---|
| CPU | Intel Core Ultra 9 275HX, 24 cores, 24 threads (8 P-cores + 16 E-cores, no SMT) |
| RAM | 64 GB (63.5 GB visible) |
| OS | Windows 11 Enterprise 10.0.26200 (build 26200) |
| Rust | rustc 1.99.1 (58cf8f9c3 2026-10-13), LLVM 23.1.1, toolchain `stable-x86_64-pc-windows-msvc` (host `x86_64-pc-windows-msvc`), Visual Studio's linker |
| Power | on AC (battery about 91 %), power plan "Power" |
| Open apps | **Not a quiet machine**, by the owner's choice: Revit 2023 with a model open (1.9 GB), Chrome (59 processes, 10.4 GB), the Claude desktop app (5.3 GB), Edge WebView processes (2.4 GB) and the BIMCraft desktop, idle. CPU load right after the runs: 55 %. Read ratios rather than decimals. |

## Code

| bench | where | commit |
|---|---|---|
| `geo_bench` | `acpvarchitects/BIMCraft`, #81's head (`vp/geo2ef-void-fast-paths`) | `f3901009` |
| `panels` | this branch, `arris-perf/` (arris 0.11.1, rayon 1.12.0) | `bc91122d` |
| `panels` with mimalloc | a local copy of `arris-perf/`, **not committed**: `mimalloc = "0.1"` in `Cargo.toml` (resolved to mimalloc 0.1.52, libmimalloc-sys 0.1.49) and two lines at the top of `src/bin/panels.rs`: `#[global_allocator] static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;` | `bc91122d` + that change |

## How it ran

- **Calls:** `bimcraft_core-<hash>.exe geo_bench --ignored --nocapture` with `GEO_PANELS` 10000 and 1000 and `RAYON_NUM_THREADS` 1, 8, 16, 24; `panels <b|c> <n> <T> mesh` with n 10000 and 1000 and T 1, 8, 16, 24. These are the calls of `harness/suites/{bimcraft,arris}.json`, with 16 and 24 threads in place of 72.
- **One process per call**, one after another, from a PowerShell runner. Wall time is measured outside the process. Peak RSS is the process's peak working set, sampled every 100 ms. Each call had a budget of 900 s (ADE's was 90 s). No call reached it, so every row is complete.
- **Build:** release. Not clean figures: the `bimcraft-core` test build (11 min 40 s) and `arris-perf` (114 s) ran at the same time.
- **mimalloc:** b only, after the main run: 1,000 panels at 1 and 8 threads, then 10,000 at 8 and 24.

## What the numbers say

- **The toolchain explains Vincenzo's slow single thread.** arris b at 1,000 panels on 1 thread builds in 51.7 s here with MSVC, against 122 s with windows-gnu on the same CPU and 44.2 s on ADE. `geo_bench` shows the same at 1 thread: 0.77 s no void and 2.48 s with void, against 1.56 and 4.65 s on gnu.
- **b at 10,000 panels doesn't scale with the default Windows allocator.** Build: 495 s at 1 thread, 435 at 8, 397 at 16, 411 at 24 (gnu: 357 s at 8, 288 at 16, 187 at 24; ADE: 49.4 s at 8, 15.3 at 72). At 1,000 panels it scales normally (51.7 s at 1 thread, 4.8 s at 24).
- **mimalloc fixes that.** b at 10,000 panels builds in **57.5 s at 8 threads** (7.6× faster than the default allocator, close to ADE's 49.4 s) and **31.0 s at 24** (13×). At 1,000 panels: 38.9 s at 1 thread (1.33×) and 5.4 s at 8 (1.47×). Meshing gets faster too (2 mm at 10,000 panels, 24 threads: 10.9 s against 40.1 s). So the collapse is allocator contention in the Windows heap under the boolean path's allocations, not Windows itself and not arris's algorithm. c, which barely allocates, scales on both toolchains.
- **c (void as a profile hole)** at 10,000 panels builds in 20.0 s at 1 thread, 3.0 s at 8 and 1.5 s at 24 (gnu: 17.0, 2.55, 1.33; ADE: 26.1 at 1, 4.21 at 8). Meshing dominates: 137 s at 2 mm on one thread.
- **BIMCraft's mesh path:** 10,000 panels in 0.17 s (no void) and 0.28 s (void) at 24 threads, and 0.77 and 2.48 s at 1 thread, with 344 and 476 triangles a panel and under 30 MB. Its panel is comparable in kind to arris's, not value for value (see the ADE README's Method). The void case is about 5 to 8 times faster than arris's profile-hole variant (c), and more than 100 times faster than its boolean variant (b) on the default allocator.

## geo_bench with mimalloc (A/B, added later the same day)

The apps' global allocator is `cap::Cap<std::alloc::System>`, so on Windows BIMCraft's own mesh path also runs on the system
heap. To see what mimalloc does there:

- **Binaries:** two release builds of `bimcraft-core`'s tests at `f3901009`. `geo-system` is the code as it is, the same
  binary as the main run (`bimcraft_core-881d6430b5e6e2c7`). `geo-mimalloc` adds `mimalloc = "0.1"` (0.1.52) to
  `crates/core`'s `[dev-dependencies]` and, at the top of `src/lib.rs`, `#[cfg(test)] #[global_allocator] static GLOBAL:
  mimalloc::MiMalloc = mimalloc::MiMalloc;` (`bimcraft_core-8b3bfbea48945fdb`). Not committed anywhere.
- **Runs:** `GEO_PANELS=10000`, `RAYON_NUM_THREADS` 1, 8 and 24, three rounds with the two binaries alternating within each
  round, the same open apps as above. Every run is in `geo-mimalloc.csv`, with logs `logs/geo_bench-ab-*`. Round 3 was
  noisier (other apps picked up), so the table gives the median of the three rounds.

| threads | no void, system | no void, mimalloc | with void, system | with void, mimalloc |
|---|---|---|---|---|
| 1 | 0.78 s | 0.57 s | 2.50 s | 2.16 s |
| 8 | 0.27 s | 0.11 s | 0.61 s | 0.38 s |
| 24 | 0.18 s | 0.06 s | 0.36 s | 0.22 s |

- **With mimalloc**, the void case is about 1.6 times faster at 8 and 24 threads and 1.16 times at 1 thread. The no-void
  case is 2.5 times faster at 8 threads and 3 times at 24.
- **The void case scales better with threads:** 5.7 times from 1 to 8 threads with mimalloc, against 4.1 times with the
  system heap.
- **The per-panel figures barely change:** one core per panel, 10 panels changed, and the probe. The difference is in the
  parallel builds, which is where the system heap is contended.

## Results

### geo_bench (BIMCraft mesh path, #81 head f3901009), times as it prints them, extrapolated to 10,000 panels

| call | GEO_PANELS | threads | all cores, no void | all cores, with void | one core per panel, no void / void | 10 changed, no void / void | tris/panel | peak RSS | process wall |
|---|---|---|---|---|---|---|---|---|---|
| geo_bench-10000-t1 | 10000 | 1 | 0.77 s | 2.48 s | 0.1 / 0.3 ms | 0.6 / 2.4 ms | 344 / 476 | 19 MB | 3.69 s |
| geo_bench-10000-t8 | 10000 | 8 | 0.24 s | 0.46 s | 0.1 / 0.2 ms | 0.6 / 2.2 ms | 344 / 476 | 22 MB | 1.06 s |
| geo_bench-10000-t16 | 10000 | 16 | 0.23 s | 0.35 s | 0.1 / 0.2 ms | 0.7 / 2.0 ms | 344 / 476 | 25 MB | 0.95 s |
| geo_bench-10000-t24 | 10000 | 24 | 0.17 s | 0.28 s | 0.1 / 0.2 ms | 0.8 / 1.9 ms | 344 / 476 | 29 MB | 0.79 s |
| geo_bench-1000-t1 | 1000 | 1 | 0.73 s | 2.31 s | 0.1 / 0.2 ms | 0.7 / 2.3 ms | 344 / 476 | 19 MB | 0.63 s |
| geo_bench-1000-t8 | 1000 | 8 | 0.34 s | 1.04 s | 0.1 / 0.3 ms | 0.7 / 4.0 ms | 344 / 476 | 21 MB | 0.49 s |
| geo_bench-1000-t16 | 1000 | 16 | 0.31 s | 0.79 s | 0.1 / 0.2 ms | 1.0 / 2.1 ms | 344 / 476 | 24 MB | 0.54 s |
| geo_bench-1000-t24 | 1000 | 24 | 0.26 s | 0.58 s | 0.1 / 0.2 ms | 0.6 / 2.3 ms | 344 / 476 | 27 MB | 0.50 s |

### arris-perf `panels` (bench/kernels bc91122d, arris 0.11.1), with meshing at 2 mm and 0.5 mm

| call | variant | panels | threads | build wall | mesh 2 mm (tris/panel, wall) | mesh 0.5 mm (tris/panel, wall) | peak RSS | process wall | status |
|---|---|---|---|---|---|---|---|---|---|
| c-10000-t1 | c | 10000 | 1 | 20.02 s | 736, 136.95 s | 2212, 178.85 s | 783 MB | 336.4 s | ok |
| c-10000-t8 | c | 10000 | 8 | 3.04 s | 736, 21.28 s | 2212, 33.17 s | 795 MB | 62.7 s | ok |
| c-10000-t16 | c | 10000 | 16 | 2.38 s | 736, 15.37 s | 2212, 17.85 s | 804 MB | 40.6 s | ok |
| c-10000-t24 | c | 10000 | 24 | 1.46 s | 736, 12.05 s | 2212, 16.33 s | 812 MB | 36.0 s | ok |
| c-1000-t1 | c | 1000 | 1 | 1.71 s | 736, 15.28 s | 2212, 17.10 s | 88 MB | 34.4 s | ok |
| c-1000-t8 | c | 1000 | 8 | 0.30 s | 736, 1.63 s | 2212, 2.17 s | 96 MB | 4.3 s | ok |
| c-1000-t16 | c | 1000 | 16 | 0.18 s | 736, 1.16 s | 2212, 1.21 s | 102 MB | 2.9 s | ok |
| c-1000-t24 | c | 1000 | 24 | 0.16 s | 736, 1.10 s | 2212, 1.04 s | 106 MB | 2.5 s | ok |
| b-10000-t1 | b | 10000 | 1 | 494.56 s | 736, 151.49 s | 2212, 214.40 s | 1956 MB | 862.5 s | ok |
| b-10000-t8 | b | 10000 | 8 | 435.40 s | 736, 36.99 s | 2212, 74.57 s | 2102 MB | 581.0 s | ok |
| b-10000-t16 | b | 10000 | 16 | 397.41 s | 736, 49.52 s | 2212, 101.06 s | 2135 MB | 581.4 s | ok |
| b-10000-t24 | b | 10000 | 24 | 411.46 s | 736, 40.07 s | 2212, 80.50 s | 2165 MB | 566.9 s | ok |
| b-1000-t1 | b | 1000 | 1 | 51.67 s | 736, 15.51 s | 2212, 14.64 s | 208 MB | 82.1 s | ok |
| b-1000-t8 | b | 1000 | 8 | 7.95 s | 736, 2.37 s | 2212, 3.02 s | 245 MB | 13.7 s | ok |
| b-1000-t16 | b | 1000 | 16 | 5.02 s | 736, 1.32 s | 2212, 1.56 s | 269 MB | 8.3 s | ok |
| b-1000-t24 | b | 1000 | 24 | 4.78 s | 736, 1.16 s | 2212, 1.41 s | 292 MB | 7.7 s | ok |
| b-1000-t8-mimalloc | b (mimalloc) | 1000 | 8 | 5.39 s | 736, 1.95 s | 2212, 2.35 s | 415 MB | 10.3 s | ok |
| b-1000-t1-mimalloc | b (mimalloc) | 1000 | 1 | 38.85 s | 736, 20.05 s | 2212, 16.47 s | 246 MB | 75.7 s | ok |
| b-10000-t8-mimalloc | b (mimalloc) | 10000 | 8 | 57.52 s | 736, 20.20 s | 2212, 22.58 s | 2312 MB | 100.9 s | ok |
| b-10000-t24-mimalloc | b (mimalloc) | 10000 | 24 | 30.95 s | 736, 10.86 s | 2212, 15.85 s | 2586 MB | 58.8 s | ok |


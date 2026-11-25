# Workstation bench on Emiliano's laptop (MSVC), 2026-11-18

The #499 bench on an ordinary machine: `geo_bench` (BIMCraft's mesh path) and arris's `panels` (variants b and c) at 1, 8,
16 and 24 threads, the same calls as ADE's `harness/suites`. Same CPU as `results/workstation-vincenzo/`, but built with the
**MSVC** toolchain instead of windows-gnu, so the two laptops differ in toolchain (linker, C runtime, allocator) only. Raw
logs: `logs/<call>.out` and `.err`; one line per call: `runs.csv`.

**Step 2 (2026-11-25), at the end:** arris 0.12.0 and solidcore `main` on the same panel, and the 43 hard cases, all
with mimalloc.

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

## Step 2 (2026-11-25): arris 0.12.0 and solidcore `main` with mimalloc, and the hard cases

Step 2 of the #499 plan on the same laptop and toolchain: every binary built with mimalloc as its global allocator; the
facade panel for arris 0.12.0 and for solidcore `main`; and the 43 hard cases of the ADE run (`../../README.md`, "Hard
cases (ADE run, 2026-11-25)"), with CadCore limited to its 15 cases that have an operation and no panel for CadCore.

### Code

| | |
|---|---|
| bench | this branch, `6122ecfc` plus the commit that adds this section |
| arris | 0.12.0 from crates.io, `arris12-t/`. Panel: `arris12-t/src/bin/panels.rs`, `arris-perf/src/bin/panels.rs` unchanged but for its header |
| solidcore | git `main` `a60ac806`, through `solidcore-ops` and `solidcore-tess` (both 0.1.0), `sc-main-t/`. Panel: `sc-main-t/src/bin/panels.rs`, new: the panel and parameters of `sc-t/src/common.rs` (solidcore 1.0.1, ADE) built through `solidcore-ops`, meshed with `solidcore-tess` |
| CadCore | git `main` `31b2808b` (0.2.6), `cadcore-t/`, hard cases only |
| allocator | mimalloc 0.1.52 (libmimalloc-sys 0.1.49), behind a `mimalloc` Cargo feature in the three crates, off by default so ADE's builds don't change; built with `cargo build --release --features mimalloc` |
| Rust | rustc 1.99.1, `stable-x86_64-pc-windows-msvc`, release with `debug = "line-tables-only"`, rayon 1.12.0 |

### How it ran

- **Runner:** `harness/run_win.py`, `run.py` for Windows: the same suites, records and files, one child process per call;
  peak RSS is the child's peak working set and CPU time comes from `GetProcessTimes`; a call over budget is killed.
- **Hard cases:** `harness/suites/arris-hard.json`, `sc-main-hard.json` and `cadcore-hard-ops.json` (new: CadCore's 15),
  90 s a case as on ADE. Records in `hardcases/<kernel>.jsonl`, table and notes in `hardcases/table.md`, logs in
  `logs/hardcases/`.
- **Panel:** `harness/suites/panels-workstation.json` (new): `panels <b|c> <n> <T> mesh` with n 1,000 and 10,000 and T 1,
  8, 16 and 24, 900 s a call as on 11-18. b = void by a boolean cut, c = void as a hole in the profile; mesh at a 2 mm and
  a 0.5 mm chord. Records in `panels/<kernel>.jsonl`, logs in `logs/panels/`.
- **Machine:** on AC, power plan "Power", 12:01 to 13:27. Not a quiet machine, but quieter than 11-18: **Revit was not
  open** this time; Chrome (55-60 processes, 8-10 GB), the Claude desktop app, Edge WebView, Slack and ChatGPT were.
  Sampled CPU load: 35 % before, 20 % after the hard cases, 10 % after arris's panels, 16 % at the end; free RAM 22-23 GB
  throughout.
- **Scripts:** `run-step2.sh` ran it all (its console output, with the machine samples, is `logs/step2-run.log`);
  `tables-step2.py` makes the tables below and `hardcases/table.md` (`python -I results/workstation-emiliano/tables-step2.py . out.md`).

### What the numbers say

- **Hard cases: the laptop gives the same outcome as ADE on all 101 case runs** (43 arris, 43 solidcore, 15 CadCore).
  arris 26 ok, 16 refused, 1 timeout (5d, NURBS-NURBS round); solidcore 31 ok, 11 refused, 1 wrong (1d, the revolve
  across the axis); CadCore 2 ok, 2 refused, 10 wrong, 1 timeout (4e). Windows, MSVC and mimalloc change nothing in
  robustness. The laptop is faster case by case: median 1.59 times ADE for arris (12 cases of 5 ms or more) and 1.67 times
  for solidcore (27 cases).
- **arris 0.12.0: every call builds and meshes all its panels, no error.** 10,000 panels, b: build 256 s on 1 thread,
  36.1 s on 8, 24.0 s on 16, 21.4 s on 24, the whole call (build and both meshes) 38.5 s on 24. c: build 18.6 s on 1
  thread, 1.84 s on 8, 0.90 s on 24, the whole call 16.7 s on 24. Peak 2.6 GB (b) and 0.6 GB (c).
- **arris 0.12.0 against 0.11.1, both with mimalloc (11-18's rows): b builds 1.45 to 1.78 times faster.** 1,000 panels:
  25.7 s against 38.9 on 1 thread, 3.0 against 5.4 on 8. 10,000 panels: 36.1 against 57.5 on 8 threads, 21.4 against
  31.0 on 24. Same triangles (736 and 2,212 a panel), so the same geometry. 11-18 has c only on the system allocator, so
  for c the version and the allocator are mixed: 10,000 panels on 24 threads build in 0.90 s against 1.46 and mesh at 2 mm
  in 7.2 s against 12.1. 11-18 also had Revit open and a higher load.
- **solidcore `main` works end to end, mesh included**: every panel builds and meshes at both chords, with no error, panic
  or failed mesh. Its mesh is finer: 1,935 triangles a panel at 2 mm and 6,236 at 0.5 mm (arris: 736 and 2,212).
- **But solidcore `main` is slow on this panel.** 1,000 panels on 1 thread: build 128 s (b) and 172 s (c), against 25.7 s
  and 1.2 s for arris 0.12. 10,000 panels on 1 thread go over the 900 s budget in both variants (the 1,000-panel rate
  gives about 1,300 and 1,700 s for the build alone). On 24 threads: build 77 s (b) and 106 s (c), the whole call 134 and
  168 s, peak 5.9 GB. It scales (b: 7 times on 8 threads, 16.6 times on 24, against 1,000 panels on 1 thread times ten), so
  the cost is per panel, not contention.
  - Unlike arris, the profile-hole variant c is slower than the boolean cut b.
  - **Against solidcore 1.0.1 on ADE** (`sc-t`, its core API, no mesh): 1,000 panels on 1 thread took 19.6 s (b) and
    16.4 s (c) there, 128 and 172 s here, on a machine that runs the hard cases 1.67 times faster than ADE. So the same
    panel through `main`'s `solidcore-ops` costs about 6.5 (b) and 10.5 (c) times the wall time of 1.0.1. This run doesn't
    split it by operation (extrude, revolve, loft, cut); `sc-main-t/src/bin/panels.rs` is where to do that.
- **Mesh on 24 threads, 10,000 panels:** solidcore 11.9 s at 2 mm and 43.5 s at 0.5 mm (b); arris 7.6 and 9.0 s (b).
- **mimalloc:** arris b scales with threads here (256 s on 1 thread to 21.4 on 24, 12 times), as 0.11.1 did with mimalloc
  on 11-18. This run has no system-allocator rows; the 11-18 A/B stands.

### Results: hard cases on the laptop

Times are the kernel's own (`ms` in the case's last line), not the process wall. — = not run: CadCore runs only its 15 cases that have an operation (the other 28 are no op, as on ADE).

| case | arris 0.12.0 | solidcore main | CadCore main |
|---|---|---|---|
| 1a loft rect to apex | refused 0.4 ms | ok 10.1 ms | — |
| 1b loft side shrinks to zero | refused 0.2 ms | refused 0.2 ms | — |
| 1c revolve profile touching axis | ok 0.5 ms | ok 48.7 ms | — |
| 1d revolve profile crossing axis | refused 0.1 ms | **wrong** 207.3 ms | — |
| 1e revolve cone apex on axis | ok 0.4 ms | ok 56.3 ms | — |
| 1f cone cut through its apex | ok 14.5 ms | ok 43.0 ms | — |
| 1g cone apex inside a box fuse | ok 18.2 ms | ok 37.1 ms | — |
| 2a cylinder tangent to a face fuse | ok 47.4 ms | ok 174.5 ms | **wrong** 2.1 ms |
| 2b cylinder tangent inside a face cut | refused 51.3 ms | ok 42.3 ms | — |
| 2c equal cylinders crossing at 90deg fuse | ok 359.4 ms | ok 207.4 ms | **wrong** 23.3 ms |
| 2d coplanar faces partly overlapping fuse | ok 1.2 ms | ok 6.7 ms | **wrong** 0.4 ms |
| 2e boxes stacked sharing a face fuse | ok 1.1 ms | ok 5.7 ms | **wrong** 0.3 ms |
| 2f void face on a body face cut | ok 1.1 ms | ok 4.5 ms | — |
| 3a cup wall 1mm | ok 1.1 ms | ok 5.2 ms | — |
| 3b cup wall 0.1mm | ok 0.9 ms | ok 4.4 ms | — |
| 3c cup wall 0.01mm | ok 1.4 ms | ok 6.0 ms | — |
| 3d sliver 0.01mm from a near-miss cut | ok 1.1 ms | ok 5.2 ms | — |
| 3e plane cut at 0.1deg | ok 1.4 ms | ok 15.8 ms | ok 0.3 ms |
| 3f wedge of 0.1deg | ok 1.0 ms | ok 11.7 ms | **wrong** 0.5 ms |
| 3g box minus a thin NURBS panel | ok 579.0 ms | ok 20.6 ms | — |
| 4a cylinder minus a cylinder at 45deg | ok 734.3 ms | ok 102.8 ms | — |
| 4b ruled loft minus a cylinder | ok 459.9 ms | ok 78.5 ms | — |
| 4c two lofts fused | refused 543.9 ms | ok 315.6 ms | — |
| 4d revolved ring cut by an inclined plane | ok 173.0 ms | ok 152.9 ms | — |
| 4e cylinder cut by an inclined plane | ok 60.5 ms | ok 14.8 ms | **timeout** 90 s |
| 5a fillet plane-plane | ok 0.9 ms | ok 3.4 ms | — |
| 5b fillet plane-cylinder | ok 35.5 ms | ok 94.7 ms | — |
| 5c fillet NURBS-plane | refused 25709.7 ms | ok 5425.8 ms | — |
| 5d fillet NURBS-NURBS | **timeout** 90 s | refused 123.3 ms | — |
| 5e shell a cylinder open at the top | ok 40.3 ms | ok 1022.2 ms | — |
| 5f offset a cylindrical face | ok 17.3 ms | ok 1469.7 ms | — |
| 6a box with a NaN corner | refused 0.0 ms | refused 0.0 ms | **wrong** 0.1 ms |
| 6b box of zero thickness | refused 0.0 ms | refused 0.0 ms | **wrong** 0.0 ms |
| 6c box with inverted corners | refused 0.0 ms | refused 0.0 ms | ok 0.0 ms |
| 6d cylinder of radius 0 | refused 0.0 ms | refused 0.0 ms | refused 0.0 ms |
| 6e cylinder of radius NaN | refused 0.0 ms | refused 0.0 ms | **wrong** 0.0 ms |
| 6f cylinder of negative radius | refused 0.0 ms | refused 0.0 ms | refused 0.0 ms |
| 6g unit box at 1e9 from the origin | ok 0.2 ms | ok 134.1 ms | **wrong** 0.1 ms |
| 6h box fused with itself | ok 1.9 ms | ok 4.5 ms | **wrong** 0.3 ms |
| 6i box cut by itself | refused 1.1 ms | ok 5.3 ms | — |
| 6j extrude a NaN profile | refused 0.1 ms | refused 0.1 ms | — |
| 6k extrude by zero | refused 0.1 ms | refused 0.0 ms | — |
| 6l extrude a self-intersecting profile | refused 0.1 ms | refused 1.0 ms | — |
| **total** | 26 ok, 16 refused, 1 timeout | 31 ok, 11 refused, 1 wrong | 2 ok, 2 refused, 10 wrong, 1 timeout |

Notes on every result other than ok: `hardcases/table.md` (the same as ADE's).

### Laptop against ADE

101 case runs have the same outcome on both machines. None differs.


- arris 0.12.0: on the 12 cases both ran ok in 5 ms or more, the laptop is 1.59x faster than ADE (median of ADE time / laptop time); slowest here: 4a cylinder minus a cylinder at 45deg 734 ms (ADE 1075); 3g box minus a thin NURBS panel 579 ms (ADE 795); 4b ruled loft minus a cylinder 460 ms (ADE 994)
- solidcore main: on the 27 cases both ran ok in 5 ms or more, the laptop is 1.67x faster than ADE (median of ADE time / laptop time); slowest here: 5c fillet NURBS-plane 5426 ms (ADE 9265); 5f offset a cylindrical face 1470 ms (ADE 2536); 5e shell a cylinder open at the top 1022 ms (ADE 1783)
- CadCore main: no case that both ran ok takes 5 ms or more, so no ratio.

### Results: panel, arris 0.12.0, mimalloc

| call | variant | panels | threads | build wall | mesh 2 mm (tris/panel, wall) | mesh 0.5 mm (tris/panel, wall) | peak RSS | CPU | process wall | status |
|---|---|---|---|---|---|---|---|---|---|---|
| b-1000-t1 | b | 1000 | 1 | 25.70 s | 736, 11.86 s | 2212, 14.78 s | 245 MB | 51.55 s | 52.5 s | ok |
| b-1000-t8 | b | 1000 | 8 | 3.02 s | 736, 1.48 s | 2212, 1.78 s | 451 MB | 49.3 s | 6.4 s | ok |
| b-1000-t16 | b | 1000 | 16 | 1.92 s | 736, 0.73 s | 2212, 0.95 s | 586 MB | 56.09 s | 3.7 s | ok |
| b-1000-t24 | b | 1000 | 24 | 1.80 s | 736, 0.66 s | 2212, 0.79 s | 723 MB | 64.19 s | 3.4 s | ok |
| c-1000-t1 | c | 1000 | 1 | 1.20 s | 736, 10.78 s | 2212, 13.60 s | 67 MB | 25.2 s | 25.7 s | ok |
| c-1000-t8 | c | 1000 | 8 | 0.21 s | 736, 1.33 s | 2212, 1.77 s | 71 MB | 25.31 s | 3.5 s | ok |
| c-1000-t16 | c | 1000 | 16 | 0.11 s | 736, 0.72 s | 2212, 0.84 s | 75 MB | 25.81 s | 1.8 s | ok |
| c-1000-t24 | c | 1000 | 24 | 0.09 s | 736, 0.63 s | 2212, 0.72 s | 79 MB | 28.86 s | 1.7 s | ok |
| b-10000-t1 | b | 10000 | 1 | 256.16 s | 736, 138.68 s | 2212, 111.75 s | 2138 MB | 498.09 s | 506.8 s | ok |
| b-10000-t8 | b | 10000 | 8 | 36.07 s | 736, 16.50 s | 2212, 19.82 s | 2335 MB | 565.3 s | 72.8 s | ok |
| b-10000-t16 | b | 10000 | 16 | 23.96 s | 736, 9.28 s | 2212, 11.23 s | 2496 MB | 673.55 s | 44.9 s | ok |
| b-10000-t24 | b | 10000 | 24 | 21.40 s | 736, 7.63 s | 2212, 8.97 s | 2577 MB | 732.73 s | 38.5 s | ok |
| c-10000-t1 | c | 10000 | 1 | 18.63 s | 736, 126.47 s | 2212, 153.33 s | 576 MB | 293.69 s | 298.6 s | ok |
| c-10000-t8 | c | 10000 | 8 | 1.84 s | 736, 15.29 s | 2212, 19.39 s | 580 MB | 287.27 s | 36.8 s | ok |
| c-10000-t16 | c | 10000 | 16 | 1.05 s | 736, 8.66 s | 2212, 10.62 s | 584 MB | 312.97 s | 20.6 s | ok |
| c-10000-t24 | c | 10000 | 24 | 0.90 s | 736, 7.19 s | 2212, 8.37 s | 589 MB | 333.55 s | 16.7 s | ok |


### Results: panel, solidcore main, mimalloc

| call | variant | panels | threads | build wall | mesh 2 mm (tris/panel, wall) | mesh 0.5 mm (tris/panel, wall) | peak RSS | CPU | process wall | status |
|---|---|---|---|---|---|---|---|---|---|---|
| b-1000-t1 | b | 1000 | 1 | 128.16 s | 1935, 20.47 s | 6236, 74.02 s | 571 MB | 218.2 s | 224.5 s | ok |
| b-1000-t8 | b | 1000 | 8 | 15.57 s | 1935, 2.61 s | 6236, 9.12 s | 749 MB | 213.52 s | 27.6 s | ok |
| b-1000-t16 | b | 1000 | 16 | 9.48 s | 1935, 1.52 s | 6236, 5.17 s | 947 MB | 246.7 s | 16.5 s | ok |
| b-1000-t24 | b | 1000 | 24 | 7.35 s | 1935, 1.14 s | 6236, 4.21 s | 1039 MB | 257.56 s | 13.0 s | ok |
| c-1000-t1 | c | 1000 | 1 | 171.78 s | 1933, 18.73 s | 6238, 68.06 s | 572 MB | 254.75 s | 258.9 s | ok |
| c-1000-t8 | c | 1000 | 8 | 20.81 s | 1933, 2.59 s | 6238, 8.73 s | 761 MB | 251.28 s | 32.4 s | ok |
| c-1000-t16 | c | 1000 | 16 | 12.17 s | 1933, 1.43 s | 6238, 5.55 s | 962 MB | 298.25 s | 19.5 s | ok |
| c-1000-t24 | c | 1000 | 24 | 10.04 s | 1933, 1.12 s | 6238, 4.47 s | 1108 MB | 303.75 s | 15.9 s | ok |
| b-10000-t1 | b | 10000 | 1 | — | — | — | 3222 MB | 884.89 s | 901.0 s | TIMEOUT>900s |
| b-10000-t8 | b | 10000 | 8 | 183.22 s | 1935, 24.79 s | 6233, 92.53 s | 5487 MB | 2224.84 s | 303.0 s | ok |
| b-10000-t16 | b | 10000 | 16 | 100.92 s | 1935, 15.48 s | 6233, 55.42 s | 5700 MB | 2691.19 s | 173.3 s | ok |
| b-10000-t24 | b | 10000 | 24 | 76.94 s | 1935, 11.91 s | 6233, 43.53 s | 5899 MB | 2819.95 s | 134.1 s | ok |
| c-10000-t1 | c | 10000 | 1 | — | — | — | 2632 MB | 886.28 s | 901.1 s | TIMEOUT>900s |
| c-10000-t8 | c | 10000 | 8 | 227.26 s | 1934, 27.85 s | 6235, 99.68 s | 5508 MB | 2765.97 s | 357.3 s | ok |
| c-10000-t16 | c | 10000 | 16 | 133.09 s | 1934, 15.70 s | 6235, 64.07 s | 5723 MB | 3223.88 s | 214.4 s | ok |
| c-10000-t24 | c | 10000 | 24 | 105.64 s | 1934, 17.30 s | 6235, 43.47 s | 5933 MB | 3282.52 s | 168.2 s | ok |

— = nothing printed: the build of 10,000 panels on 1 thread had not finished when the call was killed at 900 s.

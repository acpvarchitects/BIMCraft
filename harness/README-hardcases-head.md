## Hard cases (ADE run, 2026-11-25)

A second suite, about robustness rather than speed: 43 cases that are known to break B-rep kernels, run on arris 0.12.0, solidcore from its git `main` and CadCore from its git `main`. The case list, the expected answers and the classifier are in `harness/hardcases.py`; each kernel's cases are a `hardcases` binary in `arris12-t/`, `sc-main-t/` and `cadcore-t/`; raw records are in `results/ade-hardcases/<kernel>.jsonl` and logs in `logs/ade-hardcases/`.

**Pins.**

- arris `=0.12.0` from crates.io (the panel and forms results above are 0.11.1, kept in `arris-t/`).
- solidcore: git `https://github.com/tangency-labs/solidcore`, rev `a60ac806bea3312ae4dea1c9460c6988efbbfc66` (main on 2026-11-25), used through `solidcore-ops`, its application API, with `solidcore-tess` for the mesh. The published 1.0.1 results above stay in `sc-t/`.
- CadCore: git `https://github.com/YATSKOVSKYI/cadcore`, rev `31b2808bc76d31d55cf7e407b8a0f3a40de8d1a1` (main on 2026-11-25, workspace version 0.2.6), MIT.

**Groups.**

1. Degenerate edges and points:
   - a ruled loft from a rectangle to an apex, and one where a side shrinks to zero;
   - a revolve whose profile touches the axis, and one that crosses it, which must be refused;
   - a cone (apex on the axis), cut through its apex and fused with a box around its apex.
2. Tangency and coincidence:
   - a cylinder tangent to a box face (fuse), and tangent from inside (cut);
   - two equal cylinders crossing at 90°;
   - coplanar faces partly overlapping, boxes stacked on a shared face;
   - a void whose face lies on the body's face.
3. Thin and near-degenerate:
   - cups with 1, 0.1 and 0.01 mm walls;
   - a 0.01 mm sliver left by a near-miss cut;
   - a plane cut at 0.1° and a 0.1° wedge;
   - a box minus a 2 mm curved (NURBS) panel.
4. Curved booleans:
   - a cylinder minus a cylinder at 45°;
   - a ruled square-to-circle loft minus a cylinder, and two such lofts fused;
   - a revolved ring and a cylinder cut by an inclined plane.
5. Fillets and offsets:
   - a round on a box edge, on the foot of a cylinder boss, on a loft's planar-to-NURBS edge and on its NURBS-to-NURBS edge;
   - a cylinder shelled open at the top, and its side face offset.
6. Hostile input: NaN, zero, negative and inverted sizes, a unit box at 1e9 mm, a box fused with and cut by itself, and NaN, zero-length and self-crossing extrusions.

**How each result is read.** Each case runs in its own process with a 90 s budget (`harness/run.py`). The kernel reports what it returned, the time of the operation itself, the volume, faces and edges, its own validity check, whether it meshes at a 0.5 mm chord and whether it writes STEP. `harness/hardcases.py` then gives one of these results:

- **ok**: a solid that passes the kernel's check, with the right volume. The volume is compared with a closed form where there is one. Otherwise it is compared with the kernel's own input volume: for the loft cases, the cut is the loft's volume minus the cylinder's, and a fused or filleted volume must fall in a bound. The tolerance is 1e-6 relative for planar results. For curved results it is 1e-5 for an exact integrator (arris, solidcore) and 2e-3 for a mesh volume (CadCore, whose only volume is its mesh, made through its own STEP writer and reader).
- **refused**: a clean error.
- **wrong**: a solid where a refusal was due; a volume off by more than the tolerance (an empty or zero-volume solid included); or a result the kernel's own check calls invalid.
- **crash**: a panic or a dead process.
- **timeout**: more than 90 s.
- **no op**: the kernel has no operation for the case. CadCore has boxes, circles swept along a polyline, an N-way union and a plane cut, and nothing is approximated with another operation. Its plane cut is used only for the cases that are plane cuts (3e, 3f, 4e).

For the hostile cases, refusing is always fine. A solid is accepted only where the input still describes one (an inverted box, a negative radius, a self-crossing extrusion counted as two triangles), and only if its volume is right. For "box cut by itself" the right answer is empty or a refusal.

Times are for one call on a shared machine (ADE, 72 logical cores, under load from other users); read them as orders of magnitude.

To run it: `python3 harness/hardcases.py suites <kernel> hardcases`, then `cargo build --release` in the kernel's crate. Then `BENCH_RESULTS=results/ade-hardcases BENCH_LOGS=logs/ade-hardcases python3 harness/run.py <kernel> <target>/release harness/suites/<kernel>-hard.json`. Finally, `python3 harness/hardcases.py readme` rebuilds this section from the records.


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

### Results

| case | arris 0.12.0 | solidcore main | CadCore main |
|---|---|---|---|
| 1a loft rect to apex | refused 0.1 ms | ok 8.9 ms | no op |
| 1b loft side shrinks to zero | refused 0.1 ms | refused 0.4 ms | no op |
| 1c revolve profile touching axis | ok 0.5 ms | ok 61.5 ms | no op |
| 1d revolve profile crossing axis | refused 0.2 ms | **wrong** 290.8 ms | no op |
| 1e revolve cone apex on axis | ok 0.3 ms | ok 43.7 ms | no op |
| 1f cone cut through its apex | ok 23.9 ms | ok 67.3 ms | no op |
| 1g cone apex inside a box fuse | ok 30.2 ms | ok 55.7 ms | no op |
| 2a cylinder tangent to a face fuse | ok 70.0 ms | ok 412.9 ms | **wrong** 6.2 ms |
| 2b cylinder tangent inside a face cut | refused 91.2 ms | ok 84.6 ms | no op |
| 2c equal cylinders crossing at 90deg fuse | ok 542.6 ms | ok 365.5 ms | **wrong** 60.5 ms |
| 2d coplanar faces partly overlapping fuse | ok 1.6 ms | ok 7.7 ms | **wrong** 0.5 ms |
| 2e boxes stacked sharing a face fuse | ok 1.3 ms | ok 7.3 ms | **wrong** 0.6 ms |
| 2f void face on a body face cut | ok 1.3 ms | ok 6.3 ms | no op |
| 3a cup wall 1mm | ok 0.8 ms | ok 8.5 ms | no op |
| 3b cup wall 0.1mm | ok 0.8 ms | ok 6.8 ms | no op |
| 3c cup wall 0.01mm | ok 1.2 ms | ok 8.0 ms | no op |
| 3d sliver 0.01mm from a near-miss cut | ok 0.8 ms | ok 6.0 ms | no op |
| 3e plane cut at 0.1deg | ok 1.0 ms | ok 18.0 ms | ok 0.4 ms |
| 3f wedge of 0.1deg | ok 1.2 ms | ok 17.9 ms | **wrong** 0.4 ms |
| 3g box minus a thin NURBS panel | ok 795.0 ms | ok 43.6 ms | no op |
| 4a cylinder minus a cylinder at 45deg | ok 1075.5 ms | ok 181.9 ms | no op |
| 4b ruled loft minus a cylinder | ok 993.5 ms | ok 184.7 ms | no op |
| 4c two lofts fused | refused 785.0 ms | ok 644.3 ms | no op |
| 4d revolved ring cut by an inclined plane | ok 204.1 ms | ok 236.8 ms | no op |
| 4e cylinder cut by an inclined plane | ok 114.6 ms | ok 28.9 ms | **timeout** 90 s |
| 5a fillet plane-plane | ok 1.2 ms | ok 4.9 ms | no op |
| 5b fillet plane-cylinder | ok 64.0 ms | ok 206.9 ms | no op |
| 5c fillet NURBS-plane | refused 33765.7 ms | ok 9265.3 ms | no op |
| 5d fillet NURBS-NURBS | **timeout** 90 s | refused 234.6 ms | no op |
| 5e shell a cylinder open at the top | ok 61.5 ms | ok 1782.9 ms | no op |
| 5f offset a cylindrical face | ok 29.5 ms | ok 2536.0 ms | no op |
| 6a box with a NaN corner | refused 0.0 ms | refused 0.0 ms | **wrong** 0.0 ms |
| 6b box of zero thickness | refused 0.0 ms | refused 0.0 ms | **wrong** 0.0 ms |
| 6c box with inverted corners | refused 0.0 ms | refused 0.0 ms | ok 0.0 ms |
| 6d cylinder of radius 0 | refused 0.0 ms | refused 0.0 ms | refused 0.0 ms |
| 6e cylinder of radius NaN | refused 0.0 ms | refused 0.0 ms | **wrong** 0.0 ms |
| 6f cylinder of negative radius | refused 0.0 ms | refused 0.1 ms | refused 0.0 ms |
| 6g unit box at 1e9 from the origin | ok 0.2 ms | ok 224.5 ms | **wrong** 0.0 ms |
| 6h box fused with itself | ok 1.8 ms | ok 9.5 ms | **wrong** 0.2 ms |
| 6i box cut by itself | refused 1.2 ms | ok 9.4 ms | no op |
| 6j extrude a NaN profile | refused 0.1 ms | refused 0.4 ms | no op |
| 6k extrude by zero | refused 0.0 ms | refused 0.0 ms | no op |
| 6l extrude a self-intersecting profile | refused 0.1 ms | refused 1.5 ms | no op |
| **total** | 26 ok, 16 refused, 1 timeout | 31 ok, 11 refused, 1 wrong | 2 ok, 2 refused, 28 no op, 10 wrong, 1 timeout |

Notes (every result other than ok and no op):

- 1a loft rect to apex, arris: refused: the loft sections are not valid: section 1: loop 0, segment 0 is shorter than the tolerance
- 1b loft side shrinks to zero, arris: refused: the loft sections are not valid: section 1: loop 0, segment 3 is shorter than the tolerance
- 1b loft side shrinks to zero, sc-main: refused: invalid argument
- 1d revolve profile crossing axis, arris: refused: degenerate result: the profile crosses the revolve axis
- 1d revolve profile crossing axis, sc-main: wrong: a solid where a refusal was due (volume 125663706.14359178)
- 2a cylinder tangent to a face fuse, cadcore: wrong: volume off 2.1e-05; the kernel's own check says invalid
- 2b cylinder tangent inside a face cut, arris: refused: degenerate result: two faces of one lump of the result meet at a tangent contact (+f5, +f7)
- 2c equal cylinders crossing at 90deg fuse, cadcore: wrong: volume 4.18837e+07, expected 5.74985e+07 (off 2.7e-01, tol 2e-03)
- 2d coplanar faces partly overlapping fuse, cadcore: wrong: volume 8.25e+08, expected 7.875e+08 (off 4.8e-02, tol 1e-06)
- 2e boxes stacked sharing a face fuse, cadcore: wrong: volume 7.5e+08, expected 6e+08 (off 2.5e-01, tol 1e-06)
- 3f wedge of 0.1deg, cadcore: wrong: volume off 1.4e-08; the kernel's own check says invalid
- 4c two lofts fused, arris: refused: kernel bug: a geometry query failed: the section of the NURBS surface and the NURBS surface is degenerate: turning point
- 4e cylinder cut by an inclined plane, cadcore: timeout: TIMEOUT>90s
- 5c fillet NURBS-plane, arris: refused: degenerate result: the corner at the edge's end is one the blend's closed forms do not cover (+e4, +e7, +v4)
- 5d fillet NURBS-NURBS, arris: timeout: TIMEOUT>90s
- 5d fillet NURBS-NURBS, sc-main: refused: the blend failed (status 1001): failed: a step of the fillet executive failed (the error info says which): Fillet Surfac
- 6a box with a NaN corner, arris: refused: degenerate result: min is not finite
- 6a box with a NaN corner, sc-main: refused: invalid argument
- 6a box with a NaN corner, cadcore: wrong: a solid where a refusal was due (volume 150000000.0)
- 6b box of zero thickness, arris: refused: degenerate result: z extent must be positive, not 0
- 6b box of zero thickness, sc-main: refused: invalid argument
- 6b box of zero thickness, cadcore: wrong: a solid where a refusal was due (volume 0.0)
- 6c box with inverted corners, arris: refused: degenerate result: x extent must be positive, not -1000
- 6c box with inverted corners, sc-main: refused: invalid argument
- 6d cylinder of radius 0, arris: refused: degenerate result: radius must be positive, not 0
- 6d cylinder of radius 0, sc-main: refused: invalid argument
- 6d cylinder of radius 0, cadcore: refused: InvalidRadius(0.0)
- 6e cylinder of radius NaN, arris: refused: degenerate result: radius is not finite
- 6e cylinder of radius NaN, sc-main: refused: invalid argument
- 6e cylinder of radius NaN, cadcore: wrong: a solid where a refusal was due (volume 0.0)
- 6f cylinder of negative radius, arris: refused: degenerate result: radius must be positive, not -100
- 6f cylinder of negative radius, sc-main: refused: invalid argument
- 6f cylinder of negative radius, cadcore: refused: InvalidRadius(-100.0)
- 6g unit box at 1e9 from the origin, cadcore: wrong: volume 0.0
- 6h box fused with itself, cadcore: wrong: volume 3e+08, expected 4.5e+08 (off 3.3e-01, tol 1e-06)
- 6i box cut by itself, arris: refused: degenerate result: the result has no thickness (+b0, +b1)
- 6j extrude a NaN profile, arris: refused: the profile is not valid: loop 0, segment 0 is shorter than the tolerance
- 6j extrude a NaN profile, sc-main: refused: invalid argument
- 6k extrude by zero, arris: refused: degenerate result: length must be positive, not 0
- 6k extrude by zero, sc-main: refused: invalid argument
- 6l extrude a self-intersecting profile, arris: refused: the profile is not valid: loop 0 encloses no area
- 6l extrude a self-intersecting profile, sc-main: refused: solidcore failed (status 1001)

### Reading (author's, from the table only)

- **arris 0.12.0** gave no wrong solid. Its 26 ok include every planar, thin-wall, sliver and 0.1° case, the 45° cylinder boolean, the cuts through a cone's apex and the inclined-plane cuts. It refuses clearly at the edges of what it covers:
  - lofts to an apex or with a side of zero length (rejected as input);
  - the tangent-from-inside cut (it says a tangent contact is left);
  - the fusion of two lofts (reported as an internal geometry failure);
  - the round on a loft's NURBS-to-planar edge, after 34 s.

  The round on the NURBS-to-NURBS edge still runs past 90 s. Building the loft and measuring it took 1.9 s of that; the rest is the fillet. Every hostile input is refused.
- **solidcore `main`** builds the most: 31 ok. That includes the apex loft (it has an operation for that), the tangent-from-inside cut, the two-loft fusion and the NURBS-to-planar round (9 s). Its free-form booleans take 40–650 ms; arris needs about a second for those. It gives one wrong result: a revolve whose profile crosses the axis returns a "valid" solid of the volume of the ring from r 100 to 300, where the right answer is a refusal. It refuses the loft with a zero-length side and the NURBS-to-NURBS round, and refuses every hostile input; most of those refusals say only "invalid argument". The mesh (`solidcore-tess`) worked on every solid it built. STEP export was not wired in this harness.
- **CadCore** has no loft, revolve, solid difference, fillet on an edge, shell, offset or profile extrusion, so 28 of 43 cases have no operation. Of the 15 it can attempt, it gets 2 right (the 0.1° plane cut and the inverted box) and 2 right refusals (zero and negative radius). It gives 10 wrong results:
  - four unions of boxes or cylinders return the wrong volume, and its own `validate_shell` flags them invalid;
  - the 0.1° wedge has the right volume but fails its own check;
  - a NaN corner, a zero thickness and a NaN radius all build a solid;
  - a unit box at 1e9 mm meshes to zero volume;
  - a box fused with itself loses a third of its volume.

  One plane cut of a swept cylinder runs past 90 s. As its README says, it is built for swept-filament lattices, not for general solids; for this suite it is closest to a planar-cut helper.
- No kernel crashed or panicked in any of the 129 runs.
- Not covered here: STEP for solidcore, time under several threads, and a run on the workstations (planned in step 2).

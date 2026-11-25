# Hard cases on Emiliano's laptop (MSVC, mimalloc), 2026-11-25

Step 2 in ../README.md. Records: `<kernel>.jsonl` here; logs: `../logs/hardcases/`.

### Hard cases on the laptop

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


### Laptop against ADE

101 case runs have the same outcome on both machines. None differs.


- arris 0.12.0: on the 12 cases both ran ok in 5 ms or more, the laptop is 1.59x faster than ADE (median of ADE time / laptop time); slowest here: 4a cylinder minus a cylinder at 45deg 734 ms (ADE 1075); 3g box minus a thin NURBS panel 579 ms (ADE 795); 4b ruled loft minus a cylinder 460 ms (ADE 994)
- solidcore main: on the 27 cases both ran ok in 5 ms or more, the laptop is 1.67x faster than ADE (median of ADE time / laptop time); slowest here: 5c fillet NURBS-plane 5426 ms (ADE 9265); 5f offset a cylindrical face 1470 ms (ADE 2536); 5e shell a cylinder open at the top 1022 ms (ADE 1783)
- CadCore main: no case that both ran ok takes 5 ms or more, so no ratio.

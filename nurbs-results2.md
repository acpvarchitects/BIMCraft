| 3a loft open, ruled | ok: 6 faces (4 NURBS), valid=true | 5.19 ms |
| 3b loft open, smooth 3 sections | ok: 6 faces (4 NURBS), valid=true | 5.49 ms |
| 3c loft closed (4 circles round a ring) | ok: 1 faces (1 NURBS), valid=true | 1.09 ms |
| 3e sweep1 along a NURBS rail | ok: 6 faces (4 NURBS), valid=true | 3355.87 ms |
| 4a loft capped into a solid | ok: loft is born a capped solid: 6 faces (4 NURBS), valid=true | 5.28 ms |
| 4b shell a smooth loft 10 mm, top open | ERR: degenerate result: the face's surface has no exact offset of its own kind (+f2) | 4.26 ms |
| 4c offset a NURBS face of a solid (offset_faces) | ERR: degenerate result: the face's surface has no exact offset of its own kind (+f2) | 3.77 ms |
| 5a smooth loft - box (box through a NURBS side) | ok: 11 faces (4 NURBS), valid=true in 2190.3 ms | 2194.05 ms |
| 5b smooth loft - cylinder through it (horizontal) | ok: 7 faces (4 NURBS), valid=true in 1900.2 ms | 1905.91 ms |
| 5c two smooth lofts fused (overlapping) | ok: 12 faces (8 NURBS), valid=true in 5482.1 ms, edges {"Nurbs": 30} | 5489.12 ms |
| 5d box - thin NURBS panel | ERR: kernel bug: the result's shells are not lumps: +b2: the nesting is undecided: B1? b2: f16 (NURBS) against f21 (NURBS), of two shells, is not decided; | 1173.03 ms |
| 5e NURBS face flush with a box face (loft cap on box top) | ok: 11 faces (4 NURBS), valid=true | 13.55 ms |
| 6a fillet plane-plane (box edge, r 20) | ok: 7 faces (0 NURBS), valid=true | 2.08 ms |
| 6b fillet plane-cylinder (hole rim, r 5) | ok: 8 faces (0 NURBS), valid=true | 239.03 ms |
| 6c | TIMEOUT >90 s (or crash) | |
| 6d fillet NURBS-plane edge (loft top rim, r 10) | ERR: no closed form for +e5 (NURBS surface) against +f2 (NURBS surface) | 5869.93 ms |
| 6e chamfer NURBS-NURBS edge (5 mm) | ok: 7 faces (5 NURBS), valid=true | 3264.69 ms |
| 7a split smooth loft by a plane | ok: + 6 faces (4 NURBS), valid=true / - 6 faces (4 NURBS), valid=true | 3479.94 ms |
| 8 tessellate smooth loft at 0.5 mm | ok: 14444 tris in 85.2 ms | 89.87 ms |
| 8 tessellate smooth loft at 2 mm | ok: 3692 tris in 47.5 ms | 51.27 ms |
| 8 tessellate smooth loft at 5 mm | ok: 1676 tris in 50.9 ms | 55.11 ms |
| 9 STEP of a smooth loft | ok: 21704 bytes; B_SPLINE_SURFACE_WITH_KNOTS=4 RATIONAL_B_SPLINE_SURFACE=4 B_SPLINE_CURVE_WITH_KNOTS=20 | 291.06 ms |
DONE

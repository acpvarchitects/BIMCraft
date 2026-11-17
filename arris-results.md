| test | result | faces | edges | op time | tess 0.5mm |
|---|---|---|---|---|---|
| 1a extrude rect | ok (valid) | 6 faces {"Plane": 6} | 12 edges {"Line": 12} | 0.61 ms | 12 tris, 0.30 ms, closed=true |
| 1b extrude L | ok (valid) | 8 faces {"Plane": 8} | 18 edges {"Line": 18} | 0.24 ms | 20 tris, 0.14 ms, closed=true |
| 2 revolve ring | ok (valid) | 4 faces {"Cylinder": 2, "Plane": 2} | 6 edges {"Circle": 4, "Line": 2} | 0.23 ms | 348 tris, 1.50 ms, closed=true |
| 3a sweep L path (corner) | ERR: the path is not valid: the path has a corner of 1.5707963267948966 rad at arc length 1000, entering segment 1 | | | 0.10 ms | |
| 3a2 L as fuse of 2 boxes | ok (valid) | 14 faces {"Plane": 14} | 28 edges {"Line": 28} | 1.58 ms | 28 tris, 0.18 ms, closed=true |
| 3b sweep arc R1000 | ok (valid) | 6 faces {"Cylinder": 2, "Plane": 4} | 12 edges {"Circle": 4, "Line": 8} | 0.29 ms | 208 tris, 0.97 ms, closed=true |
| 3c sweep straight line | ok (valid) | 6 faces {"Plane": 6} | 12 edges {"Line": 12} | 0.22 ms | 12 tris, 0.14 ms, closed=true |
| 4 loft square->circle | ok (valid) | 6 faces {"Nurbs": 4, "Plane": 2} | 12 edges {"Nurbs": 12} | 3.68 ms | 5612 tris, 29.18 ms, closed=true |
| 4b loft smooth 3 sect | ok (valid) | 6 faces {"Nurbs": 4, "Plane": 2} | 12 edges {"Nurbs": 12} | 5.83 ms | 9938 tris, 39.49 ms, closed=true |
| 5b blend along arc (smooth loft 3 sect) | ok (valid) | 6 faces {"Nurbs": 4, "Plane": 2} | 12 edges {"Nurbs": 12} | 3.84 ms | 7448 tris, 33.66 ms, closed=true |
| 6a box - cylinder | ok (valid) | 7 faces {"Cylinder": 1, "Plane": 6} | 15 edges {"Circle": 2, "Line": 13} | 87.43 ms | 144 tris, 1.21 ms, closed=true |
| 6b box - coplanar box | ok (valid) | 10 faces {"Plane": 10} | 24 edges {"Line": 24} | 1.01 ms | 28 tris, 0.29 ms, closed=true |
| 6c fuse boxes sharing face | ok (valid) | 10 faces {"Plane": 10} | 20 edges {"Line": 20} | 0.87 ms | 20 tris, 0.18 ms, closed=true |
| 6d fuse partly shared face | ok (valid) | 11 faces {"Plane": 11} | 25 edges {"Line": 25} | 0.81 ms | 28 tris, 0.17 ms, closed=true |
| 6e ring(revolve) - box | ok (valid) | 7 faces {"Cylinder": 2, "Plane": 5} | 18 edges {"Circle": 8, "Line": 10} | 135.15 ms | 358 tris, 1.54 ms, closed=true |
| hostile: NaN extrude | ERR: the profile is not valid: loop 0, segment 0 is shorter than the tolerance | | | 0.01 ms | |
| hostile: zero-length extrude | ERR: degenerate result: length must be positive, not 0 | | | 0.00 ms | |
| hostile: self-intersecting | ERR: the profile is not valid: loop 0 encloses no area | | | 0.03 ms | |
STEP: 15150 bytes, CIRCLE=4 B_SPLINE=0 CYLINDRICAL_SURFACE=1 FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));

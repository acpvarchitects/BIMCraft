| test | result | time |
|---|---|---|
| 1a curve by 6 control points, degree 3 | ok: eval(1.5)=(500.0,150.0) | 0.05 ms |
| 1b split (segment + insert_knot) | ok: piece 5 cps, refined 9 cps | 0.04 ms |
| 1c closed periodic curve (fit to a closed function) | ok: period Some(6.283185307179586), 63 cps | 0.73 ms |
| 1d curve through points (interpolation) | ERR: missing: no API; fit_curve fits a closure f(t), not a point list | 0.00 ms |
| 1e offset curve in plane | ERR: missing: no curve offset in arris-geom | 0.00 ms |
| 2a surface 6x6 control points degree 3 | ok: 0.0018 ms each (x1000), 6 | 1.76 ms |
| 2b edit a control point and rebuild | ok: z at (2.5,2.5): 29.0 -> 84.0 | 0.02 ms |
| 2c surface through a grid (interpolate_skin over row curves) | ok: [6, 6] cps; rows are control polygons, not fitted points | 0.09 ms |
| 2d offset NURBS surface 20 mm (offset_fitted) | ok: [26, 26] cps | 367.59 ms |
| 2e sheet body from a bare NURBS surface | ERR: missing as an op: only the low-level Builder (you stage edges, pcurves, face) | 0.00 ms |
| 3a loft open, ruled | ok: 6 faces (4 NURBS), valid=true | 2282.52 ms |
| 3b loft open, smooth 3 sections | ok: 6 faces (4 NURBS), valid=true | 18303.86 ms |
| 3c loft closed (4 circles round a ring) | ok: 1 faces (1 NURBS), valid=true | 52.86 ms |
| 3d loft with rails / sweep2 / network patch / blend surface | ERR: missing | 0.00 ms |

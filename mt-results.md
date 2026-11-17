| test | result | faces | edges | op time | tess 0.5mm |
|---|---|---|---|---|---|
| 1a extrude rect | ok | 6 faces {"Plane": 6} | 12 edges {"Line": 12} | 0.19 ms | 12 tris + 0 quads + 0 other, 2.29 ms |
| 1b extrude L | ok | 8 faces {"Plane": 8} | 18 edges {"Line": 18} | 0.03 ms | 20 tris + 0 quads + 0 other, 0.44 ms |
| 2 revolve ring (division 2) | ok | 8 faces {"RevolutionSurface": 8} | 16 edges {"Line": 8, "NurbsCurve": 8} | 0.08 ms | 8232 tris + 0 quads + 0 other, 11.24 ms |
| 2 revolve ring (division 4) | ok | 16 faces {"RevolutionSurface": 16} | 32 edges {"Line": 16, "NurbsCurve": 16} | 0.09 ms | 15820 tris + 0 quads + 0 other, 15.66 ms |

division must be 2 or greater for closed revolve
| 2x revolve division 0 (hostile) | PANIC | | | 0.12 ms | |
| 3a L path: no sweep-along-path builder; fuse 2 boxes | ERR: InvalidOutputShell { operation: "or", source: NotClosedShell } | | | 2.44 ms | |
| 3b arc sweep as partial revolve 90deg | ok | 6 faces {"Plane": 2, "RevolutionSurface": 4} | 12 edges {"Line": 8, "NurbsCurve": 4} | 0.04 ms | 4104 tris + 0 quads + 0 other, 8.03 ms |
| 4 loft square->circle (skin + caps) | ok | 6 faces {"NurbsSurface": 4, "Plane": 2} | 12 edges {"Line": 8, "NurbsCurve": 4} | 0.10 ms | 7820 tris + 0 quads + 0 other, 11.37 ms |
| 6a box - cylinder | ok | 10 faces {"NurbsSurface": 4, "Plane": 6} | 24 edges {"IntersectionCurve": 8, "Line": 16} | 657.37 ms | 18272 tris + 0 quads + 0 other, 39.61 ms |
| 6b box - coplanar box | ERR: InvalidOutputShell { operation: "and", source: NotClosedShell } | | | 2.18 ms | |
| 6c fuse boxes sharing face | ERR: InvalidOutputShell { operation: "or", source: NotClosedShell } | | | 1.81 ms | |
| 6d fuse partly shared face | ERR: InvalidOutputShell { operation: "or", source: NotClosedShell } | | | 1.00 ms | |
| 6e ring(revolve) - box | ERR: InvalidOutputShell { operation: "and", source: NotClosedShell } | | | 97.25 ms | |
| hostile: NaN face | ERR: WireNotInOnePlane | | | 0.01 ms | |
| hostile: self-intersecting face | ERR: WireNotInOnePlane | | | 0.01 ms | |

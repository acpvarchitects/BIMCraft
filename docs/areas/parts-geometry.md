# Parts: dividing a solid into pieces (the geometry)

`crates/core/src/divide.rs` and `divide/` are the geometric half of element subdivision, after Revit's Parts (Divide
Parts) and Archicad's Morphs, for restoration work (HBIM): a zone of rising damp or of detached plaster outlined on a
wall's face, a stratigraphic unit of masonry, a wall out of plumb or tapering, a lacuna. It is pure geometry: no model
types, no commands, no UI. The data model (the part element, its layers, IFC) is the other half; it keeps a recipe and
asks this module for the pieces.

## What goes in and what comes out

- **The solid** is a closed surface of triangles in model coordinates (mm; x east, y north, z up), `&[[P3; 3]]`, each
  triangle's corners counter-clockwise seen from outside, so its normal points out: the convention of `csg::subtract`
  and of the data model's `Model::part_solid`. The pieces come back the same way (`Piece::mesh`).
- **The recipe** (`Recipe`) is all a project keeps: `cuts`, in order (`Division`: a `Cut`, its `gap`, and optionally the
  piece it divides, `of`), then `offsets` (`Offset`: a face of a finished piece moved inwards). It serializes as it is
  (`deny_unknown_fields`, snake_case tags), so a part can hold it. Keep it in the host's own coordinates and bring the
  solid into them (or carry the frames to the model with the host): a `Frame` holds its own x axis, so it turns with its
  host, where a `workplane::Frame` works its x out from the world's up (`Frame::from` converts one).
- **`divide(solid, recipe)`** gives an `Outcome`: the `pieces` (each a `Piece`: `key`, `mesh`, `volume`, `closed`),
  `removed` (mm³ taken by gaps, offsets and noise) and no `conflicts`. An error is a `DivideError`: `Input` (the solid
  or the recipe can't be used: what to fix), `Conflict { at, conflict }` (a cut or offset of a good recipe no longer
  meets this solid) or `TooMuch` (the work budget).
- **`regenerate(solid, recipe)`** is the same for a host that changed: a cut or offset in conflict is left out and
  listed in `Outcome::conflicts` (`(Stage, Conflict)`), and the rest is done. This is what a part calls after its host
  moved, and what it turns into a conflict flag.
- **`divide_prism(prism, recipe)`** is the same for a part that is a prism (a `Prism`: a face's outline, its openings,
  a thickness), as a wall's layer is. See *The prism's way*.
- **Measuring**, for the bill of quantities: `volume(mesh)`, `area(mesh)`, `area_facing(mesh, normal, tolerance)` and
  `is_closed(mesh)`. `cut_plane(solid, point, normal)` is one plane cut by itself.

## Naming the pieces

A piece's name is its `PieceKey`: one `Step { cut, side, lump }` for each cut that divided it, in order; no steps for
the solid whole. `cut` is the cut's place in the recipe (from 0), `side` the side of it the piece is on, `lump` which
lump of that side.

- **Sides**: `front` (the way the normal points) and `back` of a plane; `left` and `right` of an open line, going from
  its first point to its last and seen from where the frame's normal points; `inside` and `outside` of a closed line or
  a profile. `Cut::sides` gives a cut's two, in the order their pieces come.
- **Lumps**: where a cut leaves several lumps apart on one side (a line across a door leaves two feet), they are
  counted from 0 in order of their centres of volume in the cut's frame: by x, then y, then along the normal, each
  compared to the millimetre first, so two lumps one above the other don't swap on a rounding. A plane's frame is a
  work plane's (x level across it; east on a level plane). Lumps that touch along an edge or at a corner are one (their
  surfaces share corners), and so are the prisms of a solid that comes with faces inside it. A hollow inside a lump
  belongs to it.
- **Order**: the pieces come in the order of their keys: a divided piece's parts take its place, first side first.
- **A cut divides every piece it meets**, or only the piece named in `of` and what was made of it since (as Revit
  divides the part selected). A piece it passes by keeps its key. A cut that divides none of its pieces is a conflict.
- **What survives a change of host**: a key holds each cut's own place in the recipe, so leaving a conflicting cut out
  doesn't rename the others' pieces, and moving, turning or stretching the host keeps every key as long as the same
  cuts still divide the same pieces. A lump index changes only when lumps appear, vanish or pass each other.
- `Display` writes a key as `0.left.0/2.inside.1`, and `whole`.

## The cuts, and how each is made

| Cut | What it is | How |
|---|---|---|
| `Cut::Plane { point, normal }` | a plane at any angle: a wall out of plumb, a taper | every triangle clipped, each side capped (`clip.rs`, `cap.rs`) |
| `Cut::Sketch { frame, points, closed: true, depth }` | an outline drawn on a face: through the part, or `depth` mm in from the face (plaster 15 mm deep), what is behind staying with the rest | the solid and the outline's prism shared, and the solid less it (`csg::intersect`, `csg::subtract`), then mended |
| `Cut::Sketch { .., closed: false }` | a line drawn across a face, through the part | a plane when the line is straight and spans the piece; otherwise a thin wall taken out along it |
| `Cut::Profile { frame, points, from, to }` | a closed profile in any frame, extruded along its normal over a range or through everything: a niche, a lacuna | as a closed outline |

**A plane** is the robust case and is done directly. A corner within `WELD` (1e-5 mm) of the plane lies on it; where an
edge crosses, the new corner is made once for the edge, so both triangles on it share it; the cap of each side is made
from the edges its triangles leave open, chained into rings (the leftmost turn at a corner where rings touch), holes
bridged to their outlines, and cut ear by ear using the rings' own corners and no others. So the pieces of a closed
solid are closed by construction, whatever the numbers.

**A closed line or a profile** is a prism, and a prism against a general solid is what `csg`'s binary space partitions
do. A partition leaves T-junctions (a corner of one triangle on another's edge) and drops triangles too small to have
a plane, so each result is welded and mended (`Soup::mend`: an open edge is cut at every corner lying on it, and a
pinhole, a ring of open edges round less than 0.0001 mm², is closed with its own corners) and then tidied.

**An open line cuts where it is drawn and nowhere else.** A line across one jamb of a door, ending in the opening,
takes the foot of that jamb and leaves the other jamb whole; so the cut can be no plane and no half space, whose far
boundary would cut the rest of the part. Instead the solid loses a wall `KERF` (0.002 mm) thick along the line
(`REACH`, 1 mm, past both ends, so a line drawn up to an outline's edge crosses it), what is left falls into lumps, and
each lump is on the left or the right by the face the wall left on it. With no gap the wall is taken twice, once wholly
on the right of the line and once wholly on its left, and each side is kept from the time the wall lay across the line
from it: its cut face is then on the line itself, the two sides meet exactly and nothing is lost. A lump with a wall
face on both sides was not cut through: `Conflict::DoesNotCross`. A straight line that spans the whole piece is the
plane through it and is cut as one, exactly.

A line or a profile is tidied before use (`flat::clean`): points closer than 0.01 mm are one, points in line with their
neighbours are left out, a ring is turned counter-clockwise. It is refused when it crosses or touches itself, folds
back on itself, or (closed) has no area. An open line to a depth is refused: a scratch parts nothing.

**Tidying** (`tidy.rs`): after each cut, the triangles of a piece that lie in one plane and touch along edges are one
face again; its outline is traced, corners that only stand on a straight edge between the same two faces are left out
(on both), and the face is filled anew. A box cut from a wall is 12 triangles again, the rest round a through patch 32.
The tidied surface is kept only if it is still closed and holds the same volume; otherwise the piece stays as it was.

## Gaps and offsets

- **A gap** (`Division::gap`, mm) is a joint: what lies within half of it of the cut is taken away, so the pieces don't
  touch. For a plane, two clips half a gap apart; for a closed line the outline is drawn in for the inside and out for
  the outside (mitred corners), and to a depth the joint runs under the patch too; for an open line the wall is the
  gap. A gap too wide for its outline (nothing left inside) or for the line's corners is the recipe's fault: `Input`.
- **An offset** (`Offset { piece, face, distance }`) moves one face of a finished piece into it: erosion, missing
  material, a joint on one side only. The face is named so as to be found again:
  - `FaceRef::Cut(k)`: the face cut `k` made on the piece (the piece's key must hold a step of cut `k`). It is that cut
    made again further in, on the piece's own side: a plane clipped again, an outline drawn in or out, a line's strip.
  - `FaceRef::Plane { point, normal }`: the piece's flat face in that plane looking that way (the normal points out of
    the piece; within `FACING`, 1°, and 0.001 mm). Where the whole piece lies behind the plane it is clipped by the
    plane moved in; otherwise only the slab behind the face itself is taken, so the foot of an L isn't cut off.
  - Inwards only: a negative distance is refused (`Input`): there is no solid outside a face to add. An offset never
    changes a piece's key; if it parts the piece, the mesh holds the lumps together.

## Conflicts

`Conflict` is what a part shows after its host changed; each has a message that says what to do.

| Conflict | When |
|---|---|
| `Misses` | the cut passes every piece it was for, or only grazes them (what it would cut off is noise) |
| `HoldsAll` | a closed line or profile holds all of every piece it meets |
| `DoesNotCross` | an open line stops short of a piece's outline, or ends in an opening the piece closes round (a window): strict, as Revit: one piece it enters and doesn't cross refuses the cut |
| `TooDeep` | a patch's depth is the part's whole thickness or more under it |
| `NoSuchPiece` | the piece a cut's `of` or an offset names isn't among the pieces |
| `NoSuchFace` | the plane an offset names holds no face of its piece |
| `NothingLeft` | an offset takes all its piece has |

## What is promised, and what isn't

- **Closed**: pieces of plane cuts are closed by construction. Pieces of the other cuts are closed after mending in
  every case the tests cover (boxes, walls with doors and windows, a faceted curved wall, leaning profiles, sequences
  of cuts, 240 random recipes on three solids), but a partition gives no proof, so every `Piece` says so itself (`closed`, by `is_closed`: with corners
  within `WELD` made one, every edge has as many triangles running one way along it as the other).
- **Volumes add up**: the pieces and `removed` make the solid; the tests hold each to a millionth of it.
- **Noise**: a lump of less than `MIN_VOLUME` (1 mm³) or thinner than `THIN` (0.01 mm: twice its volume over its
  surface) is left out and counted in `removed`.
- **Deterministic**: the same solid and recipe give the same pieces, triangle for triangle (no hash order reaches the
  output).
- **Bounded**: numbers are checked finite and within `MAX_COORD`; `csg::MAX_TRIANGLES` (20 000) triangles in the solid,
  `MAX_CUTS` (64) cuts and as many offsets, `MAX_LINE_POINTS` (2 000) points a line, `MAX_PIECES` (256) pieces,
  `MAX_GAP` (10 m); every loop spends from a budget (`Work`: its own steps and a `csg::Meter` of eight cuts' worth;
  the worst a hostile solid and recipe take before they are refused is a few seconds).
  Nothing panics; `hostile_input_is_refused_and_never_panics_or_hangs` and
  `random_recipes_never_panic_and_what_they_make_adds_up` sweep it.
- **Precision** is `f64`'s at the coordinates given: a part kilometres from the origin loses the hundred-thousandths of
  a millimetre the welding relies on. Divide in the host's coordinates.

## Measuring a piece

`volume(mesh)` is the net volume after cuts, gaps and offsets. `area_facing(mesh, normal, tolerance)` is the summed
area of the triangles whose outward normal is within `tolerance` degrees of `normal` (use `FACING`, 1°, for flat
faces; more for a faceted curve): a part's main face, a wall's elevation or a floor's top, net of openings, of holes
cut through it and of what gaps and offsets took. A pocket's floor looks the same way as the face it was cut into, so
the rest round a patch cut to a depth still shows the whole elevation; the patch shows its own.

## The prism's way

A wall's layer is a prism: `Prism { frame, outline, holes, thickness }`, the frame on its face with the normal pointing
out, the solid `thickness` behind it (an outline and each opening of `MAX_LINE_POINTS` points at most, `MAX_HOLES`,
256, openings, none reaching outside the outline or over another). `Prism::mesh` is its closed surface. `divide_prism` does a recipe of lines drawn
on the face (or on a plane level with it, seen from either side) that cut through with no gap in the plane itself: the
outline is divided by `geom::faces` and each region raised into its piece, exact and closed by construction, with the
same keys, sides, lump order, noise rule and conflicts as the general way (two tests hold the two to each other, one on
150 random recipes). Any other recipe (a plane, a depth, a gap, a profile, an offset) goes to `divide` on the prism's surface.

## Wiring it to parts

A part's solid is `Model::part_solid(part)` (the data model's half: a closed mesh in model coordinates, the same
winding). Keep a `Recipe` on the part, in the host's coordinates; on any change of the host call `regenerate` on the
solid brought into those coordinates, make a sub-part per `Piece` under its `PieceKey` (which is `Ord`, `Hash` and
serializes), and show `Outcome::conflicts` on the part. `volume` and `area_facing` give each sub-part's quantities.
The other half's `parts/solid.rs` has its own `volume`, `facing` and `closed`; they agree with these on a closed mesh
(its welding is 1e-6 mm, finer than `WELD`, so what it calls closed is closed here; its `facing` takes a hundredth of a
degree where `area_facing` takes the tolerance it is given).

## Limits and open questions

- **Curved hosts**: a curved wall comes as flat facets; planes, lines and profiles cut it, but a line "drawn on its
  face" is drawn on a plane and cuts square to that plane, not to the curve. A sketch developed along the curve would
  need a frame per facet or an unrolled face.
- **Arcs in the sketch**: lines are polylines; an arc is given as its points (2 000 at most).
- **Outward offsets** are refused. Growing a piece needs the neighbour to give way, which is a recipe on both.
- **Several lines in one cut**: each line is its own cut, so that a piece's side of it is its name. Two lines that only
  together part a piece (one each side of a window) are drawn as one line across the opening.
- **Performance**: planes, and straight lines that span the piece, clip triangle by triangle: a curved wall of 7 200
  triangles is cut by three of them in tens of milliseconds. The other cuts go through a partition, a few milliseconds
  a cut on a wall's layer of a few hundred triangles. A partition cuts each face of the cutter by every face of the
  solid it runs through, so a solid of thousands of small faces that aren't in one plane (a curve in hundreds of
  facets) is costly, and past ten times the triangle cap in one piece the cut is refused (`TooMuch`), as is a recipe
  that spends the budget. Cutting only the part of the solid near the cutter (clip it to the cutter's box first, and
  join what was left out back on) would lift this; it isn't done.
- **A closed patch touching the outline** is fine; an open line running along an edge of the solid cuts off noise and
  is a `Misses`.

# GEO-1: families as boundary representations

Status: design note, for the project's decision. No kernel code lands before it is approved.

## Why

A family's forms are built today as triangles (`family::Piece::tris`), with arcs chorded at 32 per turn (`sketch::chorded`).
What follows from that is visible and wrong for an architect: a circle is a polygon when zoomed, a flat face has no identity
(its triangles are what plans, picking and snapping see), a revolve's flat top can show its triangulation in plan, and IFC
writes every form as an `IfcTriangulatedFaceSet` / closed face set. The project's decision, which this note takes as given: a form is
a boundary representation as Rhino's is, exact curves (line, arc, circle, ellipse, NURBS) bounding exact faces (plane, cylinder,
cone, sphere, torus, surface of revolution, ruled and swept, NURBS), and triangles are only what a picture is drawn with.

## The kernel

A spike (a scratch crate, nothing in the repo) built the six forms and their voids on three pure-Rust kernels:

| | arris 0.11.1 | monstertruck 0.4.2 | brepkit 4.11 |
|---|---|---|---|
| Licence | MIT / Apache-2.0, every dependency permissive | Apache-2.0 | AGPL-3.0-only: excluded |
| `unsafe` | `forbid` in all 8 crates | none found, not forbidden | |
| Errors | every operation returns `Result`, cancellable | errors, and a revolve of division 0 panics | |
| Revolved ring | 4 faces (2 planes, 2 cylinders), 4 exact circles | 8 to 16 NURBS faces, circles as NURBS | |
| Box − box sharing faces | ok | fails (`NotClosedShell`) | |
| Box − cylinder | ok, exact circles, 87 ms | inexact edges, 657 ms | |
| Sweep along a path with a corner | refused (corners) | no sweep along a path | |
| Loft square → circle | ok, 4 NURBS faces + 2 caps | assembled by hand | |
| Clean release build | 4 min 41 s, 51 crates | 3 min 45 s, 94 crates | |

**arris** is the recommendation: the only one that kept analytic faces exact, did the shared-face booleans and never panicked.

### Pin and upgrades

arris is pre-1.0 and breaks its API about every ten days. It is pinned exactly (`arris = "=0.11.1"`, and its sub-crates), with
`Cargo.lock` committed as today. An upgrade is a PR of its own: the version bump, the adapter changes, the full kernel test
suite (below) and the benchmark, compared with the last run. No upgrade rides along with a feature. Its debug-only validity
check panics on failure (release returns an error): every kernel call goes through the adapter, inside `catch_unwind`, as
`Session::execute` already wraps every command, and a panic there is a refused build of that form, not a crash.

## The trait

`bimcraft-core` gains `family::kernel`, a trait the forms are built through:

```rust
pub trait FormKernel: Send + Sync {
    type Solid: Send + Sync + Clone;
    fn extrude(&self, profile: &Loops, frame: &Frame, from: f64, to: f64) -> Result<Self::Solid, KernelError>;
    fn revolve(&self, profile: &Loops, axis: Axis2, start: f64, end: f64) -> Result<Self::Solid, KernelError>;
    fn sweep(&self, profile: &Loops, path: &Path) -> Result<Self::Solid, KernelError>;
    fn loft(&self, sections: &[Section]) -> Result<Self::Solid, KernelError>;
    fn cut(&self, body: &Self::Solid, tool: &Self::Solid) -> Result<Self::Solid, KernelError>;
    fn fuse(&self, a: &Self::Solid, b: &Self::Solid) -> Result<Self::Solid, KernelError>;
    fn bounds(&self, s: &Self::Solid) -> Bounds;
    fn tessellate(&self, s: &Self::Solid, chord: f64) -> Mesh;
    fn edges(&self, s: &Self::Solid) -> Vec<ExactEdge>;
    fn faces(&self, s: &Self::Solid) -> Vec<ExactFace>;
}
```

`Loops`, `Path`, `Section` are what `forms.rs` reads from a sketch today, with arcs kept as arcs (centre, radius, angles) rather
than chorded. `ExactEdge` is a line, an arc, a circle, an ellipse or a NURBS curve; `ExactFace` a plane, cylinder, cone, sphere,
torus or NURBS surface with its trimming loops. Both are BIMCraft's own types, so nothing outside the adapter names arris.

Two implementations:

- **`MeshKernel`, always built**: today's code, behind the trait. `edges` returns the creases with arcs as chords, `faces` the
  coplanar groups of triangles. Every family builds and draws exactly as it does today with it.
- **`ArrisKernel`, behind the cargo feature `brep`** (in `bimcraft-core`, on in the desktop and the CLI, off by default for a
  build that doesn't want it): the adapter.

With `brep` off, core builds and draws every family through `MeshKernel`: nothing changes for upstream until it chooses to
turn the feature on.

## Fill-in work on top of arris

- **Mitred sweep along a path with corners**: arris refuses corners. The adapter splits the path at each corner, sweeps each
  run and closes the mitre with the planar cut along the bisector (what `forms::stations` does today), then fuses: a fuse of
  solids meeting on a planar face, the case the spike showed arris handles.
- **Swept blend**: no such operation. A loft through the two profiles placed square to the path at its ends and through
  intermediate sections along it (the spike's smooth loft through three sections worked along an arc).
- **Coplanar faces after a fuse**: arris leaves them split. A merge pass joins adjacent faces on the same plane with no edge of
  another face between them.
- **Vertex mapping of a blend** (`forms::mapped`, twist): kept, as the order the loft's sections are given in.

## Speed, measured for the worst case

The worst case is a facade of 10,000 panels, **every one different** (instance parameters), so no two instances
share a solid. Release build, 8 cores:

| 10,000 unique panels | extrusion + revolve + loft, no boolean | the same + one cylinder void each |
|---|---|---|
| Build, all cores | 6.8 s | 255 s |
| Full regeneration (every panel changes) | 5.4 s | **805 s** |
| 10 panels changed | 8 ms | 0.68 s (the cut is what costs: through-voids as profile holes bring it to the no-void case, about 10 ms) |
| Plan edges, exact, projected | 29 ms (280k edges) | 38 ms |
| Mesh at 2 mm | 6.9M triangles, 50 s | 7.4M triangles, 60 s |
| Peak memory | 1.5 GB | 4.2 GB |

Booleans are 95 to 97% of the time with a void, and scale badly across cores (55 ms on one thread, about 200 ms of CPU each on
eight). arris has no bounding-box early-out: a void that misses the slab still costs 33 ms. Cut probes on a slab: a box void
0.6 ms, a 32-gon prism 4 ms, **the same hole as a hole of the profile 0.15 ms**.

The user may model anything, so these become the program's work, not the user's limits:

1. **Fast paths before any boolean**, in the adapter:
   - a void that is an extrusion along the body's own direction and passes through it becomes a hole of the profile (about
     400 times cheaper);
   - a void whose bounds miss the body's is skipped;
   - a void with planar faces only takes the planar boolean (0.6 ms for a box);
   - the general boolean only where none of these applies, its contention across threads investigated (allocation, shared
     state) so eight cores give about eight times.
   Measured target for the facade with one void per panel: the voids that are through-holes (most windows and fixings in a
   panel) at the profile-hole cost, so the full regeneration in the order of the no-boolean case, about 10 s; the rest by the
   planar or general path.
2. **A cache of solids by recipe**: a solid is keyed by what built it (the form's sketch read for the instance, its numbers, its
   voids' keys). An edit rebuilds only the keys that changed (8 ms for 10 panels). Builds run on worker threads, the view
   showing the last solid of each until its new one arrives, as `section_worker.rs` does for sections.
3. **Meshes lazily, by zoom**: a solid is meshed at the chord the view needs (about a pixel), on a worker, and kept per chord
   band. Instancing still serves the families that do share.

**Is the cache saved with the project?** Not in the first step: it lives in memory and on disk beside the project as a cache
file (like a thumbnail cache, safe to delete), so FORMAT doesn't change. Storing solids inside `.bimc` would change FORMAT and
is a decision of its own; it is not proposed here.

## Drawings, picking and IFC

- **Plans, ceiling plans, elevations and sections** draw the exact edges and the silhouettes of curved faces (a cylinder's
  outline lines, a sphere's circle), cut where the section cuts, arcs as arcs (`render::Prim` already draws arcs). Nothing
  draws a triangle's edge. `render::below_cut`'s shadow of triangles becomes the exact faces' shadow.
- **{3D}** draws the lazy mesh; its edges are the exact edges, tessellated at the same chord.
- **Picking, snapping, Align, dimensions, FE-28's locks** take exact faces and edges: a cylinder's axis, a circle's centre, a
  planar face as one face.
- **IFC4**: a form whose solid is a single extrusion or revolution of a profile goes out as `IfcExtrudedAreaSolid` or
  `IfcRevolvedAreaSolid` with `IfcArbitraryProfileDefWithVoids` and `IfcCircle`/`IfcTrimmedCurve` in its curves; anything else as
  `IfcAdvancedBrep` (`IfcAdvancedFace` on `IfcPlane`, `IfcCylindricalSurface`, `IfcBSplineSurfaceWithKnots`, edges as
  `IfcCircle`, `IfcLine`, `IfcBSplineCurveWithKnots`). IFC2x3 has no advanced B-rep: it keeps the faceted brep from the mesh.
  The IfcOpenShell checks (`ci/ifc-coverage.txt`) gain each entity.

## What stays as it is

- **Files**: no FORMAT change. A family stores its recipe (sketches, numbers, voids, cut and join lists) as today; the B-rep is
  built from it. Every file that opens today opens and builds the same forms.
- **Project elements** (walls, floors, roofs, openings) stay mesh. A family placed in the project gives the project a mesh
  (from `tessellate`) where the project needs one: hosting cuts of walls, the derived caches, `bounds.rs`, the GPU scene. Its
  plan edges come from the B-rep.
- **Constraints** are solved per type and instance before building, as now; the kernel only builds.

## Meanwhile

Until `ArrisKernel` lands, a zoomed circle stays faceted (32 chords a turn): the chord no longer follows the zoom in a
bridge of its own, since the kernel makes it exact. That is the trade-off of waiting for the kernel rather than patching
the mesh.

## Costs

- **CI build time**: about 4 min 41 s more for a clean release build of the spike alone on Windows. The cost on the three CI
  jobs is to be measured on the first PR that adds the dependency, and reported in it; incremental CI builds cache it.
- **Binary size and memory**: to be measured on that PR (the spike's peak memory is above).

## Landing

1. This note.
2. `family::kernel` with `MeshKernel` only: today's code behind the trait, arcs kept exact in `Loops` and `Path` though chorded
   in the mesh. No dependency, no behaviour change, tests unchanged.
3. `ArrisKernel` behind `brep`, with the kernel test suite: the six forms, voids, cut and join lists, hostile input (NaN,
   zero, self-crossing, huge), the spike's cases, and a pixel test of the revolve's plan. The fast paths and the cache.
4. Plans and elevations from exact edges; {3D} lazy meshes by zoom; picking on faces.
5. IFC4 with exact curves.
6. Upstream: one PR per step from 2, each with `brep` off by default until upstream turns it on.

# Exact and fast family geometry: where we stand

Status: recap for discussion, 2026-11-19. It gathers the B-rep and NURBS work on the fork so far: the goal, every test run,
what is merged, what the numbers say and the options still open. It decides nothing. **It summarises the GEO-1 design note and
does not replace it**: the note
([`docs/brep-plan.md`, PR #74](https://github.com/acpvarchitects/BIMCraft/blob/cp/geo1-design-note/docs/brep-plan.md)) holds
the design (the trait, the fill-ins on top of arris, the landing steps) and is the document to decide on. This recap adds what
happened since: the benches, the merged steps and the options;
the raw tables, logs and harness are on the orphan branch
[`bench/kernels`](https://github.com/acpvarchitects/BIMCraft/tree/bench/kernels). Upstream thread: IPedrax/BIMCraft#499.

## 1. The problem

A family's forms are built as triangles (`family::Piece::tris`). That is fast, but an architect sees what follows from it:

- a circle is a polygon when zoomed;
- a flat face has no identity of its own, so plans, picking and snapping see its triangles;
- IFC writes every form as a triangulated face set, so curves are lost on export;
- a boolean between two curved forms is only as exact as the chord it was meshed at.

The goal is family geometry that is **exact** (exact curves bounding exact faces: plane, cylinder, cone, sphere,
torus, NURBS) **and fast**, with no feature lost. A model with no curved surfaces is not acceptable. A model that is slow to
navigate or edit isn't either.

## 2. Tests run

### 2.1 First spike: three pure-Rust kernels (scratch crate)

| | arris 0.11.1 | monstertruck 0.4.2 | brepkit 4.11 |
|---|---|---|---|
| Licence | MIT / Apache-2.0 | Apache-2.0 | AGPL-3.0-only: excluded |
| Revolved ring | 4 faces (2 planes, 2 cylinders), exact circles | 8 to 16 NURBS faces, circles as NURBS | |
| Box − box sharing faces | ok | fails (`NotClosedShell`) | |
| Box − cylinder | ok, exact circles, 87 ms | inexact edges, 657 ms | |
| Hostile input | errors, never panics | a revolve of division 0 panics | |

arris was the only one that kept analytic faces exact, did the shared-face booleans and never panicked.

### 2.2 Six-kernel bench (ADE, shared 72-core Linux box, 2026-11-17)

A single suite was ported to every kernel: the six family forms, the shared-face booleans, hostile input, a NURBS table and
10,000 panels that are all different. Each panel is an L slab, a revolved ring, a square-to-circle loft and one round
through-void. Every call ran in a child process with a 90 s budget. The machine was under heavy load, so compare ratios.

| | arris 0.11.1 | solidcore 1.0.1 | OCCT 7.8 (opencascade-rs) | truck 0.6 | Fornjot 0.49 |
|---|---|---|---|---|---|
| Forms | all but the cornered sweep | all, the cornered sweep too | all | no cornered sweep, no smooth loft | no revolve, loft, sweep or boolean |
| Shared-face booleans | ok | ok | ok, fast | four return nothing | can't build |
| Hostile input | errors | degenerate input accepted as valid | NaN segfaults | errors | panics |
| Meshing | chord-controlled | **none yet** | yes | yes | plane and circle only |
| 10,000 panels, void as a profile hole, 8 / 72 threads | 4.2 / 0.8 s | 23 / 7.4 s | 5.8 / 5.2 s | 0.12 / 0.21 s (meshing times out) | 8.1 / 3.2 s (slab only) |
| The same, void as a boolean cut | 49 / 15 s | 35 / 13 s | 14 / 27 s | over 90 s | 9.0 / 3.9 s (slab only) |
| Licence, `unsafe` | MIT/Apache, forbidden | Apache, forbidden | LGPL C++ | Apache, not forbidden | MIT/Apache, used |

The baseline is BIMCraft's mesh path: 0.04 s without voids and 0.12 s with a mesh-CSG void, on 72 threads, at a fixed
tessellation.

What the bench says about each kernel:
- **arris** is the candidate;
- **solidcore** is the strongest on geometry (free-form booleans in 17 to 52 ms, fillets, mitred sweeps), but its released
  crate can't mesh a surface, has no STEP writer and accepts degenerate input silently (its `main` meshes: see 4.3);
- **OCCT** is useful only as a test oracle;
- **truck** and **Fornjot** are out.

### 2.3 Two ordinary laptops (2026-11-18)

Both laptops are Core Ultra 9 275HX (24 threads, 64 GB, Windows 11), with other apps open. One run used MSVC and the other
windows-gnu. Results are in `results/workstation-emiliano/` and `results/workstation-vincenzo/`. These are 10,000 panels on
MSVC:

| | 1 thread | 8 threads | 24 threads |
|---|---|---|---|
| Mesh path, no void / mesh-CSG void | 0.77 / 2.48 s | 0.24 / 0.46 s | 0.17 / 0.28 s |
| arris, void as a profile hole | 20.0 s | 3.0 s | **1.5 s** |
| arris, void as a boolean, default heap | 495 s | 435 s | 411 s |
| arris, void as a boolean, mimalloc | – | 57.5 s | 31.0 s |

**The allocator matters as much as the kernel on Windows.** The boolean path didn't scale at all on Windows' default heap, and
mimalloc as the global allocator brought it close to the Linux box. The mesh path gains as well: with the void, 1.6× faster at
8 and 24 threads; without it, 2.5 to 3× faster. `cap::Cap<mimalloc::MiMalloc>` in the apps is a candidate upstream change,
for the maintainers to decide, because it is a new dependency.

### 2.4 What the numbers mean, and what they don't

- **The two paths do different work.** The mesh path's 0.04 s makes triangles at one tessellation. arris's 1.5 s makes exact
  solids, and meshing them at 0.5 mm costs more than building them. The two are not the same product at different speeds.
- **The mesh path already shows curved surfaces.** Revolves and lofts are curved, and since #84 their chord follows the zoom, so
  on screen they look curved at any zoom. What is missing is the exact description (this is a cylinder of radius 300) for
  drawings, export and booleans between curved forms.
- **10,000 unique panels is the worst case on purpose.** In a real project panels repeat, and a solid is built once per unique
  recipe.
- **Booleans are the gap.** With a void, booleans are 95 to 97% of arris's time. Making a through-void a hole of the profile
  makes it about 20× faster.
- **We have no reference for what an established BIM tool takes for the same facade.** Getting one is an open question for the
  maintainers.

## 3. What is merged on the fork (`followups/family-editor`)

| PR | What |
|---|---|
| #80 | `FormKernel` trait, today's code behind it bit for bit (`MeshKernel`) |
| #81 | Void fast paths: a through-void becomes a hole of the profile, a void that misses is skipped |
| #84 | Chord follows the zoom (`Fineness::for_view`) |
| #87 | Face tags (`FormFace`): every face knows which cap, side or cut made it |
| #88 | Fast plans of many unique components: culling, envelopes and a build pool. First draw of the facade 10.3 s → 0.3 s |
| #90 | Plan index kept by revision: redraw 38 ms, pan 53 ms, edit 39 ms on 10,000 unique panels |
| #89, #91 | Paint on a form's faces, in the family and per instance in the project, with surface patterns in projection |

None of these adds a dependency. arris is not in the product yet.

## 4. Options

### 4.1 Recommended: two representations of each form, each used where it pays

Every family form keeps two representations: its **mesh** and its **parametric definition** (the profile, the operation and
its parameters, as the user built it). Which one is read depends on the task.

**1. Display and interaction ({3D}, navigation, editing): the mesh.**
The tessellation is adaptive: the chord tolerance follows the view's scale, so curved surfaces look continuous at any zoom.
This is the current path, and nobody waits on it.
- Done for plans (#84).
- To do for {3D}, sections and elevations, which still use a fixed tessellation (GEO-2c-2).

**2. Plans, sections and IFC export: the parametric definition.**
- Plans and sections draw true arcs and circles taken from the definition, not the mesh's polylines.
- IFC4 writes the form as a procedural solid: `IfcExtrudedAreaSolid` or `IfcRevolvedAreaSolid`, with profiles on
  `IfcCircle` and `IfcTrimmedCurve`, and a void as an `IfcBooleanResult`. The geometry stays exact, and the importing
  application rebuilds it.
- Nothing is computed: the definition is only written out.
- Status: to do. Today, plans of curved forms are drawn from the outline of the mesh, and IFC is written as triangulated face
  sets.

**3. Exact computation: the B-rep kernel (arris, behind a `brep` feature).**
It runs only where a result has to be computed:
- booleans between curved surfaces, such as a cylinder cutting another cylinder;
- exact volumes and areas for quantities;
- STEP, or `IfcAdvancedBrep` with NURBS surfaces.

It runs on worker threads, never blocking the user, as sections already do (`section_worker.rs`). Each solid is computed once
per combination of parameters and kept in a cache, and while it is being computed the view shows the mesh. Measured cost: 1.5 s
for 10,000 unique panels on a laptop. Status: to do (`ArrisKernel`, the kernel test suite, the cache, lazy meshing, and the
GEO-1 fill-ins: mitred sweeps, swept blends, merging coplanar faces after a fuse).

**Result:** interaction at the mesh's speed, and B-rep precision where precision counts: drawings, export and quantities.

### 4.2 Our own exact kernel, limited to BIM forms

Families use few kinds of surface: planes, cylinders, cones, tori, and NURBS only for blends. A kernel that handles only
these, booleans included, is much smaller than a general one.
- For: exact, fast, made to measure, and no dependency.
- Against: weeks of work, and every boolean edge case becomes ours to own.

### 4.3 solidcore from `main`

The released crate (1.0.1) can't mesh, but solidcore's `main` already has the display stack: `solidcore-tess` and `convert`,
and an exact hidden-line module (`hlr`). harijsvucens measured them on IPedrax/BIMCraft#499 (Windows, 8 threads, a box with a
cylinder void per panel, so a different mix from ours: compare ratios):
- 10,000 panels: booleans in 20.5 s, near-linear scaling, 1.25 GB; built **and meshed** in 24.7 s;
- a one-panel edit: about 8 ms in the kernel plus 3 ms to mesh, with every triangle tagged by its face;
- exact plans (`hlr`): 1,000 panels in about 3 s; 10,000 are refused by its internal budgets, quickly and without hanging.

What's missing: a loft (blend), sweeps along cornered paths, published crates for the new parts, and a validity gate,
since some degenerate input still comes back as an empty or zero-volume "valid" solid. So the work of writing a tessellator
for it is no longer ours to do. The next step is to run solidcore's `main` through our same suite and panel on the same two
laptops, next to arris, and keep whichever does better behind `brep`.

### 4.4 monstertruck

monstertruck was tested in the first spike (2.1). It failed the shared-face boolean, its edges were inexact, and it panicked
on one hostile input. Not worth a second run unless it has changed.

### 4.5 Ruled out

- **Distance-field (implicit) modelling.** Very good for fillets and organic forms, but it doesn't give the exact faces IFC and
  STEP need.
- **OCCT as the product kernel.** It is LGPL C++, a NaN input segfaults it, and it scales badly with threads. It stays a test
  oracle.

## 5. Questions for the discussion

1. Is "mesh for interaction, parametric definition for drawings and IFC, B-rep only where a result must be computed" the right
   answer to "fast *and* curved"? Or should {3D} also show the B-rep by default, at the cost of a delay after big edits?
2. The parametric definition gives IFC4 exact curves for extrusions and revolutions with no kernel. Is that enough for the first upstream step,
   with `IfcAdvancedBrep` coming later?
3. Should mimalloc be proposed upstream now, independent of any kernel?
4. arris or solidcore? Decide after both run the same suite and panel on the same laptops (4.3).
5. How do we present the speed we have to people who judge it against established BIM tools?

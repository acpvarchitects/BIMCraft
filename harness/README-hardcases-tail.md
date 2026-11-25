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

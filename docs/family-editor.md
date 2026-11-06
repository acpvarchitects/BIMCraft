# The Family Editor: a plan, and how the work is split

This is the plan for ROADMAP 0.8's **Family editor sketch mode** (XL): reference planes, sketches drawn on screen, sweeps
along a path and blends. It also covers the 0.8 items that cannot be built apart from it. The goal is 0.8's exit criteria:
a user draws a parametric window in the Family Editor, places it, flexes its types, and it schedules and exports to IFC
correctly.

The work is split between two people: **Cristiano** (`feature-adaptive-families`, discussion #217) and **Emiliano** (MOD-16).
The split is in section 5. Each step below goes to discussion #217 before it is built, and lands as its own upstream PR.

---

## 1. Where things stand

### On upstream `main`

- **The family** (`family.rs`) holds parameters (with formulas), types and **forms**. Each form is one of these:
  - a profile in one of three fixed planes (`xy`, `xz`, `yz`), extruded from `from` to `to`;
  - the same profile revolved about an `axis`;
  - either of the above as a `void`, cut exactly by `csg::subtract`;
  - a `mesh` brought in from IFC.

  A family also has plan `symbols`, `connectors` and an `operation`. It is saved as this data, in the project and in a
  family `.json` file; its geometry is rebuilt on every use.
- **The Family Editor** (`editor.rs`) is a dialog with text tabs (Parameters, Types, Forms, Symbols) and a preview that turns.
  It changes the project only through the `family` command.
- **The project already has what sketch mode is made of:**
  - `sketch.rs`: lines and arcs solved into loops, with every problem named;
  - `draw.rs`: Revit's whole Draw gallery;
  - `workplane.rs` and the desktop's `sketch3d.rs`: sketching in 3D on any plane;
  - `constraints.rs`: locked and equal dimensions, re-solved after each command;
  - `csg.rs`: the void cut;
  - `massing.rs`: a loft between twin outlines, which is the first half of a blend.

### On `feature-adaptive-families` (Cristiano, 6 commits on top of `0733fd4`, 291 behind upstream)

| What | Where | Tied to adaptive? |
|---|---|---|
| The family opened as a **document that takes the window**: its own ribbon with Revit's tabs and contextual tabs, greyed placeholders for tools not built yet, Properties with Revit's groups, a Project Browser of the family's views, the Options Bar, Undo in the title bar, Close Family, and Load into Project (and Close) | `ui-egui/src/editor/document.rs`, `editor.rs` | **Shell no, contents yes**: it opens only for an adaptive family, and its panels read `ed.adaptive` and `ed.modeller` |
| **Family Category and Parameters** and **Family Types** as Revit's dialogs | `editor/document.rs` | No |
| Revit's **"="** on a Properties row: Associate Family Parameter and New Parameter | `adaptive_editor.rs` (`associate`, `FamilyEditor::associating`) | Rows are adaptive's; the mechanism is not |
| The template's **datums**: Ref. Level, Center (Front/Back) and Center (Left/Right) | `core/adaptive` (`Datum`) | Lives in `adaptive::Definition`, but is not adaptive-specific |
| **Work planes**: Placement Plane with Pick…, Set and Show (tinted, with a grid), and the plane under the pointer lit | `core/adaptive` (`WorkPlane`, `Evaluation::pick_plane`), `adaptive_view.rs` | Datums: no. A point's three planes: yes |
| **The modelling view**, drawn as Revit draws a family: grey ground, dash-dot level, dashed green reference planes, form edges where faces fold (`Mesh::folds`), the ViewCube, Shift+middle orbit, window and crossing box selection | `adaptive_view.rs` | Camera, canvas and selection: no. The things drawn: yes |
| **Move, Copy and Rotate** as Revit's two clicks; a gizmo with axis arrows; snaps to points | `core/adaptive/edit.rs`, `adaptive_view.rs` | The edits are on adaptive nodes; the interaction is not |
| **Edit Family** on an instance's right-click menu | commit `267fee7` | **No: it could go upstream on its own today** |
| How the work was done: a parity study measured in Revit (`docs/adaptive-revit-parity.md`), a core test for each edit, and a desktop test that drives each tool as a user would (`tests/adaptive_modelling.rs`) | | The method carries over as it is |
| Points (types, numbering, Orients to, measurements), curves, Patch, Extrude and Loft over a node graph, placement frames, capture, `adaptive_place` | `core/adaptive` | **Yes**. This is the conceptual environment, and stays adaptive's |

**About two thirds of the branch's desktop work is the Family Editor's foundation.** The node graph is not. It is the
right model for adaptive components, but not for a conventional family.

---

## 2. The one decision to make first: two environments, one family

Revit has **two family environments**:

1. **The conventional Family Editor**: reference planes; sketch-based Extrusion, Blend, Revolve, Sweep and Swept Blend;
   dimensions labelled with parameters. Doors, windows, furniture and fixtures are built here.
2. **The conceptual environment**: points, lines and Create Form. Adaptive components and masses are built here.

`feature-adaptive-families` says its `Definition` is the one canonical geometry and that nothing should be added beside it.
**The proposal here is to keep that true for adaptive families only.** Conventional families keep `Family.forms`,
extended, for these reasons:

- Upstream's families and the starter library are `forms`. Every path that reads them already works: plans, sections, 3D,
  IFC, schedules, `render`, and the hosted door and window rules.
- Sketch-based forms are profiles on planes, which is what `Form` already is. The conventional forms grow from it with a
  migration, not a rewrite.
- The node graph has no reference planes driven by dimensions, which is the heart of a conventional family.

**What both environments share, and so moves into the family itself (`Family`, not `adaptive::Definition`):**

- the datums and reference planes;
- work planes;
- parameters and the "=" association;
- types;
- the document shell.

A form of either kind is a `Piece` in the end, as `Family::pieces_at` already makes it, so everything downstream stays
one path.

**One editor, two kinds of form.** The editor is one: the document, its ribbon, Properties, reference and work planes,
parameters with "=", Family Types and Load into Project serve every family. Revit keeps two editors; we don't need to.
What stays two is the geometry under it.

**Why not one geometry model for both:**

1. **Every family would migrate.** That means the starter families, families in projects, Favorites and family files, and
   IFC imports. It takes a format bump and a migration, and puts at risk what works now.
2. **They are driven differently.** Conventional forms are 2D sketches tied to reference planes, moved by dimensions
   through a 2D constraint solver. Adaptive forms are 3D points with frames, evaluated as a graph. One model has to carry
   both or force one onto the other, and both solvers get harder.
3. **What is downstream is built on forms.** That covers a door's or window's opening sized from Width and Height, plan
   symbols, visibility by detail level, section cuts and IFC. An adaptive instance's shape is its own, while a
   conventional one shares its type's: per-type geometry reuse (`component_geometry.rs`) and the component caches rely
   on that.
4. **Revit users expect the difference.** The tools differ (Extrusion, Blend and Sweep against Create Form), so the
   interface would need modes anyway.
5. **It would not merge well.** One rewrite of `Family`, edited by two people at once, is a harder review upstream than
   steps that each leave everything working.

**What two models cost, and how to keep it small:**

- **Duplication.** Extrude, Loft against Blend, voids, materials and visibility exist in both. Both call the same kernel
  underneath: profile triangulation, extrude, loft and `csg`. Blend is built on the adaptive Loft's code, not on a
  second copy.
- **The adaptive note** ("no disconnected geometry recipe") is read as applying within adaptive families.
- **Mixing stays possible.** A family can already hold both `forms` and `adaptive`, and `pieces_at` builds both, so a
  conventional family with an adaptive part needs no merged model.

If the two converge later, one model can follow then, as a migration of data rather than a rewrite.

**Ask on #217 (dansity, IPedrax) before step F2.** This changes the saved family, so it needs their agreement.

---

## 3. What the family gains (the data, all in `bimcraft-core`)

A sketch, not a schema: each step's PR settles the field names. Every new field is optional and skipped when empty, so
families that don't use it are written as before. A family that uses one needs the next free file format number.

```text
Family
  planes:  [RefPlane]       named reference planes; the three datums are there by default
  forms:   [Form]           as today, plus:
Form
  host:    work plane       a RefPlane, a level of the family, or a face, in place of the fixed xy/xz/yz
  sketch:  [SketchLine]     lines and arcs (sketch.rs's), in place of a point list; the old profile migrates
  kind:    extrusion | revolve | blend | sweep | swept_blend
           blend:  a top sketch at a height
           sweep:  a path (sketch or picked lines) and a profile square to it
  void, material, visible   as today
RefPlane
  id, guid, name, two ends in a view, is_reference (Strong / Weak / Not a Reference), defines_origin
Dimension (family)
  between planes or sketch lines; locked; EQ; label = a parameter (the solver, step F7)
Alignment (family)
  a sketch line locked to a plane (Align's padlock)
```

---

## 4. The steps

Sizes: **S** a few days, **M** one to two weeks, **L** about a month. Each step lands with core tests first, then a
desktop test that drives the tool.

| # | Step | Size | Depends on |
|---|---|---|---|
| **F0** | **Bring `feature-adaptive-families` up to upstream**: merge 291 commits and take the next free file format number (upstream is on 17). `family.rs`, `editor.rs` and `lib.rs` have changed a lot upstream. | M | none |
| **F1** | **Edit Family on an instance's right-click menu**, PR'd on its own from `267fee7`. | S | F0 |
| **F2** | **Datums and reference planes in the family (core)**: `Family.planes`, with the three datums moved out of `adaptive` (adaptive's `WorkPlane` then names them); a `ref_plane` part of the `family` command, by id and GUID like forms; `Form.host` replaces `plane` (migration: `xy` → Ref. Level, `xz` → Center (Front/Back), `yz` → Center (Left/Right)); IFC unchanged. Adaptive reads the datums from `Family.planes` (`adaptive::Datum` and `WorkPlane::Datum` go; `pick_plane`, Placement Plane and point hosts follow), and a point hosted on a datum names the plane by its GUID. The proposal agrees this shape with Cristiano before code; he does the adaptive side. | M | the decision in section 2 |
| **F3** | **The family document for every family**: the window-wide document opens for any family; the old dialog's tabs become Properties, Family Types, and Family Category and Parameters; "=" on any number, yes/no or material row; the ribbon's Create tab shows Revit's Forms panel (Extrusion, Blend, Revolve, Sweep, Swept Blend, Void Forms), greyed until each lands. | L | F0 |
| **F4** | **The family's views**: Ref. Level floor and ceiling plans, the four elevations and {3D} in the Project Browser, each drawn as Revit draws a family (reference planes dashed green with their names, forms seen and cut); **Reference Plane** drawn in plan or elevation; Work Plane Set / Show over the new planes. The project's own drawing code runs on a scratch model, as `render {family}` already does. | M | F2, F3 |
| **F5** | **Sketch mode in the family**: Create Extrusion and Revolution enter sketch mode on the work plane, with the Draw gallery, Pick Lines, the loop checks, and Finish or Cancel Edit Mode; **Edit Sketch** on a form; drag an extrusion's ends. `Form.sketch` holds lines and arcs; arcs are tessellated when built, as `sketch.rs` does. | L | F2, F3, F4 |
| **F6** | **Blend, Sweep and Swept Blend** (core and tool): a blend's two sketches, joined vertex to vertex as `massing.rs`'s loft does, with Edit Vertices later; a sweep's path (sketched or picked) and its profile square to the path's start, mitred at corners; a swept blend's two profiles; voids of each. | L | F5 |
| **F7** | **The 2D constraint solver** (ROADMAP's own L item): dimensions between reference planes and sketch lines, labelled with parameters, locked and EQ; Align with the padlock; flexing a type re-solves the planes, and what is locked to them follows. It builds on `constraints.rs` and on F3's "=". | L | F4, F5 |
| **F8** | **Visibility of forms by view direction and detail level** (ROADMAP's M item): Visibility/Graphics of a form (Coarse, Medium, Fine; plan and RCP, front/back, left/right), and symbolic lines in plan replacing a form's outline. | M | F4 |
| **F9** | **Sections cut family forms** (ROADMAP's S item): poché where a section passes through a component's or an opening's form, from the pieces already built. | S | none |
| **F10** | **The exit test**: a parametric window drawn from blank in the document, with reference planes, dimensions labelled Width and Height, an extruded frame and a void, then placed, flexed through its types, scheduled and exported to IFC (IfcOpenShell checks it). This scripted test is 0.8's exit criteria. | S | F5 to F8 |
| **F11** | **Shape by Code** (suggested by dansity on #267): a form whose shape is a short script over the family's parameters and materials, written in a code box opened from a button in the family document, where a sketch-based form would be drawn. The script produces the 3D solid and the plan's 2D lines, and it flexes with the types like any form. It suits very complex families (a window covering every reveal, shading and opening). It builds far faster than sketches and booleans, and agents write it more easily than sketches. ArchiCAD's GDL is the precedent; we aim for a more modern language. **Research first:** which language and syntax (JavaScript, or an embeddable Rust one such as Rhai or Lua), and how it runs safely (sandboxed, no I/O, capped steps, memory and triangles, never crashing on hostile code). The result goes on #267 as a proposal before code. | L | F2, F3 |

Not in this plan: nested and shared families, type catalogs, hosted families on faces, and adaptive parity. Each is its
own roadmap item and builds on F2.

---

## 5. Who does what

The split follows what each of us has already built. **Cristiano** owns the document, the desktop shell and the solver,
which builds on his "=". **Emiliano** owns the core geometry and the drawing of views, next to MOD-16's work on forms,
planes, sketches and IFC.

| Step | Owner | Why |
|---|---|---|
| F0 bring the branch up to upstream | **Cristiano** | His branch; it blocks F1 and F3 |
| F1 Edit Family PR | **Cristiano** | His commit |
| F2 datums, reference planes and `Form.host` (core) | **Emiliano** | The data model and its migration; it moves `Datum` out of `adaptive`, so Cristiano reviews it |
| F3 the document for every family | **Cristiano** | His shell, generalised |
| F4 the family's views | **Emiliano** | Plans, elevations and cuts on a scratch model, the project's drawing code |
| F5 sketch mode in the family | **Emiliano** for the core and the sketch tools; **Cristiano** wires them into the ribbon and Options Bar | Reuses the project's `sketch.rs`, `draw.rs` and `sketch3d.rs`; the document is his |
| F6 Blend, Sweep, Swept Blend | **Emiliano** | Solid-building code in `family.rs` and `csg` |
| F7 the constraint solver | **Cristiano** | He proposed it on #217, and it builds on his "=" |
| F8 visibility per view and detail level | **Emiliano** | View-side drawing |
| F9 sections cut family forms | **Emiliano** | Section drawing; independent, can start now |
| F10 the exit test | **both** | Last, together |
| F11 Shape by Code | **to be decided** | Research and a proposal on #267 first; not part of 0.8's exit test |
| CW-04 adaptive panels, adaptive parity | **Cristiano** | Goes on alongside, on F2's planes |

**How the two lines run side by side:**

- **Start now, in parallel:** Cristiano does F0 and then F1. Emiliano does F9 (nothing to coordinate) and drafts F2's
  decision for #217.
- **Then:** F2 (Emiliano) and F3 (Cristiano) run at the same time. F3 can open on today's fixed planes and switch to F2's
  planes when F2 lands.
- **Then:** F4 and F5 (Emiliano) while F3's tail and F7's design (Cristiano) go on. F6 and F8 follow F5.

---

## 6. Rules between the two branches

- **The file format number:** whoever opens their upstream PR first takes the next free number; the other merges upstream
  and takes the one after. Say so in the Agora room the moment a number is taken.
- **`Family` is shared ground.** A new field on `Family`, `Form` or `RefPlane` is proposed in the room before either of
  us writes it.
- **Each step is proposed on #217 before it is built**, as promised there; it gets one PR, and the other person reviews it.
- **The project's rules hold as they are:**
  - every change is a command;
  - `Model::check` validates;
  - the core has no UI;
  - tests come first;
  - nothing panics;
  - text goes through `msg!`, with `cargo i18n update` before a push;
  - nothing from Autodesk's content or help text;
  - every Revit behaviour we copy is described in our own words from using it.

# Heritage Parts: a plan, and how the work is split

This is the plan for **Parts** in BIMCraft, after Revit's Parts and Archicad's Morphs, aimed at the recording and
restoration of historic buildings (HBIM). It is written for review before anything goes upstream: what is built, the
decisions that need the maintainers' agreement, the steps that remain, and the order in which they would land.

**The goal.** A user takes a historic wall apart into its layers (historic plaster, rubble masonry, a later lining). They
outline on its face a zone of rising damp, a detached render or a stratigraphic unit, and record what the survey says of
each piece. They get a decay map in plan, elevation and 3D, and a bill of quantities in m² and m³ by intervention. They
export IFC that other tools read, with each piece an `IfcBuildingElementPart` of its wall. When the wall moves, is
stretched or gets a window, the pieces follow it. When a piece can no longer follow, it says so instead of disappearing.

The work is Vincenzo Panasiti's (ACPV), done by his agents in the Agorà room *BIMCraft Heritage Parts (HBIM)*. The
workstream's owner is `vp-w-cld-ade-heritage`. It is separate from the Family Editor: it starts from upstream `main`, not
from `release/family-editor` or `followups/family-editor`, and it touches different files, apart from the shared
`FORMAT` (section 2.3). **Nothing goes upstream without Emiliano's OK.** Section 9 proposes the PRs.

---

## 1. Where things stand

Integration branch on the fork: **`feature/heritage-parts`**, 7 commits on upstream `main` f202abd (FORMAT 18); the
steps after HP-1 are fork PRs into it, each with green CI, waiting for review (2026-11-12).
Compare: <https://github.com/acpvarchitects/BIMCraft/compare/f202abd...feature/heritage-parts>

| Step | What | Where | State |
|---|---|---|---|
| HP-1 | The part element, its solid and quantities, its values by name, `create_parts` / `part` / `parts`, IFC export, material values, `docs/areas/parts.md` | `feature/heritage-parts` (b7a9847) | **Done**, waiting for review |
| HP-1 fix | A part measured on the face it shows: a wall's interior plaster on the interior face, a ceiling's plaster on its underside (Vincenzo's decision, 10.2) | [#19](https://github.com/acpvarchitects/BIMCraft/pull/19), `vp/heritage-parts-net-area` | **Done**, in review |
| HP-2 | The division engine: a part's solid cut by planes, lines drawn on a face, extruded profiles; gaps and offsets; measuring | [#16](https://github.com/acpvarchitects/BIMCraft/pull/16), `vp/heritage-parts-divide`, `docs/areas/parts-geometry.md` | **Done**, in review |
| HP-3 | `divide_parts`: HP-1 and HP-2 wired together | | Next (on HP-2) |
| HP-4 | Parts Visibility per view: Show Original, Show Parts, Show Both | [#20](https://github.com/acpvarchitects/BIMCraft/pull/20), `vp/heritage-parts-views` | **Done**, in review |
| HP-5 | Colour maps (`theme`) by `Decay_Pathology`, `Historical_Period` or any part value, with a legend | same PR | **Done**, in review |
| HP-6 | Schedules of parts: the bill of quantities (Restoration Quantities), CSV and xlsx | [#17](https://github.com/acpvarchitects/BIMCraft/pull/17), `vp/heritage-parts-schedules` | **Done**, in review |
| HP-7 | Desktop: Create Parts, Divide Parts (sketch on the face), Properties of a part | | Not started |
| HP-8 | IFC import of parts | | Not started |
| HP-9 | The `FORMAT` step | | Not started; needs agreement (2.3) |

What HP-1 was checked with, on b7a9847:
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings` and `cargo i18n check` are clean.
- `cargo test --workspace` excluding the desktop: 2200 passed. One failure, `network_tests::a_drain_as_long_as_a_query_takes_is_checked_in_time`, is a 10-second timing test that fails the same way on untouched f202abd when the machine is loaded, and passes alone.
- The desktop: 822 + 57 GPU tests passed. 53 tests are new.
- The hostile-params sweep takes a wall and a floor apart in its base project and works out every part's solid after each command.
- IFC: `ci/validate_ifc.py` with IfcOpenShell 0.8.5 reports no problems, on the sample and on `ci/ifc-coverage.txt`, which now takes a wall and a floor apart.

---

## 2. The decisions to agree first

### 2.1 A part is an element of its own

`Element::Part(BuildingPart)`, category `part`, with an id and a GUID as every element has (`ids.rs`). It is not a
parameter on a layer, and not a copy of its host:

- **Pieces are many per layer.** Once divided, a layer is many pieces, each with its own data: survey values, material,
  excluded or not. They need their own ids, selection, schedule rows, view graphics and IFC entities.
- **That is Revit's model and IFC's.** In Revit a Part is an element made from a layer of its original. In IFC,
  `IfcBuildingElementPart` is related to its whole by `IfcRelAggregates`.
- **The host stays what it is.** A wall with parts is still a wall: its type, joins, openings, rooms and tags are
  unchanged. Only its drawing and export change, and only where parts are shown or written.

### 2.2 A part stores a recipe, never a shape

A part holds *what it is of*, never geometry:
- `host`: its element, by GUID;
- `source`: a layer of the host's type, by the layer's GUID (which layers already have, `HasPart`), or the whole element. HP-3 adds the division (2.5).
- `parent`: the part it was divided from.

Its solid, thickness, material, net area and net volume are worked out from the host as it stands, every time they are
asked for (`Model::part_solid`, `part_quantities`). So moving, stretching or retyping the wall, or cutting a window in it,
needs no code for parts: nothing is stored that could go stale.

**When a recipe no longer applies, the part is in conflict. It is not deleted, and the model is not refused.** That
happens when the host's type lost the layer or the layer has no thickness, when what the part was divided from is in
conflict, and later when a cut no longer meets the solid. `Model::check` passes. The part stays with its survey data,
and its conflict says what is wrong and how to put it right (`part {id, layer}` points it at a layer of the new type).
A part in conflict is not drawn, counted or exported, and the `parts` command lists it. Losing survey data because a
wall type was edited would be worse than a visible conflict.

### 2.3 The file format: one `FORMAT` step, agreed with the Family Editor's

- **Why a step.** An older build reading an element of an unknown category refuses the whole file (`unknown variant`).
  That would be correct, but it makes the feature branch's files unreadable by every other build while it is developed.
- **What the branch does meanwhile.** `feature/heritage-parts` writes parts under a provisional top-level `parts` key. An
  older build leaves that key out and lists it in `left_out`. FORMAT stays 18, and only a part may point at a part.
- **Why that can't go upstream.** An older build that opens such a file and saves it **drops the parts without a word**.
  `crates/core/CLAUDE.md` counts that as a misreading. So before upstream, parts move into `elements` with a
  `Model::migrate` step (HP-9).
- **The second thing the step must cover:** `PARAMETER_CATEGORIES` gained `part`, so a project parameter for parts is
  refused by an older build.
- **The number.** The Family Editor line holds the next steps: T23 owns 19→20, which also counts FE-12's sweep-profile
  dimensions. **Proposal:** Parts take the first number free when their PR is ready (21 if the family line lands
  first), and nothing is bumped independently before the branches are aligned.

### 2.4 IFC: a decomposed element has no body of its own

- **What is written.** Each part that is not excluded and not in conflict is an `IfcBuildingElementPart`. GlobalId is its
  GUID; `PredefinedType` and `ObjectType` are as the part says; its body is a closed `IfcTriangulatedFaceSet` with its
  material.
- **The relations.** Host → parts and part → divisions are `IfcRelAggregates`, the relation BIMCraft already writes for a
  host's bars.
- **No body on the whole.** The decomposed host has no `Body` (a wall keeps its `Axis`), as a stair has none beside its
  flights. Otherwise the shape would be drawn and measured twice.
- **Quantities and survey data.** `BIMCraft_PartQuantities` (`Width`, `NetArea`, `NetVolume`), since IFC4 has no quantity
  set for parts. `BIMCraft_Heritage` holds the survey values that were set. Material values go out as
  `Pset_MaterialCommon` / `Pset_MaterialThermal`.
- **Vincenzo's choice (10.1): no body on the host, and HP-8 rebuilds it from the parts.** BIMCraft's own IFC import skips a host with no body, and its doors and windows with it,
  listing them in `skipped`. HP-8 should read a decomposed host's shape from its parts. Until then a file exported with
  parts does not come back whole. **The alternative** is to keep the host's body too: simpler import, but every reader
  then sees the wall twice.

### 2.5 Divisions are cuts in the host's own coordinates

- **What a division holds.** A divided part keeps its host and source, plus a `Recipe` of cuts (HP-2) in the host's
  local coordinates, so a cut moves and turns with its host.
- **Pieces are recomputed.** They come from the recipe each time, as everything else does.
- **Keys survive changes.** A piece is named by a stable key (`0.left.0/2.inside.1`: which cut, which side, which lump),
  so its survey data stays with it when the host changes.
- **A cut that no longer meets the solid** is a conflict, not an error (`divide::regenerate`).

### 2.6 Clean-room

- **The decay list is words only.** `Decay_Pathology` takes one of 26 English terms of the usual stone-decay vocabulary,
  or a custom text: Alveolization, Biological Colonization, Blistering, Crack, Crust, Deformation, Delamination, Deposit,
  Detachment, Discolouration, Disintegration, Efflorescence, Encrustation, Erosion, Exfoliation, Fragmentation, Graffiti,
  Missing Part, Moist Area, Patina, Peeling, Pitting, Scaling, Soiling, Staining, Vegetation.
- **No definitions.** No definition or text is taken from NorMaL 1/88, UNI 11182 or ICOMOS-ISCS.
- **No Revit or Archicad content.** Only the behaviour is followed.

---

## 3. What the model gains (`bimcraft-core`)

**`BuildingPart`** (`crates/core/src/parts.rs`) holds:
- `host`, `source` (`Whole` | `Layer(Guid)`) and `parent`;
- `material`, which overrides the layer's;
- `predefined_type`: IFC's `INSULATION`, `PRECASTPANEL`, `USERDEFINED` or `NOTDEFINED`. A thermal layer's part starts as INSULATION.
- `object_type`, which a user-defined part must have, e.g. `SU` or `Decay`;
- `excluded`, as Revit's Exclude Parts;
- `heritage`;
- `params`: the project's parameters.

**The heritage set** has five values, shown by these names in schedules, filters and IFC:

| Name | Type | Meaning |
|---|---|---|
| `SU_Code` | text | the stratigraphic unit's code (masonry or covering unit; USM/USR in Italian practice) |
| `Historical_Period` | text | the construction phase or epoch, in the project's own words |
| `Decay_Pathology` | one of 26 terms, or custom text | the decay seen (2.6) |
| `Decay_Severity` | integer 1–5 | how bad, checked |
| `Intervention_Type` | text | cleaning, consolidation, repointing… |

**Hosts.** A wall, floor, roof or ceiling, one part per layer with some thickness. A component gets one part of all of
it.

**What a part reads from its host (derived, never stored):**
- its solid: a closed mesh in model coordinates, counter-clockwise from outside. A wall's solid has its joins,
  attached top and base, and openings cut through. A floor's, roof's and ceiling's lose their openings and shafts.
- thickness and material;
- net volume, and net area of the face it shows: a wall part's exterior face, but the interior face for a layer inside
  the structure (the interior plaster); the top of a floor's or a roof's part; a ceiling's top, but the underside for a
  layer under its structure (its plaster);
- mass and thermal resistance, where its material says density and conductivity.

`Material` gains three optional values for that: `density`, `conductivity` and `specific_heat`.

**`Model::check`** asks of a part:
- a host that may have parts;
- a parent that is a part of the same host, with no cycles and at most 16 levels;
- a real layer GUID and a material the library has;
- an `object_type` when the part is user-defined;
- a severity from 1 to 5;
- text within `MAX_TEXT`;
- no second part of the same thing.

**What happens to the host's parts:**
- deleting the host deletes its parts, and deleting a part deletes its divisions;
- copying the host copies its parts;
- a part can't be moved on its own;
- a part takes its phase and design option from its host unless it says otherwise.

---

## 4. Commands

| Command | What it does | Step |
|---|---|---|
| `create_parts {ids}` | Revit's Create Parts: makes the parts an element lacks. A wall's run from its exterior side in, the others' from the top down | HP-1 ✔ |
| `part {id \| ids, …}` | sets `material`, `predefined_type`, `object_type`, `excluded`, the five survey values, and `layer` (to put a conflict right); null clears a value | HP-1 ✔ |
| `parts {host?, ids?, conflicts?}` | lists parts with what they hold and what they measure now | HP-1 ✔ |
| `divide_parts {id, cuts, gap?}` | Divide Parts: cuts drawn on a face, planes or profiles; and edge offsets | HP-3 |
| `graphics {view, parts_visibility}` | Show Original / Show Parts / Show Both, on a view or a view template | HP-4 |
| `graphics {view, parts_theme}` (name to settle) | colour parts by a value, with a legend | HP-5 |
| `schedule` with category `part` | schedules of parts, and the "Restoration quantities" starter | HP-6 |

`elements`, `modify`, `delete`, `copy`, `select_by_id` and `filter_selection` already work on parts. Schedules, filters
and tags read a part's values by name (`schedule::fields(m, "part")`): `Host`, `Host Type`, `Layer`, `Function`,
`Material`, `Thickness`, `Net Area`, `Net Volume`, `Mass`, `Thermal Resistance`, the five survey names, `Conflict`, the
phase fields and the project's parameters. A value nobody gave is no value, never 0.

---

## 5. Dividing (HP-2 and HP-3)

`crates/core/src/divide.rs` is pure geometry: no model types, commands or UI. Its write-up is
`docs/areas/parts-geometry.md`.

**Input.** A closed solid and a `Recipe` (serde, `deny_unknown_fields`). The recipe is a list of cuts, each with an
optional gap, then offsets.

**The cuts:**

| Cut | Restoration use |
|---|---|
| a plane at any angle | out-of-plumb or tapering masonry |
| a closed outline drawn on a face, **through** the part or **to a depth** | a zone of rising damp; detached plaster 15 mm deep, with what is behind it left with the rest; a stratigraphic unit |
| an open line across a face | the boundary between two units |
| a closed profile extruded along any frame (the Morph-like cut) | lacunae, niches, irregular volumes |

**Gaps and offsets.**
- A gap takes the material within gap/2 of a cut: a joint.
- An offset moves one face of a finished piece inwards: erosion, missing material.
- Outward offsets are refused.

**What comes out.**
- `divide` gives pieces with stable keys, their volumes, and what was removed.
- `regenerate` does the same for a host that changed. A cut or offset that no longer applies is left out and listed as a
  conflict, and the rest is done.
- For prisms such as a wall's layer, a 2D route (`divide_prism`) is exact. It is checked against the general route on
  random lines.

**Measuring.** `volume`, `area` and `area_facing` give the bill of quantities.

**Robustness.** Hostile input is capped and every step is metered: an error, never a hang. The engine adds `intersect`
beside `csg::subtract`, in the same style, and leaves `subtract` unchanged.

**HP-3** wires the engine to the model. A division becomes a new `Source` holding the recipe and the piece key. The
divided part has no area or volume of its own, because its divisions hold them, so nothing is counted twice.
`divide_parts` checks a recipe once, the way `Session::execute` checks every command.

**Known limits:**
- A curved wall is cut by planes, not along its curve.
- An arc in a sketch is given as points.
- Lines across an opening are drawn as one line.
- A solid of thousands of small non-planar faces is slow, and over a budget it is refused.

---

## 6. Views (HP-4 and HP-5)

**Parts Visibility** (HP-4) is a property of a view's graphics and of view templates, Show Original by default.
- **Compatibility.** A project that never asks is saved unchanged, and an older build leaves the field out.
- **Show Parts.** An element with parts to draw is drawn as its parts; one with a part in conflict stays whole.
- **Show Both** draws both.
- **How parts are drawn.** The 3D scene holds each part's solid beside its host's, and refreshes it when the host,
  what it joins or the part changes. Sections cut parts and elevations see them. Plans draw a part's cut as a layer of
  its host is drawn.
- **Picking.** Parts are picked and highlighted by their own ids, and the edge between two parts in one plane is drawn.

**Colour maps** (HP-5) colour parts by a value, in plan, elevation, section and 3D, with a legend. The value is
`Decay_Pathology` or `Historical_Period`; `Decay_Severity` gets an ordered ramp.
- **Stable colours.** One value has one colour in every view and run. The 26 named decays have a fixed table of colours
  picked far apart in CIELAB (any two a survey uses are told apart; before, Detachment and Biological colonization were
  one green); `Decay_Severity` a ramp; any other value a palette slot by a hash of its text; or the user's choice.
- **The command.** `theme {view, by, colors?, legend?, off?}`; a template holding a Parts Visibility holds the theme too.
- **The legend** lists value, colour, count and total net area, the figure a decay map reports. "Not set" is a row of
  its own.
- **How it is built.** It rides on the existing view filters and overrides (`graphics.rs`, `Look`), not on a parallel
  mechanism.

---

## 7. Quantities (HP-6)

- **A schedule category.** Parts become a category ("Parts"). Excluded parts are left out. Parts in conflict are listed
  with their conflict, so a bill of quantities never drops one silently.
- **Totals.** Grouped by `Intervention_Type` then `Decay_Pathology`, a schedule totals `Net Area`, `Net Volume` and
  `Count` per group and overall. Rows with no value are grouped under a blank heading, not counted as 0.
- **The starter.** A "Restoration quantities" schedule makes this in one command.
- **Export.** The xlsx keeps numbers as numbers, units in the headers and totals rows, so it can be priced.
- **Precision.** Small patches need more than two decimals of m³.
- **Prices are not in scope.** A unit price per `Intervention_Type` can be added later as a project parameter and a
  calculated value.

---

## 8. Desktop (HP-7) and IFC import (HP-8)

**HP-7, the desktop:**
- Create Parts, Exclude/Restore Parts;
- Divide Parts as a sketch on the selected part's face, drawn with the project's Draw gallery and sketch mode, plus a plane and a profile tool;
- Properties of a part with the survey values;
- the Parts Visibility selector and the theme in the view's Properties;
- the material values in the Material Browser.
Each tool drives a command; the UI stays thin.

**HP-8, IFC import:**
- read `IfcBuildingElementPart` and its `IfcRelAggregates`;
- rebuild a decomposed host's shape from its parts (2.4);
- read the `BIMCraft_Heritage` set and material values back;
- a round-trip test: export, import, compare.

---

## 9. Order, and how it goes upstream

- **In order:** HP-2 → HP-3 → HP-4/5 → HP-6 → HP-9 → HP-8 → HP-7. Within the workstream, each step is a branch and a
  fork PR into `feature/heritage-parts`, reviewed before it is merged there.
- **Upstream**, if the maintainers agree with section 2, in PRs that each leave everything working:

| PR | Contents | Depends on |
|---|---|---|
| 1 | **The part element**: HP-1 with the `FORMAT` step (HP-9) inside it, so no build ever writes the provisional layout | the number agreed with the family line |
| 2 | **Divide Parts**: HP-2 + HP-3 | 1 |
| 3 | **Parts in views**: HP-4 + HP-5 | 1 (2 for divided pieces) |
| 4 | **Schedules of parts**: HP-6 | 1 |
| 5 | **IFC import of parts**: HP-8 | 1 |
| 6 | **The desktop tools**: HP-7 | 2, 3 |

- **The discussion.** Section 2 goes to an upstream discussion first, as #217 and #267 did for the Family Editor, only
  with Vincenzo's and Emiliano's OK.

**What every step is checked with:**
- tests first;
- the hostile sweep extended to each new command;
- `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings`;
- `cargo test --workspace`, with the desktop's GPU tests run serially (`gpu:: retained:: --test-threads=1`), since they
  hang when run concurrently on a software renderer;
- `cargo i18n update` / `check`;
- IFC through `ci/validate_ifc.py`;
- drawings looked at as `docs/areas/looking-at-results.md` says.

---

## 10. Open questions

Answered by Vincenzo on 2026-11-11 (the IFC one still for the maintainers to agree, as 2.4):
1. ~~IFC body of a decomposed host~~: **no body on the host; HP-8 rebuilds it from the parts.**
2. ~~A wall part's net area~~: **each layer on the face it shows**: a wall's interior plaster on the interior face, a
   ceiling's plaster on its underside (#19).
3. ~~The decay list~~: **the 26 terms stand.**
4. ~~`SU_Code` or `MSU_Code`~~: **`SU_Code`.**

Still open:
5. **The `FORMAT` number** (2.3), with the family line.
6. **Parts in a project brought in by Insert from File**: a type with the same name but different layer GUIDs puts its
   parts in conflict, which `part {layer}` fixes. Is that acceptable?
7. **`csg::MAX_TRIANGLES`** (20 000) can be reached by a large floor's part. Raise it for parts, or cut near the cutter
   only (`parts-geometry.md`)?
8. **A theme's legend in an elevation** counts every part from its line to its far clip, hidden behind others or not
   (a box seen from the south lists the north wall's parts too). Count only what the elevation shows?

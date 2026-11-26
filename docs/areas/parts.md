# Parts of elements and the heritage set

Read when touching `crates/core/src/parts.rs`, `parts/solid.rs`, `parts/state.rs`, `parts/drawn.rs`, the `create_parts`, `part` and `parts`
commands, how a part goes to IFC and comes back (`ifc.rs`, `import/parts.rs`), or the desktop's parts tools
(`crates/ui-egui/src/parts.rs`, `divide.rs`). Aimed at the recording and restoration of historic buildings (HBIM): a wall is its
plaster and its masonry, and each is described, dated, measured and treated on its own.

## What a part is

A part (`Element::Part(BuildingPart)`, category `part`) is an element of its own, with an id and a GUID, that is a piece of
another: a wall, a floor, a roof, a ceiling or a component (`parts::HOSTS`), as Revit's Parts. It holds a **recipe, never a
shape**: its `host` (by GUID), its `source` (`Source::Layer(guid)`, a layer of the host's type by the layer's own GUID, or
`Source::Whole`, all of it), the part it was divided from (`parent`, none for one made straight from its host), a `material` of
its own in place of its layer's, IFC's kind of part (`predefined_type`: insulation, precastpanel, userdefined, notdefined, with
`object_type`, which a user-defined one must say), `excluded` (Revit's Exclude Parts: it stays, and is left out of what is
exported), the heritage set, and its values of the project's parameters (`params`; `part` is one of
`PARAMETER_CATEGORIES`). `Source` is an enum so that a division can be one more kind of source.

Everything else is **worked out from the host as it stands, each time it is asked for** (`parts/state.rs`), so nothing goes
stale: move the wall, stretch it, put a window in it, change its type's layer thicknesses, and its parts are what they are of
it now. `Model::part_solid(&BuildingPart) -> Result<Vec<[P3; 3]>, String>` is the solid: a closed surface of triangles in
**model coordinates** (mm, x east, y north, z up; not the host's axes), each counter-clockwise seen from outside, so its normal
points out. A wall's part is its layer between the layer's two faces (cut as the 3D view cuts a wall into layers: by the
distance from the location line, the outermost and innermost layers taking what lies past them at the ends), with the ends the
wall's joins give it, its top and base where what it is attached to puts them, and its doors and windows cut through. A
floor's, a roof's or a ceiling's is its layer between two planes, from the top down, less its openings and the shafts through
it; over a variable layer those above follow a shaped top and those below stay level. A component's is its family's forms.
The host's prisms (`walls::prisms`, `floor_prisms`, `roof_prisms`) are welded into one surface by `solid::mesh`: no face lies
inside it and every edge of a triangle is another's the other way round (`solid::closed`), which is what `csg::subtract` takes
and IFC calls a closed face set. `Model::part_prisms` gives the prisms themselves, for what would rather cut in plan.

`Model::part_quantities` reads the **net volume** and the **net area** off that solid, so openings, shafts, joins and, later,
divisions, gaps and offsets are in them with no code of their own. The volume is the solid's (`solid::volume`). The area is
that of the solid's faces that lie in the plane of the layer's own face and look the way it does (`solid::facing`,
`Model::part_facing`): for a wall's part the face on its own side of the wall (its elevation, less doors and windows): a layer past the last
structure layer, the interior plaster, on the wall's interior face, any other on its exterior face (the structure, as all of a
wall is, on its exterior one; a type with no structure layer has its thickest stand for it; `parts/state.rs`'s `interior`), for
a floor's, a roof's or a ceiling's part its top (less openings and shafts; a sloped one's along its slope), but for a
ceiling's layer under its structure, its plaster, the underside, the face it shows. A component's part
has a volume and no area. `part_layer`, `part_material` (its own, else its layer's, else its host's structure's),
`part_thickness`, `parts_of`, `part_divisions` and `part_state` (all of it at once) give the rest. A material may say how heavy
it is and how it carries and holds heat (`Material::density`, kg/m³, `conductivity`, W/(m·K), `specific_heat`, J/(kg·K); the
`material` command, each null for not said, written to a file only when said): a part made of it then shows them, its
`mass_kg` (density by its net volume) and, one layer of one material, its `thermal_resistance` (its thickness in metres over
the conductivity, m²·K/W).

## Conflicts

When the recipe no longer applies, the part is **in conflict**: it stays in the model, `Model::check` passes, and
`Model::part_conflict` (and `conflict` in every listing) says why and what to do. The cases: the host's type has no layer of
that GUID (the type was swapped, or lost the layer; a type's layers keep their GUIDs by place when its list is given again);
the layer has no thickness; the host has no layers at all; the part it was divided from is in conflict; or it is all of an
element that has parts of its layers too, which would be counted twice. A part in conflict has no solid, measures nothing and
is not exported. `part {id, layer: n}` says which layer of the host's type it is now, which keeps what was said of it.
`Model::check` asks only what never depends on the host's state: a host of a kind that has parts, a parent that is another part
of the same host and none of its own divisions, a layer named by a real GUID, a material of the library, an `object_type` for a
user-defined part, a severity of 1 to 5, text within `MAX_TEXT`, and no second part of the same thing.

Deleting an element deletes its parts, as Revit deletes them with the original, and a part's divisions go with it; copying an
element copies them (and what was said of them); a part has no place of its own to move. A part with no phase or design
option of its own is in its host's (`phasing::value`), and `Session::stamp` gives a new part none.

## Commands

`create_parts {ids}` (Revit's Create Parts) makes a part for each layer with some thickness of each element's type, or one of
all of it where the type has no layers or the element is a component; a thermal layer's starts as INSULATION. An element that
has parts gets those it lacks; refused when none lacks any. `part {id | ids, …}` sets what parts say: `material`,
`predefined_type`, `object_type`, `excluded`, the heritage set (keys in any case) and `layer`. `parts {host?, ids?, conflicts?}`
lists them with what they hold and what they are now. `modify` and `elements` work on a part as on any element.

## Divide Parts

`divide_parts {id | ids, cuts, offsets?}` (Revit's Divide Parts) cuts a part into pieces with the engine of `divide.rs`
([parts-geometry](parts-geometry.md)): the command takes a `Recipe`'s `cuts` and `offsets` as the engine serializes them,
in model millimetres, divides the part's solid strictly (`divide::divide`: a cut that misses is refused, nothing changes),
and keeps the recipe on the part (`BuildingPart::division`) in its host's own coordinates (`Model::element_frame`: a wall's
start and the way it runs, a slab's first corner and side, a component's place and angle, at its level's elevation;
`HostFrame::local_recipe` and `model_recipe`). Each piece is a part of its own with the same host and source, its
`parent` the divided part and `piece` its `PieceKey`; it starts with the divided part's material, kind, heritage values and
parameters. Its solid is worked out when asked for (`Model::part_division`, then the piece by its key, brought back to the
model), so the pieces follow the host as the part does; a cut that no longer meets the solid is left out
(`divide::regenerate`), listed on the divided part as `division_conflicts`, and a piece it no longer makes is in conflict.
Divided again, a part keeps the pieces whose keys the new recipe still makes (ids and values) and the others are deleted,
with what was divided from them; `cuts: []` makes it whole. A part divided by hand (`parent` without `piece`) is refused,
since its pieces would be counted with the division's. The divided part is the sum of its pieces, as HP-1 already counted
it: listings and schedules count the pieces.

What is approximate: each piece works its parent's division out again, so a part of n pieces is divided n times when all
are drawn or listed; a mirrored component's frame doesn't mirror, so a division of one follows it turned, not mirrored.

## The heritage set

Five values every part may have (`Heritage`), shown everywhere under `Heritage::NAMES`: `SU_Code` (the stratigraphic unit's
code), `Historical_Period` (in the project's own names), `Decay_Pathology` (`Decay`: one of a list of the words conservation
uses for forms of decay, or `Custom(text)`; the words only, never a standard's definitions), `Decay_Severity` (1 to 5) and
`Intervention_Type`. Anything else is a project parameter for parts.

## Read by name

`schedule::fields(m, "part")` lists a part's fields and `schedule::field_text`, `picks` and `check_rules` read them from
`parts::json` (what `elements` shows): `Id`, `Host`, `Host Category`, `Host Type`, `Level`, `Layer`, `Function`, `Material`,
`Thickness`, `Net Area`, `Net Volume`, `Density`, `Mass`, `Thermal Conductivity`, `Thermal Resistance`, `Specific Heat`,
`Predefined Type`, `Object Type`, `Excluded`, the five heritage names, `Conflict`,
`Count`, the phases and the design option. A value nobody gave is no value, never 0 (`From::Held`). A part that parts were
divided from lists them (`divisions`) and no area or volume: they hold them.

## Schedules

Parts are a category schedules are of (`schedule {category: "parts"}`), and a schedule of them grouped by `Intervention_Type`
or `Decay_Pathology`, with its net areas, net volumes and counts totalled, is the restoration's bill of quantities:
`schedule {starter: "restoration-quantities"}` makes it in one go, and `path` writes it as CSV or a workbook. Excluded parts
are left out; parts in conflict are listed with no area or volume, and named in the table's `notes`. The detail is in
[schedules](schedules.md#parts-the-restorations-bill-of-quantities).

## IFC

**Out** (`ifc.rs`). Each part an export writes (`parts::exported`: not excluded, not in conflict, and not divided from one it
leaves out) is an `IfcBuildingElementPart`: its GUID its GlobalId, its id its tag, placed where its host is, its body a closed
face set in its host's own axes, shaded in and associated with what it is made of (named by that, else by its host's type),
its `PredefinedType` and `ObjectType` its own (IFC2x3 has no kind of part). `BIMCraft_PartQuantities` holds its `Width`,
`NetArea` and `NetVolume`, since IFC4 has no quantity set for a part; `BIMCraft_Heritage` the heritage values said. Host to
parts and part to pieces are `IfcRelAggregates`, as a host's bars are. What is the sum of parts (a host with any written, a
divided part) has no `Body` of its own (a wall keeps its `Axis`), so nothing is drawn or measured twice, and its base quantities
say its lengths only, no area or volume (the maintainers' conditions on #440). A divided part says how in
`BIMCraft_PartDivision` (`Division`: its recipe as JSON, in millimetres in its own placement's axes), which only BIMCraft reads.
A material's weight and heat go out as `Pset_MaterialCommon` and `Pset_MaterialThermal` (IFC2x3: `IfcGeneralMaterialProperties`
and `IfcThermalMaterialProperties`), in the derived units the project names once a material says any. `ci/ifc-coverage.txt`
has parts, a division and a material's values, for IfcOpenShell in all three schemas.

**Back** (`import.rs`, `import/parts.rs`). A product with no body of its own that is the sum of `IfcBuildingElementPart`s
(BIMCraft's, or Revit's shown by parts) takes their bodies as its own (`Reader::items`), so a wall, a slab, a roof, a covering
or a component comes in measured from its parts; the layer set says how thick it is even where a part is missing. Once
everything is in, the element is taken apart again as `create_parts` would, and each part of the file is made the part whose
box holds the middle of its own, the nearest first; only where the file has one, so a part an export left out stays out. Each
takes the file's GlobalId, kind, object type, material (where it isn't its layer's), heritage set, project parameters and
phase. A part whose division BIMCraft reads is divided so again, its pieces matched in their turn; a division the file doesn't
say (another program's) or that can't be read leaves the part whole and lists its pieces in `skipped`, as a part where no part
of its element is. A part names its layer by the layer's GUID, which the import gives anew at the end (`relayer`). Materials
take their weight and heat back. `ifc_parts_tests.rs` exports, imports and compares a wall with a window and a door, a floor,
a roof, a ceiling, a component, a divided part, and IFC2x3.

**Not yet**: an excluded part isn't in the file, so it doesn't come back, nor that it was excluded; a floor whose top layer's
part was excluded comes back that much lower (its top is its parts'), and a roof or a ceiling the same at the face it is read
by; a part divided by hand (`parent` without `piece`) comes back whole.

## The file

Parts are elements (category `part`) since `FORMAT` 21. A BIMCraft that reads up to 20 refuses such a file as a newer one's,
rather than leaving the parts out and dropping them when it saves. A project parameter for parts names the category `part`.
Builds of the heritage fork kept parts under a top-level `parts` key in a file of 18; that key is one this BIMCraft doesn't
know, so it leaves it out and names it (`left_out`).

## In views

A view draws an element whole, as its parts, or both: Revit's Parts Visibility (`graphics {view, parts_visibility: original |
parts | both}`, Properties' Parts Visibility), Show Original unless it says. How a plan, a section, an elevation and the 3D
view draw and pick a part, what stands for what, and what is left for later are in
[views and graphics](views-and-graphics.md#parts-in-views). A view's Properties set it, and below it Color Parts By (the
view's `theme`: none, one of the heritage set's names, Material, or a project parameter of parts) and, once the view colours
its parts, Parts Legend (`theme.legend`); a template that sets a view's parts visibility sets these too, and greys them out.

## In the desktop

`crates/ui-egui/src/parts.rs`, each action a command, tested in `tests/parts.rs` (`BIMCRAFT_PARTS_DUMP=<folder>` writes
screenshots of them in both profiles, light and dark):
- **Create Parts**, the Modify tab's Create panel while walls, floors, roofs, ceilings or components are selected: `create_parts`
  on them and, as Revit's, the view set to Show Parts (unless it shows them, or its template says what it shows), one undo
  step; the parts made are then selected.
- **Exclude Parts** and **Restore Parts**, the Part panel while parts are selected: `part {ids, excluded}`, each greyed out
  while none of the selection is in or out.
- **A part's Properties**: Identity Data (Original Category and Original Type, its Layer and what that does, Material, its own
  or By Original, which is its layer's; Kind, IFC's kind; Object Type; Excluded), Dimensions (Thickness, Area, Volume, as
  `parts` works them out; a divided part's are its pieces') and Heritage (the set by its names: a decay of the list's words,
  or Other Decay for one in the project's own; Decay_Severity 1 to 5). Of several parts, what they share is shown and a value
  set goes to all of them, through `part`; what the core refuses is said in the status bar and changes nothing.
- **The Materials window** says a material's Density (kg/m³), Thermal Conductivity (W/(m·K)) and Specific Heat (J/(kg·K)),
  each blank for not said (`material {density, conductivity, specific_heat}`).
- **Divide Parts** and **Edit Division**, the Part panel (`crates/ui-egui/src/divide.rs`, tested in `tests/divide.rs`):
  - Divide Parts, while the parts selected are neither divided nor pieces, opens the 3D view and waits for a click on a face
    of them (`Scene::face_at`). Where the view shows the original, a face of the host is taken only where one of the parts has
    a face in its plane, so a depth is measured from the part's face. Esc gives up.
  - The view turns square to the face, parallel and fitted to the parts, and the sketch begins on it (`Of::Divide`): its
    points are in the face's own axes (`sketch3d::Surface::Face`, x level, y up the face), drawn with the Draw panel's shapes,
    snapped to the sketch's own ends. The Options Bar has Gap, the joint's width, and Depth, how far in an outline cuts
    (blank: through).
  - Finish chains the lines end to end (`parts::drawn::redivided`): a chain that closes, or a closed shape, is an outline,
    one that doesn't a line across; curves go as their points. Each part gets `divide_parts` with its cuts, one undo step that
    also shows parts in the 3D view; the pieces are then selected. What the core refuses is said, the sketch kept. Lines that
    branch are refused before that. Cancel leaves the parts as they were, the selection back.
  - Edit Division, while a divided part or pieces of one part are selected, takes up the division's sketch on the face its
    first line was drawn on (`parts::drawn::sketched`), with its Gap and Depth. A line left as it was keeps its cut's place
    in the division, so its pieces keep their keys, ids and values; new lines go last; cuts drawn elsewhere, planes and
    profiles, and offsets stay as they are.

## Next

Dividing by planes (levels, grids, reference planes) and by profiles in the desktop (HP-7c), sketching a division in an
elevation, snapping to the part's edges while sketching, tags of parts, the theme's own
colours chosen in the desktop, its key placed clear of the window's view tabs, and a slab whose outer layer's part is excluded
read back where it was.
